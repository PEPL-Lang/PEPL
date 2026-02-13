//! PEPL Type Checker — walks a parsed AST and validates types.
//!
//! Entry point: [`TypeChecker::check`].
//!
//! Error codes emitted:
//! - E200: unknown type / `any` in user code
//! - E201: type mismatch
//! - E202: wrong argument count
//! - E210: non-exhaustive match
//! - E400: undeclared capability
//! - E401: capability unavailable
//! - E500: variable already declared
//! - E501: `set` outside action / capability used in view
//! - E601: derived field modification
//! - E604: undeclared credential
//! - E605: credential modification

use std::collections::{HashMap, HashSet};

use pepl_types::ast::*;
use pepl_types::{CompileErrors, ErrorCode, SourceFile, Span};

use crate::env::{ScopeKind, TypeEnv};
use crate::stdlib::{self, StdlibRegistry};
use crate::ty::{FnSig, RecordField, SumVariant, Type};

// ══════════════════════════════════════════════════════════════════════════════
// TypeChecker
// ══════════════════════════════════════════════════════════════════════════════

/// Walks a parsed [`Program`] and validates all types.
pub struct TypeChecker<'a> {
    env: TypeEnv,
    errors: &'a mut CompileErrors,
    source: &'a SourceFile,
    stdlib: StdlibRegistry,
    /// User-defined sum types: name → variants.
    sum_types: HashMap<String, Vec<SumVariant>>,
    /// State field names → types (for `set` target validation).
    state_fields: HashMap<String, Type>,
    /// Derived field names → types (read-only).
    derived_fields: HashMap<String, Type>,
    /// Declared action names.
    action_names: HashSet<String>,
    /// Declared required capabilities.
    required_capabilities: HashSet<String>,
    /// Declared optional capabilities.
    optional_capabilities: HashSet<String>,
    /// Declared credential names → types.
    credentials: HashMap<String, Type>,
    /// Capability module mapping (e.g. "http" → "http").
    capability_modules: HashMap<&'static str, &'static str>,
}

impl<'a> TypeChecker<'a> {
    /// Create a new type checker.
    pub fn new(errors: &'a mut CompileErrors, source: &'a SourceFile) -> Self {
        Self {
            env: TypeEnv::new(),
            errors,
            source,
            stdlib: StdlibRegistry::new(),
            sum_types: HashMap::new(),
            state_fields: HashMap::new(),
            derived_fields: HashMap::new(),
            action_names: HashSet::new(),
            required_capabilities: HashSet::new(),
            optional_capabilities: HashSet::new(),
            credentials: HashMap::new(),
            capability_modules: stdlib::capability_modules(),
        }
    }

    /// Type-check a complete program.
    pub fn check(&mut self, program: &Program) {
        self.check_space(&program.space);
        for test_block in &program.tests {
            self.check_tests_block(test_block);
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Space-level
    // ══════════════════════════════════════════════════════════════════════

    fn check_space(&mut self, space: &SpaceDecl) {
        let body = &space.body;

        // 1. Register user-defined sum types
        for td in &body.types {
            self.register_type_decl(td);
        }

        // 2. Register state fields
        for field in &body.state.fields {
            let ty = self.resolve_type_annotation(&field.type_ann);
            self.state_fields.insert(field.name.name.clone(), ty.clone());
            self.env.define(&field.name.name, ty);
        }

        // 3. Register capabilities
        if let Some(caps) = &body.capabilities {
            for cap in &caps.required {
                self.required_capabilities.insert(cap.name.clone());
            }
            for cap in &caps.optional {
                self.optional_capabilities.insert(cap.name.clone());
            }
        }

        // 4. Register credentials
        if let Some(creds) = &body.credentials {
            for field in &creds.fields {
                let ty = self.resolve_type_annotation(&field.type_ann);
                self.credentials.insert(field.name.name.clone(), ty.clone());
                self.env.define(&field.name.name, ty);
            }
        }

        // 5. Check state field initializers (pure stdlib only)
        for field in &body.state.fields {
            self.check_state_initializer(&field.default, &field.name.name, field.span);
        }

        // 6. Check derived fields (in order — each can reference prior derived)
        if let Some(derived) = &body.derived {
            for field in &derived.fields {
                let declared_ty = self.resolve_type_annotation(&field.type_ann);
                self.env.push_scope(ScopeKind::Derived);
                let inferred_ty = self.check_expr(&field.value);
                self.env.pop_scope();

                if !inferred_ty.is_assignable_to(&declared_ty) {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "derived field '{}' declared as {} but expression has type {}",
                            field.name.name, declared_ty, inferred_ty
                        ),
                        field.value.span,
                    );
                }

                self.derived_fields
                    .insert(field.name.name.clone(), declared_ty.clone());
                self.env.define(&field.name.name, declared_ty);
            }
        }

        // 7. Check invariants
        for inv in &body.invariants {
            self.env.push_scope(ScopeKind::Invariant);
            let ty = self.check_expr(&inv.condition);
            self.env.pop_scope();

            if !ty.is_bool() {
                self.error(
                    ErrorCode::TYPE_MISMATCH,
                    format!(
                        "invariant '{}' condition must be bool, got {}",
                        inv.name.name, ty
                    ),
                    inv.condition.span,
                );
            }
        }

        // 8. Register action names and check action declarations
        for action in &body.actions {
            self.action_names.insert(action.name.name.clone());
        }
        for action in &body.actions {
            self.check_action(action);
        }

        // 9. Check views
        for view in &body.views {
            self.check_view(view);
        }

        // 10. Check update
        if let Some(update) = &body.update {
            self.check_update(update);
        }

        // 11. Check handleEvent
        if let Some(handle_event) = &body.handle_event {
            self.check_handle_event(handle_event);
        }
    }

    fn register_type_decl(&mut self, td: &TypeDecl) {
        match &td.body {
            TypeDeclBody::SumType(variants) => {
                let sum_variants: Vec<SumVariant> = variants
                    .iter()
                    .map(|v| SumVariant {
                        name: v.name.name.clone(),
                        params: v
                            .params
                            .iter()
                            .map(|p| {
                                (
                                    p.name.name.clone(),
                                    self.resolve_type_annotation(&p.type_ann),
                                )
                            })
                            .collect(),
                    })
                    .collect();
                self.sum_types
                    .insert(td.name.name.clone(), sum_variants.clone());

                let named_ty = Type::Named(td.name.name.clone());

                // Register the type name in the environment
                self.env.define(
                    &td.name.name,
                    Type::SumType {
                        name: td.name.name.clone(),
                        variants: sum_variants.clone(),
                    },
                );

                // Register each variant constructor as an identifier
                for variant in &sum_variants {
                    if variant.params.is_empty() {
                        // Zero-arg variant: value of the named type
                        self.env.define(&variant.name, named_ty.clone());
                    } else {
                        // Parameterised variant: function → named type
                        let param_types: Vec<Type> =
                            variant.params.iter().map(|(_, ty)| ty.clone()).collect();
                        self.env.define(
                            &variant.name,
                            Type::Function(param_types, Box::new(named_ty.clone())),
                        );
                    }
                }
            }
            TypeDeclBody::Alias(type_ann) => {
                let aliased = self.resolve_type_annotation(type_ann);
                self.env.define(&td.name.name, aliased);
            }
        }
    }

    fn check_state_initializer(&mut self, expr: &Expr, field_name: &str, _span: Span) {
        // State initializers may only use literals and pure stdlib calls
        // (no capability calls, no state field references)
        match &expr.kind {
            ExprKind::NumberLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::NilLit => {}
            ExprKind::ListLit(items) => {
                for item in items {
                    self.check_state_initializer(item, field_name, _span);
                }
            }
            ExprKind::RecordLit(entries) => {
                for entry in entries {
                    match entry {
                        RecordEntry::Field { value, .. } => {
                            self.check_state_initializer(value, field_name, _span);
                        }
                        RecordEntry::Spread(expr) => {
                            self.check_state_initializer(expr, field_name, _span);
                        }
                    }
                }
            }
            ExprKind::StringInterpolation(_) => {
                // Allow string interpolation with literals
            }
            ExprKind::Unary { operand, .. } => {
                self.check_state_initializer(operand, field_name, _span);
            }
            ExprKind::QualifiedCall { module, function, args } => {
                // Pure stdlib calls are allowed
                if self.capability_modules.contains_key(module.name.as_str()) {
                    self.error(
                        ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
                        format!(
                            "state initializer for '{}' cannot call capability function '{}.{}'",
                            field_name, module.name, function.name
                        ),
                        expr.span,
                    );
                }
                for arg in args {
                    self.check_state_initializer(arg, field_name, _span);
                }
            }
            ExprKind::Identifier(name) => {
                // Cannot reference other state fields
                if self.state_fields.contains_key(name) {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "state initializer for '{}' cannot reference state field '{}'",
                            field_name, name
                        ),
                        expr.span,
                    );
                }
            }
            _ => {
                // Other expressions in state initializers are questionable
                // but we'll allow them for now and let type inference catch issues
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Actions
    // ══════════════════════════════════════════════════════════════════════

    fn check_action(&mut self, action: &ActionDecl) {
        self.env.push_scope(ScopeKind::Action);

        // Register parameters
        for param in &action.params {
            let ty = self.resolve_type_annotation(&param.type_ann);
            if !self.env.define(&param.name.name, ty) {
                self.error(
                    ErrorCode::VARIABLE_ALREADY_DECLARED,
                    format!("parameter '{}' already declared", param.name.name),
                    param.span,
                );
            }
        }

        self.check_block(&action.body);
        self.env.pop_scope();
    }

    // ══════════════════════════════════════════════════════════════════════
    // Views
    // ══════════════════════════════════════════════════════════════════════

    fn check_view(&mut self, view: &ViewDecl) {
        self.env.push_scope(ScopeKind::View);

        for param in &view.params {
            let ty = self.resolve_type_annotation(&param.type_ann);
            if !self.env.define(&param.name.name, ty) {
                self.error(
                    ErrorCode::VARIABLE_ALREADY_DECLARED,
                    format!("parameter '{}' already declared", param.name.name),
                    param.span,
                );
            }
        }

        self.check_ui_block(&view.body);
        self.env.pop_scope();
    }

    fn check_ui_block(&mut self, block: &UIBlock) {
        for element in &block.elements {
            self.check_ui_element(element);
        }
    }

    fn check_ui_element(&mut self, element: &UIElement) {
        match element {
            UIElement::Component(comp) => {
                // Check prop expressions
                for prop in &comp.props {
                    // Action references in on_tap/on_change etc. are just identifiers
                    // that resolve to action names — they don't need to type-check as
                    // expressions. Check if this looks like an action reference.
                    if prop.name.name.starts_with("on_") {
                        if let ExprKind::Identifier(name) = &prop.value.kind {
                            if self.action_names.contains(name) {
                                continue; // Valid action reference
                            }
                        }
                    }
                    self.check_expr(&prop.value);
                }
                if let Some(children) = &comp.children {
                    self.check_ui_block(children);
                }
            }
            UIElement::Let(binding) => {
                self.check_let_binding(binding);
            }
            UIElement::If(ui_if) => {
                self.check_ui_if(ui_if);
            }
            UIElement::For(ui_for) => {
                self.check_ui_for(ui_for);
            }
        }
    }

    fn check_ui_if(&mut self, ui_if: &UIIf) {
        let cond_ty = self.check_expr(&ui_if.condition);
        if !cond_ty.is_bool() {
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!("if condition must be bool, got {}", cond_ty),
                ui_if.condition.span,
            );
        }
        self.check_ui_block(&ui_if.then_block);
        if let Some(else_block) = &ui_if.else_block {
            match else_block {
                UIElse::ElseIf(elif) => self.check_ui_if(elif),
                UIElse::Block(block) => self.check_ui_block(block),
            }
        }
    }

    fn check_ui_for(&mut self, ui_for: &UIFor) {
        let iter_ty = self.check_expr(&ui_for.iterable);

        self.env.push_scope(ScopeKind::Block);

        match &iter_ty {
            Type::List(elem_ty) => {
                self.env.define(&ui_for.item.name, *elem_ty.clone());
            }
            Type::Any | Type::Unknown => {
                self.env.define(&ui_for.item.name, Type::Any);
            }
            _ => {
                self.error(
                    ErrorCode::TYPE_MISMATCH,
                    format!("for loop requires list type, got {}", iter_ty),
                    ui_for.iterable.span,
                );
                self.env.define(&ui_for.item.name, Type::Unknown);
            }
        }

        if let Some(index) = &ui_for.index {
            self.env.define(&index.name, Type::Number);
        }

        self.check_ui_block(&ui_for.body);
        self.env.pop_scope();
    }

    // ══════════════════════════════════════════════════════════════════════
    // Update & HandleEvent
    // ══════════════════════════════════════════════════════════════════════

    fn check_update(&mut self, update: &UpdateDecl) {
        self.env.push_scope(ScopeKind::Update);
        let ty = self.resolve_type_annotation(&update.param.type_ann);
        if !ty.is_numeric() {
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!("update parameter must be number, got {}", ty),
                update.param.span,
            );
        }
        self.env.define(&update.param.name.name, ty);
        self.check_block(&update.body);
        self.env.pop_scope();
    }

    fn check_handle_event(&mut self, he: &HandleEventDecl) {
        self.env.push_scope(ScopeKind::HandleEvent);
        let ty = self.resolve_type_annotation(&he.param.type_ann);
        self.env.define(&he.param.name.name, ty);
        self.check_block(&he.body);
        self.env.pop_scope();
    }

    // ══════════════════════════════════════════════════════════════════════
    // Tests
    // ══════════════════════════════════════════════════════════════════════

    fn check_tests_block(&mut self, tests: &TestsBlock) {
        for case in &tests.cases {
            self.env.push_scope(ScopeKind::TestCase);
            self.check_block(&case.body);
            self.env.pop_scope();
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Blocks & Statements
    // ══════════════════════════════════════════════════════════════════════

    fn check_block(&mut self, block: &Block) {
        self.env.push_scope(ScopeKind::Block);
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        self.env.pop_scope();
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Set(set) => self.check_set(set),
            Stmt::Let(binding) => self.check_let_binding(binding),
            Stmt::If(if_expr) => {
                self.check_if_expr(if_expr);
            }
            Stmt::For(for_expr) => {
                self.check_for_expr(for_expr);
            }
            Stmt::Match(match_expr) => {
                self.check_match_expr(match_expr);
            }
            Stmt::Return(ret) => {
                if !self.env.in_action() {
                    self.error(
                        ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
                        "return is only allowed inside actions".to_string(),
                        ret.span,
                    );
                }
            }
            Stmt::Assert(assert_stmt) => {
                let ty = self.check_expr(&assert_stmt.condition);
                if !ty.is_bool() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!("assert condition must be bool, got {}", ty),
                        assert_stmt.condition.span,
                    );
                }
            }
            Stmt::Expr(expr_stmt) => {
                self.check_expr(&expr_stmt.expr);
            }
        }
    }

    fn check_set(&mut self, set: &SetStmt) {
        // set only inside actions
        if !self.env.in_action() && !self.env.in_test() {
            self.error(
                ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
                "set is only allowed inside actions".to_string(),
                set.span,
            );
        }
        // Cannot set in views (purity)
        if self.env.in_view() {
            self.error(
                ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
                "set is not allowed inside views (views must be pure)".to_string(),
                set.span,
            );
        }

        let target_name = &set.target[0].name;

        // Cannot set derived fields
        if self.derived_fields.contains_key(target_name) {
            self.error(
                ErrorCode::DERIVED_FIELD_MODIFIED,
                format!(
                    "derived field '{}' is read-only — it recomputes automatically",
                    target_name
                ),
                set.span,
            );
            return;
        }

        // Cannot set credentials
        if self.credentials.contains_key(target_name) {
            self.error(
                ErrorCode::CREDENTIAL_MODIFIED,
                format!("credential '{}' is read-only", target_name),
                set.span,
            );
            return;
        }

        // Target must be a state field
        if !self.state_fields.contains_key(target_name) {
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!("'{}' is not a declared state field", target_name),
                set.target[0].span,
            );
            return;
        }

        // Type-check the value
        let value_ty = self.check_expr(&set.value);

        // For nested set (set x.y = ...), resolve through the field chain
        let mut target_ty = self.state_fields.get(target_name).cloned().unwrap_or(Type::Unknown);
        for field_ident in set.target.iter().skip(1) {
            match &target_ty {
                Type::Record(fields) => {
                    if let Some(rf) = fields.iter().find(|f| f.name == field_ident.name) {
                        target_ty = rf.ty.clone();
                    } else {
                        self.error(
                            ErrorCode::TYPE_MISMATCH,
                            format!("record has no field '{}'", field_ident.name),
                            field_ident.span,
                        );
                        return;
                    }
                }
                Type::Any | Type::Unknown => break,
                _ => {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "cannot access field '{}' on type {}",
                            field_ident.name, target_ty
                        ),
                        field_ident.span,
                    );
                    return;
                }
            }
        }

        if !value_ty.is_assignable_to(&target_ty) {
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!(
                    "cannot assign {} to '{}' (expected {})",
                    value_ty, target_name, target_ty
                ),
                set.value.span,
            );
        }
    }

    fn check_let_binding(&mut self, binding: &LetBinding) {
        let value_ty = self.check_expr(&binding.value);

        if let Some(type_ann) = &binding.type_ann {
            let declared_ty = self.resolve_type_annotation(type_ann);

            // Reject `any` in user code
            if matches!(declared_ty, Type::Any) {
                self.error(
                    ErrorCode::UNKNOWN_TYPE,
                    "'any' cannot be used in type annotations".to_string(),
                    type_ann.span,
                );
            }

            if !value_ty.is_assignable_to(&declared_ty) {
                self.error(
                    ErrorCode::TYPE_MISMATCH,
                    format!(
                        "let binding type mismatch: declared {}, got {}",
                        declared_ty, value_ty
                    ),
                    binding.value.span,
                );
            }
        }

        if let Some(name) = &binding.name {
            let ty = if let Some(ann) = &binding.type_ann {
                self.resolve_type_annotation(ann)
            } else {
                value_ty
            };

            // Check for shadowing in the SAME scope
            if self.env.defined_in_current_scope(&name.name) {
                self.error(
                    ErrorCode::VARIABLE_ALREADY_DECLARED,
                    format!("variable '{}' already declared in this scope", name.name),
                    name.span,
                );
            } else {
                self.env.define(&name.name, ty);
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Expression Type Inference
    // ══════════════════════════════════════════════════════════════════════

    fn check_expr(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            // ── Literals ──
            ExprKind::NumberLit(_) => Type::Number,
            ExprKind::StringLit(_) => Type::String,
            ExprKind::BoolLit(_) => Type::Bool,
            ExprKind::NilLit => Type::Nil,

            ExprKind::StringInterpolation(parts) => {
                for part in parts {
                    if let StringPart::Expr(e) = part {
                        self.check_expr(e);
                    }
                }
                Type::String
            }

            ExprKind::ListLit(items) => {
                if items.is_empty() {
                    Type::List(Box::new(Type::Any))
                } else {
                    let first_ty = self.check_expr(&items[0]);
                    for item in items.iter().skip(1) {
                        let item_ty = self.check_expr(item);
                        if !item_ty.is_assignable_to(&first_ty) && !first_ty.is_assignable_to(&item_ty) {
                            self.error(
                                ErrorCode::TYPE_MISMATCH,
                                format!(
                                    "list element type mismatch: expected {}, got {}",
                                    first_ty, item_ty
                                ),
                                item.span,
                            );
                        }
                    }
                    Type::List(Box::new(first_ty))
                }
            }

            ExprKind::RecordLit(entries) => {
                let mut fields = Vec::new();
                for entry in entries {
                    match entry {
                        RecordEntry::Field { name, value } => {
                            let ty = self.check_expr(value);
                            fields.push(RecordField {
                                name: name.name.clone(),
                                ty,
                                optional: false,
                            });
                        }
                        RecordEntry::Spread(spread_expr) => {
                            let ty = self.check_expr(spread_expr);
                            // Spread source must be a record
                            if let Type::Record(spread_fields) = &ty {
                                for sf in spread_fields {
                                    fields.push(sf.clone());
                                }
                            } else if !matches!(ty, Type::Any | Type::Unknown) {
                                self.error(
                                    ErrorCode::TYPE_MISMATCH,
                                    format!("spread requires record type, got {}", ty),
                                    spread_expr.span,
                                );
                            }
                        }
                    }
                }
                Type::Record(fields)
            }

            // ── Identifiers ──
            ExprKind::Identifier(name) => {
                if let Some(ty) = self.env.lookup(name) {
                    ty.clone()
                } else {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!("undefined variable '{}'", name),
                        expr.span,
                    );
                    Type::Unknown
                }
            }

            // ── Calls ──
            ExprKind::Call { name, args } => {
                self.check_unqualified_call(name, args, expr.span)
            }

            ExprKind::QualifiedCall {
                module,
                function,
                args,
            } => self.check_qualified_call(module, function, args, expr.span),

            // ── Field Access ──
            ExprKind::FieldAccess { object, field } => {
                let obj_ty = self.check_expr(object);
                self.resolve_field_access(&obj_ty, &field.name, field.span)
            }

            ExprKind::MethodCall {
                object,
                method,
                args,
            } => {
                let obj_ty = self.check_expr(object);
                self.check_method_call(&obj_ty, method, args, expr.span)
            }

            // ── Operators ──
            ExprKind::Binary { left, op, right } => {
                self.check_binary(left, *op, right, expr.span)
            }

            ExprKind::Unary { op, operand } => self.check_unary(*op, operand, expr.span),

            ExprKind::ResultUnwrap(inner) => {
                let ty = self.check_expr(inner);
                match &ty {
                    Type::Result(ok, _) => *ok.clone(),
                    Type::Any | Type::Unknown => Type::Any,
                    _ => {
                        self.error(
                            ErrorCode::TYPE_MISMATCH,
                            format!("operator '?' requires Result type, got {}", ty),
                            expr.span,
                        );
                        Type::Unknown
                    }
                }
            }

            ExprKind::NilCoalesce { left, right } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                // Left should be nullable; result is the non-nil type
                match &left_ty {
                    Type::Nullable(inner) => {
                        if !right_ty.is_assignable_to(inner) {
                            self.error(
                                ErrorCode::TYPE_MISMATCH,
                                format!(
                                    "nil-coalescing fallback type {} doesn't match {}",
                                    right_ty, inner
                                ),
                                right.span,
                            );
                        }
                        *inner.clone()
                    }
                    Type::Nil => right_ty,
                    Type::Any | Type::Unknown => right_ty,
                    _ => {
                        // Not nullable — ?? is a no-op (warning, but not an error for now)
                        left_ty
                    }
                }
            }

            // ── Control Flow ──
            ExprKind::If(if_expr) => self.check_if_expr(if_expr),
            ExprKind::For(for_expr) => self.check_for_expr(for_expr),
            ExprKind::Match(match_expr) => self.check_match_expr(match_expr),

            // ── Lambda ──
            ExprKind::Lambda(lambda) => self.check_lambda(lambda),

            // ── Grouping ──
            ExprKind::Paren(inner) => self.check_expr(inner),
        }
    }

    // ── Call checking ─────────────────────────────────────────────────────

    fn check_unqualified_call(&mut self, name: &Ident, args: &[Expr], span: Span) -> Type {
        // Check if it's an action call (inside test blocks)
        if self.action_names.contains(&name.name) {
            // Type-check arguments but return void
            for arg in args {
                self.check_expr(arg);
            }
            return Type::Void;
        }

        // Check if it resolves to a function in scope
        if let Some(ty) = self.env.lookup(&name.name).cloned() {
            match &ty {
                Type::Function(param_types, ret_ty) => {
                    self.validate_arg_count(&name.name, args.len(), param_types.len(), false, span);
                    for (arg, expected) in args.iter().zip(param_types.iter()) {
                        let arg_ty = self.check_expr(arg);
                        if !arg_ty.is_assignable_to(expected) {
                            self.error(
                                ErrorCode::TYPE_MISMATCH,
                                format!(
                                    "argument type mismatch in '{}': expected {}, got {}",
                                    name.name, expected, arg_ty
                                ),
                                arg.span,
                            );
                        }
                    }
                    return *ret_ty.clone();
                }
                _ => {
                    // Not a function — just type check args
                    for arg in args {
                        self.check_expr(arg);
                    }
                    return Type::Unknown;
                }
            }
        }

        // Unknown function
        self.error(
            ErrorCode::TYPE_MISMATCH,
            format!("undefined function '{}'", name.name),
            span,
        );
        for arg in args {
            self.check_expr(arg);
        }
        Type::Unknown
    }

    fn check_qualified_call(
        &mut self,
        module: &Ident,
        function: &Ident,
        args: &[Expr],
        span: Span,
    ) -> Type {
        // Check capability constraints
        if let Some(required_cap) = self.capability_modules.get(module.name.as_str()) {
            let cap = required_cap.to_string();
            if !self.required_capabilities.contains(&cap)
                && !self.optional_capabilities.contains(&cap)
            {
                self.error(
                    ErrorCode::UNDECLARED_CAPABILITY,
                    format!(
                        "module '{}' requires capability '{}' but it is not declared",
                        module.name, cap
                    ),
                    module.span,
                );
            }
            // Cannot use capabilities in views
            if self.env.in_view() {
                self.error(
                    ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
                    format!(
                        "capability module '{}' cannot be used in views (views must be pure)",
                        module.name
                    ),
                    module.span,
                );
            }
        }

        // Check if it's a constant access (e.g. math.PI)
        if args.is_empty() {
            if let Some(ty) = self.stdlib.get_constant(&module.name, &function.name) {
                return ty.clone();
            }
        }

        // Check stdlib module exists
        if !self.stdlib.has_module(&module.name) {
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!("unknown module '{}'", module.name),
                module.span,
            );
            for arg in args {
                self.check_expr(arg);
            }
            return Type::Unknown;
        }

        // Check function exists in module
        let sig = if let Some(sig) = self.stdlib.get(&module.name, &function.name) {
            sig.clone()
        } else {
            // Could be a constant (already checked above), otherwise unknown
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!(
                    "unknown function '{}.{}'",
                    module.name, function.name
                ),
                function.span,
            );
            for arg in args {
                self.check_expr(arg);
            }
            return Type::Unknown;
        };

        self.check_call_against_sig(&format!("{}.{}", module.name, function.name), &sig, args, span)
    }

    fn check_call_against_sig(
        &mut self,
        name: &str,
        sig: &FnSig,
        args: &[Expr],
        span: Span,
    ) -> Type {
        self.validate_arg_count(name, args.len(), sig.params.len(), sig.variadic, span);

        for (i, arg) in args.iter().enumerate() {
            let arg_ty = self.check_expr(arg);
            if i < sig.params.len() {
                let expected = &sig.params[i].1;
                if !arg_ty.is_assignable_to(expected) {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "argument {} of '{}' expected {}, got {}",
                            i + 1,
                            name,
                            expected,
                            arg_ty
                        ),
                        arg.span,
                    );
                }
            }
        }

        sig.ret.clone()
    }

    fn validate_arg_count(
        &mut self,
        name: &str,
        got: usize,
        expected: usize,
        variadic: bool,
        span: Span,
    ) {
        if variadic {
            // Variadic functions accept any number of arguments
            return;
        }
        if got != expected {
            self.error(
                ErrorCode::WRONG_ARG_COUNT,
                format!(
                    "'{}' expects {} argument{}, got {}",
                    name,
                    expected,
                    if expected == 1 { "" } else { "s" },
                    got
                ),
                span,
            );
        }
    }

    // ── Field access ──────────────────────────────────────────────────────

    fn resolve_field_access(&mut self, obj_ty: &Type, field_name: &str, span: Span) -> Type {
        match obj_ty {
            Type::Record(fields) => {
                if let Some(rf) = fields.iter().find(|f| f.name == field_name) {
                    if rf.optional {
                        Type::Nullable(Box::new(rf.ty.clone()))
                    } else {
                        rf.ty.clone()
                    }
                } else {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!("record has no field '{}'", field_name),
                        span,
                    );
                    Type::Unknown
                }
            }
            Type::Any | Type::Unknown => Type::Any,
            _ => {
                self.error(
                    ErrorCode::TYPE_MISMATCH,
                    format!("cannot access field '{}' on type {}", field_name, obj_ty),
                    span,
                );
                Type::Unknown
            }
        }
    }

    fn check_method_call(
        &mut self,
        obj_ty: &Type,
        method: &Ident,
        args: &[Expr],
        span: Span,
    ) -> Type {
        // Method calls on lists: items.filter(fn(x) { ... })
        // These are syntactic sugar for stdlib calls: list.filter(items, fn(x) { ... })
        if let Type::List(_) = obj_ty {
            let mut full_args = Vec::with_capacity(args.len());
            // Type-check arguments
            for arg in args {
                full_args.push(self.check_expr(arg));
            }
            // Delegate to list module lookup
            if let Some(sig) = self.stdlib.get("list", &method.name).cloned() {
                // Validate arg count (method call adds the object as first arg)
                self.validate_arg_count(
                    &format!("list.{}", method.name),
                    args.len() + 1,
                    sig.params.len(),
                    sig.variadic,
                    span,
                );
                return sig.ret.clone();
            }
        }

        // String methods
        if matches!(obj_ty, Type::String) {
            if let Some(sig) = self.stdlib.get("string", &method.name).cloned() {
                self.validate_arg_count(
                    &format!("string.{}", method.name),
                    args.len() + 1,
                    sig.params.len(),
                    sig.variadic,
                    span,
                );
                for arg in args {
                    self.check_expr(arg);
                }
                return sig.ret.clone();
            }
        }

        // Generic case — type check args
        for arg in args {
            self.check_expr(arg);
        }

        match obj_ty {
            Type::Any | Type::Unknown => Type::Any,
            _ => {
                self.error(
                    ErrorCode::TYPE_MISMATCH,
                    format!("type {} has no method '{}'", obj_ty, method.name),
                    span,
                );
                Type::Unknown
            }
        }
    }

    // ── Binary operators ──────────────────────────────────────────────────

    fn check_binary(&mut self, left: &Expr, op: BinOp, right: &Expr, _span: Span) -> Type {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);

        match op {
            // Arithmetic: number × number → number
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                // String concatenation with +
                if op == BinOp::Add
                    && (matches!(left_ty, Type::String) || matches!(right_ty, Type::String))
                {
                    return Type::String;
                }
                if !left_ty.is_numeric() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!("left operand of '{}' must be number, got {}", op_symbol(op), left_ty),
                        left.span,
                    );
                }
                if !right_ty.is_numeric() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "right operand of '{}' must be number, got {}",
                            op_symbol(op),
                            right_ty
                        ),
                        right.span,
                    );
                }
                Type::Number
            }
            // Comparison: same type → bool
            BinOp::Eq | BinOp::NotEq => {
                // Equality works on any types (structural equality)
                Type::Bool
            }
            BinOp::Less | BinOp::Greater | BinOp::LessEq | BinOp::GreaterEq => {
                if !left_ty.is_numeric() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "left operand of '{}' must be number, got {}",
                            op_symbol(op),
                            left_ty
                        ),
                        left.span,
                    );
                }
                if !right_ty.is_numeric() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "right operand of '{}' must be number, got {}",
                            op_symbol(op),
                            right_ty
                        ),
                        right.span,
                    );
                }
                Type::Bool
            }
            // Logical: bool × bool → bool
            BinOp::And | BinOp::Or => {
                if !left_ty.is_bool() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "left operand of '{}' must be bool, got {}",
                            op_symbol(op),
                            left_ty
                        ),
                        left.span,
                    );
                }
                if !right_ty.is_bool() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!(
                            "right operand of '{}' must be bool, got {}",
                            op_symbol(op),
                            right_ty
                        ),
                        right.span,
                    );
                }
                Type::Bool
            }
        }
    }

    fn check_unary(&mut self, op: UnaryOp, operand: &Expr, span: Span) -> Type {
        let ty = self.check_expr(operand);
        match op {
            UnaryOp::Neg => {
                if !ty.is_numeric() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!("unary '-' requires number, got {}", ty),
                        span,
                    );
                }
                Type::Number
            }
            UnaryOp::Not => {
                if !ty.is_bool() {
                    self.error(
                        ErrorCode::TYPE_MISMATCH,
                        format!("'not' requires bool, got {}", ty),
                        span,
                    );
                }
                Type::Bool
            }
        }
    }

    // ── Control flow ──────────────────────────────────────────────────────

    fn check_if_expr(&mut self, if_expr: &IfExpr) -> Type {
        let cond_ty = self.check_expr(&if_expr.condition);
        if !cond_ty.is_bool() {
            self.error(
                ErrorCode::TYPE_MISMATCH,
                format!("if condition must be bool, got {}", cond_ty),
                if_expr.condition.span,
            );
        }

        // Nil narrowing: if x != nil, narrow x in then-block
        self.apply_nil_narrowing(&if_expr.condition, &if_expr.then_block);

        self.check_block(&if_expr.then_block);
        if let Some(else_branch) = &if_expr.else_branch {
            match else_branch {
                ElseBranch::ElseIf(elif) => {
                    self.check_if_expr(elif);
                }
                ElseBranch::Block(block) => {
                    self.check_block(block);
                }
            }
        }
        // If expressions used as statements return Void
        Type::Void
    }

    fn apply_nil_narrowing(&mut self, condition: &Expr, _then_block: &Block) {
        // Pattern: x != nil  →  narrow x from Nullable(T) to T in then block
        if let ExprKind::Binary {
            left,
            op: BinOp::NotEq,
            right,
        } = &condition.kind
        {
            if matches!(right.kind, ExprKind::NilLit) {
                if let ExprKind::Identifier(name) = &left.kind {
                    if let Some(Type::Nullable(inner)) = self.env.lookup(name).cloned() {
                        self.env.narrow(name, *inner);
                    }
                }
            }
        }
    }

    fn check_for_expr(&mut self, for_expr: &ForExpr) -> Type {
        let iter_ty = self.check_expr(&for_expr.iterable);

        self.env.push_scope(ScopeKind::Block);

        match &iter_ty {
            Type::List(elem_ty) => {
                self.env.define(&for_expr.item.name, *elem_ty.clone());
            }
            Type::Any | Type::Unknown => {
                self.env.define(&for_expr.item.name, Type::Any);
            }
            _ => {
                self.error(
                    ErrorCode::TYPE_MISMATCH,
                    format!("for loop requires list type, got {}", iter_ty),
                    for_expr.iterable.span,
                );
                self.env.define(&for_expr.item.name, Type::Unknown);
            }
        }

        if let Some(index) = &for_expr.index {
            self.env.define(&index.name, Type::Number);
        }

        // Check body within block scope (already pushed)
        for stmt in &for_expr.body.stmts {
            self.check_stmt(stmt);
        }

        self.env.pop_scope();
        Type::Void
    }

    fn check_match_expr(&mut self, match_expr: &MatchExpr) -> Type {
        let subject_ty = self.check_expr(&match_expr.subject);

        // Collect matched variant names for exhaustiveness check
        let mut matched_variants: HashSet<String> = HashSet::new();
        let mut has_wildcard = false;

        for arm in &match_expr.arms {
            self.env.push_scope(ScopeKind::Block);

            match &arm.pattern {
                Pattern::Variant { name, bindings } => {
                    matched_variants.insert(name.name.clone());

                    // Resolve variant parameter types
                    if let Type::SumType { variants, .. } = &subject_ty {
                        if let Some(variant) = variants.iter().find(|v| v.name == name.name) {
                            if bindings.len() != variant.params.len() {
                                self.error(
                                    ErrorCode::WRONG_ARG_COUNT,
                                    format!(
                                        "variant '{}' has {} parameter{}, but {} binding{} provided",
                                        name.name,
                                        variant.params.len(),
                                        if variant.params.len() == 1 { "" } else { "s" },
                                        bindings.len(),
                                        if bindings.len() == 1 { "" } else { "s" },
                                    ),
                                    arm.span,
                                );
                            }
                            for (binding, (_, param_ty)) in bindings.iter().zip(variant.params.iter()) {
                                self.env.define(&binding.name, param_ty.clone());
                            }
                        } else {
                            self.error(
                                ErrorCode::TYPE_MISMATCH,
                                format!(
                                    "type {} has no variant '{}'",
                                    subject_ty, name.name
                                ),
                                name.span,
                            );
                        }
                    } else if let Type::Named(type_name) = &subject_ty {
                        // Resolve named type
                        if let Some(variants) = self.sum_types.get(type_name) {
                            if let Some(variant) = variants.iter().find(|v| v.name == name.name) {
                                for (binding, (_, param_ty)) in
                                    bindings.iter().zip(variant.params.iter())
                                {
                                    self.env.define(&binding.name, param_ty.clone());
                                }
                            }
                        }
                    }
                }
                Pattern::Wildcard(_) => {
                    has_wildcard = true;
                }
            }

            // Check arm body
            match &arm.body {
                MatchArmBody::Expr(expr) => {
                    self.check_expr(expr);
                }
                MatchArmBody::Block(block) => {
                    // Don't push another scope — we already have one
                    for stmt in &block.stmts {
                        self.check_stmt(stmt);
                    }
                }
            }

            self.env.pop_scope();
        }

        // Exhaustiveness check
        if !has_wildcard {
            let all_variants = match &subject_ty {
                Type::SumType { variants, .. } => {
                    Some(variants.iter().map(|v| v.name.clone()).collect::<HashSet<_>>())
                }
                Type::Named(name) => self.sum_types.get(name).map(|variants| {
                    variants.iter().map(|v| v.name.clone()).collect::<HashSet<_>>()
                }),
                _ => None,
            };

            if let Some(all) = all_variants {
                let missing: Vec<_> = all.difference(&matched_variants).cloned().collect();
                if !missing.is_empty() {
                    self.error(
                        ErrorCode::NON_EXHAUSTIVE_MATCH,
                        format!(
                            "non-exhaustive match: missing variant{} {}",
                            if missing.len() == 1 { "" } else { "s" },
                            missing.join(", ")
                        ),
                        match_expr.span,
                    );
                }
            }
        }

        Type::Void
    }

    // ── Lambda ────────────────────────────────────────────────────────────

    fn check_lambda(&mut self, lambda: &LambdaExpr) -> Type {
        self.env.push_scope(ScopeKind::Lambda);

        let mut param_types = Vec::new();
        for param in &lambda.params {
            let ty = self.resolve_type_annotation(&param.type_ann);
            param_types.push(ty.clone());
            if !self.env.define(&param.name.name, ty) {
                self.error(
                    ErrorCode::VARIABLE_ALREADY_DECLARED,
                    format!("parameter '{}' already declared", param.name.name),
                    param.span,
                );
            }
        }

        // Check body and capture the type of the last expression (return type).
        // Must be done BEFORE popping the scope so lambda params remain visible.
        let mut last_type = Type::Void;
        for stmt in &lambda.body.stmts {
            match stmt {
                Stmt::Expr(expr_stmt) => {
                    last_type = self.check_expr(&expr_stmt.expr);
                }
                other => {
                    self.check_stmt(other);
                    last_type = Type::Void;
                }
            }
        }

        self.env.pop_scope();

        Type::Function(param_types, Box::new(last_type))
    }

    // ══════════════════════════════════════════════════════════════════════
    // Type Resolution
    // ══════════════════════════════════════════════════════════════════════

    fn resolve_type_annotation(&mut self, ann: &TypeAnnotation) -> Type {
        let ty = Type::from_annotation(ann);

        // Reject `any` in user type annotations
        if matches!(ty, Type::Any) {
            self.error(
                ErrorCode::UNKNOWN_TYPE,
                "'any' cannot be used in type annotations".to_string(),
                ann.span,
            );
        }

        // Resolve Named types to sum types
        if let Type::Named(name) = &ty {
            if self.sum_types.contains_key(name) {
                return Type::Named(name.clone());
            }
            // Check if it's a type alias in scope
            if let Some(resolved) = self.env.lookup(name).cloned() {
                return resolved;
            }
            // Unknown type
            self.error(
                ErrorCode::UNKNOWN_TYPE,
                format!("unknown type '{}'", name),
                ann.span,
            );
            return Type::Unknown;
        }

        ty
    }

    // ══════════════════════════════════════════════════════════════════════
    // Error Reporting
    // ══════════════════════════════════════════════════════════════════════

    fn error(&mut self, code: ErrorCode, message: String, span: Span) {
        let source_line = self
            .source
            .line(span.start_line)
            .unwrap_or("")
            .to_string();
        self.errors.push_error(pepl_types::PeplError::new(
            &self.source.name,
            code,
            message,
            span,
            source_line,
        ));
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

fn op_symbol(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Less => "<",
        BinOp::Greater => ">",
        BinOp::LessEq => "<=",
        BinOp::GreaterEq => ">=",
        BinOp::And => "and",
        BinOp::Or => "or",
    }
}

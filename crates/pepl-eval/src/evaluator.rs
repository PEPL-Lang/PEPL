//! Core expression and statement evaluator.

use crate::env::Environment;
use crate::error::{EvalError, EvalResult};
use pepl_stdlib::modules::{convert, core, json, list, math, record, string, time, timer};
use pepl_stdlib::{StdlibModule, Value, ResultValue};
use pepl_types::ast::*;
use std::collections::BTreeMap;

/// The core evaluator — walks AST nodes and produces Values.
pub struct Evaluator {
    /// Variable environment (scoped).
    pub env: Environment,
    /// Gas counter — limits total steps to prevent infinite loops.
    pub gas: u64,
    /// Gas limit.
    pub gas_limit: u64,
    /// Captured log output from `core.log`.
    pub log_output: Vec<String>,
    /// Action names registered in the space (for resolving action references).
    pub action_names: Vec<String>,
    /// Mock capability responses (module, function) → response Value.
    /// Used by the test runner for `with_responses` blocks.
    pub mock_responses: Vec<(String, String, Value)>,
}

impl Evaluator {
    /// Create a new evaluator with the given gas limit.
    pub fn new(gas_limit: u64) -> Self {
        Self {
            env: Environment::new(),
            gas: 0,
            gas_limit,
            log_output: Vec::new(),
            action_names: Vec::new(),
            mock_responses: Vec::new(),
        }
    }

    /// Consume one unit of gas. Returns error if exhausted.
    fn tick(&mut self) -> EvalResult<()> {
        self.gas += 1;
        if self.gas > self.gas_limit {
            Err(EvalError::GasExhausted)
        } else {
            Ok(())
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Expression evaluation
    // ══════════════════════════════════════════════════════════════════════

    /// Evaluate an expression to a Value.
    pub fn eval_expr(&mut self, expr: &Expr) -> EvalResult<Value> {
        self.tick()?;
        match &expr.kind {
            ExprKind::NumberLit(n) => Ok(Value::Number(*n)),
            ExprKind::StringLit(s) => Ok(Value::String(s.clone())),
            ExprKind::BoolLit(b) => Ok(Value::Bool(*b)),
            ExprKind::NilLit => Ok(Value::Nil),

            ExprKind::StringInterpolation(parts) => self.eval_string_interpolation(parts),
            ExprKind::ListLit(elems) => self.eval_list_literal(elems),
            ExprKind::RecordLit(entries) => self.eval_record_literal(entries),

            ExprKind::Identifier(name) => self.eval_identifier(name),

            ExprKind::Call { name, args } => self.eval_call(&name.name, args),
            ExprKind::QualifiedCall {
                module,
                function,
                args,
            } => self.eval_qualified_call(&module.name, &function.name, args),
            ExprKind::FieldAccess { object, field } => self.eval_field_access(object, &field.name),
            ExprKind::MethodCall {
                object,
                method,
                args,
            } => self.eval_method_call(object, &method.name, args),

            ExprKind::Binary { left, op, right } => self.eval_binary(left, *op, right),
            ExprKind::Unary { op, operand } => self.eval_unary(*op, operand),
            ExprKind::ResultUnwrap(inner) => self.eval_result_unwrap(inner),
            ExprKind::NilCoalesce { left, right } => self.eval_nil_coalesce(left, right),

            ExprKind::If(if_expr) => self.eval_if_expr(if_expr),
            ExprKind::For(for_expr) => self.eval_for_expr(for_expr),
            ExprKind::Match(match_expr) => self.eval_match_expr(match_expr),
            ExprKind::Lambda(lambda) => self.eval_lambda(lambda),
            ExprKind::Paren(inner) => self.eval_expr(inner),
        }
    }

    // ── Literals ──────────────────────────────────────────────────────────

    fn eval_string_interpolation(&mut self, parts: &[StringPart]) -> EvalResult<Value> {
        let mut result = String::new();
        for part in parts {
            match part {
                StringPart::Literal(s) => result.push_str(s),
                StringPart::Expr(expr) => {
                    let val = self.eval_expr(expr)?;
                    result.push_str(&self.value_to_display_string(&val));
                }
            }
        }
        Ok(Value::String(result))
    }

    fn eval_list_literal(&mut self, elems: &[Expr]) -> EvalResult<Value> {
        let mut values = Vec::with_capacity(elems.len());
        for elem in elems {
            values.push(self.eval_expr(elem)?);
        }
        Ok(Value::List(values))
    }

    fn eval_record_literal(&mut self, entries: &[RecordEntry]) -> EvalResult<Value> {
        let mut fields = BTreeMap::new();
        for entry in entries {
            match entry {
                RecordEntry::Field { name, value } => {
                    let val = self.eval_expr(value)?;
                    fields.insert(name.name.clone(), val);
                }
                RecordEntry::Spread(expr) => {
                    let val = self.eval_expr(expr)?;
                    if let Value::Record { fields: rf, .. } = val {
                        for (k, v) in rf {
                            fields.insert(k, v);
                        }
                    } else {
                        return Err(EvalError::TypeMismatch(
                            format!("spread requires record, got {}", val.type_name()),
                        ));
                    }
                }
            }
        }
        Ok(Value::Record {
            type_name: None,
            fields,
        })
    }

    // ── Identifiers & Calls ──────────────────────────────────────────────

    fn eval_identifier(&self, name: &str) -> EvalResult<Value> {
        self.env
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedVariable(name.to_string()))
    }

    /// Evaluate an unqualified call: `func(args)`.
    /// In PEPL, unqualified calls in action bodies are action dispatches.
    /// In view/expression context, they resolve to identifiers that are Function values.
    fn eval_call(&mut self, name: &str, args: &[Expr]) -> EvalResult<Value> {
        // Check if it's a Function value in scope
        if let Some(val) = self.env.get(name).cloned() {
            if let Value::Function(f) = val {
                let mut arg_vals = Vec::with_capacity(args.len());
                for arg in args {
                    arg_vals.push(self.eval_expr(arg)?);
                }
                return f.0(arg_vals).map_err(|e| EvalError::StdlibError(e.to_string()));
            }
        }
        // Otherwise, unknown function
        Err(EvalError::UnknownFunction(format!(
            "unknown function '{name}'"
        )))
    }

    /// Evaluate a qualified call: `module.function(args)`.
    pub fn eval_qualified_call(
        &mut self,
        module: &str,
        function: &str,
        args: &[Expr],
    ) -> EvalResult<Value> {
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            arg_vals.push(self.eval_expr(arg)?);
        }
        self.call_stdlib(module, function, arg_vals)
    }

    fn eval_field_access(&mut self, object: &Expr, field: &str) -> EvalResult<Value> {
        let obj = self.eval_expr(object)?;
        match &obj {
            Value::Record { fields, .. } => fields
                .get(field)
                .cloned()
                .ok_or_else(|| {
                    EvalError::Runtime(format!("record has no field '{field}'"))
                }),
            Value::Nil => Err(EvalError::NilAccess(format!(
                "cannot access field '{field}' on nil"
            ))),
            _ => Err(EvalError::TypeMismatch(format!(
                "cannot access field '{field}' on {}",
                obj.type_name()
            ))),
        }
    }

    fn eval_method_call(
        &mut self,
        object: &Expr,
        method: &str,
        args: &[Expr],
    ) -> EvalResult<Value> {
        let obj = self.eval_expr(object)?;
        let mut all_args = vec![obj];
        for arg in args {
            all_args.push(self.eval_expr(arg)?);
        }
        // Method calls on lists → list.method, strings → string.method
        let module = match &all_args[0] {
            Value::List(_) => "list",
            Value::String(_) => "string",
            _ => {
                return Err(EvalError::TypeMismatch(format!(
                    "cannot call method '{method}' on {}",
                    all_args[0].type_name()
                )));
            }
        };
        self.call_stdlib(module, method, all_args)
    }

    // ── Operators ────────────────────────────────────────────────────────

    fn eval_binary(&mut self, left: &Expr, op: BinOp, right: &Expr) -> EvalResult<Value> {
        // Short-circuit for logical operators
        if op == BinOp::And {
            let lv = self.eval_expr(left)?;
            return if !lv.is_truthy() {
                Ok(Value::Bool(false))
            } else {
                let rv = self.eval_expr(right)?;
                Ok(Value::Bool(rv.is_truthy()))
            };
        }
        if op == BinOp::Or {
            let lv = self.eval_expr(left)?;
            return if lv.is_truthy() {
                Ok(Value::Bool(true))
            } else {
                let rv = self.eval_expr(right)?;
                Ok(Value::Bool(rv.is_truthy()))
            };
        }

        let lv = self.eval_expr(left)?;
        let rv = self.eval_expr(right)?;

        match op {
            BinOp::Add => self.eval_add(&lv, &rv),
            BinOp::Sub => self.eval_arith(&lv, &rv, |a, b| a - b, "-"),
            BinOp::Mul => self.eval_arith(&lv, &rv, |a, b| a * b, "*"),
            BinOp::Div => {
                if let (Value::Number(a), Value::Number(b)) = (&lv, &rv) {
                    if *b == 0.0 {
                        return Err(EvalError::ArithmeticTrap("division by zero".into()));
                    }
                    let result = a / b;
                    if result.is_nan() || result.is_infinite() {
                        return Err(EvalError::ArithmeticTrap("division produced NaN/Infinity".into()));
                    }
                    Ok(Value::Number(result))
                } else {
                    Err(EvalError::TypeMismatch(format!(
                        "cannot divide {} by {}",
                        lv.type_name(),
                        rv.type_name()
                    )))
                }
            }
            BinOp::Mod => {
                if let (Value::Number(a), Value::Number(b)) = (&lv, &rv) {
                    if *b == 0.0 {
                        return Err(EvalError::ArithmeticTrap("modulo by zero".into()));
                    }
                    Ok(Value::Number(a % b))
                } else {
                    Err(EvalError::TypeMismatch(format!(
                        "cannot modulo {} by {}",
                        lv.type_name(),
                        rv.type_name()
                    )))
                }
            }
            BinOp::Eq => Ok(Value::Bool(self.structural_eq(&lv, &rv))),
            BinOp::NotEq => Ok(Value::Bool(!self.structural_eq(&lv, &rv))),
            BinOp::Less => self.eval_comparison(&lv, &rv, |a, b| a < b),
            BinOp::Greater => self.eval_comparison(&lv, &rv, |a, b| a > b),
            BinOp::LessEq => self.eval_comparison(&lv, &rv, |a, b| a <= b),
            BinOp::GreaterEq => self.eval_comparison(&lv, &rv, |a, b| a >= b),
            BinOp::And | BinOp::Or => unreachable!("handled above"),
        }
    }

    fn eval_add(&self, lv: &Value, rv: &Value) -> EvalResult<Value> {
        match (lv, rv) {
            (Value::Number(a), Value::Number(b)) => {
                let result = a + b;
                if result.is_nan() || result.is_infinite() {
                    Err(EvalError::ArithmeticTrap("addition produced NaN/Infinity".into()))
                } else {
                    Ok(Value::Number(result))
                }
            }
            (Value::String(a), Value::String(b)) => {
                Ok(Value::String(format!("{a}{b}")))
            }
            _ => Err(EvalError::TypeMismatch(format!(
                "cannot add {} and {}",
                lv.type_name(),
                rv.type_name()
            ))),
        }
    }

    fn eval_arith(
        &self,
        lv: &Value,
        rv: &Value,
        op: fn(f64, f64) -> f64,
        symbol: &str,
    ) -> EvalResult<Value> {
        if let (Value::Number(a), Value::Number(b)) = (lv, rv) {
            let result = op(*a, *b);
            if result.is_nan() || result.is_infinite() {
                Err(EvalError::ArithmeticTrap(format!("{symbol} produced NaN/Infinity")))
            } else {
                Ok(Value::Number(result))
            }
        } else {
            Err(EvalError::TypeMismatch(format!(
                "cannot apply '{symbol}' to {} and {}",
                lv.type_name(),
                rv.type_name()
            )))
        }
    }

    fn eval_comparison(
        &self,
        lv: &Value,
        rv: &Value,
        op: fn(f64, f64) -> bool,
    ) -> EvalResult<Value> {
        match (lv, rv) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(op(*a, *b))),
            (Value::String(a), Value::String(b)) => Ok(Value::Bool(op(
                a.len() as f64,
                b.len() as f64,
            ))),
            _ => Err(EvalError::TypeMismatch(format!(
                "cannot compare {} and {}",
                lv.type_name(),
                rv.type_name()
            ))),
        }
    }

    fn eval_unary(&mut self, op: UnaryOp, operand: &Expr) -> EvalResult<Value> {
        let val = self.eval_expr(operand)?;
        match op {
            UnaryOp::Neg => {
                if let Value::Number(n) = val {
                    Ok(Value::Number(-n))
                } else {
                    Err(EvalError::TypeMismatch(format!(
                        "cannot negate {}",
                        val.type_name()
                    )))
                }
            }
            UnaryOp::Not => Ok(Value::Bool(!val.is_truthy())),
        }
    }

    fn eval_result_unwrap(&mut self, inner: &Expr) -> EvalResult<Value> {
        let val = self.eval_expr(inner)?;
        match val {
            Value::Result(r) => match *r {
                ResultValue::Ok(v) => Ok(v),
                ResultValue::Err(e) => Err(EvalError::UnwrapError(format!(
                    "unwrap on Err: {}",
                    self.value_to_display_string(&e)
                ))),
            },
            _ => Err(EvalError::TypeMismatch(format!(
                "'?' requires Result, got {}",
                val.type_name()
            ))),
        }
    }

    fn eval_nil_coalesce(&mut self, left: &Expr, right: &Expr) -> EvalResult<Value> {
        let lv = self.eval_expr(left)?;
        if lv == Value::Nil {
            self.eval_expr(right)
        } else {
            Ok(lv)
        }
    }

    // ── Control Flow ─────────────────────────────────────────────────────

    pub fn eval_if_expr(&mut self, if_expr: &IfExpr) -> EvalResult<Value> {
        let cond = self.eval_expr(&if_expr.condition)?;
        if cond.is_truthy() {
            self.eval_block(&if_expr.then_block)
        } else if let Some(else_branch) = &if_expr.else_branch {
            match else_branch {
                ElseBranch::ElseIf(elif) => self.eval_if_expr(elif),
                ElseBranch::Block(block) => self.eval_block(block),
            }
        } else {
            Ok(Value::Nil)
        }
    }

    fn eval_for_expr(&mut self, for_expr: &ForExpr) -> EvalResult<Value> {
        let iterable = self.eval_expr(&for_expr.iterable)?;
        let items = match iterable {
            Value::List(items) => items,
            _ => {
                return Err(EvalError::TypeMismatch(format!(
                    "for loop requires list, got {}",
                    iterable.type_name()
                )));
            }
        };

        self.env.push_scope();
        let mut last = Value::Nil;
        for (i, item) in items.iter().enumerate() {
            self.env.define(&for_expr.item.name, item.clone());
            if let Some(idx) = &for_expr.index {
                self.env.define(&idx.name, Value::Number(i as f64));
            }
            last = self.eval_block(&for_expr.body)?;
        }
        self.env.pop_scope();
        Ok(last)
    }

    fn eval_match_expr(&mut self, match_expr: &MatchExpr) -> EvalResult<Value> {
        let subject = self.eval_expr(&match_expr.subject)?;

        for arm in &match_expr.arms {
            if let Some(bindings) = self.match_pattern(&arm.pattern, &subject) {
                self.env.push_scope();
                for (name, val) in bindings {
                    self.env.define(&name, val);
                }
                let result = self.eval_match_arm_body(&arm.body);
                self.env.pop_scope();
                return result;
            }
        }
        // No match — should not happen with exhaustive match, but return Nil
        Ok(Value::Nil)
    }

    /// Try to match a pattern against a value.
    /// Returns Some(bindings) if match, None otherwise.
    fn match_pattern(&self, pattern: &Pattern, value: &Value) -> Option<Vec<(String, Value)>> {
        match pattern {
            Pattern::Wildcard(_) => Some(vec![]),
            Pattern::Variant { name, bindings } => {
                // Match against Result variants
                if let Value::Result(r) = value {
                    match (name.name.as_str(), r.as_ref()) {
                        ("Ok", ResultValue::Ok(v)) => {
                            let mut b = Vec::new();
                            if let Some(binding) = bindings.first() {
                                b.push((binding.name.clone(), v.clone()));
                            }
                            Some(b)
                        }
                        ("Err", ResultValue::Err(v)) => {
                            let mut b = Vec::new();
                            if let Some(binding) = bindings.first() {
                                b.push((binding.name.clone(), v.clone()));
                            }
                            Some(b)
                        }
                        _ => None,
                    }
                }
                // Match against SumVariant
                else if let Value::SumVariant {
                    variant, fields, ..
                } = value
                {
                    if variant == &name.name {
                        let mut b = Vec::new();
                        for (binding, field) in bindings.iter().zip(fields.iter()) {
                            b.push((binding.name.clone(), field.clone()));
                        }
                        Some(b)
                    } else {
                        None
                    }
                }
                // Match against unit variant names (e.g., `Active`)
                else if bindings.is_empty() {
                    // Check if value is a string that equals the variant name
                    if let Value::String(s) = value {
                        if s == &name.name {
                            return Some(vec![]);
                        }
                    }
                    None
                } else {
                    None
                }
            }
        }
    }

    fn eval_match_arm_body(&mut self, body: &MatchArmBody) -> EvalResult<Value> {
        match body {
            MatchArmBody::Expr(expr) => self.eval_expr(expr),
            MatchArmBody::Block(block) => self.eval_block(block),
        }
    }

    pub fn eval_lambda(&mut self, lambda: &LambdaExpr) -> EvalResult<Value> {
        // Capture current environment snapshot for closure
        let captured_env = self.env.clone();
        let params: Vec<String> = lambda.params.iter().map(|p| p.name.name.clone()).collect();
        let body = lambda.body.clone();

        let closure = pepl_stdlib::StdlibFn(std::sync::Arc::new(move |args: Vec<Value>| {
            // Create a mini evaluator with captured env
            let mut eval = Evaluator::new(100_000);
            eval.env = captured_env.clone();
            eval.env.push_scope();
            for (param, arg) in params.iter().zip(args.into_iter()) {
                eval.env.define(param, arg);
            }
            let result = eval
                .eval_block(&body)
                .map_err(|e| pepl_stdlib::StdlibError::RuntimeError(e.to_string()))?;
            eval.env.pop_scope();
            Ok(result)
        }));

        Ok(Value::Function(closure))
    }

    // ══════════════════════════════════════════════════════════════════════
    // Block & Statement execution
    // ══════════════════════════════════════════════════════════════════════

    /// Execute a block of statements. Returns the value of the last expression, or Nil.
    pub fn eval_block(&mut self, block: &Block) -> EvalResult<Value> {
        let mut last = Value::Nil;
        for stmt in &block.stmts {
            last = self.eval_stmt(stmt)?;
        }
        Ok(last)
    }

    /// Execute a single statement.
    pub fn eval_stmt(&mut self, stmt: &Stmt) -> EvalResult<Value> {
        self.tick()?;
        match stmt {
            Stmt::Set(set) => self.eval_set(set),
            Stmt::Let(binding) => self.eval_let(binding),
            Stmt::If(if_expr) => self.eval_if_expr(if_expr),
            Stmt::For(for_expr) => self.eval_for_expr(for_expr),
            Stmt::Match(match_expr) => self.eval_match_expr(match_expr),
            Stmt::Return(ret) => {
                let _ = ret;
                // Return with Nil value — prior set statements are applied
                Err(EvalError::Return(Value::Nil))
            }
            Stmt::Assert(assert) => self.eval_assert(assert),
            Stmt::Expr(expr_stmt) => self.eval_expr(&expr_stmt.expr),
        }
    }

    fn eval_set(&mut self, set: &SetStmt) -> EvalResult<Value> {
        let value = self.eval_expr(&set.value)?;

        if set.target.len() == 1 {
            // Simple: `set x = value`
            let name = &set.target[0].name;
            if !self.env.set(name, value) {
                return Err(EvalError::UndefinedVariable(name.clone()));
            }
        } else {
            // Nested: `set a.b.c = value` → immutable record update
            self.eval_nested_set(&set.target, value)?;
        }
        Ok(Value::Nil)
    }

    /// Handle `set a.b.c = value` by immutable record reconstruction.
    fn eval_nested_set(&mut self, target: &[Ident], value: Value) -> EvalResult<()> {
        let root_name = &target[0].name;
        let root = self
            .env
            .get(root_name)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedVariable(root_name.clone()))?;

        let new_root = self.set_nested_field(&root, &target[1..], value)?;
        self.env.set(root_name, new_root);
        Ok(())
    }

    fn set_nested_field(
        &self,
        current: &Value,
        path: &[Ident],
        value: Value,
    ) -> EvalResult<Value> {
        if path.is_empty() {
            return Ok(value);
        }

        let field_name = &path[0].name;
        match current {
            Value::Record {
                type_name, fields, ..
            } => {
                let mut new_fields = fields.clone();
                if path.len() == 1 {
                    new_fields.insert(field_name.clone(), value);
                } else {
                    let inner = fields
                        .get(field_name)
                        .ok_or_else(|| {
                            EvalError::Runtime(format!("record has no field '{field_name}'"))
                        })?;
                    let new_inner = self.set_nested_field(inner, &path[1..], value)?;
                    new_fields.insert(field_name.clone(), new_inner);
                }
                Ok(Value::Record {
                    type_name: type_name.clone(),
                    fields: new_fields,
                })
            }
            _ => Err(EvalError::TypeMismatch(format!(
                "cannot set field '{}' on {}",
                field_name,
                current.type_name()
            ))),
        }
    }

    fn eval_let(&mut self, binding: &LetBinding) -> EvalResult<Value> {
        let value = self.eval_expr(&binding.value)?;
        if let Some(name) = &binding.name {
            self.env.define(&name.name, value);
        }
        // Discard binding (let _ = expr)
        Ok(Value::Nil)
    }

    fn eval_assert(&mut self, assert: &AssertStmt) -> EvalResult<Value> {
        let val = self.eval_expr(&assert.condition)?;
        if !val.is_truthy() {
            let msg = assert
                .message
                .clone()
                .unwrap_or_else(|| "assertion failed".into());
            return Err(EvalError::AssertionFailed(msg));
        }
        Ok(Value::Nil)
    }

    // ══════════════════════════════════════════════════════════════════════
    // Stdlib dispatch
    // ══════════════════════════════════════════════════════════════════════

    /// Call a stdlib function by module and function name.
    pub fn call_stdlib(
        &mut self,
        module: &str,
        function: &str,
        args: Vec<Value>,
    ) -> EvalResult<Value> {
        // Special handling for core.log → capture output
        if module == "core" && function == "log" {
            if let Some(val) = args.first() {
                self.log_output
                    .push(self.value_to_display_string(val));
            }
            return Ok(Value::Nil);
        }

        // Dispatch to the appropriate stdlib module
        let result = match module {
            "core" => core::CoreModule.call(function, args),
            "math" => math::MathModule.call(function, args),
            "string" => string::StringModule.call(function, args),
            "list" => list::ListModule.call(function, args),
            "record" => record::RecordModule.call(function, args),
            "time" => time::TimeModule.call(function, args),
            "convert" => convert::ConvertModule.call(function, args),
            "json" => json::JsonModule.call(function, args),
            "timer" => timer::TimerModule.call(function, args),
            // Capability modules — check mock responses first, then return Err for unmocked calls
            "http" | "storage" | "location" | "notifications" | "clipboard" | "share" => {
                // Check if there's a mock response available
                if let Some(response) = self.find_mock_response(module, function) {
                    Ok(response)
                } else {
                    Ok(Value::Result(Box::new(ResultValue::Err(Value::String(
                        format!("unmocked capability call: {module}.{function}"),
                    )))))
                }
            }
            _ => {
                return Err(EvalError::UnknownFunction(format!(
                    "unknown module '{module}'"
                )));
            }
        };

        result.map_err(|e| EvalError::StdlibError(e.to_string()))
    }

    // ══════════════════════════════════════════════════════════════════════
    // Mock response lookup
    // ══════════════════════════════════════════════════════════════════════

    /// Find a mock response for a capability call.
    fn find_mock_response(&self, module: &str, function: &str) -> Option<Value> {
        self.mock_responses
            .iter()
            .find(|(m, f, _)| m == module && f == function)
            .map(|(_, _, v)| v.clone())
    }

    // ══════════════════════════════════════════════════════════════════════
    // Structural equality
    // ══════════════════════════════════════════════════════════════════════

    /// Deep structural equality. NaN != NaN. Functions always false.
    pub fn structural_eq(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => {
                // NaN != NaN
                if x.is_nan() || y.is_nan() {
                    false
                } else {
                    x == y
                }
            }
            (Value::String(x), Value::String(y)) => x == y,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Nil, Value::Nil) => true,
            (Value::List(x), Value::List(y)) => {
                x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| self.structural_eq(a, b))
            }
            (
                Value::Record { fields: fa, .. },
                Value::Record { fields: fb, .. },
            ) => {
                fa.len() == fb.len()
                    && fa
                        .iter()
                        .all(|(k, v)| fb.get(k).map_or(false, |v2| self.structural_eq(v, v2)))
            }
            (Value::Result(a), Value::Result(b)) => match (a.as_ref(), b.as_ref()) {
                (ResultValue::Ok(a), ResultValue::Ok(b)) => self.structural_eq(a, b),
                (ResultValue::Err(a), ResultValue::Err(b)) => self.structural_eq(a, b),
                _ => false,
            },
            (
                Value::SumVariant {
                    variant: va,
                    fields: fa,
                    ..
                },
                Value::SumVariant {
                    variant: vb,
                    fields: fb,
                    ..
                },
            ) => {
                va == vb
                    && fa.len() == fb.len()
                    && fa
                        .iter()
                        .zip(fb.iter())
                        .all(|(a, b)| self.structural_eq(a, b))
            }
            // Functions never equal
            (Value::Function(_), _) | (_, Value::Function(_)) => false,
            _ => false,
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Display
    // ══════════════════════════════════════════════════════════════════════

    /// Convert a Value to its display string (for string interpolation, core.log, etc.)
    pub fn value_to_display_string(&self, val: &Value) -> String {
        match val {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Value::String(s) => s.clone(),
            Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Nil => "nil".to_string(),
            Value::List(items) => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|v| self.value_to_display_string(v))
                    .collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Record { fields, .. } => {
                let parts: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", self.value_to_display_string(v)))
                    .collect();
                format!("{{ {} }}", parts.join(", "))
            }
            Value::Result(r) => match r.as_ref() {
                ResultValue::Ok(v) => format!("Ok({})", self.value_to_display_string(v)),
                ResultValue::Err(v) => format!("Err({})", self.value_to_display_string(v)),
            },
            Value::SumVariant {
                variant, fields, ..
            } => {
                if fields.is_empty() {
                    variant.clone()
                } else {
                    let parts: Vec<String> = fields
                        .iter()
                        .map(|v| self.value_to_display_string(v))
                        .collect();
                    format!("{variant}({})", parts.join(", "))
                }
            }
            Value::Function(_) => "<function>".to_string(),
            Value::Color { r, g, b, a } => format!("color({r}, {g}, {b}, {a})"),
        }
    }

    /// String comparison for ordering (used by string comparison operators).
    /// Note: PEPL spec says string comparison compares by length, not lexicographic.
    /// This matches the eval_comparison implementation.
    pub fn string_comparison(&self, _a: &str, _b: &str) -> std::cmp::Ordering {
        // Not used in current implementation
        std::cmp::Ordering::Equal
    }
}

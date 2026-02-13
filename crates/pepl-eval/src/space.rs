//! SpaceInstance — runtime representation of a PEPL space.
//!
//! Manages state, derived fields, invariants, action dispatch,
//! view rendering, and atomic transactions with rollback.

use crate::error::{EvalError, EvalResult};
use crate::evaluator::Evaluator;
use crate::test_runner::MockResponse;
use pepl_stdlib::{ResultValue, Value};
use pepl_types::ast::*;
use std::collections::BTreeMap;

/// A snapshot of the view surface tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceNode {
    /// Component name (e.g., "Text", "Column").
    pub component: String,
    /// Component props as key-value pairs.
    pub props: BTreeMap<String, Value>,
    /// Child surface nodes.
    pub children: Vec<SurfaceNode>,
}

/// The result of dispatching an action.
#[derive(Debug)]
pub struct ActionResult {
    /// Whether the action committed (true) or rolled back (false).
    pub committed: bool,
    /// Invariant violation message, if rollback occurred.
    pub invariant_error: Option<String>,
}

/// Runtime instance of a PEPL space.
///
/// Holds the current state, derived fields, and references to the AST
/// for actions, views, and invariants. Supports atomic action dispatch
/// with invariant checking and rollback.
pub struct SpaceInstance {
    /// The evaluator engine.
    eval: Evaluator,
    /// State field names (for identifying which env bindings are state).
    state_fields: Vec<String>,
    /// Derived field definitions (name + expression, computed in order).
    derived_fields: Vec<(String, Expr)>,
    /// Invariant definitions (name + condition expression).
    invariants: Vec<(String, Expr)>,
    /// Action declarations.
    actions: Vec<ActionDecl>,
    /// View declarations.
    views: Vec<ViewDecl>,
    /// Credential values (set by host before dispatch).
    credentials: BTreeMap<String, Value>,
    /// Update declaration (optional game loop).
    update_decl: Option<UpdateDecl>,
    /// HandleEvent declaration (optional game loop).
    handle_event_decl: Option<HandleEventDecl>,
    /// Mock capability responses for test runner.
    mock_responses: Vec<MockResponse>,
}

impl SpaceInstance {
    /// Create a new SpaceInstance from a parsed+validated Program.
    ///
    /// Initializes state fields with their default values, computes
    /// derived fields, and registers actions/views.
    pub fn new(program: &Program) -> EvalResult<Self> {
        Self::with_gas_limit(program, 1_000_000)
    }

    /// Create with a custom gas limit.
    pub fn with_gas_limit(program: &Program, gas_limit: u64) -> EvalResult<Self> {
        let body = &program.space.body;
        let mut eval = Evaluator::new(gas_limit);

        // Register action names for reference resolution
        eval.action_names = body
            .actions
            .iter()
            .map(|a| a.name.name.clone())
            .collect();

        // Initialize state fields with default values
        let mut state_fields = Vec::new();
        for field in &body.state.fields {
            let default = eval.eval_expr(&field.default)?;
            eval.env.define(&field.name.name, default);
            state_fields.push(field.name.name.clone());
        }

        // Register credentials (as nil initially — host sets them before use)
        let mut credentials = BTreeMap::new();
        if let Some(creds) = &body.credentials {
            for field in &creds.fields {
                eval.env.define(&field.name.name, Value::Nil);
                credentials.insert(field.name.name.clone(), Value::Nil);
            }
        }

        // Collect derived field definitions
        let derived_fields: Vec<(String, Expr)> = body
            .derived
            .as_ref()
            .map(|d| {
                d.fields
                    .iter()
                    .map(|f| (f.name.name.clone(), f.value.clone()))
                    .collect()
            })
            .unwrap_or_default();

        // Collect invariants
        let invariants: Vec<(String, Expr)> = body
            .invariants
            .iter()
            .map(|inv| (inv.name.name.clone(), inv.condition.clone()))
            .collect();

        let mut instance = Self {
            eval,
            state_fields,
            derived_fields,
            invariants,
            actions: body.actions.clone(),
            views: body.views.clone(),
            credentials,
            update_decl: body.update.clone(),
            handle_event_decl: body.handle_event.clone(),
            mock_responses: Vec::new(),
        };

        // Compute initial derived fields
        instance.recompute_derived()?;

        Ok(instance)
    }

    // ══════════════════════════════════════════════════════════════════════
    // State access
    // ══════════════════════════════════════════════════════════════════════

    /// Get the current value of a state field.
    pub fn get_state(&self, name: &str) -> Option<&Value> {
        self.eval.env.get(name)
    }

    /// Get all state as a snapshot.
    pub fn state_snapshot(&self) -> BTreeMap<String, Value> {
        let mut snap = BTreeMap::new();
        for name in &self.state_fields {
            if let Some(val) = self.eval.env.get(name) {
                snap.insert(name.clone(), val.clone());
            }
        }
        snap
    }

    /// Get captured log output.
    pub fn log_output(&self) -> &[String] {
        &self.eval.log_output
    }

    /// Clear log output.
    pub fn clear_log(&mut self) {
        self.eval.log_output.clear();
    }

    /// Set a credential value (called by host before actions).
    pub fn set_credential(&mut self, name: &str, value: Value) {
        self.credentials.insert(name.to_string(), value.clone());
        self.eval.env.define(name, value);
    }

    // ══════════════════════════════════════════════════════════════════════
    // Action dispatch
    // ══════════════════════════════════════════════════════════════════════

    /// Dispatch an action by name with arguments.
    ///
    /// Implements atomic transactions:
    /// 1. Snapshot pre-action state
    /// 2. Execute action body
    /// 3. Check invariants
    /// 4. Commit or rollback
    pub fn dispatch(&mut self, action_name: &str, args: Vec<Value>) -> EvalResult<ActionResult> {
        // Find the action
        let action = self
            .actions
            .iter()
            .find(|a| a.name.name == action_name)
            .cloned()
            .ok_or_else(|| EvalError::UndefinedAction(action_name.to_string()))?;

        // Snapshot pre-action state
        let snapshot = self.eval.env.global_bindings().clone();

        // Push action scope, bind parameters
        self.eval.env.push_scope();
        for (param, arg) in action.params.iter().zip(args.into_iter()) {
            self.eval.env.define(&param.name.name, arg);
        }

        // Execute action body
        let exec_result = self.eval.eval_block(&action.body);
        self.eval.env.pop_scope();

        // Handle return (early exit — prior set statements applied)
        match exec_result {
            Ok(_) => {}
            Err(EvalError::Return(_)) => {} // Return is expected — prior sets are kept
            Err(e) => return Err(e),
        }

        // Recompute derived fields before invariant check
        self.recompute_derived()?;

        // Check invariants
        match self.check_invariants() {
            Ok(()) => Ok(ActionResult {
                committed: true,
                invariant_error: None,
            }),
            Err(msg) => {
                // Rollback to pre-action state
                self.eval.env.restore_global(snapshot);
                // Recompute derived with rolled-back state
                self.recompute_derived()?;
                Ok(ActionResult {
                    committed: false,
                    invariant_error: Some(msg),
                })
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Derived fields
    // ══════════════════════════════════════════════════════════════════════

    /// Recompute all derived fields in declaration order.
    fn recompute_derived(&mut self) -> EvalResult<()> {
        for (name, expr) in &self.derived_fields.clone() {
            let val = self.eval.eval_expr(expr)?;
            // Define/update derived field in global scope
            if !self.eval.env.set(name, val.clone()) {
                self.eval.env.define(name, val);
            }
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════════
    // Invariant checking
    // ══════════════════════════════════════════════════════════════════════

    /// Check all invariants. Returns Ok(()) if all pass, Err(message) if one fails.
    fn check_invariants(&mut self) -> Result<(), String> {
        for (name, condition) in &self.invariants.clone() {
            match self.eval.eval_expr(condition) {
                Ok(val) => {
                    if !val.is_truthy() {
                        return Err(format!("invariant '{name}' violated"));
                    }
                }
                Err(e) => {
                    return Err(format!("invariant '{name}' evaluation error: {e}"));
                }
            }
        }
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════════
    // View rendering
    // ══════════════════════════════════════════════════════════════════════

    /// Render the main view (or a named view) to a Surface tree.
    pub fn render_view(&mut self, view_name: &str) -> EvalResult<Vec<SurfaceNode>> {
        let view = self
            .views
            .iter()
            .find(|v| v.name.name == view_name)
            .cloned()
            .ok_or_else(|| EvalError::Runtime(format!("unknown view '{view_name}'")))?;

        self.eval_ui_block(&view.body)
    }

    /// Render the default "main" view.
    pub fn render(&mut self) -> EvalResult<Vec<SurfaceNode>> {
        self.render_view("main")
    }

    fn eval_ui_block(&mut self, block: &UIBlock) -> EvalResult<Vec<SurfaceNode>> {
        let mut nodes = Vec::new();
        for elem in &block.elements {
            self.eval_ui_element(elem, &mut nodes)?;
        }
        Ok(nodes)
    }

    fn eval_ui_element(
        &mut self,
        elem: &UIElement,
        out: &mut Vec<SurfaceNode>,
    ) -> EvalResult<()> {
        match elem {
            UIElement::Component(comp) => {
                let node = self.eval_component(comp)?;
                out.push(node);
            }
            UIElement::Let(binding) => {
                let value = self.eval.eval_expr(&binding.value)?;
                if let Some(name) = &binding.name {
                    self.eval.env.define(&name.name, value);
                }
            }
            UIElement::If(ui_if) => {
                self.eval_ui_if(ui_if, out)?;
            }
            UIElement::For(ui_for) => {
                self.eval_ui_for(ui_for, out)?;
            }
        }
        Ok(())
    }

    fn eval_component(&mut self, comp: &ComponentExpr) -> EvalResult<SurfaceNode> {
        let mut props = BTreeMap::new();
        for prop in &comp.props {
            let name = &prop.name.name;
            // Handle action references: on_tap, on_change, etc.
            if name.starts_with("on_") {
                match &prop.value.kind {
                    ExprKind::Identifier(action_name) => {
                        // Direct action reference: on_tap: increment
                        let mut action_props = BTreeMap::new();
                        action_props.insert(
                            "__action".to_string(),
                            Value::String(action_name.clone()),
                        );
                        props.insert(
                            name.clone(),
                            Value::Record {
                                type_name: None,
                                fields: action_props,
                            },
                        );
                        continue;
                    }
                    ExprKind::Call {
                        name: fn_name,
                        args,
                    } => {
                        // Action call with args: on_tap: toggle(index)
                        let mut action_props = BTreeMap::new();
                        action_props.insert(
                            "__action".to_string(),
                            Value::String(fn_name.name.clone()),
                        );
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            arg_vals.push(self.eval.eval_expr(arg)?);
                        }
                        action_props.insert("__args".to_string(), Value::List(arg_vals));
                        props.insert(
                            name.clone(),
                            Value::Record {
                                type_name: None,
                                fields: action_props,
                            },
                        );
                        continue;
                    }
                    ExprKind::Lambda(lambda) => {
                        // Lambda callback: on_change: fn(v) { ... }
                        // or on_change: update_input (which is an action name)
                        let closure = self.eval.eval_lambda(lambda)?;
                        let mut lambda_props = BTreeMap::new();
                        lambda_props.insert("__lambda".to_string(), closure);
                        props.insert(
                            name.clone(),
                            Value::Record {
                                type_name: None,
                                fields: lambda_props,
                            },
                        );
                        continue;
                    }
                    _ => {}
                }
            }

            // Normal prop evaluation
            let val = self.eval.eval_expr(&prop.value)?;
            props.insert(name.clone(), val);
        }

        let children = if let Some(child_block) = &comp.children {
            self.eval_ui_block(child_block)?
        } else {
            Vec::new()
        };

        Ok(SurfaceNode {
            component: comp.name.name.clone(),
            props,
            children,
        })
    }

    fn eval_ui_if(
        &mut self,
        ui_if: &UIIf,
        out: &mut Vec<SurfaceNode>,
    ) -> EvalResult<()> {
        let cond = self.eval.eval_expr(&ui_if.condition)?;
        if cond.is_truthy() {
            let nodes = self.eval_ui_block(&ui_if.then_block)?;
            out.extend(nodes);
        } else if let Some(else_block) = &ui_if.else_block {
            match else_block {
                UIElse::ElseIf(elif) => self.eval_ui_if(elif, out)?,
                UIElse::Block(block) => {
                    let nodes = self.eval_ui_block(block)?;
                    out.extend(nodes);
                }
            }
        }
        Ok(())
    }

    fn eval_ui_for(
        &mut self,
        ui_for: &UIFor,
        out: &mut Vec<SurfaceNode>,
    ) -> EvalResult<()> {
        let iterable = self.eval.eval_expr(&ui_for.iterable)?;
        let items = match iterable {
            Value::List(items) => items,
            _ => {
                return Err(EvalError::TypeMismatch(format!(
                    "for loop requires list, got {}",
                    iterable.type_name()
                )));
            }
        };

        self.eval.env.push_scope();
        for (i, item) in items.iter().enumerate() {
            self.eval.env.define(&ui_for.item.name, item.clone());
            if let Some(idx) = &ui_for.index {
                self.eval.env.define(&idx.name, Value::Number(i as f64));
            }
            let nodes = self.eval_ui_block(&ui_for.body)?;
            out.extend(nodes);
        }
        self.eval.env.pop_scope();
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════════════
    // Test runner helpers
    // ══════════════════════════════════════════════════════════════════════

    /// Install mock capability responses (used by test runner).
    pub fn set_mock_responses(&mut self, mocks: Vec<MockResponse>) {
        // Propagate to evaluator for stdlib dispatch
        self.eval.mock_responses = mocks
            .iter()
            .map(|m| (m.module.clone(), m.function.clone(), m.response.clone()))
            .collect();
        self.mock_responses = mocks;
    }

    /// Evaluate an expression via the internal evaluator (public for test runner).
    pub fn eval_expr_public(&mut self, expr: &Expr) -> EvalResult<Value> {
        self.eval.eval_expr(expr)
    }

    /// Execute a statement via the internal evaluator (public for test runner).
    pub fn eval_stmt_public(&mut self, stmt: &Stmt) -> EvalResult<Value> {
        self.eval.eval_stmt(stmt)
    }

    /// Define a variable in the current environment scope (public for test runner).
    pub fn define_in_env(&mut self, name: &str, value: Value) {
        self.eval.env.define(name, value);
    }

    /// Push a new scope in the environment (public for test runner).
    pub fn push_scope(&mut self) {
        self.eval.env.push_scope();
    }

    /// Pop the innermost scope (public for test runner).
    pub fn pop_scope(&mut self) {
        self.eval.env.pop_scope();
    }

    // ══════════════════════════════════════════════════════════════════════
    // Game loop
    // ══════════════════════════════════════════════════════════════════════

    /// Call `update(dt)` — game loop tick.
    ///
    /// Like an action dispatch: atomic, with invariant checking and rollback.
    pub fn call_update(&mut self, dt: f64) -> EvalResult<ActionResult> {
        let update = self
            .update_decl
            .clone()
            .ok_or_else(|| EvalError::Runtime("space has no update() declaration".into()))?;

        let snapshot = self.eval.env.global_bindings().clone();

        self.eval.env.push_scope();
        self.eval
            .env
            .define(&update.param.name.name, Value::Number(dt));

        let exec_result = self.eval.eval_block(&update.body);
        self.eval.env.pop_scope();

        match exec_result {
            Ok(_) => {}
            Err(EvalError::Return(_)) => {}
            Err(e) => return Err(e),
        }

        self.recompute_derived()?;

        match self.check_invariants() {
            Ok(()) => Ok(ActionResult {
                committed: true,
                invariant_error: None,
            }),
            Err(msg) => {
                self.eval.env.restore_global(snapshot);
                self.recompute_derived()?;
                Ok(ActionResult {
                    committed: false,
                    invariant_error: Some(msg),
                })
            }
        }
    }

    /// Call `handleEvent(event)` — game loop event handler.
    ///
    /// Like an action dispatch: atomic, with invariant checking and rollback.
    pub fn call_handle_event(&mut self, event: Value) -> EvalResult<ActionResult> {
        let handler = self
            .handle_event_decl
            .clone()
            .ok_or_else(|| {
                EvalError::Runtime("space has no handleEvent() declaration".into())
            })?;

        let snapshot = self.eval.env.global_bindings().clone();

        self.eval.env.push_scope();
        self.eval
            .env
            .define(&handler.param.name.name, event);

        let exec_result = self.eval.eval_block(&handler.body);
        self.eval.env.pop_scope();

        match exec_result {
            Ok(_) => {}
            Err(EvalError::Return(_)) => {}
            Err(e) => return Err(e),
        }

        self.recompute_derived()?;

        match self.check_invariants() {
            Ok(()) => Ok(ActionResult {
                committed: true,
                invariant_error: None,
            }),
            Err(msg) => {
                self.eval.env.restore_global(snapshot);
                self.recompute_derived()?;
                Ok(ActionResult {
                    committed: false,
                    invariant_error: Some(msg),
                })
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Surface serialization
    // ══════════════════════════════════════════════════════════════════════

    /// Serialize a surface tree to JSON.
    pub fn surface_to_json(nodes: &[SurfaceNode]) -> serde_json::Value {
        serde_json::Value::Array(nodes.iter().map(Self::node_to_json).collect())
    }

    fn node_to_json(node: &SurfaceNode) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert(
            "component".to_string(),
            serde_json::Value::String(node.component.clone()),
        );

        let mut props_map = serde_json::Map::new();
        for (k, v) in &node.props {
            props_map.insert(k.clone(), Self::value_to_json(v));
        }
        map.insert("props".to_string(), serde_json::Value::Object(props_map));

        if !node.children.is_empty() {
            map.insert(
                "children".to_string(),
                serde_json::Value::Array(node.children.iter().map(Self::node_to_json).collect()),
            );
        }

        serde_json::Value::Object(map)
    }

    fn value_to_json(val: &Value) -> serde_json::Value {
        Self::value_to_json_public(val)
    }

    /// Convert a Value to JSON (public for golden reference generation).
    pub fn value_to_json_public(val: &Value) -> serde_json::Value {
        match val {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                    serde_json::Value::Number(serde_json::Number::from(*n as i64))
                } else {
                    serde_json::json!(*n)
                }
            }
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Nil => serde_json::Value::Null,
            Value::List(items) => {
                serde_json::Value::Array(items.iter().map(Self::value_to_json).collect())
            }
            Value::Record { fields, .. } => {
                let mut map = serde_json::Map::new();
                for (k, v) in fields {
                    map.insert(k.clone(), Self::value_to_json(v));
                }
                serde_json::Value::Object(map)
            }
            Value::Result(r) => match r.as_ref() {
                ResultValue::Ok(v) => serde_json::json!({"Ok": Self::value_to_json(v)}),
                ResultValue::Err(v) => serde_json::json!({"Err": Self::value_to_json(v)}),
            },
            Value::SumVariant { variant, fields, .. } => {
                if fields.is_empty() {
                    serde_json::Value::String(variant.clone())
                } else {
                    serde_json::json!({
                        variant: fields.iter().map(Self::value_to_json).collect::<Vec<_>>()
                    })
                }
            }
            Value::Function(_) => serde_json::Value::String("<function>".to_string()),
            Value::Color { r, g, b, a } => {
                serde_json::json!({"r": r, "g": g, "b": b, "a": a})
            }
        }
    }
}

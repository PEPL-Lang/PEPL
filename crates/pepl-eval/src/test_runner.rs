//! PEPL test runner — executes `tests { }` blocks from PEPL programs.
//!
//! Each test case creates a fresh SpaceInstance and dispatches actions
//! by calling them as functions. `with_responses { }` provides mock
//! capability call results.

use crate::error::{EvalError, EvalResult};
use crate::space::SpaceInstance;
use pepl_stdlib::Value;
use pepl_types::ast::*;

/// Result of running a single test case.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Test description (from `test "description" { ... }`).
    pub description: String,
    /// Whether the test passed.
    pub passed: bool,
    /// Error message if the test failed.
    pub error: Option<String>,
}

impl std::fmt::Display for TestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.passed {
            write!(f, "  ✓ {}", self.description)
        } else {
            write!(
                f,
                "  ✗ {} — {}",
                self.description,
                self.error.as_deref().unwrap_or("unknown error")
            )
        }
    }
}

/// Summary of running all test blocks.
#[derive(Debug)]
pub struct TestRunSummary {
    pub results: Vec<TestResult>,
    pub passed: usize,
    pub failed: usize,
}

impl std::fmt::Display for TestRunSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for r in &self.results {
            writeln!(f, "{r}")?;
        }
        writeln!(f, "\n{} passed, {} failed", self.passed, self.failed)
    }
}

/// A mocked capability response: (module, function) → response Value.
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub module: String,
    pub function: String,
    pub response: Value,
}

/// Run all test blocks in a PEPL program.
///
/// Each test case gets a fresh `SpaceInstance`. Actions are dispatched
/// by executing test body statements that call actions as functions.
pub fn run_tests(program: &Program) -> EvalResult<TestRunSummary> {
    let mut results = Vec::new();

    for test_block in &program.tests {
        for case in &test_block.cases {
            let result = run_single_test(program, case)?;
            results.push(result);
        }
    }

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    Ok(TestRunSummary {
        results,
        passed,
        failed,
    })
}

/// Run a single test case with a fresh SpaceInstance.
fn run_single_test(program: &Program, case: &TestCase) -> EvalResult<TestResult> {
    // Resolve mock responses from `with_responses` block
    let mocks = resolve_mocks(program, case)?;

    // Create a fresh space instance for this test
    let mut instance = SpaceInstance::new(program)?;

    // Install mock responses
    if !mocks.is_empty() {
        instance.set_mock_responses(mocks);
    }

    // Execute the test body — statements that dispatch actions and check assertions
    let exec_result = execute_test_body(&mut instance, &case.body, &program.space.body);

    match exec_result {
        Ok(()) => Ok(TestResult {
            description: case.description.clone(),
            passed: true,
            error: None,
        }),
        Err(EvalError::AssertionFailed(msg)) => Ok(TestResult {
            description: case.description.clone(),
            passed: false,
            error: Some(msg),
        }),
        Err(e) => Ok(TestResult {
            description: case.description.clone(),
            passed: false,
            error: Some(format!("{e}")),
        }),
    }
}

/// Resolve `with_responses { ... }` into mock capability responses.
fn resolve_mocks(program: &Program, case: &TestCase) -> EvalResult<Vec<MockResponse>> {
    let mut mocks = Vec::new();

    if let Some(with_responses) = &case.with_responses {
        // Create a temporary evaluator for evaluating response expressions
        let mut temp_instance = SpaceInstance::new(program)?;

        for mapping in &with_responses.mappings {
            let response = temp_instance.eval_expr_public(&mapping.response)?;
            mocks.push(MockResponse {
                module: mapping.module.name.clone(),
                function: mapping.function.name.clone(),
                response,
            });
        }
    }

    Ok(mocks)
}

/// Execute the body of a test case.
///
/// In test bodies, unqualified function calls dispatch actions:
///   `increment()` → dispatches the `increment` action
///   `add_todo()` → dispatches the `add_todo` action
///
/// Statements like `assert`, `let`, `if`, `for`, `match` work as normal.
fn execute_test_body(
    instance: &mut SpaceInstance,
    body: &Block,
    space_body: &SpaceBody,
) -> EvalResult<()> {
    for stmt in &body.stmts {
        execute_test_stmt(instance, stmt, space_body)?;
    }
    Ok(())
}

/// Execute a single statement in test context.
fn execute_test_stmt(
    instance: &mut SpaceInstance,
    stmt: &Stmt,
    space_body: &SpaceBody,
) -> EvalResult<()> {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            execute_test_expr(instance, &expr_stmt.expr, space_body)?;
            Ok(())
        }
        Stmt::Assert(assert) => {
            // Evaluate the assertion condition using the space's evaluator
            let val = instance.eval_expr_public(&assert.condition)?;
            if !val.is_truthy() {
                let msg = assert
                    .message
                    .clone()
                    .unwrap_or_else(|| "assertion failed".into());
                return Err(EvalError::AssertionFailed(msg));
            }
            Ok(())
        }
        Stmt::Let(binding) => {
            let value = instance.eval_expr_public(&binding.value)?;
            if let Some(name) = &binding.name {
                instance.define_in_env(&name.name, value);
            }
            Ok(())
        }
        Stmt::If(if_expr) => {
            let cond = instance.eval_expr_public(&if_expr.condition)?;
            if cond.is_truthy() {
                execute_test_body(instance, &if_expr.then_block, space_body)?;
            } else if let Some(else_branch) = &if_expr.else_branch {
                match else_branch {
                    ElseBranch::ElseIf(elif) => {
                        let cond = instance.eval_expr_public(&elif.condition)?;
                        if cond.is_truthy() {
                            execute_test_body(instance, &elif.then_block, space_body)?;
                        }
                    }
                    ElseBranch::Block(block) => {
                        execute_test_body(instance, block, space_body)?;
                    }
                }
            }
            Ok(())
        }
        Stmt::For(for_expr) => {
            let iterable = instance.eval_expr_public(&for_expr.iterable)?;
            if let Value::List(items) = iterable {
                for (i, item) in items.iter().enumerate() {
                    instance.push_scope();
                    instance.define_in_env(&for_expr.item.name, item.clone());
                    if let Some(idx) = &for_expr.index {
                        instance.define_in_env(&idx.name, Value::Number(i as f64));
                    }
                    execute_test_body(instance, &for_expr.body, space_body)?;
                    instance.pop_scope();
                }
            }
            Ok(())
        }
        _ => {
            // Other statements (set, match, return) — evaluate normally
            instance.eval_stmt_public(stmt)?;
            Ok(())
        }
    }
}

/// Execute an expression in test context.
///
/// Unqualified calls are dispatched as actions.
fn execute_test_expr(
    instance: &mut SpaceInstance,
    expr: &Expr,
    space_body: &SpaceBody,
) -> EvalResult<Value> {
    match &expr.kind {
        ExprKind::Call { name, args } => {
            // Check if this is an action dispatch
            let is_action = space_body
                .actions
                .iter()
                .any(|a| a.name.name == name.name);

            if is_action {
                let mut arg_vals = Vec::new();
                for arg in args {
                    arg_vals.push(instance.eval_expr_public(arg)?);
                }
                let result = instance.dispatch(&name.name, arg_vals)?;
                if !result.committed {
                    if let Some(err) = result.invariant_error {
                        return Err(EvalError::InvariantViolation(err));
                    }
                }
                Ok(Value::Nil)
            } else {
                instance.eval_expr_public(expr)
            }
        }
        _ => instance.eval_expr_public(expr),
    }
}

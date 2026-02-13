//! Test block codegen — compiles PEPL `tests { }` blocks to WASM functions.
//!
//! Each test case `test "description" { ... }` becomes a WASM function
//! `__test_N()` that:
//! 1. Re-initialises state (calls init() internally)
//! 2. Executes the test body (action dispatches, assertions, let bindings)
//! 3. Returns void on success, or traps on assertion failure
//!
//! The host calls `__test_count()` to discover how many tests exist,
//! then `__test_N()` (N = 0, 1, ...) to run each.

use std::collections::HashMap;
use wasm_encoder::{Function, Instruction};

use crate::compiler::FuncContext;
use crate::error::CodegenResult;
use crate::runtime::memarg;
use crate::types::*;

use pepl_types::ast::*;

/// Compile a single test body into WASM instructions.
///
/// Emits:
/// 1. `call init` — reset state to defaults
/// 2. Compiled test body statements
pub fn emit_test_body(
    body: &Block,
    actions: &HashMap<String, u32>,
    dispatch_func_idx: u32,
    init_func_idx: u32,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Re-initialise state
    f.instruction(&Instruction::Call(init_func_idx));

    // Compile each test statement
    for stmt in &body.stmts {
        emit_test_stmt(stmt, actions, dispatch_func_idx, ctx, f)?;
    }
    Ok(())
}

/// Compile a single test statement.
fn emit_test_stmt(
    stmt: &Stmt,
    actions: &HashMap<String, u32>,
    dispatch_func_idx: u32,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    match stmt {
        Stmt::Expr(expr_stmt) => {
            if is_action_call(&expr_stmt.expr, actions) {
                emit_action_dispatch(&expr_stmt.expr, actions, dispatch_func_idx, f)
            } else {
                crate::expr::emit_expr(&expr_stmt.expr, ctx, f)?;
                f.instruction(&Instruction::Drop);
                Ok(())
            }
        }
        Stmt::Assert(assert_stmt) => emit_test_assert(assert_stmt, ctx, f),
        Stmt::Let(binding) => {
            crate::expr::emit_expr(&binding.value, ctx, f)?;
            if let Some(name) = &binding.name {
                let local = ctx.alloc_local(wasm_encoder::ValType::I32);
                f.instruction(&Instruction::LocalSet(local));
                ctx.push_local(&name.name, local);
            } else {
                f.instruction(&Instruction::Drop);
            }
            Ok(())
        }
        _ => crate::stmt::emit_stmt(stmt, ctx, f),
    }
}

/// Check if an expression is an action dispatch call.
fn is_action_call(expr: &Expr, actions: &HashMap<String, u32>) -> bool {
    matches!(&expr.kind, ExprKind::Call { name, .. } if actions.contains_key(&name.name))
}

/// Emit action dispatch call.
fn emit_action_dispatch(
    expr: &Expr,
    actions: &HashMap<String, u32>,
    dispatch_func_idx: u32,
    f: &mut Function,
) -> CodegenResult<()> {
    if let ExprKind::Call { name, .. } = &expr.kind {
        let action_id = actions[&name.name];
        f.instruction(&Instruction::I32Const(action_id as i32));
        f.instruction(&Instruction::I32Const(0)); // payload_ptr
        f.instruction(&Instruction::I32Const(0)); // payload_len
        f.instruction(&Instruction::Call(dispatch_func_idx));
    }
    Ok(())
}

/// Compile `assert condition [, "message"]` — traps if condition is false.
fn emit_test_assert(
    assert: &AssertStmt,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    crate::expr::emit_expr(&assert.condition, ctx, f)?;

    let val_local = ctx.alloc_local(wasm_encoder::ValType::I32);
    f.instruction(&Instruction::LocalSet(val_local));
    f.instruction(&Instruction::LocalGet(val_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // w1: bool payload
    f.instruction(&Instruction::I32Eqz); // true → assertion failed

    f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
    let msg = assert
        .message
        .clone()
        .unwrap_or_else(|| "assertion failed".to_string());
    let (msg_ptr, msg_len) = ctx.intern_string(&msg);
    f.instruction(&Instruction::I32Const(msg_ptr as i32));
    f.instruction(&Instruction::I32Const(msg_len as i32));
    f.instruction(&Instruction::Call(IMPORT_TRAP));
    f.instruction(&Instruction::End);

    Ok(())
}

/// Emit `__test_count() -> i32`.
pub fn emit_test_count(count: usize) -> Function {
    let mut f = Function::new(vec![]);
    f.instruction(&Instruction::I32Const(count as i32));
    f.instruction(&Instruction::End);
    f
}

//! Expression code generation.
//!
//! Every expression evaluates to an i32 *value pointer* left on the WASM
//! operand stack.  The caller can then store it, pass it to another function,
//! or ignore it.

use pepl_types::ast::*;
use wasm_encoder::{BlockType, Function, Instruction, ValType};

use crate::compiler::FuncContext;
use crate::error::CodegenResult;
use crate::gas;
use crate::runtime::*;
use crate::stmt::emit_stmts;
use crate::types::*;

/// Emit instructions for an expression.  Leaves one i32 (value ptr) on stack.
pub fn emit_expr(expr: &Expr, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    match &expr.kind {
        // ── Literals ──────────────────────────────────────────────────────
        ExprKind::NumberLit(n) => emit_number_lit(*n, ctx, f),
        ExprKind::StringLit(s) => emit_string_lit(s, ctx, f),
        ExprKind::BoolLit(b) => emit_bool_lit(*b, f),
        ExprKind::NilLit => emit_nil_lit(f),
        ExprKind::ListLit(elems) => emit_list_lit(elems, ctx, f),
        ExprKind::RecordLit(entries) => emit_record_lit(entries, ctx, f),
        ExprKind::StringInterpolation(parts) => emit_string_interpolation(parts, ctx, f),

        // ── Identifiers ──────────────────────────────────────────────────
        ExprKind::Identifier(name) => emit_identifier(name, ctx, f),

        // ── Calls ────────────────────────────────────────────────────────
        ExprKind::Call { name, args } => emit_call(&name.name, args, ctx, f),
        ExprKind::QualifiedCall {
            module,
            function,
            args,
        } => emit_qualified_call(&module.name, &function.name, args, ctx, f),
        ExprKind::FieldAccess { object, field } => {
            emit_field_access(object, &field.name, ctx, f)
        }
        ExprKind::MethodCall {
            object,
            method,
            args,
        } => emit_method_call(object, &method.name, args, ctx, f),

        // ── Operators ────────────────────────────────────────────────────
        ExprKind::Binary { left, op, right } => emit_binary(left, *op, right, ctx, f),
        ExprKind::Unary { op, operand } => emit_unary(*op, operand, ctx, f),
        ExprKind::ResultUnwrap(inner) => emit_result_unwrap(inner, ctx, f),
        ExprKind::NilCoalesce { left, right } => emit_nil_coalesce(left, right, ctx, f),

        // ── Control Flow ─────────────────────────────────────────────────
        ExprKind::If(if_expr) => emit_if_expr(if_expr, ctx, f),
        ExprKind::For(for_expr) => emit_for_expr(for_expr, ctx, f),
        ExprKind::Match(match_expr) => emit_match_expr(match_expr, ctx, f),

        // ── Lambda ───────────────────────────────────────────────────────
        ExprKind::Lambda(_lambda) => {
            // Lambda closures are lowered in a later phase.
            // For now emit nil as a placeholder.
            emit_nil_lit(f)
        }

        // ── Grouping ─────────────────────────────────────────────────────
        ExprKind::Paren(inner) => emit_expr(inner, ctx, f),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Literal emission
// ══════════════════════════════════════════════════════════════════════════════

fn emit_number_lit(n: f64, _ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // Allocate a VALUE_SIZE cell, write tag + f64 directly.
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    // duplicate ptr for tag store and f64 store
    let local = _ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::LocalTee(local));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(local));
    f.instruction(&Instruction::F64Const(n));
    f.instruction(&Instruction::F64Store(memarg(4, 3)));
    f.instruction(&Instruction::LocalGet(local));
    Ok(())
}

fn emit_string_lit(s: &str, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    let (ptr, len) = ctx.intern_string(s);
    f.instruction(&Instruction::I32Const(ptr as i32));
    f.instruction(&Instruction::I32Const(len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));
    Ok(())
}

fn emit_bool_lit(b: bool, f: &mut Function) -> CodegenResult<()> {
    f.instruction(&Instruction::I32Const(if b { 1 } else { 0 }));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_BOOL)));
    Ok(())
}

fn emit_nil_lit(f: &mut Function) -> CodegenResult<()> {
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    Ok(())
}

fn emit_list_lit(
    elems: &[Expr],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    let count = elems.len() as i32;
    if count == 0 {
        // Empty list: arr_ptr = 0, count = 0
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
        return Ok(());
    }

    // Allocate array of i32 pointers
    let arr_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const(count * 4));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(arr_local));

    // Evaluate each element and store its pointer
    for (i, elem) in elems.iter().enumerate() {
        let elem_local = ctx.alloc_local(ValType::I32);
        emit_expr(elem, ctx, f)?;
        f.instruction(&Instruction::LocalSet(elem_local));
        // arr[i] = elem_ptr
        f.instruction(&Instruction::LocalGet(arr_local));
        f.instruction(&Instruction::LocalGet(elem_local));
        f.instruction(&Instruction::I32Store(memarg(i as u64 * 4, 2)));
    }

    // Create list value
    f.instruction(&Instruction::LocalGet(arr_local));
    f.instruction(&Instruction::I32Const(count));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    Ok(())
}

fn emit_record_lit(
    entries: &[RecordEntry],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Count only Field entries (spreads are handled inline)
    let field_count: usize = entries
        .iter()
        .filter(|e| matches!(e, RecordEntry::Field { .. }))
        .count();

    if field_count == 0 && entries.is_empty() {
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
        return Ok(());
    }

    // Each entry is 12 bytes: [key_offset: i32, key_len: i32, value_ptr: i32]
    let entries_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const((field_count * 12) as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(entries_local));

    let mut idx = 0usize;
    for entry in entries {
        match entry {
            RecordEntry::Field { name, value } => {
                let (key_ptr, key_len) = ctx.intern_string(&name.name);
                let val_local = ctx.alloc_local(ValType::I32);
                emit_expr(value, ctx, f)?;
                f.instruction(&Instruction::LocalSet(val_local));

                let base_offset = (idx * 12) as u64;
                // key_offset
                f.instruction(&Instruction::LocalGet(entries_local));
                f.instruction(&Instruction::I32Const(key_ptr as i32));
                f.instruction(&Instruction::I32Store(memarg(base_offset, 2)));
                // key_len
                f.instruction(&Instruction::LocalGet(entries_local));
                f.instruction(&Instruction::I32Const(key_len as i32));
                f.instruction(&Instruction::I32Store(memarg(base_offset + 4, 2)));
                // value_ptr
                f.instruction(&Instruction::LocalGet(entries_local));
                f.instruction(&Instruction::LocalGet(val_local));
                f.instruction(&Instruction::I32Store(memarg(base_offset + 8, 2)));
                idx += 1;
            }
            RecordEntry::Spread(_spread_expr) => {
                // Spread requires copying all fields from the source record
                // into the target. For now, emit nothing (TODO).
            }
        }
    }

    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(field_count as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
    Ok(())
}

fn emit_string_interpolation(
    parts: &[StringPart],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Build string by concatenating parts left to right.
    // Start with empty string, concat each part.
    let (empty_ptr, empty_len) = ctx.intern_string("");
    f.instruction(&Instruction::I32Const(empty_ptr as i32));
    f.instruction(&Instruction::I32Const(empty_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));

    for part in parts {
        match part {
            StringPart::Literal(s) => {
                if !s.is_empty() {
                    emit_string_lit(s, ctx, f)?;
                    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING_CONCAT)));
                }
            }
            StringPart::Expr(expr) => {
                emit_expr(expr, ctx, f)?;
                f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_TO_STRING)));
                f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING_CONCAT)));
            }
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Identifiers & Calls
// ══════════════════════════════════════════════════════════════════════════════

fn emit_identifier(name: &str, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // Look up in locals first, then state fields, then action names
    if let Some(local_idx) = ctx.get_local(name) {
        f.instruction(&Instruction::LocalGet(local_idx));
        return Ok(());
    }

    // State field access: record_get(state_ptr, key)
    if ctx.is_state_field(name) {
        let (key_ptr, key_len) = ctx.intern_string(name);
        f.instruction(&Instruction::GlobalGet(GLOBAL_STATE_PTR));
        f.instruction(&Instruction::I32Const(key_ptr as i32));
        f.instruction(&Instruction::I32Const(key_len as i32));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD_GET)));
        return Ok(());
    }

    // Action reference
    if let Some(action_id) = ctx.get_action_id(name) {
        f.instruction(&Instruction::I32Const(action_id as i32));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_ACTION_REF)));
        return Ok(());
    }

    // Unknown — return nil with a note
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    Ok(())
}

fn emit_call(
    name: &str,
    args: &[Expr],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Gas tick at every call site
    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    // Check if this is a locally-defined function (action call, etc.)
    if let Some(func_idx) = ctx.get_function(name) {
        // Push args
        for arg in args {
            emit_expr(arg, ctx, f)?;
        }
        f.instruction(&Instruction::Call(func_idx));
        return Ok(());
    }

    // Unknown function — eval args and discard, return nil
    for arg in args {
        emit_expr(arg, ctx, f)?;
        f.instruction(&Instruction::Drop);
    }
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    Ok(())
}

fn emit_qualified_call(
    module: &str,
    function: &str,
    args: &[Expr],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    // Stdlib calls are dispatched via host_call for capability modules,
    // or handled inline for pure modules (math, string, list, etc.).
    // For now, we lower all qualified calls to host_call with serialized args.

    // Evaluate args into a list value
    let args_local = ctx.alloc_local(ValType::I32);
    if args.is_empty() {
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    } else {
        // Build args list
        let arr_local = ctx.alloc_local(ValType::I32);
        let count = args.len() as i32;
        f.instruction(&Instruction::I32Const(count * 4));
        f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
        f.instruction(&Instruction::LocalSet(arr_local));
        for (i, arg) in args.iter().enumerate() {
            let tmp = ctx.alloc_local(ValType::I32);
            emit_expr(arg, ctx, f)?;
            f.instruction(&Instruction::LocalSet(tmp));
            f.instruction(&Instruction::LocalGet(arr_local));
            f.instruction(&Instruction::LocalGet(tmp));
            f.instruction(&Instruction::I32Store(memarg(i as u64 * 4, 2)));
        }
        f.instruction(&Instruction::LocalGet(arr_local));
        f.instruction(&Instruction::I32Const(count));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    }
    f.instruction(&Instruction::LocalSet(args_local));

    // Intern module and function names to resolve cap_id/fn_id at compile time
    let (mod_id, fn_id) = ctx.resolve_qualified_call(module, function);

    // host_call(cap_id, fn_id, args_ptr) -> result_ptr
    f.instruction(&Instruction::I32Const(mod_id as i32));
    f.instruction(&Instruction::I32Const(fn_id as i32));
    f.instruction(&Instruction::LocalGet(args_local));
    f.instruction(&Instruction::Call(IMPORT_HOST_CALL));

    Ok(())
}

fn emit_field_access(
    object: &Expr,
    field: &str,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    emit_expr(object, ctx, f)?;
    let (key_ptr, key_len) = ctx.intern_string(field);
    f.instruction(&Instruction::I32Const(key_ptr as i32));
    f.instruction(&Instruction::I32Const(key_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD_GET)));
    Ok(())
}

fn emit_method_call(
    object: &Expr,
    method: &str,
    args: &[Expr],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Method calls in PEPL are sugar for qualified calls on the receiver type.
    // E.g., `items.length()` → `list.length(items)`
    // We emit as host_call with the receiver as the first arg.
    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    let total_args = 1 + args.len();
    let arr_local = ctx.alloc_local(ValType::I32);
    let count = total_args as i32;

    f.instruction(&Instruction::I32Const(count * 4));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(arr_local));

    // Store receiver as arg[0]
    let recv_local = ctx.alloc_local(ValType::I32);
    emit_expr(object, ctx, f)?;
    f.instruction(&Instruction::LocalSet(recv_local));
    f.instruction(&Instruction::LocalGet(arr_local));
    f.instruction(&Instruction::LocalGet(recv_local));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));

    // Store remaining args
    for (i, arg) in args.iter().enumerate() {
        let tmp = ctx.alloc_local(ValType::I32);
        emit_expr(arg, ctx, f)?;
        f.instruction(&Instruction::LocalSet(tmp));
        f.instruction(&Instruction::LocalGet(arr_local));
        f.instruction(&Instruction::LocalGet(tmp));
        f.instruction(&Instruction::I32Store(memarg((i as u64 + 1) * 4, 2)));
    }

    // Determine module from method name (heuristic: number methods → math, etc.)
    // For now, dispatch all method calls via host_call module=0 fn=method_id
    let (mod_id, fn_id) = ctx.resolve_method_call(method);

    f.instruction(&Instruction::I32Const(mod_id as i32));
    f.instruction(&Instruction::I32Const(fn_id as i32));
    f.instruction(&Instruction::LocalGet(arr_local));
    f.instruction(&Instruction::I32Const(count));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    f.instruction(&Instruction::Call(IMPORT_HOST_CALL));

    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Operators
// ══════════════════════════════════════════════════════════════════════════════

fn emit_binary(
    left: &Expr,
    op: BinOp,
    right: &Expr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    match op {
        BinOp::And => {
            // Short-circuit: if left is falsy, return left; else return right
            let left_local = ctx.alloc_local(ValType::I32);
            emit_expr(left, ctx, f)?;
            f.instruction(&Instruction::LocalTee(left_local));
            // Check truthy (for bools: w1 != 0)
            f.instruction(&Instruction::I32Load(memarg(4, 2)));
            f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
            emit_expr(right, ctx, f)?;
            f.instruction(&Instruction::Else);
            f.instruction(&Instruction::LocalGet(left_local));
            f.instruction(&Instruction::End);
            return Ok(());
        }
        BinOp::Or => {
            // Short-circuit: if left is truthy, return left; else return right
            let left_local = ctx.alloc_local(ValType::I32);
            emit_expr(left, ctx, f)?;
            f.instruction(&Instruction::LocalTee(left_local));
            f.instruction(&Instruction::I32Load(memarg(4, 2)));
            f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
            f.instruction(&Instruction::LocalGet(left_local));
            f.instruction(&Instruction::Else);
            emit_expr(right, ctx, f)?;
            f.instruction(&Instruction::End);
            return Ok(());
        }
        _ => {}
    }

    // Evaluate both sides
    let a = ctx.alloc_local(ValType::I32);
    let b = ctx.alloc_local(ValType::I32);
    emit_expr(left, ctx, f)?;
    f.instruction(&Instruction::LocalSet(a));
    emit_expr(right, ctx, f)?;
    f.instruction(&Instruction::LocalSet(b));

    match op {
        BinOp::Add => {
            // Add can be number + number or string + string
            // Check if both are numbers: tag check
            // For now, assume numeric addition (the type checker should have validated)
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_ADD)));
        }
        BinOp::Sub => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_SUB)));
        }
        BinOp::Mul => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_MUL)));
        }
        BinOp::Div => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_DIV)));
        }
        BinOp::Mod => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_MOD)));
        }
        BinOp::Eq => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_EQ)));
        }
        BinOp::NotEq => {
            // not(eq(a, b))
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_EQ)));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NOT)));
        }
        BinOp::Less => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LT)));
        }
        BinOp::LessEq => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LE)));
        }
        BinOp::Greater => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_GT)));
        }
        BinOp::GreaterEq => {
            f.instruction(&Instruction::LocalGet(a));
            f.instruction(&Instruction::LocalGet(b));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_GE)));
        }
        BinOp::And | BinOp::Or => unreachable!("handled above"),
    }
    Ok(())
}

fn emit_unary(
    op: UnaryOp,
    operand: &Expr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    emit_expr(operand, ctx, f)?;
    match op {
        UnaryOp::Neg => {
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NEG)));
        }
        UnaryOp::Not => {
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NOT)));
        }
    }
    Ok(())
}

fn emit_result_unwrap(
    inner: &Expr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // `expr?` — if the result is an error variant, trap; otherwise unwrap.
    // For now, just pass through (the type checker validates this).
    emit_expr(inner, ctx, f)?;
    // TODO: check if variant tag is "Err" and trap
    Ok(())
}

fn emit_nil_coalesce(
    left: &Expr,
    right: &Expr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // `a ?? b` — if a is nil, return b; else return a
    let left_local = ctx.alloc_local(ValType::I32);
    emit_expr(left, ctx, f)?;
    f.instruction(&Instruction::LocalTee(left_local));
    // Check tag == TAG_NIL
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::I32Const(TAG_NIL));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    emit_expr(right, ctx, f)?;
    f.instruction(&Instruction::Else);
    f.instruction(&Instruction::LocalGet(left_local));
    f.instruction(&Instruction::End);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Control Flow Expressions
// ══════════════════════════════════════════════════════════════════════════════

fn emit_if_expr(
    if_expr: &IfExpr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Evaluate condition
    emit_expr(&if_expr.condition, ctx, f)?;
    // Extract bool value: load w1 (i32)
    f.instruction(&Instruction::I32Load(memarg(4, 2)));

    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));

    // Then branch — execute stmts, the last expr is the value
    emit_block_as_expr(&if_expr.then_block, ctx, f)?;

    f.instruction(&Instruction::Else);

    // Else branch
    match &if_expr.else_branch {
        Some(ElseBranch::Block(block)) => {
            emit_block_as_expr(block, ctx, f)?;
        }
        Some(ElseBranch::ElseIf(elif)) => {
            emit_if_expr(elif, ctx, f)?;
        }
        None => {
            // No else → nil
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
        }
    }

    f.instruction(&Instruction::End);
    Ok(())
}

fn emit_for_expr(
    for_expr: &ForExpr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Evaluate iterable → should be a list value
    let list_local = ctx.alloc_local(ValType::I32);
    let arr_local = ctx.alloc_local(ValType::I32);
    let count_local = ctx.alloc_local(ValType::I32);
    let i_local = ctx.alloc_local(ValType::I32);

    emit_expr(&for_expr.iterable, ctx, f)?;
    f.instruction(&Instruction::LocalSet(list_local));

    // arr_ptr = list.w1
    f.instruction(&Instruction::LocalGet(list_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalSet(arr_local));

    // count = list.w2
    f.instruction(&Instruction::LocalGet(list_local));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(count_local));

    // i = 0
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(i_local));

    // The "item" local for each iteration
    let item_local = ctx.alloc_local(ValType::I32);
    let index_local = if for_expr.index.is_some() {
        Some(ctx.alloc_local(ValType::I32))
    } else {
        None
    };

    // Register item binding
    ctx.push_local(&for_expr.item.name, item_local);
    if let (Some(idx_ident), Some(idx_local)) = (&for_expr.index, index_local) {
        ctx.push_local(&idx_ident.name, idx_local);
    }

    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));

    // Gas tick at loop boundary
    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    // break if i >= count
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::LocalGet(count_local));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1));

    // item = arr[i]
    f.instruction(&Instruction::LocalGet(arr_local));
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(4));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::LocalSet(item_local));

    // index = i (as number value)
    if let Some(idx_local) = index_local {
        // Create a number value from i
        f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
        f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
        f.instruction(&Instruction::LocalTee(idx_local));
        f.instruction(&Instruction::I32Const(TAG_NUMBER));
        f.instruction(&Instruction::I32Store(memarg(0, 2)));
        f.instruction(&Instruction::LocalGet(idx_local));
        f.instruction(&Instruction::LocalGet(i_local));
        f.instruction(&Instruction::F64ConvertI32U);
        f.instruction(&Instruction::F64Store(memarg(4, 3)));
    }

    // Execute body
    emit_stmts(&for_expr.body.stmts, ctx, f)?;

    // i += 1
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(i_local));
    f.instruction(&Instruction::Br(0));

    f.instruction(&Instruction::End); // end loop
    f.instruction(&Instruction::End); // end block

    // Pop bindings
    ctx.pop_local(&for_expr.item.name);
    if let Some(idx_ident) = &for_expr.index {
        ctx.pop_local(&idx_ident.name);
    }

    // For-expr evaluates to nil
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    Ok(())
}

fn emit_match_expr(
    match_expr: &MatchExpr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Evaluate subject
    let subj_local = ctx.alloc_local(ValType::I32);
    emit_expr(&match_expr.subject, ctx, f)?;
    f.instruction(&Instruction::LocalSet(subj_local));

    // For now, emit a simple if/else chain testing each arm's pattern.
    // Each arm: if pattern matches → execute body, else try next.
    // We wrap in a block so we can br out when a match is found.

    let result_local = ctx.alloc_local(ValType::I32);
    // Default: nil
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    f.instruction(&Instruction::LocalSet(result_local));

    // Wrap all arms in a block for early exit
    f.instruction(&Instruction::Block(BlockType::Empty));

    for arm in &match_expr.arms {
        match &arm.pattern {
            Pattern::Wildcard(_) => {
                // Always matches — execute body and break
                match &arm.body {
                    MatchArmBody::Expr(expr) => {
                        emit_expr(expr, ctx, f)?;
                    }
                    MatchArmBody::Block(block) => {
                        emit_block_as_expr(block, ctx, f)?;
                    }
                }
                f.instruction(&Instruction::LocalSet(result_local));
                f.instruction(&Instruction::Br(0));
            }
            Pattern::Variant { name, bindings } => {
                // Check if subject is a VARIANT with matching variant_id
                let vid = ctx.get_variant_id(&name.name);

                // Load subject tag
                f.instruction(&Instruction::LocalGet(subj_local));
                f.instruction(&Instruction::I32Load(memarg(0, 2)));
                f.instruction(&Instruction::I32Const(TAG_VARIANT));
                f.instruction(&Instruction::I32Eq);
                f.instruction(&Instruction::If(BlockType::Empty));

                // Load variant_id (w1)
                f.instruction(&Instruction::LocalGet(subj_local));
                f.instruction(&Instruction::I32Load(memarg(4, 2)));
                f.instruction(&Instruction::I32Const(vid as i32));
                f.instruction(&Instruction::I32Eq);
                f.instruction(&Instruction::If(BlockType::Empty));

                // Bind destructured fields
                // The variant data is a record at w2
                if !bindings.is_empty() {
                    let data_local = ctx.alloc_local(ValType::I32);
                    f.instruction(&Instruction::LocalGet(subj_local));
                    f.instruction(&Instruction::I32Load(memarg(8, 2)));
                    f.instruction(&Instruction::LocalSet(data_local));

                    for (bi, binding) in bindings.iter().enumerate() {
                        let bind_local = ctx.alloc_local(ValType::I32);
                        // Access by index from the data record
                        f.instruction(&Instruction::LocalGet(data_local));
                        f.instruction(&Instruction::I32Const(bi as i32));
                        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST_GET)));
                        f.instruction(&Instruction::LocalSet(bind_local));
                        ctx.push_local(&binding.name, bind_local);
                    }
                }

                // Execute body
                match &arm.body {
                    MatchArmBody::Expr(expr) => {
                        emit_expr(expr, ctx, f)?;
                    }
                    MatchArmBody::Block(block) => {
                        emit_block_as_expr(block, ctx, f)?;
                    }
                }
                f.instruction(&Instruction::LocalSet(result_local));

                // Pop bindings
                for binding in bindings.iter().rev() {
                    ctx.pop_local(&binding.name);
                }

                f.instruction(&Instruction::Br(2)); // break outer block

                f.instruction(&Instruction::End); // end variant_id check
                f.instruction(&Instruction::End); // end tag check
            }
        }
    }

    f.instruction(&Instruction::End); // end outer block

    f.instruction(&Instruction::LocalGet(result_local));
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

/// Emit a block's statements and leave the last expression's value on the stack.
/// If the block has no statements, pushes nil.
pub fn emit_block_as_expr(
    block: &Block,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    if block.stmts.is_empty() {
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
        return Ok(());
    }

    // Emit all but the last statement normally
    let (last, rest) = block.stmts.split_last().unwrap();
    emit_stmts(rest, ctx, f)?;

    // The last statement: if it's an Expr statement, leave value on stack
    match last {
        Stmt::Expr(expr_stmt) => {
            emit_expr(&expr_stmt.expr, ctx, f)?;
        }
        _ => {
            // Emit the statement normally, then push nil as the block value
            emit_stmts(std::slice::from_ref(last), ctx, f)?;
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
        }
    }
    Ok(())
}

/// Create a `MemArg`.
fn memarg(offset: u64, align: u32) -> wasm_encoder::MemArg {
    wasm_encoder::MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

//! Statement code generation.
//!
//! Statements do not produce a value on the stack (they consume their results
//! or have side effects like `set`).

use pepl_types::ast::*;
use wasm_encoder::{BlockType, Function, Instruction, ValType};

use crate::compiler::FuncContext;
use crate::error::CodegenResult;
use crate::expr::emit_expr;
use crate::gas;
use crate::runtime::*;
use crate::types::*;

/// Emit a slice of statements.
pub fn emit_stmts(stmts: &[Stmt], ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    for stmt in stmts {
        emit_stmt(stmt, ctx, f)?;
    }
    Ok(())
}

/// Emit a single statement.
pub fn emit_stmt(stmt: &Stmt, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    match stmt {
        Stmt::Set(set) => emit_set(set, ctx, f),
        Stmt::Let(let_bind) => emit_let(let_bind, ctx, f),
        Stmt::If(if_expr) => emit_if_stmt(if_expr, ctx, f),
        Stmt::For(for_expr) => emit_for_stmt(for_expr, ctx, f),
        Stmt::Match(match_expr) => emit_match_stmt(match_expr, ctx, f),
        Stmt::Return(_) => emit_return(f),
        Stmt::Assert(assert_stmt) => emit_assert(assert_stmt, ctx, f),
        Stmt::Expr(expr_stmt) => emit_expr_stmt(&expr_stmt.expr, ctx, f),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Set statement
// ══════════════════════════════════════════════════════════════════════════════

fn emit_set(set: &SetStmt, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // `set field = value` updates the state record.
    // For nested paths `set a.b.c = x`, we must do immutable record update:
    //   state.a = { ...state.a, b: { ...state.a.b, c: x } }
    // For now we handle single-level and emit inline for nested.

    // Evaluate the new value
    let val_local = ctx.alloc_local(ValType::I32);
    emit_expr(&set.value, ctx, f)?;
    f.instruction(&Instruction::LocalSet(val_local));

    if set.target.len() == 1 {
        // Simple: update one field in the state record
        let field_name = &set.target[0].name;
        emit_state_field_set(field_name, val_local, ctx, f)?;
    } else {
        // Nested set: a.b.c = x
        // We need to build the chain from inside out.
        // For now, handle 2-level: set a.b = x
        // Read the outer record, create new record with updated field, store back.
        emit_nested_set(&set.target, val_local, ctx, f)?;
    }
    Ok(())
}

/// Set a single state field.
pub(crate) fn emit_state_field_set(
    field_name: &str,
    val_local: u32,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // We rebuild the state record with the updated field.
    // This is expensive but correct for the immutable-update model.
    // Strategy: iterate state fields, for each field:
    //   if name matches → use val_local
    //   else → copy from old state

    let state_fields = ctx.state_field_names.clone();
    let field_count = state_fields.len();
    let entries_local = ctx.alloc_local(ValType::I32);

    // Allocate entries array
    f.instruction(&Instruction::I32Const((field_count * 12) as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(entries_local));

    for (i, sf) in state_fields.iter().enumerate() {
        let (key_ptr, key_len) = ctx.intern_string(sf);
        let base = (i * 12) as u64;

        // key_offset
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::I32Const(key_ptr as i32));
        f.instruction(&Instruction::I32Store(memarg(base, 2)));
        // key_len
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::I32Const(key_len as i32));
        f.instruction(&Instruction::I32Store(memarg(base + 4, 2)));

        // value: either the new value or the old one
        if sf == field_name {
            f.instruction(&Instruction::LocalGet(entries_local));
            f.instruction(&Instruction::LocalGet(val_local));
            f.instruction(&Instruction::I32Store(memarg(base + 8, 2)));
        } else {
            // Read from old state
            let old_val = ctx.alloc_local(ValType::I32);
            f.instruction(&Instruction::GlobalGet(GLOBAL_STATE_PTR));
            f.instruction(&Instruction::I32Const(key_ptr as i32));
            f.instruction(&Instruction::I32Const(key_len as i32));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD_GET)));
            f.instruction(&Instruction::LocalSet(old_val));
            f.instruction(&Instruction::LocalGet(entries_local));
            f.instruction(&Instruction::LocalGet(old_val));
            f.instruction(&Instruction::I32Store(memarg(base + 8, 2)));
        }
    }

    // Build new state record
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(field_count as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
    f.instruction(&Instruction::GlobalSet(GLOBAL_STATE_PTR));

    Ok(())
}

/// Handle nested set: `set a.b.c = x` for arbitrary depth.
///
/// Algorithm (e.g. `set a.b.c = x`, target = [a, b, c]):
///   Phase 1 — Walk down:  old_a = state.a,  old_b = old_a.b
///   Phase 2 — Rebuild up: new_b = { ...old_b, c: x },  new_a = { ...old_a, b: new_b }
///   Phase 3 — Set root:   state.a = new_a
fn emit_nested_set(
    target: &[Ident],
    val_local: u32,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    let depth = target.len(); // e.g. [a, b, c] → 3

    // Phase 1: Walk down — read each intermediate record.
    // For depth=3 (a.b.c): intermediates[0] = state.a, intermediates[1] = old_a.b
    // We read depth-1 intermediates (don't read the last field — we're replacing it).
    let mut intermediates: Vec<u32> = Vec::with_capacity(depth - 1);

    for i in 0..depth - 1 {
        let (key_ptr, key_len) = ctx.intern_string(&target[i].name);
        let record_local = ctx.alloc_local(ValType::I32);

        if i == 0 {
            // First level reads from global state
            f.instruction(&Instruction::GlobalGet(GLOBAL_STATE_PTR));
        } else {
            // Subsequent levels read from the previous intermediate
            f.instruction(&Instruction::LocalGet(intermediates[i - 1]));
        }
        f.instruction(&Instruction::I32Const(key_ptr as i32));
        f.instruction(&Instruction::I32Const(key_len as i32));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD_GET)));
        f.instruction(&Instruction::LocalSet(record_local));

        intermediates.push(record_local);
    }

    // Phase 2: Rebuild from inside out.
    // Start with val_local, then wrap it in successive record rebuilds
    // working from the deepest level back up to the root.
    let mut current_val = val_local;

    for i in (0..depth - 1).rev() {
        let old_record = intermediates[i];
        let field_to_replace = &target[i + 1].name;

        current_val =
            emit_record_field_replace(old_record, field_to_replace, current_val, ctx, f)?;
    }

    // Phase 3: Set the root state field to the fully-rebuilt record.
    emit_state_field_set(&target[0].name, current_val, ctx, f)?;

    Ok(())
}

/// Clone a record with one field replaced, returning the local holding the new record.
///
/// Copies all entries from `old_record_local`, replacing the entry whose key matches
/// `field_name` with `new_val_local`. Returns the local index of the new record pointer.
fn emit_record_field_replace(
    old_record_local: u32,
    field_name: &str,
    new_val_local: u32,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<u32> {
    // Read old record's entries pointer and field count
    let old_entries_ptr = ctx.alloc_local(ValType::I32);
    let old_count = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::LocalGet(old_record_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // entries_ptr (w1)
    f.instruction(&Instruction::LocalSet(old_entries_ptr));
    f.instruction(&Instruction::LocalGet(old_record_local));
    f.instruction(&Instruction::I32Load(memarg(8, 2))); // field_count (w2)
    f.instruction(&Instruction::LocalSet(old_count));

    // Allocate new entries array: count * 12 bytes
    let new_entries = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::LocalGet(old_count));
    f.instruction(&Instruction::I32Const(12));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(new_entries));

    // Intern the target field key
    let (field_key_ptr, field_key_len) = ctx.intern_string(field_name);

    // Loop: copy each entry, replacing the target field's value
    let i_local = ctx.alloc_local(ValType::I32);
    let src_local = ctx.alloc_local(ValType::I32);
    let dst_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(i_local));

    f.instruction(&Instruction::Block(BlockType::Empty)); // break target
    f.instruction(&Instruction::Loop(BlockType::Empty));

    // if i >= count → break
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::LocalGet(old_count));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1));

    // src = old_entries + i * 12
    f.instruction(&Instruction::LocalGet(old_entries_ptr));
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(12));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(src_local));

    // dst = new_entries + i * 12
    f.instruction(&Instruction::LocalGet(new_entries));
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(12));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(dst_local));

    // Copy key_offset and key_len
    f.instruction(&Instruction::LocalGet(dst_local));
    f.instruction(&Instruction::LocalGet(src_local));
    f.instruction(&Instruction::I32Load(memarg(0, 2))); // src.key_offset
    f.instruction(&Instruction::I32Store(memarg(0, 2))); // dst.key_offset
    f.instruction(&Instruction::LocalGet(dst_local));
    f.instruction(&Instruction::LocalGet(src_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // src.key_len
    f.instruction(&Instruction::I32Store(memarg(4, 2))); // dst.key_len

    // Check if this entry's key matches field_name
    // Compare lengths first, then memcmp
    f.instruction(&Instruction::LocalGet(src_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // entry.key_len
    f.instruction(&Instruction::I32Const(field_key_len as i32));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    // Lengths match — memcmp key data
    f.instruction(&Instruction::LocalGet(src_local));
    f.instruction(&Instruction::I32Load(memarg(0, 2))); // entry.key_offset
    f.instruction(&Instruction::I32Const(field_key_ptr as i32)); // target key_ptr
    f.instruction(&Instruction::I32Const(field_key_len as i32)); // key_len
    f.instruction(&Instruction::Call(rt_func_idx(RT_MEMCMP)));
    f.instruction(&Instruction::If(BlockType::Empty));
    // Match → write new value
    f.instruction(&Instruction::LocalGet(dst_local));
    f.instruction(&Instruction::LocalGet(new_val_local));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    f.instruction(&Instruction::Else);
    // Lengths matched but content didn't → copy old value
    f.instruction(&Instruction::LocalGet(dst_local));
    f.instruction(&Instruction::LocalGet(src_local));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    f.instruction(&Instruction::End); // end memcmp check
    f.instruction(&Instruction::Else);
    // Lengths don't match → copy old value
    f.instruction(&Instruction::LocalGet(dst_local));
    f.instruction(&Instruction::LocalGet(src_local));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    f.instruction(&Instruction::End); // end length check

    // i += 1
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(i_local));
    f.instruction(&Instruction::Br(0)); // continue loop

    f.instruction(&Instruction::End); // end loop
    f.instruction(&Instruction::End); // end block

    // Build new record from entries
    let new_record = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::LocalGet(new_entries));
    f.instruction(&Instruction::LocalGet(old_count));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
    f.instruction(&Instruction::LocalSet(new_record));

    Ok(new_record)
}

// ══════════════════════════════════════════════════════════════════════════════
// Let binding
// ══════════════════════════════════════════════════════════════════════════════

fn emit_let(let_bind: &LetBinding, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    emit_expr(&let_bind.value, ctx, f)?;

    match &let_bind.name {
        Some(ident) => {
            let local = ctx.alloc_local(ValType::I32);
            f.instruction(&Instruction::LocalSet(local));
            ctx.push_local(&ident.name, local);
        }
        None => {
            // `let _ = expr` — discard
            f.instruction(&Instruction::Drop);
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// If / For / Match as statements (values are discarded)
// ══════════════════════════════════════════════════════════════════════════════

fn emit_if_stmt(if_expr: &IfExpr, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // Evaluate condition
    emit_expr(&if_expr.condition, ctx, f)?;
    f.instruction(&Instruction::I32Load(memarg(4, 2)));

    f.instruction(&Instruction::If(BlockType::Empty));
    emit_stmts(&if_expr.then_block.stmts, ctx, f)?;

    match &if_expr.else_branch {
        Some(ElseBranch::Block(block)) => {
            f.instruction(&Instruction::Else);
            emit_stmts(&block.stmts, ctx, f)?;
        }
        Some(ElseBranch::ElseIf(elif)) => {
            f.instruction(&Instruction::Else);
            emit_if_stmt(elif, ctx, f)?;
        }
        None => {}
    }

    f.instruction(&Instruction::End);
    Ok(())
}

fn emit_for_stmt(for_expr: &ForExpr, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // Same as emit_for_expr but discard the result
    let list_local = ctx.alloc_local(ValType::I32);
    let arr_local = ctx.alloc_local(ValType::I32);
    let count_local = ctx.alloc_local(ValType::I32);
    let i_local = ctx.alloc_local(ValType::I32);

    emit_expr(&for_expr.iterable, ctx, f)?;
    f.instruction(&Instruction::LocalSet(list_local));

    f.instruction(&Instruction::LocalGet(list_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalSet(arr_local));

    f.instruction(&Instruction::LocalGet(list_local));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(count_local));

    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(i_local));

    let item_local = ctx.alloc_local(ValType::I32);
    let index_local = if for_expr.index.is_some() {
        Some(ctx.alloc_local(ValType::I32))
    } else {
        None
    };

    ctx.push_local(&for_expr.item.name, item_local);
    if let (Some(idx_ident), Some(idx_local)) = (&for_expr.index, index_local) {
        ctx.push_local(&idx_ident.name, idx_local);
    }

    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));

    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::LocalGet(count_local));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1));

    f.instruction(&Instruction::LocalGet(arr_local));
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(4));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::LocalSet(item_local));

    if let Some(idx_local) = index_local {
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

    emit_stmts(&for_expr.body.stmts, ctx, f)?;

    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(i_local));
    f.instruction(&Instruction::Br(0));

    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    ctx.pop_local(&for_expr.item.name);
    if let Some(idx_ident) = &for_expr.index {
        ctx.pop_local(&idx_ident.name);
    }

    Ok(())
}

fn emit_match_stmt(
    match_expr: &MatchExpr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Emit as expr then drop the result
    crate::expr::emit_expr(
        &Expr::new(
            ExprKind::Match(Box::new(match_expr.clone())),
            match_expr.span,
        ),
        ctx,
        f,
    )?;
    f.instruction(&Instruction::Drop);
    Ok(())
}

fn emit_return(f: &mut Function) -> CodegenResult<()> {
    // In dispatch_action (returns i32), we need a value on the stack.
    // Push a nil value as the return value for early return.
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    f.instruction(&Instruction::Return);
    Ok(())
}

fn emit_assert(
    assert_stmt: &AssertStmt,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Evaluate condition
    emit_expr(&assert_stmt.condition, ctx, f)?;
    // Extract bool
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::If(BlockType::Empty));

    // Assertion failed → trap with message
    if let Some(msg) = &assert_stmt.message {
        let (ptr, len) = ctx.intern_string(msg);
        f.instruction(&Instruction::I32Const(ptr as i32));
        f.instruction(&Instruction::I32Const(len as i32));
    } else {
        f.instruction(&Instruction::I32Const(ctx.data.assert_failed_ptr as i32));
        f.instruction(&Instruction::I32Const(ctx.data.assert_failed_len as i32));
    }
    f.instruction(&Instruction::Call(IMPORT_TRAP));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
    Ok(())
}

fn emit_expr_stmt(expr: &Expr, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    emit_expr(expr, ctx, f)?;
    f.instruction(&Instruction::Drop);
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

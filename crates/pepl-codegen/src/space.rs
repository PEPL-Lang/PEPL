//! Space-level code generation.
//!
//! Generates the top-level exported functions:
//! - `init(gas_limit: i32)` — initialise state to defaults
//! - `dispatch_action(action_id: i32, args_ptr: i32) -> i32` — run an action
//! - `render(view_id: i32) -> i32` — render a view to Surface tree
//! - `get_state() -> i32` — return current state as a record value ptr
//! - Conditionally: `update(dt_ptr: i32)`, `handle_event(event_ptr: i32)`

use pepl_types::ast::*;
use wasm_encoder::{BlockType, Function, Instruction, ValType};

use crate::compiler::FuncContext;
use crate::error::CodegenResult;
use crate::expr::emit_expr;
use crate::gas;
use crate::runtime::*;
use crate::stmt::emit_stmts;
use crate::types::*;

// ══════════════════════════════════════════════════════════════════════════════
// init
// ══════════════════════════════════════════════════════════════════════════════

/// Emit the `init(gas_limit: i32)` function.
///
/// - Sets global gas_limit
/// - Resets gas counter to 0
/// - Evaluates each state field's default expression
/// - Builds the state record
/// - Evaluates derived fields
pub fn emit_init(
    state: &StateBlock,
    derived: Option<&DerivedBlock>,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // gas_limit = param 0
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::GlobalSet(GLOBAL_GAS_LIMIT));
    // gas = 0
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::GlobalSet(GLOBAL_GAS));

    // Build state record from defaults
    let field_count = state.fields.len();
    let entries_local = ctx.alloc_local(ValType::I32);

    f.instruction(&Instruction::I32Const((field_count * 12) as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(entries_local));

    for (i, field) in state.fields.iter().enumerate() {
        let (key_ptr, key_len) = ctx.intern_string(&field.name.name);
        let val_local = ctx.alloc_local(ValType::I32);
        let base = (i * 12) as u64;

        // Evaluate default
        emit_expr(&field.default, ctx, f)?;
        f.instruction(&Instruction::LocalSet(val_local));

        // key_offset
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::I32Const(key_ptr as i32));
        f.instruction(&Instruction::I32Store(memarg(base, 2)));
        // key_len
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::I32Const(key_len as i32));
        f.instruction(&Instruction::I32Store(memarg(base + 4, 2)));
        // value_ptr
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::LocalGet(val_local));
        f.instruction(&Instruction::I32Store(memarg(base + 8, 2)));
    }

    // state_ptr = record(entries, count)
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(field_count as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
    f.instruction(&Instruction::GlobalSet(GLOBAL_STATE_PTR));

    // Recompute derived fields (if any)
    if let Some(derived_block) = derived {
        emit_recompute_derived(derived_block, ctx, f)?;
    }

    f.instruction(&Instruction::End);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// dispatch_action
// ══════════════════════════════════════════════════════════════════════════════

/// Emit the `dispatch_action(action_id: i32, args_ptr: i32) -> i32` function.
///
/// Dispatches to the appropriate action handler based on action_id.
/// Checks invariants after execution and rolls back on failure.
pub fn emit_dispatch_action(
    actions: &[ActionDecl],
    invariants: &[InvariantDecl],
    derived: Option<&DerivedBlock>,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // Save state snapshot for rollback
    let snapshot_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::GlobalGet(GLOBAL_STATE_PTR));
    f.instruction(&Instruction::LocalSet(snapshot_local));

    // Reset gas counter for this dispatch
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::GlobalSet(GLOBAL_GAS));

    // Switch on action_id (param 0)
    // We emit a chain of if/else: if action_id == 0 { ... } else if == 1 { ... } ...
    let result_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    f.instruction(&Instruction::LocalSet(result_local));

    f.instruction(&Instruction::Block(BlockType::Empty)); // outer break target

    for (i, action) in actions.iter().enumerate() {
        f.instruction(&Instruction::LocalGet(0)); // action_id (param 0)
        f.instruction(&Instruction::I32Const(i as i32));
        f.instruction(&Instruction::I32Eq);
        f.instruction(&Instruction::If(BlockType::Empty));

        // Bind action parameters from args_ptr (param 1)
        for (pi, param) in action.params.iter().enumerate() {
            let param_local = ctx.alloc_local(ValType::I32);
            // args is a list value — get element by index
            f.instruction(&Instruction::LocalGet(1)); // args_ptr
            f.instruction(&Instruction::I32Const(pi as i32));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST_GET)));
            f.instruction(&Instruction::LocalSet(param_local));
            ctx.push_local(&param.name.name, param_local);
        }

        // Execute action body
        emit_stmts(&action.body.stmts, ctx, f)?;

        // Pop param bindings
        for param in action.params.iter().rev() {
            ctx.pop_local(&param.name.name);
        }

        f.instruction(&Instruction::Br(0)); // break to outer
        f.instruction(&Instruction::End); // end if
    }

    f.instruction(&Instruction::End); // end outer block

    // Recompute derived fields
    if let Some(derived_block) = derived {
        emit_recompute_derived(derived_block, ctx, f)?;
    }

    // Check invariants — if any fail, rollback
    for inv in invariants {
        emit_expr(&inv.condition, ctx, f)?;
        f.instruction(&Instruction::I32Load(memarg(4, 2)));
        f.instruction(&Instruction::I32Eqz);
        f.instruction(&Instruction::If(BlockType::Empty));
        // Rollback: restore snapshot
        f.instruction(&Instruction::LocalGet(snapshot_local));
        f.instruction(&Instruction::GlobalSet(GLOBAL_STATE_PTR));
        // Trap with invariant name
        let (ptr, len) = ctx.intern_string(&format!("invariant violated: {}", inv.name.name));
        f.instruction(&Instruction::I32Const(ptr as i32));
        f.instruction(&Instruction::I32Const(len as i32));
        f.instruction(&Instruction::Call(IMPORT_TRAP));
        f.instruction(&Instruction::Unreachable);
        f.instruction(&Instruction::End);
    }

    // Return result (nil for actions — they mutate state)
    f.instruction(&Instruction::LocalGet(result_local));
    f.instruction(&Instruction::End);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// render
// ══════════════════════════════════════════════════════════════════════════════

/// Emit the `render(view_id: i32) -> i32` function.
///
/// Evaluates the specified view and returns a serialized Surface tree
/// as a record value.
pub fn emit_render(
    views: &[ViewDecl],
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    let result_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    f.instruction(&Instruction::LocalSet(result_local));

    f.instruction(&Instruction::Block(BlockType::Empty));

    for (i, view) in views.iter().enumerate() {
        f.instruction(&Instruction::LocalGet(0)); // view_id
        f.instruction(&Instruction::I32Const(i as i32));
        f.instruction(&Instruction::I32Eq);
        f.instruction(&Instruction::If(BlockType::Empty));

        // Bind view params (if any — usually views are parameterless in main render)
        for param in view.params.iter() {
            let param_local = ctx.alloc_local(ValType::I32);
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
            f.instruction(&Instruction::LocalSet(param_local));
            ctx.push_local(&param.name.name, param_local);
        }

        // Emit UI block → Surface node tree
        emit_ui_block(&view.body, ctx, f)?;
        f.instruction(&Instruction::LocalSet(result_local));

        // Pop param bindings
        for param in view.params.iter().rev() {
            ctx.pop_local(&param.name.name);
        }

        f.instruction(&Instruction::Br(0));
        f.instruction(&Instruction::End);
    }

    f.instruction(&Instruction::End);

    f.instruction(&Instruction::LocalGet(result_local));
    f.instruction(&Instruction::End);
    Ok(())
}

/// Emit a UI block → record value representing the Surface tree.
///
/// Each component becomes a record: `{ component: "Name", props: {...}, children: [...] }`
fn emit_ui_block(block: &UIBlock, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // Build a list of surface nodes
    let count = block.elements.len();
    if count == 0 {
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
        return Ok(());
    }

    let arr_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const((count * 4) as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(arr_local));

    for (i, elem) in block.elements.iter().enumerate() {
        let node_local = ctx.alloc_local(ValType::I32);
        emit_ui_element(elem, ctx, f)?;
        f.instruction(&Instruction::LocalSet(node_local));
        f.instruction(&Instruction::LocalGet(arr_local));
        f.instruction(&Instruction::LocalGet(node_local));
        f.instruction(&Instruction::I32Store(memarg(i as u64 * 4, 2)));
    }

    f.instruction(&Instruction::LocalGet(arr_local));
    f.instruction(&Instruction::I32Const(count as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    Ok(())
}

/// Emit a single UI element → surface node record.
fn emit_ui_element(elem: &UIElement, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    match elem {
        UIElement::Component(comp) => emit_component_expr(comp, ctx, f),
        UIElement::Let(let_bind) => {
            crate::stmt::emit_stmt(&Stmt::Let(let_bind.clone()), ctx, f)?;
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
            Ok(())
        }
        UIElement::If(ui_if) => emit_ui_if(ui_if, ctx, f),
        UIElement::For(ui_for) => emit_ui_for(ui_for, ctx, f),
    }
}

/// Emit a component expression → surface record.
///
/// Produces: `{ component: "Name", props: { ... }, children: [...] }`
fn emit_component_expr(
    comp: &ComponentExpr,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    // We build a 3-field record: component, props, children
    let entries_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const(3 * 12)); // 3 fields × 12 bytes
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(entries_local));

    // Field 0: "component" = "CompName"
    let (ck_ptr, ck_len) = ctx.intern_string("component");
    let (cn_ptr, cn_len) = ctx.intern_string(&comp.name.name);
    let comp_val = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const(cn_ptr as i32));
    f.instruction(&Instruction::I32Const(cn_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));
    f.instruction(&Instruction::LocalSet(comp_val));

    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(ck_ptr as i32));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(ck_len as i32));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::LocalGet(comp_val));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));

    // Field 1: "props" = record of prop assignments
    let (pk_ptr, pk_len) = ctx.intern_string("props");
    let props_val = ctx.alloc_local(ValType::I32);
    emit_props(&comp.props, ctx, f)?;
    f.instruction(&Instruction::LocalSet(props_val));

    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(pk_ptr as i32));
    f.instruction(&Instruction::I32Store(memarg(12, 2)));
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(pk_len as i32));
    f.instruction(&Instruction::I32Store(memarg(16, 2)));
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::LocalGet(props_val));
    f.instruction(&Instruction::I32Store(memarg(20, 2)));

    // Field 2: "children" = list of child nodes
    let (chk_ptr, chk_len) = ctx.intern_string("children");
    let children_val = ctx.alloc_local(ValType::I32);
    if let Some(children_block) = &comp.children {
        emit_ui_block(children_block, ctx, f)?;
    } else {
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    }
    f.instruction(&Instruction::LocalSet(children_val));

    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(chk_ptr as i32));
    f.instruction(&Instruction::I32Store(memarg(24, 2)));
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(chk_len as i32));
    f.instruction(&Instruction::I32Store(memarg(28, 2)));
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::LocalGet(children_val));
    f.instruction(&Instruction::I32Store(memarg(32, 2)));

    // Build the surface record
    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(3));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
    Ok(())
}

/// Emit props as a record value.
fn emit_props(props: &[PropAssign], ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    if props.is_empty() {
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::I32Const(0));
        f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
        return Ok(());
    }

    let entries_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::I32Const((props.len() * 12) as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(entries_local));

    for (i, prop) in props.iter().enumerate() {
        let (key_ptr, key_len) = ctx.intern_string(&prop.name.name);
        let val_local = ctx.alloc_local(ValType::I32);
        emit_expr(&prop.value, ctx, f)?;
        f.instruction(&Instruction::LocalSet(val_local));

        let base = (i * 12) as u64;
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::I32Const(key_ptr as i32));
        f.instruction(&Instruction::I32Store(memarg(base, 2)));
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::I32Const(key_len as i32));
        f.instruction(&Instruction::I32Store(memarg(base + 4, 2)));
        f.instruction(&Instruction::LocalGet(entries_local));
        f.instruction(&Instruction::LocalGet(val_local));
        f.instruction(&Instruction::I32Store(memarg(base + 8, 2)));
    }

    f.instruction(&Instruction::LocalGet(entries_local));
    f.instruction(&Instruction::I32Const(props.len() as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_RECORD)));
    Ok(())
}

fn emit_ui_if(ui_if: &UIIf, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    emit_expr(&ui_if.condition, ctx, f)?;
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    emit_ui_block(&ui_if.then_block, ctx, f)?;
    f.instruction(&Instruction::Else);
    match &ui_if.else_block {
        Some(UIElse::Block(block)) => emit_ui_block(block, ctx, f)?,
        Some(UIElse::ElseIf(elif)) => emit_ui_if(elif, ctx, f)?,
        None => {
            f.instruction(&Instruction::I32Const(0));
            f.instruction(&Instruction::I32Const(0));
            f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
        }
    }
    f.instruction(&Instruction::End);
    Ok(())
}

fn emit_ui_for(ui_for: &UIFor, ctx: &mut FuncContext, f: &mut Function) -> CodegenResult<()> {
    // For loops in UI produce a list of surface nodes
    let list_local = ctx.alloc_local(ValType::I32);
    let arr_local = ctx.alloc_local(ValType::I32);
    let count_local = ctx.alloc_local(ValType::I32);
    let i_local = ctx.alloc_local(ValType::I32);
    let result_arr = ctx.alloc_local(ValType::I32);

    emit_expr(&ui_for.iterable, ctx, f)?;
    f.instruction(&Instruction::LocalSet(list_local));

    f.instruction(&Instruction::LocalGet(list_local));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalSet(arr_local));

    f.instruction(&Instruction::LocalGet(list_local));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(count_local));

    // Allocate result array
    f.instruction(&Instruction::LocalGet(count_local));
    f.instruction(&Instruction::I32Const(4));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(result_arr));

    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(i_local));

    let item_local = ctx.alloc_local(ValType::I32);
    let index_local = if ui_for.index.is_some() {
        Some(ctx.alloc_local(ValType::I32))
    } else {
        None
    };

    ctx.push_local(&ui_for.item.name, item_local);
    if let (Some(idx_ident), Some(idx_local)) = (&ui_for.index, index_local) {
        ctx.push_local(&idx_ident.name, idx_local);
    }

    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));

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

    // Emit body → list of nodes
    let body_result = ctx.alloc_local(ValType::I32);
    emit_ui_block(&ui_for.body, ctx, f)?;
    f.instruction(&Instruction::LocalSet(body_result));

    // Store body_result at result_arr[i]
    f.instruction(&Instruction::LocalGet(result_arr));
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(4));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalGet(body_result));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));

    // i += 1
    f.instruction(&Instruction::LocalGet(i_local));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(i_local));
    f.instruction(&Instruction::Br(0));

    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    ctx.pop_local(&ui_for.item.name);
    if let Some(idx_ident) = &ui_for.index {
        ctx.pop_local(&idx_ident.name);
    }

    f.instruction(&Instruction::LocalGet(result_arr));
    f.instruction(&Instruction::LocalGet(count_local));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_LIST)));
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// get_state
// ══════════════════════════════════════════════════════════════════════════════

/// Emit the `get_state() -> i32` function — returns the current state record.
pub fn emit_get_state(f: &mut Function) {
    f.instruction(&Instruction::GlobalGet(GLOBAL_STATE_PTR));
    f.instruction(&Instruction::End);
}

// ══════════════════════════════════════════════════════════════════════════════
// update / handle_event
// ══════════════════════════════════════════════════════════════════════════════

/// Emit `update(dt_ptr: i32)` — param is a NUMBER value pointer.
pub fn emit_update(
    update_decl: &UpdateDecl,
    derived: Option<&DerivedBlock>,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    // Bind dt param
    let dt_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::LocalSet(dt_local));
    ctx.push_local(&update_decl.param.name.name, dt_local);

    emit_stmts(&update_decl.body.stmts, ctx, f)?;

    ctx.pop_local(&update_decl.param.name.name);

    // Recompute derived
    if let Some(derived_block) = derived {
        emit_recompute_derived(derived_block, ctx, f)?;
    }

    f.instruction(&Instruction::End);
    Ok(())
}

/// Emit `handle_event(event_ptr: i32)` — param is a record value pointer.
pub fn emit_handle_event(
    handle_event_decl: &HandleEventDecl,
    derived: Option<&DerivedBlock>,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    gas::emit_gas_tick(f, ctx.data.gas_exhausted_ptr, ctx.data.gas_exhausted_len);

    let event_local = ctx.alloc_local(ValType::I32);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::LocalSet(event_local));
    ctx.push_local(&handle_event_decl.param.name.name, event_local);

    emit_stmts(&handle_event_decl.body.stmts, ctx, f)?;

    ctx.pop_local(&handle_event_decl.param.name.name);

    if let Some(derived_block) = derived {
        emit_recompute_derived(derived_block, ctx, f)?;
    }

    f.instruction(&Instruction::End);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Derived field recomputation
// ══════════════════════════════════════════════════════════════════════════════

/// Recompute all derived fields and update the state record.
fn emit_recompute_derived(
    derived: &DerivedBlock,
    ctx: &mut FuncContext,
    f: &mut Function,
) -> CodegenResult<()> {
    for field in &derived.fields {
        let val_local = ctx.alloc_local(ValType::I32);
        emit_expr(&field.value, ctx, f)?;
        f.instruction(&Instruction::LocalSet(val_local));

        // Update state record with the computed derived value
        let field_name = field.name.name.clone();
        crate::stmt::emit_state_field_set(&field_name, val_local, ctx, f)?;
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Helper
// ══════════════════════════════════════════════════════════════════════════════

fn memarg(offset: u64, align: u32) -> wasm_encoder::MemArg {
    wasm_encoder::MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

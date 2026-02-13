//! Runtime helper functions emitted into the WASM module.
//!
//! These provide the value-manipulation primitives that expression and
//! statement codegen builds upon.  Every function is registered during module
//! assembly (in `compiler.rs`) and referenced by its function index.

use wasm_encoder::{BlockType, Function, Instruction, ValType};

use crate::types::*;

// ══════════════════════════════════════════════════════════════════════════════
// Runtime function index offsets (relative to IMPORT_COUNT)
// ══════════════════════════════════════════════════════════════════════════════

/// Bump-allocate `size` bytes; returns pointer.
///
/// `alloc(size: i32) -> i32`
pub const RT_ALLOC: u32 = 0;

/// Create a NIL value on the heap; returns pointer.
///
/// `val_nil() -> i32`
pub const RT_VAL_NIL: u32 = 1;

/// Create a NUMBER value from an f64; returns pointer.
///
/// `val_number(n: f64) -> i32`  — note: declared as (i32, i32) low/high
/// Actually we pass the f64 via two i32 halves and reinterpret.
/// Simpler approach: pass as i64 bits, but WASM MVP has i64.
/// Simplest: store the f64 directly using f64.store.
/// We'll use a helper that takes no args on the WASM stack — the caller
/// pushes the f64 onto memory via `f64.store` beforehand.
///
/// Actually, let's keep it simple: `val_number()` allocates a cell,
/// and the *caller* writes tag + f64 into that cell.
/// We provide a helper: `make_number(bits_hi: i32, bits_lo: i32) -> i32`
/// Or even simpler: emit inline alloc + store in expr.rs.
///
/// For the runtime we only expose `alloc` and the constructor helpers.
pub const RT_VAL_NUMBER: u32 = 2;

/// Create a BOOL value; `val_bool(b: i32) -> i32`
pub const RT_VAL_BOOL: u32 = 3;

/// Create a STRING value; `val_string(data_ptr: i32, len: i32) -> i32`
pub const RT_VAL_STRING: u32 = 4;

/// Create a LIST value; `val_list(arr_ptr: i32, count: i32) -> i32`
pub const RT_VAL_LIST: u32 = 5;

/// Create a RECORD value; `val_record(entries_ptr: i32, count: i32) -> i32`
pub const RT_VAL_RECORD: u32 = 6;

/// Create a VARIANT value; `val_variant(id: i32, data_ptr: i32) -> i32`
pub const RT_VAL_VARIANT: u32 = 7;

/// Create an ACTION_REF value; `val_action_ref(action_id: i32) -> i32`
pub const RT_VAL_ACTION_REF: u32 = 8;

/// Read the tag of a value; `val_tag(ptr: i32) -> i32`
pub const RT_VAL_TAG: u32 = 9;

/// Read the f64 payload from a NUMBER value; `val_get_number(ptr: i32) -> f64`
/// We return the raw bits as (i32, i32) or just store to a scratch global.
/// Actually WASM does support f64 on the value stack, so we can return f64.
/// But our function type table only has i32 returns. We'll add a special type.
pub const RT_VAL_GET_NUMBER: u32 = 10;

/// Read the i32 payload word-1; `val_get_w1(ptr: i32) -> i32`
pub const RT_VAL_GET_W1: u32 = 11;

/// Read the i32 payload word-2; `val_get_w2(ptr: i32) -> i32`
pub const RT_VAL_GET_W2: u32 = 12;

/// Compare two values for structural equality; `val_eq(a: i32, b: i32) -> i32`
pub const RT_VAL_EQ: u32 = 13;

/// Convert a value to its string representation for interpolation.
/// `val_to_string(ptr: i32) -> i32` (returns a STRING value ptr)
pub const RT_VAL_TO_STRING: u32 = 14;

/// Concatenate two STRING values; `val_string_concat(a: i32, b: i32) -> i32`
pub const RT_VAL_STRING_CONCAT: u32 = 15;

/// Arithmetic: add two numbers; `val_add(a: i32, b: i32) -> i32`  
pub const RT_VAL_ADD: u32 = 16;
/// `val_sub(a: i32, b: i32) -> i32`
pub const RT_VAL_SUB: u32 = 17;
/// `val_mul(a: i32, b: i32) -> i32`
pub const RT_VAL_MUL: u32 = 18;
/// `val_div(a: i32, b: i32) -> i32` — traps on /0 and NaN
pub const RT_VAL_DIV: u32 = 19;
/// `val_mod(a: i32, b: i32) -> i32`
pub const RT_VAL_MOD: u32 = 20;
/// `val_neg(a: i32) -> i32` — unary negate
pub const RT_VAL_NEG: u32 = 21;
/// `val_not(a: i32) -> i32` — logical not (bool)
pub const RT_VAL_NOT: u32 = 22;

/// Comparisons returning BOOL value ptr:
/// `val_lt(a, b) -> i32`, `val_le`, `val_gt`, `val_ge`
pub const RT_VAL_LT: u32 = 23;
pub const RT_VAL_LE: u32 = 24;
pub const RT_VAL_GT: u32 = 25;
pub const RT_VAL_GE: u32 = 26;

/// Record field access: `val_record_get(rec_ptr: i32, key_ptr: i32, key_len: i32) -> i32`
pub const RT_VAL_RECORD_GET: u32 = 27;

/// List index access: `val_list_get(list_ptr: i32, index: i32) -> i32`
pub const RT_VAL_LIST_GET: u32 = 28;

/// NaN check + trap: `check_nan(val_ptr: i32) -> i32` — traps if NaN, returns val_ptr
pub const RT_CHECK_NAN: u32 = 29;

/// Byte-by-byte memory comparison: `memcmp(ptr_a: i32, ptr_b: i32, len: i32) -> i32`
/// Returns 1 if all bytes match, 0 otherwise.
pub const RT_MEMCMP: u32 = 30;

/// Total number of runtime helper functions.
pub const RT_FUNC_COUNT: u32 = 31;

// ── Absolute function indices ────────────────────────────────────────────────

/// Compute the absolute WASM function index of a runtime helper.
#[inline]
pub const fn rt_func_idx(rt_offset: u32) -> u32 {
    IMPORT_COUNT + rt_offset
}

// ══════════════════════════════════════════════════════════════════════════════
// Emit helpers — each builds a `wasm_encoder::Function`
// ══════════════════════════════════════════════════════════════════════════════

/// Emit the `alloc(size: i32) -> i32` function.
///
/// Bump allocator: returns the current `heap_ptr`, then advances it by `size`.
/// If we exceed memory, call `memory.grow`.
pub fn emit_alloc() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]); // local 1: old_ptr
    // old_ptr = heap_ptr
    f.instruction(&Instruction::GlobalGet(GLOBAL_HEAP_PTR));
    f.instruction(&Instruction::LocalSet(1));
    // heap_ptr += size
    f.instruction(&Instruction::GlobalGet(GLOBAL_HEAP_PTR));
    f.instruction(&Instruction::LocalGet(0)); // param: size
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::GlobalSet(GLOBAL_HEAP_PTR));
    // TODO: memory.grow if needed
    // return old_ptr
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_nil() -> i32`.
pub fn emit_val_nil() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]); // local: ptr
    // ptr = alloc(VALUE_SIZE)
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(0));
    // store tag = TAG_NIL
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Const(TAG_NIL));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    // return ptr
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_number(n_bits_lo: i32, n_bits_hi: i32) -> i32`.
///
/// We pass the f64 as two i32 halves because our type table uses i32-only
/// signatures.  The function reassembles and stores the f64.
pub fn emit_val_number() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]); // local: ptr
    // ptr = alloc(VALUE_SIZE)
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    // store tag = TAG_NUMBER
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    // store lo word at offset+4
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(0)); // bits_lo
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    // store hi word at offset+8
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(1)); // bits_hi
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    // return ptr
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_bool(b: i32) -> i32`.
pub fn emit_val_bool() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]); // local: ptr
    // ptr = alloc(VALUE_SIZE)
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(1));
    // tag = TAG_BOOL
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_BOOL));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    // w1 = b
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    // return ptr
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_string(data_ptr: i32, len: i32) -> i32`.
pub fn emit_val_string() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]); // local: ptr
    // ptr = alloc(VALUE_SIZE)
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    // tag
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_STRING));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    // w1 = data_ptr
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    // w2 = len
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    // return ptr
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_list(arr_ptr: i32, count: i32) -> i32`.
pub fn emit_val_list() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]);
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_LIST));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_record(entries_ptr: i32, count: i32) -> i32`.
pub fn emit_val_record() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]);
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_RECORD));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_variant(id: i32, data_ptr: i32) -> i32`.
pub fn emit_val_variant() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]);
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_VARIANT));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Store(memarg(8, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_action_ref(action_id: i32) -> i32`.
pub fn emit_val_action_ref() -> Function {
    let mut f = Function::new(vec![(1, ValType::I32)]);
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(1));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_ACTION_REF));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Store(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_tag(ptr: i32) -> i32` — reads the tag word.
pub fn emit_val_tag() -> Function {
    let mut f = Function::new(vec![]);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_get_number(ptr: i32) -> i32`.
///
/// Returns the low 32 bits of the f64 payload. The high 32 bits are obtained
/// via `val_get_w2`. Together they reconstruct the f64 via `f64.reinterpret_i64`.
///
/// (This is the same as `val_get_w1`, but named separately for clarity.)
pub fn emit_val_get_number() -> Function {
    // For simplicity, returns w1 (same as emit_val_get_w1)
    let mut f = Function::new(vec![]);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_get_w1(ptr: i32) -> i32`.
pub fn emit_val_get_w1() -> Function {
    let mut f = Function::new(vec![]);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_get_w2(ptr: i32) -> i32`.
pub fn emit_val_get_w2() -> Function {
    let mut f = Function::new(vec![]);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_eq(a: i32, b: i32) -> i32` — structural equality, returns bool value ptr.
pub fn emit_val_eq() -> Function {
    // Simplified: compare tags, then compare payloads.
    // For NUMBER: compare the 8 bytes. For STRING: byte-compare data.
    // Full structural equality for records/lists would be recursive — we emit
    // a simplified version that handles primitives and falls back to ptr equality.
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 2: tag_a
        (1, ValType::I32), // local 3: tag_b
        (1, ValType::I32), // local 4: result (0 or 1)
    ]);

    // tag_a = load tag(a)
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::LocalSet(2));
    // tag_b = load tag(b)
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::LocalSet(3));

    // if tags differ → false
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::I32Ne);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(4));
    f.instruction(&Instruction::Else);

    // tags match — compare by tag type
    // NIL == NIL always true
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_NIL));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::LocalSet(4));
    f.instruction(&Instruction::Else);

    // NUMBER: compare both 32-bit words of the f64
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    // w1 eq
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::I32Eq);
    // w2 eq
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::LocalSet(4));
    f.instruction(&Instruction::Else);

    // BOOL: compare w1
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_BOOL));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::LocalSet(4));
    f.instruction(&Instruction::Else);

    // STRING: compare lengths first, then byte-by-byte memcmp
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_STRING));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    // Compare lengths (w2 = byte-length)
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    // Same length — byte-by-byte comparison via RT_MEMCMP
    // memcmp(a.data_ptr, b.data_ptr, a.len) → 0 or 1
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // a.data_ptr
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // b.data_ptr
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2))); // a.len
    f.instruction(&Instruction::Call(rt_func_idx(RT_MEMCMP)));
    f.instruction(&Instruction::LocalSet(4));
    f.instruction(&Instruction::Else);
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(4));
    f.instruction(&Instruction::End); // end length check if
    f.instruction(&Instruction::Else);

    // Default: compare all payload bytes (fallback for list, record, variant, etc.)
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::I32And);
    f.instruction(&Instruction::LocalSet(4));

    f.instruction(&Instruction::End); // string else
    f.instruction(&Instruction::End); // bool else
    f.instruction(&Instruction::End); // number else
    f.instruction(&Instruction::End); // nil else

    f.instruction(&Instruction::End); // tags differ else

    // Create bool value from result
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_BOOL)));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_to_string(ptr: i32) -> i32` — converts any value to a STRING value.
///
/// v1: strings pass through, bools → "true"/"false", nil → "nil",
/// numbers (integer-valued) → decimal string, others → "[value]".
pub fn emit_val_to_string(data: &DataSegmentTracker) -> Function {
    // Locals: 0=ptr, 1=tag, 2=f64_val, 3=is_neg, 4=abs_val(i64), 5=buf_ptr,
    //         6=write_pos, 7=digit_count, 8=start, 9=result
    let mut f = Function::new(vec![
        (1, ValType::I32),  // local 1: tag
        (1, ValType::F64),  // local 2: f64_val
        (1, ValType::I32),  // local 3: is_neg
        (1, ValType::I64),  // local 4: abs_val
        (1, ValType::I32),  // local 5: buf_ptr
        (1, ValType::I32),  // local 6: write_pos
        (1, ValType::I32),  // local 7: digit_count
        (1, ValType::I32),  // local 8: start_pos
        (1, ValType::I32),  // local 9: result_ptr
    ]);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::LocalSet(1));

    // If already a string, return as-is
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_STRING));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::Else);

    // NUMBER → integer string conversion
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));

    // Load the f64 value (stored as two i32 words at offset 4)
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalSet(2));

    // Check: is it a finite integer? (floor == value && not NaN/Inf)
    // f64.floor(val) == val
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::F64Floor);
    f.instruction(&Instruction::F64Eq);
    // Also: val == val (not NaN)
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::F64Eq);
    f.instruction(&Instruction::I32And);
    // Also: abs(val) < 2^53 (safe integer range)
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::F64Abs);
    f.instruction(&Instruction::F64Const(9007199254740992.0)); // 2^53
    f.instruction(&Instruction::F64Lt);
    f.instruction(&Instruction::I32And);

    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));

    // Integer path: convert f64 → digits string
    // Check if negative
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::F64Const(0.0));
    f.instruction(&Instruction::F64Lt);
    f.instruction(&Instruction::LocalSet(3)); // is_neg

    // abs_val = i64.trunc_f64_s(abs(val))
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::F64Abs);
    f.instruction(&Instruction::I64TruncF64S);
    f.instruction(&Instruction::LocalSet(4)); // abs_val

    // Allocate a 20-byte scratch buffer for digits (max i64 decimal = 19 chars + sign)
    f.instruction(&Instruction::I32Const(20));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(5)); // buf_ptr

    // write_pos starts at end of buffer (we write digits right-to-left)
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::I32Const(20));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(6)); // write_pos

    // Handle zero specially
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::I64Eqz);
    f.instruction(&Instruction::If(BlockType::Empty));
    // write '0'
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::LocalSet(6));
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Const(48)); // ASCII '0'
    f.instruction(&Instruction::I32Store8(memarg(0, 0)));
    f.instruction(&Instruction::Else);

    // Digit extraction loop: while abs_val > 0
    f.instruction(&Instruction::Block(BlockType::Empty));
    f.instruction(&Instruction::Loop(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::I64Eqz);
    f.instruction(&Instruction::BrIf(1)); // break if zero

    // write_pos -= 1
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::LocalSet(6));

    // digit = abs_val % 10
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::I64Const(10));
    f.instruction(&Instruction::I64RemU);
    f.instruction(&Instruction::I32WrapI64);
    f.instruction(&Instruction::I32Const(48)); // ASCII '0'
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Store8(memarg(0, 0)));

    // abs_val /= 10
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::I64Const(10));
    f.instruction(&Instruction::I64DivU);
    f.instruction(&Instruction::LocalSet(4));

    f.instruction(&Instruction::Br(0)); // continue loop
    f.instruction(&Instruction::End); // end loop
    f.instruction(&Instruction::End); // end block

    f.instruction(&Instruction::End); // end zero check

    // If negative, prepend '-'
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Sub);
    f.instruction(&Instruction::LocalSet(6));
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Const(45)); // ASCII '-'
    f.instruction(&Instruction::I32Store8(memarg(0, 0)));
    f.instruction(&Instruction::End);

    // Create STRING value from write_pos..buf_ptr+20
    // data_ptr = write_pos, len = (buf_ptr + 20) - write_pos
    f.instruction(&Instruction::LocalGet(6));  // data_ptr
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::I32Const(20));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Sub);       // len
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));

    f.instruction(&Instruction::Else);
    // Non-integer number → "[value]" placeholder
    f.instruction(&Instruction::I32Const(data.value_ptr as i32));
    f.instruction(&Instruction::I32Const(data.value_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));
    f.instruction(&Instruction::End); // end integer check

    f.instruction(&Instruction::Else);

    // BOOL
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_BOOL));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    // "true"
    f.instruction(&Instruction::I32Const(data.true_ptr as i32));
    f.instruction(&Instruction::I32Const(data.true_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));
    f.instruction(&Instruction::Else);
    // "false"
    f.instruction(&Instruction::I32Const(data.false_ptr as i32));
    f.instruction(&Instruction::I32Const(data.false_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::Else);

    // NIL → "nil"
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_NIL));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    f.instruction(&Instruction::I32Const(data.nil_ptr as i32));
    f.instruction(&Instruction::I32Const(data.nil_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));
    f.instruction(&Instruction::Else);

    // Default: return "[value]" placeholder
    f.instruction(&Instruction::I32Const(data.value_ptr as i32));
    f.instruction(&Instruction::I32Const(data.value_len as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));

    f.instruction(&Instruction::End); // nil else
    f.instruction(&Instruction::End); // bool else
    f.instruction(&Instruction::End); // number else
    f.instruction(&Instruction::End); // string if/else

    f.instruction(&Instruction::End);
    f
}

/// Emit `val_string_concat(a: i32, b: i32) -> i32`.
///
/// Allocates a new string buffer, copies both payloads, returns new STRING value.
pub fn emit_val_string_concat() -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 2: len_a
        (1, ValType::I32), // local 3: len_b
        (1, ValType::I32), // local 4: new_buf
        (1, ValType::I32), // local 5: total_len
    ]);

    // len_a = a.w2
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(2));
    // len_b = b.w2
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(3));
    // total_len = len_a + len_b
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(5));

    // new_buf = alloc(total_len)
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(4));

    // memory.copy(new_buf, a.w1, len_a)
    f.instruction(&Instruction::LocalGet(4));        // dst
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // src = a.w1
    f.instruction(&Instruction::LocalGet(2));        // len_a
    f.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });

    // memory.copy(new_buf + len_a, b.w1, len_b)
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Add);             // dst = new_buf + len_a
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // src = b.w1
    f.instruction(&Instruction::LocalGet(3));        // len_b
    f.instruction(&Instruction::MemoryCopy { src_mem: 0, dst_mem: 0 });

    // return val_string(new_buf, total_len)
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_STRING)));

    f.instruction(&Instruction::End);
    f
}

/// Emit an arithmetic binary op helper.
///
/// All arithmetic helpers follow the same pattern:
/// 1. Load f64 from a (w1,w2 → i64.or → f64.reinterpret)
/// 2. Load f64 from b
/// 3. Perform f64 op
/// 4. Store result as new NUMBER value
fn emit_arith_binop(op: Instruction<'static>) -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 2: result ptr
    ]);

    // Allocate result value cell
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));

    // Store tag = TAG_NUMBER
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));

    // Load f64 from a: f64.load at a+4
    // Store result f64 at result+4
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&op);
    f.instruction(&Instruction::F64Store(memarg(4, 3)));

    // return result
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

pub fn emit_val_add() -> Function {
    emit_arith_binop(Instruction::F64Add)
}
pub fn emit_val_sub() -> Function {
    emit_arith_binop(Instruction::F64Sub)
}
pub fn emit_val_mul() -> Function {
    emit_arith_binop(Instruction::F64Mul)
}

/// Emit `val_div` — with division-by-zero and NaN trap guards.
pub fn emit_val_div(trap_msg_ptr: u32, trap_msg_len: u32) -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32),  // local 2: result ptr
        (1, ValType::F64),  // local 3: divisor
        (1, ValType::F64),  // local 4: quotient
    ]);

    // Load divisor
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalSet(3));

    // Check divisor == 0
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::F64Const(0.0));
    f.instruction(&Instruction::F64Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(trap_msg_ptr as i32));
    f.instruction(&Instruction::I32Const(trap_msg_len as i32));
    f.instruction(&Instruction::Call(IMPORT_TRAP));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);

    // Compute quotient
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::F64Div);
    f.instruction(&Instruction::LocalSet(4));

    // NaN check: quotient != quotient → trap
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::F64Ne);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(trap_msg_ptr as i32));
    f.instruction(&Instruction::I32Const(trap_msg_len as i32));
    f.instruction(&Instruction::Call(IMPORT_TRAP));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);

    // Allocate + store result
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::F64Store(memarg(4, 3)));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_mod(a, b) -> i32` — f64 remainder.
pub fn emit_val_mod() -> Function {
    // WASM doesn't have f64.rem, so we implement: a - floor(a/b) * b
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 2: result ptr
        (1, ValType::F64), // local 3: a_val
        (1, ValType::F64), // local 4: b_val
    ]);

    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalSet(3));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalSet(4));

    // result = a - floor(a/b) * b
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(2));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));

    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(3)); // a
    f.instruction(&Instruction::LocalGet(3)); // a
    f.instruction(&Instruction::LocalGet(4)); // b
    f.instruction(&Instruction::F64Div);
    f.instruction(&Instruction::F64Floor);
    f.instruction(&Instruction::LocalGet(4)); // b
    f.instruction(&Instruction::F64Mul);
    f.instruction(&Instruction::F64Sub);
    f.instruction(&Instruction::F64Store(memarg(4, 3)));

    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_neg(a: i32) -> i32` — unary negate.
pub fn emit_val_neg() -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 1: result ptr
    ]);
    f.instruction(&Instruction::I32Const(VALUE_SIZE as i32));
    f.instruction(&Instruction::Call(rt_func_idx(RT_ALLOC)));
    f.instruction(&Instruction::LocalSet(1));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Store(memarg(0, 2)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::F64Neg);
    f.instruction(&Instruction::F64Store(memarg(4, 3)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_not(a: i32) -> i32` — logical not (bool).
pub fn emit_val_not() -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 1: bool val
    ]);
    // Read bool w1
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_BOOL)));
    f.instruction(&Instruction::End);
    f
}

/// Emit a comparison helper (returns BOOL value pointer).
fn emit_cmp(op: Instruction<'static>) -> Function {
    let mut f = Function::new(vec![]);
    // Load f64 from a, f64 from b, compare, wrap as bool value
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&op);
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_BOOL)));
    f.instruction(&Instruction::End);
    f
}

pub fn emit_val_lt() -> Function {
    emit_cmp(Instruction::F64Lt)
}
pub fn emit_val_le() -> Function {
    emit_cmp(Instruction::F64Le)
}
pub fn emit_val_gt() -> Function {
    emit_cmp(Instruction::F64Gt)
}
pub fn emit_val_ge() -> Function {
    emit_cmp(Instruction::F64Ge)
}

/// Emit `val_record_get(rec_ptr, key_ptr, key_len) -> i32`.
///
/// Linear scan of record entries to find a matching key.
/// Record entries layout: array of 12-byte triples [key_offset: i32, key_len: i32, value_ptr: i32].
pub fn emit_val_record_get() -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 3: entries_ptr (rec.w1)
        (1, ValType::I32), // local 4: count (rec.w2)
        (1, ValType::I32), // local 5: i (loop counter)
        (1, ValType::I32), // local 6: entry_ptr
        (1, ValType::I32), // local 7: result
    ]);

    // entries_ptr = rec_ptr.w1
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalSet(3));
    // count = rec_ptr.w2
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(4));

    // result = 0 (nil — will create nil if not found)
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(7));
    // i = 0
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(5));

    // loop over entries
    f.instruction(&Instruction::Block(BlockType::Empty)); // break target
    f.instruction(&Instruction::Loop(BlockType::Empty));

    // if i >= count → break
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::LocalGet(4));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1));

    // entry_ptr = entries_ptr + i * 12
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::I32Const(12));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(6));

    // Compare key length first
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Load(memarg(4, 2))); // entry key_len
    f.instruction(&Instruction::LocalGet(2));            // target key_len
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    // Lengths match — byte-by-byte memcmp on key data
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Load(memarg(0, 2))); // entry key_offset
    f.instruction(&Instruction::LocalGet(1));            // target key_ptr
    f.instruction(&Instruction::LocalGet(2));            // key_len
    f.instruction(&Instruction::Call(rt_func_idx(RT_MEMCMP)));
    f.instruction(&Instruction::If(BlockType::Empty));
    // Found! result = entry.value_ptr
    f.instruction(&Instruction::LocalGet(6));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(7));
    f.instruction(&Instruction::Br(3)); // break out of loop + block
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);

    // i += 1, continue
    f.instruction(&Instruction::LocalGet(5));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(5));
    f.instruction(&Instruction::Br(0));

    f.instruction(&Instruction::End); // end loop
    f.instruction(&Instruction::End); // end block

    // If result is 0 (not found), create nil
    f.instruction(&Instruction::LocalGet(7));
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    f.instruction(&Instruction::LocalSet(7));
    f.instruction(&Instruction::End);

    f.instruction(&Instruction::LocalGet(7));
    f.instruction(&Instruction::End);
    f
}

/// Emit `val_list_get(list_ptr, index) -> i32`.
pub fn emit_val_list_get() -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 2: arr_ptr
        (1, ValType::I32), // local 3: count
    ]);
    // arr_ptr = list.w1
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(4, 2)));
    f.instruction(&Instruction::LocalSet(2));
    // count = list.w2
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(8, 2)));
    f.instruction(&Instruction::LocalSet(3));

    // bounds check: if index >= count → return nil
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::If(BlockType::Result(ValType::I32)));
    f.instruction(&Instruction::Call(rt_func_idx(RT_VAL_NIL)));
    f.instruction(&Instruction::Else);
    // return *(arr_ptr + index * 4)
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Const(4));
    f.instruction(&Instruction::I32Mul);
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::End);

    f.instruction(&Instruction::End);
    f
}

/// Emit `check_nan(val_ptr: i32) -> i32` — traps if NUMBER value is NaN.
pub fn emit_check_nan(trap_msg_ptr: u32, trap_msg_len: u32) -> Function {
    let mut f = Function::new(vec![
        (1, ValType::F64), // local 1: the f64
    ]);
    // Only check if tag == NUMBER
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::I32Load(memarg(0, 2)));
    f.instruction(&Instruction::I32Const(TAG_NUMBER));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    // Load the f64
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::F64Load(memarg(4, 3)));
    f.instruction(&Instruction::LocalSet(1));
    // NaN check: x != x
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::F64Ne);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(trap_msg_ptr as i32));
    f.instruction(&Instruction::I32Const(trap_msg_len as i32));
    f.instruction(&Instruction::Call(IMPORT_TRAP));
    f.instruction(&Instruction::Unreachable);
    f.instruction(&Instruction::End);
    f.instruction(&Instruction::End);
    // Return the value unchanged
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::End);
    f
}

/// Emit `memcmp(ptr_a: i32, ptr_b: i32, len: i32) -> i32` — byte-by-byte comparison.
///
/// Returns 1 if all `len` bytes at `ptr_a` and `ptr_b` are identical, 0 otherwise.
/// Used by `val_eq` for string content comparison and `val_record_get` for key lookup.
pub fn emit_memcmp() -> Function {
    let mut f = Function::new(vec![
        (1, ValType::I32), // local 3: loop counter (i)
    ]);
    // params: 0=ptr_a, 1=ptr_b, 2=len

    // If len == 0 → return 1 (empty strings are equal)
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32Eqz);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::Return);
    f.instruction(&Instruction::End);

    // If pointers are equal → return 1 (same memory, trivially equal)
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Eq);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::Return);
    f.instruction(&Instruction::End);

    // i = 0
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::LocalSet(3));

    // Loop: compare byte at ptr_a+i vs ptr_b+i
    f.instruction(&Instruction::Block(BlockType::Empty)); // block (break target)
    f.instruction(&Instruction::Loop(BlockType::Empty));   // loop

    // if i >= len → break (all matched → return 1)
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::LocalGet(2));
    f.instruction(&Instruction::I32GeU);
    f.instruction(&Instruction::BrIf(1)); // br outer block

    // load byte at ptr_a + i
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Load8U(memarg(0, 0)));

    // load byte at ptr_b + i
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::I32Load8U(memarg(0, 0)));

    // if bytes differ → return 0
    f.instruction(&Instruction::I32Ne);
    f.instruction(&Instruction::If(BlockType::Empty));
    f.instruction(&Instruction::I32Const(0));
    f.instruction(&Instruction::Return);
    f.instruction(&Instruction::End);

    // i += 1
    f.instruction(&Instruction::LocalGet(3));
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::LocalSet(3));

    // continue loop
    f.instruction(&Instruction::Br(0));

    f.instruction(&Instruction::End); // end loop
    f.instruction(&Instruction::End); // end block

    // Reached here → all bytes matched
    f.instruction(&Instruction::I32Const(1));
    f.instruction(&Instruction::End);
    f
}

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

/// Create a `MemArg` with the given offset and alignment power.
pub(crate) fn memarg(offset: u64, align: u32) -> wasm_encoder::MemArg {
    wasm_encoder::MemArg {
        offset,
        align,
        memory_index: 0,
    }
}

/// Tracks data-segment offsets for well-known constant strings.
///
/// Built during module assembly (`compiler.rs`) and passed to runtime helpers
/// that need to reference string constants (e.g., `val_to_string`).
pub struct DataSegmentTracker {
    pub true_ptr: u32,
    pub true_len: u32,
    pub false_ptr: u32,
    pub false_len: u32,
    pub nil_ptr: u32,
    pub nil_len: u32,
    pub value_ptr: u32,
    pub value_len: u32,
    pub gas_exhausted_ptr: u32,
    pub gas_exhausted_len: u32,
    pub div_by_zero_ptr: u32,
    pub div_by_zero_len: u32,
    pub nan_ptr: u32,
    pub nan_len: u32,
    pub assert_failed_ptr: u32,
    pub assert_failed_len: u32,
    pub invariant_failed_ptr: u32,
    pub invariant_failed_len: u32,
    pub unwrap_failed_ptr: u32,
    pub unwrap_failed_len: u32,
    /// Next free offset in the data segment.
    pub next_offset: u32,
}

impl Default for DataSegmentTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSegmentTracker {
    /// Build the tracker, interning all well-known strings starting at offset 0.
    pub fn new() -> Self {
        let mut offset = 0u32;

        let true_ptr = offset;
        let true_len = 4u32; // "true"
        offset += true_len;

        let false_ptr = offset;
        let false_len = 5u32; // "false"
        offset += false_len;

        let nil_ptr = offset;
        let nil_len = 3u32; // "nil"
        offset += nil_len;

        let value_ptr = offset;
        let value_len = 7u32; // "[value]"
        offset += value_len;

        let gas_exhausted_ptr = offset;
        let gas_exhausted_len = 13u32; // "gas exhausted"
        offset += gas_exhausted_len;

        let div_by_zero_ptr = offset;
        let div_by_zero_len = 16u32; // "division by zero"
        offset += div_by_zero_len;

        let nan_ptr = offset;
        let nan_len = 10u32; // "NaN result"
        offset += nan_len;

        let assert_failed_ptr = offset;
        let assert_failed_len = 16u32; // "assertion failed"
        offset += assert_failed_len;

        let invariant_failed_ptr = offset;
        let invariant_failed_len = 18u32; // "invariant violated"
        offset += invariant_failed_len;

        let unwrap_failed_ptr = offset;
        let unwrap_failed_len = 14u32; // "unwrap on Err"
        offset += unwrap_failed_len;

        Self {
            true_ptr,
            true_len,
            false_ptr,
            false_len,
            nil_ptr,
            nil_len,
            value_ptr,
            value_len,
            gas_exhausted_ptr,
            gas_exhausted_len,
            div_by_zero_ptr,
            div_by_zero_len,
            nan_ptr,
            nan_len,
            assert_failed_ptr,
            assert_failed_len,
            invariant_failed_ptr,
            invariant_failed_len,
            unwrap_failed_ptr,
            unwrap_failed_len,
            next_offset: offset,
        }
    }

    /// The raw bytes for the data segment — all well-known strings concatenated.
    pub fn data_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"true");
        buf.extend_from_slice(b"false");
        buf.extend_from_slice(b"nil");
        buf.extend_from_slice(b"[value]");
        buf.extend_from_slice(b"gas exhausted");
        buf.extend_from_slice(b"division by zero");
        buf.extend_from_slice(b"NaN result");
        buf.extend_from_slice(b"assertion failed");
        buf.extend_from_slice(b"invariant violated");
        buf.extend_from_slice(b"unwrap on Err!");
        buf
    }

    /// Intern a user string literal and return (offset, length).
    /// The caller must also append the bytes to the data segment.
    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        let ptr = self.next_offset;
        let len = s.len() as u32;
        self.next_offset += len;
        (ptr, len)
    }
}

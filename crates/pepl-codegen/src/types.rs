//! WASM value-representation constants and memory layout.
//!
//! Every PEPL value is stored on the linear-memory heap as a 12-byte cell:
//!
//! ```text
//! offset+0 : i32  — tag   (see TAG_* constants)
//! offset+4 : 8 bytes — payload (interpretation depends on tag)
//! ```
//!
//! # Payload layouts
//!
//! | Tag        | Bytes 4..8 (word-1)        | Bytes 8..12 (word-2)       |
//! |------------|----------------------------|----------------------------|
//! | NIL        | 0                          | 0                          |
//! | NUMBER     | f64 occupies all 8 bytes (little-endian)                |
//! | BOOL       | i32 (0 = false, 1 = true)  | 0 (padding)                |
//! | STRING     | i32 data-offset            | i32 byte-length            |
//! | LIST       | i32 array-offset           | i32 element-count          |
//! | RECORD     | i32 entries-offset          | i32 field-count            |
//! | VARIANT    | i32 variant-id             | i32 data-ptr (record)      |
//! | LAMBDA     | i32 func-index             | i32 closure-ptr            |
//! | COLOR      | i32 rgba-packed            | 0 (padding)                |
//! | ACTION_REF | i32 action-id              | 0 (padding)                |

/// Size of a single PEPL value cell on the heap (bytes).
pub const VALUE_SIZE: u32 = 12;

// ── Value tags ───────────────────────────────────────────────────────────────

pub const TAG_NIL: i32 = 0;
pub const TAG_NUMBER: i32 = 1;
pub const TAG_BOOL: i32 = 2;
pub const TAG_STRING: i32 = 3;
pub const TAG_LIST: i32 = 4;
pub const TAG_RECORD: i32 = 5;
pub const TAG_VARIANT: i32 = 6;
pub const TAG_LAMBDA: i32 = 7;
pub const TAG_COLOR: i32 = 8;
pub const TAG_ACTION_REF: i32 = 9;

// ── Global variable indices ──────────────────────────────────────────────────
// (order must match the global section emission in compiler.rs)

/// Heap allocation pointer — next free byte in linear memory.
pub const GLOBAL_HEAP_PTR: u32 = 0;
/// Gas counter — incremented on each tick.
pub const GLOBAL_GAS: u32 = 1;
/// Gas limit — set at init, trap when exceeded.
pub const GLOBAL_GAS_LIMIT: u32 = 2;
/// Pointer to the state record value.
pub const GLOBAL_STATE_PTR: u32 = 3;

// ── Imported function indices ────────────────────────────────────────────────
// (order must match the import section emission in compiler.rs)

/// `env.host_call(cap_id: i32, fn_id: i32, args_ptr: i32) -> i32`
pub const IMPORT_HOST_CALL: u32 = 0;
/// `env.log(ptr: i32, len: i32)`
pub const IMPORT_LOG: u32 = 1;
/// `env.trap(ptr: i32, len: i32)` — aborts execution with a message.
pub const IMPORT_TRAP: u32 = 2;

/// Number of imported functions (offset for locally-defined function indices).
pub const IMPORT_COUNT: u32 = 3;

// ── WASM type indices ────────────────────────────────────────────────────────
// Fixed type indices in the type section (see compiler.rs emit_types).

/// `() -> ()`
pub const TYPE_VOID_VOID: u32 = 0;
/// `() -> i32`
pub const TYPE_VOID_I32: u32 = 1;
/// `(i32) -> ()`
pub const TYPE_I32_VOID: u32 = 2;
/// `(i32) -> i32`
pub const TYPE_I32_I32: u32 = 3;
/// `(i32, i32) -> ()`
pub const TYPE_I32X2_VOID: u32 = 4;
/// `(i32, i32) -> i32`
pub const TYPE_I32X2_I32: u32 = 5;
/// `(i32, i32, i32) -> i32`
pub const TYPE_I32X3_I32: u32 = 6;
/// `(f64) -> i32`
pub const TYPE_F64_I32: u32 = 7;
/// `(i32, f64) -> ()`
pub const TYPE_I32_F64_VOID: u32 = 8;

/// Total number of fixed type signatures.
pub const TYPE_COUNT: u32 = 9;

// ── Memory ───────────────────────────────────────────────────────────────────

/// Initial linear memory size in pages (64 KiB each).
pub const INITIAL_MEMORY_PAGES: u64 = 1;
/// Maximum linear memory pages (16 MiB).
pub const MAX_MEMORY_PAGES: u64 = 256;
/// Heap starts after the data segment region. We reserve the first 4 KiB for
/// static data (string constants, etc.).
pub const HEAP_START: u32 = 4096;

// ── Custom section ───────────────────────────────────────────────────────────

/// Custom section name for PEPL metadata.
pub const CUSTOM_SECTION_NAME: &str = "pepl";
/// Compiler version embedded in the custom section.
pub const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

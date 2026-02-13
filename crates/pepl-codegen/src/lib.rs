//! PEPL WASM code generator: compiles a verified AST to `.wasm` binary.
//!
//! # Architecture
//!
//! The code generator takes a validated [`pepl_types::ast::Program`] and
//! produces a self-contained `.wasm` module.  The generated module follows
//! the PEPL host-integration contract:
//!
//! ## Imports
//! - `env.host_call(cap_id, fn_id, args_ptr) → result_ptr`
//! - `env.log(ptr, len)`
//! - `env.trap(ptr, len)`
//!
//! ## Exports
//! - `init(gas_limit)` — initialise state to defaults
//! - `dispatch_action(action_id, args_ptr) → result_ptr`
//! - `render(view_id) → surface_ptr`
//! - `get_state() → state_ptr`
//! - `alloc(size) → ptr`
//! - `memory` — linear memory
//! - (conditional) `update(dt_ptr)`, `handle_event(event_ptr)`
//!
//! ## Value Representation
//!
//! Every PEPL value is a heap-allocated 12-byte cell:
//! `[tag: i32, payload: 8 bytes]`.  See [`types`] for tag constants.

pub mod compiler;
pub mod error;
pub mod expr;
pub mod gas;
pub mod runtime;
pub mod source_map;
pub mod space;
pub mod stmt;
pub mod test_codegen;
pub mod types;

pub use compiler::{compile, compile_with_source_map};
pub use error::{CodegenError, CodegenResult};
pub use source_map::SourceMap;

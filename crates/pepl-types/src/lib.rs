//! Shared types for the PEPL compiler.
//!
//! This crate defines the AST node types, source spans, error types,
//! and other shared data structures used across all compiler stages.

mod error;
mod span;
pub mod ast;

pub use error::{CompileErrors, ErrorCategory, ErrorCode, PeplError, Severity, MAX_ERRORS};
pub use span::{SourceFile, Span};

/// Result type used throughout the PEPL compiler.
pub type Result<T> = std::result::Result<T, PeplError>;

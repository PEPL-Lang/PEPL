//! Codegen error types.

use thiserror::Error;

/// Errors that can occur during WASM code generation.
#[derive(Debug, Error)]
pub enum CodegenError {
    /// An AST feature is not yet supported by the code generator.
    #[error("unsupported feature: {0}")]
    Unsupported(String),

    /// An internal consistency check failed.
    #[error("internal codegen error: {0}")]
    Internal(String),

    /// The generated WASM module failed validation.
    #[error("WASM validation failed: {0}")]
    ValidationFailed(String),

    /// A symbol could not be resolved during codegen.
    #[error("unresolved symbol: {0}")]
    UnresolvedSymbol(String),

    /// Too many locals, functions, or other entities exceeded WASM limits.
    #[error("limit exceeded: {0}")]
    LimitExceeded(String),
}

/// Codegen result type alias.
pub type CodegenResult<T> = Result<T, CodegenError>;

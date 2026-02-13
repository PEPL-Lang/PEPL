//! PEPL compiler: orchestrates the full compilation pipeline.
//!
//! ```text
//! PEPL Source → Lexer → Parser → Type Checker → Invariant Checker → WASM Codegen → .wasm
//! ```
//!
//! # Two entry points
//!
//! - [`type_check`] — Parse + type-check only, returning structured errors.
//! - [`compile`] — Full pipeline: parse → type-check → codegen → `.wasm` bytes.
//! - [`compile_to_result`] — Full pipeline returning a [`CompileResult`] (JSON-serializable).

pub mod checker;
pub mod env;
pub mod stdlib;
pub mod ty;

use pepl_codegen::CodegenError;
use pepl_types::{CompileErrors, SourceFile};
use serde::{Deserialize, Serialize};

// ── CompileResult ─────────────────────────────────────────────────────────────

/// The result of a full compilation pipeline.
///
/// Serializable to JSON for the host / playground.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileResult {
    /// Whether compilation succeeded.
    pub success: bool,
    /// The compiled `.wasm` bytes (base64-encoded in JSON), if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<Vec<u8>>,
    /// Structured compile errors, if any.
    pub errors: CompileErrors,
}

// ── type_check ────────────────────────────────────────────────────────────────

/// Type-check a PEPL source file.
///
/// Parses the source and runs the type checker, returning any errors found.
pub fn type_check(source: &str, name: &str) -> CompileErrors {
    let source_file = SourceFile::new(name.to_string(), source.to_string());

    // 1. Lex
    let lex_result = pepl_lexer::Lexer::new(&source_file).lex();
    if lex_result.errors.has_errors() {
        return lex_result.errors;
    }

    // 2. Parse
    let parse_result = pepl_parser::Parser::new(lex_result.tokens, &source_file).parse();
    if parse_result.errors.has_errors() {
        return parse_result.errors;
    }

    let program = match parse_result.program {
        Some(p) => p,
        None => return parse_result.errors,
    };

    // 3. Type-check
    let mut errors = CompileErrors::empty();
    let mut tc = checker::TypeChecker::new(&mut errors, &source_file);
    tc.check(&program);

    errors
}

// ── compile ───────────────────────────────────────────────────────────────────

/// Full compilation pipeline: source → `.wasm` bytes.
///
/// Returns `Ok(wasm_bytes)` on success, or `Err(CompileErrors)` if there are
/// syntax, type, or invariant errors. Codegen errors are converted to a
/// single internal error in `CompileErrors`.
pub fn compile(source: &str, name: &str) -> Result<Vec<u8>, CompileErrors> {
    let source_file = SourceFile::new(name.to_string(), source.to_string());

    // 1. Lex
    let lex_result = pepl_lexer::Lexer::new(&source_file).lex();
    if lex_result.errors.has_errors() {
        return Err(lex_result.errors);
    }

    // 2. Parse
    let parse_result = pepl_parser::Parser::new(lex_result.tokens, &source_file).parse();
    if parse_result.errors.has_errors() {
        return Err(parse_result.errors);
    }

    let program = match parse_result.program {
        Some(p) => p,
        None => return Err(parse_result.errors),
    };

    // 3. Type-check (includes invariant checking)
    let mut errors = CompileErrors::empty();
    {
        let mut tc = checker::TypeChecker::new(&mut errors, &source_file);
        tc.check(&program);
    }
    if errors.has_errors() {
        return Err(errors);
    }

    // 4. Codegen → .wasm
    match pepl_codegen::compile(&program) {
        Ok(wasm) => Ok(wasm),
        Err(e) => {
            let mut errors = CompileErrors::empty();
            errors.push_error(codegen_error_to_pepl_error(&e, name));
            Err(errors)
        }
    }
}

/// Full compilation pipeline, returning a [`CompileResult`] (JSON-serializable).
///
/// This is the main entry point for the playground / WASM host.
pub fn compile_to_result(source: &str, name: &str) -> CompileResult {
    match compile(source, name) {
        Ok(wasm) => CompileResult {
            success: true,
            wasm: Some(wasm),
            errors: CompileErrors::empty(),
        },
        Err(errors) => CompileResult {
            success: false,
            wasm: None,
            errors,
        },
    }
}

/// Convert a codegen error to a PeplError for structured output.
fn codegen_error_to_pepl_error(e: &CodegenError, file: &str) -> pepl_types::PeplError {
    use pepl_types::{ErrorCode, Span};

    let (code, message) = match e {
        CodegenError::Unsupported(msg) => (ErrorCode(700), format!("Unsupported: {}", msg)),
        CodegenError::Internal(msg) => (ErrorCode(701), format!("Internal: {}", msg)),
        CodegenError::ValidationFailed(msg) => {
            (ErrorCode(702), format!("WASM validation failed: {}", msg))
        }
        CodegenError::UnresolvedSymbol(msg) => {
            (ErrorCode(703), format!("Unresolved symbol: {}", msg))
        }
        CodegenError::LimitExceeded(msg) => (ErrorCode(704), format!("Limit exceeded: {}", msg)),
    };

    pepl_types::PeplError::new(file, code, message, Span::new(1, 1, 1, 1), "")
}

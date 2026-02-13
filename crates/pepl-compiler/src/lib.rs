//! PEPL compiler: orchestrates the full compilation pipeline.
//!
//! ```text
//! PEPL Source → Lexer → Parser → Type Checker → Invariant Checker → WASM Codegen → .wasm
//! ```

pub mod checker;
pub mod env;
pub mod stdlib;
pub mod ty;

use pepl_types::{CompileErrors, SourceFile};

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

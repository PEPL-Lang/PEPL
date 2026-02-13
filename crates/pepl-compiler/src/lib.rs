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
pub mod reference;
pub mod stdlib;
pub mod ty;

use pepl_codegen::CodegenError;
use pepl_types::ast::Program;
use pepl_types::{CompileErrors, SourceFile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Version constants ─────────────────────────────────────────────────────────

/// PEPL language version (Phase 0).
pub const PEPL_LANGUAGE_VERSION: &str = "0.1.0";

/// Compiler version (matches Cargo package version).
pub const PEPL_COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ── CompileResult ─────────────────────────────────────────────────────────────

/// A declared state field with name and type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// A declared action with name and parameter types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionInfo {
    pub name: String,
    pub params: Vec<FieldInfo>,
}

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

    // ── Enrichment fields (Phase 10.1) ─────────────────────────────────

    /// Full AST (serializable to JSON).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<Program>,

    /// SHA-256 hash of the source text (hex-encoded).
    pub source_hash: String,

    /// SHA-256 hash of the compiled WASM bytes (hex-encoded), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_hash: Option<String>,

    /// State field names and types.
    pub state_fields: Vec<FieldInfo>,

    /// Action names and parameter types.
    pub actions: Vec<ActionInfo>,

    /// View names.
    pub views: Vec<String>,

    /// Declared required capabilities.
    pub capabilities: Vec<String>,

    /// Declared credentials (name + type).
    pub credentials: Vec<FieldInfo>,

    /// PEPL language version.
    pub language_version: String,

    /// Compiler version.
    pub compiler_version: String,

    /// Warnings from compilation (separate from errors).
    pub warnings: Vec<pepl_types::PeplError>,

    /// Source map: WASM function index → PEPL source location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_map: Option<pepl_codegen::SourceMap>,
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
/// Includes enriched metadata: AST, hashes, state/action/view lists, versions.
pub fn compile_to_result(source: &str, name: &str) -> CompileResult {
    let source_hash = sha256_hex(source.as_bytes());
    let source_file = SourceFile::new(name.to_string(), source.to_string());

    // 1. Lex
    let lex_result = pepl_lexer::Lexer::new(&source_file).lex();
    if lex_result.errors.has_errors() {
        return CompileResult {
            success: false,
            wasm: None,
            errors: lex_result.errors,
            ast: None,
            source_hash,
            wasm_hash: None,
            state_fields: Vec::new(),
            actions: Vec::new(),
            views: Vec::new(),
            capabilities: Vec::new(),
            credentials: Vec::new(),
            language_version: PEPL_LANGUAGE_VERSION.to_string(),
            compiler_version: PEPL_COMPILER_VERSION.to_string(),
            warnings: Vec::new(),
            source_map: None,
        };
    }

    // 2. Parse
    let parse_result = pepl_parser::Parser::new(lex_result.tokens, &source_file).parse();
    if parse_result.errors.has_errors() {
        return CompileResult {
            success: false,
            wasm: None,
            errors: parse_result.errors,
            ast: None,
            source_hash,
            wasm_hash: None,
            state_fields: Vec::new(),
            actions: Vec::new(),
            views: Vec::new(),
            capabilities: Vec::new(),
            credentials: Vec::new(),
            language_version: PEPL_LANGUAGE_VERSION.to_string(),
            compiler_version: PEPL_COMPILER_VERSION.to_string(),
            warnings: Vec::new(),
            source_map: None,
        };
    }

    let program = match parse_result.program {
        Some(p) => p,
        None => {
            return CompileResult {
                success: false,
                wasm: None,
                errors: parse_result.errors,
                ast: None,
                source_hash,
                wasm_hash: None,
                state_fields: Vec::new(),
                actions: Vec::new(),
                views: Vec::new(),
                capabilities: Vec::new(),
                credentials: Vec::new(),
                language_version: PEPL_LANGUAGE_VERSION.to_string(),
                compiler_version: PEPL_COMPILER_VERSION.to_string(),
                warnings: Vec::new(),
                source_map: None,
            };
        }
    };

    // Extract metadata from AST
    let metadata = extract_metadata(&program);

    // 3. Type-check
    let mut errors = CompileErrors::empty();
    {
        let mut tc = checker::TypeChecker::new(&mut errors, &source_file);
        tc.check(&program);
    }

    let warnings = errors.warnings.clone();

    if errors.has_errors() {
        return CompileResult {
            success: false,
            wasm: None,
            errors,
            ast: Some(program),
            source_hash,
            wasm_hash: None,
            state_fields: metadata.state_fields,
            actions: metadata.actions,
            views: metadata.views,
            capabilities: metadata.capabilities,
            credentials: metadata.credentials,
            language_version: PEPL_LANGUAGE_VERSION.to_string(),
            compiler_version: PEPL_COMPILER_VERSION.to_string(),
            warnings,
            source_map: None,
        };
    }

    // 4. Codegen → .wasm
    match pepl_codegen::compile_with_source_map(&program) {
        Ok((wasm, source_map)) => {
            let wasm_hash = sha256_hex(&wasm);
            CompileResult {
                success: true,
                wasm: Some(wasm),
                errors: CompileErrors::empty(),
                ast: Some(program),
                source_hash,
                wasm_hash: Some(wasm_hash),
                state_fields: metadata.state_fields,
                actions: metadata.actions,
                views: metadata.views,
                capabilities: metadata.capabilities,
                credentials: metadata.credentials,
                language_version: PEPL_LANGUAGE_VERSION.to_string(),
                compiler_version: PEPL_COMPILER_VERSION.to_string(),
                warnings,
                source_map: Some(source_map),
            }
        }
        Err(e) => {
            let mut errors = CompileErrors::empty();
            errors.push_error(codegen_error_to_pepl_error(&e, name));
            CompileResult {
                success: false,
                wasm: None,
                errors,
                ast: Some(program),
                source_hash,
                wasm_hash: None,
                state_fields: metadata.state_fields,
                actions: metadata.actions,
                views: metadata.views,
                capabilities: metadata.capabilities,
                credentials: metadata.credentials,
                language_version: PEPL_LANGUAGE_VERSION.to_string(),
                compiler_version: PEPL_COMPILER_VERSION.to_string(),
                warnings,
                source_map: None,
            }
        }
    }
}

// ── Metadata extraction ───────────────────────────────────────────────────────

struct SpaceMetadata {
    state_fields: Vec<FieldInfo>,
    actions: Vec<ActionInfo>,
    views: Vec<String>,
    capabilities: Vec<String>,
    credentials: Vec<FieldInfo>,
}

fn extract_metadata(program: &Program) -> SpaceMetadata {
    let body = &program.space.body;

    let state_fields = body
        .state
        .fields
        .iter()
        .map(|f| FieldInfo {
            name: f.name.name.clone(),
            ty: format!("{}", f.type_ann),
        })
        .collect();

    let actions = body
        .actions
        .iter()
        .map(|a| ActionInfo {
            name: a.name.name.clone(),
            params: a
                .params
                .iter()
                .map(|p| FieldInfo {
                    name: p.name.name.clone(),
                    ty: format!("{}", p.type_ann),
                })
                .collect(),
        })
        .collect();

    let views = body.views.iter().map(|v| v.name.name.clone()).collect();

    let capabilities = body
        .capabilities
        .as_ref()
        .map(|c| {
            c.required
                .iter()
                .chain(c.optional.iter())
                .map(|i| i.name.clone())
                .collect()
        })
        .unwrap_or_default();

    let credentials = body
        .credentials
        .as_ref()
        .map(|c| {
            c.fields
                .iter()
                .map(|f| FieldInfo {
                    name: f.name.name.clone(),
                    ty: format!("{}", f.type_ann),
                })
                .collect()
        })
        .unwrap_or_default();

    SpaceMetadata {
        state_fields,
        actions,
        views,
        capabilities,
        credentials,
    }
}

// ── Hashing ───────────────────────────────────────────────────────────────────

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex_encode(&result)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        write!(s, "{:02x}", b).unwrap();
    }
    s
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
        CodegenError::LimitExceeded(msg) => {
            (ErrorCode(704), format!("Limit exceeded: {}", msg))
        }
    };

    pepl_types::PeplError::new(file, code, message, Span::new(1, 1, 1, 1), "")
}

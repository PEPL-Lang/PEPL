# pepl-compiler

PEPL compiler: orchestrates the full compilation pipeline from source text to WASM binary.

Chains together lexing, parsing, type checking, invariant checking, and WASM code generation into a single API. Also provides an LLM reference generator and stdlib table export.

## Key Exports

```rust
use pepl_compiler::{compile, compile_to_result, type_check, CompileResult};

// Full compilation — source to WASM
let wasm: Result<Vec<u8>, _> = compile(source, "counter.pepl");

// Full compilation with metadata
let result: CompileResult = compile_to_result(source, "counter.pepl");
// result.success, result.wasm, result.errors, result.ast_hash, result.wasm_hash, ...

// Type-check only (no WASM generation)
let errors = type_check(source, "counter.pepl");
```

## `CompileResult`

```rust
pub struct CompileResult {
    pub success: bool,
    pub wasm: Option<Vec<u8>>,
    pub errors: CompileErrors,
    pub ast: Option<Program>,
    pub ast_hash: Option<String>,
    pub wasm_hash: Option<String>,
    pub metadata: Option<SpaceMetadata>,
    pub source_map: Option<SourceMap>,
}
```

## Pipeline

```
Source → Lexer → Parser → Type Checker → Invariant Checker → Codegen → WASM
```

## Constants

- `PEPL_LANGUAGE_VERSION` — the PEPL language version (`"0.1.0"`)
- `PEPL_COMPILER_VERSION` — the compiler crate version

## Install

```bash
cargo add pepl-compiler
```

## License

MIT — see [LICENSE](../../LICENSE)

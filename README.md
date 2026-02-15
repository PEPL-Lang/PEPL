# pepl

[![crates.io](https://img.shields.io/crates/v/pepl-compiler.svg)](https://crates.io/crates/pepl-compiler)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** Phase 13 (Contextual Keywords & Error Recovery) complete. Full end-to-end pipeline: source → `.wasm`. All 7 canonical examples compile through the complete pipeline with 100-iteration determinism. Browser-ready via `pepl-wasm` crate. Machine-generated LLM reference and stdlib table available. See [ROADMAP.md](ROADMAP.md) for progress.

## Architecture

```
PEPL Source → Lexer → Parser → Type Checker → Invariant Checker → Evaluator → WASM Codegen → .wasm
```

## Crates

| Crate | Purpose | Status |
|-------|---------|--------|
| `pepl-types` | Shared types: AST, Span, error infrastructure, error codes | ✅ Phase 1 done |
| `pepl-lexer` | Source → token stream (89 token kinds, string interpolation) | ✅ Phase 2 done |
| `pepl-parser` | Token stream → AST (recursive descent, precedence climbing) | ✅ Phase 3 done |
| `pepl-compiler` | Type checker + invariant checker + pipeline orchestrator + LLM reference generator | ✅ Phase 4–5, 8, 12 done |
| `pepl-eval` | Tree-walking evaluator (reference implementation) | ✅ Phase 6 core done |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`), test codegen, source maps | ✅ Phase 7, 11 done |
| `pepl-wasm` | Browser WASM package via `wasm-bindgen` (`compile`, `get_reference`, `get_stdlib_table`) | ✅ Phase 8, 12 done |

## API

### Full pipeline

```rust
use pepl_compiler::{compile, compile_to_result, CompileResult};

// Returns Ok(wasm_bytes) or Err(CompileErrors)
let wasm = compile(source, "counter.pepl")?;

// Returns CompileResult (JSON-serializable)
let result: CompileResult = compile_to_result(source, "counter.pepl");
assert!(result.success);
```

### Type-check only

```rust
let errors = pepl_compiler::type_check(source, "counter.pepl");
if errors.has_errors() { /* handle errors */ }
```

### LLM reference (machine-generated)

```rust
use pepl_compiler::reference;

// Compressed PEPL reference (~2K tokens) for LLM context injection
let reference = reference::generate_reference();

// Structured JSON stdlib table (all functions, signatures, descriptions)
let table = reference::generate_stdlib_table();
```

## Tests

588 tests across the workspace:
- `pepl-types`: 33 (error infrastructure, spans, AST diff)
- `pepl-lexer`: 80 (64 lexer + 16 token)
- `pepl-parser`: 132 (64 parser + 57 edge cases + 11 contextual keywords)
- `pepl-compiler`: 155 (70 type checker + 17 invariant checker + 12 M2 gate + 8 error code coverage + 22 pipeline + 14 LLM reference + 11 determinism/parity + 1 integration)
- `pepl-eval`: 87 (35 core eval + 52 canonical examples including test runner, game loop, determinism, golden reference)
- `pepl-codegen`: 101 (62 core codegen + 16 test codegen + 6 source map + 17 canonical/integration)

## Build

```bash
source "$HOME/.cargo/env"
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## Cross-Repo Coordination

This repo is part of the PEPL project alongside [`pepl-stdlib`](https://github.com/PEPL-Lang/PEPL-STDLIB) and [`pepl-ui`](https://github.com/PEPL-Lang/PEPL-UI). See `ORCHESTRATION.md` in the [`.github`](https://github.com/PEPL-Lang/.github) repo for the cross-repo build sequence.

## License

MIT

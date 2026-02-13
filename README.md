# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** Phase 8 (Integration & Packaging) complete. Full end-to-end pipeline: source → `.wasm`. All 7 canonical examples compile through the complete pipeline with 100-iteration determinism. Browser-ready via `pepl-wasm` crate. See [ROADMAP.md](ROADMAP.md) for progress.

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
| `pepl-compiler` | Type checker + invariant checker + full pipeline orchestrator | ✅ Phase 4–5, 8 done |
| `pepl-eval` | Tree-walking evaluator (reference implementation) | ✅ Phase 6 core done |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) | ✅ Phase 7 done |
| `pepl-wasm` | Browser WASM package via `wasm-bindgen` (Web Worker ready) | ✅ Phase 8 done |

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

## Tests

498 tests across the workspace:
- `pepl-types`: 19 (error infrastructure, spans)
- `pepl-lexer`: 80 (64 lexer + 16 token)
- `pepl-parser`: 121 (64 parser + 57 edge cases)
- `pepl-compiler`: 129 (70 type checker + 17 invariant checker + 12 M2 gate + 8 error code coverage + 22 pipeline integration)
- `pepl-eval`: 87 (35 core eval + 52 canonical examples including test runner, game loop, determinism, golden reference)
- `pepl-codegen`: 62 (module structure, exports, expressions, statements, actions, views, invariants, derived, update/handleEvent, determinism, canonical examples)

## Build

```bash
source "$HOME/.cargo/env"
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## Cross-Repo Coordination

This repo is part of the PEPL project alongside [`pepl-stdlib`](https://github.com/PEPL-Lang/PEPL-STDLIB) and [`pepl-ui`](https://github.com/PEPL-Lang/PEPL-UI). See `ORCHESTRATION.md` at the workspace root for the cross-repo build sequence.

## License

MIT

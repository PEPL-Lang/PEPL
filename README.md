# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** Phase 7 (WASM code generator) complete. All canonical examples compile to valid WASM bytes with 100-iteration determinism. See [ROADMAP.md](ROADMAP.md) for progress.

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
| `pepl-compiler` | Type checker + invariant checker (E402 component validation, Result match bindings) | ✅ Phase 4–5 done |
| `pepl-eval` | Tree-walking evaluator (reference implementation) | ✅ Phase 6 core done |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) | ✅ Phase 7 done |

## Tests

476 tests across the workspace:
- `pepl-types`: 19 (error infrastructure, spans)
- `pepl-lexer`: 80 (64 lexer + 16 token)
- `pepl-parser`: 121 (64 parser + 57 edge cases)
- `pepl-compiler`: 107 (70 type checker + 17 invariant checker + 12 M2 gate + 8 error code coverage)
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

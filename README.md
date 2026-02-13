# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** M2 milestone complete (all 7 canonical examples pass front-end). See [ROADMAP.md](ROADMAP.md) for progress.

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
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) | Planned (Phase 7) |

## Tests

362 tests across the workspace:
- `pepl-types`: 19 (error infrastructure, spans)
- `pepl-lexer`: 80 (64 lexer + 16 token)
- `pepl-parser`: 121 (64 parser + 57 edge cases)
- `pepl-compiler`: 107 (70 type checker + 17 invariant checker + 12 M2 gate + 8 error code coverage)
- `pepl-eval`: 35 (state init, actions, invariants, derived fields, stdlib, views, canonical examples)

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

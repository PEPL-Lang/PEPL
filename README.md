# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** Phase 4 complete (Type Checker). See [ROADMAP.md](ROADMAP.md) for progress.

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
| `pepl-compiler` | Type checker (17-variant Type enum, 88 stdlib sigs, scoped env) | ✅ Phase 4 done |
| `pepl-eval` | Tree-walking evaluator (reference implementation) | Planned (Phase 6) |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) | Planned (Phase 7) |

## Tests

290 tests across the workspace:
- `pepl-types`: 19 (error infrastructure, spans)
- `pepl-lexer`: 80 (64 lexer + 16 token)
- `pepl-parser`: 121 (64 parser + 57 edge cases)
- `pepl-compiler`: 70 (type checker integration tests)

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

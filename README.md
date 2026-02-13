# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** Phase 5 complete (Invariant Checker). See [ROADMAP.md](ROADMAP.md) for progress.

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
| `pepl-compiler` | Type checker + invariant checker (E300 derived-in-invariant, E502 recursion detection) | ✅ Phase 4–5 done |
| `pepl-eval` | Tree-walking evaluator (reference implementation) | Planned (Phase 6) |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) | Planned (Phase 7) |

## Tests

307 tests across the workspace:
- `pepl-types`: 19 (error infrastructure, spans)
- `pepl-lexer`: 80 (64 lexer + 16 token)
- `pepl-parser`: 121 (64 parser + 57 edge cases)
- `pepl-compiler`: 87 (70 type checker + 17 invariant checker)

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

# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, evaluates via a tree-walking interpreter, and emits WASM bytecode.

**Status:** Phase 0 — under construction. See [ROADMAP.md](ROADMAP.md) for progress.

## Architecture

```
PEPL Source → Lexer → Parser → Type Checker → Invariant Checker → Evaluator → WASM Codegen → .wasm
```

## Crates

| Crate | Purpose | Status |
|-------|---------|--------|
| `pepl-types` | Shared types: Span, error infrastructure, error codes | ✅ Phase 1 done |
| `pepl-lexer` | Source → token stream | 🚧 Phase 2 in progress |
| `pepl-parser` | Token stream → AST | Planned |
| `pepl-eval` | Tree-walking evaluator (reference implementation) | Planned |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) | Planned |
| `pepl-compiler` | Pipeline orchestration | Planned |

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

# pepl

The PEPL compiler — parses PEPL source, type-checks, validates invariants, and emits WASM bytecode.

**Status:** Phase 0 — under construction. See [ROADMAP.md](ROADMAP.md) for progress.

## Architecture

```
PEPL Source → Lexer → Parser → Type Checker → Invariant Checker → WASM Codegen → .wasm
```

## Crates

| Crate | Purpose |
|-------|---------|
| `pepl-types` | Shared types: AST nodes, Span, error infrastructure |
| `pepl-lexer` | Source → token stream |
| `pepl-parser` | Token stream → AST |
| `pepl-codegen` | Verified AST → `.wasm` binary (via `wasm-encoder`) |
| `pepl-compiler` | Pipeline orchestration |

## Build

```bash
cargo build --workspace
cargo test --workspace
```

## License

MIT

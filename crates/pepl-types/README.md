# pepl-types

Shared types for the PEPL compiler: AST nodes, Span, and structured errors.

This is the foundation crate — every other crate in the PEPL compiler workspace depends on `pepl-types`.

## What's Inside

| Module | Purpose |
|--------|---------|
| `ast` | All AST node types: expressions, statements, declarations, types, UI |
| `ast_diff` | Structural diffing between two ASTs |
| `error` | `PeplError`, `CompileErrors`, error codes, severity levels |
| `span` | `Span` (byte-range source locations) and `SourceFile` |

## Key Exports

```rust
use pepl_types::{Span, SourceFile, PeplError, CompileErrors, Severity};
use pepl_types::ast::*;
use pepl_types::Result; // alias for std::result::Result<T, PeplError>
```

## Install

```bash
cargo add pepl-types
```

## License

MIT — see [LICENSE](../../LICENSE)

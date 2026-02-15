# pepl-codegen

PEPL WASM code generator: compiles a verified AST into a `.wasm` binary.

Takes a type-checked and invariant-checked PEPL AST and emits a valid WebAssembly module using `wasm-encoder`. Includes gas metering, source maps, and the PEPL runtime ABI.

## Key Exports

```rust
use pepl_codegen::{compile, compile_with_source_map, CodegenError, CodegenResult, SourceMap};

let wasm_bytes: CodegenResult<Vec<u8>> = compile(&ast, &source_file);
let (wasm_bytes, source_map) = compile_with_source_map(&ast, &source_file)?;
```

## Features

- **WASM output** — generates valid `.wasm` binaries via `wasm-encoder`
- **Gas metering** — injects gas accounting into generated code
- **Source maps** — maps WASM instructions back to PEPL source locations
- **Runtime ABI** — defines the host import/export contract for PEPL modules

## Install

```bash
cargo add pepl-codegen
```

## License

MIT — see [LICENSE](../../LICENSE)

# pepl-wasm

PEPL compiler packaged as a WASM module for browser environments.

Wraps `pepl-compiler` with `wasm-bindgen` so the full compilation pipeline can run in a browser Web Worker. Used by the PEPL playground and any web-based PEPL tooling.

## JavaScript API

```js
import init, { compile, type_check, version, get_reference, get_stdlib_table } from 'pepl-wasm';

await init();

// Compile PEPL source to WASM
const result = JSON.parse(compile("space Counter { ... }", "counter.pepl"));
// { success: true, wasm: [0, 97, 115, 109, ...], errors: { ... } }

// Type-check only (faster, no WASM generation)
const errors = JSON.parse(type_check(source, "counter.pepl"));

// Compiler version
const v = version();

// LLM reference (compressed language spec)
const ref = get_reference();

// Structured stdlib table (JSON)
const table = JSON.parse(get_stdlib_table());
```

## Exported Functions

| Function | Returns | Purpose |
|----------|---------|---------|
| `compile(source, filename)` | JSON string | Full compilation to WASM binary |
| `type_check(source, filename)` | JSON string | Diagnostics without WASM generation |
| `version()` | String | Compiler version |
| `get_reference()` | String | Compressed PEPL language reference |
| `get_stdlib_table()` | JSON string | All stdlib functions with signatures |

## Build

```bash
wasm-pack build --target web
```

## License

MIT — see [LICENSE](../../LICENSE)

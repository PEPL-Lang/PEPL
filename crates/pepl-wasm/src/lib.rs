//! PEPL compiler as a WASM module for browser environments.
//!
//! This crate exposes the PEPL compilation pipeline via `wasm-bindgen`,
//! suitable for running in a browser Web Worker.
//!
//! # Usage (JavaScript)
//!
//! ```js
//! import init, { compile, type_check } from 'pepl-wasm';
//!
//! await init();
//!
//! const result = compile("space Counter { ... }", "counter.pepl");
//! console.log(JSON.parse(result));
//! // { success: true, wasm: [0, 97, 115, 109, ...], errors: { ... } }
//! ```

use wasm_bindgen::prelude::*;

/// Compile a PEPL source file to WASM.
///
/// Returns a JSON string containing a `CompileResult`:
/// ```json
/// {
///   "success": true,
///   "wasm": [0, 97, 115, 109, ...],
///   "errors": { "errors": [], "warnings": [], "total_errors": 0, "total_warnings": 0 }
/// }
/// ```
///
/// On failure, `success` is `false`, `wasm` is `null`, and `errors` contains
/// structured error information.
#[wasm_bindgen]
pub fn compile(source: &str, filename: &str) -> String {
    let result = pepl_compiler::compile_to_result(source, filename);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            r#"{{"success":false,"wasm":null,"errors":{{"errors":[{{"message":"Serialization error: {}"}}],"warnings":[],"total_errors":1,"total_warnings":0}}}}"#,
            e
        )
    })
}

/// Type-check a PEPL source file without generating WASM.
///
/// Returns a JSON string containing structured errors/warnings.
/// Faster than full compilation when only diagnostics are needed
/// (e.g., editor integration).
#[wasm_bindgen]
pub fn type_check(source: &str, filename: &str) -> String {
    let errors = pepl_compiler::type_check(source, filename);
    serde_json::to_string(&errors).unwrap_or_else(|e| {
        format!(
            r#"{{"errors":[{{"message":"Serialization error: {}"}}],"warnings":[],"total_errors":1,"total_warnings":0}}"#,
            e
        )
    })
}

/// Return the compiler version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

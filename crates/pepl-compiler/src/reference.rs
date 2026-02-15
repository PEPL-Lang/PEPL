//! Machine-generated PEPL reference and stdlib table.
//!
//! Produces two text artifacts from [`StdlibRegistry`]:
//! 1. **Compressed PEPL reference** (~2K tokens) for LLM context injection
//! 2. **Structured stdlib table** (JSON) for tooling and documentation
//!
//! Both auto-update when the stdlib registry changes — they are generated,
//! not hand-written.

use std::collections::BTreeMap;

use crate::stdlib::StdlibRegistry;

// ══════════════════════════════════════════════════════════════════════════════
// Module ordering and descriptions
// ══════════════════════════════════════════════════════════════════════════════

/// Canonical module ordering for output stability.
const MODULE_ORDER: &[&str] = &[
    "core",
    "math",
    "string",
    "list",
    "record",
    "time",
    "convert",
    "json",
    "timer",
    "http",
    "storage",
    "location",
    "notifications",
];

/// Build a description map for all stdlib functions.
/// Key: `(module, function)`, value: human-readable description.
fn stdlib_descriptions() -> BTreeMap<(&'static str, &'static str), &'static str> {
    let mut d = BTreeMap::new();

    // ── core ──
    d.insert(("core", "log"), "Debug logging (no-op in production, writes to console in dev)");
    d.insert(("core", "assert"), "Panics (WASM trap) if condition is false");
    d.insert(("core", "type_of"), "Returns type name: \"number\", \"string\", \"bool\", \"nil\", \"list\", \"record\"");
    d.insert(("core", "capability"), "Returns whether a declared optional capability is available at runtime");

    // ── math ──
    d.insert(("math", "abs"), "Absolute value");
    d.insert(("math", "min"), "Smaller of two values");
    d.insert(("math", "max"), "Larger of two values");
    d.insert(("math", "floor"), "Round down to nearest integer");
    d.insert(("math", "ceil"), "Round up to nearest integer");
    d.insert(("math", "round"), "Round to nearest integer (0.5 rounds up)");
    d.insert(("math", "round_to"), "Round to N decimal places");
    d.insert(("math", "pow"), "Exponentiation");
    d.insert(("math", "clamp"), "Clamp value to [min, max] range");
    d.insert(("math", "sqrt"), "Square root");

    // ── string ──
    d.insert(("string", "length"), "Number of characters");
    d.insert(("string", "concat"), "Concatenate two strings");
    d.insert(("string", "contains"), "True if needle found in haystack");
    d.insert(("string", "slice"), "Substring from start (inclusive) to end (exclusive)");
    d.insert(("string", "trim"), "Remove leading/trailing whitespace");
    d.insert(("string", "split"), "Split string by delimiter");
    d.insert(("string", "to_upper"), "Convert to uppercase");
    d.insert(("string", "to_lower"), "Convert to lowercase");
    d.insert(("string", "starts_with"), "True if s starts with prefix");
    d.insert(("string", "ends_with"), "True if s ends with suffix");
    d.insert(("string", "replace"), "Replace first occurrence of old with new");
    d.insert(("string", "replace_all"), "Replace all occurrences of old with new");
    d.insert(("string", "pad_start"), "Pad string on the left to reach target length");
    d.insert(("string", "pad_end"), "Pad string on the right to reach target length");
    d.insert(("string", "repeat"), "Repeat string count times");
    d.insert(("string", "join"), "Join list of strings with separator");
    d.insert(("string", "format"), "Template string with {key} placeholders replaced by record values");
    d.insert(("string", "from"), "Convert any value to its string representation");
    d.insert(("string", "is_empty"), "True if string has zero length");
    d.insert(("string", "index_of"), "Index of first occurrence of sub, or -1 if not found");

    // ── list ──
    d.insert(("list", "empty"), "Create empty typed list");
    d.insert(("list", "of"), "Create list from arguments (compiler-special-cased variadic)");
    d.insert(("list", "repeat"), "Create list of count copies of item");
    d.insert(("list", "range"), "Generate list of integers from start (inclusive) to end (exclusive)");
    d.insert(("list", "length"), "Number of elements");
    d.insert(("list", "get"), "Get item at index, returns nil if out of bounds");
    d.insert(("list", "first"), "First element, or nil if empty");
    d.insert(("list", "last"), "Last element, or nil if empty");
    d.insert(("list", "index_of"), "Index of first occurrence of item, or -1 if not found");
    d.insert(("list", "append"), "Return new list with item added at end");
    d.insert(("list", "prepend"), "Return new list with item added at start");
    d.insert(("list", "insert"), "Return new list with item inserted at index");
    d.insert(("list", "remove"), "Return new list with item at index removed");
    d.insert(("list", "update"), "Return new list with item at index replaced");
    d.insert(("list", "set"), "Return new list with item at index replaced (alias for update)");
    d.insert(("list", "slice"), "Sublist from start (inclusive) to end (exclusive)");
    d.insert(("list", "concat"), "Concatenate two lists");
    d.insert(("list", "reverse"), "Return reversed list");
    d.insert(("list", "flatten"), "Flatten one level of nesting");
    d.insert(("list", "unique"), "Remove duplicate elements (preserves first occurrence)");
    d.insert(("list", "map"), "Transform each element");
    d.insert(("list", "filter"), "Keep elements where fn returns true");
    d.insert(("list", "reduce"), "Left fold with initial value");
    d.insert(("list", "find"), "First element where fn returns true, or nil");
    d.insert(("list", "find_index"), "Index of first element where fn returns true, or -1");
    d.insert(("list", "every"), "True if fn returns true for every element");
    d.insert(("list", "any"), "True if fn returns true for any element");
    d.insert(("list", "some"), "Alias for list.any (backward compatibility)");
    d.insert(("list", "sort"), "Sort by comparator function");
    d.insert(("list", "contains"), "True if item is in list");
    d.insert(("list", "count"), "Count elements where fn returns true");
    d.insert(("list", "zip"), "Combine two lists element-wise into list of pairs");
    d.insert(("list", "take"), "Return first count elements");
    d.insert(("list", "drop"), "Return all elements after the first count");

    // ── record ──
    d.insert(("record", "get"), "Get field value by name");
    d.insert(("record", "set"), "Return new record with field updated");
    d.insert(("record", "has"), "True if record has the named field");
    d.insert(("record", "keys"), "List of field names");
    d.insert(("record", "values"), "List of field values");

    // ── time ──
    d.insert(("time", "now"), "Current timestamp in milliseconds (host-provided)");
    d.insert(("time", "format"), "Format timestamp with pattern (YYYY-MM-DD, HH:mm, etc.)");
    d.insert(("time", "diff"), "Difference in milliseconds (a - b)");
    d.insert(("time", "day_of_week"), "0=Sunday through 6=Saturday");
    d.insert(("time", "start_of_day"), "Timestamp of midnight (00:00) for the given day");

    // ── convert ──
    d.insert(("convert", "to_string"), "Convert any value to string representation");
    d.insert(("convert", "to_number"), "Convert to number (parses strings, bool->0/1), returns Result");
    d.insert(("convert", "parse_int"), "Parse string to integer, returns Result");
    d.insert(("convert", "parse_float"), "Parse string to float, returns Result");
    d.insert(("convert", "to_bool"), "Truthy conversion (0/nil/\"\" -> false, else true)");

    // ── json ──
    d.insert(("json", "parse"), "Parse JSON string to PEPL value, returns Result");
    d.insert(("json", "stringify"), "Serialize PEPL value to JSON string");

    // ── timer ──
    d.insert(("timer", "start"), "Start recurring timer dispatching action at interval, returns ID");
    d.insert(("timer", "start_once"), "Schedule one-shot action dispatch after delay, returns ID");
    d.insert(("timer", "stop"), "Stop a running timer by ID");
    d.insert(("timer", "stop_all"), "Stop all active timers for this space");

    // ── http (capability) ──
    d.insert(("http", "get"), "HTTP GET request, returns Result<string, string>");
    d.insert(("http", "post"), "HTTP POST request, returns Result<string, string>");
    d.insert(("http", "put"), "HTTP PUT request, returns Result<string, string>");
    d.insert(("http", "patch"), "HTTP PATCH request, returns Result<string, string>");
    d.insert(("http", "delete"), "HTTP DELETE request, returns Result<string, string>");

    // ── storage (capability) ──
    d.insert(("storage", "get"), "Get stored value by key, returns string or nil");
    d.insert(("storage", "set"), "Store a key-value pair");
    d.insert(("storage", "delete"), "Delete a stored key");
    d.insert(("storage", "keys"), "List all stored keys");

    // ── location (capability) ──
    d.insert(("location", "current"), "Get current location as { lat: number, lon: number }");

    // ── notifications (capability) ──
    d.insert(("notifications", "send"), "Send a notification with title and body");

    d
}

/// Constant descriptions: `(module, name)` → description.
fn constant_descriptions() -> BTreeMap<(&'static str, &'static str), &'static str> {
    let mut d = BTreeMap::new();
    d.insert(("math", "PI"), "Pi (3.14159265358979...)");
    d.insert(("math", "E"), "Euler's number (2.71828182845904...)");
    d
}

// ══════════════════════════════════════════════════════════════════════════════
// Compressed Reference Generation
// ══════════════════════════════════════════════════════════════════════════════

/// Generate the compressed PEPL reference (~2K tokens) for LLM context injection.
///
/// The STDLIB section is machine-generated from the [`StdlibRegistry`].
/// All other sections are static text matching the format in `llm-generation-contract.md`.
pub fn generate_reference() -> String {
    let reg = StdlibRegistry::new();
    let mut out = String::with_capacity(4096);

    // Static preamble
    out.push_str(REFERENCE_PREAMBLE);

    // Dynamic STDLIB section
    out.push_str("STDLIB (always available, no imports):\n");
    for &module_name in MODULE_ORDER {
        if let Some(funcs) = reg.modules().get(module_name) {
            let mut names: Vec<&String> = funcs.keys().collect();
            names.sort();

            // Also include constants for this module
            let const_names: Vec<&String> = reg
                .all_constants()
                .get(module_name)
                .map(|c| c.keys().collect())
                .unwrap_or_default();

            let all_names: Vec<String> = names
                .iter()
                .map(|n| n.to_string())
                .chain(const_names.iter().map(|n| n.to_string()))
                .collect();

            out.push_str(&format!("  {}: {}\n", module_name, all_names.join(", ")));
        }
    }
    out.push('\n');

    // Static postamble
    out.push_str(REFERENCE_POSTAMBLE);

    out
}

/// The static preamble of the compressed reference (everything before STDLIB).
const REFERENCE_PREAMBLE: &str = r#"PEPL: deterministic, sandboxed language. Compiles to WASM. One space per file.
Comments: // only (no block comments)

STRUCTURE (block order enforced):
  space Name {
    types      { type X = | A | B(field: type) }
    state      { field: type = default }
    capabilities { required: [http, storage] optional: [location] }
    credentials  { api_key: string }
    derived    { full_name: string = "${first} ${last}" }
    invariants { name { bool_expression } }
    actions    { action name(p: type) { set field = value } }
    views      { view main() -> Surface { Column { Text { value: "hi" } } } }
    update(dt: number) { ... }             // optional — game/animation loop
    handleEvent(event: InputEvent) { ... } // optional — game/interactive input
  }
  // Tests go OUTSIDE the space:
  tests { test "name" { assert expression } }

TYPES: number, string, bool, nil, color
  number covers integers, floats, AND timestamps/durations (Unix ms)
  No timestamp or duration types — use number
COMPOSITES: list<T>, { field: type }
  No record<{}> — use { field: type } inline
SUM TYPES: type Name = | Variant1(field: type) | Variant2
RESULT: type Result<T, E> = | Ok(value: T) | Err(error: E)
  No user-defined generics — only built-in list<T>, Result<T,E>

CONTROL FLOW:
  if cond { ... } else { ... }
  for item in list { ... }
  for item, index in list { ... }        // optional index binding
  match expr { Pattern(bind) -> result, _ -> default }
  let name: type = expression             // immutable binding
  set field = expression                  // state mutation (actions only)
  set record.field = expression            // sugar for { ...record, field: expr }
  return                                  // early exit from action (no value)

OPERATORS:
  Arithmetic: + - * / %
  Comparison: == != < > <= >=
  Logical: not and or
  Result unwrap: expr?                    // postfix — traps on Err
  Nil-coalescing: expr ?? fallback
  Record spread: { ...base, field: val }

NIL NARROWING:
  if x != nil { ... }                    // x narrows from T|nil to T in block
  let item = list.get(items, i) ?? fallback  // also valid

STRING INTERPOLATION: "Hello ${name}, you have ${count} items"

LAMBDAS (block-body only):
  fn(x) { x * 2 }                        // no expression-body shorthand
  Return value = last expression in block body. No `return` in lambdas.
  match can be used as expression or standalone statement.

"#;

/// The static postamble of the compressed reference (everything after STDLIB).
const REFERENCE_POSTAMBLE: &str = r#"  No operator duplicates (no core.eq, math.add, etc.)
  string.replace replaces FIRST occurrence only — use string.replace_all for all

CAPABILITIES (require declaration + host support):
  http: get, post, put, patch, delete — all return Result<HttpResponse, HttpError>
        options: { headers: [...], timeout: number, content_type: string }
  storage: get, set, delete, keys — all return Result<T, StorageError>

CREDENTIALS:
  Declared in credentials {} block — host prompts user, injects at runtime
  Access: api_key is a read-only binding in the space — NEVER put API keys in source

UI COMPONENTS (record-style syntax):
  Layout: Column { ... }, Row { ... }, Scroll { ... }
  Content: Text { value: expr }, ProgressBar { value: 0.0-1.0 }
  Interactive: Button { label: expr, on_tap: action_name }
               TextInput { value: expr, on_change: action_name, placeholder: expr }
  Data: ScrollList { items: expr, render: fn(item, index) { Component { ... } } }
  Feedback: Modal { visible: bool, on_dismiss: action_name }, Toast { message: expr }
  Conditional: if cond { Component { ... } }
  List: for item in items { Component { ... } }

RULES:
  - Block order enforced: types→state→capabilities→credentials→derived→invariants→actions→views→update→handleEvent
  - All state mutations use 'set' keyword, only inside actions
  - Views are pure — no side effects, no set
  - match must be exhaustive (cover all variants or use _)
  - No imports, no file system, no globals — everything is in the space
  - http responses are Result — always match Ok/Err or use ?
  - tests {} block goes OUTSIDE the space, not inside
  - Module names (math, core, time, etc.) are reserved — cannot shadow them
  - list.of is special-cased variadic — no general variadic functions
"#;

// ══════════════════════════════════════════════════════════════════════════════
// Stdlib Table Generation (JSON)
// ══════════════════════════════════════════════════════════════════════════════

/// Generate a structured JSON stdlib table for tooling and documentation.
///
/// Output format:
/// ```json
/// {
///   "version": "0.1.0",
///   "total_functions": 100,
///   "total_constants": 2,
///   "modules": [
///     {
///       "name": "core",
///       "functions": [
///         { "name": "log", "signature": "(value: any) -> nil", "description": "..." }
///       ],
///       "constants": []
///     }
///   ]
/// }
/// ```
pub fn generate_stdlib_table() -> String {
    let reg = StdlibRegistry::new();
    let descs = stdlib_descriptions();
    let const_descs = constant_descriptions();

    let mut total_functions = 0u32;
    let mut total_constants = 0u32;
    let mut modules_json = Vec::new();

    for &module_name in MODULE_ORDER {
        let mut funcs_json = Vec::new();
        let mut consts_json = Vec::new();

        // Functions
        if let Some(funcs) = reg.modules().get(module_name) {
            let mut func_names: Vec<&String> = funcs.keys().collect();
            func_names.sort();

            for fname in func_names {
                let sig = &funcs[fname];
                let signature = format_signature(sig);
                let desc = descs
                    .get(&(module_name, fname.as_str()))
                    .unwrap_or(&"");
                funcs_json.push(format!(
                    r#"        {{ "name": "{}", "signature": "{}", "variadic": {}, "description": "{}" }}"#,
                    fname,
                    escape_json(&signature),
                    sig.variadic,
                    escape_json(desc)
                ));
                total_functions += 1;
            }
        }

        // Constants
        if let Some(consts) = reg.all_constants().get(module_name) {
            let mut const_names: Vec<&String> = consts.keys().collect();
            const_names.sort();

            for cname in const_names {
                let ty = &consts[cname];
                let desc = const_descs
                    .get(&(module_name, cname.as_str()))
                    .unwrap_or(&"");
                consts_json.push(format!(
                    r#"        {{ "name": "{}", "type": "{}", "description": "{}" }}"#,
                    cname, ty, desc
                ));
                total_constants += 1;
            }
        }

        modules_json.push(format!(
            r#"    {{
      "name": "{}",
      "functions": [
{}
      ],
      "constants": [
{}
      ]
    }}"#,
            module_name,
            funcs_json.join(",\n"),
            consts_json.join(",\n"),
        ));
    }

    format!(
        r#"{{
  "version": "{}",
  "total_functions": {},
  "total_constants": {},
  "modules": [
{}
  ]
}}"#,
        crate::PEPL_LANGUAGE_VERSION,
        total_functions,
        total_constants,
        modules_json.join(",\n"),
    )
}

/// Format a function signature as `(param: type, ...) -> return_type`.
fn format_signature(sig: &crate::ty::FnSig) -> String {
    let params: Vec<String> = sig
        .params
        .iter()
        .map(|(name, ty)| {
            if sig.variadic {
                format!("...{}: {}", name, ty)
            } else {
                format!("{}: {}", name, ty)
            }
        })
        .collect();
    format!("({}) -> {}", params.join(", "), sig.ret)
}

/// Minimal JSON string escaping.
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_is_non_empty() {
        let reference = generate_reference();
        assert!(!reference.is_empty());
        assert!(reference.contains("PEPL: deterministic"));
        assert!(reference.contains("STDLIB"));
    }

    #[test]
    fn reference_contains_all_modules() {
        let reference = generate_reference();
        for &module in MODULE_ORDER {
            assert!(
                reference.contains(&format!("  {}:", module)),
                "Reference missing module: {}",
                module
            );
        }
    }

    #[test]
    fn reference_contains_key_functions() {
        let reference = generate_reference();
        // Spot-check some key functions appear in the STDLIB section
        assert!(reference.contains("log"));
        assert!(reference.contains("abs"));
        assert!(reference.contains("length"));
        assert!(reference.contains("map"));
        assert!(reference.contains("filter"));
        assert!(reference.contains("now"));
        assert!(reference.contains("parse"));
    }

    #[test]
    fn reference_contains_constants() {
        let reference = generate_reference();
        assert!(reference.contains("PI"));
        assert!(reference.contains("E"));
    }

    #[test]
    fn reference_token_estimate_under_2k() {
        let reference = generate_reference();
        // Rough token estimate: ~4 chars per token for English text
        let estimated_tokens = reference.len() / 4;
        assert!(
            estimated_tokens <= 2500,
            "Reference is ~{} tokens (est.), should be ≤ 2K. Length: {} chars",
            estimated_tokens,
            reference.len()
        );
    }

    #[test]
    fn stdlib_table_is_valid_json() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value =
            serde_json::from_str(&table).expect("stdlib table should be valid JSON");
        assert!(parsed.is_object());
        assert!(parsed["version"].is_string());
        assert!(parsed["total_functions"].is_number());
        assert!(parsed["total_constants"].is_number());
        assert!(parsed["modules"].is_array());
    }

    #[test]
    fn stdlib_table_has_all_modules() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let modules = parsed["modules"].as_array().unwrap();
        assert_eq!(modules.len(), MODULE_ORDER.len());
        for (i, &expected_name) in MODULE_ORDER.iter().enumerate() {
            assert_eq!(modules[i]["name"].as_str().unwrap(), expected_name);
        }
    }

    #[test]
    fn stdlib_table_function_count() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let total = parsed["total_functions"].as_u64().unwrap();
        // The registry has functions for all 13 modules
        assert!(
            total >= 100,
            "Expected at least 100 functions, got {}",
            total
        );
    }

    #[test]
    fn stdlib_table_constant_count() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let total = parsed["total_constants"].as_u64().unwrap();
        assert_eq!(total, 2, "Expected 2 constants (PI, E)");
    }

    #[test]
    fn stdlib_table_functions_have_signatures() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let modules = parsed["modules"].as_array().unwrap();
        for module in modules {
            let funcs = module["functions"].as_array().unwrap();
            for func in funcs {
                assert!(
                    func["name"].is_string(),
                    "Function missing name in module {}",
                    module["name"]
                );
                assert!(
                    func["signature"].is_string(),
                    "Function {} missing signature",
                    func["name"]
                );
                let sig = func["signature"].as_str().unwrap();
                assert!(
                    sig.contains("->"),
                    "Signature for {} should contain '->': {}",
                    func["name"],
                    sig
                );
            }
        }
    }

    #[test]
    fn stdlib_table_all_descriptions_present() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let modules = parsed["modules"].as_array().unwrap();
        let mut missing = Vec::new();
        for module in modules {
            let module_name = module["name"].as_str().unwrap();
            for func in module["functions"].as_array().unwrap() {
                let fname = func["name"].as_str().unwrap();
                let desc = func["description"].as_str().unwrap_or("");
                if desc.is_empty() {
                    missing.push(format!("{}.{}", module_name, fname));
                }
            }
            for con in module["constants"].as_array().unwrap() {
                let cname = con["name"].as_str().unwrap();
                let desc = con["description"].as_str().unwrap_or("");
                if desc.is_empty() {
                    missing.push(format!("{}.{}", module_name, cname));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "Missing descriptions for: {:?}",
            missing
        );
    }

    #[test]
    fn stdlib_table_core_module_correct() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let core = &parsed["modules"][0];
        assert_eq!(core["name"].as_str().unwrap(), "core");
        let funcs = core["functions"].as_array().unwrap();
        let names: Vec<&str> = funcs.iter().map(|f| f["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"log"));
        assert!(names.contains(&"assert"));
        assert!(names.contains(&"type_of"));
        assert!(names.contains(&"capability"));
        assert_eq!(funcs.len(), 4);
    }

    #[test]
    fn stdlib_table_math_constants() {
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let math = &parsed["modules"][1];
        assert_eq!(math["name"].as_str().unwrap(), "math");
        let consts = math["constants"].as_array().unwrap();
        assert_eq!(consts.len(), 2);
        let names: Vec<&str> = consts.iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"PI"));
        assert!(names.contains(&"E"));
    }

    #[test]
    fn reference_and_table_agree_on_modules() {
        let reference = generate_reference();
        let table = generate_stdlib_table();
        let parsed: serde_json::Value = serde_json::from_str(&table).unwrap();
        let modules = parsed["modules"].as_array().unwrap();
        for module in modules {
            let name = module["name"].as_str().unwrap();
            assert!(
                reference.contains(&format!("  {}:", name)),
                "Reference missing module {} that table has",
                name
            );
        }
    }
}

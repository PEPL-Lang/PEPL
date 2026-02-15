//! Integration tests for the PEPL WASM code generator.
//!
//! Tests validate:
//! - Minimal programs compile to valid WASM
//! - Module structure (imports, exports, memory, globals)
//! - Expression compilation (arithmetic, strings, booleans, lists, records)
//! - Statement compilation (set, let, if, for, match, assert)
//! - Action dispatch codegen
//! - View rendering codegen
//! - Deterministic output (same input → same bytes)
//! - Canonical examples compile successfully

use pepl_codegen::{compile, CodegenError};
use pepl_lexer::Lexer;
use pepl_parser::Parser;
use pepl_types::SourceFile;
use wasmparser::{ExternalKind, Parser as WasmParser, Payload};

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

/// Parse PEPL source into a Program AST (panics on parse errors).
fn parse(source: &str) -> pepl_types::ast::Program {
    let sf = SourceFile::new("test.pepl", source);
    let lex = Lexer::new(&sf).lex();
    let result = Parser::new(lex.tokens, &sf).parse();
    if result.errors.has_errors() {
        panic!(
            "parse errors:\n{}",
            result
                .errors
                .errors
                .iter()
                .map(|e| format!("  [{}] {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    result.program.expect("no program after successful parse")
}

/// Compile PEPL source to WASM bytes (panics on error).
fn compile_source(source: &str) -> Vec<u8> {
    let prog = parse(source);
    compile(&prog).unwrap_or_else(|e| panic!("codegen failed: {e}"))
}

/// Compile and return Result for error-testing.
fn try_compile(source: &str) -> Result<Vec<u8>, CodegenError> {
    let prog = parse(source);
    compile(&prog)
}

/// Minimal valid PEPL space.
const MINIMAL_SPACE: &str = r#"
space Minimal {
  state { x: number = 0 }
  view main() -> Surface { Column { } { } }
}
"#;

/// Counter space (canonical example).
const COUNTER_SPACE: &str = r#"
space Counter {
  state {
    count: number = 0
    label: string = "Counter"
  }

  action increment() {
    set count = count + 1
  }

  action decrement() {
    set count = count - 1
  }

  action reset() {
    set count = 0
  }

  view main() -> Surface {
    Column { } {
      Text { value: label }
      Text { value: count }
    }
  }
}
"#;

/// Extract exports from WASM bytes.
fn get_exports(wasm: &[u8]) -> Vec<(String, ExternalKind)> {
    let mut exports = Vec::new();
    for payload in WasmParser::new(0).parse_all(wasm) {
        if let Ok(Payload::ExportSection(reader)) = payload {
            for export in reader {
                let exp = export.expect("valid export");
                exports.push((exp.name.to_string(), exp.kind));
            }
        }
    }
    exports
}

/// Extract import count from WASM bytes.
fn get_import_count(wasm: &[u8]) -> usize {
    let mut count = 0;
    for payload in WasmParser::new(0).parse_all(wasm) {
        if let Ok(Payload::ImportSection(reader)) = payload {
            for import in reader {
                if import.is_ok() {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Check whether a custom section with the given name exists.
fn has_custom_section(wasm: &[u8], name: &str) -> bool {
    for payload in WasmParser::new(0).parse_all(wasm) {
        if let Ok(Payload::CustomSection(reader)) = payload {
            if reader.name() == name {
                return true;
            }
        }
    }
    false
}

/// Get custom section data.
fn get_custom_section_data(wasm: &[u8], name: &str) -> Option<Vec<u8>> {
    for payload in WasmParser::new(0).parse_all(wasm) {
        if let Ok(Payload::CustomSection(reader)) = payload {
            if reader.name() == name {
                return Some(reader.data().to_vec());
            }
        }
    }
    None
}

/// Check if WASM bytes are valid per wasmparser.
fn is_valid_wasm(wasm: &[u8]) -> bool {
    wasmparser::validate(wasm).is_ok()
}

// ══════════════════════════════════════════════════════════════════════════════
// Basic Module Structure
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn minimal_space_compiles() {
    let wasm = compile_source(MINIMAL_SPACE);
    assert!(!wasm.is_empty());
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn minimal_space_starts_with_wasm_magic() {
    let wasm = compile_source(MINIMAL_SPACE);
    // WASM magic: \0asm
    assert_eq!(&wasm[0..4], b"\0asm");
    // WASM version 1
    assert_eq!(&wasm[4..8], &[1, 0, 0, 0]);
}

#[test]
fn minimal_space_has_required_exports() {
    let wasm = compile_source(MINIMAL_SPACE);
    let exports = get_exports(&wasm);
    let export_names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();

    assert!(export_names.contains(&"init"), "missing init export");
    assert!(
        export_names.contains(&"dispatch_action"),
        "missing dispatch_action export"
    );
    assert!(export_names.contains(&"render"), "missing render export");
    assert!(
        export_names.contains(&"get_state"),
        "missing get_state export"
    );
    assert!(export_names.contains(&"dealloc"), "missing dealloc export");
    assert!(export_names.contains(&"alloc"), "missing alloc export");
    assert!(export_names.contains(&"memory"), "missing memory export");
}

#[test]
fn minimal_space_memory_export_is_memory_kind() {
    let wasm = compile_source(MINIMAL_SPACE);
    let exports = get_exports(&wasm);
    let mem = exports.iter().find(|(n, _)| n == "memory").unwrap();
    assert_eq!(mem.1, ExternalKind::Memory);
}

#[test]
fn minimal_space_function_exports_are_func_kind() {
    let wasm = compile_source(MINIMAL_SPACE);
    let exports = get_exports(&wasm);
    for name in &["init", "dispatch_action", "render", "get_state", "dealloc", "alloc"] {
        let exp = exports.iter().find(|(n, _)| n == name).unwrap();
        assert_eq!(
            exp.1,
            ExternalKind::Func,
            "{name} should be function export"
        );
    }
}

#[test]
fn minimal_space_has_four_imports() {
    let wasm = compile_source(MINIMAL_SPACE);
    assert_eq!(get_import_count(&wasm), 4); // host_call, log, trap, get_timestamp
}

#[test]
fn minimal_space_has_pepl_custom_section() {
    let wasm = compile_source(MINIMAL_SPACE);
    assert!(has_custom_section(&wasm, "pepl"));
}

#[test]
fn custom_section_contains_version() {
    let wasm = compile_source(MINIMAL_SPACE);
    let data = get_custom_section_data(&wasm, "pepl").unwrap();
    let version = std::str::from_utf8(&data).unwrap();
    assert_eq!(version, "0.1.0");
}

// ══════════════════════════════════════════════════════════════════════════════
// Determinism
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn deterministic_output_same_input() {
    let wasm1 = compile_source(MINIMAL_SPACE);
    let wasm2 = compile_source(MINIMAL_SPACE);
    assert_eq!(wasm1, wasm2, "same input must produce identical bytes");
}

#[test]
fn deterministic_output_100_iterations() {
    let reference = compile_source(COUNTER_SPACE);
    for i in 0..100 {
        let wasm = compile_source(COUNTER_SPACE);
        assert_eq!(
            wasm, reference,
            "iteration {i} produced different bytes"
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Counter Example
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn counter_compiles() {
    let wasm = compile_source(COUNTER_SPACE);
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn counter_no_update_or_handle_event_exports() {
    let wasm = compile_source(COUNTER_SPACE);
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
    assert!(!names.contains(&"update"));
    assert!(!names.contains(&"handle_event"));
}

// ══════════════════════════════════════════════════════════════════════════════
// Expressions
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn number_literal_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 42 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn string_literal_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: string = "hello world" }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn bool_literal_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: bool = true }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn nil_literal_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action check() {
    let y = nil
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn arithmetic_expressions_compile() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action compute() {
    set x = (1 + 2) * 3 - 4
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn comparison_expressions_compile() {
    let wasm = compile_source(
        r#"
space T {
  state { x: bool = false }
  action compute() {
    set x = 1 < 2
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn boolean_logic_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: bool = false }
  action compute() {
    set x = true and false or true
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn negation_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action compute() {
    set x = -x
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn not_operator_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { flag: bool = false }
  action toggle() {
    set flag = not flag
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn list_literal_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { items: list<number> = [] }
  action add() {
    set items = [1, 2, 3]
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn record_literal_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action check() {
    let point = { x: 1, y: 2 }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn if_expression_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action check() {
    set x = if x > 0 { x } else { 0 }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn string_concat_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { name: string = "world" }
  action check() {
    let greeting = "hello " + name
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Statements
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn set_statement_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action update_x() {
    set x = 10
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn let_binding_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action compute() {
    let y = 42
    set x = y
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn if_statement_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action check() {
    if x > 10 {
      set x = 0
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn if_else_statement_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action check() {
    if x > 10 {
      set x = 0
    } else {
      set x = x + 1
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn for_loop_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { total: number = 0 }
  action sum_list() {
    let items = [1, 2, 3]
    for item in items {
      set total = total + item
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn return_statement_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action check() {
    if x > 100 {
      return
    }
    set x = x + 1
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn assert_statement_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action verify() {
    assert x >= 0
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Multiple Actions
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_actions_compile() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action inc() { set x = x + 1 }
  action dec() { set x = x - 1 }
  action reset() { set x = 0 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn action_with_params_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action add(amount: number) {
    set x = x + amount
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Views
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn view_with_text_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { msg: string = "hello" }
  view main() -> Surface {
    Text { value: msg }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn view_with_nested_components_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  view main() -> Surface {
    Column { } {
      Text { value: "Title" }
      Row { } {
        Text { value: "Count" }
        Text { value: x }
      }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn view_with_conditional_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { show: bool = true }
  view main() -> Surface {
    Column { } {
      if show {
        Text { value: "visible" }
      }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn view_with_for_loop_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { items: list<number> = [1, 2, 3] }
  view main() -> Surface {
    Column { } {
      for item in items {
        Text { value: item }
      }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn multiple_views_compile() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  view main() -> Surface {
    Text { value: x }
  }
  view detail() -> Surface {
    Column { } {
      Text { value: "Detail" }
      Text { value: x }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Invariants
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn invariant_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  invariant non_negative { x >= 0 }
  action inc() { set x = x + 1 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn multiple_invariants_compile() {
    let wasm = compile_source(
        r#"
space T {
  state {
    x: number = 0
    y: number = 0
  }
  invariant non_neg_x { x >= 0 }
  invariant non_neg_y { y >= 0 }
  action bump() { set x = 1 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Derived Fields
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn derived_field_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    x: number = 1
    y: number = 2
  }
  derived { sum: number = x + y }
  action inc() { set x = x + 1 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Update & HandleEvent
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn update_compiles_and_exports() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action bump() { set x = 1 }
  view main() -> Surface { Column { } { } }
  update(dt: number) {
    set x = x + dt
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"update"), "update should be exported");
}

#[test]
fn handle_event_compiles_and_exports() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action bump() { set x = 1 }
  view main() -> Surface { Column { } { } }
  handleEvent(event: InputEvent) {
    set x = x + 1
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"handle_event"),
        "handle_event should be exported"
    );
}

#[test]
fn update_and_handle_event_together() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action bump() { set x = 1 }
  view main() -> Surface { Column { } { } }
  update(dt: number) {
    set x = x + dt
  }
  handleEvent(event: InputEvent) {
    set x = x + 1
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"update"));
    assert!(names.contains(&"handle_event"));
}

// ══════════════════════════════════════════════════════════════════════════════
// State Fields
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_state_fields_compile() {
    let wasm = compile_source(
        r#"
space T {
  state {
    count: number = 0
    name: string = "hello"
    active: bool = true
    items: list<number> = []
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn state_used_in_action_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    x: number = 0
    y: number = 0
  }
  action swap() {
    let tmp = x
    set x = y
    set y = tmp
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Module Size
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn minimal_module_reasonable_size() {
    let wasm = compile_source(MINIMAL_SPACE);
    // A minimal module should be under 10KB
    assert!(
        wasm.len() < 10_000,
        "minimal module too large: {} bytes",
        wasm.len()
    );
}

#[test]
fn counter_module_reasonable_size() {
    let wasm = compile_source(COUNTER_SPACE);
    // Counter with 3 actions should be under 20KB
    assert!(
        wasm.len() < 20_000,
        "counter module too large: {} bytes",
        wasm.len()
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Examples
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn todo_list_compiles() {
    let wasm = compile_source(
        r#"
space TodoList {
  state {
    items: list<{ id: number, text: string, done: bool }> = []
    next_id: number = 1
  }

  action add_item(text: string) {
    let item = { id: next_id, text: text, done: false }
    set items = items + [item]
    set next_id = next_id + 1
  }

  action clear() {
    set items = []
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Todo List" }
      for item in items {
        Row { } {
          Text { value: item }
        }
      }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn unit_converter_compiles() {
    let wasm = compile_source(
        r#"
space UnitConverter {
  state {
    celsius: number = 0
    fahrenheit: number = 32
  }

  derived {
    computed_f: number = celsius * 9 / 5 + 32
    computed_c: number = (fahrenheit - 32) * 5 / 9
  }

  action set_celsius(value: number) {
    set celsius = value
  }

  action set_fahrenheit(value: number) {
    set fahrenheit = value
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Unit Converter" }
      Text { value: celsius }
      Text { value: fahrenheit }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Types
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn sum_type_declaration_compiles() {
    let wasm = compile_source(
        r#"
space T {
  type Status = | Active | Inactive | Pending
  state { status: Status = Active }
  action activate() {
    set status = Active
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Complex Scenarios
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn nested_if_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action categorize() {
    if x > 100 {
      set x = 100
    } else {
      if x < 0 {
        set x = 0
      }
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn action_with_multiple_stmts_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    x: number = 0
    y: number = 0
  }
  action complex(a: number, b: number) {
    let sum = a + b
    set x = sum
    set y = a * b
    if sum > 100 {
      set x = 100
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn empty_action_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action noop() { }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn view_with_button_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { count: number = 0 }
  action inc() { set count = count + 1 }
  view main() -> Surface {
    Column { } {
      Text { value: count }
      Button { label: "Click", onPress: inc }
    }
  }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// WASM Section Ordering
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn sections_in_correct_order() {
    let wasm = compile_source(MINIMAL_SPACE);
    let mut section_ids: Vec<u8> = Vec::new();

    for payload in WasmParser::new(0).parse_all(&wasm) {
        match payload {
            Ok(Payload::TypeSection(_)) => section_ids.push(1),
            Ok(Payload::ImportSection(_)) => section_ids.push(2),
            Ok(Payload::FunctionSection(_)) => section_ids.push(3),
            Ok(Payload::MemorySection(_)) => section_ids.push(5),
            Ok(Payload::GlobalSection(_)) => section_ids.push(6),
            Ok(Payload::ExportSection(_)) => section_ids.push(7),
            Ok(Payload::CodeSectionStart { .. }) => section_ids.push(10),
            Ok(Payload::DataSection(_)) => section_ids.push(11),
            _ => {}
        }
    }

    // Verify ordering: each section must come after the previous
    for window in section_ids.windows(2) {
        assert!(
            window[0] <= window[1],
            "section {} comes after section {} — invalid order",
            window[0],
            window[1]
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Edge Cases
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn single_state_field_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn many_state_fields_compile() {
    let wasm = compile_source(
        r#"
space T {
  state {
    a: number = 0
    b: number = 1
    c: number = 2
    d: string = "hello"
    e: bool = true
    f: list<number> = []
    g: number = 99
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn many_actions_compile() {
    let wasm = compile_source(
        r#"
space T {
  state { x: number = 0 }
  action a1() { set x = 1 }
  action a2() { set x = 2 }
  action a3() { set x = 3 }
  action a4() { set x = 4 }
  action a5() { set x = 5 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

// ══════════════════════════════════════════════════════════════════════════════
// Error type check (compile returns proper error type)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn compile_returns_codegen_result() {
    let result = try_compile(MINIMAL_SPACE);
    assert!(result.is_ok());
}

#[test]
fn compile_counter_returns_ok() {
    let result = try_compile(COUNTER_SPACE);
    assert!(result.is_ok());
}

// ══════════════════════════════════════════════════════════════════════════════
// M5 Parity: Lambda / Record Spread / Result Unwrap
// ══════════════════════════════════════════════════════════════════════════════
// These tests verify the Phase 9 codegen fixes produce valid WASM for
// features that previously emitted placeholder/incorrect code (F2, F3, F4).

#[test]
fn lambda_in_derived_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    items: list<number> = [1, 2, 3]
  }
  derived {
    positive: number = list.length(list.filter(items, fn(x: number) { x > 0 }))
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn lambda_with_capture_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    items: list<number> = [1, 2, 3]
    threshold: number = 5
  }
  derived {
    above: number = list.length(list.filter(items, fn(x: number) { x > threshold }))
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn lambda_multi_param_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    nums: list<number> = [3, 1, 2]
  }
  derived {
    total: number = list.reduce(nums, 0, fn(acc: number, x: number) { acc + x })
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn record_spread_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    item: { name: string, done: bool, count: number } = { name: "a", done: false, count: 0 }
  }
  action toggle() {
    set item = { ...item, done: not item.done }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn record_spread_with_multiple_overrides_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state {
    item: { a: number, b: number, c: number } = { a: 1, b: 2, c: 3 }
  }
  action change() {
    set item = { ...item, a: 10, c: 30 }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn result_unwrap_compiles() {
    let wasm = compile_source(
        r#"
space T {
  state { value: number = 0 }
  action do_parse(s: string) {
    let n = convert.parse_float(s)?
    set value = n
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn combined_lambda_spread_compiles() {
    // Exercises lambda + spread together (like TodoList canonical)
    let wasm = compile_source(
        r#"
space T {
  state {
    todos: list<{ text: string, done: bool }> = []
    input: string = ""
  }
  derived {
    remaining: number = list.length(list.filter(todos, fn(t: { text: string, done: bool }) { not t.done }))
  }
  action add() {
    set todos = list.append(todos, { text: input, done: false })
    set input = ""
  }
  action toggle(i: number) {
    let todo = list.get(todos, i)
    if todo != nil {
      set todos = list.set(todos, i, { ...todo, done: not todo.done })
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn lambda_spread_unwrap_deterministic() {
    // 100-iteration determinism for lambda/spread/unwrap features
    let source = r#"
space T {
  state {
    items: list<{ name: string, val: number }> = []
    parsed: number = 0
  }
  derived {
    total: number = list.reduce(items, 0, fn(acc: number, x: { name: string, val: number }) { acc + x.val })
  }
  action add(name: string) {
    set items = list.append(items, { name: name, val: 1 })
  }
  action modify(i: number) {
    let item = list.get(items, i)
    if item != nil {
      set items = list.set(items, i, { ...item, val: item.val + 1 })
    }
  }
  action do_parse(s: string) {
    let n = convert.parse_float(s)?
    set parsed = n
  }
  view main() -> Surface { Column { } { } }
}
"#;
    let reference = compile_source(source);
    for i in 0..100 {
        let wasm = compile_source(source);
        assert_eq!(
            wasm, reference,
            "lambda/spread/unwrap WASM bytes differ at iteration {}",
            i
        );
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Nested set — 3+ level depth
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn nested_set_3_level_compiles() {
    // 3-level nested set: set settings.theme.shade = "red"
    // Note: avoid `color` as a field name — it's the KwColor keyword.
    let source = r#"
space Settings {
  state {
    settings: { theme: { shade: string, size: number }, lang: string } = { theme: { shade: "blue", size: 12 }, lang: "en" }
  }
  action change_shade() {
    set settings.theme.shade = "red"
  }
  view main() -> Surface { Column { } { } }
}
"#;
    let wasm = compile_source(source);
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn nested_set_2_level_still_compiles() {
    // Ensure the 2-level case still works after refactor
    let source = r#"
space Config {
  state {
    prefs: { volume: number, muted: bool } = { volume: 50, muted: false }
  }
  action mute() {
    set prefs.muted = true
  }
  view main() -> Surface { Column { } { } }
}
"#;
    let wasm = compile_source(source);
    assert!(is_valid_wasm(&wasm));
}

#[test]
fn nested_set_3_level_deterministic() {
    let source = r#"
space DeepNest {
  state {
    data: { inner: { value: number, label: string }, count: number } = { inner: { value: 0, label: "x" }, count: 10 }
  }
  action update_value() {
    set data.inner.value = 42
  }
  view main() -> Surface { Column { } { } }
}
"#;
    let reference = compile_source(source);
    for i in 0..100 {
        let wasm = compile_source(source);
        assert_eq!(
            wasm, reference,
            "3-level nested set WASM bytes differ at iteration {}",
            i
        );
    }
}

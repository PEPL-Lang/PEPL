//! Integration tests for PEPL test block → WASM compilation.
//!
//! Validates that PEPL programs with `tests { }` blocks compile to valid WASM
//! modules that export `__test_N()` and `__test_count()` functions, and that
//! these functions execute correctly via wasmi.

use pepl_codegen::{compile, compile_with_source_map};
use pepl_codegen::source_map::FuncKind;
use pepl_lexer::Lexer;
use pepl_parser::Parser;
use pepl_types::SourceFile;
use wasmi::{Engine, Linker, Module, Store};
use wasmparser::{ExternalKind, Payload};

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

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

fn compile_source(source: &str) -> Vec<u8> {
    let prog = parse(source);
    compile(&prog).unwrap_or_else(|e| panic!("codegen failed: {e}"))
}

fn get_exports(wasm: &[u8]) -> Vec<(String, ExternalKind)> {
    let parser = wasmparser::Parser::new(0);
    let mut exports = Vec::new();
    for payload in parser.parse_all(wasm) {
        if let Ok(Payload::ExportSection(reader)) = payload {
            for export in reader {
                let exp = export.unwrap();
                exports.push((exp.name.to_string(), exp.kind));
            }
        }
    }
    exports
}

/// Instantiate a WASM module via wasmi and return the store + instance.
fn instantiate(wasm: &[u8]) -> (wasmi::Store<()>, wasmi::Instance) {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm).expect("failed to parse wasm module");
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::<()>::new(&engine);

    // Stub host imports
    linker
        .func_wrap(
            "env",
            "host_call",
            |_: wasmi::Caller<'_, ()>, _: i32, _: i32, _: i32| -> i32 { 0 },
        )
        .unwrap();
    linker
        .func_wrap(
            "env",
            "log",
            |_: wasmi::Caller<'_, ()>, _: i32, _: i32| {},
        )
        .unwrap();
    linker
        .func_wrap(
            "env",
            "trap",
            |_: wasmi::Caller<'_, ()>, _ptr: i32, _len: i32| -> () {
                panic!("WASM trap triggered");
            },
        )
        .unwrap();
    linker
        .func_wrap(
            "env",
            "get_timestamp",
            |_: wasmi::Caller<'_, ()>| -> i64 { 0 },
        )
        .unwrap();

    let instance = linker
        .instantiate(&mut store, &module)
        .expect("failed to instantiate")
        .start(&mut store)
        .expect("failed to start instance");
    (store, instance)
}

// ══════════════════════════════════════════════════════════════════════════════
// Test sources
// ══════════════════════════════════════════════════════════════════════════════

/// Counter space with simple tests.
const COUNTER_WITH_TESTS: &str = r#"
space Counter {
  state {
    count: number = 0
  }

  action increment() {
    set count = count + 1
  }

  action decrement() {
    set count = count - 1
  }

  view main() -> Surface { Column { } { } }
}

tests {
  test "count starts at zero" {
    assert count == 0
  }

  test "increment increases count" {
    increment()
    assert count == 1
  }

  test "decrement decreases count" {
    increment()
    increment()
    decrement()
    assert count == 1
  }
}
"#;

/// Space with assert message.
const ASSERT_WITH_MESSAGE: &str = r#"
space Demo {
  state {
    x: number = 5
  }

  view main() -> Surface { Column { } { } }
}

tests {
  test "x has expected value" {
    assert x == 5, "x should be 5"
  }
}
"#;

/// Space with multiple test blocks.
const MULTI_BLOCK: &str = r#"
space Multi {
  state {
    a: number = 1
    b: number = 2
  }

  action swap() {
    let tmp = a
    set a = b
    set b = tmp
  }

  view main() -> Surface { Column { } { } }
}

tests {
  test "initial values" {
    assert a == 1
    assert b == 2
  }
}

tests {
  test "swap works" {
    swap()
    assert a == 2
    assert b == 1
  }
}
"#;

/// Space with no tests — no __test exports expected.
const NO_TESTS: &str = r#"
space Empty {
  state { x: number = 0 }
  view main() -> Surface { Column { } { } }
}
"#;

// ══════════════════════════════════════════════════════════════════════════════
// Tests — Export structure
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_exports_present_when_tests_exist() {
    let wasm = compile_source(COUNTER_WITH_TESTS);
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();

    assert!(names.contains(&"__test_0"), "missing __test_0 export");
    assert!(names.contains(&"__test_1"), "missing __test_1 export");
    assert!(names.contains(&"__test_2"), "missing __test_2 export");
    assert!(
        names.contains(&"__test_count"),
        "missing __test_count export"
    );
    // Should NOT have __test_3
    assert!(
        !names.contains(&"__test_3"),
        "__test_3 should not exist for 3 test cases"
    );
}

#[test]
fn test_no_test_exports_without_tests() {
    let wasm = compile_source(NO_TESTS);
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();

    assert!(
        !names.contains(&"__test_0"),
        "__test_0 should not exist without tests"
    );
    assert!(
        !names.contains(&"__test_count"),
        "__test_count should not exist without tests"
    );
}

#[test]
fn test_multi_block_flattened() {
    let wasm = compile_source(MULTI_BLOCK);
    let exports = get_exports(&wasm);
    let names: Vec<&str> = exports.iter().map(|(n, _)| n.as_str()).collect();

    // 2 test cases total (1 in first block, 1 in second)
    assert!(names.contains(&"__test_0"));
    assert!(names.contains(&"__test_1"));
    assert!(names.contains(&"__test_count"));
    assert!(!names.contains(&"__test_2"));
}

#[test]
fn test_exports_are_functions() {
    let wasm = compile_source(COUNTER_WITH_TESTS);
    let exports = get_exports(&wasm);

    for (name, kind) in &exports {
        if name.starts_with("__test") {
            assert_eq!(
                *kind,
                ExternalKind::Func,
                "{name} should be a function export"
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests — Execution via wasmi
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_count_returns_correct_value() {
    let wasm = compile_source(COUNTER_WITH_TESTS);
    let (mut store, instance) = instantiate(&wasm);

    let test_count = instance
        .get_typed_func::<(), i32>(&store, "__test_count")
        .expect("__test_count export missing");
    let count = test_count.call(&mut store, ()).unwrap();
    assert_eq!(count, 3, "expected 3 test cases");
}

#[test]
fn test_passing_assertions_dont_trap() {
    let wasm = compile_source(COUNTER_WITH_TESTS);
    let (mut store, instance) = instantiate(&wasm);

    // All 3 tests should pass without trapping
    for i in 0..3 {
        let test_fn = instance
            .get_typed_func::<(), ()>(&store, &format!("__test_{i}"))
            .unwrap_or_else(|_| panic!("__test_{i} export missing"));
        test_fn
            .call(&mut store, ())
            .unwrap_or_else(|e| panic!("__test_{i} trapped: {e}"));
    }
}

#[test]
fn test_assert_with_message_passes() {
    let wasm = compile_source(ASSERT_WITH_MESSAGE);
    let (mut store, instance) = instantiate(&wasm);

    let test_fn = instance
        .get_typed_func::<(), ()>(&store, "__test_0")
        .expect("__test_0 export missing");
    test_fn
        .call(&mut store, ())
        .expect("assertion with message should pass");
}

#[test]
fn test_multi_block_execution() {
    let wasm = compile_source(MULTI_BLOCK);
    let (mut store, instance) = instantiate(&wasm);

    let count = instance
        .get_typed_func::<(), i32>(&store, "__test_count")
        .unwrap()
        .call(&mut store, ())
        .unwrap();
    assert_eq!(count, 2);

    // Both tests should pass
    for i in 0..2 {
        let test_fn = instance
            .get_typed_func::<(), ()>(&store, &format!("__test_{i}"))
            .unwrap();
        test_fn
            .call(&mut store, ())
            .unwrap_or_else(|e| panic!("__test_{i} trapped: {e}"));
    }
}

#[test]
fn test_failing_assertion_traps() {
    let source = r#"
space Fail {
  state { x: number = 0 }
  view main() -> Surface { Column { } { } }
}
tests {
  test "should fail" {
    assert x == 99
  }
}
"#;
    let wasm = compile_source(source);
    let (mut store, instance) = instantiate(&wasm);

    let test_fn = instance
        .get_typed_func::<(), ()>(&store, "__test_0")
        .expect("__test_0 export missing");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        test_fn.call(&mut store, ())
    }));
    assert!(result.is_err(), "failing assertion should trap");
}

#[test]
fn test_deterministic_compilation_with_tests() {
    let wasm1 = compile_source(COUNTER_WITH_TESTS);
    let wasm2 = compile_source(COUNTER_WITH_TESTS);
    assert_eq!(wasm1, wasm2, "same input must produce identical bytes");
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests — Source map
// ══════════════════════════════════════════════════════════════════════════════

fn compile_with_map(source: &str) -> (Vec<u8>, pepl_codegen::SourceMap) {
    let prog = parse(source);
    compile_with_source_map(&prog).unwrap_or_else(|e| panic!("codegen failed: {e}"))
}

#[test]
fn source_map_has_space_infra_entries() {
    let (_wasm, sm) = compile_with_map(NO_TESTS);
    let names: Vec<&str> = sm.entries.iter().map(|e| e.func_name.as_str()).collect();
    assert!(names.contains(&"init"), "missing init in source map");
    assert!(
        names.contains(&"dispatch_action"),
        "missing dispatch_action in source map"
    );
    assert!(names.contains(&"render"), "missing render in source map");
}

#[test]
fn source_map_has_action_entries() {
    let (_wasm, sm) = compile_with_map(COUNTER_WITH_TESTS);
    let action_entries: Vec<_> = sm
        .entries
        .iter()
        .filter(|e| e.kind == FuncKind::Action)
        .collect();
    let action_names: Vec<&str> = action_entries.iter().map(|e| e.func_name.as_str()).collect();
    assert!(action_names.contains(&"increment"), "missing increment action");
    assert!(action_names.contains(&"decrement"), "missing decrement action");
}

#[test]
fn source_map_has_test_entries() {
    let (_wasm, sm) = compile_with_map(COUNTER_WITH_TESTS);
    let test_entries: Vec<_> = sm
        .entries
        .iter()
        .filter(|e| e.kind == FuncKind::Test)
        .collect();
    assert_eq!(test_entries.len(), 3, "expected 3 test entries");

    let count_entry = sm.entries.iter().find(|e| e.kind == FuncKind::TestCount);
    assert!(count_entry.is_some(), "missing __test_count entry");
}

#[test]
fn source_map_spans_are_nonzero() {
    let (_wasm, sm) = compile_with_map(COUNTER_WITH_TESTS);
    for entry in &sm.entries {
        assert!(
            entry.span.start_line > 0,
            "{} has zero start_line",
            entry.func_name
        );
    }
}

#[test]
fn source_map_embedded_in_wasm_custom_section() {
    let (wasm, sm) = compile_with_map(COUNTER_WITH_TESTS);

    // Find the pepl_source_map custom section
    let parser = wasmparser::Parser::new(0);
    let mut found = false;
    for payload in parser.parse_all(&wasm) {
        if let Ok(Payload::CustomSection(reader)) = payload {
            if reader.name() == "pepl_source_map" {
                let embedded =
                    pepl_codegen::SourceMap::from_json(reader.data()).expect("invalid JSON");
                assert_eq!(embedded.entries.len(), sm.entries.len());
                found = true;
            }
        }
    }
    assert!(found, "pepl_source_map custom section not found in WASM");
}

#[test]
fn source_map_find_by_func_index() {
    let (_wasm, sm) = compile_with_map(COUNTER_WITH_TESTS);
    let init_entry = sm.entries.iter().find(|e| e.func_name == "init").unwrap();
    let found = sm.find_by_func_index(init_entry.wasm_func_index);
    assert!(found.is_some());
    assert_eq!(found.unwrap().func_name, "init");
}

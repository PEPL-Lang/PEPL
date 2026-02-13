//! Phase 11.2 — Determinism & Parity Infrastructure
//!
//! Two harnesses:
//! 1. **Determinism proof**: compile space → WASM bytes, repeat N times, all
//!    outputs byte-for-byte identical.  Then instantiate the same WASM module
//!    N times with the same inputs and compare post-init memory snapshots.
//! 2. **Eval↔codegen parity**: run the same PEPL program through the
//!    tree-walking evaluator (`pepl_eval::SpaceInstance`) and through the
//!    compiled WASM module (via `wasmi`), compare state after init and after
//!    action dispatches.

use pepl_codegen::compile;
use pepl_eval::SpaceInstance;
use pepl_lexer::Lexer;
use pepl_parser::Parser;
use pepl_stdlib::Value;
use pepl_types::SourceFile;

use std::collections::BTreeMap;

// ══════════════════════════════════════════════════════════════════════════════
// WASM value tags (mirrors pepl-codegen/src/types.rs)
// ══════════════════════════════════════════════════════════════════════════════

const TAG_NIL: i32 = 0;
const TAG_NUMBER: i32 = 1;
const TAG_BOOL: i32 = 2;
const TAG_STRING: i32 = 3;
const TAG_LIST: i32 = 4;
const TAG_RECORD: i32 = 5;
#[allow(dead_code)]
const TAG_VARIANT: i32 = 6;
#[allow(dead_code)]
const TAG_LAMBDA: i32 = 7;
#[allow(dead_code)]
const TAG_COLOR: i32 = 8;
#[allow(dead_code)]
const TAG_ACTION_REF: i32 = 9;

// ══════════════════════════════════════════════════════════════════════════════
// Helpers — parsing & compilation
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

// ══════════════════════════════════════════════════════════════════════════════
// WasmRunner — instantiate a compiled PEPL module via wasmi
// ══════════════════════════════════════════════════════════════════════════════

/// Host state held by the wasmi Store.
#[derive(Default)]
struct HostState {
    /// Log messages captured from `env.log`.
    logs: Vec<String>,
    /// Trap message if `env.trap` was called.
    trap_message: Option<String>,
    /// Simulated timestamp for `env.get_timestamp`.
    timestamp: i64,
}

struct WasmRunner {
    store: wasmi::Store<HostState>,
    instance: wasmi::Instance,
    memory: wasmi::Memory,
}

impl WasmRunner {
    /// Instantiate a compiled PEPL WASM module.
    fn new(wasm_bytes: &[u8]) -> Self {
        let engine = wasmi::Engine::default();
        let module =
            wasmi::Module::new(&engine, wasm_bytes).expect("failed to parse WASM module");

        let mut store = wasmi::Store::new(&engine, HostState::default());
        let mut linker = <wasmi::Linker<HostState>>::new(&engine);

        // env.host_call(cap_id: i32, fn_id: i32, args_ptr: i32) -> i32
        //
        // Stub: for determinism/parity tests on simple programs we return a
        // NIL value.  The caller should avoid programs that depend on specific
        // stdlib results unless they are pure-math (those are compiled inline).
        linker
            .func_wrap(
                "env",
                "host_call",
                |caller: wasmi::Caller<'_, HostState>,
                 _cap_id: i32,
                 _fn_id: i32,
                 _args_ptr: i32|
                 -> i32 {
                    // Allocate a NIL value cell in memory and return its pointer.
                    // Read the heap pointer from global 0, allocate 12 bytes.
                    // Simple stub: return 0 for NIL.
                    // Programs using stdlib calls should not depend on
                    // specific return values for parity tests.
                    let _ = caller;
                    0
                },
            )
            .expect("link host_call");

        // env.log(ptr: i32, len: i32)
        linker
            .func_wrap(
                "env",
                "log",
                |mut caller: wasmi::Caller<'_, HostState>, ptr: i32, len: i32| -> () {
                    let mem = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .expect("memory export");
                    let data = mem.data(&caller);
                    let start = ptr as usize;
                    let end = start + len as usize;
                    if end <= data.len() {
                        let msg =
                            String::from_utf8_lossy(&data[start..end]).to_string();
                        caller.data_mut().logs.push(msg);
                    }
                },
            )
            .expect("link log");

        // env.trap(ptr: i32, len: i32)
        linker
            .func_wrap(
                "env",
                "trap",
                |mut caller: wasmi::Caller<'_, HostState>, ptr: i32, len: i32| -> () {
                    let mem = caller
                        .get_export("memory")
                        .and_then(|e| e.into_memory())
                        .expect("memory export");
                    let data = mem.data(&caller);
                    let start = ptr as usize;
                    let end = start + len as usize;
                    let msg = if end <= data.len() {
                        String::from_utf8_lossy(&data[start..end]).to_string()
                    } else {
                        "<invalid trap message>".to_string()
                    };
                    caller.data_mut().trap_message = Some(msg.clone());
                    panic!("WASM trap: {msg}");
                },
            )
            .expect("link trap");

        // env.get_timestamp() -> i64
        linker
            .func_wrap(
                "env",
                "get_timestamp",
                |caller: wasmi::Caller<'_, HostState>| -> i64 {
                    caller.data().timestamp
                },
            )
            .expect("link get_timestamp");

        let instance = linker
            .instantiate(&mut store, &module)
            .expect("instantiation failed")
            .start(&mut store)
            .expect("start failed");

        let memory = instance
            .get_memory(&store, "memory")
            .expect("no memory export");

        Self {
            store,
            instance,
            memory,
        }
    }

    /// Call `init()` — initializes state with default values.
    fn init(&mut self) {
        let init_fn = self
            .instance
            .get_typed_func::<(), ()>(&self.store, "init")
            .expect("no init export");
        init_fn.call(&mut self.store, ()).expect("init() trapped");
    }

    /// Call `get_state() -> i32` — returns pointer to state record.
    fn get_state_ptr(&mut self) -> i32 {
        let get_state_fn = self
            .instance
            .get_typed_func::<(), i32>(&self.store, "get_state")
            .expect("no get_state export");
        get_state_fn
            .call(&mut self.store, ())
            .expect("get_state() trapped")
    }

    /// Snapshot the entire linear memory.
    fn memory_snapshot(&self) -> Vec<u8> {
        self.memory.data(&self.store).to_vec()
    }

    /// Read an i32 from memory at the given byte offset.
    fn read_i32(&self, offset: usize) -> i32 {
        let data = self.memory.data(&self.store);
        if offset + 4 > data.len() {
            panic!(
                "read_i32 OOB: offset={offset}, mem_len={}",
                data.len()
            );
        }
        i32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
    }

    /// Read an f64 from memory at the given byte offset (8 bytes LE).
    fn read_f64(&self, offset: usize) -> f64 {
        let data = self.memory.data(&self.store);
        if offset + 8 > data.len() {
            panic!(
                "read_f64 OOB: offset={offset}, mem_len={}",
                data.len()
            );
        }
        f64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
    }

    /// Read a UTF-8 string from memory at `ptr` with `len` bytes.
    fn read_string(&self, ptr: usize, len: usize) -> String {
        let data = self.memory.data(&self.store);
        if ptr + len > data.len() {
            panic!(
                "read_string OOB: ptr={ptr}, len={len}, mem_len={}",
                data.len()
            );
        }
        String::from_utf8_lossy(&data[ptr..ptr + len]).to_string()
    }

    /// Read a PEPL value from WASM memory at the given pointer.
    /// Returns a `pepl_stdlib::Value` for comparison with the evaluator.
    fn read_value(&self, ptr: i32) -> Value {
        let offset = ptr as usize;
        let tag = self.read_i32(offset);
        match tag {
            TAG_NIL => Value::Nil,
            TAG_NUMBER => {
                let n = self.read_f64(offset + 4);
                Value::Number(n)
            }
            TAG_BOOL => {
                let b = self.read_i32(offset + 4);
                Value::Bool(b != 0)
            }
            TAG_STRING => {
                let data_ptr = self.read_i32(offset + 4) as usize;
                let byte_len = self.read_i32(offset + 8) as usize;
                Value::String(self.read_string(data_ptr, byte_len))
            }
            TAG_LIST => {
                let arr_offset = self.read_i32(offset + 4) as usize;
                let count = self.read_i32(offset + 8) as usize;
                let mut items = Vec::with_capacity(count);
                for i in 0..count {
                    // Each element is an i32 pointer stored at arr_offset + i*4
                    let elem_ptr = self.read_i32(arr_offset + i * 4);
                    items.push(self.read_value(elem_ptr));
                }
                Value::List(items)
            }
            TAG_RECORD => {
                let entries_offset = self.read_i32(offset + 4) as usize;
                let field_count = self.read_i32(offset + 8) as usize;
                let mut fields = BTreeMap::new();
                for i in 0..field_count {
                    let base = entries_offset + i * 12;
                    let key_ptr = self.read_i32(base) as usize;
                    let key_len = self.read_i32(base + 4) as usize;
                    let val_ptr = self.read_i32(base + 8);
                    let key = self.read_string(key_ptr, key_len);
                    let val = self.read_value(val_ptr);
                    fields.insert(key, val);
                }
                Value::Record {
                    type_name: None,
                    fields,
                }
            }
            _ => panic!("unknown WASM value tag: {tag} at offset {offset}"),
        }
    }

    /// Read a state field by name from the state record.
    #[allow(dead_code)]
    fn read_state_field(&mut self, name: &str) -> Option<Value> {
        let state_ptr = self.get_state_ptr();
        let state = self.read_value(state_ptr);
        match state {
            Value::Record { fields, .. } => fields.get(name).cloned(),
            _ => panic!("state is not a record"),
        }
    }

    /// Read all state fields as a BTreeMap.
    fn read_state(&mut self) -> BTreeMap<String, Value> {
        let state_ptr = self.get_state_ptr();
        let state = self.read_value(state_ptr);
        match state {
            Value::Record { fields, .. } => fields,
            _ => panic!("state is not a record"),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Value comparison helper
// ══════════════════════════════════════════════════════════════════════════════

/// Compare two Values for parity, handling f64 equality (including NaN).
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Number(x), Value::Number(y)) => x.to_bits() == y.to_bits(),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::List(xs), Value::List(ys)) => {
            xs.len() == ys.len()
                && xs.iter().zip(ys.iter()).all(|(a, b)| values_equal(a, b))
        }
        (
            Value::Record {
                fields: fa,
                type_name: _,
            },
            Value::Record {
                fields: fb,
                type_name: _,
            },
        ) => {
            fa.len() == fb.len()
                && fa
                    .iter()
                    .zip(fb.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && values_equal(va, vb))
        }
        _ => false,
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical PEPL sources (subset safe for WASM execution — no stdlib calls)
// ══════════════════════════════════════════════════════════════════════════════

/// Minimal counter without stdlib calls (no math.max — pure arithmetic only).
const SIMPLE_COUNTER: &str = r#"
space SimpleCounter {
  state {
    count: number = 0
  }

  action increment() {
    set count = count + 1
  }

  action decrement() {
    if count > 0 {
      set count = count - 1
    }
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Count" }
    }
  }
}
"#;

/// Boolean toggle — no stdlib calls.
const TOGGLE: &str = r#"
space Toggle {
  state {
    active: bool = false
  }

  action toggle() {
    if active {
      set active = false
    } else {
      set active = true
    }
  }

  view main() -> Surface {
    Text { value: "Toggle" }
  }
}
"#;

/// Multi-field state — no stdlib calls.
const MULTI_STATE: &str = r#"
space MultiState {
  state {
    x: number = 10
    y: number = 20
    label: string = "hello"
    flag: bool = true
  }

  action set_x(val: number) {
    set x = val
  }

  action update_label(new_label: string) {
    set label = new_label
  }

  view main() -> Surface {
    Text { value: label }
  }
}
"#;

/// Minimal — single dummy state field (PEPL requires at least one).
const MINIMAL_SPACE: &str = r#"
space Minimal {
  state {
    placeholder: number = 0
  }

  view main() -> Surface {
    Text { value: "minimal" }
  }
}
"#;

/// Arithmetic expressions — no stdlib calls.
const ARITHMETIC: &str = r#"
space Arithmetic {
  state {
    a: number = 5
    b: number = 3
    sum: number = 0
    product: number = 0
  }

  action compute() {
    set sum = a + b
    set product = a * b
  }

  action set_values(new_a: number, new_b: number) {
    set a = new_a
    set b = new_b
  }

  view main() -> Surface {
    Text { value: "calc" }
  }
}
"#;

// ══════════════════════════════════════════════════════════════════════════════
// Determinism Tests
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn determinism_compilation_100_iterations() {
    let sources = [SIMPLE_COUNTER, TOGGLE, MULTI_STATE, MINIMAL_SPACE, ARITHMETIC];
    for source in &sources {
        let reference = compile_source(source);
        for i in 0..100 {
            let output = compile_source(source);
            assert_eq!(
                reference, output,
                "compilation not deterministic at iteration {i}"
            );
        }
    }
}

#[test]
fn determinism_wasm_execution_init() {
    let sources = [SIMPLE_COUNTER, TOGGLE, MULTI_STATE, ARITHMETIC];
    for source in &sources {
        let wasm = compile_source(source);

        // Run init() N times with fresh instances, compare memory
        let mut reference: Option<Vec<u8>> = None;
        for i in 0..10 {
            let mut runner = WasmRunner::new(&wasm);
            runner.init();
            let snapshot = runner.memory_snapshot();
            if let Some(ref prev) = reference {
                assert_eq!(
                    prev, &snapshot,
                    "WASM execution not deterministic at iteration {i}"
                );
            } else {
                reference = Some(snapshot);
            }
        }
    }
}

#[test]
fn determinism_wasm_execution_with_actions() {
    let wasm = compile_source(SIMPLE_COUNTER);

    let mut reference: Option<Vec<u8>> = None;
    for i in 0..10 {
        let mut runner = WasmRunner::new(&wasm);
        runner.init();

        // Dispatch increment 5 times
        let dispatch_fn = runner
            .instance
            .get_typed_func::<(i32, i32, i32), ()>(&runner.store, "dispatch_action")
            .expect("no dispatch_action export");

        for _ in 0..5 {
            // action_id=0 (increment is first action), payload_ptr=0, payload_len=0
            dispatch_fn
                .call(&mut runner.store, (0, 0, 0))
                .expect("dispatch_action trapped");
        }

        let snapshot = runner.memory_snapshot();
        if let Some(ref prev) = reference {
            assert_eq!(
                prev, &snapshot,
                "WASM execution with actions not deterministic at iteration {i}"
            );
        } else {
            reference = Some(snapshot);
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Parity Tests — Eval ↔ Codegen
// ══════════════════════════════════════════════════════════════════════════════

/// Helper: create a SpaceInstance from source.
fn eval_instance(source: &str) -> SpaceInstance {
    let prog = parse(source);
    SpaceInstance::new(&prog).expect("SpaceInstance creation failed")
}

/// Assert that all state fields match between evaluator and WASM.
fn assert_state_parity(
    eval: &SpaceInstance,
    runner: &mut WasmRunner,
    state_fields: &[&str],
    context: &str,
) {
    let wasm_state = runner.read_state();

    for &field in state_fields {
        let eval_val = eval
            .get_state(field)
            .unwrap_or_else(|| panic!("eval missing state field '{field}' in {context}"));

        let wasm_val = wasm_state
            .get(field)
            .unwrap_or_else(|| panic!("WASM missing state field '{field}' in {context}"));

        assert!(
            values_equal(eval_val, wasm_val),
            "parity mismatch for '{field}' in {context}:\n  eval: {eval_val:?}\n  wasm: {wasm_val:?}"
        );
    }
}

#[test]
fn parity_simple_counter_init() {
    let eval = eval_instance(SIMPLE_COUNTER);
    let wasm = compile_source(SIMPLE_COUNTER);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    assert_state_parity(&eval, &mut runner, &["count"], "SimpleCounter init");
}

#[test]
fn parity_simple_counter_increment() {
    let mut eval = eval_instance(SIMPLE_COUNTER);
    let wasm = compile_source(SIMPLE_COUNTER);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    // Dispatch increment 3 times in both
    let dispatch_fn = runner
        .instance
        .get_typed_func::<(i32, i32, i32), ()>(&runner.store, "dispatch_action")
        .expect("no dispatch_action");

    for i in 0..3 {
        eval.dispatch("increment", vec![]).expect("eval dispatch");
        dispatch_fn
            .call(&mut runner.store, (0, 0, 0))
            .expect("wasm dispatch");

        assert_state_parity(
            &eval,
            &mut runner,
            &["count"],
            &format!("SimpleCounter after increment #{}", i + 1),
        );
    }
}

#[test]
fn parity_simple_counter_decrement_floor() {
    let mut eval = eval_instance(SIMPLE_COUNTER);
    let wasm = compile_source(SIMPLE_COUNTER);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    let dispatch_fn = runner
        .instance
        .get_typed_func::<(i32, i32, i32), ()>(&runner.store, "dispatch_action")
        .expect("no dispatch_action");

    // Decrement when count is 0 — should stay at 0
    eval.dispatch("decrement", vec![]).expect("eval dispatch");
    dispatch_fn
        .call(&mut runner.store, (1, 0, 0))
        .expect("wasm dispatch");

    assert_state_parity(
        &eval,
        &mut runner,
        &["count"],
        "SimpleCounter decrement at floor",
    );
}

#[test]
fn parity_toggle() {
    let mut eval = eval_instance(TOGGLE);
    let wasm = compile_source(TOGGLE);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    assert_state_parity(&eval, &mut runner, &["active"], "Toggle init");

    let dispatch_fn = runner
        .instance
        .get_typed_func::<(i32, i32, i32), ()>(&runner.store, "dispatch_action")
        .expect("no dispatch_action");

    // Toggle on
    eval.dispatch("toggle", vec![]).expect("eval dispatch");
    dispatch_fn
        .call(&mut runner.store, (0, 0, 0))
        .expect("wasm dispatch");
    assert_state_parity(&eval, &mut runner, &["active"], "Toggle after first toggle");

    // Toggle off
    eval.dispatch("toggle", vec![]).expect("eval dispatch");
    dispatch_fn
        .call(&mut runner.store, (0, 0, 0))
        .expect("wasm dispatch");
    assert_state_parity(
        &eval,
        &mut runner,
        &["active"],
        "Toggle after second toggle",
    );
}

#[test]
fn parity_multi_state_init() {
    let eval = eval_instance(MULTI_STATE);
    let wasm = compile_source(MULTI_STATE);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    assert_state_parity(
        &eval,
        &mut runner,
        &["x", "y", "label", "flag"],
        "MultiState init",
    );
}

#[test]
fn parity_arithmetic_compute() {
    let mut eval = eval_instance(ARITHMETIC);
    let wasm = compile_source(ARITHMETIC);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    assert_state_parity(
        &eval,
        &mut runner,
        &["a", "b", "sum", "product"],
        "Arithmetic init",
    );

    let dispatch_fn = runner
        .instance
        .get_typed_func::<(i32, i32, i32), ()>(&runner.store, "dispatch_action")
        .expect("no dispatch_action");

    // action compute() is action index 0
    eval.dispatch("compute", vec![]).expect("eval dispatch");
    dispatch_fn
        .call(&mut runner.store, (0, 0, 0))
        .expect("wasm dispatch");

    assert_state_parity(
        &eval,
        &mut runner,
        &["a", "b", "sum", "product"],
        "Arithmetic after compute",
    );
}

#[test]
fn parity_minimal_space_init() {
    let eval = eval_instance(MINIMAL_SPACE);
    let wasm = compile_source(MINIMAL_SPACE);
    let mut runner = WasmRunner::new(&wasm);
    runner.init();

    assert_state_parity(&eval, &mut runner, &["placeholder"], "Minimal space init");
}

// ══════════════════════════════════════════════════════════════════════════════
// Determinism: full end-to-end (compile + execute + compare)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn determinism_end_to_end_all_sources() {
    let sources = [
        ("SimpleCounter", SIMPLE_COUNTER),
        ("Toggle", TOGGLE),
        ("MultiState", MULTI_STATE),
        ("Minimal", MINIMAL_SPACE),
        ("Arithmetic", ARITHMETIC),
    ];

    for (name, source) in &sources {
        // Phase 1: compilation determinism
        let wasm_a = compile_source(source);
        let wasm_b = compile_source(source);
        assert_eq!(
            wasm_a, wasm_b,
            "{name}: compilation not deterministic"
        );

        // Phase 2: execution determinism (init)
        let mut runner_a = WasmRunner::new(&wasm_a);
        let mut runner_b = WasmRunner::new(&wasm_a);
        runner_a.init();
        runner_b.init();
        assert_eq!(
            runner_a.memory_snapshot(),
            runner_b.memory_snapshot(),
            "{name}: execution not deterministic after init"
        );
    }
}

//! Integration tests for the PEPL tree-walking evaluator.
//!
//! Tests key evaluator features:
//! - state initialization
//! - action dispatch & mutation
//! - derived fields
//! - invariant checking & rollback
//! - expression evaluation (arithmetic, string, list, record)
//! - view rendering
//! - gas metering
//! - canonical Counter / TodoList / UnitConverter examples

use pepl_eval::{EvalError, SpaceInstance, SurfaceNode};
use pepl_lexer::Lexer;
use pepl_parser::Parser;
use pepl_stdlib::Value;
use pepl_types::SourceFile;
use std::collections::BTreeMap;

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

/// Parse and create a SpaceInstance.
fn instance(source: &str) -> SpaceInstance {
    let prog = parse(source);
    SpaceInstance::new(&prog).expect("failed to create SpaceInstance")
}

/// Parse with a custom gas limit.
fn instance_with_gas(source: &str, gas: u64) -> SpaceInstance {
    let prog = parse(source);
    SpaceInstance::with_gas_limit(&prog, gas).expect("failed to create SpaceInstance")
}

// ══════════════════════════════════════════════════════════════════════════════
// State initialisation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn state_init_number() {
    let si = instance(
        r#"
space T {
  state { x: number = 42 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("x"), Some(&Value::Number(42.0)));
}

#[test]
fn state_init_string() {
    let si = instance(
        r#"
space T {
  state { name: string = "hello" }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("name"), Some(&Value::String("hello".into())));
}

#[test]
fn state_init_bool() {
    let si = instance(
        r#"
space T {
  state { flag: bool = false }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("flag"), Some(&Value::Bool(false)));
}

#[test]
fn state_init_list_empty() {
    let si = instance(
        r#"
space T {
  state { items: list<number> = [] }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("items"), Some(&Value::List(vec![])));
}

#[test]
fn state_init_list_with_values() {
    let si = instance(
        r#"
space T {
  state { items: list<number> = [1, 2, 3] }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(
        si.get_state("items"),
        Some(&Value::List(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]))
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Action dispatch & mutation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn action_increment() {
    let mut si = instance(
        r#"
space T {
  state { count: number = 0 }
  action increment() {
    set count = count + 1
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("count"), Some(&Value::Number(0.0)));

    let r = si.dispatch("increment", vec![]).unwrap();
    assert!(r.committed);
    assert_eq!(si.get_state("count"), Some(&Value::Number(1.0)));

    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&Value::Number(3.0)));
}

#[test]
fn action_with_params() {
    let mut si = instance(
        r#"
space T {
  state { total: number = 0 }
  action add(n: number) {
    set total = total + n
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("add", vec![Value::Number(10.0)]).unwrap();
    si.dispatch("add", vec![Value::Number(5.0)]).unwrap();
    assert_eq!(si.get_state("total"), Some(&Value::Number(15.0)));
}

#[test]
fn action_undefined_err() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 0 }
  action foo() { set x = 1 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    let err = si.dispatch("bar", vec![]).unwrap_err();
    match err {
        EvalError::UndefinedAction(name) => assert_eq!(name, "bar"),
        other => panic!("expected UndefinedAction, got {other:?}"),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Derived fields
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn derived_basic() {
    let si = instance(
        r#"
space T {
  state { price: number = 100 }
  derived {
    doubled: number = price * 2
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("price"), Some(&Value::Number(100.0)));
    // derived is in the environment too
    assert_eq!(
        si.state_snapshot().get("price"),
        Some(&Value::Number(100.0))
    );
}

#[test]
fn derived_recomputes_after_action() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 5 }
  derived {
    doubled: number = x * 2
  }
  action inc() { set x = x + 1 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    // Check initial derived
    let env_val = si.get_state("doubled");
    assert_eq!(env_val, Some(&Value::Number(10.0)));

    // After action, derived recomputes
    si.dispatch("inc", vec![]).unwrap();
    assert_eq!(si.get_state("doubled"), Some(&Value::Number(12.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Invariant checking & rollback
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn invariant_pass() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 5 }
  invariant positive { x >= 0 }
  action inc() { set x = x + 1 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    let r = si.dispatch("inc", vec![]).unwrap();
    assert!(r.committed);
    assert_eq!(si.get_state("x"), Some(&Value::Number(6.0)));
}

#[test]
fn invariant_rollback() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 1 }
  invariant positive { x > 0 }
  action sub(n: number) { set x = x - n }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    // This would set x to -4, violating the invariant
    let r = si.dispatch("sub", vec![Value::Number(5.0)]).unwrap();
    assert!(!r.committed);
    assert!(r.invariant_error.is_some());
    assert!(r.invariant_error.as_ref().unwrap().contains("positive"));
    // State should be rolled back
    assert_eq!(si.get_state("x"), Some(&Value::Number(1.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Expression evaluation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn arithmetic_operations() {
    let mut si = instance(
        r#"
space T {
  state { a: number = 10 }
  action compute() {
    set a = (2 + 3) * 4 - 1
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("compute", vec![]).unwrap();
    assert_eq!(si.get_state("a"), Some(&Value::Number(19.0)));
}

#[test]
fn string_interpolation() {
    let mut si = instance(
        r#"
space T {
  state {
    name: string = "world"
    greeting: string = ""
  }
  action greet() {
    set greeting = "Hello, ${name}!"
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("greet", vec![]).unwrap();
    assert_eq!(
        si.get_state("greeting"),
        Some(&Value::String("Hello, world!".into()))
    );
}

#[test]
fn boolean_logic() {
    let mut si = instance(
        r#"
space T {
  state { result: bool = false }
  action check() {
    set result = true and not false
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("check", vec![]).unwrap();
    assert_eq!(si.get_state("result"), Some(&Value::Bool(true)));
}

#[test]
fn list_operations_via_stdlib() {
    let mut si = instance(
        r#"
space T {
  state { items: list<number> = [1, 2, 3] }
  action add_four() {
    set items = list.append(items, 4)
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("add_four", vec![]).unwrap();
    let expected = Value::List(vec![
        Value::Number(1.0),
        Value::Number(2.0),
        Value::Number(3.0),
        Value::Number(4.0),
    ]);
    assert_eq!(si.get_state("items"), Some(&expected));
}

#[test]
fn record_field_access() {
    let mut si = instance(
        r#"
space T {
  state {
    r: { x: number, y: number } = { x: 1, y: 2 }
    sum: number = 0
  }
  action calc() {
    set sum = r.x + r.y
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("calc", vec![]).unwrap();
    assert_eq!(si.get_state("sum"), Some(&Value::Number(3.0)));
}

#[test]
fn nil_coalesce() {
    let mut si = instance(
        r#"
space T {
  state {
    out: number = 0
  }
  action check() {
    set out = nil ?? 99
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("check", vec![]).unwrap();
    assert_eq!(si.get_state("out"), Some(&Value::Number(99.0)));
}

#[test]
fn if_expression() {
    let mut si = instance(
        r#"
space T {
  state {
    x: number = 10
    label: string = ""
  }
  action classify() {
    set label = if x > 5 { "big" } else { "small" }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("classify", vec![]).unwrap();
    assert_eq!(si.get_state("label"), Some(&Value::String("big".into())));
}

#[test]
fn for_loop_in_action() {
    let mut si = instance(
        r#"
space T {
  state {
    total: number = 0
    nums: list<number> = [1, 2, 3, 4]
  }
  action sum_all() {
    for n in nums {
      set total = total + n
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("sum_all", vec![]).unwrap();
    assert_eq!(si.get_state("total"), Some(&Value::Number(10.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Match expressions
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn match_wildcard() {
    let mut si = instance(
        r#"
space T {
  state {
    status: string = "active"
    code: number = 0
  }
  action classify() {
    set code = if status == "active" { 1 } else { 0 }
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("classify", vec![]).unwrap();
    assert_eq!(si.get_state("code"), Some(&Value::Number(1.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Gas metering
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn gas_exhaustion() {
    let mut si = instance_with_gas(
        r#"
space T {
  state { x: number = 0 }
  action spin() {
    for i in [1,2,3,4,5,6,7,8,9,10] {
      for j in [1,2,3,4,5,6,7,8,9,10] {
        set x = x + 1
      }
    }
  }
  view main() -> Surface { Column { } { } }
}
"#,
        50, // Very low gas limit
    );
    let err = si.dispatch("spin", vec![]).unwrap_err();
    assert!(matches!(err, EvalError::GasExhausted));
}

// ══════════════════════════════════════════════════════════════════════════════
// View rendering
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn view_basic_render() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 42 }
  view main() -> Surface {
    Column { } {
      Text { value: "hello" }
    }
  }
}
"#,
    );
    let nodes = si.render().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].component, "Column");
    assert_eq!(nodes[0].children.len(), 1);
    assert_eq!(nodes[0].children[0].component, "Text");
    assert_eq!(
        nodes[0].children[0].props.get("value"),
        Some(&Value::String("hello".into()))
    );
}

#[test]
fn view_if_rendering() {
    let mut si = instance(
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
    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].children.len(), 1);
    assert_eq!(
        nodes[0].children[0].props.get("value"),
        Some(&Value::String("visible".into()))
    );
}

#[test]
fn view_for_rendering() {
    let mut si = instance(
        r#"
space T {
  state { items: list<string> = ["a", "b", "c"] }
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
    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].children.len(), 3);
    assert_eq!(
        nodes[0].children[0].props.get("value"),
        Some(&Value::String("a".into()))
    );
    assert_eq!(
        nodes[0].children[2].props.get("value"),
        Some(&Value::String("c".into()))
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Stdlib integration
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn stdlib_math_max() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 0 }
  action clamp() {
    set x = math.max(0, -5)
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("clamp", vec![]).unwrap();
    assert_eq!(si.get_state("x"), Some(&Value::Number(0.0)));
}

#[test]
fn stdlib_string_length() {
    let mut si = instance(
        r#"
space T {
  state { len: number = 0 }
  action measure() {
    set len = string.length("hello")
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("measure", vec![]).unwrap();
    assert_eq!(si.get_state("len"), Some(&Value::Number(5.0)));
}

#[test]
fn stdlib_list_length() {
    let mut si = instance(
        r#"
space T {
  state { len: number = 0 }
  action measure() {
    set len = list.length([10, 20, 30])
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("measure", vec![]).unwrap();
    assert_eq!(si.get_state("len"), Some(&Value::Number(3.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Log capture
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn log_capture() {
    let mut si = instance(
        r#"
space T {
  state { x: number = 0 }
  action go() {
    core.log("step 1")
    set x = 42
    core.log("step 2")
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("go", vec![]).unwrap();
    assert_eq!(si.log_output(), &["step 1", "step 2"]);
    assert_eq!(si.get_state("x"), Some(&Value::Number(42.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Surface serialisation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn surface_to_json() {
    let node = SurfaceNode {
        component: "Text".into(),
        props: {
            let mut m = BTreeMap::new();
            m.insert("value".to_string(), Value::String("hi".into()));
            m
        },
        children: vec![],
    };
    let json = SpaceInstance::surface_to_json(&[node]);
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["component"], "Text");
    assert_eq!(arr[0]["props"]["value"], "hi");
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical: Counter
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_counter_eval() {
    let mut si = instance(
        r#"
space Counter {
  state {
    count: number = 0
  }

  action increment() {
    set count = count + 1
  }

  action decrement() {
    set count = math.max(0, count - 1)
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Count: ${count}" }
      Row { } {
        Button { label: "minus", on_tap: decrement }
        Button { label: "+", on_tap: increment }
      }
    }
  }
}
"#,
    );
    assert_eq!(si.get_state("count"), Some(&Value::Number(0.0)));

    // Increment 3 times
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&Value::Number(3.0)));

    // Decrement once
    si.dispatch("decrement", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&Value::Number(2.0)));

    // Decrement to zero — should floor at 0
    si.dispatch("decrement", vec![]).unwrap();
    si.dispatch("decrement", vec![]).unwrap();
    si.dispatch("decrement", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&Value::Number(0.0)));

    // Render and check structure
    let nodes = si.render().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].component, "Column");
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical: TodoList
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_todo_list_eval() {
    let mut si = instance(
        r#"
space TodoList {
  state {
    todos: list<{ text: string, done: bool }> = []
    input: string = ""
  }

  derived {
    remaining: number = list.length(list.filter(todos, fn(t: { text: string, done: bool }) { not t.done }))
  }

  action update_input(value: string) {
    set input = value
  }

  action add_todo() {
    if string.length(input) > 0 {
      set todos = list.append(todos, { text: input, done: false })
      set input = ""
    }
  }

  action toggle(index: number) {
    let item = list.get(todos, index)
    set todos = list.update(todos, index, { text: item.text, done: not item.done })
  }

  view main() -> Surface {
    Column { } {
      Row { } {
        TextInput { value: input, on_change: update_input }
        Button { label: "Add", on_tap: add_todo }
      }
      for todo, i in todos {
        Row { } {
          Text { value: todo.text }
          Button { label: if todo.done { "undo" } else { "done" }, on_tap: toggle }
        }
      }
      Text { value: "${remaining} remaining" }
    }
  }
}
"#,
    );

    // Initially empty
    assert_eq!(si.get_state("todos"), Some(&Value::List(vec![])));

    // Add a todo
    si.dispatch("update_input", vec![Value::String("Buy milk".into())])
        .unwrap();
    si.dispatch("add_todo", vec![]).unwrap();
    assert_eq!(si.get_state("input"), Some(&Value::String("".into())));

    let todos = si.get_state("todos").unwrap().clone();
    if let Value::List(items) = &todos {
        assert_eq!(items.len(), 1);
    } else {
        panic!("expected list");
    }

    // Toggle first todo
    si.dispatch("toggle", vec![Value::Number(0.0)]).unwrap();
    let todos = si.get_state("todos").unwrap().clone();
    if let Value::List(items) = &todos {
        if let Value::Record { fields, .. } = &items[0] {
            assert_eq!(fields.get("done"), Some(&Value::Bool(true)));
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// State snapshot
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn state_snapshot() {
    let mut si = instance(
        r#"
space T {
  state {
    a: number = 1
    b: string = "x"
  }
  action go() { set a = 2 }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    let snap = si.state_snapshot();
    assert_eq!(snap.get("a"), Some(&Value::Number(1.0)));
    assert_eq!(snap.get("b"), Some(&Value::String("x".into())));

    si.dispatch("go", vec![]).unwrap();
    let snap = si.state_snapshot();
    assert_eq!(snap.get("a"), Some(&Value::Number(2.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Let bindings in actions
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn let_binding_in_action() {
    let mut si = instance(
        r#"
space T {
  state { result: number = 0 }
  action calc() {
    let x = 10
    let y = 20
    set result = x + y
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    si.dispatch("calc", vec![]).unwrap();
    assert_eq!(si.get_state("result"), Some(&Value::Number(30.0)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Multiple state blocks
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_state_fields() {
    let si = instance(
        r#"
space T {
  state {
    a: number = 1
    b: number = 2
  }
  view main() -> Surface { Column { } { } }
}
"#,
    );
    assert_eq!(si.get_state("a"), Some(&Value::Number(1.0)));
    assert_eq!(si.get_state("b"), Some(&Value::Number(2.0)));
}

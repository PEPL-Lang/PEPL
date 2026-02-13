//! M3 Gate Tests — Evaluate all 7 canonical examples.
//!
//! Tests verify:
//! - All 7 canonical programs execute correctly
//! - State snapshots match expected values after action sequences
//! - Surface trees render correctly
//! - Test runner executes `tests {}` blocks
//! - Capability mocking via with_responses
//! - Game loop (update/handleEvent)
//! - Determinism (100-iteration)
//! - Golden reference capture

use pepl_eval::{EvalError, SpaceInstance, SurfaceNode};
use pepl_lexer::Lexer;
use pepl_parser::Parser;
use pepl_stdlib::Value;
use pepl_types::SourceFile;
use std::collections::BTreeMap;

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

fn instance(source: &str) -> SpaceInstance {
    let prog = parse(source);
    SpaceInstance::new(&prog).expect("failed to create SpaceInstance")
}

/// Value helper — number
fn num(n: f64) -> Value {
    Value::Number(n)
}

/// Value helper — string
fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

/// Value helper — bool
fn b(v: bool) -> Value {
    Value::Bool(v)
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 1: Counter
// ══════════════════════════════════════════════════════════════════════════════

const COUNTER_SOURCE: &str = r#"
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
"#;

#[test]
fn counter_state_init() {
    let si = instance(COUNTER_SOURCE);
    assert_eq!(si.get_state("count"), Some(&num(0.0)));
}

#[test]
fn counter_increment_sequence() {
    let mut si = instance(COUNTER_SOURCE);
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&num(3.0)));
}

#[test]
fn counter_decrement_floor() {
    let mut si = instance(COUNTER_SOURCE);
    si.dispatch("increment", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&num(1.0)));

    si.dispatch("decrement", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&num(0.0)));

    // Floor at 0
    si.dispatch("decrement", vec![]).unwrap();
    assert_eq!(si.get_state("count"), Some(&num(0.0)));
}

#[test]
fn counter_render() {
    let mut si = instance(COUNTER_SOURCE);
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();

    let nodes = si.render().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].component, "Column");
    assert_eq!(nodes[0].children.len(), 2);
    assert_eq!(nodes[0].children[0].component, "Text");
    assert_eq!(
        nodes[0].children[0].props.get("value"),
        Some(&s("Count: 2"))
    );
}

#[test]
fn counter_golden_reference() {
    let mut si = instance(COUNTER_SOURCE);
    let snap_init = si.state_snapshot();
    assert_eq!(snap_init.get("count"), Some(&num(0.0)));

    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    let snap_after = si.state_snapshot();
    assert_eq!(snap_after.get("count"), Some(&num(3.0)));

    si.dispatch("decrement", vec![]).unwrap();
    let snap_final = si.state_snapshot();
    assert_eq!(snap_final.get("count"), Some(&num(2.0)));

    // Surface JSON snapshot
    let nodes = si.render().unwrap();
    let json = SpaceInstance::surface_to_json(&nodes);
    let json_str = serde_json::to_string_pretty(&json).unwrap();
    assert!(json_str.contains("Count: 2"));
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 2: TodoList
// ══════════════════════════════════════════════════════════════════════════════

const TODO_LIST_SOURCE: &str = r#"
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
    let todo = list.get(todos, index)
    if todo != nil {
      set todos = list.set(todos, index, { ...todo, done: not todo.done })
    }
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
"#;

#[test]
fn todo_list_empty_init() {
    let si = instance(TODO_LIST_SOURCE);
    assert_eq!(si.get_state("todos"), Some(&Value::List(vec![])));
    assert_eq!(si.get_state("input"), Some(&s("")));
    assert_eq!(si.get_state("remaining"), Some(&num(0.0)));
}

#[test]
fn todo_list_add_todo() {
    let mut si = instance(TODO_LIST_SOURCE);
    si.dispatch("update_input", vec![s("Buy milk")]).unwrap();
    si.dispatch("add_todo", vec![]).unwrap();

    assert_eq!(si.get_state("input"), Some(&s("")));
    let todos = si.get_state("todos").unwrap().clone();
    if let Value::List(items) = todos {
        assert_eq!(items.len(), 1);
        if let Value::Record { fields, .. } = &items[0] {
            assert_eq!(fields.get("text"), Some(&s("Buy milk")));
            assert_eq!(fields.get("done"), Some(&b(false)));
        } else {
            panic!("expected record");
        }
    } else {
        panic!("expected list");
    }
    assert_eq!(si.get_state("remaining"), Some(&num(1.0)));
}

#[test]
fn todo_list_toggle_done() {
    let mut si = instance(TODO_LIST_SOURCE);
    si.dispatch("update_input", vec![s("Task 1")]).unwrap();
    si.dispatch("add_todo", vec![]).unwrap();
    si.dispatch("update_input", vec![s("Task 2")]).unwrap();
    si.dispatch("add_todo", vec![]).unwrap();
    assert_eq!(si.get_state("remaining"), Some(&num(2.0)));

    // Toggle first
    si.dispatch("toggle", vec![num(0.0)]).unwrap();
    assert_eq!(si.get_state("remaining"), Some(&num(1.0)));

    let todos = si.get_state("todos").unwrap().clone();
    if let Value::List(items) = todos {
        if let Value::Record { fields, .. } = &items[0] {
            assert_eq!(fields.get("done"), Some(&b(true)));
        }
        if let Value::Record { fields, .. } = &items[1] {
            assert_eq!(fields.get("done"), Some(&b(false)));
        }
    }
}

#[test]
fn todo_list_render() {
    let mut si = instance(TODO_LIST_SOURCE);
    si.dispatch("update_input", vec![s("Groceries")]).unwrap();
    si.dispatch("add_todo", vec![]).unwrap();

    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].component, "Column");
    // Row (input/button) + 1 for-loop Row + Text (remaining)
    assert_eq!(nodes[0].children.len(), 3);

    // Last child = remaining text
    let remaining_text = &nodes[0].children[2];
    assert_eq!(remaining_text.component, "Text");
    assert_eq!(remaining_text.props.get("value"), Some(&s("1 remaining")));
}

#[test]
fn todo_list_empty_input_no_add() {
    let mut si = instance(TODO_LIST_SOURCE);
    // Don't set input — should be ""
    si.dispatch("add_todo", vec![]).unwrap();
    assert_eq!(si.get_state("todos"), Some(&Value::List(vec![])));
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 3: UnitConverter
// ══════════════════════════════════════════════════════════════════════════════

const UNIT_CONVERTER_SOURCE: &str = r#"
space UnitConverter {
  state {
    value: number = 0
  }

  action set_value(v: string) {
    match convert.parse_float(v) {
      Ok(n) -> { set value = n }
      Err(e) -> { }
    }
  }

  view main() -> Surface {
    Column { } {
      TextInput { value: convert.to_string(value), on_change: set_value, placeholder: "Enter value" }
      Text { value: "km to miles: ${math.round_to(value * 0.621371, 2)}" }
      Text { value: "miles to km: ${math.round_to(value * 1.60934, 2)}" }
      Text { value: "C to F: ${math.round_to(value * 9 / 5 + 32, 1)}" }
      Text { value: "F to C: ${math.round_to((value - 32) * 5 / 9, 1)}" }
    }
  }
}
"#;

#[test]
fn unit_converter_init() {
    let si = instance(UNIT_CONVERTER_SOURCE);
    assert_eq!(si.get_state("value"), Some(&num(0.0)));
}

#[test]
fn unit_converter_set_value() {
    let mut si = instance(UNIT_CONVERTER_SOURCE);
    si.dispatch("set_value", vec![s("100")]).unwrap();
    assert_eq!(si.get_state("value"), Some(&num(100.0)));
}

#[test]
fn unit_converter_invalid_input() {
    let mut si = instance(UNIT_CONVERTER_SOURCE);
    si.dispatch("set_value", vec![s("abc")]).unwrap();
    // Value should remain 0 since parse failed
    assert_eq!(si.get_state("value"), Some(&num(0.0)));
}

#[test]
fn unit_converter_render() {
    let mut si = instance(UNIT_CONVERTER_SOURCE);
    si.dispatch("set_value", vec![s("100")]).unwrap();

    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].component, "Column");
    // TextInput + 4 Text children
    assert_eq!(nodes[0].children.len(), 5);

    // Check km → miles: 100 * 0.621371 = 62.14 (rounded to 2dp)
    let km_text = &nodes[0].children[1];
    assert_eq!(km_text.component, "Text");
    assert_eq!(
        km_text.props.get("value"),
        Some(&s("km to miles: 62.14"))
    );

    // Check miles → km: 100 * 1.60934 = 160.93 (rounded to 2dp)
    let miles_text = &nodes[0].children[2];
    assert_eq!(
        miles_text.props.get("value"),
        Some(&s("miles to km: 160.93"))
    );

    // Check °C → °F: 100 * 9/5 + 32 = 212.0 → displays as "212" (whole number)
    let c_to_f = &nodes[0].children[3];
    assert_eq!(c_to_f.props.get("value"), Some(&s("C to F: 212")));

    // Check °F → °C: (100-32) * 5/9 = 37.8 (rounded to 1dp)
    let f_to_c = &nodes[0].children[4];
    assert_eq!(f_to_c.props.get("value"), Some(&s("F to C: 37.8")));
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 4: WeatherDashboard (capability mocking)
// ══════════════════════════════════════════════════════════════════════════════

const WEATHER_DASHBOARD_SOURCE: &str = r#"
space WeatherDashboard {
  state {
    city: string = "London"
    temperature: string = "--"
    description: string = "Enter a city and tap Search"
    loading: bool = false
    error_message: string = ""
  }

  capabilities {
    required: [display, keyboard_or_touch, http]
  }

  credentials {
    weather_api_key: string
  }

  action update_city(value: string) {
    set city = value
  }

  action fetch_weather() {
    set loading = true
    set error_message = ""
    let response = http.get("https://api.weather.dev/v1/current?city=${city}")
    match response {
      Ok(body) -> {
        let data = json.parse(body)
        match data {
          Ok(parsed) -> {
            set temperature = record.get(parsed, "temp")
            set description = record.get(parsed, "description")
          }
          Err(e) -> { set error_message = "Invalid response format" }
        }
      }
      Err(e) -> { set error_message = e }
    }
    set loading = false
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Weather Dashboard" }
      Row { } {
        TextInput { value: city, on_change: update_city, placeholder: "City name" }
        Button { label: "Search", on_tap: fetch_weather }
      }
      if loading {
        Text { value: "Loading..." }
      }
      if string.length(error_message) > 0 {
        Text { value: "Error: ${error_message}" }
      }
      if not loading {
        Text { value: temperature }
        Text { value: description }
      }
    }
  }
}
"#;

#[test]
fn weather_dashboard_init() {
    let si = instance(WEATHER_DASHBOARD_SOURCE);
    assert_eq!(si.get_state("city"), Some(&s("London")));
    assert_eq!(si.get_state("temperature"), Some(&s("--")));
    assert_eq!(si.get_state("loading"), Some(&b(false)));
}

#[test]
fn weather_dashboard_update_city() {
    let mut si = instance(WEATHER_DASHBOARD_SOURCE);
    si.dispatch("update_city", vec![s("Paris")]).unwrap();
    assert_eq!(si.get_state("city"), Some(&s("Paris")));
}

#[test]
fn weather_dashboard_fetch_unmocked() {
    let mut si = instance(WEATHER_DASHBOARD_SOURCE);
    // Without mock responses, http.get returns Err("unmocked_call")
    si.dispatch("fetch_weather", vec![]).unwrap();
    assert_eq!(si.get_state("loading"), Some(&b(false)));
    // error_message should contain the unmocked message  
    let err = si.get_state("error_message").unwrap().clone();
    if let Value::String(msg) = err {
        assert!(msg.contains("unmocked"), "error_message = {msg}");
    }
}

#[test]
fn weather_dashboard_fetch_mocked_success() {
    let mut si = instance(WEATHER_DASHBOARD_SOURCE);
    // Mock http.get to return Ok with valid JSON body
    si.set_mock_responses(vec![pepl_eval::MockResponse {
        module: "http".into(),
        function: "get".into(),
        response: Value::Result(Box::new(pepl_stdlib::ResultValue::Ok(
            s("{\"temp\": \"25\", \"description\": \"Sunny\"}")
        ))),
    }]);

    si.dispatch("fetch_weather", vec![]).unwrap();
    assert_eq!(si.get_state("loading"), Some(&b(false)));
    assert_eq!(si.get_state("temperature"), Some(&s("25")));
    assert_eq!(si.get_state("description"), Some(&s("Sunny")));
    assert_eq!(si.get_state("error_message"), Some(&s("")));
}

#[test]
fn weather_dashboard_fetch_mocked_error() {
    let mut si = instance(WEATHER_DASHBOARD_SOURCE);
    si.set_mock_responses(vec![pepl_eval::MockResponse {
        module: "http".into(),
        function: "get".into(),
        response: Value::Result(Box::new(pepl_stdlib::ResultValue::Err(
            s("Network timeout"),
        ))),
    }]);

    si.dispatch("fetch_weather", vec![]).unwrap();
    assert_eq!(si.get_state("loading"), Some(&b(false)));
    assert_eq!(si.get_state("error_message"), Some(&s("Network timeout")));
}

#[test]
fn weather_dashboard_render_initial() {
    let mut si = instance(WEATHER_DASHBOARD_SOURCE);
    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].component, "Column");
    // "Weather Dashboard" title + Row + (not loading → temp + desc) = 4 children
    // (no loading text, no error text)
    let children = &nodes[0].children;
    assert!(children.len() >= 3);
    assert_eq!(children[0].props.get("value"), Some(&s("Weather Dashboard")));
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 5: PomodoroTimer
// ══════════════════════════════════════════════════════════════════════════════

const POMODORO_SOURCE: &str = r#"
space PomodoroTimer {
  state {
    mode: string = "idle"
    seconds_left: number = 1500
    total_pomodoros: number = 0
    timer_id: string = ""
  }

  capabilities {
    required: [display, keyboard_or_touch, timer]
  }

  action start_work() {
    timer.stop_all()
    set mode = "work"
    set seconds_left = 1500
    set timer_id = timer.start("tick", 1000)
  }

  action start_break() {
    timer.stop_all()
    set mode = "break"
    set seconds_left = 300
    set timer_id = timer.start("tick", 1000)
  }

  action tick() {
    if seconds_left > 0 {
      set seconds_left = seconds_left - 1
    } else {
      timer.stop(timer_id)
      if mode == "work" {
        set total_pomodoros = total_pomodoros + 1
        set mode = "done_work"
      } else {
        set mode = "idle"
      }
    }
  }

  action reset() {
    timer.stop_all()
    set mode = "idle"
    set seconds_left = 1500
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Pomodoro Timer" }
      Text { value: "${math.floor(seconds_left / 60)}:${string.pad_start(convert.to_string(seconds_left % 60), 2, "0")}" }
      Text { value: "Mode: ${mode}" }
      Row { } {
        Button { label: "Start Work", on_tap: start_work }
        Button { label: "Start Break", on_tap: start_break }
        Button { label: "Reset", on_tap: reset }
      }
      Text { value: "Completed: ${total_pomodoros} pomodoros" }
    }
  }
}
"#;

#[test]
fn pomodoro_init() {
    let si = instance(POMODORO_SOURCE);
    assert_eq!(si.get_state("mode"), Some(&s("idle")));
    assert_eq!(si.get_state("seconds_left"), Some(&num(1500.0)));
    assert_eq!(si.get_state("total_pomodoros"), Some(&num(0.0)));
}

#[test]
fn pomodoro_start_work() {
    let mut si = instance(POMODORO_SOURCE);
    si.dispatch("start_work", vec![]).unwrap();
    assert_eq!(si.get_state("mode"), Some(&s("work")));
    assert_eq!(si.get_state("seconds_left"), Some(&num(1500.0)));
    assert_eq!(si.get_state("timer_id"), Some(&s("tick")));
}

#[test]
fn pomodoro_tick_countdown() {
    let mut si = instance(POMODORO_SOURCE);
    si.dispatch("start_work", vec![]).unwrap();

    // Tick 3 times
    si.dispatch("tick", vec![]).unwrap();
    si.dispatch("tick", vec![]).unwrap();
    si.dispatch("tick", vec![]).unwrap();
    assert_eq!(si.get_state("seconds_left"), Some(&num(1497.0)));
}

#[test]
fn pomodoro_work_complete() {
    let mut si = instance(POMODORO_SOURCE);
    si.dispatch("start_work", vec![]).unwrap();

    // Set seconds_left to 1 manually by ticking down
    // Simulate by dispatching a helper action... or set directly
    // Instead, we use a shorter version:
    let prog = parse(r#"
space PomodoroShort {
  state {
    mode: string = "work"
    seconds_left: number = 1
    total_pomodoros: number = 0
    timer_id: string = "tick"
  }

  action tick() {
    if seconds_left > 0 {
      set seconds_left = seconds_left - 1
    } else {
      timer.stop(timer_id)
      if mode == "work" {
        set total_pomodoros = total_pomodoros + 1
        set mode = "done_work"
      } else {
        set mode = "idle"
      }
    }
  }

  action start_break() {
    timer.stop_all()
    set mode = "break"
    set seconds_left = 300
    set timer_id = timer.start("tick", 1000)
  }

  view main() -> Surface { Column { } { } }
}
"#);
    let mut si2 = SpaceInstance::new(&prog).unwrap();
    assert_eq!(si2.get_state("seconds_left"), Some(&num(1.0)));

    si2.dispatch("tick", vec![]).unwrap(); // 1 → 0
    assert_eq!(si2.get_state("seconds_left"), Some(&num(0.0)));

    si2.dispatch("tick", vec![]).unwrap(); // 0 → triggers work complete
    assert_eq!(si2.get_state("mode"), Some(&s("done_work")));
    assert_eq!(si2.get_state("total_pomodoros"), Some(&num(1.0)));
}

#[test]
fn pomodoro_reset() {
    let mut si = instance(POMODORO_SOURCE);
    si.dispatch("start_work", vec![]).unwrap();
    si.dispatch("tick", vec![]).unwrap();
    si.dispatch("reset", vec![]).unwrap();
    assert_eq!(si.get_state("mode"), Some(&s("idle")));
    assert_eq!(si.get_state("seconds_left"), Some(&num(1500.0)));
}

#[test]
fn pomodoro_render() {
    let mut si = instance(POMODORO_SOURCE);
    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].component, "Column");
    // Title + timer display + mode + Row (buttons) + completed text = 5 children
    assert_eq!(nodes[0].children.len(), 5);
    assert_eq!(
        nodes[0].children[0].props.get("value"),
        Some(&s("Pomodoro Timer"))
    );
    // Time display: 1500s = 25:00
    assert_eq!(
        nodes[0].children[1].props.get("value"),
        Some(&s("25:00"))
    );
    assert_eq!(
        nodes[0].children[2].props.get("value"),
        Some(&s("Mode: idle"))
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 6: HabitTracker (capability mocking)
// ══════════════════════════════════════════════════════════════════════════════

const HABIT_TRACKER_SOURCE: &str = r#"
space HabitTracker {
  state {
    habits: list<{ name: string, streak: number, last_done: number }> = []
    new_habit: string = ""
  }

  capabilities {
    required: [display, keyboard_or_touch, storage]
  }

  action update_new_habit(value: string) {
    set new_habit = value
  }

  action add_habit() {
    if string.length(new_habit) > 0 {
      set habits = list.append(habits, { name: new_habit, streak: 0, last_done: 0 })
      set new_habit = ""
    }
  }

  action mark_done(index: number) {
    let habit = list.get(habits, index)
    if habit != nil {
      let now = time.now()
      set habits = list.set(habits, index, { ...habit, streak: habit.streak + 1, last_done: now })
    }
  }

  view main() -> Surface {
    Column { } {
      Text { value: "Habit Tracker" }
      Row { } {
        TextInput { value: new_habit, on_change: update_new_habit, placeholder: "New habit..." }
        Button { label: "Add", on_tap: add_habit }
      }
      for habit, index in habits {
        Row { } {
          Text { value: habit.name }
          Text { value: "Streak: ${habit.streak}" }
          Button { label: "Done today", on_tap: mark_done(index) }
        }
      }
    }
  }
}
"#;

#[test]
fn habit_tracker_init() {
    let si = instance(HABIT_TRACKER_SOURCE);
    assert_eq!(si.get_state("habits"), Some(&Value::List(vec![])));
    assert_eq!(si.get_state("new_habit"), Some(&s("")));
}

#[test]
fn habit_tracker_add_habit() {
    let mut si = instance(HABIT_TRACKER_SOURCE);
    si.dispatch("update_new_habit", vec![s("Exercise")])
        .unwrap();
    si.dispatch("add_habit", vec![]).unwrap();
    assert_eq!(si.get_state("new_habit"), Some(&s("")));

    let habits = si.get_state("habits").unwrap().clone();
    if let Value::List(items) = habits {
        assert_eq!(items.len(), 1);
        if let Value::Record { fields, .. } = &items[0] {
            assert_eq!(fields.get("name"), Some(&s("Exercise")));
            assert_eq!(fields.get("streak"), Some(&num(0.0)));
        }
    }
}

#[test]
fn habit_tracker_mark_done() {
    let mut si = instance(HABIT_TRACKER_SOURCE);
    si.dispatch("update_new_habit", vec![s("Read")])
        .unwrap();
    si.dispatch("add_habit", vec![]).unwrap();
    si.dispatch("mark_done", vec![num(0.0)]).unwrap();

    let habits = si.get_state("habits").unwrap().clone();
    if let Value::List(items) = habits {
        if let Value::Record { fields, .. } = &items[0] {
            assert_eq!(fields.get("streak"), Some(&num(1.0)));
            // time.now() returns 0.0 as a deterministic stub
            assert_eq!(fields.get("last_done"), Some(&num(0.0)));
        }
    }
}

#[test]
fn habit_tracker_render() {
    let mut si = instance(HABIT_TRACKER_SOURCE);
    si.dispatch("update_new_habit", vec![s("Meditate")])
        .unwrap();
    si.dispatch("add_habit", vec![]).unwrap();

    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].component, "Column");
    // Title + input Row + 1 for-loop Row (habit) = 3
    assert_eq!(nodes[0].children.len(), 3);
    assert_eq!(
        nodes[0].children[0].props.get("value"),
        Some(&s("Habit Tracker"))
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 7: QuizApp
// ══════════════════════════════════════════════════════════════════════════════

const QUIZ_APP_SOURCE: &str = r#"
space QuizApp {
  state {
    current_question: number = 0
    score: number = 0
    answered: bool = false
    selected: string = ""
    questions: list<{ question: string, options: list<string>, correct: string }> = [
      { question: "What is 2 + 2?", options: ["3", "4", "5"], correct: "4" },
      { question: "Capital of France?", options: ["London", "Paris", "Berlin"], correct: "Paris" },
      { question: "Largest planet?", options: ["Earth", "Mars", "Jupiter"], correct: "Jupiter" }
    ]
  }

  action select_answer(answer: string) {
    set selected = answer
    set answered = true
    let q = list.get(questions, current_question)
    if q != nil {
      if answer == q.correct {
        set score = score + 1
      }
    }
  }

  action next_question() {
    set current_question = current_question + 1
    set answered = false
    set selected = ""
  }

  view main() -> Surface {
    Column { } {
      if current_question < list.length(questions) {
        let q = list.get(questions, current_question)
        if q != nil {
          Text { value: "Question ${current_question + 1} of ${list.length(questions)}" }
          Text { value: q.question }
          for option in q.options {
            Button { label: option, on_tap: select_answer(option) }
          }
          if answered {
            Text { value: if selected == q.correct { "Correct!" } else { "Wrong!" } }
            Button { label: "Next", on_tap: next_question }
          }
        }
      } else {
        Text { value: "Quiz Complete!" }
        Text { value: "Score: ${score} / ${list.length(questions)}" }
      }
    }
  }
}
"#;

#[test]
fn quiz_app_init() {
    let si = instance(QUIZ_APP_SOURCE);
    assert_eq!(si.get_state("current_question"), Some(&num(0.0)));
    assert_eq!(si.get_state("score"), Some(&num(0.0)));
    assert_eq!(si.get_state("answered"), Some(&b(false)));
    assert_eq!(si.get_state("selected"), Some(&s("")));

    let questions = si.get_state("questions").unwrap().clone();
    if let Value::List(items) = questions {
        assert_eq!(items.len(), 3);
    }
}

#[test]
fn quiz_app_correct_answer() {
    let mut si = instance(QUIZ_APP_SOURCE);
    si.dispatch("select_answer", vec![s("4")]).unwrap();
    assert_eq!(si.get_state("score"), Some(&num(1.0)));
    assert_eq!(si.get_state("answered"), Some(&b(true)));
    assert_eq!(si.get_state("selected"), Some(&s("4")));
}

#[test]
fn quiz_app_wrong_answer() {
    let mut si = instance(QUIZ_APP_SOURCE);
    si.dispatch("select_answer", vec![s("3")]).unwrap();
    assert_eq!(si.get_state("score"), Some(&num(0.0)));
    assert_eq!(si.get_state("answered"), Some(&b(true)));
}

#[test]
fn quiz_app_full_run() {
    let mut si = instance(QUIZ_APP_SOURCE);

    // Q1: correct (4)
    si.dispatch("select_answer", vec![s("4")]).unwrap();
    assert_eq!(si.get_state("score"), Some(&num(1.0)));
    si.dispatch("next_question", vec![]).unwrap();
    assert_eq!(si.get_state("current_question"), Some(&num(1.0)));

    // Q2: wrong (London)
    si.dispatch("select_answer", vec![s("London")]).unwrap();
    assert_eq!(si.get_state("score"), Some(&num(1.0)));
    si.dispatch("next_question", vec![]).unwrap();

    // Q3: correct (Jupiter)
    si.dispatch("select_answer", vec![s("Jupiter")]).unwrap();
    assert_eq!(si.get_state("score"), Some(&num(2.0)));
    si.dispatch("next_question", vec![]).unwrap();

    // Quiz complete
    assert_eq!(si.get_state("current_question"), Some(&num(3.0)));
}

#[test]
fn quiz_app_render_first_question() {
    let mut si = instance(QUIZ_APP_SOURCE);
    let nodes = si.render().unwrap();
    assert_eq!(nodes[0].component, "Column");

    // Should show Q1 text and 3 option buttons (no answer feedback yet)
    let children = &nodes[0].children;
    // "Question 1 of 3", "What is 2 + 2?", Button "3", Button "4", Button "5"
    assert!(children.len() >= 5, "expected at least 5 children, got {}", children.len());
    assert_eq!(
        children[0].props.get("value"),
        Some(&s("Question 1 of 3"))
    );
    assert_eq!(
        children[1].props.get("value"),
        Some(&s("What is 2 + 2?"))
    );
}

#[test]
fn quiz_app_render_complete() {
    let mut si = instance(QUIZ_APP_SOURCE);
    // Answer all questions
    si.dispatch("select_answer", vec![s("4")]).unwrap();
    si.dispatch("next_question", vec![]).unwrap();
    si.dispatch("select_answer", vec![s("Paris")]).unwrap();
    si.dispatch("next_question", vec![]).unwrap();
    si.dispatch("select_answer", vec![s("Jupiter")]).unwrap();
    si.dispatch("next_question", vec![]).unwrap();

    let nodes = si.render().unwrap();
    let children = &nodes[0].children;
    // Should show "Quiz Complete!" and "Score: 3 / 3"
    assert_eq!(children.len(), 2);
    assert_eq!(
        children[0].props.get("value"),
        Some(&s("Quiz Complete!"))
    );
    assert_eq!(
        children[1].props.get("value"),
        Some(&s("Score: 3 / 3"))
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Test Runner
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_runner_simple_pass() {
    let prog = parse(r#"
space Counter {
  state { count: number = 0 }
  action increment() { set count = count + 1 }
  view main() -> Surface { Column { } { } }
}

tests {
  test "increment works" {
    increment()
    increment()
    assert count == 2
  }
}
"#);
    let summary = pepl_eval::run_tests(&prog).unwrap();
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 0);
}

#[test]
fn test_runner_assert_failure() {
    let prog = parse(r#"
space Counter {
  state { count: number = 0 }
  action increment() { set count = count + 1 }
  view main() -> Surface { Column { } { } }
}

tests {
  test "wrong assertion" {
    increment()
    assert count == 99, "expected 99"
  }
}
"#);
    let summary = pepl_eval::run_tests(&prog).unwrap();
    assert_eq!(summary.passed, 0);
    assert_eq!(summary.failed, 1);
    assert!(summary.results[0].error.as_ref().unwrap().contains("expected 99"));
}

#[test]
fn test_runner_multiple_tests() {
    let prog = parse(r#"
space Counter {
  state { count: number = 0 }
  action increment() { set count = count + 1 }
  action decrement() { set count = math.max(0, count - 1) }
  view main() -> Surface { Column { } { } }
}

tests {
  test "increment adds 1" {
    increment()
    assert count == 1
  }

  test "decrement floors at 0" {
    decrement()
    assert count == 0
  }

  test "increment then decrement" {
    increment()
    increment()
    decrement()
    assert count == 1
  }
}
"#);
    let summary = pepl_eval::run_tests(&prog).unwrap();
    assert_eq!(summary.passed, 3);
    assert_eq!(summary.failed, 0);
}

#[test]
fn test_runner_action_with_params() {
    let prog = parse(r#"
space TodoList {
  state {
    todos: list<{ text: string, done: bool }> = []
    input: string = ""
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

  view main() -> Surface { Column { } { } }
}

tests {
  test "add todo" {
    update_input("Buy milk")
    add_todo()
    assert list.length(todos) == 1
    assert input == ""
  }
}
"#);
    let summary = pepl_eval::run_tests(&prog).unwrap();
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 0);
}

#[test]
fn test_runner_fresh_state_per_test() {
    let prog = parse(r#"
space Counter {
  state { count: number = 0 }
  action increment() { set count = count + 1 }
  view main() -> Surface { Column { } { } }
}

tests {
  test "first test" {
    increment()
    increment()
    assert count == 2
  }

  test "second test starts fresh" {
    assert count == 0
    increment()
    assert count == 1
  }
}
"#);
    let summary = pepl_eval::run_tests(&prog).unwrap();
    assert_eq!(summary.passed, 2);
    assert_eq!(summary.failed, 0);
}

// ══════════════════════════════════════════════════════════════════════════════
// Game Loop (update / handleEvent)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn game_loop_update() {
    let prog = parse(r#"
space Timer {
  state {
    elapsed: number = 0
  }

  view main() -> Surface { Column { } { } }

  update(dt: number) {
    set elapsed = elapsed + dt
  }
}
"#);
    let mut si = SpaceInstance::new(&prog).unwrap();
    assert_eq!(si.get_state("elapsed"), Some(&num(0.0)));

    si.call_update(0.016).unwrap();
    si.call_update(0.016).unwrap();

    let elapsed = si.get_state("elapsed").unwrap().clone();
    if let Value::Number(e) = elapsed {
        assert!((e - 0.032).abs() < 0.001);
    }
}

#[test]
fn game_loop_handle_event() {
    let prog = parse(r#"
space Game {
  state {
    last_event: string = "none"
  }

  view main() -> Surface { Column { } { } }

  handleEvent(event: InputEvent) {
    set last_event = "received"
  }
}
"#);
    let mut si = SpaceInstance::new(&prog).unwrap();
    assert_eq!(si.get_state("last_event"), Some(&s("none")));

    let event = Value::Record {
        type_name: None,
        fields: {
            let mut m = BTreeMap::new();
            m.insert("type".into(), s("tap"));
            m
        },
    };
    si.call_handle_event(event).unwrap();
    assert_eq!(si.get_state("last_event"), Some(&s("received")));
}

#[test]
fn game_loop_update_with_invariant() {
    let prog = parse(r#"
space Timer {
  state {
    elapsed: number = 0
  }

  invariant bounded { elapsed <= 1.0 }

  view main() -> Surface { Column { } { } }

  update(dt: number) {
    set elapsed = elapsed + dt
  }
}
"#);
    let mut si = SpaceInstance::new(&prog).unwrap();

    let r = si.call_update(0.5).unwrap();
    assert!(r.committed);
    assert_eq!(si.get_state("elapsed"), Some(&num(0.5)));

    // This would exceed 1.0 → rollback
    let r = si.call_update(0.6).unwrap();
    assert!(!r.committed);
    assert_eq!(si.get_state("elapsed"), Some(&num(0.5)));
}

// ══════════════════════════════════════════════════════════════════════════════
// Determinism
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn determinism_action_dispatch_100_iterations() {
    for _ in 0..100 {
        let mut si = instance(COUNTER_SOURCE);
        si.dispatch("increment", vec![]).unwrap();
        si.dispatch("increment", vec![]).unwrap();
        si.dispatch("increment", vec![]).unwrap();
        si.dispatch("decrement", vec![]).unwrap();
        assert_eq!(si.get_state("count"), Some(&num(2.0)));
    }
}

#[test]
fn determinism_expression_eval_100_iterations() {
    for _ in 0..100 {
        let mut si = instance(UNIT_CONVERTER_SOURCE);
        si.dispatch("set_value", vec![s("100")]).unwrap();
        let nodes = si.render().unwrap();
        let km_text = &nodes[0].children[1];
        assert_eq!(
            km_text.props.get("value"),
            Some(&s("km to miles: 62.14"))
        );
    }
}

#[test]
fn determinism_all_canonical_100_iterations() {
    for _ in 0..100 {
        // Counter
        let mut si = instance(COUNTER_SOURCE);
        si.dispatch("increment", vec![]).unwrap();
        assert_eq!(si.get_state("count"), Some(&num(1.0)));

        // TodoList
        let mut si = instance(TODO_LIST_SOURCE);
        si.dispatch("update_input", vec![s("test")]).unwrap();
        si.dispatch("add_todo", vec![]).unwrap();
        assert_eq!(si.get_state("remaining"), Some(&num(1.0)));

        // UnitConverter
        let mut si = instance(UNIT_CONVERTER_SOURCE);
        si.dispatch("set_value", vec![s("1")]).unwrap();
        assert_eq!(si.get_state("value"), Some(&num(1.0)));

        // QuizApp
        let mut si = instance(QUIZ_APP_SOURCE);
        si.dispatch("select_answer", vec![s("4")]).unwrap();
        assert_eq!(si.get_state("score"), Some(&num(1.0)));
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Golden Reference — State + Surface JSON capture
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn golden_counter() {
    let mut si = instance(COUNTER_SOURCE);
    let init_json = serde_json::to_value(state_to_json(&si.state_snapshot())).unwrap();
    assert_eq!(init_json["count"], 0);

    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    si.dispatch("increment", vec![]).unwrap();
    let surf = si.render().unwrap();
    let surf_json = SpaceInstance::surface_to_json(&surf);
    let surf_str = serde_json::to_string(&surf_json).unwrap();
    assert!(surf_str.contains("Count: 3"));
}

#[test]
fn golden_todo_list() {
    let mut si = instance(TODO_LIST_SOURCE);
    si.dispatch("update_input", vec![s("Task A")]).unwrap();
    si.dispatch("add_todo", vec![]).unwrap();
    si.dispatch("update_input", vec![s("Task B")]).unwrap();
    si.dispatch("add_todo", vec![]).unwrap();
    si.dispatch("toggle", vec![num(0.0)]).unwrap();

    // remaining is a derived field, accessed via get_state (not state_snapshot)
    assert_eq!(si.get_state("remaining"), Some(&num(1.0)));

    let surf = si.render().unwrap();
    let surf_json = SpaceInstance::surface_to_json(&surf);
    let surf_str = serde_json::to_string(&surf_json).unwrap();
    assert!(surf_str.contains("Task A"));
    assert!(surf_str.contains("Task B"));
    assert!(surf_str.contains("1 remaining"));
}

#[test]
fn golden_unit_converter() {
    let mut si = instance(UNIT_CONVERTER_SOURCE);
    si.dispatch("set_value", vec![s("100")]).unwrap();

    let surf = si.render().unwrap();
    let surf_json = SpaceInstance::surface_to_json(&surf);
    let surf_str = serde_json::to_string(&surf_json).unwrap();
    assert!(surf_str.contains("62.14"));
    assert!(surf_str.contains("160.93"));
    assert!(surf_str.contains("212"));
    assert!(surf_str.contains("37.8"));
}

#[test]
fn golden_pomodoro_timer() {
    let mut si = instance(POMODORO_SOURCE);
    si.dispatch("start_work", vec![]).unwrap();
    for _ in 0..5 {
        si.dispatch("tick", vec![]).unwrap();
    }

    let snap = si.state_snapshot();
    assert_eq!(snap.get("mode"), Some(&s("work")));
    assert_eq!(snap.get("seconds_left"), Some(&num(1495.0)));

    let surf = si.render().unwrap();
    let surf_json = SpaceInstance::surface_to_json(&surf);
    let surf_str = serde_json::to_string(&surf_json).unwrap();
    assert!(surf_str.contains("24:55"));
}

#[test]
fn golden_quiz_app() {
    let mut si = instance(QUIZ_APP_SOURCE);
    si.dispatch("select_answer", vec![s("4")]).unwrap();
    si.dispatch("next_question", vec![]).unwrap();
    si.dispatch("select_answer", vec![s("Paris")]).unwrap();
    si.dispatch("next_question", vec![]).unwrap();
    si.dispatch("select_answer", vec![s("Jupiter")]).unwrap();
    si.dispatch("next_question", vec![]).unwrap();

    let snap = si.state_snapshot();
    assert_eq!(snap.get("score"), Some(&num(3.0)));
    assert_eq!(snap.get("current_question"), Some(&num(3.0)));

    let surf = si.render().unwrap();
    let surf_json = SpaceInstance::surface_to_json(&surf);
    let surf_str = serde_json::to_string(&surf_json).unwrap();
    assert!(surf_str.contains("Quiz Complete!"));
    assert!(surf_str.contains("Score: 3 / 3"));
}

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

fn state_to_json(state: &BTreeMap<String, Value>) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in state {
        map.insert(k.clone(), SpaceInstance::value_to_json_public(v));
    }
    serde_json::Value::Object(map)
}

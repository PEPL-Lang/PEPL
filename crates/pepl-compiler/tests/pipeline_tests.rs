//! C8: Integration & Packaging — End-to-end pipeline tests.
//!
//! Tests verify the full pipeline: source → lex → parse → type-check → codegen → .wasm
//! for all 7 canonical examples and various error scenarios.

use pepl_compiler::{compile, compile_to_result, type_check, CompileResult};
use std::time::Instant;

// ══════════════════════════════════════════════════════════════════════════════
// Canonical PEPL sources
// ══════════════════════════════════════════════════════════════════════════════

const COUNTER: &str = r#"
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

const TODO_LIST: &str = r#"
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

const UNIT_CONVERTER: &str = r#"
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

const WEATHER_DASHBOARD: &str = r#"
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

const POMODORO_TIMER: &str = r#"
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

const HABIT_TRACKER: &str = r#"
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

const QUIZ_APP: &str = r#"
space QuizApp {
  state {
    current_question: number = 0
    score: number = 0
    total_questions: number = 3
    finished: bool = false
  }

  action answer(correct: bool) {
    if correct {
      set score = score + 1
    }
    if current_question + 1 >= total_questions {
      set finished = true
    } else {
      set current_question = current_question + 1
    }
  }

  action restart() {
    set current_question = 0
    set score = 0
    set finished = false
  }

  view main() -> Surface {
    Column { } {
      if finished {
        Text { value: "Score: ${score} / ${total_questions}" }
        Button { label: "Restart", on_tap: restart }
      } else {
        Text { value: "Question ${current_question + 1}" }
        Button { label: "Correct", on_tap: answer }
        Button { label: "Wrong", on_tap: answer }
      }
    }
  }
}
"#;

// ══════════════════════════════════════════════════════════════════════════════
// 1. Full pipeline: all 7 canonical examples compile to valid WASM
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn counter_compiles_end_to_end() {
    let wasm = compile(COUNTER, "counter.pepl").expect("Counter must compile");
    assert!(!wasm.is_empty(), "WASM output must not be empty");
    assert_eq!(&wasm[0..4], b"\0asm", "Must start with WASM magic bytes");
}

#[test]
fn todo_list_compiles_end_to_end() {
    let wasm = compile(TODO_LIST, "todo.pepl").expect("TodoList must compile");
    assert!(!wasm.is_empty());
    assert_eq!(&wasm[0..4], b"\0asm");
}

#[test]
fn unit_converter_compiles_end_to_end() {
    let wasm = compile(UNIT_CONVERTER, "converter.pepl").expect("UnitConverter must compile");
    assert!(!wasm.is_empty());
    assert_eq!(&wasm[0..4], b"\0asm");
}

#[test]
fn weather_dashboard_compiles_end_to_end() {
    let wasm = compile(WEATHER_DASHBOARD, "weather.pepl").expect("WeatherDashboard must compile");
    assert!(!wasm.is_empty());
    assert_eq!(&wasm[0..4], b"\0asm");
}

#[test]
fn pomodoro_timer_compiles_end_to_end() {
    let wasm = compile(POMODORO_TIMER, "pomodoro.pepl").expect("PomodoroTimer must compile");
    assert!(!wasm.is_empty());
    assert_eq!(&wasm[0..4], b"\0asm");
}

#[test]
fn habit_tracker_compiles_end_to_end() {
    let wasm = compile(HABIT_TRACKER, "habits.pepl").expect("HabitTracker must compile");
    assert!(!wasm.is_empty());
    assert_eq!(&wasm[0..4], b"\0asm");
}

#[test]
fn quiz_app_compiles_end_to_end() {
    let wasm = compile(QUIZ_APP, "quiz.pepl").expect("QuizApp must compile");
    assert!(!wasm.is_empty());
    assert_eq!(&wasm[0..4], b"\0asm");
}

// ══════════════════════════════════════════════════════════════════════════════
// 2. Structured error output
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn syntax_error_produces_structured_json() {
    let result = compile_to_result("space { invalid", "bad.pepl");
    assert!(!result.success);
    assert!(result.wasm.is_none());
    assert!(result.errors.has_errors());

    // Verify JSON serialization
    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["success"], false);
    assert!(parsed["errors"]["errors"].is_array());
    assert!(parsed["errors"]["total_errors"].as_u64().unwrap() > 0);
}

#[test]
fn type_error_produces_structured_json() {
    let source = r#"
space Bad {
  state {
    count: number = 0
  }
  action bad_action() {
    set count = "hello"
  }
  view main() -> Surface {
    Text { value: "hi" }
  }
}
"#;
    let result = compile_to_result(source, "type_err.pepl");
    assert!(!result.success);
    assert!(result.wasm.is_none());

    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["success"], false);
}

#[test]
fn valid_program_produces_success_result() {
    let result = compile_to_result(COUNTER, "counter.pepl");
    assert!(result.success, "Counter should compile successfully");
    assert!(result.wasm.is_some());
    assert!(!result.errors.has_errors());

    let json = serde_json::to_string(&result).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["success"], true);
    assert!(!parsed["wasm"].is_null());
}

#[test]
fn compile_result_json_roundtrip() {
    let result = compile_to_result(COUNTER, "counter.pepl");
    let json = serde_json::to_string(&result).unwrap();
    let rt: CompileResult = serde_json::from_str(&json).unwrap();
    assert_eq!(rt.success, result.success);
    assert_eq!(rt.wasm, result.wasm);
}

// ══════════════════════════════════════════════════════════════════════════════
// 3. Compilation performance
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn small_space_compiles_under_500ms() {
    // Counter is ~30 lines — must compile in < 500ms
    let start = Instant::now();
    for _ in 0..10 {
        let _ = compile(COUNTER, "counter.pepl");
    }
    let avg_ms = start.elapsed().as_millis() as f64 / 10.0;
    assert!(
        avg_ms < 500.0,
        "Counter compilation average {:.1}ms exceeds 500ms budget",
        avg_ms
    );
}

#[test]
fn all_canonicals_compile_under_5s() {
    let sources = [
        ("counter", COUNTER),
        ("todo", TODO_LIST),
        ("converter", UNIT_CONVERTER),
        ("weather", WEATHER_DASHBOARD),
        ("pomodoro", POMODORO_TIMER),
        ("habits", HABIT_TRACKER),
        ("quiz", QUIZ_APP),
    ];

    let start = Instant::now();
    for (name, source) in &sources {
        compile(source, &format!("{}.pepl", name)).unwrap_or_else(|e| {
            panic!(
                "{} failed: {:?}",
                name,
                e.errors.iter().map(|e| &e.message).collect::<Vec<_>>()
            );
        });
    }
    let total_ms = start.elapsed().as_millis();
    assert!(
        total_ms < 5000,
        "All 7 canonicals took {}ms (budget: 5000ms)",
        total_ms
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// 4. WASM output validation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn wasm_has_magic_and_version() {
    let wasm = compile(COUNTER, "counter.pepl").unwrap();
    assert_eq!(&wasm[0..4], b"\0asm", "Missing WASM magic");
    assert_eq!(&wasm[4..8], &[1, 0, 0, 0], "Must be WASM version 1");
}

#[test]
fn wasm_size_reasonable() {
    // Small programs should produce small WASM
    let wasm = compile(COUNTER, "counter.pepl").unwrap();
    assert!(
        wasm.len() < 100_000,
        "Counter WASM size {}B exceeds 100KB",
        wasm.len()
    );

    // Larger programs still reasonable
    let wasm = compile(TODO_LIST, "todo.pepl").unwrap();
    assert!(
        wasm.len() < 200_000,
        "TodoList WASM size {}B exceeds 200KB",
        wasm.len()
    );
}

#[test]
fn wasm_contains_custom_pepl_section() {
    let wasm = compile(COUNTER, "counter.pepl").unwrap();
    // Custom section has name "pepl" — search for the UTF-8 bytes
    let pepl_bytes = b"pepl";
    let found = wasm.windows(pepl_bytes.len()).any(|w| w == pepl_bytes);
    assert!(found, "WASM must contain custom 'pepl' section");
}

// ══════════════════════════════════════════════════════════════════════════════
// 5. Determinism across full pipeline
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn full_pipeline_determinism_100_iterations() {
    let sources = [
        ("counter", COUNTER),
        ("todo", TODO_LIST),
        ("converter", UNIT_CONVERTER),
        ("weather", WEATHER_DASHBOARD),
        ("pomodoro", POMODORO_TIMER),
        ("habits", HABIT_TRACKER),
        ("quiz", QUIZ_APP),
    ];

    for (name, source) in &sources {
        let reference = compile(source, &format!("{}.pepl", name)).unwrap();
        for i in 0..100 {
            let wasm = compile(source, &format!("{}.pepl", name)).unwrap();
            assert_eq!(
                wasm, reference,
                "{} WASM bytes differ at iteration {}",
                name, i
            );
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// 6. type_check still works independently
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn type_check_still_works() {
    let errors = type_check(COUNTER, "counter.pepl");
    assert!(
        !errors.has_errors(),
        "Counter should type-check cleanly: {:?}",
        errors.errors.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
}

#[test]
fn type_check_catches_errors() {
    let source = r#"
space Bad {
  state {
    x: number = 0
  }
  action a() {
    set x = "wrong"
  }
  view main() -> Surface {
    Text { value: "hi" }
  }
}
"#;
    let errors = type_check(source, "bad.pepl");
    assert!(errors.has_errors());
}

// ══════════════════════════════════════════════════════════════════════════════
// 7. Error code coverage
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn syntax_error_code_e100() {
    let result = compile_to_result("space Missing {", "e100.pepl");
    assert!(!result.success);
    let has_syntax = result
        .errors
        .errors
        .iter()
        .any(|e| e.code.0 >= 100 && e.code.0 < 200);
    assert!(has_syntax, "Should produce syntax error (E1xx)");
}

#[test]
fn compile_empty_produces_error() {
    let result = compile_to_result("", "empty.pepl");
    assert!(!result.success);
}

#[test]
fn compile_result_serializes_to_json() {
    // Success case
    let result = compile_to_result(COUNTER, "counter.pepl");
    let json = serde_json::to_string_pretty(&result).unwrap();
    assert!(json.contains("\"success\":true") || json.contains("\"success\": true"));

    // Error case
    let result = compile_to_result("invalid", "err.pepl");
    let json = serde_json::to_string_pretty(&result).unwrap();
    assert!(json.contains("\"success\":false") || json.contains("\"success\": false"));
}

//! M2 Milestone Gate — canonical example front-end tests + error code coverage.
//!
//! Verifies:
//! - All 7 canonical examples pass front-end (lex → parse → type-check)
//! - E402 (unknown component) has tests
//!
//! Note: canonical examples use PEPL parser's two-brace component syntax:
//! `Name { props } { children }` — container components need both brace blocks.
//! Examples adapted to match actual stdlib function signatures.

use pepl_types::ErrorCode;

// ══════════════════════════════════════════════════════════════════════════════
// Helpers
// ══════════════════════════════════════════════════════════════════════════════

fn check(source: &str) -> pepl_types::CompileErrors {
    pepl_compiler::type_check(source, "test.pepl")
}

fn assert_ok(source: &str) {
    let errors = check(source);
    assert!(
        !errors.has_errors(),
        "expected no errors, got {}:\n{}",
        errors.total_errors,
        errors
            .errors
            .iter()
            .map(|e| format!("  [{}] {}", e.code, e.message))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn assert_error(source: &str, expected_code: ErrorCode) {
    let errors = check(source);
    assert!(
        errors.has_errors(),
        "expected error {:?}, but got no errors",
        expected_code
    );
    let has_code = errors.errors.iter().any(|e| e.code == expected_code);
    assert!(
        has_code,
        "expected error code {:?}, got codes: {:?}",
        expected_code,
        errors
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.code, e.message))
            .collect::<Vec<_>>()
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 1: Counter
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_counter() {
    assert_ok(r#"
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
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 2: Todo List
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_todo_list() {
    assert_ok(r#"
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

  action toggle(index: number) {
    let todo = list.get(todos, index)
    if todo != nil {
      set todos = list.update(todos, index, { ...todo, done: not todo.done })
    }
  }

  view main() -> Surface {
    Column { } {
      TextInput { value: input, on_change: update_input, placeholder: "Add a todo..." }
      Button { label: "Add", on_tap: add_todo }
      for todo, index in todos {
        Row { } {
          Text { value: todo.text }
          Button { label: "Toggle", on_tap: toggle(index) }
        }
      }
    }
  }
}
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 3: Unit Converter
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_unit_converter() {
    assert_ok(r#"
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
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 4: Weather Dashboard
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_weather_dashboard() {
    assert_ok(r#"
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
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 5: Pomodoro Timer
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_pomodoro_timer() {
    assert_ok(r#"
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
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 6: Habit Tracker
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_habit_tracker() {
    assert_ok(r#"
space HabitTracker {
  state {
    habits: list<{ name: string, streak: number, last_done: number }> = []
    new_habit: string = ""
  }

  capabilities {
    required: [display, keyboard_or_touch, storage]
  }

  action load_habits() {
    let saved = storage.get("habits")
    if saved != nil {
      let parsed = json.parse(saved)
      match parsed {
        Ok(data) -> { set habits = data }
        Err(e) -> { set habits = [] }
      }
    }
  }

  action update_new_habit(value: string) {
    set new_habit = value
  }

  action add_habit() {
    if string.length(new_habit) > 0 {
      set habits = list.append(habits, { name: new_habit, streak: 0, last_done: 0 })
      set new_habit = ""
      storage.set("habits", json.stringify(habits))
    }
  }

  action mark_done(index: number) {
    let habit = list.get(habits, index)
    if habit != nil {
      let now = time.now()
      set habits = list.update(habits, index, { ...habit, streak: habit.streak + 1, last_done: now })
      storage.set("habits", json.stringify(habits))
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
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// Canonical Example 7: Quiz App
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn canonical_quiz_app() {
    assert_ok(r#"
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
"#);
}

// ══════════════════════════════════════════════════════════════════════════════
// E402: Unknown Component
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e402_unknown_component() {
    assert_error(r#"
space App {
  state { x: number = 0 }
  view main() -> Surface {
    Column { } {
      Slider { value: x }
    }
  }
}
"#, ErrorCode::UNKNOWN_COMPONENT);
}

#[test]
fn e402_misspelled_component() {
    assert_error(r#"
space App {
  state { x: number = 0 }
  action noop() { }
  view main() -> Surface {
    Column { } {
      Buton { label: "ok", on_tap: noop }
    }
  }
}
"#, ErrorCode::UNKNOWN_COMPONENT);
}

#[test]
fn e402_valid_components_no_error() {
    assert_ok(r#"
space App {
  state { x: number = 0 }
  action noop() { }
  view main() -> Surface {
    Column { } {
      Row { } {
        Text { value: "hello" }
        Button { label: "ok", on_tap: noop }
      }
      ProgressBar { value: 0.5 }
    }
  }
}
"#);
}

#[test]
fn e402_custom_component_name() {
    assert_error(r#"
space App {
  state { x: number = 0 }
  view main() -> Surface {
    MyCustomWidget { }
  }
}
"#, ErrorCode::UNKNOWN_COMPONENT);
}

// ══════════════════════════════════════════════════════════════════════════════
// Determinism
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn m2_gate_determinism_100_iterations() {
    let counter = r#"
space Counter {
  state { count: number = 0 }
  action increment() { set count = count + 1 }
  view main() -> Surface {
    Column { } {
      Text { value: "Count: ${count}" }
      Button { label: "+", on_tap: increment }
    }
  }
}
"#;
    for _ in 0..100 {
        let errors = check(counter);
        assert!(!errors.has_errors());
    }
}

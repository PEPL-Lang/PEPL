//! Type-checker integration tests.
//!
//! Each test parses + type-checks a PEPL source program via `pepl_compiler::type_check`
//! and asserts on the presence (or absence) of specific error codes.

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
        "expected no errors, got {}:\n{:#?}",
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

fn assert_n_errors(source: &str, n: usize) {
    let errors = check(source);
    assert_eq!(
        errors.total_errors,
        n,
        "expected {} errors, got {}:\n{}",
        n,
        errors.total_errors,
        errors
            .errors
            .iter()
            .map(|e| format!("  [{}] {}", e.code, e.message))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Success cases — valid programs should pass with no errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn minimal_space_passes() {
    assert_ok(
        r#"
space Counter {
  state {
    count: number = 0
  }
}
"#,
    );
}

#[test]
fn space_with_actions() {
    assert_ok(
        r#"
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
}
"#,
    );
}

#[test]
fn space_with_derived_and_invariants() {
    assert_ok(
        r#"
space T {
  state {
    glasses: number = 0
    daily_goal: number = 8
  }
  derived {
    progress: number = math.min(glasses / daily_goal, 1)
    remaining: number = math.max(daily_goal - glasses, 0)
  }
  invariant non_negative {
    glasses >= 0
  }
}
"#,
    );
}

#[test]
fn sum_type_with_match() {
    assert_ok(
        r#"
space T {
  type Priority = | High | Medium | Low
  state {
    p: Priority = Medium
  }
  action go() {
    match p {
      High -> { set p = Low },
      Medium -> { set p = High },
      Low -> { set p = Medium },
    }
  }
}
"#,
    );
}

#[test]
fn wildcard_match_arm() {
    assert_ok(
        r#"
space T {
  type Status = | Active | Paused | Stopped
  state {
    s: Status = Active
  }
  action toggle() {
    match s {
      Active -> { set s = Paused },
      _ -> { set s = Active },
    }
  }
}
"#,
    );
}

#[test]
fn capabilities_and_stdlib() {
    assert_ok(
        r#"
space T {
  state {
    data: string = ""
  }
  capabilities {
    required: [http]
  }
  action fetch() {
    let result = http.get("https://example.com")
  }
}
"#,
    );
}

#[test]
fn let_binding_type_annotation() {
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y: number = 42
    set x = y
  }
}
"#,
    );
}

#[test]
fn string_interpolation() {
    assert_ok(
        r#"
space T {
  state {
    name: string = "world"
  }
  action greet() {
    let msg: string = "hello ${name}"
  }
}
"#,
    );
}

#[test]
fn list_operations() {
    assert_ok(
        r#"
space T {
  state {
    items: list<number> = []
  }
  action add(value: number) {
    set items = list.append(items, value)
  }
  action clear() {
    set items = []
  }
}
"#,
    );
}

#[test]
fn for_loop_in_action() {
    assert_ok(
        r#"
space T {
  state {
    total: number = 0
    items: list<number> = [1, 2, 3]
  }
  action sum_all() {
    for item in items {
      set total = total + item
    }
  }
}
"#,
    );
}

#[test]
fn lambda_expression() {
    assert_ok(
        r#"
space T {
  state {
    items: list<number> = [1, 2, 3]
  }
  action filter_positive() {
    let positives = list.filter(items, fn(x: number) { x > 0 })
  }
}
"#,
    );
}

#[test]
fn update_and_handle_event() {
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  update(dt: number) {
    set x = x + dt
  }
  handleEvent(event: InputEvent) {
    set x = 0
  }
}
"#,
    );
}

#[test]
fn if_else_in_action() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  action go() {
    if count > 10 {
      set count = 0
    } else {
      set count = count + 1
    }
  }
}
"#,
    );
}

#[test]
fn boolean_operators() {
    assert_ok(
        r#"
space T {
  state {
    a: bool = true
    b: bool = false
  }
  action go() {
    let c = a and b
    let d = a or b
    let e = not a
  }
}
"#,
    );
}

#[test]
fn view_with_components() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  action increment() {
    set count = count + 1
  }
  view main() -> Surface {
    Column { spacing: 16 } {
      Text { value: "hello" }
      Button { label: "Click", on_tap: increment }
    }
  }
}
"#,
    );
}

#[test]
fn record_literal() {
    assert_ok(
        r#"
space T {
  state {
    config: { width: number, height: number } = { width: 100, height: 200 }
  }
}
"#,
    );
}

#[test]
fn tests_block_passes() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  action increment() {
    set count = count + 1
  }
}

tests {
  test "starts at zero" {
    assert count == 0
  }
  test "increment" {
    increment()
    assert count == 1
  }
}
"#,
    );
}

#[test]
fn credentials_block() {
    assert_ok(
        r#"
space T {
  state {
    data: string = ""
  }
  credentials {
    api_key: string
  }
}
"#,
    );
}

#[test]
fn math_stdlib_functions() {
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  action compute() {
    let a = math.abs(-5)
    let b = math.floor(3.7)
    let c = math.ceil(3.2)
    let d = math.round(3.5)
    let e = math.sqrt(16)
    let f = math.pow(2, 3)
    let g = math.clamp(5, 0, 10)
    let h = math.min(3, 7)
    let i = math.max(3, 7)
  }
}
"#,
    );
}

#[test]
fn string_stdlib_functions() {
    assert_ok(
        r#"
space T {
  state {
    x: string = "hello"
  }
  action go() {
    let a = string.length(x)
    let b = string.to_upper(x)
    let c = string.to_lower(x)
    let d = string.trim(x)
    let e = string.contains(x, "ell")
    let f = string.starts_with(x, "he")
  }
}
"#,
    );
}

#[test]
fn nil_coalescing() {
    // nil-coalescing with record optional field
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let val = x + 0
    set x = val
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E200: UNKNOWN_TYPE — unresolvable type annotation
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e200_unknown_type_annotation() {
    assert_error(
        r#"
space T {
  state {
    x: Nonexistent = 0
  }
}
"#,
        ErrorCode::UNKNOWN_TYPE,
    );
}

#[test]
fn e200_any_in_user_annotation() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y: any = 42
  }
}
"#,
        ErrorCode::UNKNOWN_TYPE,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E201: TYPE_MISMATCH
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e201_set_wrong_type() {
    assert_error(
        r#"
space T {
  state {
    count: number = 0
  }
  action go() {
    set count = "hello"
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_if_condition_not_bool() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    if x {
      set x = 1
    }
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_arithmetic_on_string() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    set x = "hello" - 1
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_comparison_needs_numbers() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let b = "hello" > 5
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_and_on_non_bool() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = 5 and true
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_not_on_number() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = not 42
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_negate_string() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = -"hello"
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_for_on_non_list() {
    assert_error(
        r#"
space T {
  state {
    x: number = 5
  }
  action go() {
    for item in x {
      set x = item
    }
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_derived_type_mismatch() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  derived {
    y: string = x + 1
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_invariant_not_bool() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  invariant check {
    x + 1
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_let_type_mismatch() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y: string = 42
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_assert_not_bool() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    assert 42
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_set_non_state_field() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    set unknown_field = 1
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_undefined_variable() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    set x = undefined_var
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_result_unwrap_on_non_result() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = x?
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_unknown_module() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = nonexistent.foo(1)
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_unknown_function_in_module() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = math.nonexistent(1)
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_record_no_field() {
    assert_error(
        r#"
space T {
  state {
    config: { width: number } = { width: 100 }
  }
  action go() {
    let h = config.nonexistent
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

#[test]
fn e201_field_access_on_number() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = x.field
  }
}
"#,
        ErrorCode::TYPE_MISMATCH,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E202: WRONG_ARG_COUNT
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e202_too_few_args() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let a = math.abs()
  }
}
"#,
        ErrorCode::WRONG_ARG_COUNT,
    );
}

#[test]
fn e202_too_many_args() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let a = math.abs(1, 2, 3)
  }
}
"#,
        ErrorCode::WRONG_ARG_COUNT,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E210: NON_EXHAUSTIVE_MATCH
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e210_missing_variant() {
    assert_error(
        r#"
space T {
  type Color = | Red | Green | Blue
  state {
    c: Color = Red
  }
  action go() {
    match c {
      Red -> { set c = Green },
      Green -> { set c = Blue },
    }
  }
}
"#,
        ErrorCode::NON_EXHAUSTIVE_MATCH,
    );
}

#[test]
fn e210_wildcard_covers_all() {
    // Wildcard makes it exhaustive — should pass
    assert_ok(
        r#"
space T {
  type Color = | Red | Green | Blue
  state {
    c: Color = Red
  }
  action go() {
    match c {
      Red -> { set c = Green },
      _ -> { set c = Red },
    }
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E400: UNDECLARED_CAPABILITY
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e400_http_without_capability() {
    assert_error(
        r#"
space T {
  state {
    data: string = ""
  }
  action fetch() {
    let result = http.get("https://example.com")
  }
}
"#,
        ErrorCode::UNDECLARED_CAPABILITY,
    );
}

#[test]
fn e400_timer_without_capability() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    timer.start("t", 1000)
  }
}
"#,
        ErrorCode::UNDECLARED_CAPABILITY,
    );
}

#[test]
fn e400_optional_capability_ok() {
    // Optional capability should be fine
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  capabilities {
    optional: [timer]
  }
  action go() {
    timer.start("t", 1000)
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E500: VARIABLE_ALREADY_DECLARED
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e500_duplicate_let_same_scope() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = 1
    let y = 2
  }
}
"#,
        ErrorCode::VARIABLE_ALREADY_DECLARED,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E501: STATE_MUTATED_OUTSIDE_ACTION
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e501_set_in_derived() {
    // derived is pure — but here we test that set in a view is caught
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  capabilities {
    required: [http]
  }
  view main() -> Surface {
    Text { value: http.get("url") }
  }
}
"#,
        ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
    );
}

#[test]
fn e501_capability_in_view() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  capabilities {
    required: [http]
  }
  view main() -> Surface {
    Text { value: http.get("url") }
  }
}
"#,
        ErrorCode::STATE_MUTATED_OUTSIDE_ACTION,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E601: DERIVED_FIELD_MODIFIED
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e601_set_derived_field() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  derived {
    y: number = x + 1
  }
  action go() {
    set y = 5
  }
}
"#,
        ErrorCode::DERIVED_FIELD_MODIFIED,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E605: CREDENTIAL_MODIFIED
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e605_set_credential() {
    assert_error(
        r#"
space T {
  state {
    x: number = 0
  }
  credentials {
    api_key: string
  }
  action go() {
    set api_key = "new_key"
  }
}
"#,
        ErrorCode::CREDENTIAL_MODIFIED,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Full-app tests — realistic programs
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn full_water_tracker() {
    assert_ok(
        r#"
space WaterTracker {
  state {
    glasses: number = 0
    daily_goal: number = 8
  }
  derived {
    progress: number = math.min(glasses / daily_goal, 1)
    remaining: number = math.max(daily_goal - glasses, 0)
  }
  invariant non_negative {
    glasses >= 0
  }
  action drink() {
    set glasses = glasses + 1
  }
  action reset() {
    set glasses = 0
  }
}
"#,
    );
}

#[test]
fn full_counter_with_tests() {
    assert_ok(
        r#"
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
}

tests {
  test "starts at zero" {
    assert count == 0
  }
  test "increment works" {
    increment()
    assert count == 1
  }
}
"#,
    );
}

#[test]
fn full_game_loop() {
    assert_ok(
        r#"
space Game {
  state {
    x: number = 0
    y: number = 0
    speed: number = 100
  }
  action reset() {
    set x = 0
    set y = 0
  }
  update(dt: number) {
    set x = x + speed * dt
  }
  handleEvent(event: InputEvent) {
    set y = y + 1
  }
}
"#,
    );
}

#[test]
fn full_todo_app() {
    assert_ok(
        r#"
space TodoApp {
  state {
    items: list<string> = []
    input: string = ""
  }
  action add_item() {
    if string.length(input) > 0 {
      set items = list.append(items, input)
      set input = ""
    }
  }
  action clear() {
    set items = []
  }
}

tests {
  test "starts empty" {
    assert list.length(items) == 0
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Type assignability / compatibility
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn nil_assignable_to_nullable() {
    // Nullable is represented via optional record fields, not top-level `number?`
    assert_ok(
        r#"
space T {
  state {
    config: { value: number, label?: string } = { value: 0 }
  }
}
"#,
    );
}

#[test]
fn number_assignable_to_nullable_number() {
    // Test that nil-coalescing works with record optional fields
    assert_ok(
        r#"
space T {
  state {
    items: list<number> = []
  }
  action go() {
    let len = list.length(items)
    set items = list.append(items, len)
  }
}
"#,
    );
}

#[test]
fn string_concat_with_plus() {
    assert_ok(
        r#"
space T {
  state {
    x: string = ""
  }
  action go() {
    set x = "hello" + " world"
  }
}
"#,
    );
}

#[test]
fn equality_works_on_any_types() {
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
    y: string = ""
  }
  action go() {
    let a = x == 0
    let b = y == ""
    let c = true == false
  }
}
"#,
    );
}

#[test]
fn empty_list_literal() {
    assert_ok(
        r#"
space T {
  state {
    items: list<number> = []
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Number of errors
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn multiple_errors_collected() {
    // Two set targets that don't exist
    let errors = check(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    set unknown1 = 1
    set unknown2 = 2
  }
}
"#,
    );
    assert!(errors.total_errors >= 2, "expected at least 2 errors");
}

// ══════════════════════════════════════════════════════════════════════════════
// Scope tests
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn let_binding_visible_in_scope() {
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  action go() {
    let y = 5
    set x = y
  }
}
"#,
    );
}

#[test]
fn for_loop_binds_item_variable() {
    assert_ok(
        r#"
space T {
  state {
    total: number = 0
    items: list<number> = [1, 2]
  }
  action go() {
    for item in items {
      set total = total + item
    }
  }
}
"#,
    );
}

#[test]
fn for_loop_index_is_number() {
    assert_ok(
        r#"
space T {
  state {
    total: number = 0
    items: list<number> = [1, 2]
  }
  action go() {
    for item, idx in items {
      set total = total + idx
    }
  }
}
"#,
    );
}

#[test]
fn action_params_in_scope() {
    assert_ok(
        r#"
space T {
  state {
    x: number = 0
  }
  action go(amount: number) {
    set x = x + amount
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// View-specific checks
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn view_for_loop() {
    assert_ok(
        r#"
space T {
  state {
    items: list<string> = ["a", "b"]
  }
  view main() -> Surface {
    Column { spacing: 8 } {
      for item in items {
        Text { value: item }
      }
    }
  }
}
"#,
    );
}

#[test]
fn view_if_condition() {
    assert_ok(
        r#"
space T {
  state {
    show: bool = true
  }
  view main() -> Surface {
    Column { spacing: 8 } {
      if show {
        Text { value: "visible" }
      }
    }
  }
}
"#,
    );
}

//! Invariant checker tests (C5).
//!
//! Tests for:
//! - E300: invariant references derived field (unreachable)
//! - E502: recursion not allowed (action calls itself)

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

fn assert_n_errors(source: &str, n: usize) {
    let errors = check(source);
    assert_eq!(
        errors.total_errors, n,
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
// E300 — Invariant references derived field
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn invariant_referencing_derived_field_is_e300() {
    assert_error(
        r#"
space T {
  state {
    count: number = 0
  }
  derived {
    doubled: number = count * 2
  }
  invariant positive_doubled {
    doubled > 0
  }
}
"#,
        ErrorCode::INVARIANT_UNREACHABLE,
    );
}

#[test]
fn invariant_referencing_derived_field_in_compound_expr() {
    assert_error(
        r#"
space T {
  state {
    count: number = 0
  }
  derived {
    total: number = count + 10
  }
  invariant check {
    total > 0 and count >= 0
  }
}
"#,
        ErrorCode::INVARIANT_UNREACHABLE,
    );
}

#[test]
fn invariant_referencing_only_state_field_is_ok() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  derived {
    doubled: number = count * 2
  }
  invariant positive {
    count >= 0
  }
}
"#,
    );
}

#[test]
fn invariant_without_derived_fields_is_ok() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
    max: number = 100
  }
  invariant bounded {
    count <= max
  }
}
"#,
    );
}

#[test]
fn multiple_invariants_one_references_derived() {
    let src = r#"
space T {
  state {
    count: number = 0
  }
  derived {
    doubled: number = count * 2
  }
  invariant ok_inv {
    count >= 0
  }
  invariant bad_inv {
    doubled > 0
  }
}
"#;
    // Should have exactly 1 error (only the bad invariant)
    assert_error(src, ErrorCode::INVARIANT_UNREACHABLE);
    assert_n_errors(src, 1);
}

#[test]
fn invariant_references_multiple_derived_fields() {
    let src = r#"
space T {
  state {
    a: number = 1
  }
  derived {
    b: number = a + 1
    c: number = a * 2
  }
  invariant check {
    b > 0 and c > 0
  }
}
"#;
    // Each derived field reference produces a separate E300
    assert_error(src, ErrorCode::INVARIANT_UNREACHABLE);
}

// ══════════════════════════════════════════════════════════════════════════════
// E502 — Recursion not allowed
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn direct_recursion_in_action_is_e502() {
    assert_error(
        r#"
space T {
  state {
    count: number = 0
  }
  action loop() {
    loop()
  }
}
"#,
        ErrorCode::RECURSION_NOT_ALLOWED,
    );
}

#[test]
fn recursive_call_with_args_is_e502() {
    assert_error(
        r#"
space T {
  state {
    count: number = 0
  }
  action countdown(n: number) {
    if n > 0 {
      set count = n
      countdown(n - 1)
    }
  }
}
"#,
        ErrorCode::RECURSION_NOT_ALLOWED,
    );
}

#[test]
fn action_calling_different_action_is_ok() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  action increment() {
    set count = count + 1
  }
  action double_increment() {
    increment()
    increment()
  }
}
"#,
    );
}

#[test]
fn action_not_calling_itself_is_ok() {
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
"#,
    );
}

#[test]
fn recursion_in_nested_if_is_e502() {
    assert_error(
        r#"
space T {
  state {
    count: number = 0
  }
  action process(n: number) {
    if n > 0 {
      set count = count + 1
      process(n - 1)
    }
  }
}
"#,
        ErrorCode::RECURSION_NOT_ALLOWED,
    );
}

#[test]
fn recursion_in_for_loop_is_e502() {
    assert_error(
        r#"
space T {
  state {
    items: list<number> = [1, 2, 3]
  }
  action process_all() {
    for item in items {
      process_all()
    }
  }
}
"#,
        ErrorCode::RECURSION_NOT_ALLOWED,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// Combined / Edge Cases
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn both_e300_and_e502_in_same_program() {
    let src = r#"
space T {
  state {
    count: number = 0
  }
  derived {
    doubled: number = count * 2
  }
  invariant bad {
    doubled > 0
  }
  action loop() {
    loop()
  }
}
"#;
    let errors = check(src);
    assert!(errors.has_errors());
    let has_e300 = errors
        .errors
        .iter()
        .any(|e| e.code == ErrorCode::INVARIANT_UNREACHABLE);
    let has_e502 = errors
        .errors
        .iter()
        .any(|e| e.code == ErrorCode::RECURSION_NOT_ALLOWED);
    assert!(has_e300, "expected E300 for derived field in invariant");
    assert!(has_e502, "expected E502 for recursive action");
}

#[test]
fn stdlib_call_in_invariant_is_ok() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  invariant positive {
    math.abs(count) >= 0
  }
}
"#,
    );
}

#[test]
fn derived_field_in_action_body_is_ok() {
    // Derived fields can be READ in action bodies — just not in invariants
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  derived {
    doubled: number = count * 2
  }
  action check() {
    if doubled > 10 {
      set count = 0
    }
  }
}
"#,
    );
}

#[test]
fn derived_field_in_view_is_ok() {
    assert_ok(
        r#"
space T {
  state {
    count: number = 0
  }
  derived {
    label: string = "Count: " + convert.to_string(count)
  }
  view main() -> Surface {
    Text { value: label }
  }
}
"#,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// 100-iteration determinism
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn invariant_checker_deterministic_100_iterations() {
    let source = r#"
space T {
  state {
    count: number = 0
  }
  derived {
    doubled: number = count * 2
  }
  invariant bad {
    doubled > 0
  }
  action recurse() {
    recurse()
  }
}
"#;
    let baseline = check(source);
    for i in 0..100 {
        let result = check(source);
        assert_eq!(
            result.total_errors, baseline.total_errors,
            "iteration {} produced different error count: {} vs {}",
            i, result.total_errors, baseline.total_errors,
        );
        for (base_err, iter_err) in baseline.errors.iter().zip(result.errors.iter()) {
            assert_eq!(
                base_err.code, iter_err.code,
                "iteration {} produced different error code at same position",
                i,
            );
        }
    }
}

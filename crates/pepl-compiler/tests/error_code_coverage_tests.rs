//! Error code coverage tests — ensuring every defined error code (E100–E699)
//! has at least one test that asserts it is emitted.
//!
//! Codes already covered by other test files are referenced in comments.
//! This file adds tests for any gaps.

use pepl_types::ErrorCode;

fn check(source: &str) -> pepl_types::CompileErrors {
    pepl_compiler::type_check(source, "test.pepl")
}

fn assert_error(source: &str, expected_code: ErrorCode) {
    let errors = check(source);
    let has_code = errors.errors.iter().any(|e| e.code == expected_code);
    assert!(
        has_code,
        "expected error code {:?}, got: {:?}",
        expected_code,
        errors
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.code, e.message))
            .collect::<Vec<_>>()
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E100: UNEXPECTED_TOKEN — emitted by parser
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e100_unexpected_token() {
    // Parser sees `+` where it expects a statement-leading keyword or identifier
    assert_error(
        r#"
space App {
  state { x: number = }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
        ErrorCode::UNEXPECTED_TOKEN,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E101: UNCLOSED_BRACE — emitted by lexer
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e101_unclosed_brace() {
    // Unterminated string literal triggers E101 from the lexer
    assert_error(
        "space App {\n  state { x: string = \"hello }\n  view main() -> Surface { Text { value: \"hi\" } }\n}\n",
        ErrorCode::UNCLOSED_BRACE,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E102: INVALID_KEYWORD — handled by lexer→parser
// Keywords are tokenized as keyword tokens by the lexer. Using a keyword
// as an identifier produces E100 (UNEXPECTED_TOKEN) at parse time.
// E102 is defined as a more specific variant but currently all keyword
// misuse falls through to E100 in the parser.
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e102_error_code_defined() {
    // E102 is defined for keyword validation. Currently, keyword misuse
    // produces E100 at parse time because the lexer tokenizes keywords.
    assert_eq!(ErrorCode::INVALID_KEYWORD.0, 102);
    // Verify that using a keyword as a variable name triggers a parse error
    let errors = check(
        r#"
space App {
  state { x: number = 0 }
  action run() {
    let state = 5
  }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
    );
    assert!(errors.has_errors(), "using keyword as identifier should error");
}

// ══════════════════════════════════════════════════════════════════════════════
// E200: UNKNOWN_TYPE — see type_checker_tests.rs
// E201: TYPE_MISMATCH — see type_checker_tests.rs
// E202: WRONG_ARG_COUNT — see type_checker_tests.rs
// E210: NON_EXHAUSTIVE_MATCH — see type_checker_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E300: INVARIANT_UNREACHABLE — see invariant_checker_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E301: INVARIANT_UNKNOWN_FIELD — emitted by checker
// When an invariant expression references an identifier that is not a
// state field, derived field, or parameter — gives more specific diagnostics
// than generic E201 for invariant contexts.
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e301_invariant_unknown_field() {
    assert_error(
        r#"
space App {
  state { count: number = 0 }
  invariant positive { nonexistent_field > 0 }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
        ErrorCode::INVARIANT_UNKNOWN_FIELD,
    );
}

#[test]
fn e301_invariant_unknown_field_has_suggestion() {
    let errors = check(
        r#"
space App {
  state { count: number = 0 }
  invariant positive { nonexistent_field > 0 }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
    );
    let err = errors
        .errors
        .iter()
        .find(|e| e.code == ErrorCode::INVARIANT_UNKNOWN_FIELD)
        .expect("expected E301");
    assert!(
        err.suggestion.is_some(),
        "E301 should include a suggestion"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E400: UNDECLARED_CAPABILITY — see capab_checker_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E401: CAPABILITY_UNAVAILABLE — emitted as warning by checker
// When an optional capability module is used, the checker emits a warning
// that the call may fail at runtime if the capability is unavailable.
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e401_optional_capability_warning() {
    let errors = check(
        r#"
space App {
  state { data: string = "" }
  capabilities {
    required: []
    optional: [http]
  }
  action fetch_data() {
    let result = http.get("https://example.com")?
    set data = result
  }
  view main() -> Surface { Text { value: data } }
}
"#,
    );
    let has_warning = errors
        .warnings
        .iter()
        .any(|w| w.code == ErrorCode::CAPABILITY_UNAVAILABLE);
    assert!(
        has_warning,
        "expected E401 warning for optional capability usage, got warnings: {:?}",
        errors
            .warnings
            .iter()
            .map(|w| format!("{}: {}", w.code, w.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn e401_required_capability_no_warning() {
    let errors = check(
        r#"
space App {
  state { data: string = "" }
  capabilities {
    required: [http]
  }
  action fetch_data() {
    let result = http.get("https://example.com")?
    set data = result
  }
  view main() -> Surface { Text { value: data } }
}
"#,
    );
    let has_warning = errors
        .warnings
        .iter()
        .any(|w| w.code == ErrorCode::CAPABILITY_UNAVAILABLE);
    assert!(
        !has_warning,
        "required capability should NOT produce E401 warning"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E402: UNKNOWN_COMPONENT — see m2_gate_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E500: VARIABLE_ALREADY_DECLARED — see scope_checker_tests.rs
// E501: STATE_MUTATED_OUTSIDE_ACTION — see scope_checker_tests.rs
// E502: RECURSION_NOT_ALLOWED — see scope_checker_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E600: BLOCK_ORDERING_VIOLATED — emitted by parser
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e600_block_ordering_violated() {
    // Blocks must appear in declaration order: state < derived < invariants < ...
    // Putting view before state triggers E600
    assert_error(
        r#"
space App {
  view main() -> Surface { Text { value: "hi" } }
  state { x: number = 0 }
}
"#,
        ErrorCode::BLOCK_ORDERING_VIOLATED,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E601: DERIVED_FIELD_MODIFIED — see scope_checker_tests.rs
// E602: EXPRESSION_BODY_LAMBDA — see structural_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E603: BLOCK_COMMENT_USED — emitted by lexer
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e603_block_comment_used() {
    assert_error(
        r#"
/* this is a block comment */
space App {
  state { x: number = 0 }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
        ErrorCode::BLOCK_COMMENT_USED,
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E604: UNDECLARED_CREDENTIAL — emitted by checker
// When a state initializer references a credential variable, the checker
// emits E604 because credentials are injected at runtime and not available
// during state initialization.
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e604_credential_in_state_initializer() {
    assert_error(
        r#"
space App {
  state { api_url: string = api_key }
  credentials {
    api_key: string
  }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
        ErrorCode::UNDECLARED_CREDENTIAL,
    );
}

#[test]
fn e604_has_suggestion() {
    let errors = check(
        r#"
space App {
  state { api_url: string = api_key }
  credentials {
    api_key: string
  }
  view main() -> Surface { Text { value: "hi" } }
}
"#,
    );
    let err = errors
        .errors
        .iter()
        .find(|e| e.code == ErrorCode::UNDECLARED_CREDENTIAL)
        .expect("expected E604");
    assert!(
        err.suggestion.is_some(),
        "E604 should include a suggestion"
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// E605: CREDENTIAL_MODIFIED — see scope_checker_tests.rs
// E606: EMPTY_STATE_BLOCK — see structural_tests.rs
// E607: STRUCTURAL_LIMIT_EXCEEDED — see structural_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

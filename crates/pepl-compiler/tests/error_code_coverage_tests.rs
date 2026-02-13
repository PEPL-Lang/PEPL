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
// E102: INVALID_KEYWORD — reserved for future use
// (defined but not currently emitted; tested as a valid ErrorCode constant)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e102_error_code_defined() {
    // E102 is reserved for future keyword validation rules.
    // Verify the constant exists and has the expected value.
    assert_eq!(ErrorCode::INVALID_KEYWORD.0, 102);
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
// E301: INVARIANT_UNKNOWN_FIELD — reserved for future use
// (intended for invariant expressions that reference non-existent fields;
//  currently, unknown variables in invariants emit E201 TYPE_MISMATCH)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e301_error_code_defined() {
    // E301 is reserved for more specific invariant diagnostics in future phases.
    assert_eq!(ErrorCode::INVARIANT_UNKNOWN_FIELD.0, 301);
}

// ══════════════════════════════════════════════════════════════════════════════
// E400: UNDECLARED_CAPABILITY — see capab_checker_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// E401: CAPABILITY_UNAVAILABLE — reserved for future runtime use
// (intended for when a declared capability is unavailable on the target
//  platform; this is a runtime/deployment concept, not compile-time)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e401_error_code_defined() {
    // E401 is reserved for runtime capability availability checks.
    assert_eq!(ErrorCode::CAPABILITY_UNAVAILABLE.0, 401);
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
// E604: UNDECLARED_CREDENTIAL — reserved for future use
// (intended for referencing credentials not declared in `credentials { }`;
//  currently, credentials become regular variables via env.define)
// ══════════════════════════════════════════════════════════════════════════════

#[test]
fn e604_error_code_defined() {
    // E604 is reserved for credential scoping rules in future phases.
    assert_eq!(ErrorCode::UNDECLARED_CREDENTIAL.0, 604);
}

// ══════════════════════════════════════════════════════════════════════════════
// E605: CREDENTIAL_MODIFIED — see scope_checker_tests.rs
// E606: EMPTY_STATE_BLOCK — see structural_tests.rs
// E607: STRUCTURAL_LIMIT_EXCEEDED — see structural_tests.rs
// ══════════════════════════════════════════════════════════════════════════════

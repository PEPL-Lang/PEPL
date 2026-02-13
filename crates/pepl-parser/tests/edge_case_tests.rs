//! Grammar edge-case tests for the M1 milestone gate.
//!
//! Covers:
//! 1. Precedence worked examples (from grammar-edge-cases.md)
//! 2. Structural limit enforcement (lambda depth, record depth,
//!    expression depth, for depth, parameter counts)
//! 3. Specific error codes (E602, E606, E607)
//! 4. Edge-case acceptance (empty action body, nested lambdas/records,
//!    result unwrap, nil-coalescing)

use pepl_lexer::Lexer;
use pepl_parser::{ParseResult, Parser};
use pepl_types::ast::*;
use pepl_types::{ErrorCode, SourceFile};

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

/// Parse source and return the result (program + errors).
fn parse(source: &str) -> ParseResult {
    let sf = SourceFile::new("test.pepl", source);
    let lex = Lexer::new(&sf).lex();
    Parser::new(lex.tokens, &sf).parse()
}

/// Parse source and return the program, panicking if there are errors.
fn parse_ok(source: &str) -> Program {
    let result = parse(source);
    if result.errors.has_errors() {
        for e in &result.errors.errors {
            eprintln!("  ERROR: {} ({})", e.message, e.code);
        }
        panic!("unexpected parse errors (see above)");
    }
    result.program.expect("no program returned")
}

/// Parse source and return the error count.
fn error_count(source: &str) -> usize {
    parse(source).errors.total_errors
}

/// Parse source and return all error codes.
fn error_codes(source: &str) -> Vec<ErrorCode> {
    parse(source).errors.errors.iter().map(|e| e.code).collect()
}

/// Wrap an expression in a minimal space's derived block for parsing.
fn wrap_expr(expr: &str) -> String {
    format!(
        r#"space T {{
  state {{
    a: number = 0
    b: number = 0
    c: number = 0
    d: number = 0
    e: boolean = false
    result: number = 0
    value: number = 0
    fallback: number = 0
    items: list<number> = []
  }}
  derived {{
    x: number = {expr}
  }}
}}"#
    )
}

/// Wrap a boolean expression in a minimal space's invariant block.
fn wrap_bool_expr(expr: &str) -> String {
    format!(
        r#"space T {{
  state {{
    a: number = 0
    b: number = 0
    c: number = 0
    d: number = 0
    e: boolean = false
    result: number = 0
    value: number = 0
    fallback: number = 0
  }}
  invariant check {{
    {expr}
  }}
}}"#
    )
}

/// Extract the derived expression from a parsed program.
fn derived_expr(prog: &Program) -> &Expr {
    &prog
        .space
        .body
        .derived
        .as_ref()
        .expect("no derived block")
        .fields[0]
        .value
}

/// Extract the invariant expression from a parsed program.
fn invariant_expr(prog: &Program) -> &Expr {
    &prog.space.body.invariants[0].condition
}

// ═══════════════════════════════════════════════════════════════════════
// 1. Precedence Worked Examples
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_prec_mul_binds_tighter_than_add() {
    // a + b * c  →  a + (b * c)
    let prog = parse_ok(&wrap_expr("a + b * c"));
    let expr = derived_expr(&prog);
    // Top-level should be Binary Add
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::Add);
            assert!(matches!(left.kind, ExprKind::Identifier(ref n) if n == "a"));
            match &right.kind {
                ExprKind::Binary { op: inner_op, left: rl, right: rr, .. } => {
                    assert_eq!(*inner_op, BinOp::Mul);
                    assert!(matches!(rl.kind, ExprKind::Identifier(ref n) if n == "b"));
                    assert!(matches!(rr.kind, ExprKind::Identifier(ref n) if n == "c"));
                }
                other => panic!("expected Binary Mul, got {:?}", other),
            }
        }
        other => panic!("expected Binary Add, got {:?}", other),
    }
}

#[test]
fn test_prec_left_assoc_same_level() {
    // a + b * c - d  →  (a + (b * c)) - d
    let prog = parse_ok(&wrap_expr("a + b * c - d"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::Sub, "top-level should be Sub");
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "d"));
            match &left.kind {
                ExprKind::Binary { op: inner_op, right: inner_right, .. } => {
                    assert_eq!(*inner_op, BinOp::Add);
                    match &inner_right.kind {
                        ExprKind::Binary { op: mul_op, .. } => {
                            assert_eq!(*mul_op, BinOp::Mul);
                        }
                        other => panic!("expected Binary Mul, got {:?}", other),
                    }
                }
                other => panic!("expected Binary Add, got {:?}", other),
            }
        }
        other => panic!("expected Binary Sub, got {:?}", other),
    }
}

#[test]
fn test_prec_comparison_tighter_than_and() {
    // a > b and c < d  →  (a > b) and (c < d)
    let prog = parse_ok(&wrap_bool_expr("a > b and c < d"));
    let expr = invariant_expr(&prog);
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::And);
            match &left.kind {
                ExprKind::Binary { op: lop, .. } => assert_eq!(*lop, BinOp::Greater),
                other => panic!("expected Binary Greater, got {:?}", other),
            }
            match &right.kind {
                ExprKind::Binary { op: rop, .. } => assert_eq!(*rop, BinOp::Less),
                other => panic!("expected Binary Less, got {:?}", other),
            }
        }
        other => panic!("expected Binary And, got {:?}", other),
    }
}

#[test]
fn test_prec_and_tighter_than_or() {
    // a or b and c  →  a or (b and c)
    // Using booleans in invariant
    let src = r#"space T {
  state {
    a: boolean = false
    b: boolean = false
    c: boolean = false
  }
  invariant check {
    a or b and c
  }
}"#;
    let prog = parse_ok(src);
    let expr = &prog.space.body.invariants[0].condition;
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::Or);
            assert!(matches!(left.kind, ExprKind::Identifier(ref n) if n == "a"));
            match &right.kind {
                ExprKind::Binary { op: rop, .. } => assert_eq!(*rop, BinOp::And),
                other => panic!("expected Binary And, got {:?}", other),
            }
        }
        other => panic!("expected Binary Or, got {:?}", other),
    }
}

#[test]
fn test_prec_unary_not_tightest() {
    // not a and b  →  (not a) and b
    let src = r#"space T {
  state {
    a: boolean = false
    b: boolean = false
  }
  invariant check {
    not a and b
  }
}"#;
    let prog = parse_ok(src);
    let expr = &prog.space.body.invariants[0].condition;
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::And);
            match &left.kind {
                ExprKind::Unary { op: uop, operand, .. } => {
                    assert_eq!(*uop, UnaryOp::Not);
                    assert!(matches!(operand.kind, ExprKind::Identifier(ref n) if n == "a"));
                }
                other => panic!("expected Unary Not, got {:?}", other),
            }
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "b"));
        }
        other => panic!("expected Binary And, got {:?}", other),
    }
}

#[test]
fn test_prec_unary_neg_tightest() {
    // -a + b  →  (-a) + b
    let prog = parse_ok(&wrap_expr("-a + b"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::Binary { op, left, .. } => {
            assert_eq!(*op, BinOp::Add);
            match &left.kind {
                ExprKind::Unary { op: uop, .. } => assert_eq!(*uop, UnaryOp::Neg),
                other => panic!("expected Unary Neg, got {:?}", other),
            }
        }
        other => panic!("expected Binary Add, got {:?}", other),
    }
}

#[test]
fn test_prec_comparison_chaining_error() {
    // a == b == c  →  COMPILE ERROR
    let errors = error_count(&wrap_expr("a == b == c"));
    assert!(errors > 0, "comparison chaining must be rejected");
}

#[test]
fn test_prec_full_chain() {
    // a + b > c * d or e  →  ((a + b) > (c * d)) or e
    let src = r#"space T {
  state {
    a: number = 0
    b: number = 0
    c: number = 0
    d: number = 0
    e: boolean = false
  }
  invariant check {
    a + b > c * d or e
  }
}"#;
    let prog = parse_ok(src);
    let expr = &prog.space.body.invariants[0].condition;
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::Or, "top = or");
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "e"));
            match &left.kind {
                ExprKind::Binary { op: cmp_op, left: cmp_l, right: cmp_r, .. } => {
                    assert_eq!(*cmp_op, BinOp::Greater, "cmp = >");
                    // left of >: a + b
                    match &cmp_l.kind {
                        ExprKind::Binary { op: add_op, .. } => {
                            assert_eq!(*add_op, BinOp::Add);
                        }
                        other => panic!("expected a + b, got {:?}", other),
                    }
                    // right of >: c * d
                    match &cmp_r.kind {
                        ExprKind::Binary { op: mul_op, .. } => {
                            assert_eq!(*mul_op, BinOp::Mul);
                        }
                        other => panic!("expected c * d, got {:?}", other),
                    }
                }
                other => panic!("expected Binary Greater, got {:?}", other),
            }
        }
        other => panic!("expected Binary Or, got {:?}", other),
    }
}

#[test]
fn test_prec_result_unwrap_postfix() {
    // result?  →  (result?)
    let prog = parse_ok(&wrap_expr("result?"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::ResultUnwrap(inner) => {
            assert!(matches!(inner.kind, ExprKind::Identifier(ref n) if n == "result"));
        }
        other => panic!("expected ResultUnwrap, got {:?}", other),
    }
}

#[test]
fn test_prec_result_unwrap_then_field_access() {
    // result?.field  →  ((result?).field)
    let prog = parse_ok(&wrap_expr("result?.field"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::FieldAccess { object, field, .. } => {
            assert_eq!(field.name, "field");
            match &object.kind {
                ExprKind::ResultUnwrap(inner) => {
                    assert!(matches!(inner.kind, ExprKind::Identifier(ref n) if n == "result"));
                }
                other => panic!("expected ResultUnwrap, got {:?}", other),
            }
        }
        other => panic!("expected FieldAccess, got {:?}", other),
    }
}

#[test]
fn test_prec_nil_coalescing() {
    // value ?? fallback  →  (value ?? fallback)
    let prog = parse_ok(&wrap_expr("value ?? fallback"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::NilCoalesce { left, right, .. } => {
            assert!(matches!(left.kind, ExprKind::Identifier(ref n) if n == "value"));
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "fallback"));
        }
        other => panic!("expected NilCoalesce, got {:?}", other),
    }
}

#[test]
fn test_prec_nil_coalesce_tighter_than_and() {
    // a ?? b and c  →  (a ?? b) and c
    let src = r#"space T {
  state {
    a: boolean = false
    b: boolean = false
    c: boolean = false
  }
  invariant check {
    a ?? b and c
  }
}"#;
    let prog = parse_ok(src);
    let expr = &prog.space.body.invariants[0].condition;
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::And);
            match &left.kind {
                ExprKind::NilCoalesce { .. } => { /* correct */ }
                other => panic!("expected NilCoalesce, got {:?}", other),
            }
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "c"));
        }
        other => panic!("expected Binary And, got {:?}", other),
    }
}

#[test]
fn test_prec_qualified_call_unwrap_field_access() {
    // http.get(url)?.body  →  ((http.get(url))?).body
    let src = r#"space T {
  state {
    url: string = ""
  }
  derived {
    body: string = http.get(url)?.body
  }
}"#;
    let prog = parse_ok(src);
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::FieldAccess { object, field, .. } => {
            assert_eq!(field.name, "body");
            match &object.kind {
                ExprKind::ResultUnwrap(inner) => {
                    match &inner.kind {
                        ExprKind::QualifiedCall { module, function, args } => {
                            assert_eq!(module.name, "http");
                            assert_eq!(function.name, "get");
                            assert_eq!(args.len(), 1);
                        }
                        other => panic!("expected QualifiedCall, got {:?}", other),
                    }
                }
                other => panic!("expected ResultUnwrap, got {:?}", other),
            }
        }
        other => panic!("expected FieldAccess, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 2. Edge-Case Acceptance
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_empty_action_body_allowed() {
    // Empty action body is valid (no-op action)
    let errors = error_count(
        r#"space T {
  state {
    x: number = 0
  }
  action reset() {}
}"#,
    );
    assert_eq!(errors, 0, "empty action body must be allowed");
}

#[test]
fn test_nested_lambda_depth_1_ok() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    mapped: list<number> = list.map(items, fn(x: number) { x + 1 })
  }
}"#,
    );
    let expr = derived_expr(&prog);
    // Should be a QualifiedCall with a Lambda arg
    match &expr.kind {
        ExprKind::QualifiedCall { args, .. } => {
            assert!(matches!(args[1].kind, ExprKind::Lambda(_)));
        }
        other => panic!("expected QualifiedCall, got {:?}", other),
    }
}

#[test]
fn test_nested_lambda_depth_2_ok() {
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(x: number) { list.map(items, fn(y: number) { x + y }) })
  }
}"#,
    );
    assert_eq!(errors, 0, "lambda depth 2 must be allowed");
}

#[test]
fn test_nested_lambda_depth_3_ok() {
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(a: number) {
      list.map(items, fn(b: number) {
        list.map(items, fn(c: number) { a + b + c })
      })
    })
  }
}"#,
    );
    assert_eq!(errors, 0, "lambda depth 3 must be allowed");
}

#[test]
fn test_nested_record_depth_1_ok() {
    let prog = parse_ok(&wrap_expr("{ a: 1 }"));
    let expr = derived_expr(&prog);
    assert!(matches!(expr.kind, ExprKind::RecordLit(_)));
}

#[test]
fn test_nested_record_depth_2_ok() {
    let errors = error_count(&wrap_expr("{ a: { b: 1 } }"));
    assert_eq!(errors, 0, "record depth 2 must be allowed");
}

#[test]
fn test_nested_record_depth_3_ok() {
    let errors = error_count(&wrap_expr("{ a: { b: { c: 1 } } }"));
    assert_eq!(errors, 0, "record depth 3 must be allowed");
}

#[test]
fn test_nested_record_depth_4_ok() {
    let errors = error_count(&wrap_expr("{ a: { b: { c: { d: 1 } } } }"));
    assert_eq!(errors, 0, "record depth 4 must be allowed");
}

#[test]
fn test_for_depth_1_ok() {
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  action go() {
    for x in items {
      set items = items
    }
  }
}"#,
    );
    assert_eq!(errors, 0, "for depth 1 must be allowed");
}

#[test]
fn test_for_depth_2_ok() {
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  action go() {
    for x in items {
      for y in items {
        set items = items
      }
    }
  }
}"#,
    );
    assert_eq!(errors, 0, "for depth 2 must be allowed");
}

#[test]
fn test_for_depth_3_ok() {
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  action go() {
    for x in items {
      for y in items {
        for z in items {
          set items = items
        }
      }
    }
  }
}"#,
    );
    assert_eq!(errors, 0, "for depth 3 must be allowed");
}

#[test]
fn test_params_at_limit_8_ok() {
    let errors = error_count(
        r#"space T {
  state {
    x: number = 0
  }
  action go(a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) {
    set x = a
  }
}"#,
    );
    assert_eq!(errors, 0, "8 params must be allowed");
}

#[test]
fn test_lambda_params_at_limit_8_ok() {
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) { a })
  }
}"#,
    );
    assert_eq!(errors, 0, "lambda with 8 params must be allowed");
}

// ═══════════════════════════════════════════════════════════════════════
// 3. Parser Error Tests — Specific Error Codes
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_empty_state_block_e606() {
    let codes = error_codes(
        r#"space T {
  state {}
}"#,
    );
    assert!(
        codes.contains(&ErrorCode::EMPTY_STATE_BLOCK),
        "empty state block must produce E606, got: {:?}",
        codes,
    );
}

#[test]
fn test_expression_body_lambda_e602() {
    let codes = error_codes(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(x: number) x + 1)
  }
}"#,
    );
    assert!(
        codes.contains(&ErrorCode::EXPRESSION_BODY_LAMBDA),
        "expression-body lambda must produce E602, got: {:?}",
        codes,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 4. Structural Limit Enforcement (E607)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_lambda_depth_4_exceeds_limit() {
    let codes = error_codes(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(a: number) {
      list.map(items, fn(b: number) {
        list.map(items, fn(c: number) {
          list.map(items, fn(d: number) { a + b + c + d })
        })
      })
    })
  }
}"#,
    );
    assert!(
        codes.contains(&ErrorCode::STRUCTURAL_LIMIT_EXCEEDED),
        "lambda depth 4 must produce E607, got: {:?}",
        codes,
    );
}

#[test]
fn test_record_depth_5_exceeds_limit() {
    let codes = error_codes(&wrap_expr("{ a: { b: { c: { d: { e: 1 } } } } }"));
    assert!(
        codes.contains(&ErrorCode::STRUCTURAL_LIMIT_EXCEEDED),
        "record depth 5 must produce E607, got: {:?}",
        codes,
    );
}

#[test]
fn test_for_depth_4_exceeds_limit() {
    let codes = error_codes(
        r#"space T {
  state {
    items: list<number> = []
  }
  action go() {
    for a in items {
      for b in items {
        for c in items {
          for d in items {
            set items = items
          }
        }
      }
    }
  }
}"#,
    );
    assert!(
        codes.contains(&ErrorCode::STRUCTURAL_LIMIT_EXCEEDED),
        "for depth 4 must produce E607, got: {:?}",
        codes,
    );
}

#[test]
fn test_action_params_9_exceeds_limit() {
    let codes = error_codes(
        r#"space T {
  state {
    x: number = 0
  }
  action go(a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) {
    set x = a
  }
}"#,
    );
    assert!(
        codes.contains(&ErrorCode::STRUCTURAL_LIMIT_EXCEEDED),
        "9 action params must produce E607, got: {:?}",
        codes,
    );
}

#[test]
fn test_lambda_params_9_exceeds_limit() {
    let codes = error_codes(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) { a })
  }
}"#,
    );
    assert!(
        codes.contains(&ErrorCode::STRUCTURAL_LIMIT_EXCEEDED),
        "9 lambda params must produce E607, got: {:?}",
        codes,
    );
}

#[test]
fn test_expression_depth_17_exceeds_limit() {
    // Build an expression with nesting depth > 16: a + (a + (a + ... (a + b) ...))
    // We wrap in parens to force nesting depth to increase at each level.
    let mut expr = String::from("b");
    for _ in 0..17 {
        expr = format!("(a + {})", expr);
    }
    let codes = error_codes(&wrap_expr(&expr));
    assert!(
        codes.contains(&ErrorCode::STRUCTURAL_LIMIT_EXCEEDED),
        "expression depth 17 must produce E607, got: {:?}",
        codes,
    );
}

#[test]
fn test_expression_depth_16_boundary_ok() {
    // Build an expression with nesting depth exactly 16 (should be ok).
    let mut expr = String::from("b");
    for _ in 0..15 {
        expr = format!("(a + {})", expr);
    }
    let errors = error_count(&wrap_expr(&expr));
    assert_eq!(errors, 0, "expression depth 16 must be allowed");
}

// ═══════════════════════════════════════════════════════════════════════
// 5. Additional Precedence Edge Cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_prec_mul_div_mod_same_level_left_assoc() {
    // a * b / c % d  →  ((a * b) / c) % d
    let prog = parse_ok(&wrap_expr("a * b / c % d"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::Binary { op, left, right, .. } => {
            assert_eq!(*op, BinOp::Mod, "top-level should be Mod");
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "d"));
            match &left.kind {
                ExprKind::Binary { op: div_op, left: div_l, .. } => {
                    assert_eq!(*div_op, BinOp::Div);
                    match &div_l.kind {
                        ExprKind::Binary { op: mul_op, .. } => {
                            assert_eq!(*mul_op, BinOp::Mul);
                        }
                        other => panic!("expected Mul, got {:?}", other),
                    }
                }
                other => panic!("expected Div, got {:?}", other),
            }
        }
        other => panic!("expected Mod, got {:?}", other),
    }
}

#[test]
fn test_prec_add_sub_same_level_left_assoc() {
    // a + b - c + d  →  ((a + b) - c) + d
    let prog = parse_ok(&wrap_expr("a + b - c + d"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::Binary { op, right, left, .. } => {
            assert_eq!(*op, BinOp::Add, "top = Add");
            assert!(matches!(right.kind, ExprKind::Identifier(ref n) if n == "d"));
            match &left.kind {
                ExprKind::Binary { op: sub_op, left: sub_l, .. } => {
                    assert_eq!(*sub_op, BinOp::Sub);
                    match &sub_l.kind {
                        ExprKind::Binary { op: add_op, .. } => {
                            assert_eq!(*add_op, BinOp::Add);
                        }
                        other => panic!("expected inner Add, got {:?}", other),
                    }
                }
                other => panic!("expected Sub, got {:?}", other),
            }
        }
        other => panic!("expected outer Add, got {:?}", other),
    }
}

#[test]
fn test_prec_eq_neq_same_level() {
    // a == b  and  a != b  must parse as comparisons
    let prog = parse_ok(&wrap_bool_expr("a == b"));
    let expr = invariant_expr(&prog);
    assert!(matches!(expr.kind, ExprKind::Binary { op: BinOp::Eq, .. }));

    let prog2 = parse_ok(&wrap_bool_expr("a != b"));
    let expr2 = invariant_expr(&prog2);
    assert!(matches!(expr2.kind, ExprKind::Binary { op: BinOp::NotEq, .. }));
}

#[test]
fn test_prec_comparison_chaining_lt_lt_error() {
    let errors = error_count(&wrap_bool_expr("a < b < c"));
    assert!(errors > 0, "a < b < c must be rejected");
}

#[test]
fn test_prec_comparison_chaining_gte_lte_error() {
    let errors = error_count(&wrap_bool_expr("a >= b <= c"));
    assert!(errors > 0, "a >= b <= c must be rejected");
}

#[test]
fn test_prec_double_negation_rejected() {
    // --a is not supported (unary `-` does not chain)
    let errors = error_count(&wrap_expr("--a"));
    assert!(errors > 0, "double negation must be rejected");
}

#[test]
fn test_prec_not_not_rejected() {
    // not not a is not supported (unary `not` does not chain)
    let src = r#"space T {
  state {
    a: boolean = false
  }
  invariant check {
    not not a
  }
}"#;
    let errors = error_count(src);
    assert!(errors > 0, "double not must be rejected");
}

#[test]
fn test_prec_paren_override() {
    // (a + b) * c  →  parens override default precedence
    let prog = parse_ok(&wrap_expr("(a + b) * c"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::Binary { op, left, .. } => {
            assert_eq!(*op, BinOp::Mul);
            // left should be a grouped expression containing Add
            match &left.kind {
                ExprKind::Paren(inner) => {
                    match &inner.kind {
                        ExprKind::Binary { op: add_op, .. } => {
                            assert_eq!(*add_op, BinOp::Add);
                        }
                        other => panic!("expected Binary Add inside parens, got {:?}", other),
                    }
                }
                other => panic!("expected Paren, got {:?}", other),
            }
        }
        other => panic!("expected Binary Mul, got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 6. Method Chaining & Postfix Edge Cases
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_method_chain() {
    // a.b.c  →  (a.b).c
    let prog = parse_ok(&wrap_expr("a.b.c"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::FieldAccess { object, field, .. } => {
            assert_eq!(field.name, "c");
            match &object.kind {
                ExprKind::FieldAccess { object: inner_obj, field: inner_field, .. } => {
                    assert_eq!(inner_field.name, "b");
                    assert!(matches!(inner_obj.kind, ExprKind::Identifier(ref n) if n == "a"));
                }
                other => panic!("expected inner FieldAccess, got {:?}", other),
            }
        }
        other => panic!("expected FieldAccess, got {:?}", other),
    }
}

#[test]
fn test_result_unwrap_on_field_access() {
    // a.b?  →  (a.b)?
    let prog = parse_ok(&wrap_expr("a.b?"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::ResultUnwrap(inner) => {
            assert!(matches!(&inner.kind, ExprKind::FieldAccess { .. }));
        }
        other => panic!("expected ResultUnwrap(FieldAccess), got {:?}", other),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// 7. Record Spread & String Interpolation
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_record_with_spread() {
    let prog = parse_ok(&wrap_expr("{ ...a, b: 1 }"));
    let expr = derived_expr(&prog);
    match &expr.kind {
        ExprKind::RecordLit(entries) => {
            assert_eq!(entries.len(), 2);
            assert!(matches!(entries[0], RecordEntry::Spread(_)));
            assert!(matches!(entries[1], RecordEntry::Field { .. }));
        }
        other => panic!("expected RecordLit, got {:?}", other),
    }
}

#[test]
fn test_string_interpolation_in_derived() {
    let prog = parse_ok(
        r#"space T {
  state {
    name: string = "world"
  }
  derived {
    greeting: string = "hello ${name}"
  }
}"#,
    );
    let expr = derived_expr(&prog);
    assert!(matches!(expr.kind, ExprKind::StringInterpolation(_)));
}

// ═══════════════════════════════════════════════════════════════════════
// 8. Depth Tracking Reset Between Constructs
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_lambda_depth_resets_between_actions() {
    // Two separate derived fields each with depth-3 lambdas should both be OK
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    r1: number = list.map(items, fn(a: number) {
      list.map(items, fn(b: number) {
        list.map(items, fn(c: number) { a + b + c })
      })
    })
    r2: number = list.map(items, fn(x: number) {
      list.map(items, fn(y: number) {
        list.map(items, fn(z: number) { x + y + z })
      })
    })
  }
}"#,
    );
    assert_eq!(
        errors, 0,
        "lambda depth should reset between separate expressions"
    );
}

#[test]
fn test_for_depth_resets_between_actions() {
    // Two separate actions, each with depth-3 for loops, should both be OK
    let errors = error_count(
        r#"space T {
  state {
    items: list<number> = []
  }
  action a1() {
    for x in items {
      for y in items {
        for z in items {
          set items = items
        }
      }
    }
  }
  action a2() {
    for a in items {
      for b in items {
        for c in items {
          set items = items
        }
      }
    }
  }
}"#,
    );
    assert_eq!(
        errors, 0,
        "for depth should reset between separate action bodies"
    );
}

#[test]
fn test_record_depth_resets_between_fields() {
    // Two derived fields each with depth-4 records should be OK
    let errors = error_count(
        r#"space T {
  state {
    x: number = 0
  }
  derived {
    r1: number = { a: { b: { c: { d: 1 } } } }
    r2: number = { w: { x: { y: { z: 2 } } } }
  }
}"#,
    );
    assert_eq!(
        errors, 0,
        "record depth should reset between separate expressions"
    );
}

// ═══════════════════════════════════════════════════════════════════════
// 9. Boundary Value Tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_exactly_zero_params_ok() {
    let errors = error_count(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    set x = 1
  }
}"#,
    );
    assert_eq!(errors, 0, "zero params must be allowed");
}

#[test]
fn test_single_state_field_ok() {
    let errors = error_count(
        r#"space T {
  state {
    x: number = 0
  }
}"#,
    );
    assert_eq!(errors, 0, "single state field must be allowed");
}

#[test]
fn test_many_state_fields_ok() {
    // No limit on state fields — should be fine with 20
    let mut fields = String::new();
    for i in 0..20 {
        fields.push_str(&format!("    f{}: number = {}\n", i, i));
    }
    let src = format!(
        "space T {{\n  state {{\n{fields}  }}\n}}"
    );
    let errors = error_count(&src);
    assert_eq!(errors, 0, "many state fields must be allowed");
}

#[test]
fn test_many_actions_ok() {
    // No limit on number of actions
    let mut actions = String::new();
    for i in 0..10 {
        actions.push_str(&format!(
            "  action a{}() {{\n    set x = {}\n  }}\n",
            i, i
        ));
    }
    let src = format!(
        "space T {{\n  state {{\n    x: number = 0\n  }}\n{actions}}}"
    );
    let errors = error_count(&src);
    assert_eq!(errors, 0, "many actions must be allowed");
}

// ═══════════════════════════════════════════════════════════════════════
// 10. Error Recovery After Limit Violations
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_error_recovery_after_e606() {
    // Even with empty state, parser should still produce a program
    let result = parse(
        r#"space T {
  state {}
}"#,
    );
    assert!(result.errors.has_errors());
    assert!(result.program.is_some(), "should recover after E606");
}

#[test]
fn test_error_recovery_after_e602() {
    // Expression-body lambda — parser should recover
    let result = parse(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(x: number) x + 1)
  }
}"#,
    );
    assert!(result.errors.has_errors());
    // Parser should still produce something
    assert!(result.program.is_some(), "should recover after E602");
}

#[test]
fn test_error_recovery_after_e607_lambda() {
    // Lambda depth exceeded — parser should recover
    let result = parse(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    result: number = list.map(items, fn(a: number) {
      list.map(items, fn(b: number) {
        list.map(items, fn(c: number) {
          list.map(items, fn(d: number) { a })
        })
      })
    })
  }
}"#,
    );
    assert!(result.errors.has_errors());
    assert!(result.program.is_some(), "should recover after E607 lambda depth");
}

//! Comprehensive parser tests for PEPL Phase 3 (C3).
//!
//! Covers: full programs, expressions (precedence, postfix, interpolation),
//! declarations (types, state, capabilities, credentials, derived, invariants,
//! actions, views, update, handleEvent), statements, UI blocks, tests blocks,
//! block ordering (E600), error recovery, and determinism.

use pepl_lexer::Lexer;
use pepl_parser::{ParseResult, Parser};
use pepl_types::ast::*;
use pepl_types::SourceFile;

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

// ─────────────────────────────────────────────────────────────────────
// Minimal programs
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_minimal_space() {
    let prog = parse_ok(
        r#"space Counter {
  state {
    count: number = 0
  }
}"#,
    );
    assert_eq!(prog.space.name.name, "Counter");
    assert_eq!(prog.space.body.state.fields.len(), 1);
    assert_eq!(prog.space.body.state.fields[0].name.name, "count");
}

#[test]
fn test_space_with_multiple_state_fields() {
    let prog = parse_ok(
        r#"space App {
  state {
    name: string = "hello"
    count: number = 42
    active: bool = true
  }
}"#,
    );
    assert_eq!(prog.space.body.state.fields.len(), 3);
    assert_eq!(prog.space.body.state.fields[0].name.name, "name");
    assert_eq!(prog.space.body.state.fields[1].name.name, "count");
    assert_eq!(prog.space.body.state.fields[2].name.name, "active");
}

// ─────────────────────────────────────────────────────────────────────
// Type declarations
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_type_alias() {
    let prog = parse_ok(
        r#"space T {
  type Meters = number
  state {
    dist: Meters = 0
  }
}"#,
    );
    assert_eq!(prog.space.body.types.len(), 1);
    assert_eq!(prog.space.body.types[0].name.name, "Meters");
    match &prog.space.body.types[0].body {
        TypeDeclBody::Alias(ta) => assert_eq!(ta.kind, TypeKind::Number),
        _ => panic!("expected alias"),
    }
}

#[test]
fn test_sum_type() {
    let prog = parse_ok(
        r#"space T {
  type Shape =
    | Circle(radius: number)
    | Rectangle(w: number, h: number)
    | Point
  state {
    s: Shape = Point
  }
}"#,
    );
    assert_eq!(prog.space.body.types.len(), 1);
    match &prog.space.body.types[0].body {
        TypeDeclBody::SumType(variants) => {
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name.name, "Circle");
            assert_eq!(variants[0].params.len(), 1);
            assert_eq!(variants[1].name.name, "Rectangle");
            assert_eq!(variants[1].params.len(), 2);
            assert_eq!(variants[2].name.name, "Point");
            assert_eq!(variants[2].params.len(), 0);
        }
        _ => panic!("expected sum type"),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Capabilities, Credentials, Derived
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_capabilities_block() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  capabilities {
    required: [http, storage]
    optional: [location]
  }
}"#,
    );
    let caps = prog.space.body.capabilities.expect("missing capabilities");
    assert_eq!(caps.required.len(), 2);
    assert_eq!(caps.required[0].name, "http");
    assert_eq!(caps.required[1].name, "storage");
    assert_eq!(caps.optional.len(), 1);
    assert_eq!(caps.optional[0].name, "location");
}

#[test]
fn test_credentials_block() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  credentials {
    api_key: string
  }
}"#,
    );
    let creds = prog.space.body.credentials.expect("missing credentials");
    assert_eq!(creds.fields.len(), 1);
    assert_eq!(creds.fields[0].name.name, "api_key");
}

#[test]
fn test_derived_block() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    total: number = list.length(items)
  }
}"#,
    );
    let derived = prog.space.body.derived.expect("missing derived");
    assert_eq!(derived.fields.len(), 1);
    assert_eq!(derived.fields[0].name.name, "total");
}

// ─────────────────────────────────────────────────────────────────────
// Invariants
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_invariant() {
    let prog = parse_ok(
        r#"space T {
  state {
    count: number = 0
  }
  invariant non_negative {
    count >= 0
  }
}"#,
    );
    assert_eq!(prog.space.body.invariants.len(), 1);
    assert_eq!(prog.space.body.invariants[0].name.name, "non_negative");
}

// ─────────────────────────────────────────────────────────────────────
// Actions
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_action_decl() {
    let prog = parse_ok(
        r#"space T {
  state {
    count: number = 0
  }
  action increment() {
    set count = count + 1
  }
}"#,
    );
    assert_eq!(prog.space.body.actions.len(), 1);
    assert_eq!(prog.space.body.actions[0].name.name, "increment");
    assert!(prog.space.body.actions[0].params.is_empty());
}

#[test]
fn test_action_with_params() {
    let prog = parse_ok(
        r#"space T {
  state {
    count: number = 0
  }
  action add(n: number) {
    set count = count + n
  }
}"#,
    );
    assert_eq!(prog.space.body.actions[0].params.len(), 1);
    assert_eq!(prog.space.body.actions[0].params[0].name.name, "n");
}

// ─────────────────────────────────────────────────────────────────────
// Views
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_view_decl() {
    let prog = parse_ok(
        r#"space T {
  state {
    count: number = 0
  }
  view main() -> Surface {
    Text { value: "hello" }
  }
}"#,
    );
    assert_eq!(prog.space.body.views.len(), 1);
    assert_eq!(prog.space.body.views[0].name.name, "main");
    assert_eq!(prog.space.body.views[0].body.elements.len(), 1);
}

#[test]
fn test_view_with_multiple_components() {
    let prog = parse_ok(
        r#"space T {
  state {
    count: number = 0
  }
  view main() -> Surface {
    Column {
      spacing: 16,
    } {
      Text { value: "Counter" }
      Button { label: "Increment", on_tap: increment }
    }
  }
}"#,
    );
    let view = &prog.space.body.views[0];
    assert_eq!(view.body.elements.len(), 1);
    match &view.body.elements[0] {
        UIElement::Component(c) => {
            assert_eq!(c.name.name, "Column");
            assert!(c.children.is_some());
            assert_eq!(c.children.as_ref().unwrap().elements.len(), 2);
        }
        _ => panic!("expected component"),
    }
}

#[test]
fn test_view_if_element() {
    let prog = parse_ok(
        r#"space T {
  state {
    visible: bool = true
  }
  view main() -> Surface {
    if visible {
      Text { value: "shown" }
    }
  }
}"#,
    );
    let elem = &prog.space.body.views[0].body.elements[0];
    assert!(matches!(elem, UIElement::If(_)));
}

#[test]
fn test_view_for_element() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<string> = []
  }
  view main() -> Surface {
    for item in items {
      Text { value: item }
    }
  }
}"#,
    );
    let elem = &prog.space.body.views[0].body.elements[0];
    assert!(matches!(elem, UIElement::For(_)));
}

// ─────────────────────────────────────────────────────────────────────
// Game Loop
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_update_decl() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  update(dt: number) {
    set x = x + dt
  }
}"#,
    );
    let upd = prog.space.body.update.expect("missing update");
    assert_eq!(upd.param.name.name, "dt");
}

#[test]
fn test_handle_event_decl() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  handleEvent(event: InputEvent) {
    set x = 1
  }
}"#,
    );
    let he = prog.space.body.handle_event.expect("missing handleEvent");
    assert_eq!(he.param.name.name, "event");
}

// ─────────────────────────────────────────────────────────────────────
// Tests blocks
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_tests_block() {
    let prog = parse_ok(
        r#"space T {
  state {
    count: number = 0
  }
  action increment() {
    set count = count + 1
  }
}

tests {
  test "count starts at zero" {
    assert count == 0
  }
  test "increment works" {
    increment()
    assert count == 1
  }
}"#,
    );
    assert_eq!(prog.tests.len(), 1);
    assert_eq!(prog.tests[0].cases.len(), 2);
    assert_eq!(prog.tests[0].cases[0].description, "count starts at zero");
    assert_eq!(prog.tests[0].cases[1].description, "increment works");
}

#[test]
fn test_with_responses() {
    let prog = parse_ok(
        r#"space T {
  state {
    data: string = ""
  }
  capabilities {
    required: [http]
  }
  action fetch() {
    let _ = http.get("https://example.com")
  }
}

tests {
  test "fetch with mock" with_responses {
    http.get("https://example.com") -> Ok("data"),
  } {
    fetch()
    assert data == "data"
  }
}"#,
    );
    let tc = &prog.tests[0].cases[0];
    assert!(tc.with_responses.is_some());
    let wr = tc.with_responses.as_ref().unwrap();
    assert_eq!(wr.mappings.len(), 1);
    assert_eq!(wr.mappings[0].module.name, "http");
    assert_eq!(wr.mappings[0].function.name, "get");
}

// ─────────────────────────────────────────────────────────────────────
// Expressions: Literals
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_number_literal() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 42.5
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::NumberLit(n) => assert_eq!(*n, 42.5),
        other => panic!("expected number, got {other:?}"),
    }
}

#[test]
fn test_string_literal() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: string = "hello"
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::StringLit(s) => assert_eq!(s, "hello"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn test_bool_literal() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: bool = true
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::BoolLit(b) => assert!(*b),
        other => panic!("expected bool, got {other:?}"),
    }
}

#[test]
fn test_nil_literal() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: string = nil
  }
}"#,
    );
    assert!(matches!(
        &prog.space.body.state.fields[0].default.kind,
        ExprKind::NilLit
    ));
}

#[test]
fn test_list_literal() {
    let prog = parse_ok(
        r#"space T {
  state {
    xs: list<number> = [1, 2, 3]
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::ListLit(items) => assert_eq!(items.len(), 3),
        other => panic!("expected list, got {other:?}"),
    }
}

#[test]
fn test_empty_list() {
    let prog = parse_ok(
        r#"space T {
  state {
    xs: list<number> = []
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::ListLit(items) => assert!(items.is_empty()),
        other => panic!("expected empty list, got {other:?}"),
    }
}

#[test]
fn test_record_literal() {
    let prog = parse_ok(
        r#"space T {
  state {
    r: { name: string } = { name: "Alice" }
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::RecordLit(entries) => {
            assert_eq!(entries.len(), 1);
            match &entries[0] {
                RecordEntry::Field { name, .. } => assert_eq!(name.name, "name"),
                _ => panic!("expected field"),
            }
        }
        other => panic!("expected record, got {other:?}"),
    }
}

#[test]
fn test_record_with_spread() {
    let prog = parse_ok(
        r#"space T {
  state {
    r: { a: number, b: number } = { a: 1, b: 2 }
  }
  action update_a() {
    set r = { ...r, a: 10 }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    // The set value should be a record with spread
    if let Stmt::Set(set_stmt) = &body.stmts[0] {
        if let ExprKind::RecordLit(entries) = &set_stmt.value.kind {
            assert!(matches!(&entries[0], RecordEntry::Spread(_)));
            assert!(matches!(&entries[1], RecordEntry::Field { .. }));
        } else {
            panic!("expected record literal");
        }
    } else {
        panic!("expected set statement");
    }
}

// ─────────────────────────────────────────────────────────────────────
// Expressions: Operators & Precedence
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_binary_arithmetic() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 1 + 2 * 3
  }
}"#,
    );
    // 1 + (2 * 3) — mul binds tighter than add
    let expr = &prog.space.body.state.fields[0].default;
    match &expr.kind {
        ExprKind::Binary { op, right, .. } => {
            assert_eq!(*op, BinOp::Add);
            match &right.kind {
                ExprKind::Binary { op, .. } => assert_eq!(*op, BinOp::Mul),
                _ => panic!("expected binary mul"),
            }
        }
        other => panic!("expected binary add, got {other:?}"),
    }
}

#[test]
fn test_comparison() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  invariant pos {
    x >= 0
  }
}"#,
    );
    match &prog.space.body.invariants[0].condition.kind {
        ExprKind::Binary { op, .. } => assert_eq!(*op, BinOp::GreaterEq),
        other => panic!("expected comparison, got {other:?}"),
    }
}

#[test]
fn test_logical_operators() {
    let prog = parse_ok(
        r#"space T {
  state {
    a: bool = true
    b: bool = false
  }
  invariant combined {
    a and b or true
  }
}"#,
    );
    // `a and b or true` → `(a and b) or true`
    match &prog.space.body.invariants[0].condition.kind {
        ExprKind::Binary { op, left, .. } => {
            assert_eq!(*op, BinOp::Or);
            match &left.kind {
                ExprKind::Binary { op, .. } => assert_eq!(*op, BinOp::And),
                _ => panic!("expected and"),
            }
        }
        other => panic!("expected or, got {other:?}"),
    }
}

#[test]
fn test_unary_negation() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = -5
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::Unary { op, .. } => assert_eq!(*op, UnaryOp::Neg),
        other => panic!("expected unary neg, got {other:?}"),
    }
}

#[test]
fn test_unary_not() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: bool = true
  }
  invariant check {
    not x
  }
}"#,
    );
    match &prog.space.body.invariants[0].condition.kind {
        ExprKind::Unary { op, .. } => assert_eq!(*op, UnaryOp::Not),
        other => panic!("expected unary not, got {other:?}"),
    }
}

#[test]
fn test_nil_coalesce() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: string = nil
  }
  derived {
    y: string = x ?? "default"
  }
}"#,
    );
    let field = &prog.space.body.derived.unwrap().fields[0];
    assert!(matches!(&field.value.kind, ExprKind::NilCoalesce { .. }));
}

#[test]
fn test_result_unwrap() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  capabilities {
    required: [http]
  }
  action fetch() {
    let val = http.get("url")?
    set x = 1
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Let(binding) = &body.stmts[0] {
        assert!(matches!(&binding.value.kind, ExprKind::ResultUnwrap(_)));
    } else {
        panic!("expected let binding");
    }
}

#[test]
fn test_parenthesized_expr() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = (1 + 2) * 3
  }
}"#,
    );
    match &prog.space.body.state.fields[0].default.kind {
        ExprKind::Binary { op, left, .. } => {
            assert_eq!(*op, BinOp::Mul);
            assert!(matches!(&left.kind, ExprKind::Paren(_)));
        }
        other => panic!("expected binary mul, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Expressions: Calls
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_qualified_call() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = []
  }
  derived {
    count: number = list.length(items)
  }
}"#,
    );
    let field = &prog.space.body.derived.unwrap().fields[0];
    match &field.value.kind {
        ExprKind::QualifiedCall {
            module,
            function,
            args,
        } => {
            assert_eq!(module.name, "list");
            assert_eq!(function.name, "length");
            assert_eq!(args.len(), 1);
        }
        other => panic!("expected qualified call, got {other:?}"),
    }
}

#[test]
fn test_unqualified_call() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    increment()
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Expr(es) = &body.stmts[0] {
        match &es.expr.kind {
            ExprKind::Call { name, args } => {
                assert_eq!(name.name, "increment");
                assert!(args.is_empty());
            }
            other => panic!("expected call, got {other:?}"),
        }
    } else {
        panic!("expected expression statement");
    }
}

#[test]
fn test_field_access() {
    let prog = parse_ok(
        r#"space T {
  state {
    r: { x: number } = { x: 5 }
  }
  derived {
    val: number = r.x
  }
}"#,
    );
    let field = &prog.space.body.derived.unwrap().fields[0];
    match &field.value.kind {
        ExprKind::FieldAccess { field: f, .. } => {
            assert_eq!(f.name, "x");
        }
        other => panic!("expected field access, got {other:?}"),
    }
}

#[test]
fn test_method_call() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = []
  }
  action go() {
    let filtered = items.filter(fn(x: number) { x > 0 })
    set items = filtered
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Let(binding) = &body.stmts[0] {
        match &binding.value.kind {
            ExprKind::MethodCall { method, .. } => {
                assert_eq!(method.name, "filter");
            }
            other => panic!("expected method call, got {other:?}"),
        }
    } else {
        panic!("expected let binding");
    }
}

// ─────────────────────────────────────────────────────────────────────
// Expressions: Control Flow
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_if_expression() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    if x > 0 {
      set x = 0
    } else {
      set x = 1
    }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    assert!(matches!(&body.stmts[0], Stmt::If(_)));
}

#[test]
fn test_if_else_if_chain() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    if x == 0 {
      set x = 1
    } else if x == 1 {
      set x = 2
    } else {
      set x = 0
    }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::If(ie) = &body.stmts[0] {
        match &ie.else_branch {
            Some(ElseBranch::ElseIf(inner)) => {
                assert!(inner.else_branch.is_some());
            }
            other => panic!("expected else-if, got {other:?}"),
        }
    }
}

#[test]
fn test_for_expression() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = [1, 2, 3]
    total: number = 0
  }
  action sum_items() {
    for item in items {
      set total = total + item
    }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::For(fe) = &body.stmts[0] {
        assert_eq!(fe.item.name, "item");
        assert!(fe.index.is_none());
    } else {
        panic!("expected for");
    }
}

#[test]
fn test_for_with_index() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = [1, 2, 3]
    total: number = 0
  }
  action go() {
    for item, idx in items {
      set total = total + idx
    }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::For(fe) = &body.stmts[0] {
        assert_eq!(fe.item.name, "item");
        assert_eq!(fe.index.as_ref().unwrap().name, "idx");
    } else {
        panic!("expected for");
    }
}

#[test]
fn test_match_expression() {
    let prog = parse_ok(
        r#"space T {
  type Priority = | High | Medium | Low
  state {
    p: Priority = Medium
  }
  action go() {
    match p {
      High -> { set p = Low },
      Medium -> { set p = High },
      _ -> { set p = Medium },
    }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Match(me) = &body.stmts[0] {
        assert_eq!(me.arms.len(), 3);
        assert!(matches!(&me.arms[2].pattern, Pattern::Wildcard(_)));
    } else {
        panic!("expected match");
    }
}

#[test]
fn test_match_with_bindings() {
    let prog = parse_ok(
        r#"space T {
  type Shape = | Circle(radius: number) | Rect(w: number, h: number)
  state {
    s: Shape = Circle(5)
    area: number = 0
  }
  action compute() {
    match s {
      Circle(r) -> { set area = r },
      Rect(w, h) -> { set area = w * h },
    }
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Match(me) = &body.stmts[0] {
        if let Pattern::Variant { name, bindings } = &me.arms[0].pattern {
            assert_eq!(name.name, "Circle");
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].name, "r");
        } else {
            panic!("expected variant pattern");
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Expressions: Lambda
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_lambda() {
    let prog = parse_ok(
        r#"space T {
  state {
    items: list<number> = [1, 2, 3]
  }
  action go() {
    let pos = items.filter(fn(x: number) { x > 0 })
    set items = pos
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Let(binding) = &body.stmts[0] {
        if let ExprKind::MethodCall { args, .. } = &binding.value.kind {
            assert!(matches!(&args[0].kind, ExprKind::Lambda(_)));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Expressions: String Interpolation
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_string_interpolation() {
    let prog = parse_ok(
        r#"space T {
  state {
    name: string = "world"
    greeting: string = "hello ${name}!"
  }
}"#,
    );
    match &prog.space.body.state.fields[1].default.kind {
        ExprKind::StringInterpolation(parts) => {
            assert_eq!(parts.len(), 3); // "hello ", name, "!"
            assert!(matches!(&parts[0], StringPart::Literal(s) if s == "hello "));
            assert!(matches!(&parts[1], StringPart::Expr(_)));
            assert!(matches!(&parts[2], StringPart::Literal(s) if s == "!"));
        }
        other => panic!("expected string interpolation, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Statements
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_set_statement() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    set x = 5
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Set(ss) = &body.stmts[0] {
        assert_eq!(ss.target.len(), 1);
        assert_eq!(ss.target[0].name, "x");
    } else {
        panic!("expected set");
    }
}

#[test]
fn test_set_nested() {
    let prog = parse_ok(
        r#"space T {
  state {
    r: { a: { b: number } } = { a: { b: 0 } }
  }
  action go() {
    set r.a.b = 5
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Set(ss) = &body.stmts[0] {
        assert_eq!(ss.target.len(), 3);
        assert_eq!(ss.target[0].name, "r");
        assert_eq!(ss.target[1].name, "a");
        assert_eq!(ss.target[2].name, "b");
    }
}

#[test]
fn test_let_binding() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    let y: number = 5
    set x = y
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Let(lb) = &body.stmts[0] {
        assert_eq!(lb.name.as_ref().unwrap().name, "y");
        assert!(lb.type_ann.is_some());
    }
}

#[test]
fn test_let_discard() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  capabilities {
    required: [http]
  }
  action go() {
    let _ = http.get("url")
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::Let(lb) = &body.stmts[0] {
        assert!(lb.name.is_none());
    } else {
        panic!("expected let _ binding");
    }
}

#[test]
fn test_return_stmt() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
  action go() {
    if x > 0 {
      return
    }
    set x = 1
  }
}"#,
    );
    let body = &prog.space.body.actions[0].body;
    if let Stmt::If(ie) = &body.stmts[0] {
        assert!(matches!(&ie.then_block.stmts[0], Stmt::Return(_)));
    }
}

#[test]
fn test_assert_stmt() {
    let prog = parse_ok(
        r#"space T {
  state {
    x: number = 0
  }
}

tests {
  test "basic" {
    assert x == 0, "x should be zero"
  }
}"#,
    );
    let body = &prog.tests[0].cases[0].body;
    if let Stmt::Assert(a) = &body.stmts[0] {
        assert_eq!(a.message.as_deref(), Some("x should be zero"));
    } else {
        panic!("expected assert");
    }
}

// ─────────────────────────────────────────────────────────────────────
// Type Annotations
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_list_type() {
    let prog = parse_ok(
        r#"space T {
  state {
    xs: list<number> = []
  }
}"#,
    );
    match &prog.space.body.state.fields[0].type_ann.kind {
        TypeKind::List(inner) => assert_eq!(inner.kind, TypeKind::Number),
        other => panic!("expected list type, got {other:?}"),
    }
}

#[test]
fn test_result_type() {
    let prog = parse_ok(
        r#"space T {
  state {
    r: Result<string, string> = nil
  }
}"#,
    );
    match &prog.space.body.state.fields[0].type_ann.kind {
        TypeKind::Result(ok, err) => {
            assert_eq!(ok.kind, TypeKind::String);
            assert_eq!(err.kind, TypeKind::String);
        }
        other => panic!("expected result type, got {other:?}"),
    }
}

#[test]
fn test_record_type() {
    let prog = parse_ok(
        r#"space T {
  state {
    r: { name: string, age?: number } = { name: "Alice" }
  }
}"#,
    );
    match &prog.space.body.state.fields[0].type_ann.kind {
        TypeKind::Record(fields) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name.name, "name");
            assert!(!fields[0].optional);
            assert_eq!(fields[1].name.name, "age");
            assert!(fields[1].optional);
        }
        other => panic!("expected record type, got {other:?}"),
    }
}

#[test]
fn test_function_type() {
    let prog = parse_ok(
        r#"space T {
  state {
    f: (number) -> bool = fn(x: number) { x > 0 }
  }
}"#,
    );
    match &prog.space.body.state.fields[0].type_ann.kind {
        TypeKind::Function { params, ret } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].kind, TypeKind::Number);
            assert_eq!(ret.kind, TypeKind::Bool);
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn test_named_type() {
    let prog = parse_ok(
        r#"space T {
  type Priority = | High | Low
  state {
    p: Priority = High
  }
}"#,
    );
    match &prog.space.body.state.fields[0].type_ann.kind {
        TypeKind::Named(name) => assert_eq!(name, "Priority"),
        other => panic!("expected named type, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────
// Block Ordering (E600)
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_block_order_violation() {
    // action before state should produce E600
    let errors = error_count(
        r#"space T {
  action go() {
    set x = 1
  }
  state {
    x: number = 0
  }
}"#,
    );
    assert!(errors > 0, "expected block ordering error");
}

#[test]
fn test_correct_block_order() {
    // All blocks in correct order should parse cleanly
    let errors = error_count(
        r#"space T {
  type P = | High | Low
  state {
    x: number = 0
    p: P = High
  }
  capabilities {
    required: [http]
  }
  credentials {
    key: string
  }
  derived {
    doubled: number = x * 2
  }
  invariant pos {
    x >= 0
  }
  action go() {
    set x = x + 1
  }
  view main() -> Surface {
    Text { value: "hi" }
  }
}"#,
    );
    assert_eq!(errors, 0, "expected no errors");
}

// ─────────────────────────────────────────────────────────────────────
// Error Recovery
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_missing_state_block() {
    // No state block — should still produce a program with errors
    let result = parse(
        r#"space T {
  action go() {
    set x = 1
  }
}"#,
    );
    assert!(result.errors.has_errors());
    // Should still produce a program (error recovery)
    assert!(result.program.is_some());
}

#[test]
fn test_comparison_chaining_rejected() {
    let errors = error_count(
        r#"space T {
  state {
    x: number = 0
  }
  invariant bad {
    0 < x < 10
  }
}"#,
    );
    assert!(errors > 0, "expected comparison chaining error");
}

// ─────────────────────────────────────────────────────────────────────
// Full Programs
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_water_tracker() {
    let prog = parse_ok(
        r#"space WaterTracker {
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

  view main() -> Surface {
    Column {
      spacing: 16,
    } {
      Text { value: "${glasses} / ${daily_goal} glasses" }
      ProgressBar { progress: progress }
      Button { label: "Drink Water", on_tap: drink }
      Button { label: "Reset", on_tap: reset }
    }
  }
}"#,
    );
    assert_eq!(prog.space.name.name, "WaterTracker");
    assert_eq!(prog.space.body.state.fields.len(), 2);
    assert_eq!(prog.space.body.derived.as_ref().unwrap().fields.len(), 2);
    assert_eq!(prog.space.body.invariants.len(), 1);
    assert_eq!(prog.space.body.actions.len(), 2);
    assert_eq!(prog.space.body.views.len(), 1);
}

#[test]
fn test_todo_app() {
    let prog = parse_ok(
        r#"space TodoApp {
  state {
    items: list<{ text: string, done: bool }> = []
    input: string = ""
  }

  action add_item() {
    if string.length(input) > 0 {
      set items = list.append(items, { text: input, done: false })
      set input = ""
    }
  }

  action toggle(index: number) {
    let item = list.get(items, index)
    let toggled = { ...item, done: not item.done }
    set items = list.set(items, index, toggled)
  }

  view main() -> Surface {
    Column {
      spacing: 8,
    } {
      TextInput { value: input, on_change: fn(v: string) { set input = v } }
      Button { label: "Add", on_tap: add_item }
      for item, idx in items {
        Row {
          spacing: 8,
        } {
          Text { value: item.text }
          Button {
            label: if item.done { "Undo" } else { "Done" },
            on_tap: toggle(idx),
          }
        }
      }
    }
  }
}

tests {
  test "starts empty" {
    assert list.length(items) == 0
  }
}"#,
    );
    assert_eq!(prog.space.name.name, "TodoApp");
    assert_eq!(prog.space.body.actions.len(), 2);
    assert_eq!(prog.tests.len(), 1);
}

// ─────────────────────────────────────────────────────────────────────
// Determinism
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_parse_determinism_100_iterations() {
    let source = r#"space Counter {
  state {
    count: number = 0
  }
  action increment() {
    set count = count + 1
  }
  view main() -> Surface {
    Column {
      spacing: 8,
    } {
      Text { value: "${count}" }
      Button { label: "+1", on_tap: increment }
    }
  }
}"#;
    let first = parse(source);
    let first_program = format!("{:?}", first.program);
    let first_errors = first.errors.total_errors;
    for _ in 1..100 {
        let result = parse(source);
        assert_eq!(format!("{:?}", result.program), first_program);
        assert_eq!(result.errors.total_errors, first_errors);
    }
}

#[test]
fn test_nested_record_type_no_keyword_clash() {
    // Nested record type in state — works when field names avoid keywords.
    // Note: `color` is the KwColor keyword and cannot be used as a field name
    // in record type positions (the parser's expect_identifier() fails on keywords).
    let source = r#"space T {
  state {
    settings: { theme: { clr: string, size: number }, lang: string } = { theme: { clr: "blue", size: 12 }, lang: "en" }
  }
}"#;
    let result = parse(source);
    let result = parse(source);
    assert!(!result.errors.has_errors(), "should parse without errors");
    assert!(result.program.is_some());
}

#[test]
fn test_named_type_workaround() {
    // Use named types (type aliases) to flatten record type nesting
    let source = r#"space T {
  type Theme = { clr: string, size: number }
  type Settings = { theme: Theme, lang: string }
  state {
    settings: Settings = { theme: { clr: "blue", size: 12 }, lang: "en" }
  }
}"#;
    let result = parse(source);
    assert!(!result.errors.has_errors(), "should parse without errors");
    assert!(result.program.is_some());
}

#[test]
fn test_3_level_nested_set_with_named_types() {
    let source = r#"space T {
  type Theme = { clr: string, size: number }
  type Settings = { theme: Theme, lang: string }
  state {
    settings: Settings = { theme: { clr: "blue", size: 12 }, lang: "en" }
  }
  action changeColor(c: string) {
    set settings.theme.clr = c
  }
}"#;
    let result = parse(source);
    assert!(!result.errors.has_errors(), "should parse without errors");
    assert!(result.program.is_some());
}

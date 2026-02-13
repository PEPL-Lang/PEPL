//! Comprehensive lexer tests for PEPL Phase 2.
//!
//! Covers: all 52 reserved keywords, operators, literals (number, string,
//! interpolated), comments, block comment rejection, newline handling,
//! module name reservation, edge cases, error recovery, and the
//! 100-iteration determinism test.

use pepl_lexer::{Lexer, TokenKind};
use pepl_types::SourceFile;

// ─────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────

/// Lex source text and return just the token kinds (excluding final Eof).
fn kinds(source: &str) -> Vec<TokenKind> {
    let sf = SourceFile::new("test.pepl", source);
    let result = Lexer::new(&sf).lex();
    result
        .tokens
        .into_iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .map(|t| t.kind)
        .collect()
}

/// Lex and return all token kinds including Eof.
fn kinds_with_eof(source: &str) -> Vec<TokenKind> {
    let sf = SourceFile::new("test.pepl", source);
    Lexer::new(&sf)
        .lex()
        .tokens
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

/// Lex and return the error count.
fn error_count(source: &str) -> usize {
    let sf = SourceFile::new("test.pepl", source);
    let result = Lexer::new(&sf).lex();
    result.errors.total_errors
}

/// Lex and return the first error message.
fn first_error(source: &str) -> String {
    let sf = SourceFile::new("test.pepl", source);
    let result = Lexer::new(&sf).lex();
    result
        .errors
        .errors
        .first()
        .map(|e| e.message.clone())
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────
// 2.3: Test all 39 reserved keywords
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_structural_keywords() {
    let pairs = [
        ("space", TokenKind::Space),
        ("state", TokenKind::State),
        ("action", TokenKind::Action),
        ("view", TokenKind::View),
        ("set", TokenKind::Set),
        ("let", TokenKind::Let),
        ("if", TokenKind::If),
        ("else", TokenKind::Else),
        ("for", TokenKind::For),
        ("in", TokenKind::In),
        ("match", TokenKind::Match),
        ("return", TokenKind::Return),
    ];
    for (src, expected) in &pairs {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "keyword '{src}'");
    }
}

#[test]
fn test_declaration_keywords() {
    let pairs = [
        ("invariant", TokenKind::Invariant),
        ("capabilities", TokenKind::Capabilities),
        ("required", TokenKind::Required),
        ("optional", TokenKind::Optional),
        ("credentials", TokenKind::Credentials),
        ("derived", TokenKind::Derived),
        ("tests", TokenKind::Tests),
        ("test", TokenKind::Test),
        ("assert", TokenKind::Assert),
    ];
    for (src, expected) in &pairs {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "keyword '{src}'");
    }
}

#[test]
fn test_expression_keywords() {
    let pairs = [
        ("fn", TokenKind::Fn),
        ("type", TokenKind::Type),
        ("true", TokenKind::True),
        ("false", TokenKind::False),
        ("nil", TokenKind::Nil),
        ("not", TokenKind::Not),
        ("and", TokenKind::And),
        ("or", TokenKind::Or),
    ];
    for (src, expected) in &pairs {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "keyword '{src}'");
    }
}

#[test]
fn test_type_name_keywords() {
    let pairs = [
        ("number", TokenKind::KwNumber),
        ("string", TokenKind::KwString),
        ("bool", TokenKind::KwBool),
        ("list", TokenKind::KwList),
        ("color", TokenKind::KwColor),
    ];
    for (src, expected) in &pairs {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "keyword '{src}'");
    }
}

#[test]
fn test_builtin_type_and_gameloop_keywords() {
    let pairs = [
        ("update", TokenKind::Update),
        ("handleEvent", TokenKind::HandleEvent),
        ("Result", TokenKind::KwResult),
        ("Surface", TokenKind::KwSurface),
        ("InputEvent", TokenKind::KwInputEvent),
    ];
    for (src, expected) in &pairs {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "keyword '{src}'");
    }
}

#[test]
fn test_all_39_keywords() {
    // Verify exact count: 12 + 9 + 8 + 5 + 5 = 39
    let keywords_39: Vec<&str> = vec![
        "space",
        "state",
        "action",
        "view",
        "set",
        "let",
        "if",
        "else",
        "for",
        "in",
        "match",
        "return",
        "invariant",
        "capabilities",
        "required",
        "optional",
        "credentials",
        "derived",
        "tests",
        "test",
        "assert",
        "fn",
        "type",
        "true",
        "false",
        "nil",
        "not",
        "and",
        "or",
        "number",
        "string",
        "bool",
        "list",
        "color",
        "update",
        "handleEvent",
        "Result",
        "Surface",
        "InputEvent",
    ];
    assert_eq!(keywords_39.len(), 39);
    for kw in &keywords_39 {
        let k = kinds(kw);
        assert_eq!(k.len(), 1, "keyword '{kw}' should lex to exactly 1 token");
        assert!(
            k[0].is_keyword(),
            "'{kw}' should be recognised as a keyword"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────
// Module name reservation
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_module_names_reserved() {
    let modules = ["core", "math", "record", "time", "convert", "json", "timer"];
    for m in &modules {
        let k = kinds(m);
        assert_eq!(k.len(), 1);
        assert!(k[0].is_keyword(), "module name '{m}' should be a keyword");
    }
}

#[test]
fn test_capability_names_reserved() {
    let caps = [
        "http",
        "storage",
        "location",
        "notifications",
        "clipboard",
        "share",
    ];
    for c in &caps {
        let k = kinds(c);
        assert_eq!(k.len(), 1);
        assert!(
            k[0].is_keyword(),
            "capability name '{c}' should be a keyword"
        );
    }
}

#[test]
fn test_module_and_capability_count() {
    // 7 modules + 6 capabilities = 13
    let reserved_module_cap = [
        "core",
        "math",
        "record",
        "time",
        "convert",
        "json",
        "timer",
        "http",
        "storage",
        "location",
        "notifications",
        "clipboard",
        "share",
    ];
    assert_eq!(reserved_module_cap.len(), 13);
    for name in &reserved_module_cap {
        assert!(
            TokenKind::from_keyword(name).is_some(),
            "'{name}' should be a reserved identifier"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────
// Operator tokens
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_all_operator_tokens() {
    let cases = [
        ("+", TokenKind::Plus),
        ("-", TokenKind::Minus),
        ("*", TokenKind::Star),
        ("/", TokenKind::Slash),
        ("%", TokenKind::Percent),
        ("==", TokenKind::EqEq),
        ("!=", TokenKind::BangEq),
        ("<", TokenKind::Less),
        (">", TokenKind::Greater),
        ("<=", TokenKind::LessEq),
        (">=", TokenKind::GreaterEq),
        ("?", TokenKind::Question),
        ("??", TokenKind::QuestionQuestion),
        ("...", TokenKind::DotDotDot),
    ];
    for (src, expected) in &cases {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "operator '{src}'");
    }
}

#[test]
fn test_arrow_operator() {
    let k = kinds("->");
    assert_eq!(k, vec![TokenKind::Arrow]);
}

#[test]
fn test_pipe_operator() {
    let k = kinds("|");
    assert_eq!(k, vec![TokenKind::Pipe]);
}

// ─────────────────────────────────────────────────────────────────────
// Number literals
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_integer_literals() {
    assert_eq!(kinds("0"), vec![TokenKind::NumberLit(0.0)]);
    assert_eq!(kinds("42"), vec![TokenKind::NumberLit(42.0)]);
    assert_eq!(kinds("100"), vec![TokenKind::NumberLit(100.0)]);
    assert_eq!(kinds("999999"), vec![TokenKind::NumberLit(999999.0)]);
}

#[test]
fn test_decimal_literals() {
    assert_eq!(kinds("3.15"), vec![TokenKind::NumberLit(3.15)]);
    assert_eq!(kinds("0.5"), vec![TokenKind::NumberLit(0.5)]);
    assert_eq!(kinds("100.0"), vec![TokenKind::NumberLit(100.0)]);
}

#[test]
fn test_number_followed_by_dot_no_digit() {
    // `42.` followed by something that isn't a digit → number + dot
    let k = kinds("42.field");
    assert_eq!(k[0], TokenKind::NumberLit(42.0));
    assert_eq!(k[1], TokenKind::Dot);
    assert_eq!(k[2], TokenKind::Identifier("field".into()));
}

// ─────────────────────────────────────────────────────────────────────
// String literals — plain
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_plain_string() {
    assert_eq!(
        kinds(r#""hello""#),
        vec![TokenKind::StringLiteral("hello".into())]
    );
    assert_eq!(kinds(r#""""#), vec![TokenKind::StringLiteral("".into())]);
    assert_eq!(
        kinds(r#""hello world""#),
        vec![TokenKind::StringLiteral("hello world".into())]
    );
}

#[test]
fn test_string_escape_sequences() {
    assert_eq!(
        kinds(r#""a\"b""#),
        vec![TokenKind::StringLiteral("a\"b".into())]
    );
    assert_eq!(
        kinds(r#""a\\b""#),
        vec![TokenKind::StringLiteral("a\\b".into())]
    );
    assert_eq!(
        kinds(r#""a\nb""#),
        vec![TokenKind::StringLiteral("a\nb".into())]
    );
    assert_eq!(
        kinds(r#""a\tb""#),
        vec![TokenKind::StringLiteral("a\tb".into())]
    );
    assert_eq!(
        kinds(r#""a\rb""#),
        vec![TokenKind::StringLiteral("a\rb".into())]
    );
    assert_eq!(
        kinds(r#""a\$b""#),
        vec![TokenKind::StringLiteral("a$b".into())]
    );
}

// ─────────────────────────────────────────────────────────────────────
// String literals — interpolation
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_string_interpolation_simple() {
    // "hello ${name}"
    let k = kinds(r#""hello ${name}""#);
    assert_eq!(
        k,
        vec![
            TokenKind::StringStart("hello ".into()),
            TokenKind::InterpolationStart,
            TokenKind::Identifier("name".into()),
            TokenKind::InterpolationEnd,
            TokenKind::StringEnd("".into()),
        ]
    );
}

#[test]
fn test_string_interpolation_multiple() {
    // "${a} and ${b}"
    let k = kinds(r#""${a} and ${b}""#);
    assert_eq!(
        k,
        vec![
            TokenKind::StringStart("".into()),
            TokenKind::InterpolationStart,
            TokenKind::Identifier("a".into()),
            TokenKind::InterpolationEnd,
            TokenKind::StringPart(" and ".into()),
            TokenKind::InterpolationStart,
            TokenKind::Identifier("b".into()),
            TokenKind::InterpolationEnd,
            TokenKind::StringEnd("".into()),
        ]
    );
}

#[test]
fn test_string_interpolation_with_expr() {
    // "count: ${x + 1}"
    let k = kinds(r#""count: ${x + 1}""#);
    assert_eq!(
        k,
        vec![
            TokenKind::StringStart("count: ".into()),
            TokenKind::InterpolationStart,
            TokenKind::Identifier("x".into()),
            TokenKind::Plus,
            TokenKind::NumberLit(1.0),
            TokenKind::InterpolationEnd,
            TokenKind::StringEnd("".into()),
        ]
    );
}

#[test]
fn test_string_interpolation_with_braces() {
    // "result: ${if done { "yes" } else { "no" }}"
    // Inner braces should not close the interpolation
    let k = kinds(r#""result: ${if done { "yes" } else { "no" }}""#);
    // StringStart("result: "), InterpolationStart, if, done, {, "yes", }, else, {, "no", }, InterpolationEnd, StringEnd("")
    assert!(k.contains(&TokenKind::InterpolationStart));
    assert!(k.contains(&TokenKind::InterpolationEnd));
    assert!(k.last() == Some(&TokenKind::StringEnd("".into())));
}

#[test]
fn test_string_with_escaped_dollar() {
    // "price: \${42}"  — the \$ prevents interpolation
    let k = kinds(r#""price: \${42}""#);
    assert_eq!(k, vec![TokenKind::StringLiteral("price: ${42}".into())]);
}

// ─────────────────────────────────────────────────────────────────────
// Comment stripping
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_single_line_comment_stripped() {
    let k = kinds("42 // this is a comment");
    assert_eq!(k, vec![TokenKind::NumberLit(42.0)]);
}

#[test]
fn test_comment_only_line() {
    let k = kinds("// just a comment");
    assert!(k.is_empty());
}

#[test]
fn test_comment_before_code() {
    let k = kinds("// comment\n42");
    assert_eq!(k, vec![TokenKind::Newline, TokenKind::NumberLit(42.0)]);
}

// ─────────────────────────────────────────────────────────────────────
// Block comment rejection (E603)
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_block_comment_rejected() {
    let count = error_count("/* block comment */");
    assert_eq!(count, 1);
    let msg = first_error("/* block comment */");
    assert!(msg.contains("single-line comments"), "error message: {msg}");
}

#[test]
fn test_block_comment_multiline_rejected() {
    let count = error_count("/* multi\nline\ncomment */");
    assert_eq!(count, 1);
}

#[test]
fn test_block_comment_unclosed_rejected() {
    let count = error_count("/* unclosed");
    assert_eq!(count, 1);
}

// ─────────────────────────────────────────────────────────────────────
// Newline handling
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_newlines_are_tokens() {
    let k = kinds("a\nb");
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("a".into()),
            TokenKind::Newline,
            TokenKind::Identifier("b".into()),
        ]
    );
}

#[test]
fn test_multiple_newlines() {
    let k = kinds("a\n\n\nb");
    // Each newline is its own token
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("a".into()),
            TokenKind::Newline,
            TokenKind::Newline,
            TokenKind::Newline,
            TokenKind::Identifier("b".into()),
        ]
    );
}

#[test]
fn test_no_trailing_newline() {
    let k = kinds("42");
    assert_eq!(k, vec![TokenKind::NumberLit(42.0)]);
}

#[test]
fn test_trailing_newline() {
    let k = kinds("42\n");
    assert_eq!(k, vec![TokenKind::NumberLit(42.0), TokenKind::Newline]);
}

// ─────────────────────────────────────────────────────────────────────
// Punctuation
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_punctuation_tokens() {
    let cases = [
        ("(", TokenKind::LParen),
        (")", TokenKind::RParen),
        ("{", TokenKind::LBrace),
        ("}", TokenKind::RBrace),
        ("[", TokenKind::LBracket),
        ("]", TokenKind::RBracket),
        (",", TokenKind::Comma),
        (":", TokenKind::Colon),
        (".", TokenKind::Dot),
        ("=", TokenKind::Eq),
        ("_", TokenKind::Underscore),
    ];
    for (src, expected) in &cases {
        let k = kinds(src);
        assert_eq!(k, vec![expected.clone()], "punctuation '{src}'");
    }
}

// ─────────────────────────────────────────────────────────────────────
// Underscore and identifiers
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_underscore_standalone() {
    let k = kinds("_");
    assert_eq!(k, vec![TokenKind::Underscore]);
}

#[test]
fn test_underscore_in_let() {
    // let _ = expr
    let k = kinds("let _ = 42");
    assert_eq!(
        k,
        vec![
            TokenKind::Let,
            TokenKind::Underscore,
            TokenKind::Eq,
            TokenKind::NumberLit(42.0),
        ]
    );
}

#[test]
fn test_identifier_starting_with_underscore() {
    let k = kinds("_foo");
    assert_eq!(k, vec![TokenKind::Identifier("_foo".into())]);
}

#[test]
fn test_identifier_with_underscores() {
    let k = kinds("my_var_name");
    assert_eq!(k, vec![TokenKind::Identifier("my_var_name".into())]);
}

// ─────────────────────────────────────────────────────────────────────
// Error recovery
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_unexpected_character_error() {
    let count = error_count("@");
    assert_eq!(count, 1);
    let msg = first_error("@");
    assert!(msg.contains("Unexpected character"), "got: {msg}");
}

#[test]
fn test_bang_without_eq_error() {
    // ! alone (not !=) should error
    let count = error_count("!");
    assert_eq!(count, 1);
    let msg = first_error("!");
    assert!(msg.contains("!"), "got: {msg}");
}

#[test]
fn test_unterminated_string_error() {
    let count = error_count(r#""unterminated"#);
    assert_eq!(count, 1);
    let msg = first_error(r#""unterminated"#);
    assert!(msg.contains("Unterminated"), "got: {msg}");
}

#[test]
fn test_invalid_escape_error() {
    let count = error_count(r#""\z""#);
    assert_eq!(count, 1);
    let msg = first_error(r#""\z""#);
    assert!(msg.contains("escape sequence"), "got: {msg}");
}

#[test]
fn test_error_recovery_continues() {
    // Multiple errors should be collected, and lexing continues
    let sf = SourceFile::new("test.pepl", "@ # ~ 42");
    let result = Lexer::new(&sf).lex();
    assert!(result.errors.total_errors >= 3);
    // Should still produce the 42 token
    assert!(result
        .tokens
        .iter()
        .any(|t| t.kind == TokenKind::NumberLit(42.0)));
}

#[test]
fn test_max_errors_cap() {
    // Generate more than 20 errors — lexer should stop at MAX_ERRORS
    let source = "@ ".repeat(25); // 25 invalid chars
    let sf = SourceFile::new("test.pepl", &source);
    let result = Lexer::new(&sf).lex();
    assert_eq!(
        result.errors.total_errors, 20,
        "should cap at MAX_ERRORS=20"
    );
}

#[test]
fn test_unterminated_interpolation() {
    // String with unclosed interpolation: "hello ${name
    let count = error_count("\"hello ${name");
    assert!(
        count >= 1,
        "unterminated interpolation should produce an error"
    );
}

// ─────────────────────────────────────────────────────────────────────
// Eof
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_eof_on_empty_source() {
    let k = kinds_with_eof("");
    assert_eq!(k, vec![TokenKind::Eof]);
}

#[test]
fn test_eof_always_last() {
    let k = kinds_with_eof("42 + 3");
    assert_eq!(k.last(), Some(&TokenKind::Eof));
}

// ─────────────────────────────────────────────────────────────────────
// Complex real-world samples
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_state_field_declaration() {
    let k = kinds("count: number = 0");
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("count".into()),
            TokenKind::Colon,
            TokenKind::KwNumber,
            TokenKind::Eq,
            TokenKind::NumberLit(0.0),
        ]
    );
}

#[test]
fn test_action_signature() {
    let k = kinds("action increment(amount: number)");
    assert_eq!(
        k,
        vec![
            TokenKind::Action,
            TokenKind::Identifier("increment".into()),
            TokenKind::LParen,
            TokenKind::Identifier("amount".into()),
            TokenKind::Colon,
            TokenKind::KwNumber,
            TokenKind::RParen,
        ]
    );
}

#[test]
fn test_set_statement() {
    let k = kinds("set count = count + 1");
    assert_eq!(
        k,
        vec![
            TokenKind::Set,
            TokenKind::Identifier("count".into()),
            TokenKind::Eq,
            TokenKind::Identifier("count".into()),
            TokenKind::Plus,
            TokenKind::NumberLit(1.0),
        ]
    );
}

#[test]
fn test_sum_type_declaration() {
    let k = kinds("type Status = | Active | Paused | Done");
    assert_eq!(
        k,
        vec![
            TokenKind::Type,
            TokenKind::Identifier("Status".into()),
            TokenKind::Eq,
            TokenKind::Pipe,
            TokenKind::Identifier("Active".into()),
            TokenKind::Pipe,
            TokenKind::Identifier("Paused".into()),
            TokenKind::Pipe,
            TokenKind::Identifier("Done".into()),
        ]
    );
}

#[test]
fn test_view_header() {
    let k = kinds("view main() -> Surface");
    assert_eq!(
        k,
        vec![
            TokenKind::View,
            TokenKind::Identifier("main".into()),
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::Arrow,
            TokenKind::KwSurface,
        ]
    );
}

#[test]
fn test_component_with_trailing_comma() {
    let k = kinds(r#"Text { value: "hi", }"#);
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("Text".into()),
            TokenKind::LBrace,
            TokenKind::Identifier("value".into()),
            TokenKind::Colon,
            TokenKind::StringLiteral("hi".into()),
            TokenKind::Comma,
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn test_record_spread() {
    let k = kinds("{ ...existing, name: value }");
    assert_eq!(
        k,
        vec![
            TokenKind::LBrace,
            TokenKind::DotDotDot,
            TokenKind::Identifier("existing".into()),
            TokenKind::Comma,
            TokenKind::Identifier("name".into()),
            TokenKind::Colon,
            TokenKind::Identifier("value".into()),
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn test_match_expression() {
    let src = "match status {\n  Active -> 1,\n  Done -> 2,\n}";
    let k = kinds(src);
    assert!(k.contains(&TokenKind::Match));
    assert!(k.contains(&TokenKind::Arrow));
}

#[test]
fn test_qualified_call() {
    let k = kinds("math.round(3.15)");
    assert_eq!(
        k,
        vec![
            TokenKind::Math,
            TokenKind::Dot,
            TokenKind::Identifier("round".into()),
            TokenKind::LParen,
            TokenKind::NumberLit(3.15),
            TokenKind::RParen,
        ]
    );
}

#[test]
fn test_nil_coalescing() {
    let k = kinds("value ?? default");
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("value".into()),
            TokenKind::QuestionQuestion,
            TokenKind::Identifier("default".into()),
        ]
    );
}

#[test]
fn test_result_unwrap() {
    let k = kinds("result?");
    assert_eq!(
        k,
        vec![TokenKind::Identifier("result".into()), TokenKind::Question,]
    );
}

#[test]
fn test_lambda_expression() {
    let k = kinds("fn(x) { x + 1 }");
    assert_eq!(
        k,
        vec![
            TokenKind::Fn,
            TokenKind::LParen,
            TokenKind::Identifier("x".into()),
            TokenKind::RParen,
            TokenKind::LBrace,
            TokenKind::Identifier("x".into()),
            TokenKind::Plus,
            TokenKind::NumberLit(1.0),
            TokenKind::RBrace,
        ]
    );
}

#[test]
fn test_list_literal() {
    let k = kinds("[1, 2, 3]");
    assert_eq!(
        k,
        vec![
            TokenKind::LBracket,
            TokenKind::NumberLit(1.0),
            TokenKind::Comma,
            TokenKind::NumberLit(2.0),
            TokenKind::Comma,
            TokenKind::NumberLit(3.0),
            TokenKind::RBracket,
        ]
    );
}

#[test]
fn test_boolean_operators() {
    let k = kinds("a and b or not c");
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("a".into()),
            TokenKind::And,
            TokenKind::Identifier("b".into()),
            TokenKind::Or,
            TokenKind::Not,
            TokenKind::Identifier("c".into()),
        ]
    );
}

#[test]
fn test_comparison_chain() {
    let k = kinds("a >= b and c != d");
    assert_eq!(
        k,
        vec![
            TokenKind::Identifier("a".into()),
            TokenKind::GreaterEq,
            TokenKind::Identifier("b".into()),
            TokenKind::And,
            TokenKind::Identifier("c".into()),
            TokenKind::BangEq,
            TokenKind::Identifier("d".into()),
        ]
    );
}

// ─────────────────────────────────────────────────────────────────────
// Span correctness
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_span_positions() {
    let sf = SourceFile::new("test.pepl", "let x = 42");
    let result = Lexer::new(&sf).lex();
    let tokens: Vec<_> = result
        .tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .collect();

    // `let` starts at col 1
    assert_eq!(tokens[0].span.start_line, 1);
    assert_eq!(tokens[0].span.start_col, 1);

    // `x` starts at col 5
    assert_eq!(tokens[1].span.start_line, 1);
    assert_eq!(tokens[1].span.start_col, 5);

    // `=` at col 7
    assert_eq!(tokens[2].span.start_line, 1);
    assert_eq!(tokens[2].span.start_col, 7);

    // `42` at col 9
    assert_eq!(tokens[3].span.start_line, 1);
    assert_eq!(tokens[3].span.start_col, 9);
}

#[test]
fn test_span_multiline() {
    let sf = SourceFile::new("test.pepl", "a\nb");
    let result = Lexer::new(&sf).lex();
    let tokens: Vec<_> = result
        .tokens
        .iter()
        .filter(|t| t.kind != TokenKind::Eof)
        .collect();

    // `a` on line 1
    assert_eq!(tokens[0].span.start_line, 1);
    // newline
    assert_eq!(tokens[1].kind, TokenKind::Newline);
    // `b` on line 2
    assert_eq!(tokens[2].span.start_line, 2);
    assert_eq!(tokens[2].span.start_col, 1);
}

// ─────────────────────────────────────────────────────────────────────
// 100-iteration determinism test
// ─────────────────────────────────────────────────────────────────────

#[test]
fn test_lexer_determinism_100_iterations() {
    let source = r#"
space Counter {
  type Status = | Active | Paused

  state {
    count: number = 0
    name: string = "default"
    active: bool = true
  }

  capabilities {
    required: [http]
    optional: [storage]
  }

  derived {
    display_count: string = "Count: ${count}"
  }

  invariant non_negative {
    count >= 0
  }

  action increment(amount: number) {
    set count = count + amount
  }

  action reset() {
    set count = 0
    set name = "reset"
    return
  }

  view main() -> Surface {
    Column {
      Text { value: display_count, }
      Button { label: "Add", on_tap: increment(1), }
    }
  }
}

tests {
  test "increment works" {
    assert count == 0
    increment(5)
    assert count == 5
  }
}
"#;

    let sf = SourceFile::new("counter.pepl", source);
    let baseline = Lexer::new(&sf).lex();
    let baseline_kinds: Vec<TokenKind> = baseline.tokens.into_iter().map(|t| t.kind).collect();

    for i in 1..100 {
        let sf = SourceFile::new("counter.pepl", source);
        let result = Lexer::new(&sf).lex();
        let result_kinds: Vec<TokenKind> = result.tokens.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            baseline_kinds, result_kinds,
            "Determinism failed on iteration {i}: token streams differ"
        );
    }
}

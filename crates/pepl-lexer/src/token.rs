//! Token types for the PEPL lexer.
//!
//! Defines [`TokenKind`] covering every lexeme in PEPL Phase 0 and
//! [`Token`], which pairs a kind with a source [`Span`].

use pepl_types::Span;
use std::fmt;

/// All 52 reserved identifiers in PEPL Phase 0.
///
/// These cannot be used as user-defined names. The lexer recognises each
/// one and emits a specific keyword token instead of [`TokenKind::Identifier`].
pub const ALL_KEYWORDS: &[&str] = &[
    // Structural (12)
    "space", "state", "action", "view", "set", "let", "if", "else", "for", "in",
    "match", "return",
    // Declarations (9)
    "invariant", "capabilities", "required", "optional", "credentials", "derived",
    "tests", "test", "assert",
    // Expressions (8)
    "fn", "type", "true", "false", "nil", "not", "and", "or",
    // Type names (5)
    "number", "string", "bool", "list", "color",
    // Built-in types & game loop (5)
    "update", "handleEvent", "Result", "Surface", "InputEvent",
    // Module names (7)
    "core", "math", "record", "time", "convert", "json", "timer",
    // Capability names (6)
    "http", "storage", "location", "notifications", "clipboard", "share",
];

// ─────────────────────────────────────────────────────────────────────
// Token
// ─────────────────────────────────────────────────────────────────────

/// A single token produced by the PEPL lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What kind of token this is.
    pub kind: TokenKind,
    /// Source location.
    pub span: Span,
}

impl Token {
    /// Create a new token.
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Returns `true` if this token is a reserved keyword.
    pub fn is_keyword(&self) -> bool {
        self.kind.is_keyword()
    }
}

// ─────────────────────────────────────────────────────────────────────
// TokenKind
// ─────────────────────────────────────────────────────────────────────

/// Every token kind in the PEPL language.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── Literals ──────────────────────────────────────────────

    /// Numeric literal (integer or decimal): `42`, `3.14`
    NumberLit(f64),
    /// Complete string literal with no interpolation: `"hello"`
    StringLiteral(String),
    /// `true`
    True,
    /// `false`
    False,
    /// `nil`
    Nil,

    // ── String Interpolation ─────────────────────────────────

    /// Start of an interpolated string — text before the first `${`.
    /// Example: for `"hello ${name}"`, carries `"hello "`.
    StringStart(String),
    /// Text between a `}` and the next `${` inside an interpolated string.
    StringPart(String),
    /// End of an interpolated string — text after the last `}` to `"`.
    StringEnd(String),
    /// The `${` that opens an interpolation expression.
    InterpolationStart,
    /// The `}` that closes an interpolation expression.
    InterpolationEnd,

    // ── Identifiers ──────────────────────────────────────────

    /// User-defined identifier: `my_var`, `add_item`
    Identifier(String),

    // ── Structural Keywords ──────────────────────────────────

    /// `space`
    Space,
    /// `state`
    State,
    /// `action`
    Action,
    /// `view`
    View,
    /// `set`
    Set,
    /// `let`
    Let,
    /// `if`
    If,
    /// `else`
    Else,
    /// `for`
    For,
    /// `in`
    In,
    /// `match`
    Match,
    /// `return`
    Return,

    // ── Declaration Keywords ─────────────────────────────────

    /// `invariant`
    Invariant,
    /// `capabilities`
    Capabilities,
    /// `required`
    Required,
    /// `optional`
    Optional,
    /// `credentials`
    Credentials,
    /// `derived`
    Derived,
    /// `tests`
    Tests,
    /// `test`
    Test,
    /// `assert`
    Assert,

    // ── Expression Keywords ──────────────────────────────────

    /// `fn`
    Fn,
    /// `type`
    Type,
    /// `not` (unary boolean negation)
    Not,
    /// `and` (boolean conjunction)
    And,
    /// `or` (boolean disjunction)
    Or,

    // ── Type-Name Keywords ───────────────────────────────────

    /// `number` (type name; also used in type annotations)
    KwNumber,
    /// `string` (type name and module prefix: `string.length()`)
    KwString,
    /// `bool` (type name)
    KwBool,
    /// `list` (type name and module prefix: `list.append()`)
    KwList,
    /// `color` (type name)
    KwColor,

    // ── Built-in Type & Game-Loop Keywords ───────────────────

    /// `update` (game-loop time-step handler)
    Update,
    /// `handleEvent` (game-loop input handler)
    HandleEvent,
    /// `Result` (built-in sum type)
    KwResult,
    /// `Surface` (view return type)
    KwSurface,
    /// `InputEvent` (built-in sum type for game input)
    KwInputEvent,

    // ── Module Names (reserved) ──────────────────────────────

    /// `core`
    Core,
    /// `math`
    Math,
    /// `record`
    Record,
    /// `time`
    Time,
    /// `convert`
    Convert,
    /// `json`
    Json,
    /// `timer`
    Timer,

    // ── Capability Names (reserved) ──────────────────────────

    /// `http`
    Http,
    /// `storage`
    Storage,
    /// `location`
    Location,
    /// `notifications`
    Notifications,
    /// `clipboard`
    Clipboard,
    /// `share`
    Share,

    // ── Operators ────────────────────────────────────────────

    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessEq,
    /// `>=`
    GreaterEq,
    /// `?` (Result unwrap postfix)
    Question,
    /// `??` (nil-coalescing)
    QuestionQuestion,
    /// `...` (record spread)
    DotDotDot,

    // ── Punctuation ──────────────────────────────────────────

    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `=`
    Eq,
    /// `->`
    Arrow,
    /// `|` (sum type variant separator)
    Pipe,
    /// `_` (wildcard / discard binding)
    Underscore,

    // ── Special ──────────────────────────────────────────────

    /// Newline (statement separator)
    Newline,
    /// End of file
    Eof,
}

impl TokenKind {
    /// Look up a reserved identifier. Returns `Some(kind)` for all 52
    /// reserved words, `None` for user identifiers.
    pub fn from_keyword(s: &str) -> Option<TokenKind> {
        Some(match s {
            // Structural
            "space" => TokenKind::Space,
            "state" => TokenKind::State,
            "action" => TokenKind::Action,
            "view" => TokenKind::View,
            "set" => TokenKind::Set,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "match" => TokenKind::Match,
            "return" => TokenKind::Return,
            // Declarations
            "invariant" => TokenKind::Invariant,
            "capabilities" => TokenKind::Capabilities,
            "required" => TokenKind::Required,
            "optional" => TokenKind::Optional,
            "credentials" => TokenKind::Credentials,
            "derived" => TokenKind::Derived,
            "tests" => TokenKind::Tests,
            "test" => TokenKind::Test,
            "assert" => TokenKind::Assert,
            // Expression keywords
            "fn" => TokenKind::Fn,
            "type" => TokenKind::Type,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "nil" => TokenKind::Nil,
            "not" => TokenKind::Not,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            // Type names
            "number" => TokenKind::KwNumber,
            "string" => TokenKind::KwString,
            "bool" => TokenKind::KwBool,
            "list" => TokenKind::KwList,
            "color" => TokenKind::KwColor,
            // Built-in types & game loop
            "update" => TokenKind::Update,
            "handleEvent" => TokenKind::HandleEvent,
            "Result" => TokenKind::KwResult,
            "Surface" => TokenKind::KwSurface,
            "InputEvent" => TokenKind::KwInputEvent,
            // Module names
            "core" => TokenKind::Core,
            "math" => TokenKind::Math,
            "record" => TokenKind::Record,
            "time" => TokenKind::Time,
            "convert" => TokenKind::Convert,
            "json" => TokenKind::Json,
            "timer" => TokenKind::Timer,
            // Capability names
            "http" => TokenKind::Http,
            "storage" => TokenKind::Storage,
            "location" => TokenKind::Location,
            "notifications" => TokenKind::Notifications,
            "clipboard" => TokenKind::Clipboard,
            "share" => TokenKind::Share,
            _ => return None,
        })
    }

    /// Returns `true` if this token kind is a reserved keyword
    /// (including module and capability names).
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            TokenKind::Space
                | TokenKind::State
                | TokenKind::Action
                | TokenKind::View
                | TokenKind::Set
                | TokenKind::Let
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::For
                | TokenKind::In
                | TokenKind::Match
                | TokenKind::Return
                | TokenKind::Invariant
                | TokenKind::Capabilities
                | TokenKind::Required
                | TokenKind::Optional
                | TokenKind::Credentials
                | TokenKind::Derived
                | TokenKind::Tests
                | TokenKind::Test
                | TokenKind::Assert
                | TokenKind::Fn
                | TokenKind::Type
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Nil
                | TokenKind::Not
                | TokenKind::And
                | TokenKind::Or
                | TokenKind::KwNumber
                | TokenKind::KwString
                | TokenKind::KwBool
                | TokenKind::KwList
                | TokenKind::KwColor
                | TokenKind::Update
                | TokenKind::HandleEvent
                | TokenKind::KwResult
                | TokenKind::KwSurface
                | TokenKind::KwInputEvent
                | TokenKind::Core
                | TokenKind::Math
                | TokenKind::Record
                | TokenKind::Time
                | TokenKind::Convert
                | TokenKind::Json
                | TokenKind::Timer
                | TokenKind::Http
                | TokenKind::Storage
                | TokenKind::Location
                | TokenKind::Notifications
                | TokenKind::Clipboard
                | TokenKind::Share
        )
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Literals
            TokenKind::NumberLit(n) => write!(f, "{n}"),
            TokenKind::StringLiteral(s) => write!(f, "\"{s}\""),
            TokenKind::True => f.write_str("true"),
            TokenKind::False => f.write_str("false"),
            TokenKind::Nil => f.write_str("nil"),
            // String interpolation
            TokenKind::StringStart(_) => f.write_str("string start"),
            TokenKind::StringPart(_) => f.write_str("string part"),
            TokenKind::StringEnd(_) => f.write_str("string end"),
            TokenKind::InterpolationStart => f.write_str("${"),
            TokenKind::InterpolationEnd => f.write_str("interpolation end"),
            // Identifiers
            TokenKind::Identifier(s) => f.write_str(s),
            // Keywords — display the PEPL source text
            TokenKind::Space => f.write_str("space"),
            TokenKind::State => f.write_str("state"),
            TokenKind::Action => f.write_str("action"),
            TokenKind::View => f.write_str("view"),
            TokenKind::Set => f.write_str("set"),
            TokenKind::Let => f.write_str("let"),
            TokenKind::If => f.write_str("if"),
            TokenKind::Else => f.write_str("else"),
            TokenKind::For => f.write_str("for"),
            TokenKind::In => f.write_str("in"),
            TokenKind::Match => f.write_str("match"),
            TokenKind::Return => f.write_str("return"),
            TokenKind::Invariant => f.write_str("invariant"),
            TokenKind::Capabilities => f.write_str("capabilities"),
            TokenKind::Required => f.write_str("required"),
            TokenKind::Optional => f.write_str("optional"),
            TokenKind::Credentials => f.write_str("credentials"),
            TokenKind::Derived => f.write_str("derived"),
            TokenKind::Tests => f.write_str("tests"),
            TokenKind::Test => f.write_str("test"),
            TokenKind::Assert => f.write_str("assert"),
            TokenKind::Fn => f.write_str("fn"),
            TokenKind::Type => f.write_str("type"),
            TokenKind::Not => f.write_str("not"),
            TokenKind::And => f.write_str("and"),
            TokenKind::Or => f.write_str("or"),
            TokenKind::KwNumber => f.write_str("number"),
            TokenKind::KwString => f.write_str("string"),
            TokenKind::KwBool => f.write_str("bool"),
            TokenKind::KwList => f.write_str("list"),
            TokenKind::KwColor => f.write_str("color"),
            TokenKind::Update => f.write_str("update"),
            TokenKind::HandleEvent => f.write_str("handleEvent"),
            TokenKind::KwResult => f.write_str("Result"),
            TokenKind::KwSurface => f.write_str("Surface"),
            TokenKind::KwInputEvent => f.write_str("InputEvent"),
            TokenKind::Core => f.write_str("core"),
            TokenKind::Math => f.write_str("math"),
            TokenKind::Record => f.write_str("record"),
            TokenKind::Time => f.write_str("time"),
            TokenKind::Convert => f.write_str("convert"),
            TokenKind::Json => f.write_str("json"),
            TokenKind::Timer => f.write_str("timer"),
            TokenKind::Http => f.write_str("http"),
            TokenKind::Storage => f.write_str("storage"),
            TokenKind::Location => f.write_str("location"),
            TokenKind::Notifications => f.write_str("notifications"),
            TokenKind::Clipboard => f.write_str("clipboard"),
            TokenKind::Share => f.write_str("share"),
            // Operators
            TokenKind::Plus => f.write_str("+"),
            TokenKind::Minus => f.write_str("-"),
            TokenKind::Star => f.write_str("*"),
            TokenKind::Slash => f.write_str("/"),
            TokenKind::Percent => f.write_str("%"),
            TokenKind::EqEq => f.write_str("=="),
            TokenKind::BangEq => f.write_str("!="),
            TokenKind::Less => f.write_str("<"),
            TokenKind::Greater => f.write_str(">"),
            TokenKind::LessEq => f.write_str("<="),
            TokenKind::GreaterEq => f.write_str(">="),
            TokenKind::Question => f.write_str("?"),
            TokenKind::QuestionQuestion => f.write_str("??"),
            TokenKind::DotDotDot => f.write_str("..."),
            // Punctuation
            TokenKind::LParen => f.write_str("("),
            TokenKind::RParen => f.write_str(")"),
            TokenKind::LBrace => f.write_str("{"),
            TokenKind::RBrace => f.write_str("}"),
            TokenKind::LBracket => f.write_str("["),
            TokenKind::RBracket => f.write_str("]"),
            TokenKind::Comma => f.write_str(","),
            TokenKind::Colon => f.write_str(":"),
            TokenKind::Dot => f.write_str("."),
            TokenKind::Eq => f.write_str("="),
            TokenKind::Arrow => f.write_str("->"),
            TokenKind::Pipe => f.write_str("|"),
            TokenKind::Underscore => f.write_str("_"),
            // Special
            TokenKind::Newline => f.write_str("newline"),
            TokenKind::Eof => f.write_str("end of file"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_keywords_count() {
        assert_eq!(ALL_KEYWORDS.len(), 52);
    }

    #[test]
    fn test_from_keyword_recognises_all() {
        for &kw in ALL_KEYWORDS {
            assert!(
                TokenKind::from_keyword(kw).is_some(),
                "from_keyword should recognise '{kw}'"
            );
        }
    }

    #[test]
    fn test_from_keyword_returns_none_for_identifiers() {
        let non_keywords = [
            "foo", "bar", "my_var", "handleevent", "SPACE", "True",
            "result", "surface", "inputevent", "with_responses",
        ];
        for &name in &non_keywords {
            assert!(
                TokenKind::from_keyword(name).is_none(),
                "from_keyword should not recognise '{name}'"
            );
        }
    }

    #[test]
    fn test_is_keyword_true_for_all() {
        for &kw in ALL_KEYWORDS {
            let kind = TokenKind::from_keyword(kw).unwrap();
            assert!(
                kind.is_keyword(),
                "is_keyword should return true for '{kw}'"
            );
        }
    }

    #[test]
    fn test_is_keyword_false_for_non_keywords() {
        let non_keyword_kinds = [
            TokenKind::NumberLit(42.0),
            TokenKind::StringLiteral("hi".into()),
            TokenKind::Identifier("foo".into()),
            TokenKind::Plus,
            TokenKind::LParen,
            TokenKind::Pipe,
            TokenKind::Underscore,
            TokenKind::Newline,
            TokenKind::Eof,
            TokenKind::InterpolationStart,
        ];
        for kind in &non_keyword_kinds {
            assert!(
                !kind.is_keyword(),
                "is_keyword should be false for {kind:?}"
            );
        }
    }

    #[test]
    fn test_token_construction() {
        let span = Span::new(1, 1, 1, 6);
        let token = Token::new(TokenKind::Space, span);
        assert_eq!(token.kind, TokenKind::Space);
        assert_eq!(token.span, span);
        assert!(token.is_keyword());
    }

    #[test]
    fn test_token_identifier_not_keyword() {
        let span = Span::new(1, 1, 1, 4);
        let token = Token::new(TokenKind::Identifier("foo".into()), span);
        assert!(!token.is_keyword());
    }

    #[test]
    fn test_keyword_case_sensitivity() {
        // Keywords are case-sensitive
        assert!(TokenKind::from_keyword("space").is_some());
        assert!(TokenKind::from_keyword("Space").is_none());
        assert!(TokenKind::from_keyword("SPACE").is_none());
        // PascalCase keywords must match exactly
        assert!(TokenKind::from_keyword("Result").is_some());
        assert!(TokenKind::from_keyword("result").is_none());
        assert!(TokenKind::from_keyword("Surface").is_some());
        assert!(TokenKind::from_keyword("surface").is_none());
        assert!(TokenKind::from_keyword("InputEvent").is_some());
        assert!(TokenKind::from_keyword("inputevent").is_none());
        assert!(TokenKind::from_keyword("handleEvent").is_some());
        assert!(TokenKind::from_keyword("handleevent").is_none());
    }

    #[test]
    fn test_from_keyword_structural() {
        assert_eq!(TokenKind::from_keyword("space"), Some(TokenKind::Space));
        assert_eq!(TokenKind::from_keyword("state"), Some(TokenKind::State));
        assert_eq!(TokenKind::from_keyword("action"), Some(TokenKind::Action));
        assert_eq!(TokenKind::from_keyword("view"), Some(TokenKind::View));
        assert_eq!(TokenKind::from_keyword("set"), Some(TokenKind::Set));
        assert_eq!(TokenKind::from_keyword("let"), Some(TokenKind::Let));
        assert_eq!(TokenKind::from_keyword("if"), Some(TokenKind::If));
        assert_eq!(TokenKind::from_keyword("else"), Some(TokenKind::Else));
        assert_eq!(TokenKind::from_keyword("for"), Some(TokenKind::For));
        assert_eq!(TokenKind::from_keyword("in"), Some(TokenKind::In));
        assert_eq!(TokenKind::from_keyword("match"), Some(TokenKind::Match));
        assert_eq!(TokenKind::from_keyword("return"), Some(TokenKind::Return));
    }

    #[test]
    fn test_from_keyword_modules_and_capabilities() {
        assert_eq!(TokenKind::from_keyword("core"), Some(TokenKind::Core));
        assert_eq!(TokenKind::from_keyword("math"), Some(TokenKind::Math));
        assert_eq!(TokenKind::from_keyword("record"), Some(TokenKind::Record));
        assert_eq!(TokenKind::from_keyword("time"), Some(TokenKind::Time));
        assert_eq!(TokenKind::from_keyword("convert"), Some(TokenKind::Convert));
        assert_eq!(TokenKind::from_keyword("json"), Some(TokenKind::Json));
        assert_eq!(TokenKind::from_keyword("timer"), Some(TokenKind::Timer));
        assert_eq!(TokenKind::from_keyword("http"), Some(TokenKind::Http));
        assert_eq!(TokenKind::from_keyword("storage"), Some(TokenKind::Storage));
        assert_eq!(TokenKind::from_keyword("location"), Some(TokenKind::Location));
        assert_eq!(TokenKind::from_keyword("notifications"), Some(TokenKind::Notifications));
        assert_eq!(TokenKind::from_keyword("clipboard"), Some(TokenKind::Clipboard));
        assert_eq!(TokenKind::from_keyword("share"), Some(TokenKind::Share));
    }

    #[test]
    fn test_display_keywords() {
        assert_eq!(TokenKind::Space.to_string(), "space");
        assert_eq!(TokenKind::HandleEvent.to_string(), "handleEvent");
        assert_eq!(TokenKind::KwResult.to_string(), "Result");
        assert_eq!(TokenKind::KwSurface.to_string(), "Surface");
        assert_eq!(TokenKind::KwInputEvent.to_string(), "InputEvent");
    }

    #[test]
    fn test_display_operators() {
        assert_eq!(TokenKind::Plus.to_string(), "+");
        assert_eq!(TokenKind::EqEq.to_string(), "==");
        assert_eq!(TokenKind::BangEq.to_string(), "!=");
        assert_eq!(TokenKind::QuestionQuestion.to_string(), "??");
        assert_eq!(TokenKind::DotDotDot.to_string(), "...");
        assert_eq!(TokenKind::Arrow.to_string(), "->");
    }

    #[test]
    fn test_display_punctuation() {
        assert_eq!(TokenKind::LParen.to_string(), "(");
        assert_eq!(TokenKind::RBrace.to_string(), "}");
        assert_eq!(TokenKind::Pipe.to_string(), "|");
        assert_eq!(TokenKind::Underscore.to_string(), "_");
    }

    #[test]
    fn test_display_literals() {
        assert_eq!(TokenKind::NumberLit(42.0).to_string(), "42");
        assert_eq!(TokenKind::NumberLit(3.14).to_string(), "3.14");
        assert_eq!(
            TokenKind::StringLiteral("hello".into()).to_string(),
            "\"hello\""
        );
        assert_eq!(TokenKind::True.to_string(), "true");
        assert_eq!(TokenKind::False.to_string(), "false");
        assert_eq!(TokenKind::Nil.to_string(), "nil");
    }

    #[test]
    fn test_display_special() {
        assert_eq!(TokenKind::Newline.to_string(), "newline");
        assert_eq!(TokenKind::Eof.to_string(), "end of file");
        assert_eq!(
            TokenKind::Identifier("my_var".into()).to_string(),
            "my_var"
        );
    }

    #[test]
    fn test_display_roundtrip_keywords() {
        // Every keyword's Display output should match its source text
        for &kw in ALL_KEYWORDS {
            let kind = TokenKind::from_keyword(kw).unwrap();
            let display = kind.to_string();
            assert_eq!(
                display, kw,
                "Display output should match keyword text for '{kw}'"
            );
        }
    }
}

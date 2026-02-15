//! Core parser infrastructure: token cursor, error reporting, helpers.

use pepl_lexer::token::{Token, TokenKind};
use pepl_types::{CompileErrors, ErrorCode, PeplError, SourceFile, Span};

/// The PEPL parser.
///
/// Consumes a token stream produced by the lexer and builds an AST.
/// Collects errors and attempts recovery when possible.
pub struct Parser<'src> {
    /// The token stream.
    tokens: Vec<Token>,
    /// Current index into `tokens`.
    pos: usize,
    /// Source file for error context.
    source_file: &'src SourceFile,
    /// File name for error messages.
    file_name: String,
    /// Collected errors.
    errors: CompileErrors,
    /// Current lambda nesting depth (max 3).
    pub(crate) lambda_depth: u32,
    /// Current record literal nesting depth (max 4).
    pub(crate) record_depth: u32,
    /// Current expression nesting depth (max 16).
    pub(crate) expr_depth: u32,
    /// Current for-loop nesting depth (max 3).
    pub(crate) for_depth: u32,
}

/// Result of parsing.
pub struct ParseResult {
    pub program: Option<pepl_types::ast::Program>,
    pub errors: CompileErrors,
}

impl<'src> Parser<'src> {
    /// Create a new parser from a token stream and source file.
    pub fn new(tokens: Vec<Token>, source_file: &'src SourceFile) -> Self {
        Self {
            tokens,
            pos: 0,
            file_name: source_file.name.clone(),
            source_file,
            errors: CompileErrors::empty(),
            lambda_depth: 0,
            record_depth: 0,
            expr_depth: 0,
            for_depth: 0,
        }
    }

    // ── Token Cursor ──────────────────────────────────────────────────────────

    /// Returns the current token without advancing.
    pub(crate) fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("token stream should end with Eof")
        })
    }

    /// Returns the kind of the current token.
    pub(crate) fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    /// Advance the cursor by one and return the consumed token.
    pub(crate) fn advance(&mut self) -> Token {
        let token = self.peek().clone();
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        token
    }

    /// Returns the previously consumed token's span.
    pub(crate) fn previous_span(&self) -> Span {
        if self.pos > 0 {
            self.tokens[self.pos - 1].span
        } else {
            Span::point(1, 1)
        }
    }

    /// Returns the span of the current token.
    pub(crate) fn current_span(&self) -> Span {
        self.peek().span
    }

    /// Returns `true` if the current token is `Eof`.
    pub(crate) fn at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    /// Check if the current token matches the given kind (by discriminant).
    #[allow(dead_code)]
    pub(crate) fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind)
    }

    /// Check if the current token matches the given kind exactly.
    pub(crate) fn check_exact(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
    }

    /// If the current token matches, advance and return `true`.
    pub(crate) fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check_exact(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Look ahead by `n` tokens from current position.
    pub(crate) fn look_ahead(&self, n: usize) -> &TokenKind {
        let idx = self.pos + n;
        self.tokens
            .get(idx)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    // ── Newline Handling ──────────────────────────────────────────────────────

    /// Skip all consecutive newline tokens.
    pub(crate) fn skip_newlines(&mut self) {
        while self.check_exact(&TokenKind::Newline) {
            self.advance();
        }
    }

    /// Expect a newline or end of file. Reports error if neither.
    pub(crate) fn expect_newline_or_eof(&mut self) {
        if self.at_end() {
            return;
        }
        if self.check_exact(&TokenKind::Newline) {
            self.advance();
            self.skip_newlines();
        } else if !self.check_exact(&TokenKind::RBrace) {
            // RBrace is acceptable — the closing brace ends the block
            self.error_at_current(
                ErrorCode::UNEXPECTED_TOKEN,
                format!("expected newline, got '{}'", self.peek_kind()),
            );
        }
    }

    // ── Expect Helpers ────────────────────────────────────────────────────────

    /// Expect a specific token kind. Returns the token if matched, or emits an error.
    pub(crate) fn expect(&mut self, expected: &TokenKind) -> Option<Token> {
        if self.check_exact(expected) {
            Some(self.advance())
        } else {
            self.error_at_current(
                ErrorCode::UNEXPECTED_TOKEN,
                format!("expected '{}', got '{}'", expected, self.peek_kind()),
            );
            None
        }
    }

    /// Expect an identifier token. Returns the name and span.
    pub(crate) fn expect_identifier(&mut self) -> Option<pepl_types::ast::Ident> {
        match self.peek_kind().clone() {
            TokenKind::Identifier(name) => {
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected identifier, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    /// Expect an identifier OR any keyword used as a record field name.
    ///
    /// Keywords are contextually valid as field names in:
    /// - Record type fields: `{ color: string }`
    /// - Record literal fields: `{ color: "#ff0000" }`
    /// - State/derived field declarations: `color: string = "#000"`
    /// - `set` path segments after `.`: `set theme.color = "#fff"`
    pub(crate) fn expect_field_name(&mut self) -> Option<pepl_types::ast::Ident> {
        let kind = self.peek_kind().clone();
        match &kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            _ if kind.is_keyword() => {
                let name = kind.to_string();
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected field name, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    /// Expect an identifier OR a module/capability keyword used as an identifier.
    /// This handles cases like `record.get(...)` where `record` is a keyword but
    /// used as a module name in qualified calls.
    pub(crate) fn expect_ident_or_module_name(&mut self) -> Option<pepl_types::ast::Ident> {
        let kind = self.peek_kind().clone();
        match &kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            // Module names used as identifiers in qualified calls
            TokenKind::Core
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
            // Type keywords that are also module prefixes
            | TokenKind::KwString
            | TokenKind::KwList
            | TokenKind::KwColor => {
                let name = kind.to_string();
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected identifier, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    /// Expect a string literal token. Returns the string value.
    pub(crate) fn expect_string_literal(&mut self) -> Option<String> {
        match self.peek_kind().clone() {
            TokenKind::StringLiteral(s) => {
                self.advance();
                Some(s)
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected string literal, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    /// Expect an upper-case identifier (component name or type name).
    pub(crate) fn expect_upper_identifier(&mut self) -> Option<pepl_types::ast::Ident> {
        match self.peek_kind().clone() {
            TokenKind::Identifier(ref name)
                if name.starts_with(|c: char| c.is_ascii_uppercase()) =>
            {
                let name = name.clone();
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected PascalCase identifier, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    /// Eat an optional trailing comma.
    pub(crate) fn eat_comma(&mut self) -> bool {
        self.eat(&TokenKind::Comma)
    }

    /// Expect an identifier or a keyword that can serve as a field/function name
    /// after `.` (e.g., `list.set(...)` where `set` is a keyword).
    pub(crate) fn expect_member_name(&mut self) -> Option<pepl_types::ast::Ident> {
        let kind = self.peek_kind().clone();
        match &kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new(name, span))
            }
            // Keywords that can appear as function/field names after `.`
            TokenKind::Set => {
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new("set", span))
            }
            TokenKind::Type => {
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new("type", span))
            }
            TokenKind::Match => {
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new("match", span))
            }
            TokenKind::Update => {
                let span = self.advance().span;
                Some(pepl_types::ast::Ident::new("update", span))
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected identifier, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    // ── Error Reporting ───────────────────────────────────────────────────────

    /// Report an error at the current token position.
    pub(crate) fn error_at_current(&mut self, code: ErrorCode, message: impl Into<String>) {
        let span = self.current_span();
        self.error_at(code, message, span);
    }

    /// Report an error at a specific span.
    pub(crate) fn error_at(&mut self, code: ErrorCode, message: impl Into<String>, span: Span) {
        let source_line = self
            .source_file
            .line(span.start_line)
            .unwrap_or("")
            .to_string();
        let error = PeplError::new(&self.file_name, code, message, span, source_line);
        self.errors.push_error(error);
    }

    /// Returns `true` if we've hit the error limit and should stop.
    pub(crate) fn too_many_errors(&self) -> bool {
        self.errors.has_errors() && self.errors.total_errors >= pepl_types::MAX_ERRORS
    }

    // ── Synchronization ───────────────────────────────────────────────────────

    /// Skip tokens until we reach a synchronization point.
    /// Used after an error to resume at a known-good position.
    pub(crate) fn synchronize(&mut self) {
        while !self.at_end() {
            // Stop at newline — each statement starts on a new line
            if self.check_exact(&TokenKind::Newline) {
                self.advance();
                self.skip_newlines();
                return;
            }
            // Stop at block-level keywords
            match self.peek_kind() {
                TokenKind::Space
                | TokenKind::State
                | TokenKind::Action
                | TokenKind::View
                | TokenKind::Set
                | TokenKind::Let
                | TokenKind::If
                | TokenKind::For
                | TokenKind::Match
                | TokenKind::Return
                | TokenKind::Invariant
                | TokenKind::Capabilities
                | TokenKind::Credentials
                | TokenKind::Derived
                | TokenKind::Tests
                | TokenKind::Test
                | TokenKind::Assert
                | TokenKind::Type
                | TokenKind::Update
                | TokenKind::HandleEvent
                | TokenKind::RBrace => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Parse the token stream into a `Program` AST.
    pub fn parse(mut self) -> ParseResult {
        self.skip_newlines();
        let program = self.parse_program();
        ParseResult {
            program,
            errors: self.errors,
        }
    }
}

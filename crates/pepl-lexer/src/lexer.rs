//! Core PEPL lexer — converts source text to a token stream.
//!
//! Features:
//! - All PEPL Phase 0 tokens (52 reserved words, operators, punctuation, literals)
//! - String interpolation with `${expr}` via a mode stack
//! - Single-line comments stripped (`//`)
//! - Block comments rejected (`/* */`) with error E603
//! - Error recovery: collects up to 20 errors instead of stopping at the first
//! - Newline-separated statements (no semicolons)

use pepl_types::{CompileErrors, ErrorCode, PeplError, SourceFile, Span};

use crate::token::{Token, TokenKind};

/// Lexer mode — tracks whether we're scanning top-level code or inside
/// a string interpolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Normal code scanning.
    Normal,
    /// Inside a string literal — scanning text until `"` or `${`.
    String,
    /// Inside a `${...}` interpolation expression. The `u32` tracks the
    /// brace depth so we know when the interpolation's closing `}` is reached.
    Interpolation { brace_depth: u32 },
}

/// The PEPL lexer.
///
/// Converts source text into a vector of [`Token`]s, collecting up to
/// [`pepl_types::MAX_ERRORS`] errors along the way.
pub struct Lexer<'src> {
    /// The full source text as bytes.
    source: &'src [u8],
    /// Source file for error reporting.
    source_file: &'src SourceFile,
    /// File name (for errors).
    file_name: &'src str,
    /// Current byte offset into `source`.
    pos: usize,
    /// Current line number (1-based).
    line: u32,
    /// Current column number (1-based).
    col: u32,
    /// Collected errors.
    errors: CompileErrors,
    /// Mode stack for string interpolation.
    mode_stack: Vec<Mode>,
    /// Pending tokens to emit before the next scan (used for interpolation).
    pending: Vec<Token>,
}

/// Result of lexing: tokens + any errors collected.
pub struct LexResult {
    /// The token stream (always ends with [`TokenKind::Eof`]).
    pub tokens: Vec<Token>,
    /// Errors encountered during lexing.
    pub errors: CompileErrors,
}

impl<'src> Lexer<'src> {
    /// Create a new lexer for the given source file.
    pub fn new(source_file: &'src SourceFile) -> Self {
        Self {
            source: source_file.source.as_bytes(),
            source_file,
            file_name: &source_file.name,
            pos: 0,
            line: 1,
            col: 1,
            errors: CompileErrors::empty(),
            mode_stack: vec![Mode::Normal],
            pending: Vec::new(),
        }
    }

    /// Lex the entire source file into a token stream.
    pub fn lex(mut self) -> LexResult {
        let mut tokens = Vec::new();

        loop {
            if self.errors.has_errors() && self.errors.total_errors >= pepl_types::MAX_ERRORS {
                break;
            }

            // Drain any pending tokens first (e.g. InterpolationStart after StringStart)
            if let Some(pending) = self.pending.pop() {
                tokens.push(pending);
                continue;
            }

            let token = match self.current_mode() {
                Mode::Normal => self.scan_normal(),
                Mode::String => self.scan_string_continuation(),
                Mode::Interpolation { .. } => self.scan_normal(),
            };

            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);

            if is_eof {
                break;
            }
        }

        // Ensure token stream always ends with Eof
        if tokens.last().is_none_or(|t| t.kind != TokenKind::Eof) {
            tokens.push(Token::new(TokenKind::Eof, self.current_span()));
        }

        LexResult {
            tokens,
            errors: self.errors,
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Mode stack helpers
    // ─────────────────────────────────────────────────────────────

    fn current_mode(&self) -> Mode {
        *self.mode_stack.last().unwrap_or(&Mode::Normal)
    }

    fn push_mode(&mut self, mode: Mode) {
        self.mode_stack.push(mode);
    }

    fn pop_mode(&mut self) {
        if self.mode_stack.len() > 1 {
            self.mode_stack.pop();
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Character-level helpers
    // ─────────────────────────────────────────────────────────────

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    fn at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn current_span(&self) -> Span {
        Span::point(self.line, self.col)
    }

    fn span_from(&self, start_line: u32, start_col: u32) -> Span {
        Span::new(
            start_line,
            start_col,
            self.line,
            self.col.saturating_sub(1).max(1),
        )
    }

    fn source_line_at(&self, line: u32) -> String {
        self.source_file.line(line).unwrap_or("").to_string()
    }

    fn emit_error(&mut self, code: ErrorCode, message: impl Into<String>, span: Span) {
        let source_line = self.source_line_at(span.start_line);
        let err = PeplError::new(self.file_name, code, message, span, source_line);
        self.errors.push_error(err);
    }

    fn emit_error_with_suggestion(
        &mut self,
        code: ErrorCode,
        message: impl Into<String>,
        span: Span,
        suggestion: impl Into<String>,
    ) {
        let source_line = self.source_line_at(span.start_line);
        let err = PeplError::new(self.file_name, code, message, span, source_line)
            .with_suggestion(suggestion);
        self.errors.push_error(err);
    }

    // ─────────────────────────────────────────────────────────────
    // Whitespace & comments
    // ─────────────────────────────────────────────────────────────

    /// Skip spaces and tabs (NOT newlines — those are tokens).
    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == b' ' || ch == b'\t' || ch == b'\r' {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Skip a single-line comment (`// ...`).
    /// Returns `true` if a comment was consumed.
    fn skip_comment(&mut self) -> bool {
        if self.peek() == Some(b'/') && self.peek_at(1) == Some(b'/') {
            // Consume everything until end-of-line (but not the newline itself)
            while let Some(ch) = self.peek() {
                if ch == b'\n' {
                    break;
                }
                self.advance();
            }
            true
        } else {
            false
        }
    }

    /// Check for block comment (`/* ... */`) and emit E603 if found.
    /// Returns `true` if a block comment was detected.
    fn check_block_comment(&mut self) -> bool {
        if self.peek() == Some(b'/') && self.peek_at(1) == Some(b'*') {
            let start_line = self.line;
            let start_col = self.col;
            // Consume `/*`
            self.advance();
            self.advance();
            // Consume until `*/` or EOF
            loop {
                match self.peek() {
                    None => break,
                    Some(b'*') if self.peek_at(1) == Some(b'/') => {
                        self.advance();
                        self.advance();
                        break;
                    }
                    _ => {
                        self.advance();
                    }
                }
            }
            let span = self.span_from(start_line, start_col);
            self.emit_error_with_suggestion(
                ErrorCode::BLOCK_COMMENT_USED,
                "Only single-line comments (//) are supported",
                span,
                "Replace /* ... */ with // on each line",
            );
            true
        } else {
            false
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Normal-mode scanning
    // ─────────────────────────────────────────────────────────────

    /// Scan one token in normal (non-string) mode.
    fn scan_normal(&mut self) -> Token {
        // Skip whitespace
        self.skip_whitespace();

        // If we've hit the error cap, stop immediately
        if self.errors.has_errors() && self.errors.total_errors >= pepl_types::MAX_ERRORS {
            return Token::new(TokenKind::Eof, self.current_span());
        }

        // Check for EOF
        if self.at_end() {
            // If we're still inside a string or interpolation, that's an error
            if self
                .mode_stack
                .iter()
                .any(|m| matches!(m, Mode::String | Mode::Interpolation { .. }))
            {
                self.emit_error(
                    ErrorCode::UNEXPECTED_TOKEN,
                    "Unterminated string literal",
                    self.current_span(),
                );
            }
            return Token::new(TokenKind::Eof, self.current_span());
        }

        // Check for block comments before line comments
        if self.check_block_comment() {
            // Recursively get the next real token
            return self.scan_normal();
        }

        // Skip line comments
        if self.skip_comment() {
            self.skip_whitespace();
            // After a comment, either we hit a newline or EOF
            if self.at_end() {
                return Token::new(TokenKind::Eof, self.current_span());
            }
            if self.peek() == Some(b'\n') {
                let start_line = self.line;
                let start_col = self.col;
                self.advance();
                return Token::new(TokenKind::Newline, self.span_from(start_line, start_col));
            }
            // Shouldn't reach here, but continue scanning
            return self.scan_normal();
        }

        let start_line = self.line;
        let start_col = self.col;
        let ch = self.advance().unwrap();

        match ch {
            // ── Newline ──
            b'\n' => Token::new(TokenKind::Newline, self.span_from(start_line, start_col)),

            // ── String literal ──
            b'"' => self.scan_string(start_line, start_col),

            // ── Number literal ──
            b'0'..=b'9' => self.scan_number(start_line, start_col),

            // ── Identifiers & keywords ──
            b'a'..=b'z' | b'A'..=b'Z' => self.scan_identifier(start_line, start_col),

            // ── Underscore (wildcard / discard) ──
            b'_' => {
                // If followed by a letter/digit, it's part of an identifier
                if matches!(
                    self.peek(),
                    Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
                ) {
                    self.scan_identifier(start_line, start_col)
                } else {
                    Token::new(TokenKind::Underscore, self.span_from(start_line, start_col))
                }
            }

            // ── Operators & punctuation ──
            b'+' => Token::new(TokenKind::Plus, self.span_from(start_line, start_col)),
            b'*' => Token::new(TokenKind::Star, self.span_from(start_line, start_col)),
            b'%' => Token::new(TokenKind::Percent, self.span_from(start_line, start_col)),

            b'-' => {
                if self.peek() == Some(b'>') {
                    self.advance();
                    Token::new(TokenKind::Arrow, self.span_from(start_line, start_col))
                } else {
                    Token::new(TokenKind::Minus, self.span_from(start_line, start_col))
                }
            }

            b'/' => {
                // We've already handled // and /* above, so bare / is division
                Token::new(TokenKind::Slash, self.span_from(start_line, start_col))
            }

            b'=' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::new(TokenKind::EqEq, self.span_from(start_line, start_col))
                } else {
                    Token::new(TokenKind::Eq, self.span_from(start_line, start_col))
                }
            }

            b'!' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::new(TokenKind::BangEq, self.span_from(start_line, start_col))
                } else {
                    let span = self.span_from(start_line, start_col);
                    self.emit_error_with_suggestion(
                        ErrorCode::UNEXPECTED_TOKEN,
                        "Unexpected character '!'",
                        span,
                        "Use 'not' for boolean negation, or '!=' for inequality",
                    );
                    self.scan_normal()
                }
            }

            b'<' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::new(TokenKind::LessEq, self.span_from(start_line, start_col))
                } else {
                    Token::new(TokenKind::Less, self.span_from(start_line, start_col))
                }
            }

            b'>' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Token::new(TokenKind::GreaterEq, self.span_from(start_line, start_col))
                } else {
                    Token::new(TokenKind::Greater, self.span_from(start_line, start_col))
                }
            }

            b'?' => {
                if self.peek() == Some(b'?') {
                    self.advance();
                    Token::new(
                        TokenKind::QuestionQuestion,
                        self.span_from(start_line, start_col),
                    )
                } else {
                    Token::new(TokenKind::Question, self.span_from(start_line, start_col))
                }
            }

            b'.' => {
                if self.peek() == Some(b'.') && self.peek_at(1) == Some(b'.') {
                    self.advance();
                    self.advance();
                    Token::new(TokenKind::DotDotDot, self.span_from(start_line, start_col))
                } else {
                    Token::new(TokenKind::Dot, self.span_from(start_line, start_col))
                }
            }

            b'|' => Token::new(TokenKind::Pipe, self.span_from(start_line, start_col)),
            b'(' => Token::new(TokenKind::LParen, self.span_from(start_line, start_col)),
            b')' => Token::new(TokenKind::RParen, self.span_from(start_line, start_col)),
            b'[' => Token::new(TokenKind::LBracket, self.span_from(start_line, start_col)),
            b']' => Token::new(TokenKind::RBracket, self.span_from(start_line, start_col)),
            b',' => Token::new(TokenKind::Comma, self.span_from(start_line, start_col)),
            b':' => Token::new(TokenKind::Colon, self.span_from(start_line, start_col)),

            b'{' => {
                // If we're in interpolation mode, track brace depth
                if let Some(Mode::Interpolation { brace_depth }) = self.mode_stack.last_mut() {
                    *brace_depth += 1;
                }
                Token::new(TokenKind::LBrace, self.span_from(start_line, start_col))
            }

            b'}' => {
                // Check if this closes an interpolation
                let mode = self.current_mode();
                if let Mode::Interpolation { brace_depth } = mode {
                    if brace_depth == 0 {
                        // This `}` ends the interpolation — switch back to string mode
                        self.pop_mode();
                        self.push_mode(Mode::String);
                        return Token::new(
                            TokenKind::InterpolationEnd,
                            self.span_from(start_line, start_col),
                        );
                    } else {
                        // Decrease brace depth — this is just a nested `{}`
                        if let Some(Mode::Interpolation { brace_depth }) =
                            self.mode_stack.last_mut()
                        {
                            *brace_depth -= 1;
                        }
                    }
                }
                Token::new(TokenKind::RBrace, self.span_from(start_line, start_col))
            }

            _ => {
                let span = self.span_from(start_line, start_col);
                self.emit_error(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("Unexpected character '{}'", ch as char),
                    span,
                );
                // Error recovery: skip the character and try again
                self.scan_normal()
            }
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Number literals
    // ─────────────────────────────────────────────────────────────

    fn scan_number(&mut self, start_line: u32, start_col: u32) -> Token {
        // We already consumed the first digit
        while let Some(b'0'..=b'9') = self.peek() {
            self.advance();
        }

        // Check for decimal point
        if self.peek() == Some(b'.') && matches!(self.peek_at(1), Some(b'0'..=b'9')) {
            self.advance(); // consume '.'
            while let Some(b'0'..=b'9') = self.peek() {
                self.advance();
            }
        }

        let span = self.span_from(start_line, start_col);
        let text = &self.source[self.byte_offset_for(start_line, start_col)..self.pos];
        let text = std::str::from_utf8(text).unwrap_or("0");
        let value: f64 = text.parse().unwrap_or(0.0);

        Token::new(TokenKind::NumberLit(value), span)
    }

    // ─────────────────────────────────────────────────────────────
    // Identifiers & keywords
    // ─────────────────────────────────────────────────────────────

    fn scan_identifier(&mut self, start_line: u32, start_col: u32) -> Token {
        // First character was already consumed (letter or `_`)
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                self.advance();
            } else {
                break;
            }
        }

        let span = self.span_from(start_line, start_col);
        let text = &self.source[self.byte_offset_for(start_line, start_col)..self.pos];
        let text = std::str::from_utf8(text).unwrap_or("");

        let kind = TokenKind::from_keyword(text)
            .unwrap_or_else(|| TokenKind::Identifier(text.to_string()));

        Token::new(kind, span)
    }

    // ─────────────────────────────────────────────────────────────
    // String literals & interpolation
    // ─────────────────────────────────────────────────────────────

    /// Scan a string literal starting after the opening `"`.
    /// Handles three cases:
    /// 1. Plain string (no interpolation) → `StringLiteral`
    /// 2. String with interpolation → `StringStart`, then mode switch
    fn scan_string(&mut self, start_line: u32, start_col: u32) -> Token {
        let mut buf = String::new();

        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    // Unterminated string
                    let span = self.span_from(start_line, start_col);
                    self.emit_error(
                        ErrorCode::UNCLOSED_BRACE,
                        "Unterminated string literal",
                        span,
                    );
                    return Token::new(
                        TokenKind::StringLiteral(buf),
                        self.span_from(start_line, start_col),
                    );
                }
                Some(b'"') => {
                    // End of string
                    self.advance();
                    return Token::new(
                        TokenKind::StringLiteral(buf),
                        self.span_from(start_line, start_col),
                    );
                }
                Some(b'\\') => {
                    if let Some(escaped) = self.scan_escape_sequence() {
                        buf.push(escaped);
                    }
                }
                Some(b'$') if self.peek_at(1) == Some(b'{') => {
                    // Start of interpolation
                    self.advance(); // consume '$'
                    self.advance(); // consume '{'
                    let interp_span = self.span_from(self.line, self.col.saturating_sub(2));
                    self.push_mode(Mode::Interpolation { brace_depth: 0 });
                    // Queue InterpolationStart so it appears after StringStart
                    self.pending
                        .push(Token::new(TokenKind::InterpolationStart, interp_span));
                    return Token::new(
                        TokenKind::StringStart(buf),
                        self.span_from(start_line, start_col),
                    );
                }
                Some(ch) => {
                    self.advance();
                    buf.push(ch as char);
                }
            }
        }
    }

    /// Continue scanning string content after an interpolation ends.
    /// Called when we're in `Mode::String`.
    fn scan_string_continuation(&mut self) -> Token {
        let start_line = self.line;
        let start_col = self.col;
        let mut buf = String::new();

        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    let span = self.span_from(start_line, start_col);
                    self.emit_error(
                        ErrorCode::UNCLOSED_BRACE,
                        "Unterminated string literal",
                        span,
                    );
                    self.pop_mode();
                    return Token::new(
                        TokenKind::StringEnd(buf),
                        self.span_from(start_line, start_col),
                    );
                }
                Some(b'"') => {
                    self.advance();
                    self.pop_mode();
                    return Token::new(
                        TokenKind::StringEnd(buf),
                        self.span_from(start_line, start_col),
                    );
                }
                Some(b'\\') => {
                    if let Some(escaped) = self.scan_escape_sequence() {
                        buf.push(escaped);
                    }
                }
                Some(b'$') if self.peek_at(1) == Some(b'{') => {
                    // Another interpolation
                    self.advance(); // consume '$'
                    self.advance(); // consume '{'
                    let interp_span = self.span_from(self.line, self.col.saturating_sub(2));
                    // Replace current String mode with Interpolation
                    self.pop_mode();
                    self.push_mode(Mode::Interpolation { brace_depth: 0 });
                    // Queue InterpolationStart so it appears after StringPart
                    self.pending
                        .push(Token::new(TokenKind::InterpolationStart, interp_span));
                    return Token::new(
                        TokenKind::StringPart(buf),
                        self.span_from(start_line, start_col),
                    );
                }
                Some(ch) => {
                    self.advance();
                    buf.push(ch as char);
                }
            }
        }
    }

    /// Scan an escape sequence after consuming the `\`.
    /// Returns the unescaped character, or `None` if invalid (error emitted).
    fn scan_escape_sequence(&mut self) -> Option<char> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // consume the '\'

        match self.advance() {
            Some(b'"') => Some('"'),
            Some(b'\\') => Some('\\'),
            Some(b'n') => Some('\n'),
            Some(b't') => Some('\t'),
            Some(b'r') => Some('\r'),
            Some(b'$') => Some('$'),
            Some(ch) => {
                let span = self.span_from(start_line, start_col);
                self.emit_error(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("Invalid escape sequence '\\{}'", ch as char),
                    span,
                );
                Some(ch as char) // error recovery: emit the char as-is
            }
            None => {
                let span = self.span_from(start_line, start_col);
                self.emit_error(
                    ErrorCode::UNCLOSED_BRACE,
                    "Unexpected end of file in escape sequence",
                    span,
                );
                None
            }
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Byte-offset helper
    // ─────────────────────────────────────────────────────────────

    /// Convert a 1-based line/col position to a byte offset.
    /// This is used to extract the lexeme text from the source.
    fn byte_offset_for(&self, line: u32, col: u32) -> usize {
        let mut offset = 0usize;
        let mut current_line = 1u32;

        // Skip to the target line
        while current_line < line {
            match self.source.get(offset) {
                Some(b'\n') => {
                    current_line += 1;
                    offset += 1;
                }
                Some(_) => {
                    offset += 1;
                }
                None => return offset,
            }
        }

        // Add column offset (1-based → 0-based)
        offset + (col as usize).saturating_sub(1)
    }
}

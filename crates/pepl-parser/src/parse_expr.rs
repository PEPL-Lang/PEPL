//! Expression parsing with full operator precedence.
//!
//! Precedence (lowest → highest):
//! 9. `or`
//! 8. `and`
//! 7. `??` (nil-coalescing)
//! 6. `==`, `!=`, `<`, `>`, `<=`, `>=` (no chaining)
//! 5. `+`, `-`
//! 4. `*`, `/`, `%`
//! 3. unary `-`, `not`
//! 2. `?` (Result unwrap — postfix)
//! 1. `.` (field access / method call), `()` (call)

use pepl_lexer::token::TokenKind;
use pepl_types::ast::*;
use pepl_types::{ErrorCode, Span};

use crate::parser::Parser;

impl<'src> Parser<'src> {
    // ══════════════════════════════════════════════════════════════════════════
    // Entry Point
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse an expression.
    pub(crate) fn parse_expression(&mut self) -> Option<Expr> {
        self.expr_depth += 1;
        if self.expr_depth > 16 {
            self.error_at_current(
                ErrorCode::STRUCTURAL_LIMIT_EXCEEDED,
                format!(
                    "maximum expression nesting depth is 16, got {}",
                    self.expr_depth
                ),
            );
            self.expr_depth -= 1;
            return None;
        }
        let result = self.parse_or();
        self.expr_depth -= 1;
        result
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Precedence Chain
    // ══════════════════════════════════════════════════════════════════════════

    /// `OrExpr = AndExpr { "or" AndExpr }`
    fn parse_or(&mut self) -> Option<Expr> {
        let mut left = self.parse_and()?;
        while self.eat(&TokenKind::Or) {
            let right = self.parse_and()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: BinOp::Or,
                    right: Box::new(right),
                },
                span,
            );
        }
        Some(left)
    }

    /// `AndExpr = NilCoalesceExpr { "and" NilCoalesceExpr }`
    fn parse_and(&mut self) -> Option<Expr> {
        let mut left = self.parse_nil_coalesce()?;
        while self.eat(&TokenKind::And) {
            let right = self.parse_nil_coalesce()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op: BinOp::And,
                    right: Box::new(right),
                },
                span,
            );
        }
        Some(left)
    }

    /// `NilCoalesceExpr = CompExpr { "??" CompExpr }`
    fn parse_nil_coalesce(&mut self) -> Option<Expr> {
        let mut left = self.parse_comparison()?;
        while self.eat(&TokenKind::QuestionQuestion) {
            let right = self.parse_comparison()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::NilCoalesce {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            );
        }
        Some(left)
    }

    /// `CompExpr = AddExpr [ CompOp AddExpr ]`
    ///
    /// Comparison operators do NOT chain: `a < b < c` is a parse error.
    fn parse_comparison(&mut self) -> Option<Expr> {
        let mut left = self.parse_add()?;
        if let Some(op) = self.match_comparison_op() {
            self.advance(); // consume operator
            let right = self.parse_add()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            );
            // Reject chaining
            if self.match_comparison_op().is_some() {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    "comparison operators cannot be chained; use 'and' to combine: a < b and b < c",
                );
            }
        }
        Some(left)
    }

    /// Check if current token is a comparison operator, return corresponding BinOp.
    fn match_comparison_op(&self) -> Option<BinOp> {
        match self.peek_kind() {
            TokenKind::EqEq => Some(BinOp::Eq),
            TokenKind::BangEq => Some(BinOp::NotEq),
            TokenKind::Less => Some(BinOp::Less),
            TokenKind::Greater => Some(BinOp::Greater),
            TokenKind::LessEq => Some(BinOp::LessEq),
            TokenKind::GreaterEq => Some(BinOp::GreaterEq),
            _ => None,
        }
    }

    /// `AddExpr = MulExpr { ("+" | "-") MulExpr }`
    fn parse_add(&mut self) -> Option<Expr> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            );
        }
        Some(left)
    }

    /// `MulExpr = UnaryExpr { ("*" | "/" | "%") UnaryExpr }`
    fn parse_mul(&mut self) -> Option<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let span = left.span.merge(right.span);
            left = Expr::new(
                ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            );
        }
        Some(left)
    }

    /// `UnaryExpr = [ "not" | "-" ] PostfixExpr`
    fn parse_unary(&mut self) -> Option<Expr> {
        let start = self.current_span();
        let op = match self.peek_kind() {
            TokenKind::Not => {
                self.advance();
                Some(UnaryOp::Not)
            }
            TokenKind::Minus => {
                self.advance();
                Some(UnaryOp::Neg)
            }
            _ => None,
        };
        let operand = self.parse_postfix()?;
        if let Some(op) = op {
            let span = start.merge(operand.span);
            Some(Expr::new(
                ExprKind::Unary {
                    op,
                    operand: Box::new(operand),
                },
                span,
            ))
        } else {
            Some(operand)
        }
    }

    /// `PostfixExpr = PrimaryExpr { "?" | "." Identifier [ "(" ArgList ")" ] }`
    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek_kind() {
                TokenKind::Question => {
                    self.advance();
                    let span = expr.span.merge(self.previous_span());
                    expr = Expr::new(ExprKind::ResultUnwrap(Box::new(expr)), span);
                }
                TokenKind::Dot => {
                    self.advance(); // eat `.`
                    let field = self.expect_member_name()?;
                    // Check for method call: `.method(args)`
                    if self.check_exact(&TokenKind::LParen) {
                        self.advance(); // eat `(`
                        let args = self.parse_arg_list()?;
                        self.expect(&TokenKind::RParen)?;
                        let span = expr.span.merge(self.previous_span());
                        expr = Expr::new(
                            ExprKind::MethodCall {
                                object: Box::new(expr),
                                method: field,
                                args,
                            },
                            span,
                        );
                    } else {
                        let span = expr.span.merge(field.span);
                        expr = Expr::new(
                            ExprKind::FieldAccess {
                                object: Box::new(expr),
                                field,
                            },
                            span,
                        );
                    }
                }
                _ => break,
            }
        }
        Some(expr)
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Primary Expressions
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse a primary expression.
    fn parse_primary(&mut self) -> Option<Expr> {
        let start = self.current_span();
        match self.peek_kind().clone() {
            // ── Literals ────────────────────────────────────────────────
            TokenKind::NumberLit(n) => {
                self.advance();
                Some(Expr::new(ExprKind::NumberLit(n), start))
            }
            TokenKind::StringLiteral(s) => {
                self.advance();
                Some(Expr::new(ExprKind::StringLit(s), start))
            }
            TokenKind::StringStart(s) => {
                self.advance();
                self.parse_string_interpolation(s, start)
            }
            TokenKind::True => {
                self.advance();
                Some(Expr::new(ExprKind::BoolLit(true), start))
            }
            TokenKind::False => {
                self.advance();
                Some(Expr::new(ExprKind::BoolLit(false), start))
            }
            TokenKind::Nil => {
                self.advance();
                Some(Expr::new(ExprKind::NilLit, start))
            }

            // ── Collections ─────────────────────────────────────────────
            TokenKind::LBracket => self.parse_list_literal(),
            TokenKind::LBrace => self.parse_record_literal(),

            // ── Grouping ────────────────────────────────────────────────
            TokenKind::LParen => {
                self.advance(); // eat `(`
                let inner = self.parse_expression()?;
                self.expect(&TokenKind::RParen)?;
                let span = start.merge(self.previous_span());
                Some(Expr::new(ExprKind::Paren(Box::new(inner)), span))
            }

            // ── Control Flow ────────────────────────────────────────────
            TokenKind::If => self.parse_if_expr_node().map(|ie| {
                let span = ie.span;
                Expr::new(ExprKind::If(Box::new(ie)), span)
            }),
            TokenKind::For => self.parse_for_expr_node().map(|fe| {
                let span = fe.span;
                Expr::new(ExprKind::For(Box::new(fe)), span)
            }),
            TokenKind::Match => self.parse_match_expr_node().map(|me| {
                let span = me.span;
                Expr::new(ExprKind::Match(Box::new(me)), span)
            }),

            // ── Lambda ──────────────────────────────────────────────────
            TokenKind::Fn => self.parse_lambda(),

            // ── Qualified calls: module.function(args) ──────────────────
            // Module/capability keywords can only appear as qualified-call prefixes
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
            | TokenKind::KwString
            | TokenKind::KwList
            | TokenKind::KwColor => self.parse_qualified_call(),

            // ── Identifier or unqualified function call ─────────────────
            TokenKind::Identifier(_) => {
                // Check for function call: ident(args)
                if *self.look_ahead(1) == TokenKind::LParen {
                    self.parse_unqualified_call()
                } else {
                    let ident = self.expect_identifier()?;
                    Some(Expr::new(ExprKind::Identifier(ident.name), ident.span))
                }
            }

            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected expression, got '{}'", self.peek_kind()),
                );
                None
            }
        }
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Call Parsing
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `module.function(args...)` — a qualified (stdlib/capability) call.
    fn parse_qualified_call(&mut self) -> Option<Expr> {
        let start = self.current_span();
        let module = self.expect_ident_or_module_name()?;
        self.expect(&TokenKind::Dot)?;
        let function = self.expect_member_name()?;
        self.expect(&TokenKind::LParen)?;
        let args = self.parse_arg_list()?;
        self.expect(&TokenKind::RParen)?;
        let span = start.merge(self.previous_span());
        Some(Expr::new(
            ExprKind::QualifiedCall {
                module,
                function,
                args,
            },
            span,
        ))
    }

    /// Parse `name(args...)` — an unqualified function call.
    fn parse_unqualified_call(&mut self) -> Option<Expr> {
        let start = self.current_span();
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::LParen)?;
        let args = self.parse_arg_list()?;
        self.expect(&TokenKind::RParen)?;
        let span = start.merge(self.previous_span());
        Some(Expr::new(ExprKind::Call { name, args }, span))
    }

    /// Parse a comma-separated argument list (inside parens).
    fn parse_arg_list(&mut self) -> Option<Vec<Expr>> {
        let mut args = Vec::new();
        self.skip_newlines();
        if self.check_exact(&TokenKind::RParen) {
            return Some(args);
        }
        loop {
            self.skip_newlines();
            args.push(self.parse_expression()?);
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
            // Allow trailing comma before `)`
            if self.check_exact(&TokenKind::RParen) {
                break;
            }
        }
        Some(args)
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Control Flow Expression Nodes
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `if cond { ... } [else { ... } | else if ...]`
    pub(crate) fn parse_if_expr_node(&mut self) -> Option<IfExpr> {
        let start = self.current_span();
        self.advance(); // eat `if`
        let condition = self.parse_expression()?;
        let then_block = self.parse_block()?;
        let else_branch = if self.eat(&TokenKind::Else) {
            if self.check_exact(&TokenKind::If) {
                let else_if = self.parse_if_expr_node()?;
                Some(ElseBranch::ElseIf(Box::new(else_if)))
            } else {
                let block = self.parse_block()?;
                Some(ElseBranch::Block(block))
            }
        } else {
            None
        };
        let span = start.merge(self.previous_span());
        Some(IfExpr {
            condition,
            then_block,
            else_branch,
            span,
        })
    }

    /// Parse `for item [, index] in expr { ... }`
    pub(crate) fn parse_for_expr_node(&mut self) -> Option<ForExpr> {
        let start = self.current_span();
        self.advance(); // eat `for`

        // For-loop nesting depth limit
        self.for_depth += 1;
        if self.for_depth > 3 {
            self.error_at(
                ErrorCode::STRUCTURAL_LIMIT_EXCEEDED,
                format!(
                    "maximum for-loop nesting depth is 3, got {}",
                    self.for_depth
                ),
                start,
            );
        }

        let item = self.expect_identifier()?;
        let index = if self.eat(&TokenKind::Comma) {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect(&TokenKind::In)?;
        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;
        self.for_depth -= 1;

        let span = start.merge(self.previous_span());
        Some(ForExpr {
            item,
            index,
            iterable,
            body,
            span,
        })
    }

    /// Parse `match expr { arms... }`
    pub(crate) fn parse_match_expr_node(&mut self) -> Option<MatchExpr> {
        let start = self.current_span();
        self.advance(); // eat `match`
        let subject = self.parse_expression()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut arms = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(arm) = self.parse_match_arm() {
                arms.push(arm);
            } else {
                self.synchronize();
            }
            self.eat_comma();
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(MatchExpr {
            subject,
            arms,
            span,
        })
    }

    /// Parse `Pattern -> expr | { block }`
    fn parse_match_arm(&mut self) -> Option<MatchArm> {
        let start = self.current_span();
        let pattern = self.parse_pattern()?;
        self.expect(&TokenKind::Arrow)?;
        let body = if self.check_exact(&TokenKind::LBrace) {
            MatchArmBody::Block(self.parse_block()?)
        } else {
            MatchArmBody::Expr(self.parse_expression()?)
        };
        let span = start.merge(self.previous_span());
        Some(MatchArm {
            pattern,
            body,
            span,
        })
    }

    /// Parse a match pattern: `Variant(bindings)` or `_`
    fn parse_pattern(&mut self) -> Option<Pattern> {
        self.skip_newlines();
        if self.eat(&TokenKind::Underscore) {
            return Some(Pattern::Wildcard(self.previous_span()));
        }
        // Variant pattern: Name or Name(a, b, c)
        let name = self.expect_identifier()?;
        let bindings = if self.eat(&TokenKind::LParen) {
            let mut bindings = Vec::new();
            if !self.check_exact(&TokenKind::RParen) {
                loop {
                    bindings.push(self.expect_identifier()?);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(&TokenKind::RParen)?;
            bindings
        } else {
            Vec::new()
        };
        Some(Pattern::Variant { name, bindings })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Lambda
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `fn(params) { body }`
    fn parse_lambda(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // eat `fn`
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;

        // Structural limit: max 8 params per function/action
        if params.len() > 8 {
            self.error_at_current(
                ErrorCode::STRUCTURAL_LIMIT_EXCEEDED,
                format!("maximum 8 parameters per function, got {}", params.len()),
            );
        }

        self.expect(&TokenKind::RParen)?;

        // Lambda nesting depth limit
        self.lambda_depth += 1;
        if self.lambda_depth > 3 {
            self.error_at_current(
                ErrorCode::STRUCTURAL_LIMIT_EXCEEDED,
                format!(
                    "maximum lambda nesting depth is 3, got {}",
                    self.lambda_depth
                ),
            );
        }

        // Expression-body lambdas are rejected with E602
        if !self.check_exact(&TokenKind::LBrace) {
            self.error_at_current(
                ErrorCode::EXPRESSION_BODY_LAMBDA,
                "lambdas require block body: fn(x) { x + 1 }",
            );
            self.lambda_depth -= 1;
            return None;
        }

        let body = self.parse_block()?;
        self.lambda_depth -= 1;

        let span = start.merge(self.previous_span());
        Some(Expr::new(
            ExprKind::Lambda(Box::new(LambdaExpr { params, body, span })),
            span,
        ))
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Literals
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `[expr, ...]`
    fn parse_list_literal(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // eat `[`
        self.skip_newlines();
        let mut elements = Vec::new();
        if !self.check_exact(&TokenKind::RBracket) {
            loop {
                self.skip_newlines();
                elements.push(self.parse_expression()?);
                self.skip_newlines();
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
                // Trailing comma
                if self.check_exact(&TokenKind::RBracket) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RBracket)?;
        let span = start.merge(self.previous_span());
        Some(Expr::new(ExprKind::ListLit(elements), span))
    }

    /// Parse `{ field: expr, ...spread, ... }` or `{}`
    fn parse_record_literal(&mut self) -> Option<Expr> {
        let start = self.current_span();
        self.advance(); // eat `{`

        // Record nesting depth limit
        self.record_depth += 1;
        if self.record_depth > 4 {
            self.error_at(
                ErrorCode::STRUCTURAL_LIMIT_EXCEEDED,
                format!(
                    "maximum record nesting depth is 4, got {}",
                    self.record_depth
                ),
                start,
            );
        }

        self.skip_newlines();
        let mut entries = Vec::new();
        if !self.check_exact(&TokenKind::RBrace) {
            loop {
                self.skip_newlines();
                if self.check_exact(&TokenKind::DotDotDot) {
                    // Spread: `...expr`
                    self.advance();
                    let spread_expr = self.parse_expression()?;
                    entries.push(RecordEntry::Spread(spread_expr));
                } else {
                    // Field: `name: expr` — keywords allowed as field names
                    let name = self.expect_field_name()?;
                    self.expect(&TokenKind::Colon)?;
                    let value = self.parse_expression()?;
                    entries.push(RecordEntry::Field { name, value });
                }
                self.skip_newlines();
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
                // Trailing comma
                if self.check_exact(&TokenKind::RBrace) {
                    break;
                }
            }
        }
        self.expect(&TokenKind::RBrace)?;
        self.record_depth -= 1;
        let span = start.merge(self.previous_span());
        Some(Expr::new(ExprKind::RecordLit(entries), span))
    }

    /// Parse an interpolated string: `"text ${expr} more ${expr} end"`
    ///
    /// Called after the `StringStart` token has been consumed.
    fn parse_string_interpolation(&mut self, start_text: String, start_span: Span) -> Option<Expr> {
        let mut parts = Vec::new();
        if !start_text.is_empty() {
            parts.push(StringPart::Literal(start_text));
        }
        loop {
            // Expect InterpolationStart — the `${`
            self.expect(&TokenKind::InterpolationStart)?;
            // Parse the interpolated expression
            let expr = self.parse_expression()?;
            parts.push(StringPart::Expr(expr));
            // Expect InterpolationEnd — the `}`
            self.expect(&TokenKind::InterpolationEnd)?;
            // What follows: StringPart (more interpolations) or StringEnd
            match self.peek_kind().clone() {
                TokenKind::StringPart(s) => {
                    self.advance();
                    if !s.is_empty() {
                        parts.push(StringPart::Literal(s));
                    }
                    // Continue — more interpolations follow
                }
                TokenKind::StringEnd(s) => {
                    self.advance();
                    if !s.is_empty() {
                        parts.push(StringPart::Literal(s));
                    }
                    break;
                }
                _ => {
                    self.error_at_current(
                        ErrorCode::UNEXPECTED_TOKEN,
                        "unterminated string interpolation",
                    );
                    return None;
                }
            }
        }
        let span = start_span.merge(self.previous_span());
        Some(Expr::new(ExprKind::StringInterpolation(parts), span))
    }
}

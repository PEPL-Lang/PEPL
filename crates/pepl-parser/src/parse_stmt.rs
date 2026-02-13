//! Statement parsing.

use crate::parser::Parser;
use pepl_lexer::token::TokenKind;
use pepl_types::ast::*;

impl<'src> Parser<'src> {
    /// Parse a block of statements: `{ stmts... }`
    pub(crate) fn parse_block(&mut self) -> Option<Block> {
        let start = self.current_span();
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut stmts = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(stmt) = self.parse_statement() {
                stmts.push(stmt);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(Block { stmts, span })
    }

    /// Parse a single statement.
    pub(crate) fn parse_statement(&mut self) -> Option<Stmt> {
        self.skip_newlines();
        if self.at_end() || self.check_exact(&TokenKind::RBrace) {
            return None;
        }
        let stmt = match self.peek_kind() {
            TokenKind::Set => self.parse_set_stmt(),
            TokenKind::Let => self.parse_let_binding().map(Stmt::Let),
            TokenKind::If => self.parse_if_expr_node().map(Stmt::If),
            TokenKind::For => self.parse_for_expr_node().map(Stmt::For),
            TokenKind::Match => self.parse_match_expr_node().map(Stmt::Match),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::Assert => self.parse_assert_stmt(),
            _ => {
                // Expression statement
                let expr = self.parse_expression()?;
                let span = expr.span;
                self.expect_newline_or_eof();
                Some(Stmt::Expr(ExprStmt { expr, span }))
            }
        };
        stmt
    }

    /// `set target.path = value`
    fn parse_set_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // eat `set`
        let mut target = Vec::new();
        let first = self.expect_identifier()?;
        target.push(first);
        while self.eat(&TokenKind::Dot) {
            let field = self.expect_identifier()?;
            target.push(field);
        }
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expression()?;
        let span = start.merge(self.previous_span());
        self.expect_newline_or_eof();
        Some(Stmt::Set(SetStmt {
            target,
            value,
            span,
        }))
    }

    /// `let name: Type = expr` or `let _ = expr`
    pub(crate) fn parse_let_binding(&mut self) -> Option<LetBinding> {
        let start = self.current_span();
        self.advance(); // eat `let`
        let (name, type_ann) = if self.eat(&TokenKind::Underscore) {
            // Discard binding: `let _ = expr`
            (None, None)
        } else {
            let ident = self.expect_identifier()?;
            let type_ann = if self.eat(&TokenKind::Colon) {
                Some(self.parse_type_annotation()?)
            } else {
                None
            };
            (Some(ident), type_ann)
        };
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expression()?;
        let span = start.merge(self.previous_span());
        self.expect_newline_or_eof();
        Some(LetBinding {
            name,
            type_ann,
            value,
            span,
        })
    }

    /// `return`
    fn parse_return_stmt(&mut self) -> Option<Stmt> {
        let span = self.advance().span; // eat `return`
        self.expect_newline_or_eof();
        Some(Stmt::Return(ReturnStmt { span }))
    }

    /// `assert expr [, "message"]`
    fn parse_assert_stmt(&mut self) -> Option<Stmt> {
        let start = self.current_span();
        self.advance(); // eat `assert`
        let condition = self.parse_expression()?;
        let message = if self.eat(&TokenKind::Comma) {
            Some(self.expect_string_literal()?)
        } else {
            None
        };
        let span = start.merge(self.previous_span());
        self.expect_newline_or_eof();
        Some(Stmt::Assert(AssertStmt {
            condition,
            message,
            span,
        }))
    }
}

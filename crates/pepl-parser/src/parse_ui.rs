//! UI block and component parsing.

use pepl_lexer::token::TokenKind;
use pepl_types::ast::*;
use pepl_types::ErrorCode;

use crate::parser::Parser;

impl<'src> Parser<'src> {
    /// Parse a UI block: `{ UIElement... }`
    pub(crate) fn parse_ui_block(&mut self) -> Option<UIBlock> {
        let start = self.current_span();
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut elements = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(elem) = self.parse_ui_element() {
                elements.push(elem);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(UIBlock { elements, span })
    }

    /// Parse a single UI element.
    fn parse_ui_element(&mut self) -> Option<UIElement> {
        self.skip_newlines();
        match self.peek_kind() {
            TokenKind::Let => {
                let binding = self.parse_let_binding()?;
                Some(UIElement::Let(binding))
            }
            TokenKind::If => {
                let ui_if = self.parse_ui_if()?;
                Some(UIElement::If(ui_if))
            }
            TokenKind::For => {
                let ui_for = self.parse_ui_for()?;
                Some(UIElement::For(ui_for))
            }
            // Component: starts with an upper-case identifier
            TokenKind::Identifier(ref name)
                if name.starts_with(|c: char| c.is_ascii_uppercase()) =>
            {
                let comp = self.parse_component_expr()?;
                Some(UIElement::Component(comp))
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!(
                        "expected component, 'if', 'for', or 'let' in view block, got '{}'",
                        self.peek_kind()
                    ),
                );
                None
            }
        }
    }

    /// Parse a component expression: `Name { props } [{ children }]`
    fn parse_component_expr(&mut self) -> Option<ComponentExpr> {
        let start = self.current_span();
        let name = self.expect_upper_identifier()?;

        // Props block
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut props = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if let Some(prop) = self.parse_prop_assign() {
                props.push(prop);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;

        // Optional children block
        // Children block is another `{ ... }` immediately after props block.
        // We need to be careful — only consume LBrace if it's on the same line
        // or immediately following (no newline between).
        let children = if self.check_exact(&TokenKind::LBrace) {
            Some(self.parse_ui_block()?)
        } else {
            None
        };

        let span = start.merge(self.previous_span());
        Some(ComponentExpr {
            name,
            props,
            children,
            span,
        })
    }

    /// Parse a prop assignment: `name: expr [,]`
    fn parse_prop_assign(&mut self) -> Option<PropAssign> {
        let start = self.current_span();
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Colon)?;
        let value = self.parse_expression()?;
        let span = start.merge(self.previous_span());
        self.eat_comma();
        self.skip_newlines();
        Some(PropAssign { name, value, span })
    }

    /// Parse `if cond { UIElements } [else { UIElements }]` in UI context.
    fn parse_ui_if(&mut self) -> Option<UIIf> {
        let start = self.current_span();
        self.advance(); // eat `if`
        let condition = self.parse_expression()?;
        let then_block = self.parse_ui_block()?;
        let else_block = if self.eat(&TokenKind::Else) {
            if self.check_exact(&TokenKind::If) {
                let else_if = self.parse_ui_if()?;
                Some(UIElse::ElseIf(Box::new(else_if)))
            } else {
                let block = self.parse_ui_block()?;
                Some(UIElse::Block(block))
            }
        } else {
            None
        };
        let span = start.merge(self.previous_span());
        Some(UIIf {
            condition,
            then_block,
            else_block,
            span,
        })
    }

    /// Parse `for item [, index] in expr { UIElements }` in UI context.
    fn parse_ui_for(&mut self) -> Option<UIFor> {
        let start = self.current_span();
        self.advance(); // eat `for`
        let item = self.expect_identifier()?;
        let index = if self.eat(&TokenKind::Comma) {
            Some(self.expect_identifier()?)
        } else {
            None
        };
        self.expect(&TokenKind::In)?;
        let iterable = self.parse_expression()?;
        let body = self.parse_ui_block()?;
        let span = start.merge(self.previous_span());
        Some(UIFor {
            item,
            index,
            iterable,
            body,
            span,
        })
    }
}

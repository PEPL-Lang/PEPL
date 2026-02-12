//! Type annotation parsing.

use pepl_lexer::token::TokenKind;
use pepl_types::ast::*;
use pepl_types::ErrorCode;

use crate::parser::Parser;

impl<'src> Parser<'src> {
    /// Parse a type annotation.
    ///
    /// ```ebnf
    /// Type = "number" | "string" | "bool" | "nil" | "any" | "color"
    ///      | "Surface" | "InputEvent"
    ///      | "list" "<" Type ">"
    ///      | "{" { RecordTypeField } "}"
    ///      | "Result" "<" Type "," Type ">"
    ///      | "(" [ TypeList ] ")" "->" Type
    ///      | Identifier ;
    /// ```
    pub(crate) fn parse_type_annotation(&mut self) -> Option<TypeAnnotation> {
        let start = self.current_span();
        let kind = match self.peek_kind().clone() {
            TokenKind::KwNumber => {
                self.advance();
                TypeKind::Number
            }
            TokenKind::KwString => {
                self.advance();
                TypeKind::String
            }
            TokenKind::KwBool => {
                self.advance();
                TypeKind::Bool
            }
            TokenKind::Nil => {
                self.advance();
                TypeKind::Nil
            }
            TokenKind::KwColor => {
                self.advance();
                TypeKind::Color
            }
            TokenKind::KwSurface => {
                self.advance();
                TypeKind::Surface
            }
            TokenKind::KwInputEvent => {
                self.advance();
                TypeKind::InputEvent
            }
            TokenKind::Identifier(ref name) if name == "any" => {
                self.advance();
                TypeKind::Any
            }
            TokenKind::KwList => {
                self.advance();
                self.expect(&TokenKind::Less)?;
                let inner = self.parse_type_annotation()?;
                self.expect(&TokenKind::Greater)?;
                TypeKind::List(Box::new(inner))
            }
            TokenKind::KwResult => {
                self.advance();
                self.expect(&TokenKind::Less)?;
                let ok_type = self.parse_type_annotation()?;
                self.expect(&TokenKind::Comma)?;
                let err_type = self.parse_type_annotation()?;
                self.expect(&TokenKind::Greater)?;
                TypeKind::Result(Box::new(ok_type), Box::new(err_type))
            }
            TokenKind::LBrace => {
                // Anonymous record type: { name: string, age?: number }
                self.advance();
                let mut fields = Vec::new();
                self.skip_newlines();
                while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
                    if let Some(field) = self.parse_record_type_field() {
                        fields.push(field);
                    }
                    self.eat_comma();
                    self.skip_newlines();
                }
                self.expect(&TokenKind::RBrace)?;
                TypeKind::Record(fields)
            }
            TokenKind::LParen => {
                // Function type: (number, string) -> bool
                self.advance();
                let mut params = Vec::new();
                while !self.check_exact(&TokenKind::RParen) && !self.at_end() {
                    let param = self.parse_type_annotation()?;
                    params.push(param);
                    if !self.eat_comma() {
                        break;
                    }
                }
                self.expect(&TokenKind::RParen)?;
                self.expect(&TokenKind::Arrow)?;
                let ret = self.parse_type_annotation()?;
                TypeKind::Function {
                    params,
                    ret: Box::new(ret),
                }
            }
            TokenKind::Identifier(name) => {
                self.advance();
                TypeKind::Named(name)
            }
            _ => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    format!("expected type, got '{}'", self.peek_kind()),
                );
                return None;
            }
        };
        let span = start.merge(self.previous_span());
        Some(TypeAnnotation::new(kind, span))
    }

    /// Parse a record type field: `name?: Type`
    fn parse_record_type_field(&mut self) -> Option<RecordTypeField> {
        let start = self.current_span();
        let name = self.expect_identifier()?;
        let optional = self.eat(&TokenKind::Question);
        self.expect(&TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        let span = start.merge(self.previous_span());
        Some(RecordTypeField {
            name,
            optional,
            type_ann,
            span,
        })
    }
}

//! PEPL parser: converts a token stream into an AST.

mod parser;
mod parse_decl;
mod parse_expr;
mod parse_stmt;
mod parse_type;
mod parse_ui;

pub use parser::{ParseResult, Parser};

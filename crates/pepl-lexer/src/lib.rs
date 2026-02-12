//! PEPL lexer: converts source text into a token stream.

pub mod lexer;
pub mod token;

pub use lexer::{LexResult, Lexer};
pub use token::{Token, TokenKind, ALL_KEYWORDS};

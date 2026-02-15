# pepl-lexer

PEPL lexer: transforms source text into a token stream.

First stage of the PEPL compilation pipeline. Handles keywords, identifiers, literals, operators, and whitespace with full source-location tracking.

## Key Exports

```rust
use pepl_lexer::{Lexer, LexResult, Token, TokenKind, ALL_KEYWORDS};

let tokens: LexResult = Lexer::new(source, "example.pepl").lex();
```

## Token Kinds

Numbers, strings, booleans, `nil`, identifiers, all PEPL keywords (`space`, `state`, `action`, `view`, `match`, `if`, `for`, `let`, `set`, etc.), operators, delimiters, and comments.

## Install

```bash
cargo add pepl-lexer
```

## License

MIT — see [LICENSE](../../LICENSE)

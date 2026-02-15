# pepl-parser

PEPL parser: transforms a token stream into a typed AST.

Second stage of the PEPL compilation pipeline. Implements a recursive-descent parser for the full PEPL grammar — spaces, state declarations, action blocks, view blocks, expressions, UI components, and sum types.

## Key Exports

```rust
use pepl_parser::{Parser, ParseResult};
use pepl_lexer::Lexer;

let tokens = Lexer::new(source, "example.pepl").lex();
let ast: ParseResult = Parser::new(tokens.tokens, "example.pepl").parse();
```

## Grammar Coverage

- Space declarations with state, action, view
- Expressions: arithmetic, comparison, logical, nil-coalescing (`??`), match, if/else, string interpolation
- Statements: let bindings, set mutations, for loops, return, assert
- Type annotations: primitives, List\<T\>, Record, Result\<T,E\>, sum types, nullable (`T | nil`)
- UI components: Column, Row, Scroll, Text, ProgressBar, Button, TextInput, ScrollList, Modal, Toast

## Install

```bash
cargo add pepl-parser
```

## License

MIT — see [LICENSE](../../LICENSE)

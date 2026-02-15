# pepl-parser

PEPL parser: transforms a token stream into a typed AST.

Second stage of the PEPL compilation pipeline. Implements a recursive-descent parser for the full PEPL grammar — spaces, state declarations, actions, render blocks, pipes, expressions, UI components, and sum types.

## Key Exports

```rust
use pepl_parser::{Parser, ParseResult};
use pepl_lexer::Lexer;

let tokens = Lexer::new(source, "example.pepl").lex();
let ast: ParseResult = Parser::new(tokens.tokens, "example.pepl").parse();
```

## Grammar Coverage

- Space declarations with state, actions, render, pipe
- Expressions: arithmetic, comparison, logical, pipe chains, match, if/else
- Statements: let bindings, assignments, for loops, return, emit
- Type annotations: primitives, List, Record, Option, sum types
- UI components: Box, Text, Button, Input, Stack, List, Conditional, Each, Fragment

## Install

```bash
cargo add pepl-parser
```

## License

MIT — see [LICENSE](../../LICENSE)

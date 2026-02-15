# pepl-eval

PEPL tree-walking evaluator: a reference implementation for PEPL execution semantics.

Directly interprets the AST without compiling to WASM, producing golden reference output for testing and validation. Used to verify that the WASM codegen produces identical results.

## Key Exports

```rust
use pepl_eval::{Evaluator, Environment, EvalError, EvalResult};
use pepl_eval::{SpaceInstance, ActionResult, SurfaceNode};
use pepl_eval::{run_tests, TestResult, TestRunSummary};

let mut env = Environment::new();
let mut evaluator = Evaluator::new(&mut env);
let result = evaluator.eval_program(&ast)?;
```

## Features

- **Full PEPL semantics** — spaces, actions, views, match, UI components
- **Test runner** — `run_tests` executes PEPL test blocks and reports pass/fail
- **Deterministic** — same inputs always produce same outputs
- **Stdlib integration** — calls into `pepl-stdlib` for all built-in functions

## Install

```bash
cargo add pepl-eval
```

## License

MIT — see [LICENSE](../../LICENSE)

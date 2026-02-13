//! PEPL tree-walking evaluator: reference implementation.
//!
//! Executes PEPL programs directly from the typed AST without WASM compilation.
//! Used for semantic validation and as the golden reference for WASM output.

pub mod env;
pub mod error;
pub mod evaluator;
pub mod space;
pub mod test_runner;

pub use env::Environment;
pub use error::{EvalError, EvalResult};
pub use evaluator::Evaluator;
pub use space::{ActionResult, SpaceInstance, SurfaceNode};
pub use test_runner::{run_tests, MockResponse, TestResult, TestRunSummary};

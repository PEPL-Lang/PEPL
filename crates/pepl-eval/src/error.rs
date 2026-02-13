//! Runtime error types for the PEPL evaluator.

use std::fmt;

/// Evaluation error — runtime traps, assertion failures, invariant violations.
#[derive(Debug, Clone)]
pub enum EvalError {
    /// Division by zero, sqrt of negative, overflow, etc.
    ArithmeticTrap(String),
    /// `core.assert` failure
    AssertionFailed(String),
    /// Invariant check failed after action commit
    InvariantViolation(String),
    /// Nil access: `nil.field`, `nil[i]`, `nil` used as bool, etc.
    NilAccess(String),
    /// `?` on an `Err` variant
    UnwrapError(String),
    /// Unknown variable
    UndefinedVariable(String),
    /// Unknown action
    UndefinedAction(String),
    /// Type mismatch at runtime
    TypeMismatch(String),
    /// Stdlib call error
    StdlibError(String),
    /// Unknown module or function
    UnknownFunction(String),
    /// Gas exhaustion
    GasExhausted,
    /// `return` statement (used internally for control flow)
    Return(pepl_stdlib::Value),
    /// Generic runtime error
    Runtime(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArithmeticTrap(msg) => write!(f, "arithmetic trap: {msg}"),
            Self::AssertionFailed(msg) => write!(f, "assertion failed: {msg}"),
            Self::InvariantViolation(msg) => write!(f, "invariant violation: {msg}"),
            Self::NilAccess(msg) => write!(f, "nil access: {msg}"),
            Self::UnwrapError(msg) => write!(f, "unwrap error: {msg}"),
            Self::UndefinedVariable(name) => write!(f, "undefined variable: {name}"),
            Self::UndefinedAction(name) => write!(f, "undefined action: {name}"),
            Self::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
            Self::StdlibError(msg) => write!(f, "stdlib error: {msg}"),
            Self::UnknownFunction(msg) => write!(f, "unknown function: {msg}"),
            Self::GasExhausted => write!(f, "gas exhausted"),
            Self::Return(_) => write!(f, "return"),
            Self::Runtime(msg) => write!(f, "runtime error: {msg}"),
        }
    }
}

impl std::error::Error for EvalError {}

/// Result alias for evaluator operations.
pub type EvalResult<T> = Result<T, EvalError>;

use crate::Span;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Maximum number of errors reported before fail-fast.
pub const MAX_ERRORS: usize = 20;

/// Error severity.
///
/// Phase 0 only uses `Error`. Warnings are reserved for Phase 1+.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Error category, determined by error code range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ErrorCategory {
    Syntax,
    Type,
    Invariant,
    Capability,
    Scope,
    Structure,
}

/// Numeric error code (E100–E699).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ErrorCode(pub u16);

impl ErrorCode {
    // ── Syntax errors (E100–E199) ──
    pub const UNEXPECTED_TOKEN: Self = Self(100);
    pub const UNCLOSED_BRACE: Self = Self(101);
    pub const INVALID_KEYWORD: Self = Self(102);

    // ── Type errors (E200–E299) ──
    pub const UNKNOWN_TYPE: Self = Self(200);
    pub const TYPE_MISMATCH: Self = Self(201);
    pub const WRONG_ARG_COUNT: Self = Self(202);
    pub const NON_EXHAUSTIVE_MATCH: Self = Self(210);

    // ── Invariant errors (E300–E399) ──
    pub const INVARIANT_UNREACHABLE: Self = Self(300);
    pub const INVARIANT_UNKNOWN_FIELD: Self = Self(301);

    // ── Capability errors (E400–E499) ──
    pub const UNDECLARED_CAPABILITY: Self = Self(400);
    pub const CAPABILITY_UNAVAILABLE: Self = Self(401);
    pub const UNKNOWN_COMPONENT: Self = Self(402);

    // ── Scope errors (E500–E599) ──
    pub const VARIABLE_ALREADY_DECLARED: Self = Self(500);
    pub const STATE_MUTATED_OUTSIDE_ACTION: Self = Self(501);
    pub const RECURSION_NOT_ALLOWED: Self = Self(502);

    // ── Structure errors (E600–E699) ──
    pub const BLOCK_ORDERING_VIOLATED: Self = Self(600);
    pub const DERIVED_FIELD_MODIFIED: Self = Self(601);
    pub const EXPRESSION_BODY_LAMBDA: Self = Self(602);
    pub const BLOCK_COMMENT_USED: Self = Self(603);
    pub const UNDECLARED_CREDENTIAL: Self = Self(604);
    pub const CREDENTIAL_MODIFIED: Self = Self(605);
    pub const EMPTY_STATE_BLOCK: Self = Self(606);
    pub const STRUCTURAL_LIMIT_EXCEEDED: Self = Self(607);

    /// Get the category for this error code.
    pub fn category(self) -> ErrorCategory {
        match self.0 {
            100..=199 => ErrorCategory::Syntax,
            200..=299 => ErrorCategory::Type,
            300..=399 => ErrorCategory::Invariant,
            400..=499 => ErrorCategory::Capability,
            500..=599 => ErrorCategory::Scope,
            600..=699 => ErrorCategory::Structure,
            _ => ErrorCategory::Syntax, // fallback
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{}", self.0)
    }
}

/// A structured PEPL compiler error.
///
/// Matches the error message format defined in the spec (compiler.md).
/// The View Layer renders these — it must not parse free-form strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeplError {
    /// Source file name.
    pub file: String,
    /// Error code (e.g., E201).
    pub code: ErrorCode,
    /// Error severity.
    pub severity: Severity,
    /// Error category (derived from code).
    pub category: ErrorCategory,
    /// Human-readable error message.
    pub message: String,
    /// Source location.
    #[serde(flatten)]
    pub span: Span,
    /// The exact source line for context.
    pub source_line: String,
    /// Optional fix suggestion (for LLM re-prompting).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl PeplError {
    /// Create a new error.
    pub fn new(
        file: impl Into<String>,
        code: ErrorCode,
        message: impl Into<String>,
        span: Span,
        source_line: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            code,
            severity: Severity::Error,
            category: code.category(),
            message: message.into(),
            span,
            source_line: source_line.into(),
            suggestion: None,
        }
    }

    /// Attach a fix suggestion.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for PeplError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} [{}] {}",
            self.span, self.code, self.category, self.message
        )
    }
}

impl std::error::Error for PeplError {}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax => write!(f, "syntax"),
            Self::Type => write!(f, "type"),
            Self::Invariant => write!(f, "invariant"),
            Self::Capability => write!(f, "capability"),
            Self::Scope => write!(f, "scope"),
            Self::Structure => write!(f, "structure"),
        }
    }
}

/// The structured JSON output for compilation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileErrors {
    pub errors: Vec<PeplError>,
    pub warnings: Vec<PeplError>,
    pub total_errors: usize,
    pub total_warnings: usize,
}

impl CompileErrors {
    /// Create an empty result (no errors).
    pub fn empty() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            total_errors: 0,
            total_warnings: 0,
        }
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.total_errors > 0
    }

    /// Add an error, respecting the MAX_ERRORS limit.
    pub fn push_error(&mut self, error: PeplError) {
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(error);
        }
        self.total_errors += 1;
    }

    /// Add a warning.
    pub fn push_warning(&mut self, warning: PeplError) {
        self.warnings.push(warning);
        self.total_warnings += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_category() {
        assert_eq!(
            ErrorCode::UNEXPECTED_TOKEN.category(),
            ErrorCategory::Syntax
        );
        assert_eq!(ErrorCode::TYPE_MISMATCH.category(), ErrorCategory::Type);
        assert_eq!(
            ErrorCode::INVARIANT_UNREACHABLE.category(),
            ErrorCategory::Invariant
        );
        assert_eq!(
            ErrorCode::UNDECLARED_CAPABILITY.category(),
            ErrorCategory::Capability
        );
        assert_eq!(
            ErrorCode::VARIABLE_ALREADY_DECLARED.category(),
            ErrorCategory::Scope
        );
        assert_eq!(
            ErrorCode::BLOCK_ORDERING_VIOLATED.category(),
            ErrorCategory::Structure
        );
    }

    #[test]
    fn test_error_code_display() {
        assert_eq!(format!("{}", ErrorCode::TYPE_MISMATCH), "E201");
        assert_eq!(format!("{}", ErrorCode::UNEXPECTED_TOKEN), "E100");
    }

    #[test]
    fn test_pepl_error_creation() {
        let err = PeplError::new(
            "test.pepl",
            ErrorCode::TYPE_MISMATCH,
            "Type mismatch: expected 'Number', found 'String'",
            Span::new(12, 5, 12, 22),
            "  set state.count = \"hello\"",
        );
        assert_eq!(err.code, ErrorCode::TYPE_MISMATCH);
        assert_eq!(err.severity, Severity::Error);
        assert_eq!(err.category, ErrorCategory::Type);
    }

    #[test]
    fn test_pepl_error_with_suggestion() {
        let err = PeplError::new(
            "test.pepl",
            ErrorCode::TYPE_MISMATCH,
            "Type mismatch",
            Span::new(1, 1, 1, 10),
            "set count = \"hello\"",
        )
        .with_suggestion("Use convert.to_int(value)");
        assert_eq!(err.suggestion.as_deref(), Some("Use convert.to_int(value)"));
    }

    #[test]
    fn test_pepl_error_json_serialization() {
        let err = PeplError::new(
            "WaterTracker.pepl",
            ErrorCode::TYPE_MISMATCH,
            "Type mismatch: expected 'Number', found 'String'",
            Span::new(12, 5, 12, 22),
            "  set state.count = \"hello\"",
        )
        .with_suggestion("Use convert.to_int(value) to convert String to Number");

        let json = serde_json::to_string_pretty(&err).unwrap();
        assert!(json.contains("\"code\""));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"source_line\""));
        assert!(json.contains("\"suggestion\""));
        // Verify JSON field names match compiler.md spec
        assert!(
            json.contains("\"line\""),
            "JSON must use 'line' not 'start_line'"
        );
        assert!(
            json.contains("\"column\""),
            "JSON must use 'column' not 'start_col'"
        );
        assert!(json.contains("\"end_line\""));
        assert!(
            json.contains("\"end_column\""),
            "JSON must use 'end_column' not 'end_col'"
        );

        // Round-trip
        let deserialized: PeplError = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.code, err.code);
        assert_eq!(deserialized.message, err.message);
    }

    #[test]
    fn test_compile_errors_max_limit() {
        let mut errs = CompileErrors::empty();
        for i in 0..25 {
            errs.push_error(PeplError::new(
                "test.pepl",
                ErrorCode::UNEXPECTED_TOKEN,
                format!("Error {i}"),
                Span::point(i as u32 + 1, 1),
                "",
            ));
        }
        // Only 20 stored, but total count is 25
        assert_eq!(errs.errors.len(), 20);
        assert_eq!(errs.total_errors, 25);
        assert!(errs.has_errors());
    }

    #[test]
    fn test_compile_errors_empty() {
        let errs = CompileErrors::empty();
        assert!(!errs.has_errors());
        assert_eq!(errs.total_errors, 0);
        assert_eq!(errs.total_warnings, 0);
    }

    #[test]
    fn test_compile_errors_json_output() {
        let mut errs = CompileErrors::empty();
        errs.push_error(PeplError::new(
            "test.pepl",
            ErrorCode::TYPE_MISMATCH,
            "Type mismatch",
            Span::new(1, 1, 1, 10),
            "set count = \"hello\"",
        ));

        let json = serde_json::to_string(&errs).unwrap();
        assert!(json.contains("\"total_errors\":1"));
        assert!(json.contains("\"total_warnings\":0"));
    }

    #[test]
    fn test_error_determinism_100_iterations() {
        let first = PeplError::new(
            "test.pepl",
            ErrorCode::TYPE_MISMATCH,
            "Type mismatch",
            Span::new(12, 5, 12, 22),
            "set count = \"hello\"",
        );
        let first_json = serde_json::to_string(&first).unwrap();

        for i in 0..100 {
            let err = PeplError::new(
                "test.pepl",
                ErrorCode::TYPE_MISMATCH,
                "Type mismatch",
                Span::new(12, 5, 12, 22),
                "set count = \"hello\"",
            );
            let json = serde_json::to_string(&err).unwrap();
            assert_eq!(first_json, json, "Determinism failure at iteration {i}");
        }
    }
}

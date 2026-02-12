use serde::{Deserialize, Serialize};
use std::fmt;

/// Source location span.
///
/// All line/column values are 1-based for human-readable error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    /// Create a new span.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Create a zero-width span at a single position.
    pub fn point(line: u32, col: u32) -> Self {
        Self::new(line, col, line, col)
    }

    /// Merge two spans into one that covers both.
    pub fn merge(self, other: Span) -> Span {
        let start_line = self.start_line.min(other.start_line);
        let start_col = if self.start_line < other.start_line {
            self.start_col
        } else if other.start_line < self.start_line {
            other.start_col
        } else {
            self.start_col.min(other.start_col)
        };

        let end_line = self.end_line.max(other.end_line);
        let end_col = if self.end_line > other.end_line {
            self.end_col
        } else if other.end_line > self.end_line {
            other.end_col
        } else {
            self.end_col.max(other.end_col)
        };

        Span::new(start_line, start_col, end_line, end_col)
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start_line, self.start_col)
    }
}

/// Holds the source text for error reporting.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
    /// Cached line start byte offsets for fast line lookup.
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Create a new source file.
    pub fn new(name: impl Into<String>, source: impl Into<String>) -> Self {
        let source = source.into();
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        Self {
            name: name.into(),
            source,
            line_starts,
        }
    }

    /// Extract a source line by 1-based line number.
    ///
    /// Returns `None` if the line number is out of range.
    pub fn line(&self, line_number: u32) -> Option<&str> {
        let idx = line_number.checked_sub(1)? as usize;
        if idx >= self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[idx];
        let end = self
            .line_starts
            .get(idx + 1)
            .map(|&s| s.saturating_sub(1)) // strip the \n
            .unwrap_or(self.source.len());
        let line = &self.source[start..end];
        // Also strip trailing \r for CRLF
        Some(line.trim_end_matches('\r'))
    }

    /// Get the total number of lines.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_point() {
        let s = Span::point(1, 5);
        assert_eq!(s.start_line, 1);
        assert_eq!(s.start_col, 5);
        assert_eq!(s.end_line, 1);
        assert_eq!(s.end_col, 5);
    }

    #[test]
    fn test_span_merge() {
        let a = Span::new(1, 5, 1, 10);
        let b = Span::new(2, 3, 2, 8);
        let merged = a.merge(b);
        assert_eq!(merged.start_line, 1);
        assert_eq!(merged.start_col, 5);
        assert_eq!(merged.end_line, 2);
        assert_eq!(merged.end_col, 8);
    }

    #[test]
    fn test_span_merge_same_line() {
        let a = Span::new(1, 5, 1, 10);
        let b = Span::new(1, 3, 1, 8);
        let merged = a.merge(b);
        assert_eq!(merged.start_col, 3);
        assert_eq!(merged.end_col, 10);
    }

    #[test]
    fn test_span_display() {
        let s = Span::new(3, 7, 3, 15);
        assert_eq!(format!("{s}"), "3:7");
    }

    #[test]
    fn test_source_file_line_extraction() {
        let src = SourceFile::new("test.pepl", "line one\nline two\nline three");
        assert_eq!(src.line(1), Some("line one"));
        assert_eq!(src.line(2), Some("line two"));
        assert_eq!(src.line(3), Some("line three"));
        assert_eq!(src.line(0), None);
        assert_eq!(src.line(4), None);
    }

    #[test]
    fn test_source_file_crlf() {
        let src = SourceFile::new("test.pepl", "line one\r\nline two\r\n");
        assert_eq!(src.line(1), Some("line one"));
        assert_eq!(src.line(2), Some("line two"));
    }

    #[test]
    fn test_source_file_line_count() {
        let src = SourceFile::new("test.pepl", "a\nb\nc");
        assert_eq!(src.line_count(), 3);
    }

    #[test]
    fn test_source_file_empty() {
        let src = SourceFile::new("test.pepl", "");
        assert_eq!(src.line_count(), 1);
        assert_eq!(src.line(1), Some(""));
    }

    #[test]
    fn test_span_determinism_100_iterations() {
        let input_a = Span::new(1, 5, 1, 10);
        let input_b = Span::new(2, 3, 2, 8);
        let first = input_a.merge(input_b);
        for i in 0..100 {
            let result = input_a.merge(input_b);
            assert_eq!(first, result, "Determinism failure at iteration {i}");
        }
    }

    #[test]
    fn test_source_file_determinism_100_iterations() {
        let source_text = "space Counter {\n  state {\n    count: number = 0\n  }\n}";
        let first_file = SourceFile::new("test.pepl", source_text);
        let first_line2 = first_file.line(2).map(String::from);
        for i in 0..100 {
            let file = SourceFile::new("test.pepl", source_text);
            let line2 = file.line(2).map(String::from);
            assert_eq!(first_line2, line2, "Determinism failure at iteration {i}");
        }
    }
}

//! Source mapping — WASM function index → PEPL source location.
//!
//! Each entry maps a compiled WASM function to its originating PEPL source
//! span (line, column).  This enables the host to resolve WASM traps back to
//! human-readable source positions.
//!
//! Granularity is per-function in Phase 0.  Future phases may add
//! instruction-level mappings.

use serde::{Deserialize, Serialize};

/// A complete source map for a compiled PEPL module.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SourceMap {
    pub entries: Vec<SourceMapEntry>,
}

/// A single source map entry: one WASM function → one PEPL source region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapEntry {
    /// Absolute WASM function index (imports + runtime + space functions).
    pub wasm_func_index: u32,
    /// Human-readable name of the function (e.g. "init", "dispatch_action",
    /// "increment", "__test_0").
    pub func_name: String,
    /// Kind of function for host grouping.
    pub kind: FuncKind,
    /// Source span (1-based line/column).
    pub span: pepl_types::Span,
}

/// Classification of a compiled function for the host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FuncKind {
    /// Space-level infrastructure (init, dispatch, render, get_state, dealloc).
    SpaceInfra,
    /// An action implementation.
    Action,
    /// A view render function.
    View,
    /// The update(dt) loop callback.
    Update,
    /// The handleEvent callback.
    HandleEvent,
    /// A compiled test case (__test_N).
    Test,
    /// The __test_count helper.
    TestCount,
    /// A compiled lambda body.
    Lambda,
    /// invoke_lambda trampoline.
    InvokeLambda,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Push a new entry.
    pub fn push(
        &mut self,
        wasm_func_index: u32,
        func_name: impl Into<String>,
        kind: FuncKind,
        span: pepl_types::Span,
    ) {
        self.entries.push(SourceMapEntry {
            wasm_func_index,
            func_name: func_name.into(),
            kind,
            span,
        });
    }

    /// Look up the entry whose WASM function index matches.
    pub fn find_by_func_index(&self, idx: u32) -> Option<&SourceMapEntry> {
        self.entries.iter().find(|e| e.wasm_func_index == idx)
    }

    /// Serialize to JSON bytes for embedding in a WASM custom section.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from JSON bytes.
    pub fn from_json(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pepl_types::Span;

    #[test]
    fn round_trip_json() {
        let mut sm = SourceMap::new();
        sm.push(35, "init", FuncKind::SpaceInfra, Span::new(5, 3, 10, 4));
        sm.push(36, "dispatch_action", FuncKind::SpaceInfra, Span::new(12, 3, 30, 4));
        sm.push(37, "increment", FuncKind::Action, Span::new(15, 5, 18, 6));

        let json = sm.to_json();
        let sm2 = SourceMap::from_json(&json).expect("parse failed");
        assert_eq!(sm2.entries.len(), 3);
        assert_eq!(sm2.entries[0].func_name, "init");
        assert_eq!(sm2.entries[2].kind, FuncKind::Action);
    }

    #[test]
    fn find_by_func_index() {
        let mut sm = SourceMap::new();
        sm.push(35, "init", FuncKind::SpaceInfra, Span::new(5, 3, 10, 4));
        sm.push(36, "dispatch", FuncKind::SpaceInfra, Span::new(12, 3, 30, 4));

        assert!(sm.find_by_func_index(35).is_some());
        assert_eq!(sm.find_by_func_index(35).unwrap().func_name, "init");
        assert!(sm.find_by_func_index(99).is_none());
    }
}

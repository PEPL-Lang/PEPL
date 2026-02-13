//! Scoped variable environment for the PEPL evaluator.

use pepl_stdlib::Value;
use std::collections::BTreeMap;

/// A single scope level.
#[derive(Debug, Clone)]
struct Scope {
    bindings: BTreeMap<String, Value>,
}

impl Scope {
    fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }
}

/// Scoped variable environment with push/pop semantics.
///
/// Variables are looked up from innermost scope outward.
/// `define` always creates in the current (innermost) scope.
/// `set` updates the first scope where the variable exists.
#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<Scope>,
}

impl Environment {
    /// Create a new environment with one global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
        }
    }

    /// Push a new scope (for action bodies, let blocks, etc.).
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Pop the innermost scope.
    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Define a variable in the current (innermost) scope.
    pub fn define(&mut self, name: &str, value: Value) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name.to_string(), value);
        }
    }

    /// Look up a variable, searching from innermost to outermost scope.
    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.bindings.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Update a variable in the first scope where it exists.
    /// Returns `true` if found and updated, `false` if not found.
    pub fn set(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.bindings.contains_key(name) {
                scope.bindings.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }

    /// Get all bindings in the global (outermost) scope.
    /// Used for capturing state.
    pub fn global_bindings(&self) -> &BTreeMap<String, Value> {
        &self.scopes[0].bindings
    }

    /// Replace all bindings in the global scope (for rollback).
    pub fn restore_global(&mut self, bindings: BTreeMap<String, Value>) {
        self.scopes[0].bindings = bindings;
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

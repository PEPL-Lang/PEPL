//! Type environment with lexically scoped bindings.
//!
//! [`TypeEnv`] manages a stack of scopes, each carrying variable bindings
//! and metadata about the current context (action, view, lambda, etc.).

use std::collections::HashMap;

use crate::ty::Type;

// ══════════════════════════════════════════════════════════════════════════════
// Scope Kind
// ══════════════════════════════════════════════════════════════════════════════

/// What kind of code context a scope represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Top-level space scope — state fields, type declarations.
    Space,
    /// Inside an action body — `set` is allowed.
    Action,
    /// Inside a view body — must be pure (no `set`, no capabilities).
    View,
    /// Inside a block (if/for/match) body.
    Block,
    /// Inside a lambda body.
    Lambda,
    /// Inside a derived field expression.
    Derived,
    /// Inside an invariant expression.
    Invariant,
    /// Inside a test case body.
    TestCase,
    /// Inside `update(dt)` body.
    Update,
    /// Inside `handleEvent(event)` body.
    HandleEvent,
}

// ══════════════════════════════════════════════════════════════════════════════
// Scope
// ══════════════════════════════════════════════════════════════════════════════

/// A single scope level.
#[derive(Debug)]
struct Scope {
    kind: ScopeKind,
    bindings: HashMap<String, Type>,
}

// ══════════════════════════════════════════════════════════════════════════════
// TypeEnv
// ══════════════════════════════════════════════════════════════════════════════

/// A stack of scopes for name resolution and type tracking.
#[derive(Debug)]
pub struct TypeEnv {
    scopes: Vec<Scope>,
}

impl TypeEnv {
    /// Create a new type environment with an initial Space scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope {
                kind: ScopeKind::Space,
                bindings: HashMap::new(),
            }],
        }
    }

    /// Push a new scope onto the stack.
    pub fn push_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope {
            kind,
            bindings: HashMap::new(),
        });
    }

    /// Pop the top scope off the stack.
    pub fn pop_scope(&mut self) {
        debug_assert!(self.scopes.len() > 1, "cannot pop the root scope");
        self.scopes.pop();
    }

    /// Define a binding in the current (top) scope.
    /// Returns `false` if the name is already defined in the current scope
    /// (variable shadowing check).
    pub fn define(&mut self, name: &str, ty: Type) -> bool {
        let scope = self.scopes.last_mut().expect("no scope");
        if scope.bindings.contains_key(name) {
            return false;
        }
        scope.bindings.insert(name.to_string(), ty);
        true
    }

    /// Look up a binding by name, searching from innermost to outermost scope.
    pub fn lookup(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.bindings.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Check if a name is defined in the **current** (innermost) scope only.
    pub fn defined_in_current_scope(&self, name: &str) -> bool {
        self.scopes
            .last()
            .is_some_and(|s| s.bindings.contains_key(name))
    }

    /// Check if we are inside an action (anywhere in the scope chain).
    pub fn in_action(&self) -> bool {
        self.scopes.iter().any(|s| {
            matches!(
                s.kind,
                ScopeKind::Action | ScopeKind::Update | ScopeKind::HandleEvent
            )
        })
    }

    /// Check if we are inside a view (anywhere in the scope chain).
    pub fn in_view(&self) -> bool {
        self.scopes.iter().any(|s| s.kind == ScopeKind::View)
    }

    /// Check if we are inside a derived expression.
    pub fn in_derived(&self) -> bool {
        self.scopes.iter().any(|s| s.kind == ScopeKind::Derived)
    }

    /// Check if we are inside an invariant expression.
    pub fn in_invariant(&self) -> bool {
        self.scopes.iter().any(|s| s.kind == ScopeKind::Invariant)
    }

    /// Check if we are inside a test case.
    pub fn in_test(&self) -> bool {
        self.scopes.iter().any(|s| s.kind == ScopeKind::TestCase)
    }

    /// Get the kind of the current (innermost) scope.
    pub fn current_scope_kind(&self) -> ScopeKind {
        self.scopes.last().expect("no scope").kind
    }

    /// Narrow a binding's type in the current scope (for nil narrowing).
    /// Creates a new binding in the current scope that shadows the outer one.
    pub fn narrow(&mut self, name: &str, ty: Type) {
        let scope = self.scopes.last_mut().expect("no scope");
        scope.bindings.insert(name.to_string(), ty);
    }
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::new()
    }
}

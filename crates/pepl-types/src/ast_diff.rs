//! AST diff infrastructure for PEPL.
//!
//! Compares two PEPL ASTs and produces a structured list of changes.
//! Used for:
//! - Evolve operation scope validation
//! - Incremental compilation (re-codegen only changed subtrees)
//! - Event storage optimization (store diffs instead of full snapshots)
//! - PEPL's transformation guarantee: "diffs are AST-level, not line-level"

use crate::ast::*;
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════════════════════════════════════════════
// Types
// ══════════════════════════════════════════════════════════════════════════════

/// A single change between two ASTs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AstChange {
    /// Dot-separated path to the changed node (e.g., "state.count", "actions.increment").
    pub path: String,
    /// The kind of change.
    pub kind: ChangeKind,
}

/// What kind of change occurred.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChangeKind {
    /// A node was added (not present in old AST).
    Added,
    /// A node was removed (not present in new AST).
    Removed,
    /// A node was modified (present in both, but different).
    Modified,
}

/// A structured diff between two PEPL ASTs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AstDiff {
    pub changes: Vec<AstChange>,
}

/// Allowed scope for change validation.
#[derive(Debug, Clone, PartialEq)]
pub enum AllowedScope {
    /// Any change is allowed.
    Any,
    /// Only changes within these paths are allowed.
    Paths(Vec<String>),
}

// ══════════════════════════════════════════════════════════════════════════════
// Core diff
// ══════════════════════════════════════════════════════════════════════════════

impl AstDiff {
    /// Compute the diff between two programs.
    pub fn diff(old: &Program, new: &Program) -> Self {
        let mut changes = Vec::new();
        diff_program(old, new, &mut changes);
        AstDiff { changes }
    }

    /// True if the two ASTs are identical (no changes).
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Serialize to compact JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "[]".to_string())
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Validate that all changes fall within allowed scopes.
    /// Returns the list of out-of-scope changes (empty = all valid).
    pub fn validate_scope(&self, scope: &AllowedScope) -> Vec<&AstChange> {
        match scope {
            AllowedScope::Any => vec![],
            AllowedScope::Paths(allowed) => self
                .changes
                .iter()
                .filter(|c| !allowed.iter().any(|a| c.path.starts_with(a)))
                .collect(),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Diff walkers
// ══════════════════════════════════════════════════════════════════════════════

fn push(changes: &mut Vec<AstChange>, path: &str, kind: ChangeKind) {
    changes.push(AstChange {
        path: path.to_string(),
        kind,
    });
}

fn diff_program(old: &Program, new: &Program, changes: &mut Vec<AstChange>) {
    // Space name
    if old.space.name.name != new.space.name.name {
        push(changes, "space.name", ChangeKind::Modified);
    }

    // Space body
    diff_space_body(&old.space.body, &new.space.body, changes);

    // Test blocks – TestsBlock has no name, so diff by index; inner cases keyed by description
    let max_tests = old.tests.len().max(new.tests.len());
    for i in 0..max_tests {
        match (old.tests.get(i), new.tests.get(i)) {
            (Some(o), Some(n)) => {
                diff_vec_by_name(
                    &o.cases,
                    &n.cases,
                    |c| c.description.clone(),
                    &format!("tests[{}].cases", i),
                    changes,
                );
            }
            (None, Some(_)) => {
                changes.push(AstChange {
                    path: format!("tests[{}]", i),
                    kind: ChangeKind::Added,
                });
            }
            (Some(_), None) => {
                changes.push(AstChange {
                    path: format!("tests[{}]", i),
                    kind: ChangeKind::Removed,
                });
            }
            (None, None) => {}
        }
    }
}

fn diff_space_body(old: &SpaceBody, new: &SpaceBody, changes: &mut Vec<AstChange>) {
    // Types
    diff_vec_by_name(
        &old.types,
        &new.types,
        |t| t.name.name.clone(),
        "types",
        changes,
    );

    // State fields
    diff_vec_by_name(
        &old.state.fields,
        &new.state.fields,
        |f| f.name.name.clone(),
        "state",
        changes,
    );

    // Capabilities
    match (&old.capabilities, &new.capabilities) {
        (None, Some(_)) => push(changes, "capabilities", ChangeKind::Added),
        (Some(_), None) => push(changes, "capabilities", ChangeKind::Removed),
        (Some(o), Some(n)) => {
            // Required capabilities
            diff_vec_by_name(
                &o.required,
                &n.required,
                |c| c.name.clone(),
                "capabilities.required",
                changes,
            );
            // Optional capabilities
            diff_vec_by_name(
                &o.optional,
                &n.optional,
                |c| c.name.clone(),
                "capabilities.optional",
                changes,
            );
        }
        (None, None) => {}
    }

    // Credentials
    match (&old.credentials, &new.credentials) {
        (None, Some(_)) => push(changes, "credentials", ChangeKind::Added),
        (Some(_), None) => push(changes, "credentials", ChangeKind::Removed),
        (Some(o), Some(n)) => {
            diff_vec_by_name(
                &o.fields,
                &n.fields,
                |c| c.name.name.clone(),
                "credentials",
                changes,
            );
        }
        (None, None) => {}
    }

    // Derived
    match (&old.derived, &new.derived) {
        (None, Some(_)) => push(changes, "derived", ChangeKind::Added),
        (Some(_), None) => push(changes, "derived", ChangeKind::Removed),
        (Some(o), Some(n)) => {
            diff_vec_by_name(
                &o.fields,
                &n.fields,
                |f| f.name.name.clone(),
                "derived",
                changes,
            );
        }
        (None, None) => {}
    }

    // Invariants
    diff_vec_by_name(
        &old.invariants,
        &new.invariants,
        |i| i.name.name.clone(),
        "invariants",
        changes,
    );

    // Actions
    diff_vec_by_name(
        &old.actions,
        &new.actions,
        |a| a.name.name.clone(),
        "actions",
        changes,
    );

    // Views
    diff_vec_by_name(
        &old.views,
        &new.views,
        |v| v.name.name.clone(),
        "views",
        changes,
    );

    // Update
    diff_option_block(&old.update, &new.update, "update", changes);

    // HandleEvent
    diff_option_block(&old.handle_event, &new.handle_event, "handleEvent", changes);
}

/// Diff two vectors of named items. Items are matched by name.
fn diff_vec_by_name<T: PartialEq>(
    old: &[T],
    new: &[T],
    name_fn: impl Fn(&T) -> String,
    prefix: &str,
    changes: &mut Vec<AstChange>,
) {
    let old_names: Vec<String> = old.iter().map(&name_fn).collect();
    let new_names: Vec<String> = new.iter().map(&name_fn).collect();

    // Removed items
    for (i, name) in old_names.iter().enumerate() {
        if !new_names.contains(name) {
            push(changes, &format!("{prefix}.{name}"), ChangeKind::Removed);
        } else {
            // Check if modified
            let new_idx = new_names.iter().position(|n| n == name).unwrap();
            if old[i] != new[new_idx] {
                push(changes, &format!("{prefix}.{name}"), ChangeKind::Modified);
            }
        }
    }

    // Added items
    for name in &new_names {
        if !old_names.contains(name) {
            push(changes, &format!("{prefix}.{name}"), ChangeKind::Added);
        }
    }
}

/// Diff optional blocks (update, handle_event).
fn diff_option_block<T: PartialEq>(
    old: &Option<T>,
    new: &Option<T>,
    name: &str,
    changes: &mut Vec<AstChange>,
) {
    match (old, new) {
        (None, Some(_)) => push(changes, name, ChangeKind::Added),
        (Some(_), None) => push(changes, name, ChangeKind::Removed),
        (Some(o), Some(n)) if o != n => push(changes, name, ChangeKind::Modified),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    fn span() -> Span {
        Span::new(0, 0, 0, 0)
    }

    fn ident(name: &str) -> Ident {
        Ident {
            name: name.to_string(),
            span: span(),
        }
    }

    fn minimal_program(name: &str) -> Program {
        Program {
            space: SpaceDecl {
                name: ident(name),
                body: SpaceBody {
                    types: vec![],
                    state: StateBlock {
                        fields: vec![],
                        span: span(),
                    },
                    capabilities: None,
                    credentials: None,
                    derived: None,
                    invariants: vec![],
                    actions: vec![],
                    views: vec![],
                    update: None,
                    handle_event: None,
                    span: span(),
                },
                span: span(),
            },
            tests: vec![],
            span: span(),
        }
    }

    fn with_state_field(mut prog: Program, name: &str, default: Expr) -> Program {
        prog.space.body.state.fields.push(StateField {
            name: ident(name),
            type_ann: TypeAnnotation {
                kind: TypeKind::Named("number".to_string()),
                span: span(),
            },
            default,
            span: span(),
        });
        prog
    }

    fn with_action(mut prog: Program, name: &str) -> Program {
        prog.space.body.actions.push(ActionDecl {
            name: ident(name),
            params: vec![],
            body: Block {
                stmts: vec![],
                span: span(),
            },
            span: span(),
        });
        prog
    }

    fn with_view(mut prog: Program, name: &str) -> Program {
        prog.space.body.views.push(ViewDecl {
            name: ident(name),
            params: vec![],
            body: UIBlock {
                elements: vec![],
                span: span(),
            },
            span: span(),
        });
        prog
    }

    fn num_literal(n: f64) -> Expr {
        Expr::new(ExprKind::NumberLit(n), span())
    }

    #[allow(dead_code)]
    fn str_literal(s: &str) -> Expr {
        Expr::new(ExprKind::StringLit(s.to_string()), span())
    }

    // ─── Tests ───────────────────────────────────────────────────────────

    #[test]
    fn identical_programs_produce_empty_diff() {
        let a = minimal_program("Test");
        let b = minimal_program("Test");
        let diff = AstDiff::diff(&a, &b);
        assert!(diff.is_empty());
        assert_eq!(diff.len(), 0);
    }

    #[test]
    fn space_name_change_detected() {
        let a = minimal_program("Old");
        let b = minimal_program("New");
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "space.name");
        assert_eq!(diff.changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn state_field_added() {
        let a = minimal_program("T");
        let b = with_state_field(minimal_program("T"), "count", num_literal(0.0));
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "state.count");
        assert_eq!(diff.changes[0].kind, ChangeKind::Added);
    }

    #[test]
    fn state_field_removed() {
        let a = with_state_field(minimal_program("T"), "count", num_literal(0.0));
        let b = minimal_program("T");
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "state.count");
        assert_eq!(diff.changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn state_field_modified() {
        let a = with_state_field(minimal_program("T"), "count", num_literal(0.0));
        let b = with_state_field(minimal_program("T"), "count", num_literal(42.0));
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "state.count");
        assert_eq!(diff.changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn action_added() {
        let a = minimal_program("T");
        let b = with_action(minimal_program("T"), "increment");
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "actions.increment");
        assert_eq!(diff.changes[0].kind, ChangeKind::Added);
    }

    #[test]
    fn action_removed() {
        let a = with_action(minimal_program("T"), "increment");
        let b = minimal_program("T");
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "actions.increment");
        assert_eq!(diff.changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn view_modified() {
        let a = with_view(minimal_program("T"), "main");
        let mut b = with_view(minimal_program("T"), "main");
        // Modify the view by adding an element
        b.space.body.views[0].body.elements.push(UIElement::Component(ComponentExpr {
            name: ident("Text"),
            props: vec![],
            children: None,
            span: span(),
        }));
        let diff = AstDiff::diff(&a, &b);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff.changes[0].path, "views.main");
        assert_eq!(diff.changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn multiple_changes_detected() {
        let a = with_state_field(
            with_action(minimal_program("T"), "old_action"),
            "x",
            num_literal(0.0),
        );
        let b = with_state_field(
            with_action(minimal_program("T"), "new_action"),
            "x",
            num_literal(1.0),
        );
        let diff = AstDiff::diff(&a, &b);
        // state.x modified, actions.old_action removed, actions.new_action added
        assert_eq!(diff.len(), 3);
    }

    #[test]
    fn json_round_trip() {
        let a = minimal_program("T");
        let b = with_state_field(minimal_program("T"), "x", num_literal(0.0));
        let diff = AstDiff::diff(&a, &b);
        let json = diff.to_json();
        let restored = AstDiff::from_json(&json).unwrap();
        assert_eq!(diff, restored);
    }

    #[test]
    fn scope_validation_any_allows_all() {
        let a = minimal_program("T");
        let b = with_state_field(minimal_program("T"), "x", num_literal(0.0));
        let diff = AstDiff::diff(&a, &b);
        let violations = diff.validate_scope(&AllowedScope::Any);
        assert!(violations.is_empty());
    }

    #[test]
    fn scope_validation_rejects_out_of_scope() {
        let a = minimal_program("T");
        let b = with_action(
            with_state_field(minimal_program("T"), "x", num_literal(0.0)),
            "inc",
        );
        let diff = AstDiff::diff(&a, &b);
        let violations = diff.validate_scope(&AllowedScope::Paths(vec!["state".to_string()]));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].path, "actions.inc");
    }

    #[test]
    fn scope_validation_accepts_in_scope() {
        let a = minimal_program("T");
        let b = with_state_field(minimal_program("T"), "x", num_literal(0.0));
        let diff = AstDiff::diff(&a, &b);
        let violations = diff.validate_scope(&AllowedScope::Paths(vec!["state".to_string()]));
        assert!(violations.is_empty());
    }

    #[test]
    fn empty_diff_json() {
        let diff = AstDiff {
            changes: vec![],
        };
        let json = diff.to_json();
        assert_eq!(json, r#"{"changes":[]}"#);
    }
}

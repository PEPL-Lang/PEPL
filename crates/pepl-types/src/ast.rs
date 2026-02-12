//! AST node types for the PEPL language.
//!
//! Every node carries a [`Span`] for error reporting.
//! Large recursive types are boxed to keep enum sizes reasonable.
//! [`BTreeMap`] is NOT used here — AST preserves source order.

use crate::Span;

// ══════════════════════════════════════════════════════════════════════════════
// Top Level
// ══════════════════════════════════════════════════════════════════════════════

/// A complete PEPL program: one space declaration + zero or more test blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub space: SpaceDecl,
    pub tests: Vec<TestsBlock>,
    pub span: Span,
}

/// `space Name { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceDecl {
    pub name: Ident,
    pub body: SpaceBody,
    pub span: Span,
}

/// The body of a space declaration — blocks in enforced order.
///
/// Block ordering: types → state → capabilities → credentials → derived →
/// invariants → actions → views → update → handleEvent
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceBody {
    pub types: Vec<TypeDecl>,
    pub state: StateBlock,
    pub capabilities: Option<CapabilitiesBlock>,
    pub credentials: Option<CredentialsBlock>,
    pub derived: Option<DerivedBlock>,
    pub invariants: Vec<InvariantDecl>,
    pub actions: Vec<ActionDecl>,
    pub views: Vec<ViewDecl>,
    pub update: Option<UpdateDecl>,
    pub handle_event: Option<HandleEventDecl>,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Identifiers
// ══════════════════════════════════════════════════════════════════════════════

/// A spanned identifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self {
            name: name.into(),
            span,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Type Declarations
// ══════════════════════════════════════════════════════════════════════════════

/// `type Name = ...`
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub name: Ident,
    pub body: TypeDeclBody,
    pub span: Span,
}

/// The body of a type declaration — either a sum type or a type alias.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDeclBody {
    /// `type Shape = | Circle(radius: number) | Rectangle(width: number, height: number)`
    SumType(Vec<VariantDef>),
    /// `type Meters = number`
    Alias(TypeAnnotation),
}

/// A sum type variant: `Circle(radius: number)` or `Active` (unit variant).
#[derive(Debug, Clone, PartialEq)]
pub struct VariantDef {
    pub name: Ident,
    pub params: Vec<Param>,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// State & Related Blocks
// ══════════════════════════════════════════════════════════════════════════════

/// `state { field: type = default, ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct StateBlock {
    pub fields: Vec<StateField>,
    pub span: Span,
}

/// A single state field: `count: number = 0`
#[derive(Debug, Clone, PartialEq)]
pub struct StateField {
    pub name: Ident,
    pub type_ann: TypeAnnotation,
    pub default: Expr,
    pub span: Span,
}

/// `capabilities { required: [...], optional: [...] }`
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilitiesBlock {
    pub required: Vec<Ident>,
    pub optional: Vec<Ident>,
    pub span: Span,
}

/// `credentials { api_key: string, ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct CredentialsBlock {
    pub fields: Vec<CredentialField>,
    pub span: Span,
}

/// A credential field: `api_key: string`
#[derive(Debug, Clone, PartialEq)]
pub struct CredentialField {
    pub name: Ident,
    pub type_ann: TypeAnnotation,
    pub span: Span,
}

/// `derived { total: number = list.length(items), ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedBlock {
    pub fields: Vec<DerivedField>,
    pub span: Span,
}

/// A derived field: `total: number = list.length(items)`
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedField {
    pub name: Ident,
    pub type_ann: TypeAnnotation,
    pub value: Expr,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Invariants
// ══════════════════════════════════════════════════════════════════════════════

/// `invariant name { expr }`
#[derive(Debug, Clone, PartialEq)]
pub struct InvariantDecl {
    pub name: Ident,
    pub condition: Expr,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Actions
// ══════════════════════════════════════════════════════════════════════════════

/// `action name(params) { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct ActionDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

/// A parameter: `name: type`
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub type_ann: TypeAnnotation,
    pub span: Span,
}

/// `{ statements... }`
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Views
// ══════════════════════════════════════════════════════════════════════════════

/// `view name(params) -> Surface { ui_elements... }`
#[derive(Debug, Clone, PartialEq)]
pub struct ViewDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub body: UIBlock,
    pub span: Span,
}

/// A UI block: `{ ui_elements... }`
#[derive(Debug, Clone, PartialEq)]
pub struct UIBlock {
    pub elements: Vec<UIElement>,
    pub span: Span,
}

/// An element inside a UI block.
#[derive(Debug, Clone, PartialEq)]
pub enum UIElement {
    /// `Component { props } [{ children }]`
    Component(ComponentExpr),
    /// `let name = expr`
    Let(LetBinding),
    /// `if cond { ui... } [else { ui... }]`
    If(UIIf),
    /// `for item [, index] in expr { ui... }`
    For(UIFor),
}

/// A component expression: `Text { value: "hello" }` or `Modal { visible: v } { children }`
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentExpr {
    pub name: Ident,
    pub props: Vec<PropAssign>,
    pub children: Option<UIBlock>,
    pub span: Span,
}

/// A prop assignment: `label: "Click me"`
#[derive(Debug, Clone, PartialEq)]
pub struct PropAssign {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

/// `if` in a UI context — bodies contain UIElements, not Statements.
#[derive(Debug, Clone, PartialEq)]
pub struct UIIf {
    pub condition: Expr,
    pub then_block: UIBlock,
    pub else_block: Option<UIElse>,
    pub span: Span,
}

/// The else branch of a UI if — either another if or a UI block.
#[derive(Debug, Clone, PartialEq)]
pub enum UIElse {
    ElseIf(Box<UIIf>),
    Block(UIBlock),
}

/// `for item [, index] in expr { ui... }` in a UI context.
#[derive(Debug, Clone, PartialEq)]
pub struct UIFor {
    pub item: Ident,
    pub index: Option<Ident>,
    pub iterable: Expr,
    pub body: UIBlock,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Game Loop
// ══════════════════════════════════════════════════════════════════════════════

/// `update(dt: number) { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateDecl {
    pub param: Param,
    pub body: Block,
    pub span: Span,
}

/// `handleEvent(event: InputEvent) { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct HandleEventDecl {
    pub param: Param,
    pub body: Block,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

/// `tests { test_cases... }`
#[derive(Debug, Clone, PartialEq)]
pub struct TestsBlock {
    pub cases: Vec<TestCase>,
    pub span: Span,
}

/// `test "description" [with_responses { ... }] { body }`
#[derive(Debug, Clone, PartialEq)]
pub struct TestCase {
    pub description: String,
    pub with_responses: Option<WithResponses>,
    pub body: Block,
    pub span: Span,
}

/// `with_responses { module.function(args) -> value, ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct WithResponses {
    pub mappings: Vec<ResponseMapping>,
    pub span: Span,
}

/// `module.function(args) -> value`
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseMapping {
    pub module: Ident,
    pub function: Ident,
    pub args: Vec<Expr>,
    pub response: Expr,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Statements
// ══════════════════════════════════════════════════════════════════════════════

/// A statement in a code block.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `set field = expr` or `set record.field.nested = expr`
    Set(SetStmt),
    /// `let name: Type = expr` or `let _ = expr`
    Let(LetBinding),
    /// `if cond { ... } [else { ... }]`
    If(IfExpr),
    /// `for item [, index] in expr { ... }`
    For(ForExpr),
    /// `match expr { arms... }`
    Match(MatchExpr),
    /// `return`
    Return(ReturnStmt),
    /// `assert expr [, "message"]`
    Assert(AssertStmt),
    /// A bare expression (value is discarded unless last in block).
    Expr(ExprStmt),
}

/// `set target = value`
#[derive(Debug, Clone, PartialEq)]
pub struct SetStmt {
    /// Path segments: `["record", "field", "nested"]`
    pub target: Vec<Ident>,
    pub value: Expr,
    pub span: Span,
}

/// `let name: Type = expr` or `let _ = expr`
#[derive(Debug, Clone, PartialEq)]
pub struct LetBinding {
    /// `None` for `let _ = expr` (discard binding)
    pub name: Option<Ident>,
    pub type_ann: Option<TypeAnnotation>,
    pub value: Expr,
    pub span: Span,
}

/// `return`
#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub span: Span,
}

/// `assert expr [, "message"]`
#[derive(Debug, Clone, PartialEq)]
pub struct AssertStmt {
    pub condition: Expr,
    pub message: Option<String>,
    pub span: Span,
}

/// A bare expression statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub expr: Expr,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Expressions
// ══════════════════════════════════════════════════════════════════════════════

/// An expression node. Uses `Box` for recursive variants.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The kind of expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    // ── Literals ──
    /// `42`, `3.14`
    NumberLit(f64),
    /// `"hello"` (no interpolation)
    StringLit(String),
    /// `"hello ${name}"` — parts alternate: string, expr, string, expr, string
    StringInterpolation(Vec<StringPart>),
    /// `true` / `false`
    BoolLit(bool),
    /// `nil`
    NilLit,
    /// `[expr, ...]`
    ListLit(Vec<Expr>),
    /// `{ field: expr, ...spread, ... }`
    RecordLit(Vec<RecordEntry>),

    // ── Identifiers & Calls ──
    /// `my_var`, `count`
    Identifier(String),
    /// `func(args...)` — unqualified call
    Call {
        name: Ident,
        args: Vec<Expr>,
    },
    /// `module.function(args...)` — qualified (stdlib) call
    QualifiedCall {
        module: Ident,
        function: Ident,
        args: Vec<Expr>,
    },
    /// `expr.field`
    FieldAccess {
        object: Box<Expr>,
        field: Ident,
    },
    /// `expr.method(args...)`
    MethodCall {
        object: Box<Expr>,
        method: Ident,
        args: Vec<Expr>,
    },

    // ── Operators ──
    /// `a + b`, `a == b`, `a and b`, etc.
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    /// `-x`, `not x`
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    /// `expr?` — Result unwrap
    ResultUnwrap(Box<Expr>),
    /// `a ?? b` — nil-coalescing
    NilCoalesce {
        left: Box<Expr>,
        right: Box<Expr>,
    },

    // ── Control Flow ──
    /// `if cond { ... } [else { ... }]`
    If(Box<IfExpr>),
    /// `for item [, index] in expr { ... }`
    For(Box<ForExpr>),
    /// `match expr { arms... }`
    Match(Box<MatchExpr>),

    // ── Lambda ──
    /// `fn(params) { body }`
    Lambda(Box<LambdaExpr>),

    // ── Grouping ──
    /// `(expr)`
    Paren(Box<Expr>),
}

/// A part of an interpolated string.
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    /// Literal text segment.
    Literal(String),
    /// An interpolated expression `${expr}`.
    Expr(Expr),
}

/// An entry in a record literal.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordEntry {
    /// `field: expr`
    Field { name: Ident, value: Expr },
    /// `...expr`
    Spread(Expr),
}

// ── Binary Operators ──────────────────────────────────────────────────────────

/// Binary operators (in precedence order, lowest first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Logical
    Or,
    And,
    // Comparison
    Eq,
    NotEq,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl BinOp {
    /// Returns the operator symbol for error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            BinOp::Or => "or",
            BinOp::And => "and",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Less => "<",
            BinOp::Greater => ">",
            BinOp::LessEq => "<=",
            BinOp::GreaterEq => ">=",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-x`
    Neg,
    /// `not x`
    Not,
}

// ── Control Flow Expressions ──────────────────────────────────────────────────

/// `if cond { stmts... } [else { stmts... } | else if ...]`
#[derive(Debug, Clone, PartialEq)]
pub struct IfExpr {
    pub condition: Expr,
    pub then_block: Block,
    pub else_branch: Option<ElseBranch>,
    pub span: Span,
}

/// The else branch of an if expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ElseBranch {
    /// `else if cond { ... }`
    ElseIf(Box<IfExpr>),
    /// `else { ... }`
    Block(Block),
}

/// `for item [, index] in iterable { stmts... }`
#[derive(Debug, Clone, PartialEq)]
pub struct ForExpr {
    pub item: Ident,
    pub index: Option<Ident>,
    pub iterable: Expr,
    pub body: Block,
    pub span: Span,
}

/// `match expr { arms... }`
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub subject: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// `Pattern -> expr | { stmts... }`
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: MatchArmBody,
    pub span: Span,
}

/// The body of a match arm — either a single expression or a block.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchArmBody {
    Expr(Expr),
    Block(Block),
}

/// A pattern in a match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `VariantName` or `VariantName(a, b, c)`
    Variant {
        name: Ident,
        bindings: Vec<Ident>,
    },
    /// `_` wildcard
    Wildcard(Span),
}

// ── Lambda ────────────────────────────────────────────────────────────────────

/// `fn(params) { body }` — block-body only.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaExpr {
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

// ══════════════════════════════════════════════════════════════════════════════
// Type Annotations
// ══════════════════════════════════════════════════════════════════════════════

/// A type annotation in PEPL source code.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAnnotation {
    pub kind: TypeKind,
    pub span: Span,
}

impl TypeAnnotation {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// The kind of type.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// `number`
    Number,
    /// `string`
    String,
    /// `bool`
    Bool,
    /// `nil`
    Nil,
    /// `any` — stdlib-only, rejected in user code by the type checker
    Any,
    /// `color`
    Color,
    /// `Surface`
    Surface,
    /// `InputEvent`
    InputEvent,
    /// `list<T>`
    List(Box<TypeAnnotation>),
    /// `{ name: string, age?: number }` — anonymous record type
    Record(Vec<RecordTypeField>),
    /// `Result<T, E>`
    Result(Box<TypeAnnotation>, Box<TypeAnnotation>),
    /// `(T1, T2) -> R` — function type
    Function {
        params: Vec<TypeAnnotation>,
        ret: Box<TypeAnnotation>,
    },
    /// User-defined type name (sum type or alias): `Shape`, `Priority`
    Named(String),
}

/// A field in an anonymous record type: `name?: Type`
#[derive(Debug, Clone, PartialEq)]
pub struct RecordTypeField {
    pub name: Ident,
    pub optional: bool,
    pub type_ann: TypeAnnotation,
    pub span: Span,
}

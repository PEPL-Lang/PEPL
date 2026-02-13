//! Internal type representation for the PEPL type checker.
//!
//! [`Type`] is the semantic type used during type checking.
//! It is distinct from [`pepl_types::ast::TypeAnnotation`], which is the
//! syntactic representation produced by the parser.

use std::fmt;

// ══════════════════════════════════════════════════════════════════════════════
// Type
// ══════════════════════════════════════════════════════════════════════════════

/// A semantic type in PEPL.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // ── Primitives ──
    Number,
    String,
    Bool,
    Nil,
    Color,

    // ── Special ──
    /// The `Surface` type returned by views.
    Surface,
    /// The `InputEvent` parameter for `handleEvent`.
    InputEvent,
    /// Internal type for stdlib parameters. Rejected in user code (E200).
    Any,
    /// Actions / statements that produce no value.
    Void,
    /// Type could not be determined (error recovery).
    Unknown,

    // ── Composites ──
    /// `list<T>`
    List(Box<Type>),
    /// `{ field: Type, ... }` — structural record.
    Record(Vec<RecordField>),
    /// `Result<T, E>`
    Result(Box<Type>, Box<Type>),
    /// `(T1, T2, ...) -> R`
    Function(Vec<Type>, Box<Type>),

    // ── User-Defined ──
    /// A sum type declared with `type Name = | Variant1 | Variant2(...)`.
    SumType {
        name: std::string::String,
        variants: Vec<SumVariant>,
    },
    /// A reference to a user-defined type name (resolved during checking).
    Named(std::string::String),

    // ── Nullable ──
    /// `T | nil` — used for nil narrowing.
    Nullable(Box<Type>),
}

/// A field in a structural record type.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordField {
    pub name: std::string::String,
    pub ty: Type,
    pub optional: bool,
}

/// A variant in a sum type.
#[derive(Debug, Clone, PartialEq)]
pub struct SumVariant {
    pub name: std::string::String,
    pub params: Vec<(std::string::String, Type)>,
}

/// A function signature entry (for stdlib and user calls).
#[derive(Debug, Clone)]
pub struct FnSig {
    pub params: Vec<(std::string::String, Type)>,
    pub ret: Type,
    /// If true, parameter count is variable (e.g. `list.of`).
    pub variadic: bool,
}

// ══════════════════════════════════════════════════════════════════════════════
// Conversion from AST TypeAnnotation
// ══════════════════════════════════════════════════════════════════════════════

impl Type {
    /// Convert an AST `TypeAnnotation` into a semantic `Type`.
    pub fn from_annotation(ann: &pepl_types::ast::TypeAnnotation) -> Self {
        use pepl_types::ast::TypeKind;
        match &ann.kind {
            TypeKind::Number => Type::Number,
            TypeKind::String => Type::String,
            TypeKind::Bool => Type::Bool,
            TypeKind::Nil => Type::Nil,
            TypeKind::Color => Type::Color,
            TypeKind::Surface => Type::Surface,
            TypeKind::InputEvent => Type::InputEvent,
            TypeKind::Any => Type::Any,
            TypeKind::List(inner) => Type::List(Box::new(Type::from_annotation(inner))),
            TypeKind::Result(ok, err) => Type::Result(
                Box::new(Type::from_annotation(ok)),
                Box::new(Type::from_annotation(err)),
            ),
            TypeKind::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|f| RecordField {
                        name: f.name.name.clone(),
                        ty: Type::from_annotation(&f.type_ann),
                        optional: f.optional,
                    })
                    .collect(),
            ),
            TypeKind::Function { params, ret } => Type::Function(
                params.iter().map(Type::from_annotation).collect(),
                Box::new(Type::from_annotation(ret)),
            ),
            TypeKind::Named(name) => Type::Named(name.clone()),
        }
    }

    /// Check if this type is assignable to `target`.
    ///
    /// Rules:
    /// - Same type → yes
    /// - `Any` target accepts everything
    /// - `Any` source is accepted by everything
    /// - `Nil` is assignable to `Nullable(T)`
    /// - `T` is assignable to `Nullable(T)`
    /// - `Unknown` is compatible with anything (error recovery)
    pub fn is_assignable_to(&self, target: &Type) -> bool {
        if self == target {
            return true;
        }
        // Unknown (from error recovery) is compatible with anything
        if matches!(self, Type::Unknown) || matches!(target, Type::Unknown) {
            return true;
        }
        // Any accepts/provides anything
        if matches!(target, Type::Any) || matches!(self, Type::Any) {
            return true;
        }
        // Nil is assignable to Nullable(T)
        if matches!(self, Type::Nil)
            && matches!(target, Type::Nullable(_))
        {
            return true;
        }
        // T is assignable to Nullable(T)
        if let Type::Nullable(inner) = target {
            if self.is_assignable_to(inner) {
                return true;
            }
        }
        // Nullable(T) is assignable to Nullable(T)
        if let (Type::Nullable(a), Type::Nullable(b)) = (self, target) {
            return a.is_assignable_to(b);
        }
        // List covariance (simplified)
        if let (Type::List(a), Type::List(b)) = (self, target) {
            return a.is_assignable_to(b);
        }
        // Named types resolve to the same name
        if let (Type::Named(a), Type::Named(b)) = (self, target) {
            return a == b;
        }
        // SumType matches Named
        if let (Type::SumType { name, .. }, Type::Named(n)) = (self, target) {
            return name == n;
        }
        if let (Type::Named(n), Type::SumType { name, .. }) = (self, target) {
            return n == name;
        }
        // Record structural subtyping: source has all required fields of target
        if let (Type::Record(src_fields), Type::Record(tgt_fields)) = (self, target) {
            return tgt_fields.iter().all(|tf| {
                if tf.optional {
                    // Optional field: if present in source, must be compatible
                    src_fields
                        .iter()
                        .find(|sf| sf.name == tf.name)
                        .is_none_or(|sf| sf.ty.is_assignable_to(&tf.ty))
                } else {
                    // Required field: must be present and compatible
                    src_fields
                        .iter()
                        .find(|sf| sf.name == tf.name)
                        .is_some_and(|sf| sf.ty.is_assignable_to(&tf.ty))
                }
            });
        }
        // Function types: covariant return, contravariant parameters
        if let (Type::Function(self_params, self_ret), Type::Function(tgt_params, tgt_ret)) =
            (self, target)
        {
            if self_params.len() != tgt_params.len() {
                return false;
            }
            // Contravariant: target params must be assignable to self params
            let params_ok = self_params
                .iter()
                .zip(tgt_params.iter())
                .all(|(sp, tp)| tp.is_assignable_to(sp));
            return params_ok && self_ret.is_assignable_to(tgt_ret);
        }
        false
    }

    /// Returns the inner type if this is `Nullable(T)`, otherwise returns self.
    pub fn unwrap_nullable(&self) -> &Type {
        match self {
            Type::Nullable(inner) => inner,
            other => other,
        }
    }

    /// Returns true if this type is numeric.
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Number | Type::Any | Type::Unknown)
    }

    /// Returns true if this type is boolean.
    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool | Type::Any | Type::Unknown)
    }

    /// Returns true if this type could be nil.
    pub fn is_nullable(&self) -> bool {
        matches!(self, Type::Nil | Type::Nullable(_) | Type::Any | Type::Unknown)
    }

    /// Returns true if this type is a Result.
    pub fn is_result(&self) -> bool {
        matches!(self, Type::Result(_, _) | Type::Any | Type::Unknown)
    }

    /// Short display name for error messages.
    pub fn display_name(&self) -> std::string::String {
        format!("{}", self)
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Display
// ══════════════════════════════════════════════════════════════════════════════

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Number => write!(f, "number"),
            Type::String => write!(f, "string"),
            Type::Bool => write!(f, "bool"),
            Type::Nil => write!(f, "nil"),
            Type::Color => write!(f, "color"),
            Type::Surface => write!(f, "Surface"),
            Type::InputEvent => write!(f, "InputEvent"),
            Type::Any => write!(f, "any"),
            Type::Void => write!(f, "void"),
            Type::Unknown => write!(f, "unknown"),
            Type::List(inner) => write!(f, "list<{}>", inner),
            Type::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, rf) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    if rf.optional {
                        write!(f, "{}?: {}", rf.name, rf.ty)?;
                    } else {
                        write!(f, "{}: {}", rf.name, rf.ty)?;
                    }
                }
                write!(f, " }}")
            }
            Type::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            Type::Function(params, ret) => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::SumType { name, .. } => write!(f, "{}", name),
            Type::Named(name) => write!(f, "{}", name),
            Type::Nullable(inner) => write!(f, "{}?", inner),
        }
    }
}

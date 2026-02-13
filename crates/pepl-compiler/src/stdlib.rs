//! Standard library function signature registry.
//!
//! Registers all 88 Phase 0 stdlib function signatures so the type checker
//! can validate qualified calls like `math.abs(x)`.

use std::collections::HashMap;

use crate::ty::{FnSig, Type};

/// Registry mapping `(module, function)` → function signature.
#[derive(Debug)]
pub struct StdlibRegistry {
    modules: HashMap<String, HashMap<String, FnSig>>,
    /// Module-level constants: `(module, name)` → Type.
    constants: HashMap<String, HashMap<String, Type>>,
}

impl StdlibRegistry {
    /// Create a new registry with all 88 Phase 0 stdlib functions.
    pub fn new() -> Self {
        let mut reg = Self {
            modules: HashMap::new(),
            constants: HashMap::new(),
        };
        reg.register_core();
        reg.register_math();
        reg.register_string();
        reg.register_list();
        reg.register_record();
        reg.register_time();
        reg.register_convert();
        reg.register_json();
        reg.register_timer();
        reg.register_http();
        reg.register_storage();
        reg.register_location();
        reg.register_notifications();
        reg
    }

    /// Look up a function signature by module and function name.
    pub fn get(&self, module: &str, function: &str) -> Option<&FnSig> {
        self.modules.get(module)?.get(function)
    }

    /// Check if a module exists.
    pub fn has_module(&self, module: &str) -> bool {
        self.modules.contains_key(module) || self.constants.contains_key(module)
    }

    /// Look up a constant by module and name.
    pub fn get_constant(&self, module: &str, name: &str) -> Option<&Type> {
        self.constants.get(module)?.get(name)
    }

    /// Iterate over all registered modules and their functions.
    pub fn modules(&self) -> &HashMap<String, HashMap<String, FnSig>> {
        &self.modules
    }

    /// Iterate over all registered constants.
    pub fn all_constants(&self) -> &HashMap<String, HashMap<String, Type>> {
        &self.constants
    }

    // ──────────────────────────────────────────────────────────────────────
    // Registration helpers
    // ──────────────────────────────────────────────────────────────────────

    fn add(&mut self, module: &str, name: &str, sig: FnSig) {
        self.modules
            .entry(module.to_string())
            .or_default()
            .insert(name.to_string(), sig);
    }

    fn add_const(&mut self, module: &str, name: &str, ty: Type) {
        self.constants
            .entry(module.to_string())
            .or_default()
            .insert(name.to_string(), ty);
    }

    fn sig(params: Vec<(&str, Type)>, ret: Type) -> FnSig {
        FnSig {
            params: params
                .into_iter()
                .map(|(n, t)| (n.to_string(), t))
                .collect(),
            ret,
            variadic: false,
        }
    }

    fn variadic_sig(params: Vec<(&str, Type)>, ret: Type) -> FnSig {
        FnSig {
            params: params
                .into_iter()
                .map(|(n, t)| (n.to_string(), t))
                .collect(),
            ret,
            variadic: true,
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    // Module registration (88 functions + 2 constants)
    // ══════════════════════════════════════════════════════════════════════

    /// core: 4 functions
    fn register_core(&mut self) {
        use Type::*;
        self.add("core", "log", Self::sig(vec![("value", Any)], Nil));
        self.add(
            "core",
            "assert",
            Self::sig(vec![("condition", Bool), ("message", String)], Nil),
        );
        self.add("core", "type_of", Self::sig(vec![("value", Any)], String));
        self.add(
            "core",
            "capability",
            Self::sig(vec![("name", String)], Bool),
        );
    }

    /// math: 10 functions + 2 constants (PI, E)
    fn register_math(&mut self) {
        use Type::*;
        self.add("math", "abs", Self::sig(vec![("x", Number)], Number));
        self.add(
            "math",
            "min",
            Self::sig(vec![("a", Number), ("b", Number)], Number),
        );
        self.add(
            "math",
            "max",
            Self::sig(vec![("a", Number), ("b", Number)], Number),
        );
        self.add("math", "floor", Self::sig(vec![("x", Number)], Number));
        self.add("math", "ceil", Self::sig(vec![("x", Number)], Number));
        self.add("math", "round", Self::sig(vec![("x", Number)], Number));
        self.add(
            "math",
            "round_to",
            Self::sig(vec![("x", Number), ("decimals", Number)], Number),
        );
        self.add(
            "math",
            "pow",
            Self::sig(vec![("base", Number), ("exp", Number)], Number),
        );
        self.add(
            "math",
            "clamp",
            Self::sig(vec![("x", Number), ("min", Number), ("max", Number)], Number),
        );
        self.add("math", "sqrt", Self::sig(vec![("x", Number)], Number));

        // Constants
        self.add_const("math", "PI", Number);
        self.add_const("math", "E", Number);
    }

    /// string: 20 functions
    fn register_string(&mut self) {
        use Type::*;
        self.add(
            "string",
            "length",
            Self::sig(vec![("s", String)], Number),
        );
        self.add(
            "string",
            "concat",
            Self::sig(vec![("a", String), ("b", String)], String),
        );
        self.add(
            "string",
            "contains",
            Self::sig(vec![("s", String), ("substr", String)], Bool),
        );
        self.add(
            "string",
            "slice",
            Self::sig(
                vec![("s", String), ("start", Number), ("end", Number)],
                String,
            ),
        );
        self.add("string", "trim", Self::sig(vec![("s", String)], String));
        self.add(
            "string",
            "split",
            Self::sig(
                vec![("s", String), ("delimiter", String)],
                List(Box::new(String)),
            ),
        );
        self.add(
            "string",
            "to_upper",
            Self::sig(vec![("s", String)], String),
        );
        self.add(
            "string",
            "to_lower",
            Self::sig(vec![("s", String)], String),
        );
        self.add(
            "string",
            "starts_with",
            Self::sig(vec![("s", String), ("prefix", String)], Bool),
        );
        self.add(
            "string",
            "ends_with",
            Self::sig(vec![("s", String), ("suffix", String)], Bool),
        );
        self.add(
            "string",
            "replace",
            Self::sig(
                vec![("s", String), ("from", String), ("to", String)],
                String,
            ),
        );
        self.add(
            "string",
            "replace_all",
            Self::sig(
                vec![("s", String), ("from", String), ("to", String)],
                String,
            ),
        );
        self.add(
            "string",
            "pad_start",
            Self::sig(
                vec![("s", String), ("length", Number), ("pad", String)],
                String,
            ),
        );
        self.add(
            "string",
            "pad_end",
            Self::sig(
                vec![("s", String), ("length", Number), ("pad", String)],
                String,
            ),
        );
        self.add(
            "string",
            "repeat",
            Self::sig(vec![("s", String), ("count", Number)], String),
        );
        self.add(
            "string",
            "join",
            Self::sig(
                vec![("items", List(Box::new(String))), ("separator", String)],
                String,
            ),
        );
        self.add(
            "string",
            "format",
            Self::sig(
                vec![("template", String), ("values", Record(vec![]))],
                String,
            ),
        );
        self.add("string", "from", Self::sig(vec![("value", Any)], String));
        self.add(
            "string",
            "is_empty",
            Self::sig(vec![("s", String)], Bool),
        );
        self.add(
            "string",
            "index_of",
            Self::sig(vec![("s", String), ("substr", String)], Number),
        );
    }

    /// list: 31 functions
    fn register_list(&mut self) {
        use Type::*;
        let t = || Any; // Generic T placeholder
        let list_t = || List(Box::new(Any));

        // Construction
        self.add("list", "empty", Self::sig(vec![], list_t()));
        self.add("list", "of", Self::variadic_sig(vec![("items", t())], list_t()));
        self.add(
            "list",
            "repeat",
            Self::sig(vec![("value", t()), ("count", Number)], list_t()),
        );
        self.add(
            "list",
            "range",
            Self::sig(vec![("start", Number), ("end", Number)], List(Box::new(Number))),
        );

        // Access
        self.add(
            "list",
            "length",
            Self::sig(vec![("items", list_t())], Number),
        );
        self.add(
            "list",
            "get",
            Self::sig(vec![("items", list_t()), ("index", Number)], t()),
        );
        self.add(
            "list",
            "first",
            Self::sig(vec![("items", list_t())], t()),
        );
        self.add(
            "list",
            "last",
            Self::sig(vec![("items", list_t())], t()),
        );
        self.add(
            "list",
            "index_of",
            Self::sig(vec![("items", list_t()), ("value", t())], Number),
        );

        // Modification (immutable — returns new list)
        self.add(
            "list",
            "append",
            Self::sig(vec![("items", list_t()), ("value", t())], list_t()),
        );
        self.add(
            "list",
            "prepend",
            Self::sig(vec![("items", list_t()), ("value", t())], list_t()),
        );
        self.add(
            "list",
            "insert",
            Self::sig(
                vec![("items", list_t()), ("index", Number), ("value", t())],
                list_t(),
            ),
        );
        self.add(
            "list",
            "remove",
            Self::sig(vec![("items", list_t()), ("index", Number)], list_t()),
        );
        self.add(
            "list",
            "update",
            Self::sig(
                vec![("items", list_t()), ("index", Number), ("value", t())],
                list_t(),
            ),
        );
        // list.set is a spec alias for list.update
        self.add(
            "list",
            "set",
            Self::sig(
                vec![("items", list_t()), ("index", Number), ("value", t())],
                list_t(),
            ),
        );
        self.add(
            "list",
            "slice",
            Self::sig(
                vec![("items", list_t()), ("start", Number), ("end", Number)],
                list_t(),
            ),
        );
        self.add(
            "list",
            "concat",
            Self::sig(vec![("a", list_t()), ("b", list_t())], list_t()),
        );
        self.add(
            "list",
            "reverse",
            Self::sig(vec![("items", list_t())], list_t()),
        );
        self.add(
            "list",
            "flatten",
            Self::sig(vec![("items", list_t())], list_t()),
        );
        self.add(
            "list",
            "unique",
            Self::sig(vec![("items", list_t())], list_t()),
        );

        // Higher-order
        self.add(
            "list",
            "map",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("f", Function(vec![t()], Box::new(t()))),
                ],
                list_t(),
            ),
        );
        self.add(
            "list",
            "filter",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                list_t(),
            ),
        );
        self.add(
            "list",
            "reduce",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("initial", t()),
                    ("f", Function(vec![t(), t()], Box::new(t()))),
                ],
                t(),
            ),
        );
        self.add(
            "list",
            "find",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                Nullable(Box::new(t())),
            ),
        );
        self.add(
            "list",
            "find_index",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                Number,
            ),
        );
        self.add(
            "list",
            "every",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                Bool,
            ),
        );
        // list.any (spec name) + list.some (backward-compat alias)
        self.add(
            "list",
            "any",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                Bool,
            ),
        );
        self.add(
            "list",
            "some",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                Bool,
            ),
        );
        self.add(
            "list",
            "sort",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("compare", Function(vec![t(), t()], Box::new(Number))),
                ],
                list_t(),
            ),
        );
        self.add(
            "list",
            "contains",
            Self::sig(vec![("items", list_t()), ("value", t())], Bool),
        );
        self.add(
            "list",
            "count",
            Self::sig(
                vec![
                    ("items", list_t()),
                    ("predicate", Function(vec![t()], Box::new(Bool))),
                ],
                Number,
            ),
        );
        self.add(
            "list",
            "zip",
            Self::sig(vec![("a", list_t()), ("b", list_t())], list_t()),
        );
        self.add(
            "list",
            "take",
            Self::sig(vec![("items", list_t()), ("n", Number)], list_t()),
        );
        self.add(
            "list",
            "drop",
            Self::sig(vec![("items", list_t()), ("n", Number)], list_t()),
        );
    }

    /// record: 5 functions
    fn register_record(&mut self) {
        use Type::*;
        self.add(
            "record",
            "get",
            Self::sig(vec![("rec", Record(vec![])), ("key", String)], Any),
        );
        self.add(
            "record",
            "set",
            Self::sig(
                vec![("rec", Record(vec![])), ("key", String), ("value", Any)],
                Record(vec![]),
            ),
        );
        self.add(
            "record",
            "has",
            Self::sig(vec![("rec", Record(vec![])), ("key", String)], Bool),
        );
        self.add(
            "record",
            "keys",
            Self::sig(
                vec![("rec", Record(vec![]))],
                List(Box::new(String)),
            ),
        );
        self.add(
            "record",
            "values",
            Self::sig(vec![("rec", Record(vec![]))], List(Box::new(Any))),
        );
    }

    /// time: 5 functions
    fn register_time(&mut self) {
        use Type::*;
        self.add("time", "now", Self::sig(vec![], Number));
        self.add(
            "time",
            "format",
            Self::sig(vec![("timestamp", Number), ("pattern", String)], String),
        );
        self.add(
            "time",
            "diff",
            Self::sig(vec![("a", Number), ("b", Number)], Number),
        );
        self.add(
            "time",
            "day_of_week",
            Self::sig(vec![("timestamp", Number)], Number),
        );
        self.add(
            "time",
            "start_of_day",
            Self::sig(vec![("timestamp", Number)], Number),
        );
    }

    /// convert: 5 functions
    fn register_convert(&mut self) {
        use Type::*;
        self.add(
            "convert",
            "to_string",
            Self::sig(vec![("value", Any)], String),
        );
        self.add(
            "convert",
            "to_number",
            Self::sig(
                vec![("value", Any)],
                Result(Box::new(Number), Box::new(String)),
            ),
        );
        self.add(
            "convert",
            "parse_int",
            Self::sig(
                vec![("s", String)],
                Result(Box::new(Number), Box::new(String)),
            ),
        );
        self.add(
            "convert",
            "parse_float",
            Self::sig(
                vec![("s", String)],
                Result(Box::new(Number), Box::new(String)),
            ),
        );
        self.add(
            "convert",
            "to_bool",
            Self::sig(vec![("value", Any)], Bool),
        );
    }

    /// json: 2 functions
    fn register_json(&mut self) {
        use Type::*;
        self.add(
            "json",
            "parse",
            Self::sig(
                vec![("s", String)],
                Result(Box::new(Any), Box::new(String)),
            ),
        );
        self.add(
            "json",
            "stringify",
            Self::sig(vec![("value", Any)], String),
        );
    }

    /// timer: 4 functions (capability: timer)
    fn register_timer(&mut self) {
        use Type::*;
        self.add(
            "timer",
            "start",
            Self::sig(vec![("id", String), ("interval_ms", Number)], String),
        );
        self.add(
            "timer",
            "start_once",
            Self::sig(vec![("id", String), ("delay_ms", Number)], String),
        );
        self.add(
            "timer",
            "stop",
            Self::sig(vec![("id", String)], Nil),
        );
        self.add("timer", "stop_all", Self::sig(vec![], Nil));
    }

    /// http: 5 functions (capability: http)
    fn register_http(&mut self) {
        use Type::*;
        let result_ty = |ok: Type| Result(Box::new(ok), Box::new(String));
        self.add("http", "get", Self::sig(vec![("url", String)], result_ty(String)));
        self.add(
            "http",
            "post",
            Self::sig(vec![("url", String), ("body", String)], result_ty(String)),
        );
        self.add(
            "http",
            "put",
            Self::sig(vec![("url", String), ("body", String)], result_ty(String)),
        );
        self.add(
            "http",
            "patch",
            Self::sig(vec![("url", String), ("body", String)], result_ty(String)),
        );
        self.add("http", "delete", Self::sig(vec![("url", String)], result_ty(String)));
    }

    /// storage: 4 functions (capability: storage)
    fn register_storage(&mut self) {
        use Type::*;
        self.add("storage", "get", Self::sig(vec![("key", String)], Nullable(Box::new(String))));
        self.add("storage", "set", Self::sig(vec![("key", String), ("value", String)], Nil));
        // storage.remove removed — spec only defines storage.delete (F1a)
        self.add("storage", "delete", Self::sig(vec![("key", String)], Nil));
        self.add(
            "storage",
            "keys",
            Self::sig(vec![], List(Box::new(String))),
        );
    }

    /// location: 1 function (capability: location)
    fn register_location(&mut self) {
        use Type::*;
        self.add(
            "location",
            "current",
            Self::sig(
                vec![],
                Record(vec![
                    crate::ty::RecordField { name: "lat".into(), ty: Number, optional: false },
                    crate::ty::RecordField { name: "lon".into(), ty: Number, optional: false },
                ]),
            ),
        );
    }

    /// notifications: 1 function (capability: notifications)
    fn register_notifications(&mut self) {
        use Type::*;
        self.add(
            "notifications",
            "send",
            Self::sig(vec![("title", String), ("body", String)], Nil),
        );
    }
}

impl Default for StdlibRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Modules that require capabilities to use.
pub fn capability_modules() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("http", "http");
    m.insert("storage", "storage");
    m.insert("location", "location");
    m.insert("notifications", "notifications");
    m.insert("timer", "timer");
    m
}

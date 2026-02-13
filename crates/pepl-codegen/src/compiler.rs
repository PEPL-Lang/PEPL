//! Main WASM module assembler.
//!
//! Orchestrates the code generation pipeline:
//! 1. Analyse the AST and collect metadata (state fields, actions, views, etc.)
//! 2. Build the data segment (intern string constants)
//! 3. Emit runtime helper functions
//! 4. Emit space-level functions (init, dispatch, render, get_state, …)
//! 5. Assemble all WASM sections into a valid module
//! 6. Validate with `wasmparser`

use std::collections::HashMap;

use pepl_types::ast::*;
use wasm_encoder::{
    CodeSection, ConstExpr, CustomSection, DataSection, EntityType, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, GlobalType,
    ImportSection, MemorySection, MemoryType, Module, TypeSection, ValType,
};

use crate::error::{CodegenError, CodegenResult};
use crate::runtime::{self, DataSegmentTracker, RT_FUNC_COUNT};
use crate::types::*;

// ══════════════════════════════════════════════════════════════════════════════
// Public API
// ══════════════════════════════════════════════════════════════════════════════

/// Compile a validated PEPL [`Program`] into a `.wasm` binary.
///
/// Returns the raw bytes of a valid WebAssembly module on success, or a
/// [`CodegenError`] describing what went wrong.
pub fn compile(program: &Program) -> CodegenResult<Vec<u8>> {
    let mut compiler = Compiler::new(program);
    compiler.compile()
}

// ══════════════════════════════════════════════════════════════════════════════
// Compiler
// ══════════════════════════════════════════════════════════════════════════════

/// The top-level compiler state.
struct Compiler<'a> {
    program: &'a Program,
    /// Data segment tracker for string constants.
    data: DataSegmentTracker,
    /// Extra user string data (appended after well-known strings).
    user_data: Vec<u8>,

    // ── Metadata ─────────────────────────────────────────────────────────
    /// State field names in declaration order.
    state_field_names: Vec<String>,
    /// Action names in declaration order (index = action_id).
    action_names: Vec<String>,
    /// View names in declaration order (index = view_id).
    view_names: Vec<String>,
    /// Variant names → numeric id (for match codegen).
    variant_ids: HashMap<String, u32>,
    /// Functions registered by name → absolute WASM function index.
    function_table: HashMap<String, u32>,
}

impl<'a> Compiler<'a> {
    fn new(program: &'a Program) -> Self {
        Self {
            program,
            data: DataSegmentTracker::new(),
            user_data: Vec::new(),
            state_field_names: Vec::new(),
            action_names: Vec::new(),
            view_names: Vec::new(),
            variant_ids: HashMap::new(),
            function_table: HashMap::new(),
        }
    }

    /// Run the full compilation pipeline.
    fn compile(&mut self) -> CodegenResult<Vec<u8>> {
        self.collect_metadata();

        let mut module = Module::new();

        // 1. Type section
        let types = self.emit_types();
        module.section(&types);

        // 2. Import section
        let imports = self.emit_imports();
        module.section(&imports);

        // 3. Function section + Code section (built together)
        let (func_section, code_section) = self.emit_functions()?;
        module.section(&func_section);

        // 4. Memory section
        let memory = self.emit_memory();
        module.section(&memory);

        // 5. Global section
        let globals = self.emit_globals();
        module.section(&globals);

        // 6. Export section
        let exports = self.emit_exports();
        module.section(&exports);

        // 7. Code section (must come after function/memory/global/export)
        module.section(&code_section);

        // 8. Data section
        let data_sec = self.emit_data();
        module.section(&data_sec);

        // 9. Custom section (PEPL metadata)
        let custom = self.emit_custom();
        module.section(&custom);

        let wasm_bytes = module.finish();

        // 10. Validate
        wasmparser::validate(&wasm_bytes).map_err(|e| {
            CodegenError::ValidationFailed(format!("{e}"))
        })?;

        Ok(wasm_bytes)
    }

    // ── Metadata collection ──────────────────────────────────────────────

    fn collect_metadata(&mut self) {
        let body = &self.program.space.body;

        // State fields
        for field in &body.state.fields {
            self.state_field_names.push(field.name.name.clone());
        }

        // Also add derived fields as state fields (they live on the state record)
        if let Some(derived) = &body.derived {
            for field in &derived.fields {
                self.state_field_names.push(field.name.name.clone());
            }
        }

        // Actions
        for action in &body.actions {
            self.action_names.push(action.name.name.clone());
        }

        // Views
        for view in &body.views {
            self.view_names.push(view.name.name.clone());
        }

        // Variant IDs from type declarations
        let mut vid = 0u32;
        for type_decl in &body.types {
            if let TypeDeclBody::SumType(variants) = &type_decl.body {
                for variant in variants {
                    self.variant_ids.insert(variant.name.name.clone(), vid);
                    vid += 1;
                }
            }
        }
    }

    // ── Type section ─────────────────────────────────────────────────────

    fn emit_types(&self) -> TypeSection {
        let mut types = TypeSection::new();

        // TYPE_VOID_VOID: () -> ()
        types.ty().function(vec![], vec![]);
        // TYPE_VOID_I32: () -> i32
        types.ty().function(vec![], vec![ValType::I32]);
        // TYPE_I32_VOID: (i32) -> ()
        types.ty().function(vec![ValType::I32], vec![]);
        // TYPE_I32_I32: (i32) -> i32
        types.ty().function(vec![ValType::I32], vec![ValType::I32]);
        // TYPE_I32X2_VOID: (i32, i32) -> ()
        types.ty().function(vec![ValType::I32, ValType::I32], vec![]);
        // TYPE_I32X2_I32: (i32, i32) -> i32
        types
            .ty()
            .function(vec![ValType::I32, ValType::I32], vec![ValType::I32]);
        // TYPE_I32X3_I32: (i32, i32, i32) -> i32
        types.ty().function(
            vec![ValType::I32, ValType::I32, ValType::I32],
            vec![ValType::I32],
        );
        // TYPE_F64_I32: (f64) -> i32
        types.ty().function(vec![ValType::F64], vec![ValType::I32]);
        // TYPE_I32_F64_VOID: (i32, f64) -> ()
        types
            .ty()
            .function(vec![ValType::I32, ValType::F64], vec![]);

        types
    }

    // ── Import section ───────────────────────────────────────────────────

    fn emit_imports(&self) -> ImportSection {
        let mut imports = ImportSection::new();

        // IMPORT_HOST_CALL: env.host_call(cap_id, fn_id, args_ptr) -> i32
        imports.import("env", "host_call", EntityType::Function(TYPE_I32X3_I32));
        // IMPORT_LOG: env.log(ptr, len)
        imports.import("env", "log", EntityType::Function(TYPE_I32X2_VOID));
        // IMPORT_TRAP: env.trap(ptr, len)
        imports.import("env", "trap", EntityType::Function(TYPE_I32X2_VOID));

        imports
    }

    // ── Memory section ───────────────────────────────────────────────────

    fn emit_memory(&self) -> MemorySection {
        let mut memory = MemorySection::new();
        memory.memory(MemoryType {
            minimum: INITIAL_MEMORY_PAGES,
            maximum: Some(MAX_MEMORY_PAGES),
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        memory
    }

    // ── Global section ───────────────────────────────────────────────────

    fn emit_globals(&self) -> GlobalSection {
        let mut globals = GlobalSection::new();

        // GLOBAL_HEAP_PTR — starts after data segment
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(HEAP_START as i32),
        );

        // GLOBAL_GAS
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(0),
        );

        // GLOBAL_GAS_LIMIT
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(1_000_000),
        );

        // GLOBAL_STATE_PTR
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(0),
        );

        globals
    }

    // ── Function + Code sections ─────────────────────────────────────────

    fn emit_functions(&mut self) -> CodegenResult<(FunctionSection, CodeSection)> {
        let mut func_section = FunctionSection::new();
        let mut code_section = CodeSection::new();

        // ── Runtime helpers ──────────────────────────────────────────────
        // Each runtime function is registered with its type index.

        // RT_ALLOC
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_alloc());

        // RT_VAL_NIL
        func_section.function(TYPE_VOID_I32);
        code_section.function(&runtime::emit_val_nil());

        // RT_VAL_NUMBER (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_number());

        // RT_VAL_BOOL (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_bool());

        // RT_VAL_STRING (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_string());

        // RT_VAL_LIST (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_list());

        // RT_VAL_RECORD (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_record());

        // RT_VAL_VARIANT (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_variant());

        // RT_VAL_ACTION_REF (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_action_ref());

        // RT_VAL_TAG (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_tag());

        // RT_VAL_GET_NUMBER (i32) -> i32 (returns lo bits)
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_get_number());

        // RT_VAL_GET_W1 (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_get_w1());

        // RT_VAL_GET_W2 (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_get_w2());

        // RT_VAL_EQ (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_eq());

        // RT_VAL_TO_STRING (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_to_string(&self.data));

        // RT_VAL_STRING_CONCAT (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_string_concat());

        // RT_VAL_ADD through RT_VAL_MOD
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_add());
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_sub());
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_mul());
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_div(
            self.data.div_by_zero_ptr,
            self.data.div_by_zero_len,
        ));
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_mod());

        // RT_VAL_NEG (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_neg());

        // RT_VAL_NOT (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_val_not());

        // RT_VAL_LT, LE, GT, GE (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_lt());
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_le());
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_gt());
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_ge());

        // RT_VAL_RECORD_GET (i32, i32, i32) -> i32
        func_section.function(TYPE_I32X3_I32);
        code_section.function(&runtime::emit_val_record_get());

        // RT_VAL_LIST_GET (i32, i32) -> i32
        func_section.function(TYPE_I32X2_I32);
        code_section.function(&runtime::emit_val_list_get());

        // RT_CHECK_NAN (i32) -> i32
        func_section.function(TYPE_I32_I32);
        code_section.function(&runtime::emit_check_nan(
            self.data.nan_ptr,
            self.data.nan_len,
        ));

        // ── Space-level functions ────────────────────────────────────────
        let body = &self.program.space.body;

        // init(gas_limit: i32)
        let init_idx = IMPORT_COUNT + RT_FUNC_COUNT;
        func_section.function(TYPE_I32_VOID);
        let mut init_scratch = Function::new(vec![]);
        let mut init_ctx = self.make_func_context(1); // 1 param
        crate::space::emit_init(
            &body.state,
            body.derived.as_ref(),
            &mut init_ctx,
            &mut init_scratch,
        )?;
        self.merge_user_data(&init_ctx);
        code_section.function(&Self::finalize_function(init_scratch, &init_ctx));

        // dispatch_action(action_id: i32, args_ptr: i32) -> i32
        let dispatch_idx = init_idx + 1;
        func_section.function(TYPE_I32X2_I32);
        let mut dispatch_scratch = Function::new(vec![]);
        let mut dispatch_ctx = self.make_func_context(2);
        crate::space::emit_dispatch_action(
            &body.actions,
            &body.invariants,
            body.derived.as_ref(),
            &mut dispatch_ctx,
            &mut dispatch_scratch,
        )?;
        self.merge_user_data(&dispatch_ctx);
        code_section.function(&Self::finalize_function(dispatch_scratch, &dispatch_ctx));

        // render(view_id: i32) -> i32
        let render_idx = dispatch_idx + 1;
        func_section.function(TYPE_I32_I32);
        let mut render_scratch = Function::new(vec![]);
        let mut render_ctx = self.make_func_context(1);
        crate::space::emit_render(&body.views, &mut render_ctx, &mut render_scratch)?;
        self.merge_user_data(&render_ctx);
        code_section.function(&Self::finalize_function(render_scratch, &render_ctx));

        // get_state() -> i32
        let get_state_idx = render_idx + 1;
        func_section.function(TYPE_VOID_I32);
        let mut get_state_func = Function::new(vec![]);
        crate::space::emit_get_state(&mut get_state_func);
        // get_state has no extra locals, so no finalization needed
        code_section.function(&get_state_func);

        // alloc(size: i32) -> i32 (re-export of RT_ALLOC for host use)
        // Already part of runtime, we'll just export RT_ALLOC directly.

        // Conditionally: update(dt_ptr: i32)
        let mut next_idx = get_state_idx + 1;
        if let Some(update_decl) = &body.update {
            self.function_table
                .insert("update".to_string(), next_idx);
            func_section.function(TYPE_I32_VOID);
            let mut update_scratch = Function::new(vec![]);
            let mut update_ctx = self.make_func_context(1);
            crate::space::emit_update(
                update_decl,
                body.derived.as_ref(),
                &mut update_ctx,
                &mut update_scratch,
            )?;
            self.merge_user_data(&update_ctx);
            code_section.function(&Self::finalize_function(update_scratch, &update_ctx));
            next_idx += 1;
        }

        // Conditionally: handle_event(event_ptr: i32)
        if let Some(handle_event_decl) = &body.handle_event {
            self.function_table
                .insert("handle_event".to_string(), next_idx);
            func_section.function(TYPE_I32_VOID);
            let mut he_scratch = Function::new(vec![]);
            let mut he_ctx = self.make_func_context(1);
            crate::space::emit_handle_event(
                handle_event_decl,
                body.derived.as_ref(),
                &mut he_ctx,
                &mut he_scratch,
            )?;
            self.merge_user_data(&he_ctx);
            code_section.function(&Self::finalize_function(he_scratch, &he_ctx));
            // next_idx += 1; // not needed — no more functions after this
        }

        Ok((func_section, code_section))
    }

    // ── Export section ────────────────────────────────────────────────────

    fn emit_exports(&self) -> ExportSection {
        let mut exports = ExportSection::new();
        let base = IMPORT_COUNT + RT_FUNC_COUNT;

        exports.export("init", ExportKind::Func, base);
        exports.export("dispatch_action", ExportKind::Func, base + 1);
        exports.export("render", ExportKind::Func, base + 2);
        exports.export("get_state", ExportKind::Func, base + 3);
        exports.export("alloc", ExportKind::Func, IMPORT_COUNT + runtime::RT_ALLOC);
        exports.export("memory", ExportKind::Memory, 0);

        // Conditional exports
        if self.function_table.contains_key("update") {
            exports.export(
                "update",
                ExportKind::Func,
                *self.function_table.get("update").unwrap(),
            );
        }
        if self.function_table.contains_key("handle_event") {
            exports.export(
                "handle_event",
                ExportKind::Func,
                *self.function_table.get("handle_event").unwrap(),
            );
        }

        exports
    }

    // ── Data section ─────────────────────────────────────────────────────

    fn emit_data(&self) -> DataSection {
        let mut data_sec = DataSection::new();
        let mut all_data = self.data.data_bytes();
        all_data.extend_from_slice(&self.user_data);
        data_sec.active(0, &ConstExpr::i32_const(0), all_data);
        data_sec
    }

    // ── Custom section ───────────────────────────────────────────────────

    fn emit_custom(&self) -> CustomSection<'_> {
        CustomSection {
            name: std::borrow::Cow::Borrowed(CUSTOM_SECTION_NAME),
            data: std::borrow::Cow::Borrowed(COMPILER_VERSION.as_bytes()),
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Create a [`FuncContext`] for code-generating a function body.
    fn make_func_context(&self, param_count: u32) -> FuncContext {
        FuncContext {
            locals: Vec::new(),
            local_names: HashMap::new(),
            next_local: param_count,
            state_field_names: self.state_field_names.clone(),
            action_names: self.action_names.clone(),
            variant_ids: self.variant_ids.clone(),
            function_table: self.function_table.clone(),
            data: self.data.clone_tracker(),
            user_data: Vec::new(),
        }
    }

    /// Merge user data from a function context back into the compiler.
    fn merge_user_data(&mut self, ctx: &FuncContext) {
        self.user_data.extend_from_slice(&ctx.user_data);
        // Update data tracker offset
        self.data.next_offset = ctx.data.next_offset;
    }

    /// Finalize a scratch function: rebuild with correct local declarations.
    ///
    /// `Function::new(vec![])` declares 0 locals, so its raw body starts with
    /// a single 0x00 byte (LEB128 zero).  We strip that byte and prepend the
    /// actual locals from `ctx`.
    fn finalize_function(scratch: Function, ctx: &FuncContext) -> Function {
        let raw = scratch.into_raw_body();
        // raw[0] == 0x00 (the "0 local declarations" byte).  Everything after
        // that is instruction bytes we want to keep.
        let instr_bytes = &raw[1..];
        let mut f = Function::new(ctx.locals.clone());
        f.raw(instr_bytes.iter().copied());
        f
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// FuncContext — per-function codegen state
// ══════════════════════════════════════════════════════════════════════════════

/// State maintained while generating code for a single function body.
pub struct FuncContext {
    /// Additional locals declared during codegen: (count, type).
    pub locals: Vec<(u32, ValType)>,
    /// Name → local index stack (for scoped let bindings).
    pub local_names: HashMap<String, Vec<u32>>,
    /// Next available local index.
    pub next_local: u32,
    /// State field names for identifier resolution.
    pub state_field_names: Vec<String>,
    /// Action names for action-ref resolution.
    pub action_names: Vec<String>,
    /// Variant name → id.
    pub variant_ids: HashMap<String, u32>,
    /// Known functions.
    pub function_table: HashMap<String, u32>,
    /// Data segment tracker (for interning strings).
    pub data: DataSegmentTrackerClone,
    /// User string data accumulated during codegen.
    pub user_data: Vec<u8>,
}

impl FuncContext {
    /// Allocate a new local of the given type. Returns the local index.
    pub fn alloc_local(&mut self, ty: ValType) -> u32 {
        let idx = self.next_local;
        self.next_local += 1;
        self.locals.push((1, ty));
        idx
    }

    /// Push a named local binding (for `let` and `for` scopes).
    pub fn push_local(&mut self, name: &str, idx: u32) {
        self.local_names
            .entry(name.to_string())
            .or_default()
            .push(idx);
    }

    /// Pop a named local binding.
    pub fn pop_local(&mut self, name: &str) {
        if let Some(stack) = self.local_names.get_mut(name) {
            stack.pop();
        }
    }

    /// Get the local index for a named binding.
    pub fn get_local(&self, name: &str) -> Option<u32> {
        self.local_names
            .get(name)
            .and_then(|stack| stack.last().copied())
    }

    /// Check if a name is a state field.
    pub fn is_state_field(&self, name: &str) -> bool {
        self.state_field_names.iter().any(|s| s == name)
    }

    /// Get the action_id for a name.
    pub fn get_action_id(&self, name: &str) -> Option<usize> {
        self.action_names.iter().position(|a| a == name)
    }

    /// Get verbose function index by name.
    pub fn get_function(&self, name: &str) -> Option<u32> {
        self.function_table.get(name).copied()
    }

    /// Get variant id by name.
    pub fn get_variant_id(&self, name: &str) -> u32 {
        *self.variant_ids.get(name).unwrap_or(&0)
    }

    /// Intern a string constant, returning (offset, length).
    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        let (ptr, len) = self.data.intern_string(s);
        self.user_data.extend_from_slice(s.as_bytes());
        (ptr, len)
    }

    /// Resolve a qualified call to (module_id, function_id).
    ///
    /// For capability modules this returns the capability/function IDs.
    /// For pure stdlib modules, we use synthetic IDs starting at 100.
    pub fn resolve_qualified_call(&self, module: &str, function: &str) -> (u32, u32) {
        match module {
            "http" => {
                let fn_id = match function {
                    "get" => 1,
                    "post" => 2,
                    "put" => 3,
                    "patch" => 4,
                    "delete" => 5,
                    _ => 0,
                };
                (1, fn_id)
            }
            "storage" => {
                let fn_id = match function {
                    "get" => 1,
                    "set" => 2,
                    "delete" => 3,
                    "keys" => 4,
                    _ => 0,
                };
                (2, fn_id)
            }
            "location" => (3, if function == "current" { 1 } else { 0 }),
            "notifications" => (4, if function == "send" { 1 } else { 0 }),
            "credential" => (5, if function == "get" { 1 } else { 0 }),
            // Pure stdlib: use IDs 100+
            "math" => (100, self.stdlib_fn_id(function)),
            "string" => (101, self.stdlib_fn_id(function)),
            "list" => (102, self.stdlib_fn_id(function)),
            "record" => (103, self.stdlib_fn_id(function)),
            "json" => (104, self.stdlib_fn_id(function)),
            "convert" => (105, self.stdlib_fn_id(function)),
            "time" => (106, self.stdlib_fn_id(function)),
            "timer" => (107, self.stdlib_fn_id(function)),
            "core" => (108, self.stdlib_fn_id(function)),
            _ => (0, 0),
        }
    }

    /// Resolve a method call to (module_id, function_id).
    pub fn resolve_method_call(&self, method: &str) -> (u32, u32) {
        // Method calls map to stdlib modules based on common patterns
        let fn_id = self.stdlib_fn_id(method);
        // Module 0 = generic method dispatch
        (0, fn_id)
    }

    /// Assign a numeric ID to a stdlib function name.
    fn stdlib_fn_id(&self, function: &str) -> u32 {
        // Simple hash-based ID assignment for stdlib functions.
        // This is deterministic across compilations.
        let mut hash: u32 = 5381;
        for b in function.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u32);
        }
        hash & 0xFFFF
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DataSegmentTrackerClone — clonable view of DataSegmentTracker for FuncContext
// ══════════════════════════════════════════════════════════════════════════════

/// A clonable wrapper around key fields from [`DataSegmentTracker`].
#[derive(Clone)]
pub struct DataSegmentTrackerClone {
    pub true_ptr: u32,
    pub true_len: u32,
    pub false_ptr: u32,
    pub false_len: u32,
    pub nil_ptr: u32,
    pub nil_len: u32,
    pub value_ptr: u32,
    pub value_len: u32,
    pub gas_exhausted_ptr: u32,
    pub gas_exhausted_len: u32,
    pub div_by_zero_ptr: u32,
    pub div_by_zero_len: u32,
    pub nan_ptr: u32,
    pub nan_len: u32,
    pub assert_failed_ptr: u32,
    pub assert_failed_len: u32,
    pub invariant_failed_ptr: u32,
    pub invariant_failed_len: u32,
    pub next_offset: u32,
}

impl DataSegmentTrackerClone {
    /// Intern a string in the clone tracker.
    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        let ptr = self.next_offset;
        let len = s.len() as u32;
        self.next_offset += len;
        (ptr, len)
    }
}

impl DataSegmentTracker {
    /// Create a clonable snapshot of this tracker.
    pub fn clone_tracker(&self) -> DataSegmentTrackerClone {
        DataSegmentTrackerClone {
            true_ptr: self.true_ptr,
            true_len: self.true_len,
            false_ptr: self.false_ptr,
            false_len: self.false_len,
            nil_ptr: self.nil_ptr,
            nil_len: self.nil_len,
            value_ptr: self.value_ptr,
            value_len: self.value_len,
            gas_exhausted_ptr: self.gas_exhausted_ptr,
            gas_exhausted_len: self.gas_exhausted_len,
            div_by_zero_ptr: self.div_by_zero_ptr,
            div_by_zero_len: self.div_by_zero_len,
            nan_ptr: self.nan_ptr,
            nan_len: self.nan_len,
            assert_failed_ptr: self.assert_failed_ptr,
            assert_failed_len: self.assert_failed_len,
            invariant_failed_ptr: self.invariant_failed_ptr,
            invariant_failed_len: self.invariant_failed_len,
            next_offset: self.next_offset,
        }
    }
}

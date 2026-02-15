# ROADMAP — pepl (Compiler)

> PEPL compiler: Lexer → Parser → Type Checker → Invariant Checker → Evaluator → WASM Codegen.
> Written in Rust, compiles to WASM (runs in browser Web Worker).
> See `ORCHESTRATION.md` in the [`.github`](https://github.com/PEPL-Lang/.github) repo for cross-repo sequencing.

---

## Phase 1: Project Scaffolding & Error Infrastructure

### 1.1 Cargo Workspace Setup
- [x] Create Cargo workspace with crates: `pepl-compiler`, `pepl-lexer`, `pepl-parser`, `pepl-types`, `pepl-codegen`
- [x] Configure shared dependencies: `thiserror`, `serde`, `serde_json`
- [x] Set up `pepl-types` crate with shared AST types and Span
- [x] Workspace-level `cargo build` succeeds

### 1.2 Error Infrastructure
- [x] Define `PeplError` type with structured error fields (code, message, line, column, end_line, end_column, severity, category, suggestion, source_line)
- [x] Define error code ranges: E100–E199 (syntax), E200–E299 (type), E300–E399 (invariant), E400–E499 (capability), E500–E599 (scope), E600–E699 (structure)
- [x] Implement JSON serialization for error output
- [x] Max 20 errors per compilation (fail-fast)
- [x] Unit tests for error formatting and serialization

### 1.3 Source Location Tracking
- [x] Define `Span` type (start_line, start_col, end_line, end_col)
- [x] Define `SourceFile` for tracking source text
- [x] Helper to extract source line from source text given a Span
- [x] Unit tests for span calculations

---

## Phase 2: Lexer

### 2.1 Token Types
- [x] Define `Token` enum covering all Phase 0 tokens: keywords (39 reserved), operators (17), literals (number, string, bool, nil), identifiers, punctuation, newlines, EOF
- [x] Define `TokenKind` and `TokenSpan` (token + source location)
- [x] Unit tests for Token type construction

### 2.2 Core Lexer
- [x] Implement lexer that converts PEPL source → token stream
- [x] Handle single-line comments (`//`) — strip during lexing, not in AST
- [x] Reject block comments (`/* */`) with error E603
- [x] Handle newline-separated statements (no semicolons)
- [x] Handle string literals with escape sequences (`\"`, `\\`, `\n`, `\t`, `\r`, `\$`)
- [x] Handle string interpolation (`${expr}`) — emit InterpolationStart/End tokens
- [x] Handle number literals (integer and decimal)
- [x] Handle all operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `?`, `??`, `...`
- [x] Distinguish keywords from identifiers (39 reserved words + module names)
- [x] Handle trailing commas
- [x] Error recovery: report up to 20 errors, don't stop at first
- [x] Produce error E100 for unexpected tokens

### 2.3 Lexer Tests
- [x] Test all 39 reserved keywords
- [x] Test all operator tokens
- [x] Test number literals (integer, decimal)
- [x] Test string literals (plain, escaped, interpolated)
- [x] Test comment stripping
- [x] Test block comment rejection (E603)
- [x] Test module name reservation (cannot shadow `math`, `core`, etc.)
- [x] Test newline handling
- [x] 100-iteration determinism test: same source → identical token stream × 100

---

## Phase 3: Parser

### 3.1 AST Type Definitions
- [x] Define `Program` node (SpaceDecl + TestsBlocks)
- [x] Define `SpaceDecl` and `SpaceBody` (enforced block ordering)
- [x] Define `TypeDecl` (sum types and type aliases)
- [x] Define `StateBlock` and `StateField`
- [x] Define `CapabilitiesBlock` (required + optional)
- [x] Define `CredentialsBlock` and `CredentialField`
- [x] Define `DerivedBlock` and `DerivedField`
- [x] Define `InvariantDecl`
- [x] Define `ActionDecl` with `ParamList` and `Block`
- [x] Define `ViewDecl` and `UIBlock` / `UIElement` / `ComponentExpr`
- [x] Define `UpdateDecl` and `HandleEventDecl`
- [x] Define `TestsBlock` and `TestCase` (with `WithResponses`)
- [x] Define all statement types: `SetStatement`, `LetBinding`, `ReturnStmt`, `AssertStmt`
- [x] Define all expression types: `OrExpr` through `PrimaryExpr` (full precedence chain)
- [x] Define `MatchExpr`, `MatchArm`, `Pattern`
- [x] Define `LambdaExpr` (block-body only)
- [x] Define `Type` enum (number, string, bool, nil, any, color, Surface, InputEvent, list<T>, record, Result<T,E>, function types, user-defined)
- [x] Define `RecordTypeField` with optional marker (`field?: Type` — omitted optional fields default to nil)
- [x] All AST nodes carry `Span` for error reporting

### 3.2 Recursive Descent Parser — Declarations
- [x] Parse `space Name { ... }` top-level structure
- [x] Enforce block ordering: types → state → capabilities → credentials → derived → invariants → actions → views → update → handleEvent (E600)
- [x] Parse `type` declarations (sum types with variants, type aliases)
- [x] Parse `state { ... }` block (require at least one field)
- [x] Parse `capabilities { required: [...], optional: [...] }`
- [x] Parse `credentials { ... }` block
- [x] Parse `derived { ... }` block
- [x] Parse `invariant name { expr }`
- [x] Parse `action name(params) { ... }`
- [x] Parse `view name(params) -> Surface { ... }`
- [x] Parse `update(dt: number) { ... }`
- [x] Parse `handleEvent(event: InputEvent) { ... }`
- [x] Parse `tests { ... }` block (outside space)
- [x] Parse `test "description" [with_responses { ... }] { ... }`

### 3.3 Recursive Descent Parser — Statements & Expressions
- [x] Parse `set field = expr` and `set record.field.nested = expr`
- [x] Parse `let name: Type = expr` and `let _ = expr` (discard binding)
- [x] Parse `return` (action-only early exit)
- [x] Parse `assert expr [, "message"]`
- [x] Parse `if expr { ... } [else { ... } | else if ...]`
- [x] Parse `for item [, index] in expr { ... }`
- [x] Parse `match expr { Pattern -> expr|block, ... }`
- [x] Parse full expression precedence: `or` → `and` → `??` → comparison → `+/-` → `*/%` → unary → postfix → primary
- [x] Parse postfix `?` (Result unwrap) and `.field` / `.method()` access
- [x] Parse function calls: `name(args)`
- [x] Parse list literals: `[expr, ...]`
- [x] Parse record literals: `{ field: expr, ...spread, ... }`
- [x] Parse lambda expressions: `fn(params) { ... }` (block-body only, reject expression-body E602)
- [x] Parse string interpolation expressions within `${...}`
- [x] Parse type annotations in all positions
- [x] Parse optional record type fields (`name?: Type` in record type annotations)
- [x] Reject comparison chaining (`a == b == c` → compile error)

### 3.4 UI Parsing
- [x] Parse `ComponentExpr`: `UpperName { props } [{ children }]`
- [x] Parse `PropAssign`: `name: expr [,]`
- [x] Distinguish action references from function calls in prop position
- [x] Parse `if`/`for` inside UI blocks as UIElements (not Statements)
- [x] Validate Phase 0 component names (Text, Button, TextInput, Column, Row, Scroll, ScrollList, ProgressBar, Modal, Toast) — unknown names produce E402

### 3.5 Parser Tests
- [x] Test all canonical examples from `llm-generation-contract.md` (Counter, TodoList, UnitConverter, WeatherDashboard, PomodoroTimer, HabitTracker, QuizApp)
- [x] Test all edge cases from `grammar-edge-cases.md`
- [x] Test all operator precedence examples
- [x] Test block ordering enforcement (E600)
- [x] Test error recovery (multiple errors reported)
- [x] Test structural limits (lambda depth ≤ 3, record depth ≤ 4, expression depth ≤ 16, for depth ≤ 3, params ≤ 8)
- [x] 100-iteration determinism test: same source → identical AST × 100

---

## Phase 4: Type Checker

### 4.1 Type Environment
- [x] Build type environment from state fields, action params, let bindings
- [x] Register built-in types: number, string, bool, nil, color, Surface, InputEvent
- [x] Register built-in parameterized types: list<T>, Result<T, E>
- [x] Register stdlib function signatures (all 100 Phase 0 functions + 2 constants)
- [x] Register user-defined sum types from `type` declarations
- [x] Track scope (space-level, action-level, block-level, lambda-level)

### 4.2 Expression Type Checking
- [x] Infer and check types of all expression forms
- [x] Validate operator type constraints (`+` is numbers-only, `not`/`and`/`or` are bool-only)
- [x] Check function call argument types and counts (E202)
- [x] Validate qualified calls (module.function): check module exists, function exists, arg types match
- [x] Check list element type consistency
- [x] Check record field types
- [x] Validate `match` exhaustiveness (E210: non-exhaustive match)
- [x] Validate pattern bindings in match arms
- [x] Resolve `Result<T, E>` variant bindings in match arms (`Ok(n)` and `Err(e)`)
- [x] Check `?` only on Result types
- [x] Check `??` left side is nullable
- [x] Implement nil narrowing: `if x != nil { ... }` narrows type from `T | nil` to `T`
- [x] Reject `any` in user-authored type annotations (E200)

### 4.3 Statement Type Checking
- [x] Validate `set` targets declared state fields only (E101)
- [x] Validate `set` appears only inside actions (E501)
- [x] Validate `set` does not target derived fields (E601)
- [x] Check `let` binding type annotation matches expression type
- [x] Validate `return` appears only inside actions
- [x] Validate `assert` expression is bool
- [x] Validate `for` iterates over `list<T>` only
- [x] Check no variable shadowing (E500)

### 4.4 Declaration-Level Checks
- [x] Validate state field initializers: pure stdlib only, no capability calls, no cross-field references
- [x] Validate derived field expressions: may reference state + prior derived fields, no later derived or circular refs
- [x] Validate invariant expressions are boolean, do not reference derived fields
- [x] Validate views are pure: no `set`, no capability calls (E501)
- [x] Validate capability usage matches declarations (E400, E401)
- [x] Validate credential references exist in credentials block (E604)
- [x] Validate credentials are read-only (E605)
- [x] Check action references in UI props resolve to declared actions

### 4.5 Type Checker Tests
- [x] Test type mismatch errors (E201)
- [x] Test unknown type errors (E200)
- [x] Test wrong argument count (E202)
- [x] Test non-exhaustive match (E210)
- [x] Test `set` outside action (E501)
- [x] Test capability not declared (E400)
- [x] Test variable already declared (E500)
- [x] Test derived field modification (E601)
- [x] Test block comment rejection (E603)
- [x] Test credential errors (E604, E605)
- [x] Test block ordering violation (E600)
- [x] Test invariant unreachable (E300) and unknown field reference (E301)
- [x] Test nil narrowing works correctly
- [x] Test all canonical examples type-check successfully
- [x] 100-iteration determinism test

---

## Phase 5: Invariant Checker

### 5.1 Structural Validation
- [x] Enforce lambda nesting depth ≤ 3 *(enforced in parser)*
- [x] Enforce record nesting depth ≤ 4 *(enforced in parser)*
- [x] Enforce expression depth ≤ 16 *(enforced in parser)*
- [x] Enforce `for` nesting depth ≤ 3 *(enforced in parser)*
- [x] Enforce parameter count ≤ 8 *(enforced in parser)*
- [x] Detect and reject recursion (E502) *(folded into type checker)*
- [x] Validate invariant expressions don't reference derived fields (E300) *(folded into type checker)*

### 5.2 Invariant Checker Tests
- [x] Test each structural limit with at-limit and over-limit cases *(covered in parser edge_case_tests)*
- [x] Test recursion detection (direct self-recursion, nested in if/for)
- [x] Test invariant referencing derived field → error (single, compound, multiple derived)
- [x] Test valid invariants (state-only, no derived, stdlib calls)
- [x] Test derived fields OK in actions and views
- [x] Test combined E300 + E502 in same program
- [x] 100-iteration determinism test

---

## Phase 6: Tree-Walking Evaluator (pepl-eval)

> Reference implementation. Executes PEPL programs directly from the typed AST.
> Validates all language semantics before tackling WASM codegen.
> Output becomes the golden reference for WASM output validation.

### 6.1 Evaluator Scaffolding
- [x] Create `pepl-eval` crate in workspace with dependencies on `pepl-types`, `pepl-parser`
- [x] Define `EvalValue` enum — reuses `pepl-stdlib::Value` directly (Number, String, Bool, Nil, List, Record, SumVariant, Function, Color, Result)
- [x] Define `EvalError` type (runtime traps, assertion failures, invariant violations, gas exhaustion)
- [x] Define `EvalResult<T>` type alias
- [x] Define `Environment` (scoped variable bindings with push/pop scope, global snapshot/restore)
- [x] Unit tests for evaluator construction and gas metering

### 6.2 State Management & Action Dispatch
- [x] Initialize state fields from default expressions (pure stdlib calls only)
- [x] Implement `set` statement execution (sequential — each `set` immediately visible)
- [x] Implement nested `set` desugaring: `set a.b.c = x` → immutable record update
- [x] Implement action dispatch by name with parameter binding
- [x] Implement atomic transactions: post-action invariant checking
- [x] Implement rollback on invariant failure (revert to pre-action state)
- [x] Implement `return` (early exit from action, prior `set` statements applied)
- [x] Unit tests for action atomicity and rollback
- [x] 100-iteration determinism test for action dispatch

### 6.3 Derived Field Recomputation
- [x] Recompute all derived fields after every committed action, in declaration order
- [x] Derived fields may reference state and previously declared derived fields
- [x] Unit tests for derived field evaluation order

### 6.4 Expression Evaluation
- [x] Evaluate all arithmetic operators (`+`, `-`, `*`, `/`, `%`)
- [x] Evaluate comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) with structural equality
- [x] Evaluate logical operators (`not`, `and`, `or`) with short-circuit
- [x] Evaluate `??` nil-coalescing
- [x] Evaluate `?` Result unwrap (trap on Err)
- [x] Evaluate `if`/`else` expressions
- [x] Evaluate `for` loops (list iteration with item + optional index)
- [x] Evaluate `match` expressions (pattern matching on sum types, wildcard)
- [x] Evaluate `let` bindings (immutable, no shadowing)
- [x] Evaluate `assert` statements (trap on false)
- [x] Evaluate function calls (stdlib dispatch via module.function)
- [x] Evaluate lambda expressions (capture environment, block-body only)
- [x] Evaluate list literals, record literals (including spread), string interpolation
- [x] Implement NaN prevention (division by zero → trap, sqrt of negative → trap)
- [x] Implement structural equality for records, lists, sum types (functions always false)
- [ ] Implement `any` type runtime checks on state assignment *(deferred to Phase 1 — F17)*
- [x] Implement nil access trap (`nil.field` → runtime trap)
- [x] Unit tests for all expression forms
- [x] 100-iteration determinism test for expression evaluation

### 6.5 Stdlib Integration
- [x] Route `module.function()` calls to `pepl-stdlib` implementations
- [x] Handle `core.log` (capture output for test assertions)
- [x] Handle `core.assert` (trap with message)
- [x] All 89 pure stdlib functions callable from evaluator
- [x] Unit tests for stdlib dispatch

### 6.6 View Rendering
- [x] Walk view function bodies to construct `Surface` trees
- [x] Evaluate `ComponentExpr` nodes (resolve props, build children)
- [x] Handle action references in props (deferred, not eagerly evaluated)
- [x] Handle `if`/`for` inside UI blocks (UIElements, not Statements)
- [x] Serialize Surface tree to JSON matching host-integration.md format
- [x] Unit tests for view rendering

### 6.7 Test Runner
- [x] Execute `tests { }` blocks with fresh state per test case
- [x] Dispatch actions by calling them as functions: `increment()`, `add_item("task")`
- [x] Handle `assert` with optional message
- [x] Implement `with_responses { }` — mock capability calls with predetermined Results
- [x] Unmocked capability calls return `Err("unmocked_call")`
- [x] Report test results (pass/fail with assertion context)
- [x] Unit tests for test runner

### 6.8 Capability Call Handling
- [x] Capability calls yield `Err("unmocked_call")` outside test `with_responses` context
- [x] Inside `with_responses`, match call site to recorded response and return Result
- [x] Handle all capability modules: http, storage, location, notifications
- [x] Handle credential resolution (read-only bindings)
- [x] Unit tests for capability mocking

### 6.9 Game Loop Support
- [x] Evaluate `update(dt: number)` with delta time parameter
- [x] Evaluate `handleEvent(event: InputEvent)` with event parameter
- [x] Gas metering: count loop iterations and function calls, trap on exhaustion
- [x] Unit tests for game loop evaluation

### 6.10 Golden Reference Generation
- [x] Execute all 7 canonical examples end-to-end in the evaluator
- [x] Capture state snapshots after each action dispatch
- [x] Capture Surface tree JSON after each render
- [x] Store as golden reference fixtures for WASM output validation
- [x] 100-iteration determinism test: same programs → identical output × 100

---

## Phase 7: WASM Code Generator

### 7.1 WASM Module Structure
- [x] Set up `wasm-encoder` crate dependency
- [x] Generate WASM module skeleton: types section, function section, table, memory, exports
- [x] Generate WASM imports: `env.host_call`, `env.log`, `env.trap`
- [x] Generate WASM exports: `init`, `dispatch_action`, `render`, `get_state`, `alloc`
- [x] Conditionally export `update` and `handle_event` (only if space declares them)
- [x] Embed PEPL compiler version in WASM custom section

### 7.2 State & Memory Management
- [x] Generate memory layout for state fields
- [x] Implement `alloc` export (bump allocator)
- [x] Generate `init` function (initialize state to defaults)
- [x] Generate `get_state` function (return state_ptr global)
- [x] Handle all PEPL types in WASM memory: number (f64), string, bool, nil, list, record, sum types

### 7.3 Expression Codegen
- [x] Generate WASM instructions for all arithmetic operators
- [x] Generate WASM instructions for comparison operators
- [x] Generate WASM instructions for logical operators (`not`, `and`, `or`)
- [x] Generate function calls (stdlib dispatch)
- [x] Generate qualified calls (module.function)
- [x] Generate list operations
- [x] Generate record operations (including spread)
- [x] Generate `match` expression (branch table)
- [x] Generate `if`/`else` expressions
- [x] Generate `for` loops
- [x] Generate string interpolation (lower to concat + to_string)
- [x] Generate `?` postfix (Result unwrap, trap on Err) *(completed in Phase 9.5)*
- [x] Generate `??` nil-coalescing
- [x] Generate lambda closures (placeholder — emits nil)
- [x] Generate structural equality for `==`/`!=` (deep record/list/sum comparison, functions always false)
- [ ] Generate `any` type runtime checks (validate actual value matches declared type on state assignment) *(deferred to Phase 1 — F17)*
- [x] NaN prevention: division and sqrt emit trap-on-NaN guards

### 7.4 Action & View Codegen
- [x] Generate `dispatch_action` function (action ID → handler dispatch)
- [x] Generate action bodies (sequential set execution)
- [x] Generate `set` with nested field desugaring: `set a.b.c = x` → immutable record update
- [x] Generate invariant checks (post-action validation, rollback on failure)
- [x] Generate derived field recomputation (after every committed action)
- [x] Generate `render` function (view → serialized Surface tree as records)
- [x] Generate UI component tree serialization
- [x] Generate action reference callbacks in UI props

### 7.5 Game Loop & Test Codegen
- [x] Generate `update(dt)` export
- [x] Generate `handle_event(event)` export
- [x] Generate capability call dispatch via `env.host_call`
- [x] Generate credential resolution via capability ID 5
- [ ] Generate capability call suspension/resume (yield to host via `host_call`, resume with Result) *(deferred to Phase 1 — F20)*
- [x] Generate test execution codegen (fresh state per test, action dispatch, assert checks) *(completed in Phase 11.3)*
- [ ] Generate `with_responses` mock capability dispatch for test blocks *(deferred — no WASM programs use capabilities yet)*

### 7.6 Gas Metering
- [x] Inject gas counter at `for` loop boundaries
- [x] Inject gas counter at function/action call sites
- [x] Inject gas counter at `update()` call boundaries
- [x] Gas exhaustion → WASM trap
- [x] Host-configurable gas limit (via import or module constant)

### 7.7 WASM Output Validation
- [x] Validate generated WASM with `wasmparser`
- [x] Test all canonical examples compile to valid WASM
- [x] Test gas metering is present at all injection points
- [x] Test NaN guards are emitted for division and sqrt
- [x] Test nested `set` desugaring produces correct WASM
- [x] 100-iteration determinism test: same source → identical .wasm bytes × 100

---

## Phase 8: Integration & Packaging

### 8.1 End-to-End Pipeline
- [x] Wire all stages: source → lexer → parser → type checker → invariant checker → evaluator (dev) / codegen (prod) → .wasm
- [x] Compile all 7 canonical examples end-to-end
- [x] Verify structured error JSON output for invalid inputs
- [x] Verify compilation < 500ms for small spaces (< 200 lines)
- [x] Verify compilation < 5s for large spaces (1000+ lines)
- [x] Verify action execution < 50ms for all canonical examples
- [x] Verify memory per space < 100KB for small spaces

### 8.2 WASM-Pack Build
- [x] Configure `wasm-pack` for browser target
- [x] Expose `compile(source: &str) -> CompileResult` as WASM export
- [x] `CompileResult` returns either `.wasm` bytes or structured error JSON
- [x] Verify compiler-as-WASM runs in browser Web Worker
- [x] Package size target: < 2MB for compiler WASM

### 8.3 Final Validation
- [x] All canonical examples: compile → instantiate → init → dispatch actions → render → verify output
- [x] Error code coverage: every E-code (E100–E699) has at least one test
- [x] Validate WASM import/export contract matches host-integration.md spec
- [x] Validate LLM Generation Contract examples compile and execute correctly
- [x] WASM output matches evaluator golden reference for all canonical examples
- [x] Full determinism proof: 100 iterations across full pipeline
- [x] `cargo clippy -- -D warnings` clean
- [x] `cargo fmt --check` clean
- [x] README.md complete with build instructions and architecture overview
---

## Phase 9: WASM Codegen Fixes (Phase 0A)

> Fix critical WASM codegen issues identified in the compliance audit (findings.md F1–F16).
> The WASM backend produces silently wrong results without these fixes.
> The evaluator (pepl-eval) handles all of these correctly — codegen must reach parity.

### 9.1 Lambda/Closure Codegen
- [x] Replace placeholder `nil` emission with proper function table + environment capture
- [x] Lambda body compiled as WASM function, added to function table
- [x] Captured variables stored in a closure record on the heap
- [x] Higher-order stdlib calls (`list.map`, `list.filter`, `list.sort`, `list.reduce`, `list.find`, `list.every`, `list.some`, `list.count`, `list.find_index`) receive function table index *(completed via lambda codegen in Phase 9.1)*
- [x] UI callback props (`on_tap`, `on_change`, `render`, `key`) receive function table index *(completed via action reference callbacks in Phase 7.4)*
- [x] Unit tests: lambda creation, capture, callback dispatch
- [x] 100-iteration determinism test

### 9.2 Record Spread Codegen
- [x] Implement field copying from spread source record into target record
- [x] `{ ...base, field: val }` copies all fields from `base`, then overwrites with explicit fields
- [x] Handle multiple spreads in one record literal
- [x] `field_count` accounts for spread fields
- [x] Unit tests: spread with override, spread-only, nested spread
- [x] 100-iteration determinism test

### 9.3 String Comparison Fix
- [x] Replace pointer comparison with byte-by-byte string content comparison for `==`/`!=`
- [x] Dynamically created strings with identical content must compare as equal
- [x] String comparison used by `list.contains`, `match`, record key lookup, etc.
- [x] Unit tests: interned vs dynamic strings, empty strings, unicode equality

### 9.4 Record Key Comparison Fix
- [x] Replace pointer comparison with byte-by-byte comparison in `record.get` / `record.set` / `record.has` key lookup
- [x] Dynamic key strings (from variables, interpolation) match against record field names
- [x] Unit tests: `record.get(r, key)` where `key` is a variable, interpolated, or literal

### 9.5 Result Unwrap (`?`) Codegen
- [x] Check variant tag: if `Ok`, unwrap and return inner value
- [x] If `Err`, trap with error message
- [x] Completes the unchecked item from Phase 7.3
- [x] Unit tests: `?` on Ok, `?` on Err (trap), chained `?` expressions

### 9.6 Nested Set Fix
- [x] 2-level `set a.b = x`: preserve all sibling fields of the inner record
- [x] 3+ level `set a.b.c = x`: preserve intermediate object structure at all levels
- [x] Implement immutable record update chain (read → clone → write → replace)
- [x] Unit tests: nested set preserving siblings at 2 and 3 levels

### 9.7 Stdlib Name Alignment
- [x] Remove `storage.remove` from type checker's stdlib registry (keep only `storage.delete`)
- [x] Align `list.any`/`list.some` naming — decide on spec name and rename in type checker + codegen dispatch
- [x] Add `list.drop` to type checker's stdlib registry
- [x] Document extra functions in Phase 0 stdlib reference: `list.insert`, `list.update`, `list.find_index`, `list.zip`, `list.flatten`, `list.some`
- [x] Coordinate with pepl-stdlib Phase 7 for implementation-side changes

### 9.8 Additional Codegen Fixes
- [x] Fix `to_string` for records and lists — emit proper debug representation instead of `"[value]"` placeholder
- [ ] Resolve keyword count discrepancy: update LLM Generation Contract or grammar.md to agree on reserved word count *(deferred to Phase 1 — F15)*
- [x] Unit tests for `to_string` output format

### 9.9 Phase 9 Validation
- [x] All canonical examples compile and produce correct WASM output
- [x] Lambda-using examples (TodoList, QuizApp with callbacks) produce correct results
- [x] Record spread examples produce correct results
- [x] `cargo test --workspace` — all tests pass (498 pass)
- [x] `cargo clippy -- -D warnings` clean (2 warnings only — unused_assignments in next_idx tracking)
- [x] 100-iteration determinism test across full pipeline

---

## Phase 10: Compiler Metadata & Host Integration (Phase 0B)

> Enrich compiler output so any host application can use the compiler.
> Fix remaining host integration contract divergences from the spec.

### 10.1 CompileResult Enrichment
- [x] Surface full AST in `CompileResult` (serializable to JSON)
- [x] Add source hash (SHA-256) to `CompileResult`
- [x] Add WASM hash (SHA-256) to `CompileResult`
- [x] Surface state field list (names + types) in `CompileResult`
- [x] Surface action list (names + parameter types) in `CompileResult`
- [x] Surface view list, declared capabilities, declared credentials in `CompileResult`
- [x] Add PEPL language version and compiler version fields
- [x] Add warnings list (empty for now, reserved for future)
- [x] Unit tests for all new CompileResult fields

### 10.2 Host Integration Fixes
- [x] Add `env.get_timestamp` as a dedicated WASM import (i64 return, host-controlled)
- [x] Add `dealloc` WASM export for host memory management
- [x] Align `dispatch_action` export signature with spec: `(action_id: i32, payload_ptr: i32, payload_len: i32) -> void`
- [x] Align `init` export signature with spec (resolve `gas_limit` parameter question)
- [x] Unit tests for all import/export signatures

### 10.3 Error System Improvements
- [x] Wire E301 (`INVARIANT_UNKNOWN_FIELD`) into checker
- [x] Wire E401 (`CAPABILITY_UNAVAILABLE`) into checker
- [x] Wire E600 (`BLOCK_ORDERING_VIOLATED`) into checker (parser handles it)
- [x] Wire E602 (`EXPRESSION_BODY_LAMBDA`) into checker (parser handles it)
- [x] Wire E603 (`BLOCK_COMMENT_USED`) into checker (lexer handles it)
- [x] Wire E604 (`UNDECLARED_CREDENTIAL`) into checker
- [x] Wire E606 (`EMPTY_STATE_BLOCK`) into checker
- [x] Wire E607 (`STRUCTURAL_LIMIT_EXCEEDED`) into checker (parser handles it)
- [x] Add structured `suggestion` field to all error messages
- [x] Unit tests for all newly wired error codes

### 10.4 Runtime Improvements
- [ ] Implement capability call suspension/resume in WASM (Asyncify transform, split-execution, or JS wrapper re-entry) *(deferred to Phase 1 — F20)*
- [x] Fix derived field recomputation in codegen — emit proper computed value instead of `NilLit` placeholder
- [x] Add `memory.grow` support to bump allocator (handle OOM by growing linear memory)
- [x] Make WASM validation (`wasmparser`) a mandatory step in the compile pipeline, not just a test
- [ ] Implement `any` type runtime checks on state assignment — evaluator + codegen (completes unchecked Phase 6.4 and 7.3 items) *(deferred to Phase 1 — F17)*

### 10.5 Phase 10 Validation
- [x] All canonical examples compile with enriched `CompileResult`
- [x] CompileResult JSON includes AST, hashes, state/action/view lists
- [x] Host integration signatures match `host-integration.md` spec
- [x] All 23 error codes have tests and at least one emitter
- [ ] Capability calls suspend/resume correctly in WASM *(deferred to Phase 1 — F20)*
- [x] `cargo test --workspace` — all tests pass
- [x] `cargo clippy -- -D warnings` clean
- [x] 100-iteration determinism test

---

## Phase 11: Incremental Compilation & Testing Infrastructure (Phase 0C)

> Build the infrastructure needed for incremental compilation, code transformations, and PEPL's test pipeline.

### 11.1 AST Diff Infrastructure
- [x] Define `AstDiff` type: list of `AstChange` (added, removed, modified nodes with paths)
- [x] Implement `ast_diff(old: &Ast, new: &Ast) -> AstDiff` — walk both ASTs in parallel
- [x] Implement `AstDiff` serialization (compact JSON format, ~0.5–5 KB per diff)
- [x] Implement scope validation: given an `AstDiff` and a set of allowed change scopes, accept/reject
- [x] Unit tests: identical ASTs (empty diff), single field change, action added, view modified

### 11.2 Determinism & Parity Infrastructure
- [x] Build determinism proof harness: compile space, run with known inputs, capture output, repeat, compare byte-for-byte
- [x] Build eval↔codegen parity test harness: run same PEPL programs through evaluator and WASM backend, compare state + view output
- [x] Integrate both harnesses into `cargo test` as integration tests
- [x] Run parity tests on all 7 canonical examples

### 11.3 Test Codegen
- [x] Compile PEPL `test` blocks to WASM (fresh state per test case, action dispatch, assert checks)
- [ ] Implement `with_responses` mock capability dispatch for test blocks in WASM (deferred — no WASM programs use capabilities yet)
- [x] Test execution in WASM sandbox with 5-second timeout per test
- [x] Unit tests: test block compilation, mock dispatch, timeout enforcement

### 11.4 Source Mapping
- [x] Generate source map during compilation (WASM function index → PEPL source line/column)
- [x] Embed source map in WASM custom section (`pepl_source_map`) and as `source_map` field in `CompileResult`
- [x] Unit tests: trap at known WASM offset resolves to correct PEPL source position

### 11.5 Phase 11 Validation
- [x] AST diff produces correct diffs for all canonical example mutations
- [x] Determinism proof passes for all canonical examples
- [x] Eval↔codegen parity holds for all canonical examples
- [x] Test blocks compile and execute correctly in WASM
- [x] Source maps resolve correctly for all trap types
- [x] `cargo test --workspace` — all tests pass (423 total)
- [x] `cargo clippy -- -D warnings` clean

---

## Phase 12: LLM-First Tooling (Phase 0D)

> Machine-generate PEPL reference material from the compiler's own type registry.
> Export it from `pepl-wasm` so any host can inject it into LLM prompts.

### 12.1 Reference Generation
- [x] Machine-generate compressed PEPL reference (~2K tokens) from `StdlibRegistry` — all types, keywords, stdlib functions
- [x] Machine-generate Phase 0 stdlib table from `StdlibRegistry` — function signatures, descriptions
- [x] Output matches the format specified in `llm-generation-contract.md`
- [x] Reference auto-updates when stdlib changes (generated, not hand-written)

### 12.2 WASM Exports
- [x] Export `get_reference() -> String` from `pepl-wasm` crate
- [x] Export `get_stdlib_table() -> String` from `pepl-wasm` crate
- [x] Unit tests: reference contains all 100 functions, stdlib table is valid JSON

### 12.3 Phase 12 Validation
- [x] Generated reference is ≤ 2K tokens (measured with tiktoken or equivalent)
- [x] Generated stdlib table matches `phase-0-stdlib-reference.md` content
- [x] `cargo test --workspace` — all tests pass (577 tests)
- [x] `cargo clippy -- -D warnings` clean

---

## Phase 13: Parser — Contextual Keywords & Error Recovery

> Allow keywords (e.g., `color`, `type`, `state`) to be used as record field names.
> Fix infinite-loop bug in record type parsing when a keyword appears as a field name.

### 13.1 Contextual `expect_field_name()`
- [x] Add `expect_field_name()` to `parser.rs` — accepts `Identifier` or any keyword token
- [x] Replace `expect_identifier()` with `expect_field_name()` in 5 field-name positions:
  - `parse_record_type_field()` in `parse_type.rs`
  - `parse_record_literal()` in `parse_expr.rs`
  - `parse_state_field()` in `parse_decl.rs`
  - `parse_derived_field()` in `parse_decl.rs`
  - `parse_set_stmt()` path segments (after `.`) in `parse_stmt.rs`

### 13.2 Error Recovery — Record Type Loop
- [x] Add `too_many_errors()` + `synchronize()` to record type while loop in `parse_type.rs` — prevents infinite loop on malformed input

### 13.3 Tests
- [x] Keyword-as-field-name tests: record types, record literals, set paths, derived fields
- [x] Error recovery test: malformed record type produces errors without hanging
- [x] Revert codegen test workaround (`shade` → `color`)
- [x] All existing tests still pass (588 tests)

### 13.4 Documentation
- [x] Update `grammar.md` — RecordField/RecordTypeField rules, Reserved Keywords section
- [x] Update `reference.md` — §3 Reserved Words contextual usage note
- [x] Update `language-structure.md` — keyword table contextual note
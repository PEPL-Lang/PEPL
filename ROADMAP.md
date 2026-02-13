# ROADMAP — pepl (Compiler)

> PEPL compiler: Lexer → Parser → Type Checker → Invariant Checker → Evaluator → WASM Codegen.
> Written in Rust, compiles to WASM (runs in browser Web Worker).
> See `ORCHESTRATION.md` for cross-repo sequencing.

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
- [x] Register stdlib function signatures (all 88 Phase 0 functions)
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
- [ ] Implement `any` type runtime checks on state assignment
- [x] Implement nil access trap (`nil.field` → runtime trap)
- [x] Unit tests for all expression forms
- [x] 100-iteration determinism test for expression evaluation

### 6.5 Stdlib Integration
- [x] Route `module.function()` calls to `pepl-stdlib` implementations
- [x] Handle `core.log` (capture output for test assertions)
- [x] Handle `core.assert` (trap with message)
- [x] All 73 pure stdlib functions callable from evaluator
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
- [ ] Generate `?` postfix (Result unwrap, trap on Err)
- [x] Generate `??` nil-coalescing
- [x] Generate lambda closures (placeholder — emits nil)
- [x] Generate structural equality for `==`/`!=` (deep record/list/sum comparison, functions always false)
- [ ] Generate `any` type runtime checks (validate actual value matches declared type on state assignment)
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
- [ ] Generate capability call suspension/resume (yield to host via `host_call`, resume with Result)
- [ ] Generate test execution codegen (fresh state per test, action dispatch, assert checks)
- [ ] Generate `with_responses` mock capability dispatch for test blocks

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
- [ ] Wire all stages: source → lexer → parser → type checker → invariant checker → evaluator (dev) / codegen (prod) → .wasm
- [ ] Compile all 7 canonical examples end-to-end
- [ ] Verify structured error JSON output for invalid inputs
- [ ] Verify compilation < 500ms for small spaces (< 200 lines)
- [ ] Verify compilation < 5s for large spaces (1000+ lines)
- [ ] Verify action execution < 50ms for all canonical examples
- [ ] Verify memory per space < 100KB for small spaces

### 8.2 WASM-Pack Build
- [ ] Configure `wasm-pack` for browser target
- [ ] Expose `compile(source: &str) -> CompileResult` as WASM export
- [ ] `CompileResult` returns either `.wasm` bytes or structured error JSON
- [ ] Verify compiler-as-WASM runs in browser Web Worker
- [ ] Package size target: < 2MB for compiler WASM

### 8.3 Final Validation
- [ ] All canonical examples: compile → instantiate → init → dispatch actions → render → verify output
- [ ] Error code coverage: every E-code (E100–E699) has at least one test
- [ ] Validate WASM import/export contract matches host-integration.md spec
- [ ] Validate LLM Generation Contract examples compile and execute correctly
- [ ] WASM output matches evaluator golden reference for all canonical examples
- [ ] Full determinism proof: 100 iterations across full pipeline
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] README.md complete with build instructions and architecture overview

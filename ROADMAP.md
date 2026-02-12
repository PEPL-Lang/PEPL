# ROADMAP — pepl (Compiler)

> PEPL compiler: Parser → Type Checker → Invariant Checker → WASM Codegen + Gas Metering.
> Written in Rust, compiles to WASM (runs in browser Web Worker).

---

## Phase 1: Project Scaffolding & Error Infrastructure

### 1.1 Cargo Workspace Setup
- [ ] Create Cargo workspace with crates: `pepl-compiler`, `pepl-lexer`, `pepl-parser`, `pepl-types`, `pepl-codegen`
- [ ] Configure shared dependencies: `thiserror`, `serde`, `serde_json`
- [ ] Set up `pepl-types` crate with shared AST types and Span
- [ ] Workspace-level `cargo build` succeeds

### 1.2 Error Infrastructure
- [ ] Define `PeplError` type with structured error fields (code, message, line, column, end_line, end_column, severity, category, suggestion, source_line)
- [ ] Define error code ranges: E100–E199 (syntax), E200–E299 (type), E300–E399 (invariant), E400–E499 (capability), E500–E599 (scope), E600–E699 (structure)
- [ ] Implement JSON serialization for error output
- [ ] Max 20 errors per compilation (fail-fast)
- [ ] Unit tests for error formatting and serialization

### 1.3 Source Location Tracking
- [ ] Define `Span` type (start_line, start_col, end_line, end_col)
- [ ] Define `SourceFile` for tracking source text
- [ ] Helper to extract source line from source text given a Span
- [ ] Unit tests for span calculations

---

## Phase 2: Lexer

### 2.1 Token Types
- [ ] Define `Token` enum covering all Phase 0 tokens: keywords (39 reserved), operators (17), literals (number, string, bool, nil), identifiers, punctuation, newlines, EOF
- [ ] Define `TokenKind` and `TokenSpan` (token + source location)
- [ ] Unit tests for Token type construction

### 2.2 Core Lexer
- [ ] Implement lexer that converts PEPL source → token stream
- [ ] Handle single-line comments (`//`) — strip during lexing, not in AST
- [ ] Reject block comments (`/* */`) with error E603
- [ ] Handle newline-separated statements (no semicolons)
- [ ] Handle string literals with escape sequences (`\"`, `\\`, `\n`, `\t`, `\r`, `\$`)
- [ ] Handle string interpolation (`${expr}`) — emit InterpolationStart/End tokens
- [ ] Handle number literals (integer and decimal)
- [ ] Handle all operators: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `?`, `??`, `...`
- [ ] Distinguish keywords from identifiers (39 reserved words + module names)
- [ ] Handle trailing commas
- [ ] Error recovery: report up to 20 errors, don't stop at first
- [ ] Produce error E100 for unexpected tokens

### 2.3 Lexer Tests
- [ ] Test all 39 reserved keywords
- [ ] Test all operator tokens
- [ ] Test number literals (integer, decimal)
- [ ] Test string literals (plain, escaped, interpolated)
- [ ] Test comment stripping
- [ ] Test block comment rejection (E603)
- [ ] Test module name reservation (cannot shadow `math`, `core`, etc.)
- [ ] Test newline handling
- [ ] 100-iteration determinism test: same source → identical token stream × 100

---

## Phase 3: Parser

### 3.1 AST Type Definitions
- [ ] Define `Program` node (SpaceDecl + TestsBlocks)
- [ ] Define `SpaceDecl` and `SpaceBody` (enforced block ordering)
- [ ] Define `TypeDecl` (sum types and type aliases)
- [ ] Define `StateBlock` and `StateField`
- [ ] Define `CapabilitiesBlock` (required + optional)
- [ ] Define `CredentialsBlock` and `CredentialField`
- [ ] Define `DerivedBlock` and `DerivedField`
- [ ] Define `InvariantDecl`
- [ ] Define `ActionDecl` with `ParamList` and `Block`
- [ ] Define `ViewDecl` and `UIBlock` / `UIElement` / `ComponentExpr`
- [ ] Define `UpdateDecl` and `HandleEventDecl`
- [ ] Define `TestsBlock` and `TestCase` (with `WithResponses`)
- [ ] Define all statement types: `SetStatement`, `LetBinding`, `ReturnStmt`, `AssertStmt`
- [ ] Define all expression types: `OrExpr` through `PrimaryExpr` (full precedence chain)
- [ ] Define `MatchExpr`, `MatchArm`, `Pattern`
- [ ] Define `LambdaExpr` (block-body only)
- [ ] Define `Type` enum (number, string, bool, nil, any, color, Surface, InputEvent, list<T>, record, Result<T,E>, function types, user-defined)
- [ ] All AST nodes carry `Span` for error reporting

### 3.2 Recursive Descent Parser — Declarations
- [ ] Parse `space Name { ... }` top-level structure
- [ ] Enforce block ordering: types → state → capabilities → credentials → derived → invariants → actions → views → update → handleEvent (E600)
- [ ] Parse `type` declarations (sum types with variants, type aliases)
- [ ] Parse `state { ... }` block (require at least one field)
- [ ] Parse `capabilities { required: [...], optional: [...] }`
- [ ] Parse `credentials { ... }` block
- [ ] Parse `derived { ... }` block
- [ ] Parse `invariant name { expr }`
- [ ] Parse `action name(params) { ... }`
- [ ] Parse `view name(params) -> Surface { ... }`
- [ ] Parse `update(dt: number) { ... }`
- [ ] Parse `handleEvent(event: InputEvent) { ... }`
- [ ] Parse `tests { ... }` block (outside space)
- [ ] Parse `test "description" [with_responses { ... }] { ... }`

### 3.3 Recursive Descent Parser — Statements & Expressions
- [ ] Parse `set field = expr` and `set record.field.nested = expr`
- [ ] Parse `let name: Type = expr` and `let _ = expr` (discard binding)
- [ ] Parse `return` (action-only early exit)
- [ ] Parse `assert expr [, "message"]`
- [ ] Parse `if expr { ... } [else { ... } | else if ...]`
- [ ] Parse `for item [, index] in expr { ... }`
- [ ] Parse `match expr { Pattern -> expr|block, ... }`
- [ ] Parse full expression precedence: `or` → `and` → `??` → comparison → `+/-` → `*/%` → unary → postfix → primary
- [ ] Parse postfix `?` (Result unwrap) and `.field` / `.method()` access
- [ ] Parse function calls: `name(args)`
- [ ] Parse list literals: `[expr, ...]`
- [ ] Parse record literals: `{ field: expr, ...spread, ... }`
- [ ] Parse lambda expressions: `fn(params) { ... }` (block-body only, reject expression-body E602)
- [ ] Parse string interpolation expressions within `${...}`
- [ ] Parse type annotations in all positions
- [ ] Reject comparison chaining (`a == b == c` → compile error)

### 3.4 UI Parsing
- [ ] Parse `ComponentExpr`: `UpperName { props } [{ children }]`
- [ ] Parse `PropAssign`: `name: expr [,]`
- [ ] Distinguish action references from function calls in prop position
- [ ] Parse `if`/`for` inside UI blocks as UIElements (not Statements)
- [ ] Validate Phase 0 component names (Text, Button, TextInput, Column, Row, Scroll, ScrollList, ProgressBar, Modal, Toast) — unknown names produce E402

### 3.5 Parser Tests
- [ ] Test all canonical examples from `llm-generation-contract.md` (Counter, TodoList, UnitConverter, WeatherDashboard, PomodoroTimer, HabitTracker, QuizApp)
- [ ] Test all edge cases from `grammar-edge-cases.md`
- [ ] Test all operator precedence examples
- [ ] Test block ordering enforcement (E600)
- [ ] Test error recovery (multiple errors reported)
- [ ] Test structural limits (lambda depth ≤ 3, record depth ≤ 4, expression depth ≤ 16, for depth ≤ 3, params ≤ 8)
- [ ] 100-iteration determinism test: same source → identical AST × 100

---

## Phase 4: Type Checker

### 4.1 Type Environment
- [ ] Build type environment from state fields, action params, let bindings
- [ ] Register built-in types: number, string, bool, nil, color, Surface, InputEvent
- [ ] Register built-in parameterized types: list<T>, Result<T, E>
- [ ] Register stdlib function signatures (all 88 Phase 0 functions)
- [ ] Register user-defined sum types from `type` declarations
- [ ] Track scope (space-level, action-level, block-level, lambda-level)

### 4.2 Expression Type Checking
- [ ] Infer and check types of all expression forms
- [ ] Validate operator type constraints (`+` is numbers-only, `not`/`and`/`or` are bool-only)
- [ ] Check function call argument types and counts (E202)
- [ ] Validate qualified calls (module.function): check module exists, function exists, arg types match
- [ ] Check list element type consistency
- [ ] Check record field types
- [ ] Validate `match` exhaustiveness (E210: non-exhaustive match)
- [ ] Validate pattern bindings in match arms
- [ ] Check `?` only on Result types
- [ ] Check `??` left side is nullable
- [ ] Implement nil narrowing: `if x != nil { ... }` narrows type from `T | nil` to `T`
- [ ] Reject `any` in user-authored type annotations (E200)

### 4.3 Statement Type Checking
- [ ] Validate `set` targets declared state fields only (E101)
- [ ] Validate `set` appears only inside actions (E501)
- [ ] Validate `set` does not target derived fields (E601)
- [ ] Check `let` binding type annotation matches expression type
- [ ] Validate `return` appears only inside actions
- [ ] Validate `assert` expression is bool
- [ ] Validate `for` iterates over `list<T>` only
- [ ] Check no variable shadowing (E500)

### 4.4 Declaration-Level Checks
- [ ] Validate state field initializers: pure stdlib only, no capability calls, no cross-field references
- [ ] Validate derived field expressions: may reference state + prior derived fields, no later derived or circular refs
- [ ] Validate invariant expressions are boolean, do not reference derived fields
- [ ] Validate views are pure: no `set`, no capability calls (E501)
- [ ] Validate capability usage matches declarations (E400, E401)
- [ ] Validate credential references exist in credentials block (E604)
- [ ] Validate credentials are read-only (E605)
- [ ] Check action references in UI props resolve to declared actions

### 4.5 Type Checker Tests
- [ ] Test type mismatch errors (E201)
- [ ] Test unknown type errors (E200)
- [ ] Test wrong argument count (E202)
- [ ] Test non-exhaustive match (E210)
- [ ] Test `set` outside action (E501)
- [ ] Test capability not declared (E400)
- [ ] Test variable already declared (E500)
- [ ] Test derived field modification (E601)
- [ ] Test block comment rejection (E603)
- [ ] Test credential errors (E604, E605)
- [ ] Test block ordering violation (E600)
- [ ] Test nil narrowing works correctly
- [ ] Test all canonical examples type-check successfully
- [ ] 100-iteration determinism test

---

## Phase 5: Invariant Checker

### 5.1 Structural Validation
- [ ] Enforce lambda nesting depth ≤ 3
- [ ] Enforce record nesting depth ≤ 4
- [ ] Enforce expression depth ≤ 16
- [ ] Enforce `for` nesting depth ≤ 3
- [ ] Enforce parameter count ≤ 8
- [ ] Detect and reject recursion (E502)
- [ ] Validate invariant expressions don't reference derived fields

### 5.2 Invariant Checker Tests
- [ ] Test each structural limit with at-limit and over-limit cases
- [ ] Test recursion detection
- [ ] Test invariant referencing derived field → error
- [ ] 100-iteration determinism test

---

## Phase 6: WASM Code Generator

### 6.1 WASM Module Structure
- [ ] Set up `wasm-encoder` crate dependency
- [ ] Generate WASM module skeleton: types section, function section, table, memory, exports
- [ ] Generate WASM imports: `env.host_call`, `env.get_timestamp`, `env.log`, `env.trap`
- [ ] Generate WASM exports: `init`, `dispatch_action`, `render`, `get_state`, `alloc`, `dealloc`
- [ ] Conditionally export `update` and `handle_event` (only if space declares them)

### 6.2 State & Memory Management
- [ ] Generate memory layout for state fields
- [ ] Implement `alloc` / `dealloc` exports
- [ ] Generate `init` function (initialize state to defaults)
- [ ] Generate `get_state` function (serialize state to JSON)
- [ ] Handle all PEPL types in WASM memory: number (f64), string, bool, nil, list, record, sum types

### 6.3 Expression Codegen
- [ ] Generate WASM instructions for all arithmetic operators
- [ ] Generate WASM instructions for comparison operators
- [ ] Generate WASM instructions for logical operators (`not`, `and`, `or`)
- [ ] Generate function calls (stdlib dispatch)
- [ ] Generate qualified calls (module.function)
- [ ] Generate list operations
- [ ] Generate record operations (including spread)
- [ ] Generate `match` expression (branch table)
- [ ] Generate `if`/`else` expressions
- [ ] Generate `for` loops
- [ ] Generate string interpolation (lower to concat + to_string)
- [ ] Generate `?` postfix (Result unwrap, trap on Err)
- [ ] Generate `??` nil-coalescing
- [ ] Generate lambda closures
- [ ] NaN prevention: division and sqrt emit trap-on-NaN guards

### 6.4 Action & View Codegen
- [ ] Generate `dispatch_action` function (action ID → handler dispatch)
- [ ] Generate action bodies (sequential set execution)
- [ ] Generate `set` with nested field desugaring: `set a.b.c = x` → immutable record update
- [ ] Generate invariant checks (post-action validation, rollback on failure)
- [ ] Generate derived field recomputation (after every committed action)
- [ ] Generate `render` function (view → serialized JSON Surface tree)
- [ ] Generate UI component tree serialization
- [ ] Generate action reference callbacks in UI props

### 6.5 Game Loop & Test Codegen
- [ ] Generate `update(dt)` export
- [ ] Generate `handle_event(event)` export
- [ ] Generate capability call dispatch via `env.host_call`
- [ ] Generate credential resolution via capability ID 5

### 6.6 Gas Metering
- [ ] Inject gas counter at `for` loop boundaries
- [ ] Inject gas counter at function/action call sites
- [ ] Inject gas counter at `update()` call boundaries
- [ ] Gas exhaustion → WASM trap
- [ ] Host-configurable gas limit (via import or module constant)

### 6.7 WASM Output Validation
- [ ] Validate generated WASM with `wasmparser`
- [ ] Test all canonical examples compile to valid WASM
- [ ] Test gas metering is present at all injection points
- [ ] Test NaN guards are emitted for division and sqrt
- [ ] Test nested `set` desugaring produces correct WASM
- [ ] 100-iteration determinism test: same source → identical .wasm bytes × 100

---

## Phase 7: Integration & Packaging

### 7.1 End-to-End Pipeline
- [ ] Wire all stages: source → lexer → parser → type checker → invariant checker → codegen → .wasm
- [ ] Compile all 7 canonical examples end-to-end
- [ ] Verify structured error JSON output for invalid inputs
- [ ] Verify compilation < 2s for typical spaces (< 200 lines)
- [ ] Verify compilation < 5s for large spaces (1000+ lines)

### 7.2 WASM-Pack Build
- [ ] Configure `wasm-pack` for browser target
- [ ] Expose `compile(source: &str) -> CompileResult` as WASM export
- [ ] `CompileResult` returns either `.wasm` bytes or structured error JSON
- [ ] Verify compiler-as-WASM runs in browser Web Worker
- [ ] Package size target: < 2MB for compiler WASM

### 7.3 Final Validation
- [ ] All canonical examples: compile → instantiate → init → render → verify output
- [ ] Error code coverage: every E-code (E100–E699) has at least one test
- [ ] Full determinism proof: 100 iterations across full pipeline
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] README.md complete with build instructions and architecture overview

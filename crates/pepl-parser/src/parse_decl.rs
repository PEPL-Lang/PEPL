//! Top-level and declaration parsing.
//!
//! Handles the `space` declaration and all its inner blocks
//! (types, state, capabilities, credentials, derived, invariants,
//!  actions, views, update, handleEvent), plus `tests` blocks.

use pepl_lexer::token::TokenKind;
use pepl_types::ast::*;
use pepl_types::ErrorCode;

use crate::parser::Parser;

/// Block ordering index for E600 enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BlockOrder {
    Type = 0,
    State = 1,
    Capabilities = 2,
    Credentials = 3,
    Derived = 4,
    Invariant = 5,
    Action = 6,
    View = 7,
    Update = 8,
    HandleEvent = 9,
}

impl BlockOrder {
    fn label(self) -> &'static str {
        match self {
            BlockOrder::Type => "type",
            BlockOrder::State => "state",
            BlockOrder::Capabilities => "capabilities",
            BlockOrder::Credentials => "credentials",
            BlockOrder::Derived => "derived",
            BlockOrder::Invariant => "invariant",
            BlockOrder::Action => "action",
            BlockOrder::View => "view",
            BlockOrder::Update => "update",
            BlockOrder::HandleEvent => "handleEvent",
        }
    }
}

impl<'src> Parser<'src> {
    // ══════════════════════════════════════════════════════════════════════════
    // Program
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse a complete program: `SpaceDecl { TestsBlock }`.
    pub(crate) fn parse_program(&mut self) -> Option<Program> {
        let start = self.current_span();
        self.skip_newlines();
        let space = self.parse_space_decl()?;
        self.skip_newlines();

        let mut tests = Vec::new();
        while self.check_exact(&TokenKind::Tests) {
            if self.too_many_errors() {
                break;
            }
            if let Some(tb) = self.parse_tests_block() {
                tests.push(tb);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }

        if !self.at_end() {
            self.error_at_current(
                ErrorCode::UNEXPECTED_TOKEN,
                format!(
                    "expected end of file or 'tests' block, got '{}'",
                    self.peek_kind()
                ),
            );
        }

        let span = start.merge(self.previous_span());
        Some(Program { space, tests, span })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Space Declaration
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `space Name { body }`.
    fn parse_space_decl(&mut self) -> Option<SpaceDecl> {
        let start = self.current_span();
        self.expect(&TokenKind::Space)?;
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let body = self.parse_space_body()?;
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(SpaceDecl { name, body, span })
    }

    /// Parse the body of a space declaration with block-ordering enforcement.
    fn parse_space_body(&mut self) -> Option<SpaceBody> {
        let start = self.current_span();
        let mut last_order: Option<BlockOrder> = None;

        let mut types = Vec::new();
        let mut state: Option<StateBlock> = None;
        let mut capabilities: Option<CapabilitiesBlock> = None;
        let mut credentials: Option<CredentialsBlock> = None;
        let mut derived: Option<DerivedBlock> = None;
        let mut invariants = Vec::new();
        let mut actions = Vec::new();
        let mut views = Vec::new();
        let mut update: Option<UpdateDecl> = None;
        let mut handle_event: Option<HandleEventDecl> = None;

        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            self.skip_newlines();
            if self.check_exact(&TokenKind::RBrace) || self.at_end() {
                break;
            }

            let current_order = match self.peek_kind() {
                TokenKind::Type => BlockOrder::Type,
                TokenKind::State => BlockOrder::State,
                TokenKind::Capabilities => BlockOrder::Capabilities,
                TokenKind::Credentials => BlockOrder::Credentials,
                TokenKind::Derived => BlockOrder::Derived,
                TokenKind::Invariant => BlockOrder::Invariant,
                TokenKind::Action => BlockOrder::Action,
                TokenKind::View => BlockOrder::View,
                TokenKind::Update => BlockOrder::Update,
                TokenKind::HandleEvent => BlockOrder::HandleEvent,
                other => {
                    self.error_at_current(
                        ErrorCode::UNEXPECTED_TOKEN,
                        format!(
                            "expected space block declaration (type, state, action, view, ...), got '{}'",
                            other
                        ),
                    );
                    self.synchronize();
                    continue;
                }
            };

            // Block-ordering enforcement (E600)
            if let Some(prev) = last_order {
                if (current_order as u8) < (prev as u8) {
                    self.error_at_current(
                        ErrorCode::BLOCK_ORDERING_VIOLATED,
                        format!(
                            "'{}' block must appear before '{}' block (enforced order: type → state → capabilities → credentials → derived → invariant → action → view → update → handleEvent)",
                            current_order.label(),
                            prev.label(),
                        ),
                    );
                }
            }
            last_order = Some(current_order);

            match current_order {
                BlockOrder::Type => {
                    if let Some(td) = self.parse_type_decl() {
                        types.push(td);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::State => {
                    if state.is_some() {
                        self.error_at_current(
                            ErrorCode::UNEXPECTED_TOKEN,
                            "duplicate 'state' block",
                        );
                        self.synchronize();
                    } else if let Some(s) = self.parse_state_block() {
                        state = Some(s);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::Capabilities => {
                    if capabilities.is_some() {
                        self.error_at_current(
                            ErrorCode::UNEXPECTED_TOKEN,
                            "duplicate 'capabilities' block",
                        );
                        self.synchronize();
                    } else if let Some(c) = self.parse_capabilities_block() {
                        capabilities = Some(c);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::Credentials => {
                    if credentials.is_some() {
                        self.error_at_current(
                            ErrorCode::UNEXPECTED_TOKEN,
                            "duplicate 'credentials' block",
                        );
                        self.synchronize();
                    } else if let Some(c) = self.parse_credentials_block() {
                        credentials = Some(c);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::Derived => {
                    if derived.is_some() {
                        self.error_at_current(
                            ErrorCode::UNEXPECTED_TOKEN,
                            "duplicate 'derived' block",
                        );
                        self.synchronize();
                    } else if let Some(d) = self.parse_derived_block() {
                        derived = Some(d);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::Invariant => {
                    if let Some(inv) = self.parse_invariant_decl() {
                        invariants.push(inv);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::Action => {
                    if let Some(a) = self.parse_action_decl() {
                        actions.push(a);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::View => {
                    if let Some(v) = self.parse_view_decl() {
                        views.push(v);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::Update => {
                    if update.is_some() {
                        self.error_at_current(
                            ErrorCode::UNEXPECTED_TOKEN,
                            "duplicate 'update' block",
                        );
                        self.synchronize();
                    } else if let Some(u) = self.parse_update_decl() {
                        update = Some(u);
                    } else {
                        self.synchronize();
                    }
                }
                BlockOrder::HandleEvent => {
                    if handle_event.is_some() {
                        self.error_at_current(
                            ErrorCode::UNEXPECTED_TOKEN,
                            "duplicate 'handleEvent' block",
                        );
                        self.synchronize();
                    } else if let Some(h) = self.parse_handle_event_decl() {
                        handle_event = Some(h);
                    } else {
                        self.synchronize();
                    }
                }
            }
            self.skip_newlines();
        }

        // State block is required
        let state = match state {
            Some(s) => s,
            None => {
                self.error_at_current(
                    ErrorCode::UNEXPECTED_TOKEN,
                    "missing required 'state' block in space declaration",
                );
                StateBlock {
                    fields: Vec::new(),
                    span: self.current_span(),
                }
            }
        };

        let span = start.merge(self.current_span());
        Some(SpaceBody {
            types,
            state,
            capabilities,
            credentials,
            derived,
            invariants,
            actions,
            views,
            update,
            handle_event,
            span,
        })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Type Declarations
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `type Name = TypeBody`
    fn parse_type_decl(&mut self) -> Option<TypeDecl> {
        let start = self.current_span();
        self.advance(); // eat `type`
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Eq)?;
        self.skip_newlines(); // Allow newline between `=` and `|` for multiline sum types

        let body = if self.check_exact(&TokenKind::Pipe) {
            // Sum type: `| Variant1 | Variant2(...)`
            let mut variants = Vec::new();
            while self.eat(&TokenKind::Pipe) {
                self.skip_newlines();
                let variant = self.parse_variant_def()?;
                variants.push(variant);
                self.skip_newlines();
            }
            TypeDeclBody::SumType(variants)
        } else {
            // Type alias: `type Meters = number`
            let type_ann = self.parse_type_annotation()?;
            TypeDeclBody::Alias(type_ann)
        };

        let span = start.merge(self.previous_span());
        // For aliases, expect a newline terminator.
        // For sum types, the trailing newline was already consumed by skip_newlines.
        if matches!(body, TypeDeclBody::Alias(_)) {
            self.expect_newline_or_eof();
        }
        Some(TypeDecl { name, body, span })
    }

    /// Parse a variant definition: `Name` or `Name(param1: type, ...)`
    fn parse_variant_def(&mut self) -> Option<VariantDef> {
        let start = self.current_span();
        let name = self.expect_identifier()?;
        let params = if self.eat(&TokenKind::LParen) {
            let params = self.parse_param_list()?;
            self.expect(&TokenKind::RParen)?;
            params
        } else {
            Vec::new()
        };
        let span = start.merge(self.previous_span());
        Some(VariantDef { name, params, span })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // State Block
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `state { fields... }`
    fn parse_state_block(&mut self) -> Option<StateBlock> {
        let start = self.current_span();
        self.advance(); // eat `state`
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(field) = self.parse_state_field() {
                fields.push(field);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());

        // Empty state block is an error (E606)
        if fields.is_empty() {
            self.error_at(
                ErrorCode::EMPTY_STATE_BLOCK,
                "state block must have at least one field",
                span,
            );
        }

        Some(StateBlock { fields, span })
    }

    /// Parse `name: type = expr`
    fn parse_state_field(&mut self) -> Option<StateField> {
        let start = self.current_span();
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(&TokenKind::Eq)?;
        let default = self.parse_expression()?;
        let span = start.merge(self.previous_span());
        self.expect_newline_or_eof();
        Some(StateField {
            name,
            type_ann,
            default,
            span,
        })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Capabilities Block
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `capabilities { required: [...], optional: [...] }`
    fn parse_capabilities_block(&mut self) -> Option<CapabilitiesBlock> {
        let start = self.current_span();
        self.advance(); // eat `capabilities`
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut required = Vec::new();
        let mut optional = Vec::new();

        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            self.skip_newlines();
            if self.check_exact(&TokenKind::RBrace) {
                break;
            }
            match self.peek_kind() {
                TokenKind::Required => {
                    self.advance();
                    self.expect(&TokenKind::Colon)?;
                    required = self.parse_ident_list()?;
                    self.skip_newlines();
                }
                TokenKind::Optional => {
                    self.advance();
                    self.expect(&TokenKind::Colon)?;
                    optional = self.parse_ident_list()?;
                    self.skip_newlines();
                }
                _ => {
                    self.error_at_current(
                        ErrorCode::UNEXPECTED_TOKEN,
                        format!(
                            "expected 'required' or 'optional' in capabilities block, got '{}'",
                            self.peek_kind()
                        ),
                    );
                    self.synchronize();
                }
            }
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(CapabilitiesBlock {
            required,
            optional,
            span,
        })
    }

    /// Parse `[ident, ident, ...]` — a bracketed list of identifiers.
    fn parse_ident_list(&mut self) -> Option<Vec<Ident>> {
        self.expect(&TokenKind::LBracket)?;
        self.skip_newlines();
        let mut items = Vec::new();
        while !self.check_exact(&TokenKind::RBracket) && !self.at_end() {
            self.skip_newlines();
            // Identifiers here may be capability module names which are keywords
            let ident = self.expect_ident_or_module_name()?;
            items.push(ident);
            self.eat_comma();
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBracket)?;
        Some(items)
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Credentials Block
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `credentials { name: type, ... }`
    fn parse_credentials_block(&mut self) -> Option<CredentialsBlock> {
        let start = self.current_span();
        self.advance(); // eat `credentials`
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            let field_start = self.current_span();
            let name = self.expect_identifier()?;
            self.expect(&TokenKind::Colon)?;
            let type_ann = self.parse_type_annotation()?;
            let field_span = field_start.merge(self.previous_span());
            fields.push(CredentialField {
                name,
                type_ann,
                span: field_span,
            });
            self.expect_newline_or_eof();
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(CredentialsBlock { fields, span })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Derived Block
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `derived { name: type = expr, ... }`
    fn parse_derived_block(&mut self) -> Option<DerivedBlock> {
        let start = self.current_span();
        self.advance(); // eat `derived`
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut fields = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(field) = self.parse_derived_field() {
                fields.push(field);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(DerivedBlock { fields, span })
    }

    /// Parse `name: type = expr`
    fn parse_derived_field(&mut self) -> Option<DerivedField> {
        let start = self.current_span();
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Colon)?;
        let type_ann = self.parse_type_annotation()?;
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expression()?;
        let span = start.merge(self.previous_span());
        self.expect_newline_or_eof();
        Some(DerivedField {
            name,
            type_ann,
            value,
            span,
        })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Invariants
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `invariant name { expr }`
    fn parse_invariant_decl(&mut self) -> Option<InvariantDecl> {
        let start = self.current_span();
        self.advance(); // eat `invariant`
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let condition = self.parse_expression()?;
        self.skip_newlines();
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(InvariantDecl {
            name,
            condition,
            span,
        })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Actions
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `action name(params) { body }`
    fn parse_action_decl(&mut self) -> Option<ActionDecl> {
        let start = self.current_span();
        self.advance(); // eat `action`
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;

        // Structural limit: max 8 params per action
        if params.len() > 8 {
            self.error_at_current(
                ErrorCode::STRUCTURAL_LIMIT_EXCEEDED,
                format!("maximum 8 parameters per action, got {}", params.len()),
            );
        }

        self.expect(&TokenKind::RParen)?;
        let body = self.parse_block()?;
        let span = start.merge(self.previous_span());
        Some(ActionDecl {
            name,
            params,
            body,
            span,
        })
    }

    /// Parse a comma-separated parameter list: `name: type, ...`
    pub(crate) fn parse_param_list(&mut self) -> Option<Vec<Param>> {
        let mut params = Vec::new();
        self.skip_newlines();
        // Handle empty param list
        if self.check_exact(&TokenKind::RParen) {
            return Some(params);
        }
        loop {
            self.skip_newlines();
            let param_start = self.current_span();
            let name = self.expect_identifier()?;
            self.expect(&TokenKind::Colon)?;
            let type_ann = self.parse_type_annotation()?;
            let param_span = param_start.merge(self.previous_span());
            params.push(Param {
                name,
                type_ann,
                span: param_span,
            });
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
            // Trailing comma
            if self.check_exact(&TokenKind::RParen) {
                break;
            }
        }
        Some(params)
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Views
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `view name(params) -> Surface { ui_elements... }`
    fn parse_view_decl(&mut self) -> Option<ViewDecl> {
        let start = self.current_span();
        self.advance(); // eat `view`
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::Arrow)?;
        self.expect(&TokenKind::KwSurface)?;
        let body = self.parse_ui_block()?;
        let span = start.merge(self.previous_span());
        Some(ViewDecl {
            name,
            params,
            body,
            span,
        })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Game Loop
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `update(dt: number) { body }`
    fn parse_update_decl(&mut self) -> Option<UpdateDecl> {
        let start = self.current_span();
        self.advance(); // eat `update`
        self.expect(&TokenKind::LParen)?;
        let param_start = self.current_span();
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Colon)?;
        self.expect(&TokenKind::KwNumber)?;
        let param_span = param_start.merge(self.previous_span());
        let param = Param {
            name,
            type_ann: TypeAnnotation::new(TypeKind::Number, param_span),
            span: param_span,
        };
        self.expect(&TokenKind::RParen)?;
        let body = self.parse_block()?;
        let span = start.merge(self.previous_span());
        Some(UpdateDecl { param, body, span })
    }

    /// Parse `handleEvent(event: InputEvent) { body }`
    fn parse_handle_event_decl(&mut self) -> Option<HandleEventDecl> {
        let start = self.current_span();
        self.advance(); // eat `handleEvent`
        self.expect(&TokenKind::LParen)?;
        let param_start = self.current_span();
        let name = self.expect_identifier()?;
        self.expect(&TokenKind::Colon)?;
        self.expect(&TokenKind::KwInputEvent)?;
        let param_span = param_start.merge(self.previous_span());
        let param = Param {
            name,
            type_ann: TypeAnnotation::new(TypeKind::InputEvent, param_span),
            span: param_span,
        };
        self.expect(&TokenKind::RParen)?;
        let body = self.parse_block()?;
        let span = start.merge(self.previous_span());
        Some(HandleEventDecl { param, body, span })
    }

    // ══════════════════════════════════════════════════════════════════════════
    // Tests
    // ══════════════════════════════════════════════════════════════════════════

    /// Parse `tests { test_cases... }`
    fn parse_tests_block(&mut self) -> Option<TestsBlock> {
        let start = self.current_span();
        self.advance(); // eat `tests`
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut cases = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(tc) = self.parse_test_case() {
                cases.push(tc);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(TestsBlock { cases, span })
    }

    /// Parse `test "description" [with_responses { ... }] { body }`
    fn parse_test_case(&mut self) -> Option<TestCase> {
        let start = self.current_span();
        self.expect(&TokenKind::Test)?;
        let description = self.expect_string_literal()?;

        // Optional `with_responses { ... }`
        // `with_responses` is NOT a keyword — it's Identifier("with_responses")
        let with_responses = if matches!(
            self.peek_kind(),
            TokenKind::Identifier(ref name) if name == "with_responses"
        ) {
            Some(self.parse_with_responses()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let span = start.merge(self.previous_span());
        Some(TestCase {
            description,
            with_responses,
            body,
            span,
        })
    }

    /// Parse `with_responses { module.function(args) -> value, ... }`
    fn parse_with_responses(&mut self) -> Option<WithResponses> {
        let start = self.current_span();
        self.advance(); // eat `with_responses` identifier
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();
        let mut mappings = Vec::new();
        while !self.check_exact(&TokenKind::RBrace) && !self.at_end() {
            if self.too_many_errors() {
                break;
            }
            if let Some(mapping) = self.parse_response_mapping() {
                mappings.push(mapping);
            } else {
                self.synchronize();
            }
            self.skip_newlines();
        }
        self.expect(&TokenKind::RBrace)?;
        let span = start.merge(self.previous_span());
        Some(WithResponses { mappings, span })
    }

    /// Parse `module.function(args) -> value,`
    fn parse_response_mapping(&mut self) -> Option<ResponseMapping> {
        let start = self.current_span();
        let module = self.expect_ident_or_module_name()?;
        self.expect(&TokenKind::Dot)?;
        let function = self.expect_identifier()?;
        self.expect(&TokenKind::LParen)?;
        let args = self.parse_response_args()?;
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::Arrow)?;
        let response = self.parse_expression()?;
        let span = start.merge(self.previous_span());
        self.eat_comma();
        self.skip_newlines();
        Some(ResponseMapping {
            module,
            function,
            args,
            response,
            span,
        })
    }

    /// Parse response mapping arguments (comma-separated expressions).
    fn parse_response_args(&mut self) -> Option<Vec<Expr>> {
        let mut args = Vec::new();
        self.skip_newlines();
        if self.check_exact(&TokenKind::RParen) {
            return Some(args);
        }
        loop {
            self.skip_newlines();
            args.push(self.parse_expression()?);
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
            if self.check_exact(&TokenKind::RParen) {
                break;
            }
        }
        Some(args)
    }
}

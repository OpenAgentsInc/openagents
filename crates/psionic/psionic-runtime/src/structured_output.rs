use std::collections::{BTreeMap, BTreeSet, HashSet};

use regex_syntax::{
    hir::{Class as RegexClass, Hir as RegexHir, HirKind as RegexHirKind, Look as RegexLook},
    parse as parse_regex,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Stable structured-output kinds surfaced by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputKind {
    /// Discrete string choice from one caller-supplied set.
    Choice,
    /// Regex-constrained string output.
    Regex,
    /// Grammar-constrained output.
    Grammar,
    /// JSON-schema constrained output.
    JsonSchema,
    /// Untyped JSON-object constrained output.
    JsonObject,
    /// Tagged JSON structure with a discriminator field.
    TaggedStructure,
}

impl StructuredOutputKind {
    /// Returns the stable label used by HTTP and telemetry surfaces.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::Regex => "regex",
            Self::Grammar => "grammar",
            Self::JsonSchema => "json_schema",
            Self::JsonObject => "json_object",
            Self::TaggedStructure => "tagged_structure",
        }
    }
}

/// Explicit structured-output support level for one serving path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputSupportLevel {
    /// The serving path enforces this kind natively.
    Native,
    /// The serving path enforces this kind through a Psionic-owned fallback.
    Fallback,
    /// The serving path does not support this kind.
    Unsupported,
}

impl StructuredOutputSupportLevel {
    /// Returns the stable label used by HTTP and telemetry surfaces.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Fallback => "fallback",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Machine-readable structured-output capability fact for one serving path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredOutputCapability {
    /// Structured-output kind covered by this capability row.
    pub kind: StructuredOutputKind,
    /// Support level for the realized serving path.
    pub support_level: StructuredOutputSupportLevel,
    /// Enforcement mode when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<StructuredOutputEnforcementMode>,
    /// Parser family when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<StructuredOutputParser>,
    /// Optional detail for unsupported or degraded modes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl StructuredOutputCapability {
    #[must_use]
    pub const fn fallback(
        kind: StructuredOutputKind,
        mode: StructuredOutputEnforcementMode,
        parser: StructuredOutputParser,
    ) -> Self {
        Self {
            kind,
            support_level: StructuredOutputSupportLevel::Fallback,
            mode: Some(mode),
            parser: Some(parser),
            detail: None,
        }
    }

    #[must_use]
    pub fn unsupported(kind: StructuredOutputKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            support_level: StructuredOutputSupportLevel::Unsupported,
            mode: None,
            parser: None,
            detail: Some(detail.into()),
        }
    }
}

/// Machine-checkable local structured-output fallback modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputEnforcementMode {
    /// Discrete choice fallback owned by Psionic.
    FallbackChoice,
    /// Regex fallback lowered into the Psionic matcher.
    FallbackRegex,
    /// GBNF-compatible grammar fallback owned by Psionic.
    FallbackGrammar,
    /// JSON-schema subset lowered into the Psionic fallback matcher.
    FallbackJsonSchema,
    /// Generic JSON object fallback owned by Psionic.
    FallbackJsonObject,
    /// Tagged-structure fallback lowered into the Psionic JSON-schema path.
    FallbackTaggedStructure,
}

impl StructuredOutputEnforcementMode {
    /// Returns the stable label used by HTTP and telemetry surfaces.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FallbackChoice => "fallback_choice",
            Self::FallbackRegex => "fallback_regex",
            Self::FallbackGrammar => "fallback_grammar",
            Self::FallbackJsonSchema => "fallback_json_schema",
            Self::FallbackJsonObject => "fallback_json_object",
            Self::FallbackTaggedStructure => "fallback_tagged_structure",
        }
    }
}

/// Stable parser families exposed by the fallback matcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredOutputParser {
    /// Discrete caller-supplied choice set.
    ChoiceSet,
    /// Regex subset lowered into the Psionic matcher.
    RegexSubset,
    /// GBNF subset accepted by the current local fallback.
    GbnfSubset,
    /// JSON-schema subset accepted by the current local fallback.
    JsonSchemaSubset,
    /// Untyped JSON-object fallback owned by Psionic.
    JsonObject,
    /// Tagged-structure JSON-schema lowering owned by Psionic.
    TaggedJsonSchema,
}

impl StructuredOutputParser {
    /// Returns the stable label used by HTTP and telemetry surfaces.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ChoiceSet => "choice_set",
            Self::RegexSubset => "regex_subset",
            Self::GbnfSubset => "gbnf_subset",
            Self::JsonSchemaSubset => "json_schema_subset",
            Self::JsonObject => "json_object",
            Self::TaggedJsonSchema => "tagged_json_schema",
        }
    }
}

/// The local structured-output fallback surface currently advertised by Psionic.
#[must_use]
pub fn local_structured_output_parsers() -> Vec<StructuredOutputParser> {
    vec![
        StructuredOutputParser::ChoiceSet,
        StructuredOutputParser::RegexSubset,
        StructuredOutputParser::GbnfSubset,
        StructuredOutputParser::JsonSchemaSubset,
        StructuredOutputParser::JsonObject,
        StructuredOutputParser::TaggedJsonSchema,
    ]
}

/// The local structured-output capability inventory currently advertised by Psionic.
#[must_use]
pub fn local_structured_output_capabilities() -> Vec<StructuredOutputCapability> {
    vec![
        StructuredOutputCapability::fallback(
            StructuredOutputKind::Choice,
            StructuredOutputEnforcementMode::FallbackChoice,
            StructuredOutputParser::ChoiceSet,
        ),
        StructuredOutputCapability::fallback(
            StructuredOutputKind::Regex,
            StructuredOutputEnforcementMode::FallbackRegex,
            StructuredOutputParser::RegexSubset,
        ),
        StructuredOutputCapability::fallback(
            StructuredOutputKind::Grammar,
            StructuredOutputEnforcementMode::FallbackGrammar,
            StructuredOutputParser::GbnfSubset,
        ),
        StructuredOutputCapability::fallback(
            StructuredOutputKind::JsonSchema,
            StructuredOutputEnforcementMode::FallbackJsonSchema,
            StructuredOutputParser::JsonSchemaSubset,
        ),
        StructuredOutputCapability::fallback(
            StructuredOutputKind::JsonObject,
            StructuredOutputEnforcementMode::FallbackJsonObject,
            StructuredOutputParser::JsonObject,
        ),
        StructuredOutputCapability::fallback(
            StructuredOutputKind::TaggedStructure,
            StructuredOutputEnforcementMode::FallbackTaggedStructure,
            StructuredOutputParser::TaggedJsonSchema,
        ),
    ]
}

/// Request-local structured-output contract accepted by the fallback matcher.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StructuredOutputRequest {
    /// Discrete choice from one caller-supplied set of exact strings.
    Choice {
        /// Ordered allowed values.
        values: Vec<String>,
    },
    /// Regex-constrained string output.
    Regex {
        /// Regular-expression pattern enforced against the entire output.
        pattern: String,
    },
    /// Grammar-constrained generation using a GBNF-compatible syntax subset.
    Grammar {
        /// Grammar syntax identifier.
        syntax: StructuredGrammarSyntax,
        /// Grammar body.
        grammar: String,
    },
    /// JSON-schema subset lowered into the local fallback matcher.
    JsonSchema {
        /// Optional schema name preserved for reporting.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// OpenAPI/OpenAI-style JSON schema payload.
        schema: Value,
    },
    /// Generic JSON object fallback with no additional schema restrictions.
    JsonObject,
    /// Tagged JSON structure lowered into the local JSON-schema path.
    TaggedStructure {
        /// Optional schema name preserved for reporting.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Field name used to discriminate between variants.
        discriminator: String,
        /// Allowed tagged variants.
        variants: Vec<StructuredTaggedVariant>,
    },
}

impl StructuredOutputRequest {
    /// Returns the structured-output kind.
    #[must_use]
    pub const fn kind(&self) -> StructuredOutputKind {
        match self {
            Self::Choice { .. } => StructuredOutputKind::Choice,
            Self::Regex { .. } => StructuredOutputKind::Regex,
            Self::Grammar { .. } => StructuredOutputKind::Grammar,
            Self::JsonSchema { .. } => StructuredOutputKind::JsonSchema,
            Self::JsonObject => StructuredOutputKind::JsonObject,
            Self::TaggedStructure { .. } => StructuredOutputKind::TaggedStructure,
        }
    }

    /// Returns the enforced fallback mode for the request.
    #[must_use]
    pub const fn mode(&self) -> StructuredOutputEnforcementMode {
        match self {
            Self::Choice { .. } => StructuredOutputEnforcementMode::FallbackChoice,
            Self::Regex { .. } => StructuredOutputEnforcementMode::FallbackRegex,
            Self::Grammar { .. } => StructuredOutputEnforcementMode::FallbackGrammar,
            Self::JsonSchema { .. } => StructuredOutputEnforcementMode::FallbackJsonSchema,
            Self::JsonObject => StructuredOutputEnforcementMode::FallbackJsonObject,
            Self::TaggedStructure { .. } => {
                StructuredOutputEnforcementMode::FallbackTaggedStructure
            }
        }
    }

    /// Returns the parser family used by the request.
    #[must_use]
    pub const fn parser(&self) -> StructuredOutputParser {
        match self {
            Self::Choice { .. } => StructuredOutputParser::ChoiceSet,
            Self::Regex { .. } => StructuredOutputParser::RegexSubset,
            Self::Grammar { .. } => StructuredOutputParser::GbnfSubset,
            Self::JsonSchema { .. } => StructuredOutputParser::JsonSchemaSubset,
            Self::JsonObject => StructuredOutputParser::JsonObject,
            Self::TaggedStructure { .. } => StructuredOutputParser::TaggedJsonSchema,
        }
    }

    /// Returns the optional schema name preserved for reporting.
    #[must_use]
    pub fn schema_name(&self) -> Option<&str> {
        match self {
            Self::JsonSchema { name, .. } => name.as_deref(),
            Self::TaggedStructure { name, .. } => name.as_deref(),
            Self::Choice { .. } | Self::Regex { .. } | Self::Grammar { .. } | Self::JsonObject => {
                None
            }
        }
    }
}

/// One tagged-structure variant lowered into the local JSON-schema fallback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StructuredTaggedVariant {
    /// Discriminator value for the variant.
    pub tag: String,
    /// Schema for the payload object of this variant.
    pub schema: Value,
}

/// Machine-readable structured value preserved by the serve layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StructuredOutputValue {
    /// One discrete string from a caller-supplied choice set.
    Choice { value: String },
    /// One regex-constrained string value.
    Regex { value: String },
    /// Raw grammar-constrained string output.
    Grammar { value: String },
    /// Parsed JSON value.
    Json { value: Value },
    /// Tagged JSON object with discriminator extraction.
    TaggedStructure {
        discriminator: String,
        tag: String,
        value: Value,
    },
}

impl Eq for StructuredOutputValue {}

/// Grammar syntax identifiers accepted by the local fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuredGrammarSyntax {
    /// GBNF-compatible grammar text.
    Gbnf,
}

/// Machine-checkable report attached to successful fallback-constrained requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredOutputExecutionReport {
    /// Structured-output kind used for the request.
    pub kind: StructuredOutputKind,
    /// Support level used for the request.
    pub support_level: StructuredOutputSupportLevel,
    /// Fallback mode used for enforcement.
    pub mode: StructuredOutputEnforcementMode,
    /// Parser family used by the fallback.
    pub parser: StructuredOutputParser,
    /// Optional schema name when the request carried one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_name: Option<String>,
}

impl StructuredOutputExecutionReport {
    /// Creates a success report from the request contract.
    #[must_use]
    pub fn from_request(request: &StructuredOutputRequest) -> Self {
        Self {
            kind: request.kind(),
            support_level: StructuredOutputSupportLevel::Fallback,
            mode: request.mode(),
            parser: request.parser(),
            schema_name: request.schema_name().map(String::from),
        }
    }
}

/// The current relationship between one text prefix and the fallback matcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StructuredOutputMatchStatus {
    /// The text cannot be extended into a valid constrained output.
    Invalid,
    /// The text is a valid prefix but not yet a full constrained output.
    Prefix,
    /// The text is already a valid constrained output.
    Complete,
}

/// Classification result for one candidate text prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructuredOutputMatch {
    /// Whether the text is invalid, partial, or complete.
    pub status: StructuredOutputMatchStatus,
    /// Whether a longer constrained output is still possible from this prefix.
    pub can_continue: bool,
}

impl StructuredOutputMatch {
    #[must_use]
    const fn invalid() -> Self {
        Self {
            status: StructuredOutputMatchStatus::Invalid,
            can_continue: false,
        }
    }

    #[must_use]
    const fn prefix() -> Self {
        Self {
            status: StructuredOutputMatchStatus::Prefix,
            can_continue: true,
        }
    }

    #[must_use]
    const fn complete(can_continue: bool) -> Self {
        Self {
            status: StructuredOutputMatchStatus::Complete,
            can_continue,
        }
    }

    /// Returns whether the text is at least still admissible under the matcher.
    #[must_use]
    pub const fn is_allowed(self) -> bool {
        !matches!(self.status, StructuredOutputMatchStatus::Invalid)
    }
}

/// Failure returned when the local fallback cannot compile or match a request.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StructuredOutputError {
    /// The request declared an invalid or empty choice set.
    #[error("invalid structured choice set: {0}")]
    InvalidChoice(String),
    /// The regex failed to parse.
    #[error("invalid structured regex: {0}")]
    InvalidRegex(String),
    /// The regex requested an unsupported feature.
    #[error("unsupported structured regex feature for local fallback: {0}")]
    UnsupportedRegex(String),
    /// The grammar syntax is unsupported by the current fallback.
    #[error("unsupported structured grammar syntax `{0}`")]
    UnsupportedGrammarSyntax(String),
    /// The grammar failed to parse.
    #[error("invalid structured grammar: {0}")]
    InvalidGrammar(String),
    /// The grammar referenced an unknown rule.
    #[error("structured grammar referenced unknown rule `{0}`")]
    UnknownRule(String),
    /// The grammar requested an unsupported token-matching feature.
    #[error("structured grammar token matches are not supported by the current fallback: {0}")]
    UnsupportedTokenMatch(String),
    /// The grammar requested an unsupported feature.
    #[error("structured grammar feature is unsupported by the current fallback: {0}")]
    UnsupportedGrammarFeature(String),
    /// The JSON schema could not be lowered into the fallback matcher.
    #[error("unsupported JSON schema feature for local fallback: {0}")]
    UnsupportedJsonSchema(String),
    /// The JSON schema is malformed.
    #[error("invalid JSON schema for local fallback: {0}")]
    InvalidJsonSchema(String),
    /// The tagged-structure request is malformed.
    #[error("invalid tagged structure for local fallback: {0}")]
    InvalidTaggedStructure(String),
    /// The structured value could not be materialized from the generated text.
    #[error("structured value materialization failed: {0}")]
    InvalidStructuredValue(String),
}

/// Compiled fallback matcher used by the local serving path.
#[derive(Clone, Debug)]
pub struct StructuredOutputMatcher {
    request: StructuredOutputRequest,
    grammar: Grammar,
}

impl StructuredOutputMatcher {
    /// Compiles a request-local fallback matcher.
    pub fn compile(request: StructuredOutputRequest) -> Result<Self, StructuredOutputError> {
        let grammar = match &request {
            StructuredOutputRequest::Choice { values } => Grammar::from_choice(values)?,
            StructuredOutputRequest::Regex { pattern } => Grammar::from_regex(pattern)?,
            StructuredOutputRequest::Grammar { syntax, grammar } => match syntax {
                StructuredGrammarSyntax::Gbnf => Grammar::parse_gbnf(grammar)?,
            },
            StructuredOutputRequest::JsonSchema { schema, .. } => {
                Grammar::from_json_schema(schema)?
            }
            StructuredOutputRequest::JsonObject => Grammar::from_json_object(),
            StructuredOutputRequest::TaggedStructure {
                discriminator,
                variants,
                ..
            } => Grammar::from_tagged_structure(discriminator, variants)?,
        };
        Ok(Self { request, grammar })
    }

    /// Returns the request-local execution report for the matcher.
    #[must_use]
    pub fn execution_report(&self) -> StructuredOutputExecutionReport {
        StructuredOutputExecutionReport::from_request(&self.request)
    }

    /// Returns the fallback mode used by this matcher.
    #[must_use]
    pub const fn mode(&self) -> StructuredOutputEnforcementMode {
        self.request.mode()
    }

    /// Returns the parser family used by this matcher.
    #[must_use]
    pub const fn parser(&self) -> StructuredOutputParser {
        self.request.parser()
    }

    /// Returns the schema name preserved for reporting when present.
    #[must_use]
    pub fn schema_name(&self) -> Option<&str> {
        self.request.schema_name()
    }

    /// Returns the structured-output kind used by this matcher.
    #[must_use]
    pub const fn kind(&self) -> StructuredOutputKind {
        self.request.kind()
    }

    /// Returns whether the matcher should auto-close once a complete value is reached.
    #[must_use]
    pub const fn prefers_completion_termination(&self) -> bool {
        matches!(
            self.request,
            StructuredOutputRequest::Choice { .. }
                | StructuredOutputRequest::Regex { .. }
                | StructuredOutputRequest::JsonSchema { .. }
                | StructuredOutputRequest::JsonObject
                | StructuredOutputRequest::TaggedStructure { .. }
        )
    }

    /// Classifies one generated text prefix against the compiled matcher.
    #[must_use]
    pub fn classify(&self, text: &str) -> StructuredOutputMatch {
        self.grammar.classify(text)
    }

    /// Materializes one completed structured value from generated text.
    pub fn materialize(
        &self,
        text: &str,
    ) -> Result<Option<StructuredOutputValue>, StructuredOutputError> {
        if !matches!(
            self.classify(text).status,
            StructuredOutputMatchStatus::Complete
        ) {
            return Ok(None);
        }
        match &self.request {
            StructuredOutputRequest::Choice { values } => {
                if values.iter().any(|value| value == text) {
                    Ok(Some(StructuredOutputValue::Choice {
                        value: text.to_string(),
                    }))
                } else {
                    Err(StructuredOutputError::InvalidStructuredValue(format!(
                        "completed choice `{text}` is not in the allowed set"
                    )))
                }
            }
            StructuredOutputRequest::Regex { .. } => Ok(Some(StructuredOutputValue::Regex {
                value: text.to_string(),
            })),
            StructuredOutputRequest::Grammar { .. } => Ok(Some(StructuredOutputValue::Grammar {
                value: text.to_string(),
            })),
            StructuredOutputRequest::JsonSchema { .. } | StructuredOutputRequest::JsonObject => {
                let value = serde_json::from_str(text).map_err(|error| {
                    StructuredOutputError::InvalidStructuredValue(format!(
                        "failed to parse JSON output: {error}"
                    ))
                })?;
                Ok(Some(StructuredOutputValue::Json { value }))
            }
            StructuredOutputRequest::TaggedStructure { discriminator, .. } => {
                let value = serde_json::from_str::<Value>(text).map_err(|error| {
                    StructuredOutputError::InvalidStructuredValue(format!(
                        "failed to parse tagged JSON output: {error}"
                    ))
                })?;
                let tag = value
                    .get(discriminator)
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StructuredOutputError::InvalidStructuredValue(format!(
                            "tagged output is missing string discriminator `{discriminator}`"
                        ))
                    })?
                    .to_string();
                Ok(Some(StructuredOutputValue::TaggedStructure {
                    discriminator: discriminator.clone(),
                    tag,
                    value,
                }))
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Grammar {
    root: String,
    rules: BTreeMap<String, Expression>,
}

impl Grammar {
    fn from_choice(values: &[String]) -> Result<Self, StructuredOutputError> {
        if values.is_empty() {
            return Err(StructuredOutputError::InvalidChoice(String::from(
                "choice set must include at least one value",
            )));
        }
        if values.iter().any(|value| value.is_empty()) {
            return Err(StructuredOutputError::InvalidChoice(String::from(
                "choice values must not be empty",
            )));
        }
        let alternatives = values
            .iter()
            .map(|value| Sequence {
                terms: vec![Term::literal(value.clone())],
            })
            .collect();
        let mut rules = BTreeMap::new();
        rules.insert(String::from("root"), Expression::choice(alternatives));
        Ok(Self {
            root: String::from("root"),
            rules,
        })
    }

    fn from_regex(pattern: &str) -> Result<Self, StructuredOutputError> {
        if pattern.is_empty() {
            return Err(StructuredOutputError::InvalidRegex(String::from(
                "regex pattern must not be empty",
            )));
        }
        let hir = parse_regex(pattern)
            .map_err(|error| StructuredOutputError::InvalidRegex(error.to_string()))?;
        let mut rules = BTreeMap::new();
        rules.insert(String::from("root"), regex_hir_to_expression(&hir)?);
        let grammar = Self {
            root: String::from("root"),
            rules,
        };
        grammar.validate()?;
        Ok(grammar)
    }

    fn parse_gbnf(grammar: &str) -> Result<Self, StructuredOutputError> {
        let rule_text = preprocess_gbnf_rules(grammar)?;
        if rule_text.is_empty() {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "grammar must define at least one rule",
            )));
        }

        let mut rules = BTreeMap::new();
        for rule in rule_text {
            let Some((name, rhs)) = rule.split_once("::=") else {
                return Err(StructuredOutputError::InvalidGrammar(format!(
                    "rule is missing `::=`: {rule}"
                )));
            };
            let name = name.trim();
            if name.is_empty() {
                return Err(StructuredOutputError::InvalidGrammar(String::from(
                    "rule name must not be empty",
                )));
            }
            let expression = GrammarExpressionParser::new(rhs.trim()).parse_expression()?;
            rules.insert(name.to_string(), expression);
        }

        if !rules.contains_key("root") {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "grammar must define a `root` rule",
            )));
        }

        let grammar = Self {
            root: String::from("root"),
            rules,
        };
        grammar.validate()?;
        Ok(grammar)
    }

    fn from_json_object() -> Self {
        let mut rules = BTreeMap::new();
        install_json_value_rules(&mut rules);
        rules.insert(
            String::from("root"),
            Expression::sequence(vec![Term::rule("json-object")]),
        );
        Self {
            root: String::from("root"),
            rules,
        }
    }

    fn from_json_schema(schema: &Value) -> Result<Self, StructuredOutputError> {
        let mut lowerer = JsonSchemaLowerer::default();
        install_json_value_rules(&mut lowerer.rules);
        let root_rule = lowerer.lower_named_rule("schema-root", schema)?;
        let mut rules = lowerer.rules;
        rules.insert(
            String::from("root"),
            Expression::sequence(vec![Term::rule(root_rule)]),
        );
        let grammar = Self {
            root: String::from("root"),
            rules,
        };
        grammar.validate()?;
        Ok(grammar)
    }

    fn from_tagged_structure(
        discriminator: &str,
        variants: &[StructuredTaggedVariant],
    ) -> Result<Self, StructuredOutputError> {
        let schema = tagged_structure_schema(discriminator, variants)?;
        Self::from_json_schema(&schema)
    }

    fn validate(&self) -> Result<(), StructuredOutputError> {
        let mut referenced = BTreeSet::new();
        for expression in self.rules.values() {
            collect_rule_references(expression, &mut referenced);
        }
        for rule in referenced {
            if !self.rules.contains_key(rule.as_str()) {
                return Err(StructuredOutputError::UnknownRule(rule));
            }
        }
        Ok(())
    }

    fn classify(&self, text: &str) -> StructuredOutputMatch {
        let input = text.chars().collect::<Vec<_>>();
        let mut ctx = MatchContext::new(self, &input);
        let result = ctx.match_rule(self.root.as_str(), 0);
        let input_len = input.len();
        if result.complete_positions.contains(&input_len) {
            StructuredOutputMatch::complete(result.needs_more)
        } else if result.needs_more {
            StructuredOutputMatch::prefix()
        } else {
            StructuredOutputMatch::invalid()
        }
    }
}

fn preprocess_gbnf_rules(grammar: &str) -> Result<Vec<String>, StructuredOutputError> {
    let mut rules = Vec::new();
    let mut current = String::new();

    for raw_line in grammar.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.contains("<[") || line.contains("!<[") || line.contains('<') {
            return Err(StructuredOutputError::UnsupportedTokenMatch(
                line.to_string(),
            ));
        }
        if line.contains("::=") {
            if !current.trim().is_empty() {
                rules.push(current.trim().to_string());
            }
            current.clear();
            current.push_str(line);
        } else {
            if current.is_empty() {
                return Err(StructuredOutputError::InvalidGrammar(format!(
                    "grammar continuation line appeared before any rule: {line}"
                )));
            }
            current.push(' ');
            current.push_str(line);
        }
    }

    if !current.trim().is_empty() {
        rules.push(current.trim().to_string());
    }
    Ok(rules)
}

fn regex_hir_to_expression(hir: &RegexHir) -> Result<Expression, StructuredOutputError> {
    match hir.kind() {
        RegexHirKind::Empty => Ok(Expression::sequence(Vec::new())),
        RegexHirKind::Literal(literal) => {
            let value = std::str::from_utf8(&literal.0)
                .map_err(|_| {
                    StructuredOutputError::UnsupportedRegex(String::from(
                        "regex literals must remain valid UTF-8",
                    ))
                })?
                .to_string();
            Ok(Expression::sequence(vec![Term::literal(value)]))
        }
        RegexHirKind::Class(class) => Ok(Expression::sequence(vec![Term::class(
            regex_class_to_character_class(class)?,
        )])),
        RegexHirKind::Look(look) => regex_look_to_expression(*look),
        RegexHirKind::Repetition(repetition) => Ok(Expression::sequence(vec![
            Term::group(regex_hir_to_expression(repetition.sub.as_ref())?).repeat(
                usize::try_from(repetition.min).map_err(|_| {
                    StructuredOutputError::UnsupportedRegex(String::from(
                        "regex repetition minimum does not fit usize",
                    ))
                })?,
                repetition
                    .max
                    .map(|value| {
                        usize::try_from(value).map_err(|_| {
                            StructuredOutputError::UnsupportedRegex(String::from(
                                "regex repetition maximum does not fit usize",
                            ))
                        })
                    })
                    .transpose()?,
            ),
        ])),
        RegexHirKind::Capture(capture) => regex_hir_to_expression(capture.sub.as_ref()),
        RegexHirKind::Concat(expressions) => Ok(Expression::sequence(
            expressions
                .iter()
                .map(regex_hir_to_term)
                .collect::<Result<Vec<_>, StructuredOutputError>>()?,
        )),
        RegexHirKind::Alternation(expressions) => Ok(Expression::choice(
            expressions
                .iter()
                .map(|expression| {
                    let expression = regex_hir_to_expression(expression)?;
                    Ok(expression_as_sequence(expression))
                })
                .collect::<Result<Vec<_>, StructuredOutputError>>()?,
        )),
    }
}

fn regex_hir_to_term(hir: &RegexHir) -> Result<Term, StructuredOutputError> {
    Ok(Term::group(regex_hir_to_expression(hir)?))
}

fn expression_as_sequence(expression: Expression) -> Sequence {
    if let [sequence] = expression.alternatives.as_slice() {
        sequence.clone()
    } else {
        Sequence {
            terms: vec![Term::group(expression)],
        }
    }
}

fn regex_look_to_expression(look: RegexLook) -> Result<Expression, StructuredOutputError> {
    match look {
        RegexLook::Start | RegexLook::End => Ok(Expression::sequence(Vec::new())),
        other => Err(StructuredOutputError::UnsupportedRegex(format!(
            "look-around `{other:?}` is not supported by the local fallback"
        ))),
    }
}

fn regex_class_to_character_class(
    class: &RegexClass,
) -> Result<CharacterClass, StructuredOutputError> {
    match class {
        RegexClass::Unicode(class) => Ok(CharacterClass {
            negated: false,
            ranges: class
                .iter()
                .map(|range| CharacterRange {
                    start: range.start(),
                    end: range.end(),
                })
                .collect(),
        }),
        RegexClass::Bytes(class) => {
            let mut ranges = Vec::new();
            for range in class.iter() {
                let start = char::from(range.start());
                let end = char::from(range.end());
                if !start.is_ascii() || !end.is_ascii() {
                    return Err(StructuredOutputError::UnsupportedRegex(String::from(
                        "byte-oriented regex classes must stay ASCII in the local fallback",
                    )));
                }
                ranges.push(CharacterRange { start, end });
            }
            Ok(CharacterClass {
                negated: false,
                ranges,
            })
        }
    }
}

fn tagged_structure_schema(
    discriminator: &str,
    variants: &[StructuredTaggedVariant],
) -> Result<Value, StructuredOutputError> {
    if discriminator.trim().is_empty() {
        return Err(StructuredOutputError::InvalidTaggedStructure(String::from(
            "discriminator must not be empty",
        )));
    }
    if variants.is_empty() {
        return Err(StructuredOutputError::InvalidTaggedStructure(String::from(
            "tagged structure must include at least one variant",
        )));
    }

    let mut lowered_variants = Vec::with_capacity(variants.len());
    for variant in variants {
        if variant.tag.trim().is_empty() {
            return Err(StructuredOutputError::InvalidTaggedStructure(String::from(
                "variant tag must not be empty",
            )));
        }
        let mut schema = match &variant.schema {
            Value::Object(map) => map.clone(),
            _ => {
                return Err(StructuredOutputError::InvalidTaggedStructure(format!(
                    "variant `{}` schema must be an object",
                    variant.tag
                )));
            }
        };
        match schema.get("type") {
            Some(Value::String(kind)) if kind == "object" => {}
            Some(_) => {
                return Err(StructuredOutputError::InvalidTaggedStructure(format!(
                    "variant `{}` schema type must be `object`",
                    variant.tag
                )));
            }
            None => {
                schema.insert(String::from("type"), Value::String(String::from("object")));
            }
        }
        let properties = schema
            .entry(String::from("properties"))
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let Value::Object(properties) = properties else {
            return Err(StructuredOutputError::InvalidTaggedStructure(format!(
                "variant `{}` properties must be an object",
                variant.tag
            )));
        };
        properties.insert(
            discriminator.to_string(),
            serde_json::json!({ "const": variant.tag }),
        );

        let required = schema
            .entry(String::from("required"))
            .or_insert_with(|| Value::Array(Vec::new()));
        let Value::Array(required) = required else {
            return Err(StructuredOutputError::InvalidTaggedStructure(format!(
                "variant `{}` required entries must be an array",
                variant.tag
            )));
        };
        if !required
            .iter()
            .any(|value| value.as_str() == Some(discriminator))
        {
            required.push(Value::String(discriminator.to_string()));
        }
        lowered_variants.push(Value::Object(schema));
    }

    Ok(serde_json::json!({ "oneOf": lowered_variants }))
}

#[derive(Clone, Debug)]
struct Expression {
    alternatives: Vec<Sequence>,
}

impl Expression {
    fn sequence(terms: Vec<Term>) -> Self {
        Self {
            alternatives: vec![Sequence { terms }],
        }
    }

    fn choice(alternatives: Vec<Sequence>) -> Self {
        Self { alternatives }
    }
}

#[derive(Clone, Debug)]
struct Sequence {
    terms: Vec<Term>,
}

#[derive(Clone, Debug)]
struct Term {
    symbol: Symbol,
    min: usize,
    max: Option<usize>,
}

impl Term {
    fn rule(name: impl Into<String>) -> Self {
        Self {
            symbol: Symbol::Rule(name.into()),
            min: 1,
            max: Some(1),
        }
    }

    fn literal(value: impl Into<String>) -> Self {
        Self {
            symbol: Symbol::Literal(value.into()),
            min: 1,
            max: Some(1),
        }
    }

    fn class(character_class: CharacterClass) -> Self {
        Self {
            symbol: Symbol::CharacterClass(character_class),
            min: 1,
            max: Some(1),
        }
    }

    fn group(expression: Expression) -> Self {
        Self {
            symbol: Symbol::Group(Box::new(expression)),
            min: 1,
            max: Some(1),
        }
    }

    fn any_char() -> Self {
        Self {
            symbol: Symbol::AnyChar,
            min: 1,
            max: Some(1),
        }
    }

    fn repeat(mut self, min: usize, max: Option<usize>) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

#[derive(Clone, Debug)]
enum Symbol {
    Literal(String),
    CharacterClass(CharacterClass),
    Rule(String),
    Group(Box<Expression>),
    AnyChar,
}

#[derive(Clone, Debug)]
struct CharacterClass {
    negated: bool,
    ranges: Vec<CharacterRange>,
}

impl CharacterClass {
    fn contains(&self, candidate: char) -> bool {
        let in_range = self
            .ranges
            .iter()
            .any(|range| range.start <= candidate && candidate <= range.end);
        if self.negated { !in_range } else { in_range }
    }
}

#[derive(Clone, Debug)]
struct CharacterRange {
    start: char,
    end: char,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MatchResult {
    complete_positions: BTreeSet<usize>,
    needs_more: bool,
}

impl MatchResult {
    fn invalid() -> Self {
        Self {
            complete_positions: BTreeSet::new(),
            needs_more: false,
        }
    }
}

struct MatchContext<'a> {
    grammar: &'a Grammar,
    input: &'a [char],
    memo: BTreeMap<(String, usize), MatchResult>,
    visiting: HashSet<(String, usize)>,
}

impl<'a> MatchContext<'a> {
    fn new(grammar: &'a Grammar, input: &'a [char]) -> Self {
        Self {
            grammar,
            input,
            memo: BTreeMap::new(),
            visiting: HashSet::new(),
        }
    }

    fn match_rule(&mut self, rule: &str, position: usize) -> MatchResult {
        let key = (rule.to_string(), position);
        if let Some(result) = self.memo.get(&key) {
            return result.clone();
        }
        if !self.visiting.insert(key.clone()) {
            return MatchResult::invalid();
        }
        let result = self
            .grammar
            .rules
            .get(rule)
            .map_or_else(MatchResult::invalid, |expression| {
                self.match_expression(expression, position)
            });
        self.visiting.remove(&key);
        self.memo.insert(key, result.clone());
        result
    }

    fn match_expression(&mut self, expression: &Expression, position: usize) -> MatchResult {
        let mut complete_positions = BTreeSet::new();
        let mut needs_more = false;
        for alternative in &expression.alternatives {
            let result = self.match_sequence(alternative, position);
            complete_positions.extend(result.complete_positions);
            needs_more |= result.needs_more;
        }
        MatchResult {
            complete_positions,
            needs_more,
        }
    }

    fn match_sequence(&mut self, sequence: &Sequence, position: usize) -> MatchResult {
        let mut active_positions = BTreeSet::from([position]);
        let mut needs_more = false;
        for term in &sequence.terms {
            let mut next_positions = BTreeSet::new();
            let mut term_needs_more = false;
            for current in &active_positions {
                let result = self.match_term(term, *current);
                next_positions.extend(result.complete_positions);
                term_needs_more |= result.needs_more;
            }
            if next_positions.is_empty() {
                return MatchResult {
                    complete_positions: BTreeSet::new(),
                    needs_more: term_needs_more,
                };
            }
            active_positions = next_positions;
            needs_more |= term_needs_more;
        }
        MatchResult {
            complete_positions: active_positions,
            needs_more,
        }
    }

    fn match_term(&mut self, term: &Term, position: usize) -> MatchResult {
        let mut states = BTreeSet::from([position]);
        let mut complete_positions = BTreeSet::new();
        let mut needs_more = false;
        if term.min == 0 {
            complete_positions.insert(position);
        }

        let repeat_limit = term
            .max
            .unwrap_or_else(|| self.input.len().saturating_sub(position).saturating_add(1));
        for count in 1..=repeat_limit {
            let mut next_states = BTreeSet::new();
            let mut iteration_needs_more = false;
            for state in &states {
                let result = self.match_symbol(&term.symbol, *state);
                next_states.extend(result.complete_positions);
                iteration_needs_more |= result.needs_more;
            }
            needs_more |= iteration_needs_more;
            if next_states.is_empty() {
                break;
            }
            if count >= term.min {
                complete_positions.extend(next_states.iter().copied());
            }
            if next_states == states {
                break;
            }
            states = next_states;
        }

        MatchResult {
            complete_positions,
            needs_more,
        }
    }

    fn match_symbol(&mut self, symbol: &Symbol, position: usize) -> MatchResult {
        match symbol {
            Symbol::Literal(literal) => match_literal(literal, self.input, position),
            Symbol::CharacterClass(character_class) => {
                if position >= self.input.len() {
                    MatchResult {
                        complete_positions: BTreeSet::new(),
                        needs_more: true,
                    }
                } else if character_class.contains(self.input[position]) {
                    MatchResult {
                        complete_positions: BTreeSet::from([position + 1]),
                        needs_more: false,
                    }
                } else {
                    MatchResult::invalid()
                }
            }
            Symbol::Rule(rule) => self.match_rule(rule.as_str(), position),
            Symbol::Group(expression) => self.match_expression(expression, position),
            Symbol::AnyChar => {
                if position >= self.input.len() {
                    MatchResult {
                        complete_positions: BTreeSet::new(),
                        needs_more: true,
                    }
                } else {
                    MatchResult {
                        complete_positions: BTreeSet::from([position + 1]),
                        needs_more: false,
                    }
                }
            }
        }
    }
}

fn match_literal(literal: &str, input: &[char], position: usize) -> MatchResult {
    let literal_chars = literal.chars().collect::<Vec<_>>();
    for (offset, expected) in literal_chars.iter().enumerate() {
        let Some(actual) = input.get(position + offset) else {
            return MatchResult {
                complete_positions: BTreeSet::new(),
                needs_more: true,
            };
        };
        if actual != expected {
            return MatchResult::invalid();
        }
    }
    MatchResult {
        complete_positions: BTreeSet::from([position + literal_chars.len()]),
        needs_more: false,
    }
}

fn collect_rule_references(expression: &Expression, referenced: &mut BTreeSet<String>) {
    for alternative in &expression.alternatives {
        for term in &alternative.terms {
            match &term.symbol {
                Symbol::Rule(rule) => {
                    referenced.insert(rule.clone());
                }
                Symbol::Group(expression) => collect_rule_references(expression, referenced),
                Symbol::Literal(_) | Symbol::CharacterClass(_) | Symbol::AnyChar => {}
            }
        }
    }
}

struct GrammarExpressionParser<'a> {
    text: &'a str,
    index: usize,
}

impl<'a> GrammarExpressionParser<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, index: 0 }
    }

    fn parse_expression(mut self) -> Result<Expression, StructuredOutputError> {
        let expression = self.parse_choice()?;
        self.skip_layout();
        if self.index != self.text.len() {
            return Err(StructuredOutputError::InvalidGrammar(format!(
                "unexpected trailing grammar input starting at `{}`",
                &self.text[self.index..]
            )));
        }
        Ok(expression)
    }

    fn parse_choice(&mut self) -> Result<Expression, StructuredOutputError> {
        let mut alternatives = vec![self.parse_sequence()?];
        loop {
            self.skip_layout();
            if !self.consume_char('|') {
                break;
            }
            alternatives.push(self.parse_sequence()?);
        }
        Ok(Expression::choice(alternatives))
    }

    fn parse_sequence(&mut self) -> Result<Sequence, StructuredOutputError> {
        let mut terms = Vec::new();
        loop {
            self.skip_layout();
            if self.is_eof() || self.peek_char() == Some('|') || self.peek_char() == Some(')') {
                break;
            }
            terms.push(self.parse_term()?);
        }
        Ok(Sequence { terms })
    }

    fn parse_term(&mut self) -> Result<Term, StructuredOutputError> {
        self.skip_layout();
        let mut term = match self.peek_char() {
            Some('"') => Term::literal(self.parse_literal()?),
            Some('[') => Term::class(self.parse_character_class()?),
            Some('(') => {
                self.index += '('.len_utf8();
                let expression = self.parse_choice()?;
                self.skip_layout();
                if !self.consume_char(')') {
                    return Err(StructuredOutputError::InvalidGrammar(String::from(
                        "unterminated grouped expression",
                    )));
                }
                Term::group(expression)
            }
            Some('.') => {
                self.index += '.'.len_utf8();
                Term::any_char()
            }
            Some('<') => {
                return Err(StructuredOutputError::UnsupportedTokenMatch(
                    self.text[self.index..].to_string(),
                ));
            }
            Some(character) if is_rule_name_start(character) => Term::rule(self.parse_rule_name()?),
            Some(character) => {
                return Err(StructuredOutputError::InvalidGrammar(format!(
                    "unexpected grammar character `{character}`"
                )));
            }
            None => {
                return Err(StructuredOutputError::InvalidGrammar(String::from(
                    "expected grammar term",
                )));
            }
        };

        self.skip_layout();
        match self.peek_char() {
            Some('?') => {
                self.index += '?'.len_utf8();
                term = term.repeat(0, Some(1));
            }
            Some('*') => {
                self.index += '*'.len_utf8();
                term = term.repeat(0, None);
            }
            Some('+') => {
                self.index += '+'.len_utf8();
                term = term.repeat(1, None);
            }
            Some('{') => {
                let (min, max) = self.parse_repetition_range()?;
                term = term.repeat(min, max);
            }
            _ => {}
        }

        Ok(term)
    }

    fn parse_rule_name(&mut self) -> Result<String, StructuredOutputError> {
        let start = self.index;
        while let Some(character) = self.peek_char() {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                self.index += character.len_utf8();
            } else {
                break;
            }
        }
        if self.index == start {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "expected rule name",
            )));
        }
        Ok(self.text[start..self.index].to_string())
    }

    fn parse_literal(&mut self) -> Result<String, StructuredOutputError> {
        if !self.consume_char('"') {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "expected string literal",
            )));
        }

        let mut value = String::new();
        loop {
            let Some(character) = self.peek_char() else {
                return Err(StructuredOutputError::InvalidGrammar(String::from(
                    "unterminated string literal",
                )));
            };
            self.index += character.len_utf8();
            match character {
                '"' => break,
                '\\' => value.push(self.parse_escape()?),
                other => value.push(other),
            }
        }
        Ok(value)
    }

    fn parse_character_class(&mut self) -> Result<CharacterClass, StructuredOutputError> {
        if !self.consume_char('[') {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "expected character class",
            )));
        }

        let mut negated = false;
        if self.peek_char() == Some('^') {
            negated = true;
            self.index += '^'.len_utf8();
        }

        let mut ranges = Vec::new();
        loop {
            let Some(character) = self.peek_char() else {
                return Err(StructuredOutputError::InvalidGrammar(String::from(
                    "unterminated character class",
                )));
            };
            if character == ']' {
                self.index += ']'.len_utf8();
                break;
            }
            let start = if character == '\\' {
                self.index += '\\'.len_utf8();
                self.parse_escape()?
            } else {
                self.index += character.len_utf8();
                character
            };
            if self.peek_char() == Some('-') {
                self.index += '-'.len_utf8();
                let Some(end_char) = self.peek_char() else {
                    return Err(StructuredOutputError::InvalidGrammar(String::from(
                        "unterminated character range",
                    )));
                };
                let end = if end_char == '\\' {
                    self.index += '\\'.len_utf8();
                    self.parse_escape()?
                } else {
                    self.index += end_char.len_utf8();
                    end_char
                };
                ranges.push(CharacterRange { start, end });
            } else {
                ranges.push(CharacterRange { start, end: start });
            }
        }
        Ok(CharacterClass { negated, ranges })
    }

    fn parse_escape(&mut self) -> Result<char, StructuredOutputError> {
        let Some(character) = self.peek_char() else {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "unterminated escape sequence",
            )));
        };
        self.index += character.len_utf8();
        match character {
            '"' => Ok('"'),
            '\\' => Ok('\\'),
            '/' => Ok('/'),
            'b' => Ok('\u{0008}'),
            'f' => Ok('\u{000c}'),
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            'x' => self.parse_hex_escape(2),
            'u' => self.parse_hex_escape(4),
            'U' => self.parse_hex_escape(8),
            other => Err(StructuredOutputError::InvalidGrammar(format!(
                "unsupported escape sequence `\\{other}`"
            ))),
        }
    }

    fn parse_hex_escape(&mut self, width: usize) -> Result<char, StructuredOutputError> {
        if self.index + width > self.text.len() {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "truncated hexadecimal escape",
            )));
        }
        let hex = &self.text[self.index..self.index + width];
        if !hex.chars().all(|character| character.is_ascii_hexdigit()) {
            return Err(StructuredOutputError::InvalidGrammar(format!(
                "invalid hexadecimal escape `{hex}`"
            )));
        }
        self.index += width;
        let value = u32::from_str_radix(hex, 16).map_err(|error| {
            StructuredOutputError::InvalidGrammar(format!(
                "invalid hexadecimal escape `{hex}`: {error}"
            ))
        })?;
        char::from_u32(value).ok_or_else(|| {
            StructuredOutputError::InvalidGrammar(format!(
                "hexadecimal escape `{hex}` does not map to a valid codepoint"
            ))
        })
    }

    fn parse_repetition_range(&mut self) -> Result<(usize, Option<usize>), StructuredOutputError> {
        if !self.consume_char('{') {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "expected repetition range",
            )));
        }
        self.skip_layout();
        let min = self.parse_usize()?;
        self.skip_layout();
        if self.consume_char('}') {
            return Ok((min, Some(min)));
        }
        if !self.consume_char(',') {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "expected `,` inside repetition range",
            )));
        }
        self.skip_layout();
        let max = if self.peek_char() == Some('}') {
            None
        } else {
            Some(self.parse_usize()?)
        };
        self.skip_layout();
        if !self.consume_char('}') {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "unterminated repetition range",
            )));
        }
        if max.map_or(false, |max| max < min) {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "repetition range max must be greater than or equal to min",
            )));
        }
        Ok((min, max))
    }

    fn parse_usize(&mut self) -> Result<usize, StructuredOutputError> {
        let start = self.index;
        while let Some(character) = self.peek_char() {
            if character.is_ascii_digit() {
                self.index += character.len_utf8();
            } else {
                break;
            }
        }
        if start == self.index {
            return Err(StructuredOutputError::InvalidGrammar(String::from(
                "expected decimal integer",
            )));
        }
        self.text[start..self.index]
            .parse::<usize>()
            .map_err(|error| {
                StructuredOutputError::InvalidGrammar(format!(
                    "invalid repetition bound `{}`: {error}",
                    &self.text[start..self.index]
                ))
            })
    }

    fn skip_layout(&mut self) {
        while let Some(character) = self.peek_char() {
            if character.is_whitespace() {
                self.index += character.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.index += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text[self.index..].chars().next()
    }

    fn is_eof(&self) -> bool {
        self.index >= self.text.len()
    }
}

fn is_rule_name_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

#[derive(Default)]
struct JsonSchemaLowerer {
    rules: BTreeMap<String, Expression>,
    next_rule_id: usize,
}

impl JsonSchemaLowerer {
    fn lower_named_rule(
        &mut self,
        name: &str,
        schema: &Value,
    ) -> Result<String, StructuredOutputError> {
        let expression = self.lower_schema(schema)?;
        let rule_name = sanitize_rule_name(name);
        self.rules.insert(rule_name.clone(), expression);
        Ok(rule_name)
    }

    fn lower_schema(&mut self, schema: &Value) -> Result<Expression, StructuredOutputError> {
        if let Some(options) = schema.get("oneOf").or_else(|| schema.get("anyOf")) {
            let Value::Array(options) = options else {
                return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`oneOf` / `anyOf` must be an array",
                )));
            };
            if options.is_empty() {
                return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`oneOf` / `anyOf` must not be empty",
                )));
            }
            let mut alternatives = Vec::new();
            for option in options {
                let rule = self.define_inline_rule(option)?;
                alternatives.push(Sequence {
                    terms: vec![Term::rule(rule)],
                });
            }
            return Ok(Expression::choice(alternatives));
        }

        if let Some(value) = schema.get("const") {
            return Ok(Expression::sequence(vec![Term::literal(json_literal(
                value,
            )?)]));
        }
        if let Some(values) = schema.get("enum") {
            let Value::Array(values) = values else {
                return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`enum` must be an array",
                )));
            };
            if values.is_empty() {
                return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`enum` must not be empty",
                )));
            }
            return Ok(Expression::choice(
                values
                    .iter()
                    .map(|value| {
                        Ok(Sequence {
                            terms: vec![Term::literal(json_literal(value)?)],
                        })
                    })
                    .collect::<Result<Vec<_>, StructuredOutputError>>()?,
            ));
        }

        let schema_type = infer_schema_type(schema)?;
        match schema_type.as_str() {
            "object" => self.lower_object(schema),
            "array" => self.lower_array(schema),
            "string" => self.lower_string(schema),
            "integer" => Ok(Expression::sequence(vec![Term::rule("json-integer")])),
            "number" => Ok(Expression::sequence(vec![Term::rule("json-number")])),
            "boolean" => Ok(Expression::choice(vec![
                Sequence {
                    terms: vec![Term::literal("true")],
                },
                Sequence {
                    terms: vec![Term::literal("false")],
                },
            ])),
            "null" => Ok(Expression::sequence(vec![Term::literal("null")])),
            other => Err(StructuredOutputError::UnsupportedJsonSchema(format!(
                "schema type `{other}` is not supported by the local fallback"
            ))),
        }
    }

    fn lower_object(&mut self, schema: &Value) -> Result<Expression, StructuredOutputError> {
        if matches!(schema.get("additionalProperties"), Some(Value::Bool(true))) {
            return Err(StructuredOutputError::UnsupportedJsonSchema(String::from(
                "`additionalProperties: true` is not supported by the local fallback",
            )));
        }
        let properties = schema.get("properties").map_or_else(
            || Ok(BTreeMap::new()),
            |value| match value {
                Value::Object(map) => Ok(map.clone().into_iter().collect::<BTreeMap<_, _>>()),
                _ => Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`properties` must be an object",
                ))),
            },
        )?;
        if properties.len() > 5 {
            return Err(StructuredOutputError::UnsupportedJsonSchema(format!(
                "object schemas with more than 5 properties are not supported by the local fallback (got {})",
                properties.len()
            )));
        }

        let required = schema.get("required").map_or_else(
            || Ok(BTreeSet::new()),
            |value| match value {
                Value::Array(values) => values
                    .iter()
                    .map(|value| match value {
                        Value::String(value) => Ok(value.clone()),
                        _ => Err(StructuredOutputError::InvalidJsonSchema(String::from(
                            "`required` entries must be strings",
                        ))),
                    })
                    .collect::<Result<BTreeSet<_>, StructuredOutputError>>(),
                _ => Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`required` must be an array",
                ))),
            },
        )?;

        let mut required_properties = Vec::new();
        let mut optional_properties = Vec::new();
        for (key, property_schema) in properties {
            let value_rule = self.define_inline_rule(&property_schema)?;
            let property = JsonObjectProperty {
                key_literal: json_literal(&Value::String(key.clone()))?,
                value_rule,
            };
            if required.contains(key.as_str()) {
                required_properties.push(property);
            } else {
                optional_properties.push(property);
            }
        }

        let mut alternatives = Vec::new();
        let optional_masks = 1usize << optional_properties.len();
        for mask in 0..optional_masks {
            let mut selected = required_properties.clone();
            for (index, property) in optional_properties.iter().enumerate() {
                if mask & (1 << index) != 0 {
                    selected.push(property.clone());
                }
            }
            if selected.is_empty() {
                alternatives.push(Sequence {
                    terms: vec![Term::literal("{"), Term::literal("}")],
                });
                continue;
            }
            for order in permute_properties(selected.clone()) {
                let mut terms = vec![Term::literal("{")];
                for (index, property) in order.iter().enumerate() {
                    if index > 0 {
                        terms.push(Term::literal(","));
                    }
                    terms.push(Term::literal(property.key_literal.clone()));
                    terms.push(Term::literal(":"));
                    terms.push(Term::rule(property.value_rule.clone()));
                }
                terms.push(Term::literal("}"));
                alternatives.push(Sequence { terms });
            }
        }
        Ok(Expression::choice(alternatives))
    }

    fn lower_array(&mut self, schema: &Value) -> Result<Expression, StructuredOutputError> {
        let Some(items_schema) = schema.get("items") else {
            return Err(StructuredOutputError::UnsupportedJsonSchema(String::from(
                "array schemas require `items` in the local fallback",
            )));
        };
        let item_rule = self.define_inline_rule(items_schema)?;
        let min_items = optional_usize_field(schema, "minItems")?.unwrap_or(0);
        let max_items = optional_usize_field(schema, "maxItems")?;
        if max_items.map_or(false, |max_items| max_items < min_items) {
            return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                "`maxItems` must be greater than or equal to `minItems`",
            )));
        }

        let item_term = Term::rule(item_rule);
        let tail_term = Term::group(Expression::sequence(vec![
            Term::literal(","),
            item_term.clone(),
        ]))
        .repeat(
            min_items.saturating_sub(1),
            max_items.map(|value| value.saturating_sub(1)),
        );
        let terms = if min_items == 0 {
            vec![
                Term::literal("["),
                Term::group(Expression::sequence(vec![item_term, tail_term])).repeat(0, Some(1)),
                Term::literal("]"),
            ]
        } else {
            vec![Term::literal("["), item_term, tail_term, Term::literal("]")]
        };
        Ok(Expression::sequence(terms))
    }

    fn lower_string(&mut self, schema: &Value) -> Result<Expression, StructuredOutputError> {
        if schema.get("format").is_some() {
            return Err(StructuredOutputError::UnsupportedJsonSchema(String::from(
                "`format` is not supported by the local fallback JSON-schema path",
            )));
        }
        if let Some(pattern) = schema.get("pattern") {
            let Value::String(pattern) = pattern else {
                return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                    "`pattern` must be a string",
                )));
            };
            if schema.get("minLength").is_some() || schema.get("maxLength").is_some() {
                return Err(StructuredOutputError::UnsupportedJsonSchema(String::from(
                    "`pattern` cannot currently be combined with `minLength` or `maxLength`",
                )));
            }
            let mut regex = Grammar::from_regex(pattern)?;
            let expression = regex.rules.remove("root").ok_or_else(|| {
                StructuredOutputError::UnsupportedJsonSchema(String::from(
                    "regex lowering did not produce a root rule",
                ))
            })?;
            return Ok(Expression::sequence(vec![
                Term::literal("\""),
                Term::group(expression),
                Term::literal("\""),
            ]));
        }
        let min_length = optional_usize_field(schema, "minLength")?.unwrap_or(0);
        let max_length = optional_usize_field(schema, "maxLength")?;
        if max_length.map_or(false, |max_length| max_length < min_length) {
            return Err(StructuredOutputError::InvalidJsonSchema(String::from(
                "`maxLength` must be greater than or equal to `minLength`",
            )));
        }
        Ok(Expression::sequence(vec![
            Term::literal("\""),
            Term::rule("json-string-char").repeat(min_length, max_length),
            Term::literal("\""),
        ]))
    }

    fn define_inline_rule(&mut self, schema: &Value) -> Result<String, StructuredOutputError> {
        let rule_name = format!("schema-inline-{}", self.next_rule_id);
        self.next_rule_id += 1;
        self.lower_named_rule(rule_name.as_str(), schema)
    }
}

#[derive(Clone)]
struct JsonObjectProperty {
    key_literal: String,
    value_rule: String,
}

fn optional_usize_field(
    schema: &Value,
    field: &str,
) -> Result<Option<usize>, StructuredOutputError> {
    schema.get(field).map_or(Ok(None), |value| match value {
        Value::Number(number) => number
            .as_u64()
            .map(|value| Some(value as usize))
            .ok_or_else(|| {
                StructuredOutputError::InvalidJsonSchema(format!(
                    "`{field}` must be a non-negative integer"
                ))
            }),
        _ => Err(StructuredOutputError::InvalidJsonSchema(format!(
            "`{field}` must be a non-negative integer"
        ))),
    })
}

fn infer_schema_type(schema: &Value) -> Result<String, StructuredOutputError> {
    if let Some(value) = schema.get("type") {
        return match value {
            Value::String(value) => Ok(value.clone()),
            Value::Array(values) if values.len() == 1 => match values.first() {
                Some(Value::String(value)) => Ok(value.clone()),
                _ => Err(StructuredOutputError::UnsupportedJsonSchema(String::from(
                    "`type` arrays must contain exactly one string in the local fallback",
                ))),
            },
            _ => Err(StructuredOutputError::InvalidJsonSchema(String::from(
                "`type` must be a string",
            ))),
        };
    }
    if schema.get("properties").is_some() {
        return Ok(String::from("object"));
    }
    if schema.get("items").is_some() {
        return Ok(String::from("array"));
    }
    Err(StructuredOutputError::UnsupportedJsonSchema(String::from(
        "schema must declare `type`, `properties`, `items`, `const`, `enum`, `oneOf`, or `anyOf` in the local fallback",
    )))
}

fn json_literal(value: &Value) -> Result<String, StructuredOutputError> {
    serde_json::to_string(value).map_err(|error| {
        StructuredOutputError::InvalidJsonSchema(format!(
            "failed to serialize JSON literal for fallback grammar: {error}"
        ))
    })
}

fn permute_properties(properties: Vec<JsonObjectProperty>) -> Vec<Vec<JsonObjectProperty>> {
    let mut output = Vec::new();
    let mut used = vec![false; properties.len()];
    let mut current = Vec::new();
    permute_properties_inner(properties.as_slice(), &mut used, &mut current, &mut output);
    output
}

fn permute_properties_inner(
    properties: &[JsonObjectProperty],
    used: &mut [bool],
    current: &mut Vec<JsonObjectProperty>,
    output: &mut Vec<Vec<JsonObjectProperty>>,
) {
    if current.len() == properties.len() {
        output.push(current.clone());
        return;
    }
    for index in 0..properties.len() {
        if used[index] {
            continue;
        }
        used[index] = true;
        current.push(properties[index].clone());
        permute_properties_inner(properties, used, current, output);
        current.pop();
        used[index] = false;
    }
}

fn sanitize_rule_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn install_json_value_rules(rules: &mut BTreeMap<String, Expression>) {
    if rules.contains_key("json-value") {
        return;
    }

    let safe_string_character = CharacterClass {
        negated: true,
        ranges: vec![
            CharacterRange {
                start: '"',
                end: '"',
            },
            CharacterRange {
                start: '\\',
                end: '\\',
            },
            CharacterRange {
                start: '\u{0000}',
                end: '\u{001f}',
            },
        ],
    };
    let hex_digit = CharacterClass {
        negated: false,
        ranges: vec![
            CharacterRange {
                start: '0',
                end: '9',
            },
            CharacterRange {
                start: 'a',
                end: 'f',
            },
            CharacterRange {
                start: 'A',
                end: 'F',
            },
        ],
    };

    rules.insert(
        String::from("json-string-char"),
        Expression::choice(vec![
            Sequence {
                terms: vec![Term::class(safe_string_character)],
            },
            Sequence {
                terms: vec![
                    Term::literal("\\"),
                    Term::group(Expression::choice(vec![
                        Sequence {
                            terms: vec![Term::class(CharacterClass {
                                negated: false,
                                ranges: vec![
                                    CharacterRange {
                                        start: '"',
                                        end: '"',
                                    },
                                    CharacterRange {
                                        start: '\\',
                                        end: '\\',
                                    },
                                    CharacterRange {
                                        start: '/',
                                        end: '/',
                                    },
                                    CharacterRange {
                                        start: 'b',
                                        end: 'b',
                                    },
                                    CharacterRange {
                                        start: 'f',
                                        end: 'f',
                                    },
                                    CharacterRange {
                                        start: 'n',
                                        end: 'n',
                                    },
                                    CharacterRange {
                                        start: 'r',
                                        end: 'r',
                                    },
                                    CharacterRange {
                                        start: 't',
                                        end: 't',
                                    },
                                ],
                            })],
                        },
                        Sequence {
                            terms: vec![
                                Term::literal("u"),
                                Term::class(hex_digit.clone()).repeat(4, Some(4)),
                            ],
                        },
                    ])),
                ],
            },
        ]),
    );
    rules.insert(
        String::from("json-string"),
        Expression::sequence(vec![
            Term::literal("\""),
            Term::rule("json-string-char").repeat(0, None),
            Term::literal("\""),
        ]),
    );
    rules.insert(
        String::from("json-digit"),
        Expression::sequence(vec![Term::class(CharacterClass {
            negated: false,
            ranges: vec![CharacterRange {
                start: '0',
                end: '9',
            }],
        })]),
    );
    rules.insert(
        String::from("json-one-nine"),
        Expression::sequence(vec![Term::class(CharacterClass {
            negated: false,
            ranges: vec![CharacterRange {
                start: '1',
                end: '9',
            }],
        })]),
    );
    rules.insert(
        String::from("json-unsigned-int"),
        Expression::choice(vec![
            Sequence {
                terms: vec![Term::literal("0")],
            },
            Sequence {
                terms: vec![
                    Term::rule("json-one-nine"),
                    Term::rule("json-digit").repeat(0, None),
                ],
            },
        ]),
    );
    rules.insert(
        String::from("json-integer"),
        Expression::choice(vec![
            Sequence {
                terms: vec![Term::rule("json-unsigned-int")],
            },
            Sequence {
                terms: vec![Term::literal("-"), Term::rule("json-unsigned-int")],
            },
        ]),
    );
    rules.insert(
        String::from("json-number"),
        Expression::sequence(vec![
            Term::rule("json-integer"),
            Term::group(Expression::sequence(vec![
                Term::literal("."),
                Term::rule("json-digit").repeat(1, None),
            ]))
            .repeat(0, Some(1)),
            Term::group(Expression::sequence(vec![
                Term::class(CharacterClass {
                    negated: false,
                    ranges: vec![
                        CharacterRange {
                            start: 'e',
                            end: 'e',
                        },
                        CharacterRange {
                            start: 'E',
                            end: 'E',
                        },
                    ],
                }),
                Term::group(Expression::choice(vec![
                    Sequence {
                        terms: vec![Term::literal("+")],
                    },
                    Sequence {
                        terms: vec![Term::literal("-")],
                    },
                ]))
                .repeat(0, Some(1)),
                Term::rule("json-digit").repeat(1, None),
            ]))
            .repeat(0, Some(1)),
        ]),
    );
    rules.insert(
        String::from("json-array"),
        Expression::choice(vec![
            Sequence {
                terms: vec![Term::literal("["), Term::literal("]")],
            },
            Sequence {
                terms: vec![
                    Term::literal("["),
                    Term::rule("json-value"),
                    Term::group(Expression::sequence(vec![
                        Term::literal(","),
                        Term::rule("json-value"),
                    ]))
                    .repeat(0, None),
                    Term::literal("]"),
                ],
            },
        ]),
    );
    rules.insert(
        String::from("json-object"),
        Expression::choice(vec![
            Sequence {
                terms: vec![Term::literal("{"), Term::literal("}")],
            },
            Sequence {
                terms: vec![
                    Term::literal("{"),
                    Term::rule("json-string"),
                    Term::literal(":"),
                    Term::rule("json-value"),
                    Term::group(Expression::sequence(vec![
                        Term::literal(","),
                        Term::rule("json-string"),
                        Term::literal(":"),
                        Term::rule("json-value"),
                    ]))
                    .repeat(0, None),
                    Term::literal("}"),
                ],
            },
        ]),
    );
    rules.insert(
        String::from("json-value"),
        Expression::choice(vec![
            Sequence {
                terms: vec![Term::rule("json-object")],
            },
            Sequence {
                terms: vec![Term::rule("json-array")],
            },
            Sequence {
                terms: vec![Term::rule("json-string")],
            },
            Sequence {
                terms: vec![Term::rule("json-number")],
            },
            Sequence {
                terms: vec![Term::literal("true")],
            },
            Sequence {
                terms: vec![Term::literal("false")],
            },
            Sequence {
                terms: vec![Term::literal("null")],
            },
        ]),
    );
}

#[cfg(test)]
mod tests {
    use super::{
        StructuredGrammarSyntax, StructuredOutputKind, StructuredOutputMatchStatus,
        StructuredOutputMatcher, StructuredOutputParser, StructuredOutputRequest,
        StructuredOutputValue, StructuredTaggedVariant, local_structured_output_capabilities,
        local_structured_output_parsers,
    };
    use serde_json::json;

    #[test]
    fn local_structured_output_parser_inventory_is_stable() {
        assert_eq!(
            local_structured_output_parsers(),
            vec![
                StructuredOutputParser::ChoiceSet,
                StructuredOutputParser::RegexSubset,
                StructuredOutputParser::GbnfSubset,
                StructuredOutputParser::JsonSchemaSubset,
                StructuredOutputParser::JsonObject,
                StructuredOutputParser::TaggedJsonSchema,
            ]
        );
    }

    #[test]
    fn local_structured_output_capability_inventory_is_stable() {
        let capabilities = local_structured_output_capabilities();
        assert_eq!(
            capabilities
                .iter()
                .map(|capability| capability.kind)
                .collect::<Vec<_>>(),
            vec![
                StructuredOutputKind::Choice,
                StructuredOutputKind::Regex,
                StructuredOutputKind::Grammar,
                StructuredOutputKind::JsonSchema,
                StructuredOutputKind::JsonObject,
                StructuredOutputKind::TaggedStructure,
            ]
        );
        assert!(
            capabilities
                .iter()
                .all(|capability| capability.support_level.label() == "fallback"),
            "local capability inventory should currently advertise fallback support"
        );
    }

    #[test]
    fn gbnf_matcher_accepts_literal_choices_and_prefixes() -> Result<(), Box<dyn std::error::Error>>
    {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::Grammar {
            syntax: StructuredGrammarSyntax::Gbnf,
            grammar: String::from("root ::= choice\nchoice ::= \"yes\" | \"no\"\n"),
        })?;

        assert_eq!(
            matcher.classify("y").status,
            StructuredOutputMatchStatus::Prefix
        );
        assert_eq!(
            matcher.classify("yes").status,
            StructuredOutputMatchStatus::Complete
        );
        assert_eq!(
            matcher.classify("maybe").status,
            StructuredOutputMatchStatus::Invalid
        );
        Ok(())
    }

    #[test]
    fn gbnf_matcher_reports_open_ended_grammar_can_continue()
    -> Result<(), Box<dyn std::error::Error>> {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::Grammar {
            syntax: StructuredGrammarSyntax::Gbnf,
            grammar: String::from("root ::= [a-z]+\n"),
        })?;

        let matched = matcher.classify("a");
        assert_eq!(matched.status, StructuredOutputMatchStatus::Complete);
        assert!(matched.can_continue);
        Ok(())
    }

    #[test]
    fn json_schema_lowerer_supports_small_objects() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::JsonSchema {
            name: Some(String::from("weather")),
            schema: json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string", "minLength": 1 },
                    "ok": { "type": "boolean" }
                },
                "required": ["city", "ok"],
                "additionalProperties": false
            }),
        })?;

        assert_eq!(
            matcher.classify("{\"city\":\"A\",\"ok\":true}").status,
            StructuredOutputMatchStatus::Complete
        );
        assert_eq!(
            matcher.classify("{\"ok\":true,\"city\":\"A\"}").status,
            StructuredOutputMatchStatus::Complete
        );
        assert_eq!(
            matcher.classify("{\"city\":\"A\"}").status,
            StructuredOutputMatchStatus::Invalid
        );
        Ok(())
    }

    #[test]
    fn json_object_matcher_accepts_nested_json() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::JsonObject)?;

        assert_eq!(
            matcher.classify("{").status,
            StructuredOutputMatchStatus::Prefix
        );
        assert_eq!(
            matcher.classify("{\"items\":[1,true,null]}").status,
            StructuredOutputMatchStatus::Complete
        );
        Ok(())
    }

    #[test]
    fn choice_matcher_materializes_discrete_values() -> Result<(), Box<dyn std::error::Error>> {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::Choice {
            values: vec![String::from("red"), String::from("green")],
        })?;

        assert_eq!(
            matcher.classify("r").status,
            StructuredOutputMatchStatus::Prefix
        );
        assert_eq!(
            matcher.materialize("green")?,
            Some(StructuredOutputValue::Choice {
                value: String::from("green")
            })
        );
        Ok(())
    }

    #[test]
    fn regex_matcher_supports_prefix_and_materialization() -> Result<(), Box<dyn std::error::Error>>
    {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::Regex {
            pattern: String::from("[A-Z]{2}[0-9]{2}"),
        })?;

        assert_eq!(
            matcher.classify("AB").status,
            StructuredOutputMatchStatus::Prefix
        );
        assert_eq!(
            matcher.materialize("AB12")?,
            Some(StructuredOutputValue::Regex {
                value: String::from("AB12")
            })
        );
        Ok(())
    }

    #[test]
    fn tagged_structure_matcher_materializes_tag_and_value()
    -> Result<(), Box<dyn std::error::Error>> {
        let matcher = StructuredOutputMatcher::compile(StructuredOutputRequest::TaggedStructure {
            name: Some(String::from("decision")),
            discriminator: String::from("kind"),
            variants: vec![
                StructuredTaggedVariant {
                    tag: String::from("approve"),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "reason": { "type": "string", "minLength": 1 }
                        },
                        "required": ["reason"],
                        "additionalProperties": false
                    }),
                },
                StructuredTaggedVariant {
                    tag: String::from("reject"),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "code": { "type": "integer" }
                        },
                        "required": ["code"],
                        "additionalProperties": false
                    }),
                },
            ],
        })?;

        let value = matcher.materialize("{\"kind\":\"approve\",\"reason\":\"ok\"}")?;
        assert_eq!(
            value,
            Some(StructuredOutputValue::TaggedStructure {
                discriminator: String::from("kind"),
                tag: String::from("approve"),
                value: json!({"kind":"approve","reason":"ok"}),
            })
        );
        Ok(())
    }
}

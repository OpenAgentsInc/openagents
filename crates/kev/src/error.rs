//! The errors a request or an artifact can raise, and the refusal code each
//! publishes when a handler answers with it.

/// The most options one question may carry.
pub const MAX_OPTIONS: usize = 255;

/// The refusal class an [`Error`] publishes on the wire.
///
/// The labels are the vocabulary `gym::row::RefusalCode` records and
/// `gym::eval::classify` reads, and the statuses match the ones `crates/lev`
/// answers the same classes with, so a reader of either door meets one
/// contract. Kev admits a bounded number of forwards at once; a request that
/// arrives when every slot is taken answers `busy` at once rather than
/// queueing without limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefusalCode {
    /// The request fails contract validation, before any call.
    InvalidRequest,
    /// A `choice` or `score` names more options than the contract admits.
    TooManyOptions,
    /// The `model` field names no loaded variant, or the loaded artifact
    /// cannot serve.
    ModelUnavailable,
    /// The state, or a question branch plus the state, exceeds the serving
    /// token budget.
    BranchTooLong,
    /// The HTTP request body exceeds the server's byte limit.
    PayloadTooLarge,
    /// The door's own runtime failed on a request it accepted: the
    /// tokenizer, the tensor runtime, or the weights it loaded.
    InferenceFailure,
    /// Every inference slot is taken; the request was not evaluated.
    Busy,
}

impl RefusalCode {
    /// The wire label, which is what `error.code` carries.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::TooManyOptions => "too_many_options",
            Self::ModelUnavailable => "model_unavailable",
            Self::BranchTooLong => "branch_too_long",
            Self::PayloadTooLarge => "payload_too_large",
            Self::InferenceFailure => "inference_failure",
            Self::Busy => "busy",
        }
    }

    /// The HTTP status a refusal of this class answers with.
    ///
    /// `model_unavailable` answers `503` rather than `422` because what is
    /// missing is the door's deployment, not the request's shape, and it can
    /// arrive with the next one. `inference_failure` answers `500` because
    /// the door accepted the request and its own runtime failed it.
    #[must_use]
    pub const fn status(self) -> u16 {
        match self {
            Self::InvalidRequest | Self::TooManyOptions => 422,
            Self::BranchTooLong | Self::PayloadTooLarge => 413,
            Self::ModelUnavailable | Self::Busy => 503,
            Self::InferenceFailure => 500,
        }
    }
}

/// The forward bound a `busy` refusal names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Bound {
    /// The forward slots the host allows across every variant.
    Host,
    /// The forward slots one variant's working set allows on this host.
    Variant(String),
    /// The working-memory budget, counted in MiB.
    Memory,
}

impl std::fmt::Display for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host => f.write_str("the host's forward slots"),
            Self::Variant(model) => write!(f, "the `{model}` forward slots"),
            Self::Memory => f.write_str("the working-memory budget in MiB"),
        }
    }
}

/// A request that failed validation, or an artifact that failed to load.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The request body is not JSON, or a field has the wrong shape.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The `questions` map is empty.
    #[error("questions must hold at least one question")]
    EmptyQuestions,
    /// A question's `type` is not `noul`, `choice`, or `score`.
    #[error("question `{id}` has unsupported type `{found}`")]
    UnsupportedQuestionType { id: String, found: String },
    /// A choice question's `criteria` is missing or is not an object.
    #[error("choice question `{id}` needs a criteria object")]
    MissingChoiceCriteria { id: String },
    /// A score question's `criteria` is missing or is not an array.
    #[error("score question `{id}` needs a criteria array")]
    MissingScoreCriteria { id: String },
    /// A question carries more options than the contract admits.
    #[error("question `{id}` has {count} options; at most {MAX_OPTIONS} are allowed")]
    TooManyOptions { id: String, count: usize },
    /// A score question carries fewer than two levels.
    #[error("score question `{id}` needs at least two levels, has {count}")]
    TooFewLevels { id: String, count: usize },
    /// The state alone exceeds the encoding's token budget under `strict`.
    #[error("state exceeds {max} tokens: {tokens}")]
    StateTooLong { tokens: usize, max: usize },
    /// A question branch plus the state exceeds the token budget.
    #[error("branch too long: {tokens} > {max}")]
    BranchTooLong { tokens: usize, max: usize },
    /// The request carries more questions than the door admits in one pass.
    #[error("{count} questions; at most {max} are admitted in one request")]
    TooManyQuestions { count: usize, max: usize },
    /// The request's options, summed over every question, exceed the bound.
    #[error("{count} options across all questions; at most {max} are admitted")]
    TooManyTotalOptions { count: usize, max: usize },
    /// The packed sequence exceeds the door's total token budget; the
    /// attention mask it would need is quadratic in that length.
    #[error(
        "packed sequence of {tokens} tokens exceeds {max}; its attention mask would need about {attention_bytes} bytes"
    )]
    TooManyTokens {
        tokens: usize,
        max: usize,
        attention_bytes: usize,
    },
    /// The delimiter tokens the request's shape alone packs to exceed the
    /// door's total token budget, before any text is tokenized.
    #[error(
        "the request's {questions} questions and {options} options pack to at least {floor} tokens; at most {max} are admitted"
    )]
    SequenceFloorTooLong {
        questions: usize,
        options: usize,
        floor: usize,
        max: usize,
    },
    /// A forward bound is saturated: the host's slots, the named variant's
    /// slots, or the working-memory budget. `in_flight` and `limit` count
    /// forwards for the first two and MiB for the budget.
    #[error("busy: {bound} at its limit, {in_flight} of {limit} in use")]
    Busy {
        bound: Bound,
        in_flight: usize,
        limit: usize,
    },
    /// The `model` field names no loaded variant.
    #[error("unknown model `{model}`; known: {}", known.join(", "))]
    UnknownModel { model: String, known: Vec<String> },
    /// The tokenizer failed to load or to encode.
    #[error("tokenizer: {0}")]
    Tokenize(String),
    /// A model, adapter, or head artifact is missing or malformed.
    #[error("artifact: {0}")]
    Artifact(String),
    /// A tensor operation failed inside the runtime.
    #[error(transparent)]
    Candle(#[from] candle_core::Error),
    /// A request field failed JSON decoding.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Error {
    /// The refusal class this error publishes when a handler answers with it.
    ///
    /// `Json` lands under [`RefusalCode::InvalidRequest`] because the only
    /// JSON failures a request handler can raise are decodings of the
    /// request itself; the artifact's own JSON is read while loading, before
    /// any request arrives.
    #[must_use]
    pub fn refusal(&self) -> RefusalCode {
        match self {
            Self::InvalidRequest(_)
            | Self::EmptyQuestions
            | Self::UnsupportedQuestionType { .. }
            | Self::MissingChoiceCriteria { .. }
            | Self::MissingScoreCriteria { .. }
            | Self::TooFewLevels { .. }
            | Self::Json(_) => RefusalCode::InvalidRequest,
            Self::TooManyOptions { .. } | Self::TooManyTotalOptions { .. } => {
                RefusalCode::TooManyOptions
            }
            Self::TooManyQuestions { .. } => RefusalCode::InvalidRequest,
            Self::StateTooLong { .. }
            | Self::BranchTooLong { .. }
            | Self::TooManyTokens { .. }
            | Self::SequenceFloorTooLong { .. } => RefusalCode::BranchTooLong,
            Self::Busy { .. } => RefusalCode::Busy,
            Self::UnknownModel { .. } | Self::Artifact(_) => RefusalCode::ModelUnavailable,
            Self::Tokenize(_) | Self::Candle(_) => RefusalCode::InferenceFailure,
        }
    }

    /// The question id an error names, when it names one.
    #[must_use]
    pub fn question(&self) -> Option<&str> {
        match self {
            Self::UnsupportedQuestionType { id, .. }
            | Self::MissingChoiceCriteria { id }
            | Self::MissingScoreCriteria { id }
            | Self::TooManyOptions { id, .. }
            | Self::TooFewLevels { id, .. } => Some(id.as_str()),
            _ => None,
        }
    }
}

/// One result of validating or evaluating.
pub type Result<T> = std::result::Result<T, Error>;

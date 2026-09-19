//! The errors a request or an artifact can raise.

/// The most options one question may carry.
pub const MAX_OPTIONS: usize = 255;

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
    /// The `model` field names no loaded variant.
    #[error("unknown model `{model}`; loaded: {}", known.join(", "))]
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

/// One result of validating or evaluating.
pub type Result<T> = std::result::Result<T, Error>;

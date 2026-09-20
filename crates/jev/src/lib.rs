//! A client for TypeSafe AI's System One API, the door to the Jev model.
//!
//! A System One model reads one state, answers a map of typed questions about
//! it, and returns one typed answer per question with probabilities. It writes
//! no text. Your code owns the workflow and asks the model only where the
//! decision needs semantic understanding.
//!
//! The crate speaks the API directly:
//!
//! ```http
//! POST https://api.typesafe.ai/v1/systemone
//! Authorization: Bearer <API_KEY>
//! Content-Type: application/json
//! ```
//!
//! It mirrors TypeSafe's official Python and JavaScript SDKs: the same
//! defaults, the same retry policy, the same headers, the same message
//! extraction, and the same lenient decoding.
//!
//! # Asking a question
//!
//! ```no_run
//! use jev::{Choice, Client, Config, Noul, Questions, SystemOneRequest};
//!
//! # async fn ask() -> jev::Result<()> {
//! let client = Client::new(Config::new().api_key("ts-secret-value"))?;
//! let questions = Questions::new()
//!     .with("refund", Noul::new("Does the customer ask for money back?"))
//!     .with(
//!         "department",
//!         Choice::default()
//!             .option("billing", "Charges, invoices, and refunds")
//!             .option("technical", "Bugs and outages"),
//!     );
//! let response = client
//!     .system_one(SystemOneRequest::new("I was charged twice.", questions))
//!     .await?;
//! let refund = response.noul("refund")?;
//! if refund.noul > 0.8 {
//!     // Act on the judgment. The threshold belongs to the decision, not here.
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Where a setting comes from
//!
//! An explicit value wins, then the environment variable, then the default. An
//! environment value that is empty or holds only whitespace is ignored.
//!
//! | Setting | Variable | Default |
//! | --- | --- | --- |
//! | Key | `TYPESAFE_API_KEY` | none; required |
//! | Base URL | `TYPESAFE_BASE_URL` | `https://api.typesafe.ai` |
//! | Model | `TYPESAFE_DEFAULT_MODEL` | `jev-latest` |
//! | Timeout per attempt | none | 10 seconds |
//!
//! # Logging
//!
//! Every attempt and every retry is one event under the `jev` target: `info`
//! for the status, the elapsed time, and the request id, and `debug` for the
//! headers and the body. A credential header is masked to its scheme and last
//! four characters. No event, error, or `Display` output carries the key.
//!
//! # The public surface
//!
//! This list is the crate's surface. A removed or renamed item fails this test,
//! the way the Python SDK's snapshot fails on one.
//!
//! ```
//! use jev::{
//!     Answer, ApiError, ApiErrorKind, ApiKey, Choice, ChoiceAnswer, Client, Config, Entry, Error,
//!     ListOptions, ModelCard, Models, Noul, NoulAnswer, NoulCriteria, Question, Questions,
//!     RawResponse, ResponseBody, Result, RetryPolicy, RetryPredicate, Score, ScoreAnswer,
//!     SystemOneRequest, SystemOneResponse, Usage, defaults, env, parse_retry_after,
//!     parse_retry_after_at,
//! };
//!
//! // Constants.
//! let _: &str = jev::VERSION;
//! let _: (&str, &str, std::time::Duration) = (defaults::BASE_URL, defaults::MODEL, defaults::TIMEOUT);
//! let _: (&str, &str, &str, &str) = (env::API_KEY, env::BASE_URL, env::DEFAULT_MODEL, env::LOG_LEVEL);
//!
//! // The client and its settings.
//! let _ = Client::new;
//! let _ = Client::from_env;
//! let _ = Client::base_url;
//! let _ = Client::default_model;
//! let _ = Client::timeout;
//! let _ = Client::retry;
//! let _ = Client::default_headers;
//! let _ = Client::models;
//! let _ = Client::system_one;
//! let _ = Client::system_one_raw;
//! let _ = Config::new;
//! let _ = Config::api_key::<&str>;
//! let _ = Config::base_url::<&str>;
//! let _ = Config::default_model::<&str>;
//! let _ = Config::timeout;
//! let _ = Config::retry;
//! let _ = Config::default_headers;
//! let _ = Config::http_client;
//! let _ = ApiKey::new::<&str>;
//! let _ = ApiKey::expose;
//! let _ = Models::list;
//! let _ = Models::list_raw;
//! let _ = ListOptions::new;
//! let _ = ListOptions::retry;
//! let _ = ListOptions::timeout;
//! let _ = ListOptions::headers;
//!
//! // Questions.
//! let _ = Entry::json::<&str>;
//! let _ = Entry::to_value;
//! let _ = Entry::is_null;
//! let _ = Noul::new::<&str>;
//! let _ = Noul::with_criteria::<&str>;
//! let _ = NoulCriteria::new;
//! let _ = NoulCriteria::when_true::<&str>;
//! let _ = NoulCriteria::when_false::<&str>;
//! let _ = Choice::new::<&str>;
//! let _ = Choice::option::<&str, &str>;
//! let _ = Choice::bare_option::<&str>;
//! let _ = Score::new::<&str>;
//! let _ = Score::level::<&str>;
//! let _ = Question::kind;
//! let _ = Questions::new;
//! let _ = Questions::with::<&str, Noul>;
//! let _ = Questions::insert::<&str, Noul>;
//! let _ = Questions::get;
//! let _ = Questions::len;
//! let _ = Questions::is_empty;
//! let _ = Questions::iter;
//! let _ = Questions::validate;
//!
//! // Requests.
//! let _ = SystemOneRequest::new::<&str>;
//! let _ = SystemOneRequest::model::<&str>;
//! let _ = SystemOneRequest::retry;
//! let _ = SystemOneRequest::timeout;
//! let _ = SystemOneRequest::headers;
//! let _ = SystemOneRequest::extra_body;
//!
//! // Answers.
//! let _ = SystemOneResponse::decode;
//! let _ = SystemOneResponse::raw;
//! let _ = SystemOneResponse::request_id;
//! let _ = SystemOneResponse::noul;
//! let _ = SystemOneResponse::choice;
//! let _ = SystemOneResponse::score;
//! let _ = SystemOneResponse::nouls;
//! let _ = SystemOneResponse::choices;
//! let _ = SystemOneResponse::scores;
//! let _ = Answer::kind;
//! let _ = RawResponse::read;
//! let _ = RawResponse::body;
//! let _ = RawResponse::text;
//! let _ = RawResponse::request_id;
//!
//! // Retries and errors.
//! let _: Option<RetryPredicate> = RetryPolicy::default().predicate;
//! let _ = RetryPolicy::validate;
//! let _ = RetryPolicy::retries_status;
//! let _ = RetryPolicy::retries_error;
//! let _ = RetryPolicy::delay;
//! let _ = RetryPolicy::delay_with;
//! let _ = parse_retry_after;
//! let _ = parse_retry_after_at;
//! let _ = ApiErrorKind::of;
//! let _ = ApiError::message;
//! let _ = ApiError::retry_after;
//! let _ = Error::request_id;
//! let _ = ResponseBody::as_json;
//! let _ = ResponseBody::as_text;
//!
//! // Values a caller builds by hand.
//! let _: Result<()> = Ok(());
//! let _ = Usage::default();
//! let _ = ChoiceAnswer {
//!     choice: "billing".into(),
//!     confidence: 0.82,
//!     probabilities: Default::default(),
//! };
//! let _ = NoulAnswer { noul: 0.92, selected: None };
//! let _ = ScoreAnswer {
//!     score: 1.6,
//!     confidence: 0.78,
//!     selected: None,
//!     legend: Default::default(),
//!     probabilities: Default::default(),
//! };
//! let _ = ModelCard {
//!     name: "jev-latest".into(),
//!     description: String::new(),
//!     release_date: String::new(),
//! };
//! ```

mod answers;
mod client;
mod config;
mod error;
mod models;
mod questions;
mod retry;
mod transport;

pub use answers::{
    Answer, ChoiceAnswer, NoulAnswer, RawResponse, ScoreAnswer, SystemOneResponse, Usage,
};
#[cfg(feature = "blocking")]
pub use client::BlockingClient;
pub use client::{Client, SystemOneRequest};
pub use config::{ApiKey, Config};
pub use error::{ApiError, ApiErrorKind, Error, ResponseBody};
pub use models::{ListOptions, ModelCard, Models};
pub use questions::{Choice, Entry, Noul, NoulCriteria, Question, Questions, Score};
pub use retry::{RetryPolicy, RetryPredicate, parse_retry_after, parse_retry_after_at};

/// The crate version, which every request reports in `User-Agent` and
/// `X-TypeSafe-SDK`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What a call returns.
pub type Result<T> = std::result::Result<T, Error>;

/// What a setting falls back to when neither a caller nor the environment sets
/// it. These are the values both official SDKs ship.
pub mod defaults {
    use std::time::Duration;

    /// The API root.
    pub const BASE_URL: &str = "https://api.typesafe.ai";

    /// The model a request that names none asks.
    pub const MODEL: &str = "jev-latest";

    /// How long one attempt may take.
    pub const TIMEOUT: Duration = Duration::from_secs(10);
}

/// The environment variables a client reads.
pub mod env {
    /// The key to send.
    pub const API_KEY: &str = "TYPESAFE_API_KEY";

    /// The API root.
    pub const BASE_URL: &str = "TYPESAFE_BASE_URL";

    /// The model a request that names none asks.
    pub const DEFAULT_MODEL: &str = "TYPESAFE_DEFAULT_MODEL";

    /// The level both official SDKs read. This crate logs through `tracing`,
    /// where the subscriber sets the level, so the name is here for parity and
    /// the client does not read it.
    pub const LOG_LEVEL: &str = "TYPESAFE_LOG_LEVEL";
}

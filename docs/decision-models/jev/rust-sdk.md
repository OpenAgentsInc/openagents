# Jev Rust SDK design

**Status (2026-09-16):** proposed. The owner asked for a Rust crate that
mirrors TypeSafe's official Python and JavaScript SDKs, with the same
methods and the same behavior, so that Coder and the other Rust binaries call
Jev through one typed client. Nothing in this document is built yet; the
[to-do list](todo.md) holds the work items.

The crate speaks the official API directly:

```http
POST https://api.typesafe.ai/v1/systemone
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

The crate depends on nothing from the Vercel AI SDK or the AI Gateway: no
Gateway feature, no `boolean` alias for a Noul, no provider metadata, no
Gateway headers or wire types. The Gateway evaluation path is documented in
the [knowledge base](knowledge-base.md) for completeness only. The owner
chose the official API on 2026-09-16 and named the Gateway as a possible
backup transport for a later, separate piece of work.

## Crate

| Field | Value |
| --- | --- |
| Path | `crates/jev` |
| Package and library name | `jev` |
| Version | the workspace version, as every crate here |
| License | the workspace license |
| Edition | the workspace edition |
| Dependencies | `reqwest` with `json` and `rustls-tls` and no default features, `serde`, `serde_json`, `thiserror`, `tokio` with `time`; `tracing` for logging; `rand` for jitter |

The name `jev` is a working name. The official SDKs are named for TypeSafe,
and `typesafe-sdk` on crates.io belongs to TypeSafe if they publish one. The
owner decides the crate name before the first commit; the module layout
below does not depend on it.

Ten crates in this workspace already depend on `reqwest` with the same
feature set, so the crate adds no new locked dependency for HTTP. The
ratchet's `deps.locked` count still has to hold or fall in the commit that
adds the crate. `cargo deny` allows MIT and Apache-2.0, which cover
everything above.

The crate is a client library. It holds no Coder policy, no question
constants, and no threshold. Those belong to the crate that owns the
decision, in one reviewable module per decision, as the
[integration map](integration-map.md) lays out.

## Public surface

The table maps every public member of the two official SDKs to its Rust
name. The Rust names follow the JavaScript SDK's shapes where the two differ,
because Rust's generics can preserve criteria keys the way TypeScript does,
and follow Python's `Response` conveniences where they add something.

### Client and configuration

| JavaScript | Python | Rust |
| --- | --- | --- |
| `new TypeSafeClient(config)` | `TypeSafeClient(...)`, `AsyncTypeSafeClient(...)` | `Client::new(Config)` and `Client::from_env()`; one async client on `tokio`, with a blocking wrapper `BlockingClient` behind a `blocking` feature |
| `config.apiKey` | `api_key` | `Config::api_key(SecretString)` |
| `config.baseURL` | `base_url` | `Config::base_url(Url)` |
| `config.defaultModel` | `model` | `Config::default_model(String)` |
| `config.timeout` | `timeout` | `Config::timeout(Duration)`, per attempt |
| `config.retry` | `retry` | `Config::retry(RetryPolicy)` |
| `config.defaultHeaders` | `headers` | `Config::default_headers(HeaderMap)` |
| `config.logLevel`, `config.logger` | `TYPESAFE_LOG_LEVEL` and the `typesafe_sdk` logger | `tracing` spans and events under the `jev` target; no level option, because the subscriber decides |
| `config.fetch` | `transport`, `http_client` | `Config::http_client(reqwest::Client)` |
| `config.dangerouslyAllowBrowser` | none | none; a Rust client never runs in a browser page |
| `ENV`, `EnvVar` | `constants` | `jev::env` with `API_KEY`, `BASE_URL`, `DEFAULT_MODEL`, `LOG_LEVEL` |
| `DEFAULT_BASE_URL`, `DEFAULT_MODEL`, `DEFAULT_TIMEOUT_MS` | `DEFAULT_BASE_URL`, `DEFAULT_MODEL`, `DEFAULT_TIMEOUT` | `jev::defaults` with the same three values |
| `VERSION` | `__version__` | `jev::VERSION` |
| `client.models` | `client.models` | `client.models()` returning `Models` |
| public readonly fields | none | getters: `base_url()`, `default_model()`, `timeout()`, `retry()`, `default_headers()` |

Resolution order is the same as both SDKs: an explicit value, then the
environment variable, then the default. Empty and whitespace-only
environment values are ignored. Trailing slashes are stripped from the base
URL. A missing key, a non-positive timeout, or a retry field out of range is
an `Error::Config` at construction.

### Questions

| JavaScript | Python | Rust |
| --- | --- | --- |
| `EntryType` | `JSONContent` | `Entry`: an enum over `String`, `serde_json::Map`, `Vec<serde_json::Value>`, and `Null`, with `From` impls from `&str`, `String`, `serde_json::Value`, and any `Serialize` through `Entry::json(&T)` |
| `JsonValue` | `JSONValue` | `serde_json::Value` |
| `noul(instructions, criteria?)` | `Noul(instructions=, criteria=)` | `Noul::new(instructions)` and `Noul::with_criteria(instructions, NoulCriteria)` |
| `choice(instructions, criteria)` | `Choice(instructions=, criteria=)` | `Choice::new(instructions, criteria)` where `criteria` is an `IndexMap<String, Option<Entry>>`; a builder `Choice::option(name, description)` keeps insertion order |
| `score(instructions, criteria)` | `Score(instructions=, criteria=)` | `Score::new(instructions, Vec<Option<Entry>>)` |
| `NoulQuestion.criteria` | `NoulCriteria` | `NoulCriteria { r#true: Option<Entry>, r#false: Option<Entry> }` |
| `Question` | `Question`, `QuestionModel` | `Question` enum tagged by `type`, serializing to the wire shape; raw dictionaries are `serde_json::Value` through `Question::Raw` |
| `Questions` | `Questions` | `Questions`, an `IndexMap<String, Question>` with `insert` helpers |
| `validateQuestions` | `normalize_questions` | `Questions::validate()`, run by `system_one` before any I/O |

Validation before I/O matches both SDKs and adds the two provider limits the
AI SDK enforces: an empty question map, a Score with fewer than two levels,
a Choice with more than 255 options, and a Score with more than 10 levels
are each `Error::Question` naming the question id.

### Requests

| JavaScript | Python | Rust |
| --- | --- | --- |
| `client.systemOne({ state, questions, model? }, options?)` | `client.system_one(state, questions, *, model, retry, timeout, extra_headers, extra_body)` | `client.system_one(SystemOneRequest)` returning `Result<SystemOneResponse>`; `SystemOneRequest::new(state, questions)` with `.model()`, `.retry()`, `.timeout()`, `.headers()`, `.extra_body()` |
| `RequestOptions.signal` | none | cancellation by dropping the future, the Rust convention; a `CancellationToken` field is optional |
| `RequestOptions.timeout` | `timeout` | per-attempt `Duration` override |
| `RequestOptions.retry` | `retry` | `RetryPolicy` override, merged over the client policy field by field |
| `RequestOptions.headers` | `extra_headers` | `HeaderMap` merged over the defaults; authentication, `Accept`, the content type, and the SDK headers stay protected |
| additional forwarded properties | `extra_body` | `extra_body: serde_json::Map`, shallow-merged last, last write wins |
| `client.models.list(options)` | `client.models.list(*, retry, timeout, extra_headers)` | `client.models().list(ListOptions)` returning `Result<Vec<ModelCard>>` |

Every request sends the same headers the official SDKs send: `Authorization`,
`Accept`, `Content-Type` on a body, `User-Agent` and `X-TypeSafe-SDK` as
`jev-rust/<version>`, `X-TypeSafe-Runtime` as `rust/<rustc version>
(<os>; <arch>)`, and `X-TypeSafe-Retry-Count` on a retry. The value of the
SDK identifier is a decision for the owner: the official SDKs send
`typesafe-sdk/<version>`, and this crate is not one of them.

### Responses

| JavaScript | Python | Rust |
| --- | --- | --- |
| `SystemOneResult<Q>` | `SystemOneResponse` | `SystemOneResponse { model, answers, usage }` plus `request_id()` and `raw()` for the status, headers, and body |
| `answers: { [K]: ResultFor<Q[K]> }` | `answers: dict[str, Answer]` | `answers: IndexMap<String, Answer>`, plus typed accessors `noul(id)`, `choice(id)`, `score(id)` that return `Result<&NoulAnswer>` and so on, with `Error::AnswerType` when the id exists with another type and `Error::MissingAnswer` when it does not |
| none | `nouls`, `choices`, `scores` | `nouls()`, `choices()`, `scores()` returning filtered iterators |
| `NoulResponse { noul }` | `NoulAnswer` | `NoulAnswer { noul: f64 }` |
| `ChoiceResponse { choice, confidence, probabilities }` | `ChoiceAnswer` | `ChoiceAnswer { choice: String, confidence: f64, probabilities: IndexMap<String, f64> }` |
| `ScoreResponse { score, confidence, legend, probabilities }` | `ScoreAnswer` with integer keys | `ScoreAnswer { score: f64, confidence: f64, legend: BTreeMap<u32, Entry>, probabilities: BTreeMap<u32, f64> }`, with keys parsed from the wire strings the way Python does |
| `Usage` | `Usage` | `Usage { input_tokens: Option<u64>, output_tokens: Option<u64> }` |
| `ModelCard` | `ModelMetadata` | `ModelCard { name, description, release_date }` |
| `APIPromise.withResponse()` | `raw_http_response`, `request_id` | `SystemOneResponse::raw()` and `request_id()` |
| `APIPromise.asResponse()` | none | `client.system_one_raw(request)` returning the `reqwest::Response` before parsing, for a caller that wants the bytes |
| `APIPromise.map(fn)` | none | none; `Result::map` |

Decoding follows the Python SDK: an unknown answer type is skipped with a
`tracing::warn!` rather than failing the whole response, an unknown field is
ignored, and a malformed known field is `Error::ResponseValidation` with a
dotted `field_path` such as `answers.tone.confidence`. A Score answer with
`probabilities` missing, as the quick start's sample shows, decodes with an
empty map rather than failing.

`NoulAnswer` and `ScoreAnswer` also decode an optional `selected` field: the
option the door's answer was, when a served calibration map can leave the
reported numbers naming a different one. The hosted API does not emit it;
Lev does. [`../2026-09-20-score-contract.md`](../2026-09-20-score-contract.md)
states which field names the pick on each answer kind and what its absence
means.

### Retry policy

| Field | Default | Rust field |
| --- | --- | --- |
| Retries after the first attempt | 2 | `max_retries: u32` |
| First backoff | 500 ms | `backoff_initial: Duration` |
| Backoff cap | 5 s | `backoff_max: Duration` |
| Jitter fraction | 0.25 | `backoff_jitter: f64` |
| Retried statuses | 408, 429, 500 to 599 | `http_statuses: HashSet<u16>` |
| Honor server delay headers | true | `respect_retry_after: bool` |
| Longest honored server delay | 60 s | `max_retry_after: Duration` |
| Retry connection errors | true | `connection_errors: bool` |
| Retry timeouts | true | `timeouts: bool` |
| Total budget | none | `budget: Option<Duration>`; Python defaults to 30 s, JavaScript has none; the Rust default is none, and the owner may prefer Python's. It is a monotonic deadline for the whole call, described below |
| Predicate | none | `predicate: Option<Arc<dyn Fn(&Error) -> bool + Send + Sync>>` |

The delay for attempt `n` is `min(initial * 2^n, cap) * (1 - random() *
jitter)`, replaced by a server delay when honored and within the cap.
`retry-after-ms` wins over `Retry-After`; `Retry-After` may be seconds or an
HTTP date. Every retry is logged at `info` with the attempt number, the
reason, and the delay. Every field is validated at construction:
non-negative counts and durations, jitter between 0 and 1, statuses between
100 and 999.

`budget`, when set, is a monotonic deadline for the whole call rather than a
check only retries pass. It starts when the call dispatches, and the first
attempt, every retry, every wait between them, and the response body all
share it. Each attempt's timeout is the smaller of the call's own `timeout`
and the time the call has left, so a reply that arrives after the deadline
ends the call as `Error::Timeout` instead of succeeding; a response body
that stalls runs out of the same budget the connection does. A retry whose
wait — including one the server asks for with `Retry-After` — would reach
the deadline does not run, and the call returns the last failure it holds.
`system_one_raw` hands back the response before its body is read, but the
body stays under the same deadline: a read that runs past it fails rather
than outliving the budget. `BlockingClient` runs the same loop, so the
deadline holds there too.

### Errors

One `Error` enum with `thiserror`, matching both SDKs' hierarchies:

| Variant | Carries | Matches |
| --- | --- | --- |
| `Config(String)` | the message | `TypeSafeError` at construction |
| `Question { id, message }` | the question id | `TypeSafeError` from validation |
| `Api(ApiError)` | `status`, `headers`, `body: Option<serde_json::Value or String>`, `request_id`, `endpoint`, `kind` | `APIError` and `TypeSafeAPIError` |
| `Connection { message, source }` | the transport error | `APIConnectionError` |
| `Timeout { timeout }` | the timeout that expired: the attempt's own, or the budget's remaining time | `APITimeoutError` |
| `ResponseValidation { status, field_path, body, request_id }` | the offending field | `TypeSafeAPIResponseValidationError` |
| `AnswerType { id, expected, found }` | the typed accessor mismatch | none; Rust-only |
| `MissingAnswer { id }` | the typed accessor miss | none; Rust-only |

`ApiError::kind` is an enum: `BadRequest`, `Authentication`,
`PermissionDenied`, `NotFound`, `UnprocessableEntity`, `RateLimit { retry_after: Option<Duration> }`,
`InternalServer`, and `Other`. That gives a caller the same matching the
twelve classes give in the official SDKs without twelve types. The message
is extracted from the body the way both SDKs do it: a string body, `error`,
`error.message`, `message`, `detail`, `detail.message`, or a `detail` list
rendered as `loc: msg`, truncated at 200 characters. `Display` prints
`<METHOD> <url without credentials>: <status> <message> (request_id=...)`.

## Module layout

```text
crates/jev/
  Cargo.toml
  README.md
  src/
    lib.rs          re-exports, VERSION, defaults, env
    config.rs       Config, resolution from the environment, validation
    client.rs       Client, BlockingClient, the request loop
    questions.rs    Entry, Noul, Choice, Score, Question, Questions, validation
    answers.rs      NoulAnswer, ChoiceAnswer, ScoreAnswer, Answer, SystemOneResponse, Usage
    models.rs       Models resource, ModelCard
    retry.rs        RetryPolicy, delay computation, Retry-After parsing
    error.rs        Error, ApiError, ApiErrorKind, message extraction
    transport.rs    header assembly, redaction for logs, body encode and lenient decode
  tests/
    questions.rs    builders, serialization to the wire shape, validation errors
    answers.rs      decoding every documented response, unknown types, missing fields
    retry.rs        delay math, header parsing, status sets
    client.rs       a local HTTP server: success, each error status, retries, timeouts, headers
    live.rs         behind a `live` feature and TYPESAFE_API_KEY: one request against the API
```

The local-server tests use `tokio` and a minimal `hyper` or `axum` listener
the way `crates/coder-devin` tests its client; the choice follows whatever
that crate uses so the test dependency is already locked.

## Conformance

The official SDKs carry test suites the Rust crate can mirror case by case:
`test/client.test.ts`, `test/retry.test.ts`, `test/errors.test.ts`, and
`test/reliability.test.ts` in the JavaScript repository, and
`tests/test_clients.py`, `tests/test_retry.py`, `tests/test_errors.py`,
`tests/test_responses.py`, and `tests/test_questions.py` in the Python
repository. The Python repository also snapshots its public API surface in
`tests/__snapshots__/test_public_api_surface.ambr`; the Rust crate keeps
the same idea as a doctest that lists every public item, so a removed method
fails a test.

The JavaScript fixtures under `packages/typesafe-ai/src/__fixtures__` in the
AI SDK repository hold a recorded request and response pair verified against
the official SDK at a named commit. Copy both into `tests/fixtures/` as the
first decoding cases.

## Logging and secrets

The crate logs through `tracing` under the `jev` target. At `info`: one line
per attempt with method, path, status, elapsed time, and request id, and one
line per retry. At `debug`: headers with `Authorization`,
`proxy-authorization`, `x-api-key`, `cookie`, and `set-cookie` masked, and
bodies unmasked, the way both official SDKs do. The key is a `SecretString`
that `Debug` prints as `***`. No log line, error message, or `Display`
output ever carries the key.

## Out of scope for the crate

- Question constants and thresholds for any Coder decision. Those live with
  the decision.
- A fallback that answers questions through a language model, the way
  `system-one-adapter-python` does. That is a separate crate if Coder wants
  it, and the [integration map](integration-map.md) says when.
- The Vercel AI Gateway evaluation transport, and any type or feature that
  anticipates it. The wire protocol is recorded in the knowledge base. If
  the owner later wants the Gateway as a backup, that is a separate crate or
  a separate issue, and its `boolean` answer maps one to one onto
  `NoulAnswer`.
- Streaming, batching of unrelated states, and multilabel answers. The API
  offers none of these.

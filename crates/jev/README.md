# jev

A Rust client for TypeSafe AI's System One API, the door to the Jev model.

A System One model reads one state, answers a map of typed questions about it,
and returns one typed answer per question with probabilities. It writes no text.
Your code owns the workflow and asks the model only where the decision needs
semantic understanding.

The crate speaks the API directly:

```http
POST https://api.typesafe.ai/v1/systemone
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

It mirrors TypeSafe's official Python SDK (`typesafe-sdk` 0.6.0) and JavaScript
SDK (`@typesafe-ai/sdk` 0.6.0): the same defaults, the same retry policy, the
same headers, the same message extraction, and the same lenient decoding. The
design is `docs/jev/rust-sdk.md`, and the wire contract is
`docs/jev/knowledge-base.md`.

## Asking a question

```rust
use jev::{Choice, Client, Config, Noul, Questions, Score, SystemOneRequest};

# async fn ask() -> jev::Result<()> {
let client = Client::from_env()?;
let questions = Questions::new()
    .with("refund", Noul::new("Does the customer ask for money back?"))
    .with(
        "department",
        Choice::default()
            .option("billing", "Charges, invoices, and refunds")
            .option("technical", "Bugs and outages")
            .bare_option("other"),
    )
    .with(
        "severity",
        Score::new("How severe is the issue?", Vec::new())
            .level("Cosmetic; the product works")
            .level("Impaired; a workaround exists")
            .level("Blocking; no workaround"),
    );

let response = client
    .system_one(SystemOneRequest::new("The order was charged twice.", questions))
    .await?;

if response.noul("refund")?.noul > 0.8 {
    // Act on the judgment.
}
let department = response.choice("department")?;
println!("{} at {:.2}", department.choice, department.confidence);
# Ok(())
# }
```

The crate holds no question set, no threshold, and no policy. Those belong to
the code that owns the decision.

## Three question types

| Type | Asks | Answer |
| --- | --- | --- |
| `Noul` | Is this true? | `noul`, a probability of yes from 0 to 1 |
| `Choice` | Which one of these named options? | `choice`, `confidence`, `probabilities` |
| `Score` | Which level on this ordered rubric? | `score`, `confidence`, `legend`, `probabilities` |

Every question in one request reads the same state and is answered on its own,
so asking a question you may not need costs its tokens and little else. A Score
names two to ten levels, a Choice names at most 255 options, and a request asks
at least one question; the crate checks all three before any request.

## Where a setting comes from

An explicit value wins, then the environment variable, then the default. An
environment value that is empty or holds only whitespace is ignored, and
trailing slashes are dropped from the base URL.

| Setting | Variable | Default |
| --- | --- | --- |
| Key | `TYPESAFE_API_KEY` | none; required |
| Base URL | `TYPESAFE_BASE_URL` | `https://api.typesafe.ai` |
| Model | `TYPESAFE_DEFAULT_MODEL` | `jev-latest` |
| Timeout per attempt | none | 10 seconds |

A missing key, a base URL that is not an http or https URL, a zero timeout, and
a retry field out of range each fail at `Client::new`.

## Retries

`RetryPolicy::default()` is what both official SDKs ship: two retries, 500 ms
doubling to 5 seconds with a quarter of each wait taken off at random, and the
statuses `408`, `429`, and `500` through `599`. A `retry-after-ms` or
`Retry-After` header replaces the computed wait when it is 60 seconds or less,
and `retry-after-ms` wins. A connection failure and an attempt past its timeout
are retried. Each retry carries `X-TypeSafe-Retry-Count`.

Change one field over the default, or over the client's own policy:

```rust
use std::time::Duration;

use jev::RetryPolicy;

let policy = RetryPolicy {
    max_retries: 4,
    budget: Some(Duration::from_secs(30)),
    ..RetryPolicy::default()
};
```

The Rust default sets no total budget, which is what the JavaScript SDK does.
The Python SDK budgets 30 seconds; set `budget` to get that.

## Failures

One `Error` enum covers the dozen classes the official SDKs raise:

| Variant | Raised when |
| --- | --- |
| `Config` | A setting is missing or out of range, before any request. |
| `Question` | A question set fails its checks, before any request. |
| `Api` | The API answered outside the 2xx range. `ApiError::kind` names the class, and `RateLimit` carries the wait the server asked for. |
| `Connection` | The request never reached the API. |
| `Timeout` | One attempt ran past its timeout. |
| `ResponseValidation` | A 2xx body does not read. `field_path` names the field, such as `answers.tone.confidence`. |
| `AnswerType`, `MissingAnswer` | A typed accessor asked for an answer the response does not carry, or carries with another type. |

An API failure reads as `POST <url>: <status> <message> (request_id=…)`, with
the message taken out of the body the way both official SDKs take it and cut at
200 characters. Quote the request id when you report a failure to TypeSafe.

## Decoding

An answer type this crate does not model is skipped with a `warn` line, an
unknown field is ignored, and a Score without `probabilities` reads with an
empty map, so a newer API does not break an older client. `SystemOneResponse::raw()`
holds the status, the headers, and the bytes as they arrived, and the answers
and their probabilities keep the order the response sent them in.

## Logging and the key

Every attempt and every retry is one event under the `jev` target: `info` for
the status, the elapsed time, and the request id, and `debug` for the headers
and the body. A credential header is masked to its scheme and last four
characters. `ApiKey` prints as `***`, and no event, error, or `Display` output
carries the key.

## Features

| Feature | What it adds |
| --- | --- |
| `blocking` | `BlockingClient`, which owns a runtime and runs each call on it. |
| `live` | The test that sends one request to the API. |

## Tests

```sh
cargo test -p jev
```

The suite covers the builders and their wire shapes, the decoding of every
documented response shape and of a recorded exchange under `tests/fixtures/`,
the retry arithmetic and the server delay headers, and the client against a
local HTTP server for success, each error status, retries, timeouts, and
headers.

The live test sends one request to the API and needs a key:

```sh
set -a; . ~/work/.secrets/typesafe.env; set +a
cargo test -p jev --features live -- --nocapture
```

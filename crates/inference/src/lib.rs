//! The inference gateway library (`docs/inference/gateway.md`).
//!
//! - [`request`], [`response`], [`item`], [`event`], [`error`]: the Open
//!   Responses ([`SPEC_VERSION`]) request, response, item, streaming event,
//!   and error types.
//! - [`openagents`]: our extensions (the `openagents` request and response
//!   objects, `openagents:route` and `openagents:cost` events).
//! - [`sse`]: the server-sent event codec; [`stream`]: sequencing, folding
//!   a stream into a response, and checking a stream's order.
//! - [`chat`]: OpenAI Chat Completions types and their translation onto
//!   the same internal request, both directions, streaming included.
//! - [`router`]: model ids and task classes to ordered upstream attempts
//!   (section 5).
//! - [`run`]: the attempt loop: planned attempts sent to the adapters,
//!   fallback only before the first token, every attempt metered, and the
//!   route and cost events.
//! - [`rates`]: the public rate card and the model catalog's price rows,
//!   over the meter's rate card.
//! - [`meter`]: the measurement half (sections 5 and 6): one record per
//!   upstream attempt, live rates, and the credit ledger with burn-down
//!   alerts. Adapters report each attempt through [`meter::Recorder`].
//! - [`upstream`]: the adapters (section 4) for Vertex AI, Z.ai, the Pro
//!   door, OpenRouter, and the Vercel AI Gateway, behind one
//!   [`upstream::Upstream`] trait.
//!
//! The wire modules do no I/O; [`upstream`] is the only module that does. `docs/inference/gateway.md` is the spec; the
//! crate README lists what maps 1:1 between the two APIs and what degrades.

mod wire;

pub mod chat;
pub mod error;
pub mod event;
pub mod item;
pub mod meter;
pub mod openagents;
pub mod rates;
pub mod request;
pub mod response;
pub mod router;
pub mod run;
pub mod sse;
pub mod stream;
pub mod upstream;

/// The Open Responses specification version these types implement.
pub const SPEC_VERSION: &str = "2026-04-24";

pub use error::{ApiError, ErrorType};
pub use event::{Event, EventBody};
pub use item::{ContentPart, Item, ItemStatus, Message, MessageContent, Role};
pub use request::{CreateResponse, Input, Tool, ToolChoice};
pub use response::{Response, ResponseStatus, Usage};

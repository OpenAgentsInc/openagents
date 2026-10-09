//! OpenAI Chat Completions: the backup API, translated onto the same
//! internal request as Open Responses.
//!
//! A Chat Completions request becomes a [`CreateResponse`] plus a
//! [`ChatShape`] (how the reply must look); it routes like any other
//! request, and the result comes back through [`completion_from_response`]
//! or, streaming, [`ChunkWriter`]. The reverse direction
//! ([`from_responses_request`], [`response_from_completion`],
//! [`EventWriter`]) serves upstreams that speak only Chat Completions.
//! The crate README's table lists what maps 1:1 and what degrades.
//!
//! [`CreateResponse`]: crate::request::CreateResponse

mod stream;
mod translate;
mod types;

pub use stream::{ChunkWriter, CompletionBuilder, EventWriter, decode_chunk, encode_chunk};
pub use translate::{
    ChatShape, chat_usage, completion_from_response, finish_reason, from_responses_request,
    response_from_completion, responses_usage, to_responses_request,
};
pub use types::*;

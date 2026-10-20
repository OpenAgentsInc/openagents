//! Inference output parsing and request options for DeepSeek models.
//!
//! This is a vendored subset of `deepseek-recipe` 0.2.0 at commit
//! `935817c66f5d6e64838f419692545f06787e485f`. It keeps the streamed output
//! parser ([`stream`]), the request options and output constraints
//! ([`request`]), and leaves out the OpenAI and Anthropic protocol adapters,
//! image handling, and demo servers. Psionic maps its own API types onto
//! these pieces. `docs/dependencies.md` lists every downstream change.

mod protocol;
pub mod request;
pub mod stream;

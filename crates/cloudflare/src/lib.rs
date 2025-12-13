//! OpenAgents Cloudflare Worker
//!
//! A Nostr relay and DVM (Data Vending Machine) running on Cloudflare Workers.
//!
//! ## Architecture
//!
//! - **Worker (entry point)**: Routes HTTP/WebSocket requests to Durable Objects
//! - **RelayDurableObject**: Nostr relay with SQLite storage, manages WebSocket connections
//! - **AgentDurableObject** (future): Individual agent execution environments
//!
//! ## Usage
//!
//! ```bash
//! # Local development
//! cd crates/cloudflare && wrangler dev
//!
//! # Connect with a Nostr client
//! websocat ws://localhost:8787
//! ["REQ", "test", {"kinds": [1], "limit": 10}]
//! ```

use worker::*;

mod dvm;
mod relay_do;
mod signing;

pub use dvm::DvmProcessor;
pub use relay_do::RelayDurableObject;
pub use signing::ServiceIdentity;

/// Main entry point for the Cloudflare Worker.
///
/// Routes all requests to the Relay Durable Object.
#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Get the relay Durable Object
    let relay_do = env.durable_object("RELAY")?;

    // Use a single relay instance (can shard by pubkey later)
    let stub = relay_do.id_from_name("main-relay")?.get_stub()?;

    // Forward the request to the Durable Object
    stub.fetch_with_request(req).await
}

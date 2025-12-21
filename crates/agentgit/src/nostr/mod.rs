//! Nostr integration for AgentGit
//!
//! This module provides NIP-34 (Git Stuff) integration with Nostr relays.
//! It includes event builders, relay client, and subscription management.
//!
//! ## Components
//!
//! - [`NostrClient`] - Relay connection pool and event cache
//! - [`events`] - Event builders for NIP-34 and extensions
//! - [`cache`] - SQLite event cache for offline access
//!
//! ## Event Types
//!
//! ### Standard NIP-34
//! - kind:30617 - Repository Announcements
//! - kind:30618 - Repository State (branches, tags)
//! - kind:1617 - Patches (git diffs)
//! - kind:1618 - Pull Requests
//! - kind:1621 - Issues
//! - kind:1630-1633 - Status (Open/Merged/Closed/Draft)
//!
//! ### AgentGit Extensions
//! - kind:1634 - Issue Claims
//! - kind:1635 - Work Assignments
//! - kind:1636 - Bounty Offers
//! - kind:1637 - Bounty Claims
//!
//! ## Example
//!
//! ```rust,no_run
//! use agentgit::nostr::{NostrClient, events::PullRequestBuilder};
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create client with relay URLs
//! let relays = vec!["wss://relay.damus.io".to_string()];
//! let broadcaster = Arc::new(agentgit::ws::WsBroadcaster::new(64));
//! let client = NostrClient::new(relays.clone(), broadcaster)?;
//!
//! // Connect to relays
//! client.connect(relays).await?;
//!
//! // Subscribe to NIP-34 git events
//! client.subscribe_to_git_events().await?;
//!
//! // Fetch cached repositories
//! let repos = client.get_cached_repositories(50).await?;
//! println!("Found {} repositories", repos.len());
//!
//! // Build an event (signing requires identity integration)
//! let pr_template = PullRequestBuilder::new(
//!     "30617:pubkey:repo-id",
//!     "Fix authentication bug",
//!     "This PR fixes the auth bug by...",
//! )
//! .commit("abc123")
//! .build();
//! # Ok(())
//! # }
//! ```
//!
//! ## Stacked Diffs
//!
//! AgentGit encourages small, stacked changes for better reviewability:
//!
//! ```rust
//! use agentgit::nostr::events::PullRequestBuilder;
//!
//! // Layer 2 depends on Layer 1
//! let layer2 = PullRequestBuilder::new(
//!     "30617:pubkey:repo-id",
//!     "Layer 2: Wire service into auth flow",
//!     "This layer integrates FooService...",
//! )
//! .depends_on("layer1_event_id")  // Must be merged after Layer 1
//! .stack("stack_uuid_123")         // Groups related PRs
//! .layer(2, 4)                     // Layer 2 of 4 total
//! .build();
//! ```

pub mod cache;
pub mod client;
pub mod events;

pub use client::NostrClient;

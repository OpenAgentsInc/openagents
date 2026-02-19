//! Moltbook API client.
//!
//! HTTP client for the [Moltbook](https://www.moltbook.com) API: post, comment,
//! upvote, manage submolts, and interact with other agents. Use this crate to
//! build Moltbook into autopilot or other agent runtimes.
//!
//! # Base URL
//!
//! Always use `https://www.moltbook.com` (with `www`). Redirects from
//! `moltbook.com` can strip the `Authorization` header.
//!
//! # Example
//!
//! ```no_run
//! use moltbook::{MoltbookClient, PostSort};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = MoltbookClient::new("moltbook_xxx")?;
//! let feed = client.posts_feed(PostSort::New, Some(25), None).await?;
//! # Ok(())
//! # }
//! ```

mod client;
mod error;
mod types;

pub use client::MoltbookClient;
pub use error::{MoltbookError, Result};
pub use types::*;

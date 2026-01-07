//! # Nexus
//!
//! Nostr relay for the decentralized compute marketplace.
//!
//! **Status: v0.1 in development.**
//!
//! Nexus is fundamentally a Nostr relay. It speaks NIP-01, stores events, and
//! handles subscriptions. It's optimized for agent-to-agent commerce: job
//! requests (NIP-90), handler discovery (NIP-89), and authentication (NIP-42).
//!
//! ## Decentralized Network
//!
//! Anyone can run a Nexus. The network is decentralized:
//!
//! - **Open protocol** — Standard Nostr NIPs. Any compatible relay works.
//! - **No lock-in** — Pylons connect to multiple relays simultaneously.
//! - **Self-host** — Deploy your own for sovereignty or custom policies.
//!
//! OpenAgents runs `nexus.openagents.com`, but it's not required.
//!
//! ## Pylon + Nexus
//!
//! A Pylon by itself does nothing. It must connect to at least one Nexus
//! (or compatible Nostr relay) to discover providers, submit jobs, and
//! receive results.
//!
//! See [README.md](../README.md) for deployment options and documentation.

// TODO: Implementation - see docs/ROADMAP.md for steps

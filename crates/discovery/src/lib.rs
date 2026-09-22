//! The public discovery surface, shared by every origin.
//!
//! `corpus` is the bundled documentation the documentation tools and
//! the HTTP docs API read — one registry, one stable set of document
//! IDs, one corpus digest. `site` is the machine-readable document set
//! an unauthenticated client fetches first: `llms.txt`, the auth and
//! skill documents, the API catalog, the OpenAPI fragment, the
//! well-known agent-skills index, the agent card, and the plugin
//! manifests. Nothing here answers a decision call and nothing here
//! authenticates — discovery describes the deployed service; it never
//! grants it.

pub mod corpus;
pub mod plugins;
pub mod site;

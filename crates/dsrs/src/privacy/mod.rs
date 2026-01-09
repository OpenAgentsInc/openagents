//! Privacy module for dsrs.
//!
//! Provides redaction and privacy controls for private repository usage
//! when dispatching jobs to the swarm.
//!
//! ## Features
//!
//! - **Redaction**: Anonymize paths, identifiers, and sensitive content
//! - **Chunking**: Control how much context is sent to providers
//! - **Policy**: Define allowlists for job types and trusted providers
//!
//! ## Example
//!
//! ```ignore
//! use dsrs::privacy::{PrivacyPolicy, RedactionMode, PathRedactor};
//!
//! // Create a policy for private repos
//! let policy = PrivacyPolicy::private_repo();
//!
//! // Redact paths in content
//! let redactor = PathRedactor::new();
//! let redacted = redactor.redact("/Users/alice/secret-project/src/main.rs");
//! assert_eq!(redacted.content, "/workspace/src/main.rs");
//!
//! // Restore original paths
//! let restored = redactor.restore(&redacted);
//! assert_eq!(restored, "/Users/alice/secret-project/src/main.rs");
//! ```

pub mod chunking;
pub mod policy;
pub mod redaction;

pub use chunking::*;
pub use policy::*;
pub use redaction::*;

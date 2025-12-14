//! Strongly-typed identifiers for domain entities.
//!
//! Using newtypes for IDs provides type safety and prevents
//! accidentally mixing up different kinds of identifiers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new random ID.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Create from an existing UUID.
            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Get the underlying UUID.
            pub fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Parse from a string.
            pub fn parse(s: &str) -> Result<Self, uuid::Error> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id!(ThreadId);
define_id!(MessageId);
define_id!(ProjectId);
define_id!(WorkflowId);
define_id!(RunId);
define_id!(StepId);
define_id!(ToolUseId);
define_id!(ArtifactId);
define_id!(SessionId);
define_id!(PermissionId);

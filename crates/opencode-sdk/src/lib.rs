//! OpenCode SDK for Rust
//!
//! A native Rust SDK for communicating with OpenCode servers, enabling
//! provider-agnostic AI agent execution through REST API + SSE architecture.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use opencode_sdk::{OpencodeClient, OpencodeClientConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), opencode_sdk::Error> {
//!     let client = OpencodeClient::new(OpencodeClientConfig::default())?;
//!     
//!     // Create a session
//!     let session = client.session_create(Default::default()).await?;
//!     
//!     // Send a prompt
//!     client.session_prompt(&session.id, "Fix the bug").await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod events;
pub mod server;
pub mod types;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/codegen.rs"));
}

pub use generated::Client as GeneratedClient;
pub use generated::types as gen_types;

pub use client::{OpencodeClient, OpencodeClientConfig};
pub use error::{Error, Result};
pub use events::{Event, EventStream};
pub use server::{OpencodeServer, ServerOptions};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    mod config {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn default_config_has_localhost_url() {
            let config = OpencodeClientConfig::default();
            assert_eq!(config.base_url, "http://127.0.0.1:4096");
            assert_eq!(config.timeout_seconds, 30);
            assert!(config.directory.is_none());
        }

        #[test]
        fn config_builder_sets_base_url() {
            let config = OpencodeClientConfig::new().base_url("http://custom:8080");
            assert_eq!(config.base_url, "http://custom:8080");
        }

        #[test]
        fn config_builder_sets_directory() {
            let config = OpencodeClientConfig::new().directory("/path/to/project");
            assert_eq!(config.directory, Some(PathBuf::from("/path/to/project")));
        }

        #[test]
        fn config_builder_sets_timeout() {
            let config = OpencodeClientConfig::new().timeout(60);
            assert_eq!(config.timeout_seconds, 60);
        }

        #[test]
        fn config_builder_chains() {
            let config = OpencodeClientConfig::new()
                .base_url("http://localhost:3000")
                .directory("/code")
                .timeout(120);

            assert_eq!(config.base_url, "http://localhost:3000");
            assert_eq!(config.directory, Some(PathBuf::from("/code")));
            assert_eq!(config.timeout_seconds, 120);
        }
    }

    mod client {
        use super::*;

        #[test]
        fn client_creation_succeeds_with_valid_url() {
            let config = OpencodeClientConfig::new().base_url("http://127.0.0.1:4096");
            let result = OpencodeClient::new(config);
            assert!(result.is_ok());
        }

        #[test]
        fn client_creation_fails_with_invalid_url() {
            let config = OpencodeClientConfig::new().base_url("not-a-valid-url");
            let result = OpencodeClient::new(config);
            assert!(result.is_err());
        }
    }

    mod types_serialization {
        use super::*;

        #[test]
        fn part_text_serializes_correctly() {
            let part = Part::Text {
                text: "hello".to_string(),
            };
            let json = serde_json::to_string(&part).unwrap();
            assert!(json.contains(r#""type":"text""#));
            assert!(json.contains(r#""text":"hello""#));
        }

        #[test]
        fn part_image_serializes_correctly() {
            let part = Part::Image {
                url: "https://example.com/img.png".to_string(),
            };
            let json = serde_json::to_string(&part).unwrap();
            assert!(json.contains(r#""type":"image""#));
            assert!(json.contains(r#""url":"https://example.com/img.png""#));
        }

        #[test]
        fn part_file_serializes_correctly() {
            let part = Part::File {
                path: "/path/to/file.rs".to_string(),
            };
            let json = serde_json::to_string(&part).unwrap();
            assert!(json.contains(r#""type":"file""#));
            assert!(json.contains(r#""path":"/path/to/file.rs""#));
        }

        #[test]
        fn prompt_request_with_text_parts() {
            let request = PromptRequest {
                parts: vec![Part::Text {
                    text: "Fix the bug".to_string(),
                }],
                agent: None,
                model: None,
            };
            let json = serde_json::to_string(&request).unwrap();
            assert!(json.contains("Fix the bug"));
        }

        #[test]
        fn session_create_request_minimal() {
            let request = SessionCreateRequest::default();
            let json = serde_json::to_string(&request).unwrap();
            // Empty object when all fields are None
            assert_eq!(json, "{}");
        }

        #[test]
        fn session_create_request_with_agent() {
            let request = SessionCreateRequest {
                agent: Some("coder".to_string()),
                ..Default::default()
            };
            let json = serde_json::to_string(&request).unwrap();
            assert!(json.contains(r#""agent":"coder""#));
        }

        #[test]
        fn model_ref_serializes_correctly() {
            let model = ModelRef {
                provider_id: "anthropic".to_string(),
                model_id: "claude-sonnet-4".to_string(),
            };
            let json = serde_json::to_string(&model).unwrap();
            assert!(json.contains(r#""providerId":"anthropic""#));
            assert!(json.contains(r#""modelId":"claude-sonnet-4""#));
        }

        #[test]
        fn todo_status_serializes_as_snake_case() {
            let status = TodoStatus::InProgress;
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, r#""in_progress""#);
        }

        #[test]
        fn todo_priority_serializes_as_snake_case() {
            let priority = TodoPriority::High;
            let json = serde_json::to_string(&priority).unwrap();
            assert_eq!(json, r#""high""#);
        }

        #[test]
        fn file_type_serializes_lowercase() {
            let ft = FileType::Directory;
            let json = serde_json::to_string(&ft).unwrap();
            assert_eq!(json, r#""directory""#);
        }

        #[test]
        fn mcp_status_serializes_lowercase() {
            let status = McpStatus::Connected;
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, r#""connected""#);
        }
    }

    mod types_deserialization {
        use super::*;

        #[test]
        fn part_text_deserializes() {
            let json = r#"{"type":"text","text":"hello world"}"#;
            let part: Part = serde_json::from_str(json).unwrap();
            match part {
                Part::Text { text } => assert_eq!(text, "hello world"),
                _ => panic!("Expected Part::Text"),
            }
        }

        #[test]
        fn todo_deserializes() {
            let json = r#"{
                "id": "todo-1",
                "content": "Fix the bug",
                "status": "pending",
                "priority": "high"
            }"#;
            let todo: Todo = serde_json::from_str(json).unwrap();
            assert_eq!(todo.id, "todo-1");
            assert_eq!(todo.content, "Fix the bug");
            assert!(matches!(todo.status, TodoStatus::Pending));
            assert!(matches!(todo.priority, TodoPriority::High));
        }

        #[test]
        fn file_info_deserializes() {
            let json = r#"{
                "name": "lib.rs",
                "path": "/src/lib.rs",
                "type": "file",
                "size": 1024
            }"#;
            let info: FileInfo = serde_json::from_str(json).unwrap();
            assert_eq!(info.name, "lib.rs");
            assert_eq!(info.path, "/src/lib.rs");
            assert!(matches!(info.file_type, FileType::File));
            assert_eq!(info.size, Some(1024));
        }

        #[test]
        fn text_match_deserializes() {
            let json = r#"{
                "file": "src/main.rs",
                "line": 42,
                "column": 10,
                "text": "fn main()"
            }"#;
            let m: TextMatch = serde_json::from_str(json).unwrap();
            assert_eq!(m.file, "src/main.rs");
            assert_eq!(m.line, 42);
            assert_eq!(m.column, 10);
            assert_eq!(m.text, "fn main()");
        }

        #[test]
        fn vcs_status_deserializes() {
            let json = r#"{
                "vcs": "git",
                "branch": "main",
                "remote": "origin",
                "ahead": 2,
                "behind": 0
            }"#;
            let status: VcsStatus = serde_json::from_str(json).unwrap();
            assert_eq!(status.vcs, "git");
            assert_eq!(status.branch, "main");
            assert_eq!(status.remote, Some("origin".to_string()));
            assert_eq!(status.ahead, Some(2));
            assert_eq!(status.behind, Some(0));
        }
    }

    mod error {
        use super::*;

        #[test]
        fn server_unavailable_error_displays_url() {
            let err = Error::ServerUnavailable {
                url: "http://localhost:4096".to_string(),
            };
            let msg = format!("{}", err);
            assert!(msg.contains("localhost:4096"));
        }

        #[test]
        fn session_not_found_error_displays_id() {
            let err = Error::SessionNotFound {
                id: "abc123".to_string(),
            };
            let msg = format!("{}", err);
            assert!(msg.contains("abc123"));
        }

        #[test]
        fn health_check_failed_shows_attempts() {
            let err = Error::HealthCheckFailed { attempts: 5 };
            let msg = format!("{}", err);
            assert!(msg.contains("5"));
        }

        #[test]
        fn timeout_error_shows_seconds() {
            let err = Error::Timeout { seconds: 30 };
            let msg = format!("{}", err);
            assert!(msg.contains("30"));
        }
    }

    mod server_options {
        use super::*;
        use std::path::PathBuf;

        #[test]
        fn default_server_options() {
            let opts = ServerOptions::default();
            assert_eq!(opts.port, 4096);
            assert_eq!(opts.hostname, "127.0.0.1");
            assert_eq!(opts.timeout_ms, 30000);
        }

        #[test]
        fn server_options_builder() {
            let opts = ServerOptions::new()
                .port(8080)
                .hostname("0.0.0.0")
                .timeout_ms(60000)
                .directory(PathBuf::from("/project"));

            assert_eq!(opts.port, 8080);
            assert_eq!(opts.hostname, "0.0.0.0");
            assert_eq!(opts.timeout_ms, 60000);
            assert_eq!(opts.directory, Some(PathBuf::from("/project")));
        }
    }
}

//! The few `openagents-client` internals the shared demo fixtures name.
//!
//! `demo.rs` compiles the client's fixture files unchanged; the client
//! crate itself needs tokio and reqwest, which don't build for wasm32. Each
//! item here mirrors the one it stands in for.

/// `openagents_client::now_ms`.
pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// `openagents_client::attachments`, the part the fixtures call.
pub(crate) mod attachments {
    pub const ATTACHMENT_ONLY_TEXT: &str = "See the attached image(s).";
    pub const ATTACHMENT_MARKER: &str = "Attached images (local files — open them to view):";

    pub fn with_attachments(text: &str, paths: &[String]) -> String {
        if paths.is_empty() {
            return text.to_owned();
        }
        let body = if text.trim().is_empty() {
            ATTACHMENT_ONLY_TEXT
        } else {
            text
        };
        let refs = paths
            .iter()
            .map(|p| format!("- {p}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{body}\n\n{ATTACHMENT_MARKER}\n{refs}")
    }

    /// Splits a user message into its text and attached paths.
    pub fn split(text: &str) -> (&str, Vec<&str>) {
        match text.split_once(ATTACHMENT_MARKER) {
            Some((body, refs)) => (
                body.trim_end(),
                refs.lines()
                    .filter_map(|line| line.trim().strip_prefix("- "))
                    .collect(),
            ),
            None => (text, Vec::new()),
        }
    }
}

/// `openagents_client::client::ms`.
pub(crate) mod client {
    pub(crate) fn ms(at: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(at).unwrap_or_default()
    }
}

/// `openagents_client::config::DemoFixture`.
pub(crate) mod config {
    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug)]
    pub enum DemoFixture {
        Standard,
        NoProjects,
        IosOnly,
    }
}

/// `openagents_client::rpc::capability`, the list the fixtures read.
pub(crate) mod rpc {
    pub mod capability {
        pub const ALL_QUEUE: &[&str] = &[
            "message-queue-v1",
            "message-queue-actions-v1",
            "message-queue-attachments-v1",
        ];
    }
}

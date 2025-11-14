use convex::{ConvexClient, FunctionResult, Value};
use once_cell::sync::Lazy;
use std::collections::BTreeMap;
use tokio::sync::Mutex;
use anyhow::Result;

/// Global Convex client manager
pub static CONVEX_MANAGER: Lazy<Mutex<ConvexClientManager>> =
    Lazy::new(|| Mutex::new(ConvexClientManager { client: None }));

/// Manages the Convex client instance
pub struct ConvexClientManager {
    pub client: Option<ConvexClient>,
}

impl ConvexClientManager {
    /// Initialize the Convex client with a deployment URL
    pub async fn initialize(deployment_url: &str) -> Result<()> {
        let client = ConvexClient::new(deployment_url).await?;

        let mut manager = CONVEX_MANAGER.lock().await;
        manager.client = Some(client);

        tracing::info!("Convex client initialized for {}", deployment_url);
        Ok(())
    }

    /// Set authentication token for the Convex client
    pub async fn set_auth(token: Option<String>) -> Result<()> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            client.set_auth(token.clone()).await;
            match token {
                Some(t) => tracing::info!("Convex auth token set (length: {})", t.len()),
                None => tracing::info!("Convex auth cleared"),
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Upsert a streaming message
    pub async fn upsert_streaming_message(
        thread_id: &str,
        item_id: &str,
        role: &str,
        content: &str,
        kind: Option<&str>,
        partial: bool,
        seq: Option<i64>,
    ) -> Result<FunctionResult> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();
            args.insert("threadId".to_string(), Value::String(thread_id.to_string()));
            args.insert("itemId".to_string(), Value::String(item_id.to_string()));
            args.insert("role".to_string(), Value::String(role.to_string()));
            args.insert("content".to_string(), Value::String(content.to_string()));

            if let Some(k) = kind {
                args.insert("kind".to_string(), Value::String(k.to_string()));
            }

            args.insert("partial".to_string(), Value::Boolean(partial));

            if let Some(s) = seq {
                args.insert("seq".to_string(), Value::Int64(s));
            }

            let result = client.mutation("chat:upsertStreamingMessage", args).await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Finalize a streaming message
    pub async fn finalize_message(item_id: &str) -> Result<FunctionResult> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();
            args.insert("itemId".to_string(), Value::String(item_id.to_string()));

            let result = client.mutation("chat:finalizeMessage", args).await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Upsert a tool call
    pub async fn upsert_tool_call(
        thread_id: &str,
        tool_call_id: &str,
        title: Option<&str>,
        kind: Option<&str>,
        status: Option<&str>,
        content_json: Option<&str>,
        locations_json: Option<&str>,
    ) -> Result<FunctionResult> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();
            args.insert("threadId".to_string(), Value::String(thread_id.to_string()));
            args.insert("toolCallId".to_string(), Value::String(tool_call_id.to_string()));

            if let Some(t) = title {
                args.insert("title".to_string(), Value::String(t.to_string()));
            }
            if let Some(k) = kind {
                args.insert("kind".to_string(), Value::String(k.to_string()));
            }
            if let Some(s) = status {
                args.insert("status".to_string(), Value::String(s.to_string()));
            }
            if let Some(c) = content_json {
                args.insert("contentJson".to_string(), Value::String(c.to_string()));
            }
            if let Some(l) = locations_json {
                args.insert("locationsJson".to_string(), Value::String(l.to_string()));
            }

            let result = client.mutation("toolCalls:upsertToolCall", args).await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Upsert plan entries
    pub async fn upsert_plan(
        thread_id: &str,
        entries_json: &str,
    ) -> Result<FunctionResult> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();
            args.insert("threadId".to_string(), Value::String(thread_id.to_string()));
            args.insert("entriesJson".to_string(), Value::String(entries_json.to_string()));

            let result = client.mutation("planEntries:upsertPlan", args).await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Upsert thread state
    pub async fn upsert_thread_state(
        thread_id: &str,
        current_mode_id: Option<&str>,
        available_commands_json: Option<&str>,
    ) -> Result<FunctionResult> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();
            args.insert("threadId".to_string(), Value::String(thread_id.to_string()));

            if let Some(m) = current_mode_id {
                args.insert("currentModeId".to_string(), Value::String(m.to_string()));
            }
            if let Some(c) = available_commands_json {
                args.insert("availableCommandsJson".to_string(), Value::String(c.to_string()));
            }

            let result = client.mutation("threadState:upsertThreadState", args).await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Append an ACP event to the log
    pub async fn append_event(
        session_id: Option<&str>,
        client_thread_doc_id: Option<&str>,
        thread_id: Option<&str>,
        update_kind: Option<&str>,
        payload: &str,
    ) -> Result<FunctionResult> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();

            if let Some(s) = session_id {
                args.insert("sessionId".to_string(), Value::String(s.to_string()));
            }
            if let Some(c) = client_thread_doc_id {
                args.insert("clientThreadDocId".to_string(), Value::String(c.to_string()));
            }
            if let Some(t) = thread_id {
                args.insert("threadId".to_string(), Value::String(t.to_string()));
            }
            if let Some(u) = update_kind {
                args.insert("updateKind".to_string(), Value::String(u.to_string()));
            }

            args.insert("payload".to_string(), Value::String(payload.to_string()));

            let result = client.mutation("acpEvents:appendEvent", args).await?;
            Ok(result)
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Create a new thread
    pub async fn create_thread(
        title: Option<&str>,
        project_id: Option<&str>,
        source: Option<&str>,
        working_directory: Option<&str>,
    ) -> Result<String> {
        let mut manager = CONVEX_MANAGER.lock().await;

        if let Some(client) = &mut manager.client {
            let mut args = BTreeMap::new();

            if let Some(t) = title {
                args.insert("title".to_string(), Value::String(t.to_string()));
            }
            if let Some(p) = project_id {
                args.insert("projectId".to_string(), Value::String(p.to_string()));
            }
            if let Some(s) = source {
                args.insert("source".to_string(), Value::String(s.to_string()));
            }
            if let Some(w) = working_directory {
                args.insert("workingDirectory".to_string(), Value::String(w.to_string()));
            }

            let result = client.mutation("chat:createThreadExtended", args).await?;

            // Extract thread ID from result
            match result {
                FunctionResult::Value(Value::String(thread_id)) => Ok(thread_id),
                _ => Err(anyhow::anyhow!("Unexpected result from createThreadExtended")),
            }
        } else {
            Err(anyhow::anyhow!("Convex client not initialized"))
        }
    }

    /// Check if Convex client is initialized
    pub async fn is_initialized() -> bool {
        let manager = CONVEX_MANAGER.lock().await;
        manager.client.is_some()
    }
}

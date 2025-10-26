//! Mutations and thread creation commands for the Tauri backend.
//!
//! These commands wrap Convex mutations (`runs:enqueue`, `threads:create`) and
//! are invoked from the webview via `window.__TAURI__.core.invoke`.

#[tauri::command]
#[allow(non_snake_case)]
pub async fn enqueue_run(threadDocId: String, text: String, role: Option<String>, projectId: Option<String>, resumeId: Option<String>, convex_url: Option<String>) -> Result<(), String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    let mut client = convex::ConvexClient::new(&url).await.map_err(|e| format!("convex connect error: {e}"))?;
    println!("[tauri/runs] enqueue_run threadDocId={} role={:?} projectId={:?} resumeId={:?} text_len={} url={}", threadDocId, role, projectId, resumeId, text.len(), url);

    // Optimistically persist the user message to Convex so the UI updates immediately.
    // The bridge handles assistant/reason/tool rows during the run.
    {
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        let ts_ms: f64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64;
        args.insert("threadId".into(), Value::from(threadDocId.clone()));
        args.insert("role".into(), Value::from(role.as_deref().unwrap_or("user")));
        args.insert("kind".into(), Value::from("message"));
        args.insert("text".into(), Value::from(text.clone()));
        args.insert("ts".into(), Value::from(ts_ms));
        match client.mutation("messages:create", args).await {
            Ok(FunctionResult::Value(_)) => {
                println!("[tauri/runs] messages:create -> ok (user message persisted)");
            }
            Ok(FunctionResult::ErrorMessage(msg)) => {
                eprintln!("[tauri/runs] messages:create error: {}", msg);
            }
            Ok(FunctionResult::ConvexError(err)) => {
                eprintln!("[tauri/runs] messages:create convex error: {}", err);
            }
            Err(e) => {
                eprintln!("[tauri/runs] messages:create mutation error: {}", e);
            }
        }
    }
    let mut args: BTreeMap<String, Value> = BTreeMap::new();
    args.insert("threadDocId".into(), Value::from(threadDocId));
    args.insert("text".into(), Value::from(text));
    if let Some(r) = role { args.insert("role".into(), Value::from(r)); }
    if let Some(p) = projectId { args.insert("projectId".into(), Value::from(p)); }
    if let Some(rid) = resumeId { args.insert("resumeId".into(), Value::from(rid)); }
    match client.mutation("runs:enqueue", args).await {
        Ok(FunctionResult::Value(_)) => { println!("[tauri/runs] enqueue_run -> ok"); Ok(()) },
        Ok(FunctionResult::ErrorMessage(msg)) => { eprintln!("[tauri/runs] enqueue_run error: {}", msg); Err(msg) },
        Ok(FunctionResult::ConvexError(err)) => { eprintln!("[tauri/runs] enqueue_run convex error: {}", err); Err(err.to_string()) },
        Err(e) => { eprintln!("[tauri/runs] enqueue_run mutation error: {}", e); Err(format!("convex mutation error: {e}")) },
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn create_thread(title: Option<String>, projectId: Option<String>, convex_url: Option<String>) -> Result<(), String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    let mut client = convex::ConvexClient::new(&url).await.map_err(|e| format!("convex connect error: {e}"))?;
    let mut args: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(t) = title { args.insert("title".into(), Value::from(t)); }
    if let Some(p) = projectId { args.insert("projectId".into(), Value::from(p)); }
    match client.mutation("threads:create", args).await {
        Ok(FunctionResult::Value(_)) => Ok(()),
        Ok(FunctionResult::ErrorMessage(msg)) => Err(msg),
        Ok(FunctionResult::ConvexError(err)) => Err(err.to_string()),
        Err(e) => Err(format!("convex mutation error: {e}")),
    }
}

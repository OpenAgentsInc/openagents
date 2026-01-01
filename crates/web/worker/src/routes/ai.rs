//! AI completion routes via Stripe LLM proxy

use crate::middleware::auth::AuthenticatedUser;
use crate::services::stripe;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::JsValue;
use worker::*;

#[derive(Deserialize)]
pub struct ChatCompletionRequest {
    pub messages: Vec<MessageInput>,
    #[serde(default = "default_model")]
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub system: Option<String>,
    pub tools: Option<Vec<Value>>,
    pub tool_choice: Option<Value>,
}

fn default_model() -> String {
    "anthropic/claude-sonnet-4.5".to_string()
}

#[derive(Deserialize)]
pub struct MessageInput {
    pub role: String,
    #[serde(default)]
    pub content: Option<Value>,  // Can be string, null, or array (for tool calls)
    #[serde(default)]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

#[derive(Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub content: String,
    pub finish_reason: Option<String>,
    pub usage: Option<UsageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<Vec<ToolUseBlock>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolUseBlock {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Serialize)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Chat completion endpoint - proxies to Stripe LLM
pub async fn chat_completion(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    let db = env.d1("DB")?;

    // Get Stripe secret - return clear error if missing
    let stripe_secret = match env.secret("STRIPE_SECRET_KEY") {
        Ok(s) => s.to_string(),
        Err(_) => return Response::error("STRIPE_SECRET_KEY not configured", 500),
    };

    // Parse request
    let request: ChatCompletionRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => return Response::error(format!("Invalid request: {}", e), 400),
    };

    // Get user's Stripe customer ID or create one
    let customer_id = match db
        .prepare("SELECT stripe_customer_id FROM stripe_customers WHERE user_id = ?")
        .bind(&[user.user_id.clone().into()])?
        .first::<String>(Some("stripe_customer_id"))
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            // Create a Stripe customer for this user
            let customer = match stripe::create_customer(&stripe_secret, &user.github_username).await {
                Ok(c) => c,
                Err(e) => return Response::error(format!("Failed to create Stripe customer: {}", e), 500),
            };

            // Store in D1
            let now = chrono::Utc::now().to_rfc3339();
            if let Err(e) = db.prepare(
                "INSERT INTO stripe_customers (user_id, stripe_customer_id, created_at) VALUES (?, ?, ?)",
            )
            .bind(&[
                user.user_id.clone().into(),
                customer.id.clone().into(),
                now.into(),
            ])?
            .run()
            .await {
                return Response::error(format!("Failed to store customer: {}", e), 500);
            }

            customer.id
        }
        Err(e) => return Response::error(format!("Database error: {}", e), 500),
    };

    // Build raw JSON request to preserve message structure (for tool_result blocks)
    let mut llm_body = serde_json::json!({
        "model": request.model,
        "messages": serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("messages").cloned())
            .unwrap_or(serde_json::json!([])),
        "max_tokens": request.max_tokens.unwrap_or(1024)
    });

    // Add tools if provided
    if let Some(tools) = &request.tools {
        llm_body["tools"] = serde_json::json!(tools);
    }

    // Add tool_choice if provided
    if let Some(tool_choice) = &request.tool_choice {
        llm_body["tool_choice"] = tool_choice.clone();
    }

    // Add system if provided
    if let Some(system) = &request.system {
        llm_body["system"] = serde_json::json!(system);
    }

    // Add temperature if provided
    if let Some(temp) = request.temperature {
        llm_body["temperature"] = serde_json::json!(temp);
    }

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", stripe_secret))?;
    headers.set("Content-Type", "application/json")?;
    headers.set("X-Stripe-Customer-ID", &customer_id)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&llm_body.to_string())));

    let upstream_req = Request::new_with_init("https://llm.stripe.com/chat/completions", &init)?;
    let mut upstream_resp = Fetch::Request(upstream_req).send().await?;

    let status = upstream_resp.status_code();
    let text = upstream_resp.text().await?;

    if status != 200 {
        return Response::error(format!("LLM API error ({}): {}", status, text), status);
    }

    // Parse response and extract content + tool_use
    let llm_json: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return Response::error(format!("Failed to parse LLM response: {}", e), 500),
    };

    let id = llm_json.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let model = llm_json.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Get first choice
    let choice = llm_json.get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first());

    let finish_reason = choice
        .and_then(|c| c.get("finish_reason"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract content - could be string or array of content blocks
    let message = choice.and_then(|c| c.get("message"));
    let content = message
        .and_then(|m| m.get("content"))
        .map(|c| {
            if let Some(s) = c.as_str() {
                s.to_string()
            } else if let Some(arr) = c.as_array() {
                // Content is array of blocks, extract text
                arr.iter()
                    .filter_map(|block| {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            block.get("text").and_then(|t| t.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("")
            } else {
                String::new()
            }
        })
        .unwrap_or_default();

    // Extract tool_calls (OpenAI format)
    let tool_use: Option<Vec<ToolUseBlock>> = message
        .and_then(|m| m.get("tool_calls"))
        .and_then(|tc| tc.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|tc| {
                    let id = tc.get("id")?.as_str()?.to_string();
                    let function = tc.get("function")?;
                    let name = function.get("name")?.as_str()?.to_string();
                    let arguments = function.get("arguments")?.as_str()?;
                    let input: Value = serde_json::from_str(arguments).ok()?;
                    Some(ToolUseBlock { id, name, input })
                })
                .collect()
        })
        .filter(|v: &Vec<ToolUseBlock>| !v.is_empty());

    // Extract usage
    let usage = llm_json.get("usage").map(|u| UsageInfo {
        prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
    });

    // Save LLM call to database for tracking
    let call_id = format!("call_{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().to_rfc3339();
    let messages_json = serde_json::to_string(&llm_body.get("messages")).unwrap_or_default();
    // Combine content and tool_use for response storage
    let response_json = if let Some(ref tools) = tool_use {
        serde_json::json!({
            "content": &content,
            "tool_calls": tools
        }).to_string()
    } else {
        content.clone()
    };

    let _ = db.prepare(
        "INSERT INTO llm_calls (id, user_id, stripe_customer_id, model, prompt_tokens, completion_tokens, total_tokens, request_messages, response_content, finish_reason, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&[
        call_id.clone().into(),
        user.user_id.clone().into(),
        customer_id.clone().into(),
        model.clone().into(),
        (usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0) as i64).into(),
        (usage.as_ref().map(|u| u.completion_tokens).unwrap_or(0) as i64).into(),
        (usage.as_ref().map(|u| u.total_tokens).unwrap_or(0) as i64).into(),
        messages_json.into(),
        response_json.into(),
        finish_reason.clone().unwrap_or_default().into(),
        now.into(),
    ])?
    .run()
    .await;

    let response = ChatCompletionResponse {
        id,
        model,
        content,
        finish_reason,
        usage,
        tool_use,
    };

    Response::from_json(&response)
}

/// Streaming chat completion endpoint - proxies to Stripe LLM with SSE
pub async fn chat_completion_stream(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    let db = env.d1("DB")?;

    // Get Stripe secret
    let stripe_secret = match env.secret("STRIPE_SECRET_KEY") {
        Ok(s) => s.to_string(),
        Err(_) => return Response::error("STRIPE_SECRET_KEY not configured", 500),
    };

    // Parse request
    let request: ChatCompletionRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => return Response::error(format!("Invalid request: {}", e), 400),
    };

    // Get or create Stripe customer
    let customer_id = match db
        .prepare("SELECT stripe_customer_id FROM stripe_customers WHERE user_id = ?")
        .bind(&[user.user_id.clone().into()])?
        .first::<String>(Some("stripe_customer_id"))
        .await
    {
        Ok(Some(id)) => id,
        Ok(None) => {
            let customer = match stripe::create_customer(&stripe_secret, &user.github_username).await {
                Ok(c) => c,
                Err(e) => return Response::error(format!("Failed to create Stripe customer: {}", e), 500),
            };

            let now = chrono::Utc::now().to_rfc3339();
            if let Err(e) = db.prepare(
                "INSERT INTO stripe_customers (user_id, stripe_customer_id, created_at) VALUES (?, ?, ?)",
            )
            .bind(&[
                user.user_id.clone().into(),
                customer.id.clone().into(),
                now.into(),
            ])?
            .run()
            .await {
                return Response::error(format!("Failed to store customer: {}", e), 500);
            }

            customer.id
        }
        Err(e) => return Response::error(format!("Database error: {}", e), 500),
    };

    // Build streaming request with stream: true
    // Use raw JSON passthrough to preserve message structure (tool_calls, tool_call_id, etc.)
    let mut llm_body = serde_json::json!({
        "model": request.model,
        "messages": serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("messages").cloned())
            .unwrap_or(serde_json::json!([])),
        "max_tokens": request.max_tokens.unwrap_or(4096),
        "stream": true
    });

    // Add tools if provided
    if let Some(tools) = &request.tools {
        llm_body["tools"] = serde_json::json!(tools);
    }

    // Add tool_choice if provided
    if let Some(tool_choice) = &request.tool_choice {
        llm_body["tool_choice"] = tool_choice.clone();
    }

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {}", stripe_secret))?;
    headers.set("Content-Type", "application/json")?;
    headers.set("X-Stripe-Customer-ID", &customer_id)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&llm_body.to_string())));

    let upstream_req = Request::new_with_init("https://llm.stripe.com/chat/completions", &init)?;
    let mut upstream_resp = Fetch::Request(upstream_req).send().await?;

    // Check status
    let status = upstream_resp.status_code();
    if status != 200 {
        let text = upstream_resp.text().await.unwrap_or_default();
        return Response::error(format!("LLM API error ({}): {}", status, text), status);
    }

    // Log streaming request to database (response logged separately since it's streamed)
    let call_id = format!("stream_{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().to_rfc3339();
    let messages_json = serde_json::to_string(&llm_body.get("messages")).unwrap_or_default();

    let _ = db.prepare(
        "INSERT INTO llm_calls (id, user_id, stripe_customer_id, model, request_messages, response_content, finish_reason, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&[
        call_id.into(),
        user.user_id.clone().into(),
        customer_id.clone().into(),
        request.model.clone().into(),
        messages_json.into(),
        "[streaming - response not captured]".into(),
        "streaming".into(),
        now.into(),
    ])?
    .run()
    .await;

    // Pass through the upstream response with SSE headers
    let response_headers = Headers::new();
    response_headers.set("Content-Type", "text/event-stream")?;
    response_headers.set("Cache-Control", "no-cache")?;

    // Use the upstream response body stream directly
    Ok(upstream_resp.with_headers(response_headers))
}

//! The client for the account's cloud memories.
//!
//! The Rust port of `packages/openagents-cli/src/memory-client.ts`. Memories
//! live in the openagents.com database, account-scoped, not in a file on this
//! machine. Three calls are the whole surface — write one, read them back,
//! remove one — because that is what the store does. Nothing here recalls
//! anything; recall runs server-side inside `POST /api/v1/responses`.
//!
//! There is no update. A correction is a new memory carrying `supersedes`, so
//! the store keeps the chain a wrong memory was corrected through rather than
//! overwriting the row that was wrong.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::tracker::{ApiError, error_fields, error_sentence, header_request_id, urlencode};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryRecord {
    pub id: String,
    pub bucket: String,
    pub body: String,
    /// The thread or session the request came out of, when one was named.
    pub source_ref: Option<String>,
    /// The id of the memory that replaced this one, once one has.
    pub superseded_by: Option<String>,
    pub created_at: String,
}

/// The two buckets the server accepts. Checked here rather than left to the
/// API so a typo costs a sentence instead of a round trip.
pub fn read_bucket(raw: &str) -> Result<&'static str, ApiError> {
    match raw.trim().to_lowercase().as_str() {
        "user" => Ok("user"),
        "learned" => Ok("learned"),
        _ => Err(ApiError::Input(format!(
            "--bucket must be \"user\" or \"learned\", not \"{}\".",
            raw
        ))),
    }
}

pub struct MemoryClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

fn parse_memory(value: &Value) -> MemoryRecord {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(String::from);
    MemoryRecord {
        id: text("id").unwrap_or_default(),
        bucket: text("bucket").unwrap_or_else(|| "user".to_string()),
        body: text("body").unwrap_or_default(),
        source_ref: text("source_ref"),
        superseded_by: text("superseded_by"),
        created_at: text("created_at").unwrap_or_default(),
    }
}

impl MemoryClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", tok)) {
                map.insert(AUTHORIZATION, val);
            }
        }
        map
    }

    async fn request(
        &self,
        operation: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        accepted: &[u16],
    ) -> Result<Value, ApiError> {
        let url = format!("{}/{}", self.api_base, path.trim_start_matches('/'));
        let mut builder = match method {
            "GET" => self.http.get(&url),
            "POST" => self.http.post(&url),
            "DELETE" => self.http.delete(&url),
            other => {
                return Err(ApiError::Input(format!(
                    "{} is not an HTTP method this client sends.",
                    other
                )));
            }
        }
        .headers(self.headers());
        if let Some(payload) = body {
            builder = builder.json(&payload);
        }

        crate::diag::request(method, &url);
        let response = builder.send().await.map_err(|e| {
            crate::diag::transport(&url, &e.to_string());
            ApiError::Transport {
                operation: operation.to_string(),
                why: e.to_string(),
            }
        })?;
        let status = response.status().as_u16();
        crate::diag::response(status, &url);
        // Read before the body is consumed; the header outranks the body's own
        // `request_id`, as it does in the TypeScript transport.
        let header_id = header_request_id(&response);
        let text = response.text().await.map_err(|e| ApiError::Transport {
            operation: operation.to_string(),
            why: e.to_string(),
        })?;

        if !accepted.contains(&status) {
            let message = error_sentence(&text, status);
            crate::diag::refused(status, &message);
            let (code, body_id) = error_fields(&text);
            return Err(ApiError::Refused {
                operation: operation.to_string(),
                status,
                message,
                code,
                request_id: header_id.or(body_id),
            });
        }
        serde_json::from_str(&text).map_err(|e| ApiError::Malformed {
            operation: operation.to_string(),
            why: e.to_string(),
        })
    }

    pub async fn list_memories(
        &self,
        bucket: Option<&str>,
        limit: Option<u32>,
        include_superseded: bool,
    ) -> Result<Vec<MemoryRecord>, ApiError> {
        let mut query: Vec<String> = Vec::new();
        if let Some(name) = bucket {
            query.push(format!("bucket={}", urlencode(read_bucket(name)?)));
        }
        if let Some(count) = limit {
            query.push(format!("limit={}", count));
        }
        // The flag is only ever sent as `true`. Its absence is the default, and
        // sending `false` would read as a narrowing the server does not define.
        if include_superseded {
            query.push("include_superseded=true".to_string());
        }
        let path = if query.is_empty() {
            "memories".to_string()
        } else {
            format!("memories?{}", query.join("&"))
        };

        let body = self
            .request("list memories", "GET", &path, None, &[200])
            .await?;
        let rows = body
            .get("memories")
            .and_then(Value::as_array)
            .ok_or_else(|| ApiError::Malformed {
                operation: "list memories".to_string(),
                why: "no `memories` array in the response".to_string(),
            })?;
        Ok(rows.iter().map(parse_memory).collect())
    }

    pub async fn add_memory(
        &self,
        body_text: &str,
        bucket: Option<&str>,
        supersedes: Option<&str>,
        source_ref: Option<&str>,
    ) -> Result<MemoryRecord, ApiError> {
        if body_text.trim().is_empty() {
            return Err(ApiError::Input(
                "A memory needs a body to store.".to_string(),
            ));
        }
        // The server defaults an absent bucket to `user`, but a write path that
        // names its bucket keeps working if that default ever moves.
        let name = match bucket {
            Some(raw) => read_bucket(raw)?,
            None => "user",
        };
        let mut payload = json!({ "body": body_text, "bucket": name });
        if let Some(id) = supersedes {
            payload["supersedes"] = json!(id);
        }
        if let Some(reference) = source_ref {
            payload["source_ref"] = json!(reference);
        }

        let body = self
            .request("write memory", "POST", "memories", Some(payload), &[201])
            .await?;
        Ok(parse_memory(body.get("memory").unwrap_or(&body)))
    }

    /// Removes one memory outright, and returns the row the server removed.
    pub async fn delete_memory(&self, memory_id: &str) -> Result<MemoryRecord, ApiError> {
        if memory_id.trim().is_empty() {
            return Err(ApiError::Input(
                "Pass the id of the memory to remove.".to_string(),
            ));
        }
        let body = self
            .request(
                "remove memory",
                "DELETE",
                &format!("memories/{}", urlencode(memory_id.trim())),
                None,
                &[200],
            )
            .await?;
        Ok(parse_memory(body.get("memory").unwrap_or(&body)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_the_server_does_not_define_is_refused_before_the_round_trip() {
        assert_eq!(read_bucket("USER").unwrap(), "user");
        assert_eq!(read_bucket(" learned ").unwrap(), "learned");
        let error = read_bucket("global").unwrap_err().to_string();
        assert!(error.contains("global"), "{error}");
    }
}

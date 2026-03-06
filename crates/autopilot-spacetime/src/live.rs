//! Blocking helpers for the live OpenAgents `autopilot-sync` Spacetime module.

use std::collections::BTreeMap;

use reqwest::StatusCode;
use reqwest::blocking::Client as BlockingClient;
use serde_json::{Map, Value, json};

pub const DEFAULT_LIVE_PRESENCE_HEARTBEAT_INTERVAL_MS: u64 = 5_000;
pub const DEFAULT_LIVE_PRESENCE_STALE_AFTER_MS: u64 = 30_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveSpacetimeClientTarget {
    pub base_url: String,
    pub database: String,
    pub auth_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LivePresenceSummary {
    pub providers_online: u64,
    pub node_online: bool,
    pub node_last_seen_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveCheckpointRow {
    pub stream_id: String,
    pub last_applied_seq: u64,
    pub durable_offset: u64,
}

#[derive(Clone, Debug)]
pub struct LiveSpacetimeClient {
    client: BlockingClient,
    target: LiveSpacetimeClientTarget,
}

impl LiveSpacetimeClient {
    pub fn new(base_url: &str, database: &str, auth_token: Option<String>) -> Result<Self, String> {
        let base_url = normalize_http_base_url(base_url)?;
        let database = database.trim().to_string();
        if database.is_empty() {
            return Err("database must not be empty".to_string());
        }
        let auth_token = auth_token
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let client = BlockingClient::builder()
            .build()
            .map_err(|error| format!("spacetime blocking client init failed: {error}"))?;
        let auth_token = if let Some(token) = auth_token {
            Some(token)
        } else {
            Some(mint_host_identity_token(&client, base_url.as_str())?)
        };
        Ok(Self {
            client,
            target: LiveSpacetimeClientTarget {
                base_url,
                database,
                auth_token,
            },
        })
    }

    #[must_use]
    pub fn target(&self) -> &LiveSpacetimeClientTarget {
        &self.target
    }

    pub fn request_presence_challenge(
        &self,
        node_id: &str,
        session_id: &str,
        worker_id: Option<&str>,
        region: &str,
    ) -> Result<String, String> {
        self.call_reducer(
            "request_nostr_presence_challenge",
            &json!([
                node_id.trim(),
                session_id.trim(),
                worker_id.unwrap_or_default().trim(),
                region.trim(),
            ]),
        )?;
        let query = format!(
            "SELECT challenge FROM nostr_presence_claim WHERE node_id = {} LIMIT 1",
            sql_string_literal(node_id),
        );
        let rows = self.query_sql_rows(query.as_str())?;
        rows.first()
            .and_then(|row| row.get("challenge"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| "nostr presence challenge row missing".to_string())
    }

    pub fn bind_presence_identity(
        &self,
        node_id: &str,
        nostr_pubkey_hex: &str,
        nostr_pubkey_npub: Option<&str>,
        challenge_signature_hex: &str,
    ) -> Result<(), String> {
        self.call_reducer(
            "bind_nostr_presence_identity",
            &json!([
                node_id.trim(),
                nostr_pubkey_hex.trim(),
                nostr_pubkey_npub.unwrap_or_default().trim(),
                challenge_signature_hex.trim(),
            ]),
        )?;
        Ok(())
    }

    pub fn heartbeat(&self, node_id: &str) -> Result<(), String> {
        self.call_reducer("heartbeat", &json!([node_id.trim()]))?;
        Ok(())
    }

    pub fn register_offline(&self, node_id: &str) -> Result<(), String> {
        self.call_reducer("register_offline", &json!([node_id.trim()]))?;
        Ok(())
    }

    pub fn presence_summary(
        &self,
        node_id: &str,
        now_unix_ms: u64,
        stale_after_ms: u64,
    ) -> Result<LivePresenceSummary, String> {
        let online_floor = now_unix_ms.saturating_sub(stale_after_ms);
        let provider_query = format!(
            "SELECT COUNT(*) AS providers_online FROM active_connection \
             WHERE last_seen_unix_ms >= {} AND session_id != {}",
            online_floor,
            sql_string_literal("unbound"),
        );
        let providers_online = self
            .query_sql_rows(provider_query.as_str())?
            .first()
            .and_then(|row| row.get("providers_online"))
            .and_then(value_as_u64)
            .unwrap_or(0);

        let node_query = format!(
            "SELECT last_seen_unix_ms FROM active_connection WHERE node_id = {} LIMIT 1",
            sql_string_literal(node_id),
        );
        let node_last_seen_unix_ms = self
            .query_sql_rows(node_query.as_str())?
            .first()
            .and_then(|row| row.get("last_seen_unix_ms"))
            .and_then(value_as_u64);

        Ok(LivePresenceSummary {
            providers_online,
            node_online: node_last_seen_unix_ms.is_some_and(|last_seen| last_seen >= online_floor),
            node_last_seen_unix_ms,
        })
    }

    pub fn ack_checkpoint(
        &self,
        client_id: &str,
        stream_id: &str,
        last_applied_seq: u64,
        durable_offset: u64,
    ) -> Result<(), String> {
        self.call_reducer(
            "ack_stream_checkpoint",
            &json!([
                client_id.trim(),
                stream_id.trim(),
                last_applied_seq,
                durable_offset,
            ]),
        )?;
        Ok(())
    }

    pub fn list_checkpoints(&self, client_id: &str) -> Result<Vec<LiveCheckpointRow>, String> {
        let query = format!(
            "SELECT stream_id, last_applied_seq, durable_offset \
             FROM stream_checkpoint WHERE client_id = {}",
            sql_string_literal(client_id),
        );
        let rows = self.query_sql_rows(query.as_str())?;
        let mut checkpoints = rows
            .into_iter()
            .filter_map(|row| {
                let stream_id = row.get("stream_id").and_then(Value::as_str)?;
                let last_applied_seq = row.get("last_applied_seq").and_then(value_as_u64)?;
                let durable_offset = row.get("durable_offset").and_then(value_as_u64)?;
                Some(LiveCheckpointRow {
                    stream_id: stream_id.to_string(),
                    last_applied_seq,
                    durable_offset,
                })
            })
            .collect::<Vec<_>>();
        checkpoints.sort_by(|left, right| left.stream_id.cmp(&right.stream_id));
        Ok(checkpoints)
    }

    fn call_reducer(&self, reducer: &str, args: &Value) -> Result<Value, String> {
        let endpoint = format!(
            "{}/v1/database/{}/call/{}",
            self.target.base_url, self.target.database, reducer
        );
        let mut request = self
            .client
            .post(endpoint.as_str())
            .header("accept", "application/json")
            .header("content-type", "application/json")
            .json(args);
        if let Some(token) = self.target.auth_token.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|error| format!("spacetime reducer call failed: {error}"))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "spacetime reducer call failed status={} body={}",
                status.as_u16(),
                body
            ));
        }
        if body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(body.as_str())
            .map_err(|error| format!("spacetime reducer response parse failed: {error}"))
    }

    fn query_sql_rows(&self, query: &str) -> Result<Vec<BTreeMap<String, Value>>, String> {
        let value = self.query_sql(query)?;
        decode_sql_rows(&value)
    }

    fn query_sql(&self, query: &str) -> Result<Value, String> {
        let endpoint = format!(
            "{}/v1/database/{}/sql",
            self.target.base_url, self.target.database
        );
        let mut request = self
            .client
            .post(endpoint.as_str())
            .header("accept", "application/json")
            .header("content-type", "text/plain")
            .body(query.to_string());
        if let Some(token) = self.target.auth_token.as_deref() {
            request = request.bearer_auth(token);
        }
        let response = request
            .send()
            .map_err(|error| format!("spacetime sql request failed: {error}"))?;
        if response.status().is_success() {
            return response
                .json::<Value>()
                .map_err(|error| format!("spacetime sql parse failed: {error}"));
        }

        if response.status() == StatusCode::METHOD_NOT_ALLOWED
            || response.status() == StatusCode::NOT_FOUND
            || response.status() == StatusCode::BAD_REQUEST
        {
            let mut fallback = self
                .client
                .get(endpoint.as_str())
                .header("accept", "application/json")
                .query(&[("query", query)]);
            if let Some(token) = self.target.auth_token.as_deref() {
                fallback = fallback.bearer_auth(token);
            }
            let fallback_response = fallback
                .send()
                .map_err(|error| format!("spacetime sql GET fallback failed: {error}"))?;
            let status = fallback_response.status();
            if status.is_success() {
                return fallback_response
                    .json::<Value>()
                    .map_err(|error| format!("spacetime sql fallback parse failed: {error}"));
            }
            let body = fallback_response.text().unwrap_or_default();
            return Err(format!(
                "spacetime sql GET fallback failed status={} body={}",
                status.as_u16(),
                body
            ));
        }

        let status = response.status();
        let body = response.text().unwrap_or_default();
        Err(format!(
            "spacetime sql failed status={} body={}",
            status.as_u16(),
            body
        ))
    }
}

fn normalize_http_base_url(value: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err("base_url must not be empty".to_string());
    }
    let parsed =
        reqwest::Url::parse(normalized).map_err(|error| format!("invalid base_url: {error}"))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(format!("unsupported base_url scheme: {scheme}"));
    }
    Ok(normalized.trim_end_matches('/').to_string())
}

fn mint_host_identity_token(client: &BlockingClient, base_url: &str) -> Result<String, String> {
    let endpoint = format!("{base_url}/v1/identity");
    let response = client
        .post(endpoint.as_str())
        .header("accept", "application/json")
        .send()
        .map_err(|error| format!("spacetime identity mint request failed: {error}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "spacetime identity mint failed status={} body={}",
            status.as_u16(),
            body
        ));
    }
    let payload = serde_json::from_str::<Value>(body.as_str())
        .map_err(|error| format!("spacetime identity mint parse failed: {error}"))?;
    payload
        .get("token")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "spacetime identity mint response missing token".to_string())
}

fn sql_string_literal(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "''"))
}

fn decode_sql_rows(value: &Value) -> Result<Vec<BTreeMap<String, Value>>, String> {
    let statements = value
        .as_array()
        .ok_or_else(|| "spacetime sql response was not an array".to_string())?;
    let mut rows = Vec::new();
    for statement in statements {
        rows.extend(decode_statement_rows(statement)?);
    }
    Ok(rows)
}

fn decode_statement_rows(statement: &Value) -> Result<Vec<BTreeMap<String, Value>>, String> {
    let rows = statement
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| "spacetime sql statement missing rows".to_string())?;
    let field_names = statement
        .get("schema")
        .and_then(|schema| schema.get("elements"))
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |elements| {
            elements.iter().map(decode_field_name).collect::<Vec<_>>()
        });

    rows.iter()
        .map(|row| decode_row(row, &field_names))
        .collect::<Result<Vec<_>, _>>()
}

fn decode_row(
    row: &Value,
    field_names: &[Option<String>],
) -> Result<BTreeMap<String, Value>, String> {
    if let Some(object) = row.as_object() {
        return Ok(object_to_sorted_map(object));
    }

    let values = row
        .as_array()
        .ok_or_else(|| "spacetime sql row was not an object or array".to_string())?;
    let mut decoded = BTreeMap::new();
    for (index, value) in values.iter().enumerate() {
        let name = field_names
            .get(index)
            .and_then(|name| name.clone())
            .unwrap_or_else(|| format!("field_{index}"));
        decoded.insert(name, value.clone());
    }
    Ok(decoded)
}

fn decode_field_name(element: &Value) -> Option<String> {
    let name = element.get("name")?;
    if let Some(name) = name.as_str() {
        return Some(name.to_string());
    }
    let object = name.as_object()?;
    object
        .get("some")
        .or_else(|| object.get("Some"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn object_to_sorted_map(object: &Map<String, Value>) -> BTreeMap<String, Value> {
    object
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn value_as_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse::<u64>().ok()))
}

#[cfg(test)]
mod tests {
    use super::{LiveSpacetimeClient, decode_sql_rows, sql_string_literal};
    use serde_json::json;

    #[test]
    fn sql_row_decoder_maps_array_rows_using_schema_names() {
        let payload = json!([
            {
                "schema": {
                    "elements": [
                        { "name": { "some": "stream_id" } },
                        { "name": { "some": "last_applied_seq" } }
                    ]
                },
                "rows": [
                    ["runtime.command", 7]
                ]
            }
        ]);

        let rows = decode_sql_rows(&payload).expect("sql rows should decode");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].get("stream_id").and_then(|value| value.as_str()),
            Some("runtime.command")
        );
        assert_eq!(
            rows[0]
                .get("last_applied_seq")
                .and_then(|value| value.as_u64()),
            Some(7)
        );
    }

    #[test]
    fn sql_row_decoder_preserves_object_rows() {
        let payload = json!([
            {
                "schema": { "elements": [] },
                "rows": [
                    { "providers_online": 3 }
                ]
            }
        ]);

        let rows = decode_sql_rows(&payload).expect("sql rows should decode");
        assert_eq!(
            rows[0]
                .get("providers_online")
                .and_then(|value| value.as_u64()),
            Some(3)
        );
    }

    #[test]
    fn sql_string_literal_escapes_single_quotes() {
        assert_eq!(sql_string_literal("node'o"), "'node''o'");
    }

    #[test]
    fn live_client_rejects_invalid_base_url() {
        let error = LiveSpacetimeClient::new("://bad", "autopilot", None)
            .expect_err("invalid base url should fail");
        assert!(error.contains("invalid base_url"));
    }
}

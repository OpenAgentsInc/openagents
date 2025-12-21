//! NIP-45: Event Counts
//!
//! This NIP defines the COUNT verb for relays to provide event counts without
//! retrieving full events. This is useful for expensive queries like follower
//! counts where clients only need the count, not the actual events.
//!
//! ## Protocol
//!
//! Request format:
//! ```json
//! ["COUNT", <query_id>, <filters JSON>...]
//! ```
//!
//! Response format:
//! ```json
//! ["COUNT", <query_id>, {"count": <integer>}]
//! ["COUNT", <query_id>, {"count": <integer>, "approximate": <true|false>}]
//! ```
//!
//! Relays may refuse COUNT requests with a CLOSED message.
//!
//! ## Examples
//!
//! ```
//! use nostr::nip45::{CountRequest, CountResponse};
//!
//! // Create a count request for followers
//! let request = CountRequest::new("query-1", vec![
//!     serde_json::json!({"kinds": [3], "#p": ["pubkey-hex"]})
//! ]);
//!
//! // Create a count response
//! let response = CountResponse::new("query-1", 238, false);
//! assert_eq!(response.count, 238);
//! assert_eq!(response.approximate, Some(false));
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur during NIP-45 operations.
#[derive(Debug, Error)]
pub enum Nip45Error {
    #[error("invalid COUNT request format")]
    InvalidRequestFormat,

    #[error("invalid COUNT response format")]
    InvalidResponseFormat,

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("missing query ID")]
    MissingQueryId,

    #[error("no filters provided")]
    NoFilters,
}

/// COUNT request from client to relay.
///
/// Requests the count of events matching the provided filters.
/// Multiple filters are OR'd together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountRequest {
    /// Query identifier
    pub query_id: String,
    /// Filters to apply (same format as NIP-01 REQ filters)
    pub filters: Vec<Value>,
}

impl CountRequest {
    /// Create a new COUNT request.
    ///
    /// # Arguments
    /// * `query_id` - Unique identifier for this query
    /// * `filters` - One or more filters (OR'd together)
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountRequest;
    ///
    /// let request = CountRequest::new("q1", vec![
    ///     serde_json::json!({"kinds": [1], "authors": ["pubkey-hex"]})
    /// ]);
    /// ```
    pub fn new(query_id: impl Into<String>, filters: Vec<Value>) -> Self {
        Self {
            query_id: query_id.into(),
            filters,
        }
    }

    /// Serialize to JSON array format.
    ///
    /// Returns: `["COUNT", query_id, filter1, filter2, ...]`
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountRequest;
    ///
    /// let request = CountRequest::new("q1", vec![
    ///     serde_json::json!({"kinds": [1]})
    /// ]);
    /// let json = request.to_json().unwrap();
    /// // ["COUNT", "q1", {"kinds": [1]}]
    /// ```
    pub fn to_json(&self) -> Result<String, Nip45Error> {
        let mut array = vec![
            serde_json::json!("COUNT"),
            serde_json::json!(&self.query_id),
        ];
        array.extend(self.filters.iter().cloned());
        Ok(serde_json::to_string(&array)?)
    }

    /// Parse a COUNT request from JSON array.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountRequest;
    ///
    /// let json = r#"["COUNT", "q1", {"kinds": [1]}]"#;
    /// let request = CountRequest::from_json(json).unwrap();
    /// assert_eq!(request.query_id, "q1");
    /// assert_eq!(request.filters.len(), 1);
    /// ```
    pub fn from_json(json: &str) -> Result<Self, Nip45Error> {
        let array: Vec<Value> = serde_json::from_str(json)?;

        if array.len() < 3 {
            return Err(Nip45Error::InvalidRequestFormat);
        }

        if array[0].as_str() != Some("COUNT") {
            return Err(Nip45Error::InvalidRequestFormat);
        }

        let query_id = array[1]
            .as_str()
            .ok_or(Nip45Error::MissingQueryId)?
            .to_string();

        let filters = array[2..].to_vec();

        if filters.is_empty() {
            return Err(Nip45Error::NoFilters);
        }

        Ok(Self { query_id, filters })
    }
}

/// COUNT response from relay to client.
///
/// Contains the count of events matching the filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountResponse {
    /// Query identifier (matches the request)
    pub query_id: String,
    /// Number of events matching the filters
    pub count: u64,
    /// Whether the count is approximate (optional)
    /// If not present, the count is assumed to be exact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
}

impl CountResponse {
    /// Create a new COUNT response with exact count.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountResponse;
    ///
    /// let response = CountResponse::exact("q1", 42);
    /// assert_eq!(response.count, 42);
    /// assert_eq!(response.approximate, None);
    /// ```
    pub fn exact(query_id: impl Into<String>, count: u64) -> Self {
        Self {
            query_id: query_id.into(),
            count,
            approximate: None,
        }
    }

    /// Create a new COUNT response with specified approximation.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountResponse;
    ///
    /// let response = CountResponse::new("q1", 1000, true);
    /// assert_eq!(response.count, 1000);
    /// assert_eq!(response.approximate, Some(true));
    /// ```
    pub fn new(query_id: impl Into<String>, count: u64, approximate: bool) -> Self {
        Self {
            query_id: query_id.into(),
            count,
            approximate: Some(approximate),
        }
    }

    /// Create an approximate COUNT response.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountResponse;
    ///
    /// let response = CountResponse::approximate("q1", 93412452);
    /// assert_eq!(response.count, 93412452);
    /// assert_eq!(response.approximate, Some(true));
    /// ```
    pub fn approximate(query_id: impl Into<String>, count: u64) -> Self {
        Self::new(query_id, count, true)
    }

    /// Check if this count is approximate.
    pub fn is_approximate(&self) -> bool {
        self.approximate.unwrap_or(false)
    }

    /// Serialize to JSON array format.
    ///
    /// Returns: `["COUNT", query_id, {"count": n}]` or
    ///          `["COUNT", query_id, {"count": n, "approximate": true}]`
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountResponse;
    ///
    /// let response = CountResponse::exact("q1", 42);
    /// let json = response.to_json().unwrap();
    /// // ["COUNT", "q1", {"count": 42}]
    /// ```
    pub fn to_json(&self) -> Result<String, Nip45Error> {
        let mut result = serde_json::json!({
            "count": self.count
        });

        if let Some(approximate) = self.approximate {
            result["approximate"] = serde_json::json!(approximate);
        }

        let array = serde_json::json!(["COUNT", self.query_id, result]);
        Ok(serde_json::to_string(&array)?)
    }

    /// Parse a COUNT response from JSON array.
    ///
    /// # Example
    ///
    /// ```
    /// use nostr::nip45::CountResponse;
    ///
    /// let json = r#"["COUNT", "q1", {"count": 42}]"#;
    /// let response = CountResponse::from_json(json).unwrap();
    /// assert_eq!(response.query_id, "q1");
    /// assert_eq!(response.count, 42);
    /// ```
    pub fn from_json(json: &str) -> Result<Self, Nip45Error> {
        let array: Vec<Value> = serde_json::from_str(json)?;

        if array.len() != 3 {
            return Err(Nip45Error::InvalidResponseFormat);
        }

        if array[0].as_str() != Some("COUNT") {
            return Err(Nip45Error::InvalidResponseFormat);
        }

        let query_id = array[1]
            .as_str()
            .ok_or(Nip45Error::MissingQueryId)?
            .to_string();

        let result = array[2]
            .as_object()
            .ok_or(Nip45Error::InvalidResponseFormat)?;

        let count = result
            .get("count")
            .and_then(|v| v.as_u64())
            .ok_or(Nip45Error::InvalidResponseFormat)?;

        let approximate = result.get("approximate").and_then(|v| v.as_bool());

        Ok(Self {
            query_id,
            count,
            approximate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_request_new() {
        let request = CountRequest::new(
            "query-1",
            vec![serde_json::json!({"kinds": [1], "authors": ["pubkey"]})],
        );
        assert_eq!(request.query_id, "query-1");
        assert_eq!(request.filters.len(), 1);
    }

    #[test]
    fn test_count_request_to_json() {
        let request = CountRequest::new("q1", vec![serde_json::json!({"kinds": [1]})]);
        let json = request.to_json().unwrap();
        assert!(json.contains("COUNT"));
        assert!(json.contains("q1"));
        assert!(json.contains("kinds"));
    }

    #[test]
    fn test_count_request_from_json() {
        let json = r#"["COUNT", "q1", {"kinds": [1]}]"#;
        let request = CountRequest::from_json(json).unwrap();
        assert_eq!(request.query_id, "q1");
        assert_eq!(request.filters.len(), 1);
    }

    #[test]
    fn test_count_request_multiple_filters() {
        let json = r#"["COUNT", "q1", {"kinds": [1]}, {"kinds": [7]}]"#;
        let request = CountRequest::from_json(json).unwrap();
        assert_eq!(request.query_id, "q1");
        assert_eq!(request.filters.len(), 2);
    }

    #[test]
    fn test_count_request_invalid_format() {
        let json = r#"["COUNT", "q1"]"#; // No filters
        assert!(CountRequest::from_json(json).is_err());

        let json = r#"["WRONG", "q1", {}]"#; // Wrong verb
        assert!(CountRequest::from_json(json).is_err());
    }

    #[test]
    fn test_count_response_exact() {
        let response = CountResponse::exact("q1", 42);
        assert_eq!(response.query_id, "q1");
        assert_eq!(response.count, 42);
        assert_eq!(response.approximate, None);
        assert!(!response.is_approximate());
    }

    #[test]
    fn test_count_response_approximate() {
        let response = CountResponse::approximate("q1", 1000);
        assert_eq!(response.query_id, "q1");
        assert_eq!(response.count, 1000);
        assert_eq!(response.approximate, Some(true));
        assert!(response.is_approximate());
    }

    #[test]
    fn test_count_response_new() {
        let response = CountResponse::new("q1", 500, false);
        assert_eq!(response.query_id, "q1");
        assert_eq!(response.count, 500);
        assert_eq!(response.approximate, Some(false));
        assert!(!response.is_approximate());
    }

    #[test]
    fn test_count_response_to_json_exact() {
        let response = CountResponse::exact("q1", 42);
        let json = response.to_json().unwrap();
        assert!(json.contains("COUNT"));
        assert!(json.contains("q1"));
        assert!(json.contains("\"count\":42"));
        // Should not contain approximate field for exact counts
        assert!(!json.contains("approximate"));
    }

    #[test]
    fn test_count_response_to_json_approximate() {
        let response = CountResponse::approximate("q1", 93412452);
        let json = response.to_json().unwrap();
        assert!(json.contains("COUNT"));
        assert!(json.contains("q1"));
        assert!(json.contains("93412452"));
        assert!(json.contains("\"approximate\":true"));
    }

    #[test]
    fn test_count_response_from_json_exact() {
        let json = r#"["COUNT", "q1", {"count": 238}]"#;
        let response = CountResponse::from_json(json).unwrap();
        assert_eq!(response.query_id, "q1");
        assert_eq!(response.count, 238);
        assert_eq!(response.approximate, None);
    }

    #[test]
    fn test_count_response_from_json_approximate() {
        let json = r#"["COUNT", "q1", {"count": 93412452, "approximate": true}]"#;
        let response = CountResponse::from_json(json).unwrap();
        assert_eq!(response.query_id, "q1");
        assert_eq!(response.count, 93412452);
        assert_eq!(response.approximate, Some(true));
        assert!(response.is_approximate());
    }

    #[test]
    fn test_count_response_from_json_not_approximate() {
        let json = r#"["COUNT", "q1", {"count": 5, "approximate": false}]"#;
        let response = CountResponse::from_json(json).unwrap();
        assert_eq!(response.query_id, "q1");
        assert_eq!(response.count, 5);
        assert_eq!(response.approximate, Some(false));
        assert!(!response.is_approximate());
    }

    #[test]
    fn test_count_response_invalid_format() {
        let json = r#"["COUNT", "q1"]"#; // Missing result
        assert!(CountResponse::from_json(json).is_err());

        let json = r#"["WRONG", "q1", {"count": 1}]"#; // Wrong verb
        assert!(CountResponse::from_json(json).is_err());

        let json = r#"["COUNT", "q1", {"wrong": 1}]"#; // Missing count field
        assert!(CountResponse::from_json(json).is_err());
    }

    #[test]
    fn test_roundtrip_request() {
        let original = CountRequest::new(
            "test-query",
            vec![
                serde_json::json!({"kinds": [3], "#p": ["pubkey123"]}),
                serde_json::json!({"kinds": [1, 7], "authors": ["author123"]}),
            ],
        );

        let json = original.to_json().unwrap();
        let parsed = CountRequest::from_json(&json).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_response_exact() {
        let original = CountResponse::exact("q-123", 999);
        let json = original.to_json().unwrap();
        let parsed = CountResponse::from_json(&json).unwrap();

        assert_eq!(original.query_id, parsed.query_id);
        assert_eq!(original.count, parsed.count);
        // None and Some(false) are semantically equivalent for is_approximate
    }

    #[test]
    fn test_roundtrip_response_approximate() {
        let original = CountResponse::approximate("q-456", 1000000);
        let json = original.to_json().unwrap();
        let parsed = CountResponse::from_json(&json).unwrap();

        assert_eq!(original, parsed);
    }
}

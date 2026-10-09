//! Hosted tools (`docs/inference/gateway.md`, section 3, P2): tools the
//! gateway runs itself, so any model with function calling can use them.
//!
//! The Open Responses spec defines no built-in tools; it asks implementors
//! to prefix their own. Ours are request tools and output items named
//! `openagents:<tool>`. Web search is the first:
//!
//! - Request: `{"type": "openagents:web_search"}` in `tools`, with an
//!   optional `max_results` (1 to 10, default 5).
//! - The model sees an ordinary function, [`FUNCTION`], taking a `query`.
//!   When it calls it, the gateway searches through a [`WebSearch`]
//!   provider, hands the results back as the function's output, and lets
//!   the model continue, until it answers without searching (or the
//!   request's `max_tool_calls` is spent).
//! - Output: each search is one `openagents:web_search_call` item, in
//!   order with the model's own items:
//!
//! ```json
//! {"type": "openagents:web_search_call", "id": "ws_...", "status": "completed",
//!  "action": {"type": "search", "query": "..."},
//!  "results": [{"title": "...", "url": "...", "snippet": "..."}]}
//! ```
//!
//! A search provider has privacy terms like any upstream: a `strict`
//! request (the default) searches only through a provider whose terms
//! are verified zero retention. Each search is priced at the provider's
//! list price plus the margin, and the price joins the response's cost.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::openagents::Privacy;
use crate::request::{FunctionTool, Tool};
use crate::upstream::{BoxFuture, PrivacyTerms};

/// The request tool type for web search.
pub const WEB_SEARCH: &str = "openagents:web_search";
/// The output item type for one search.
pub const WEB_SEARCH_CALL: &str = "openagents:web_search_call";
/// The function name the model calls to search.
pub const FUNCTION: &str = "openagents_web_search";
/// Results per search when the request does not say.
pub const DEFAULT_RESULTS: u64 = 5;

/// One search result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub snippet: String,
}

/// A web search provider.
pub trait WebSearch: Send + Sync {
    /// The provider's name in records: `exa`.
    fn name(&self) -> &str;
    /// What the provider does with a query.
    fn privacy(&self) -> &PrivacyTerms;
    /// The provider's list price per search, in micro-US-dollars.
    fn price_micros(&self) -> u64;
    /// Up to `results` results for `query`.
    fn search<'a>(
        &'a self,
        query: &'a str,
        results: u64,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, String>>;
}

/// The hosted tools a request asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requested {
    /// Web search, with results per search.
    pub web_search: Option<u64>,
}

impl Requested {
    /// Whether any hosted tool was asked for.
    #[must_use]
    pub fn any(&self) -> bool {
        self.web_search.is_some()
    }
}

/// Splits `tools` into the hosted tools asked for and the tools the model
/// sees (hosted ones replaced by their functions).
///
/// # Errors
///
/// A `(param, message)` pair for a hosted tool we do not have, a bad
/// `max_results`, or a caller's function that takes a hosted function's
/// name.
pub fn split(tools: &[Tool]) -> Result<(Requested, Vec<Tool>), (String, String)> {
    let mut requested = Requested::default();
    let mut model_tools = Vec::new();
    for tool in tools {
        match tool {
            Tool::Function(function) if function.name == FUNCTION => {
                return Err((
                    "tools".to_owned(),
                    format!(
                        "`{FUNCTION}` is the name of a hosted tool; name your function something else."
                    ),
                ));
            }
            Tool::Unknown(value) => {
                let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
                if kind == WEB_SEARCH {
                    let results = match value.get("max_results") {
                        None | Some(Value::Null) => DEFAULT_RESULTS,
                        Some(number) => number
                            .as_u64()
                            .filter(|n| (1..=10).contains(n))
                            .ok_or_else(|| {
                                (
                                    "tools".to_owned(),
                                    "`max_results` on web search is a whole number from 1 to 10."
                                        .to_owned(),
                                )
                            })?,
                    };
                    requested.web_search = Some(results);
                    model_tools.push(Tool::Function(search_function()));
                } else if kind.starts_with("openagents:") {
                    return Err((
                        "tools".to_owned(),
                        format!(
                            "There is no hosted tool `{kind}`. The hosted tools are: `{WEB_SEARCH}`."
                        ),
                    ));
                } else {
                    model_tools.push(tool.clone());
                }
            }
            Tool::Function(_) => model_tools.push(tool.clone()),
        }
    }
    Ok((requested, model_tools))
}

/// The function the model calls to search.
#[must_use]
pub fn search_function() -> FunctionTool {
    FunctionTool {
        name: FUNCTION.to_owned(),
        description: Some(
            "Search the web. Returns titles, links, and snippets for the query. Use it for \
             current events or facts you are unsure of, then answer from the results and cite \
             the links."
                .to_owned(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {"query": {"type": "string", "description": "What to search for"}},
            "required": ["query"],
            "additionalProperties": false
        })),
        strict: Some(true),
        extra: crate::wire::Extra::new(),
    }
}

/// The query in a search call's arguments.
#[must_use]
pub fn query_of(arguments: &str) -> Option<String> {
    let value: Value = serde_json::from_str(arguments).ok()?;
    let query = value.get("query")?.as_str()?.trim();
    (!query.is_empty()).then(|| query.to_owned())
}

/// A search's output item.
#[must_use]
pub fn call_item(id: &str, query: &str, outcome: &Result<Vec<SearchResult>, String>) -> Value {
    match outcome {
        Ok(results) => json!({
            "type": WEB_SEARCH_CALL,
            "id": id,
            "status": "completed",
            "action": {"type": "search", "query": query},
            "results": results,
        }),
        Err(_) => json!({
            "type": WEB_SEARCH_CALL,
            "id": id,
            "status": "failed",
            "action": {"type": "search", "query": query},
            "results": [],
        }),
    }
}

/// What the model reads back from a search: the results as JSON, or a
/// sentence saying the search failed.
#[must_use]
pub fn function_output(outcome: &Result<Vec<SearchResult>, String>) -> String {
    match outcome {
        Ok(results) => json!({"results": results}).to_string(),
        Err(_) => json!({"error": "The search failed. Answer without it, or try another query."})
            .to_string(),
    }
}

/// Whether `provider` may take a query at `level`.
#[must_use]
pub fn allowed(provider: &dyn WebSearch, level: &Privacy) -> bool {
    provider.privacy().allows(level)
}

/// Exa (`https://api.exa.ai/search`), key in `EXA_API_KEY` or
/// `EXA_API_KEY_FILE`. Its terms are taken as unverified (`standard` only)
/// until `EXA_TERMS_VERIFIED=zero-retention` says the account has zero
/// data retention.
pub struct Exa {
    url: String,
    key: crate::upstream::secret::Secret,
    privacy: PrivacyTerms,
    http: reqwest::Client,
}

/// Exa's list price per search with text for up to ten results, in
/// micro-dollars: $5 per 1,000 searches plus $1 per 1,000 pages of text,
/// five pages assumed (exa.ai/pricing as recorded on 2026-10-09; confirm
/// before it is printed on the rate card).
pub const EXA_PRICE_MICROS: u64 = 10_000;

impl Exa {
    /// The provider with the key from the environment, if one is set.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let key = crate::upstream::secret::KeyRef::new(&["EXA_API_KEY"], None).local()?;
        let url = std::env::var("EXA_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "https://api.exa.ai".to_owned());
        let verified = std::env::var("EXA_TERMS_VERIFIED").is_ok_and(|v| v == "zero-retention");
        Some(Self::new(&url, key, verified))
    }

    /// The provider at `base` (`https://api.exa.ai`) with `key`;
    /// `zero_retention` when the account's terms are verified.
    #[must_use]
    pub fn new(base: &str, key: crate::upstream::secret::Secret, zero_retention: bool) -> Self {
        Self {
            url: format!("{}/search", base.trim_end_matches('/')),
            key,
            privacy: if zero_retention {
                PrivacyTerms::zero_retention("EXA_TERMS_VERIFIED=zero-retention")
            } else {
                PrivacyTerms::unverified("Exa's data terms are not verified for this account")
            },
            http: crate::upstream::http::client(crate::upstream::http::CONNECT_TIMEOUT),
        }
    }
}

impl WebSearch for Exa {
    fn name(&self) -> &str {
        "exa"
    }

    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }

    fn price_micros(&self) -> u64 {
        EXA_PRICE_MICROS
    }

    fn search<'a>(
        &'a self,
        query: &'a str,
        results: u64,
    ) -> BoxFuture<'a, Result<Vec<SearchResult>, String>> {
        Box::pin(async move {
            let reply = self
                .http
                .post(&self.url)
                .header("x-api-key", self.key.expose())
                .timeout(std::time::Duration::from_secs(20))
                .json(&json!({
                    "query": query,
                    "numResults": results,
                    "contents": {"text": {"maxCharacters": 600}},
                }))
                .send()
                .await
                .map_err(|_| "the search provider could not be reached".to_owned())?;
            let status = reply.status().as_u16();
            if status != 200 {
                return Err(format!("the search provider answered {status}"));
            }
            let body: Value = reply
                .json()
                .await
                .map_err(|_| "the search provider sent an answer we could not read".to_owned())?;
            Ok(body
                .get("results")
                .and_then(Value::as_array)
                .map(|results| {
                    results
                        .iter()
                        .filter_map(|result| {
                            let url = result.get("url")?.as_str()?.to_owned();
                            let title = result
                                .get("title")
                                .and_then(Value::as_str)
                                .unwrap_or(&url)
                                .to_owned();
                            let snippet = result
                                .get("text")
                                .or_else(|| result.get("snippet"))
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .chars()
                                .take(600)
                                .collect();
                            Some(SearchResult {
                                title,
                                url,
                                snippet,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosted_tools_become_functions_and_unknown_ones_are_refused() {
        let tools: Vec<Tool> = serde_json::from_value(json!([
            {"type": "openagents:web_search", "max_results": 3},
            {"type": "function", "name": "get_weather"}
        ]))
        .unwrap();
        let (requested, model_tools) = split(&tools).unwrap();
        assert_eq!(requested.web_search, Some(3));
        assert!(matches!(&model_tools[0], Tool::Function(f) if f.name == FUNCTION));
        assert!(matches!(&model_tools[1], Tool::Function(f) if f.name == "get_weather"));

        let unknown: Vec<Tool> =
            serde_json::from_value(json!([{"type": "openagents:code"}])).unwrap();
        assert_eq!(split(&unknown).unwrap_err().0, "tools");
        let clash: Vec<Tool> =
            serde_json::from_value(json!([{"type": "function", "name": FUNCTION}])).unwrap();
        assert!(split(&clash).is_err());
        let bad: Vec<Tool> =
            serde_json::from_value(json!([{"type": "openagents:web_search", "max_results": 50}]))
                .unwrap();
        assert!(split(&bad).is_err());
        // A provider's own prefixed tool passes through untouched.
        let other: Vec<Tool> =
            serde_json::from_value(json!([{"type": "openai:web_search_preview"}])).unwrap();
        assert_eq!(split(&other).unwrap().1.len(), 1);
    }

    #[test]
    fn queries_and_items() {
        assert_eq!(
            query_of(r#"{"query": " rust 2026 "}"#).as_deref(),
            Some("rust 2026")
        );
        assert!(query_of(r#"{"query": ""}"#).is_none());
        assert!(query_of("not json").is_none());
        let found = Ok(vec![SearchResult {
            title: "T".into(),
            url: "https://example.com".into(),
            snippet: "S".into(),
        }]);
        let item = call_item("ws_1", "q", &found);
        assert_eq!(item["type"], WEB_SEARCH_CALL);
        assert_eq!(item["results"][0]["url"], "https://example.com");
        assert!(function_output(&found).contains("example.com"));
        let failed: Result<Vec<SearchResult>, String> = Err("down".into());
        assert_eq!(call_item("ws_2", "q", &failed)["status"], "failed");
        assert!(!function_output(&failed).contains("down"));
    }
}

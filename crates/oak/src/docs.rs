//! Bounded, credential-free access to the documentation bundled with this build.

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const SCHEMA: &str = "openagents.docs.v1";
const MAX_PAGE_BYTES: usize = 16_384;
const MAX_RESULTS: usize = 50;

struct Document {
    id: &'static str,
    title: &'static str,
    path: &'static str,
    content: &'static str,
    example: bool,
}

macro_rules! document {
    ($id:literal, $title:literal, $path:literal, $example:literal) => {
        Document {
            id: $id,
            title: $title,
            path: concat!("docs/decision-models/", $path),
            content: include_str!(concat!("../../../docs/decision-models/", $path)),
            example: $example,
        }
    };
}

static DOCUMENTS: &[Document] = &[
    document!("caller", "Decision API caller guide", "caller.md", false),
    document!(
        "classification",
        "Classification callers",
        "classification-callers.md",
        false
    ),
    document!("gateway", "Gateway admission", "gateway.md", false),
    document!(
        "api-spec",
        "Decision API specification",
        "decision-api.md",
        false
    ),
    document!("openapi", "OpenAPI contract", "openapi.yaml", false),
    document!(
        "classification-schema",
        "Classification request schema",
        "schemas/classify-request-v1.json",
        false
    ),
    document!(
        "classification-response-schema",
        "Classification response schema",
        "schemas/classify-response-v1.json",
        false
    ),
    document!(
        "candidate-admission",
        "Candidate admission",
        "candidate-admission.md",
        false
    ),
    document!(
        "mcp-documentation",
        "MCP documentation tools",
        "mcp-documentation.md",
        false
    ),
    document!("examples", "Caller examples", "examples/README.md", true),
    document!(
        "native-questions",
        "Native question example",
        "examples/questions.json",
        true
    ),
    document!(
        "classification-single",
        "Single-label example",
        "fixtures/classify-v1/single.json",
        true
    ),
    document!(
        "classification-multi",
        "Independent multi-label example",
        "fixtures/classify-v1/overlapping-labels.json",
        true
    ),
    document!(
        "classification-score",
        "Score example",
        "fixtures/classify-v1/score.json",
        true
    ),
    document!(
        "classification-binary",
        "Binary example",
        "fixtures/classify-v1/binary.json",
        true
    ),
];

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let mut hash = Sha256::new();
        for doc in DOCUMENTS {
            for field in [doc.id, doc.title, doc.path, doc.content] {
                hash.update((field.len() as u64).to_be_bytes());
                hash.update(field.as_bytes());
            }
            hash.update([u8::from(doc.example)]);
        }
        format!("{:x}", hash.finalize())
    })
}

fn metadata(doc: &Document) -> Value {
    json!({"id":doc.id, "title":doc.title, "source":format!("https://github.com/OpenAgentsInc/openagents/blob/main/{}",doc.path), "source_path":doc.path, "sha256":digest(doc.content.as_bytes()), "bytes":doc.content.len(), "example":doc.example})
}

/// A stable documentation error, independent of any inference credential.
#[derive(Debug)]
pub struct Error {
    /// Machine-readable error code.
    pub code: &'static str,
    /// Bounded explanation without supplied document content.
    pub message: &'static str,
}

fn invalid(message: &'static str) -> Error {
    Error {
        code: "invalid_arguments",
        message,
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Args {
    id: Option<String>,
    query: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
    max_bytes: Option<usize>,
}

fn offset(cursor: Option<&str>, scope: &str) -> Result<usize, Error> {
    let Some(cursor) = cursor else { return Ok(0) };
    if cursor.len() > 160 {
        return Err(invalid("cursor exceeds its bound"));
    }
    let parts: Vec<&str> = cursor.split(':').collect();
    if parts.len() != 3 || parts[0] != version() || parts[1] != digest(scope.as_bytes()) {
        return Err(Error {
            code: "stale_cursor",
            message: "cursor does not match this corpus and query",
        });
    }
    parts[2]
        .parse()
        .map_err(|_| invalid("cursor position is invalid"))
}

fn cursor(scope: &str, position: usize) -> String {
    format!("{}:{}:{position}", version(), digest(scope.as_bytes()))
}

fn envelope() -> Value {
    json!({"v":SCHEMA,"corpus_sha256":version(),"next_cursor":null})
}

/// Names served without resolving endpoint or credential configuration.
#[must_use]
pub fn handles(name: &str) -> bool {
    matches!(
        name,
        "list_docs" | "read_doc" | "search_docs" | "get_examples"
    )
}

/// Read a bounded page from the compiled corpus.
///
/// # Errors
///
/// Refuses unknown fields, stale or mismatched cursors, invalid bounds, and
/// missing documents. Document IDs never resolve to filesystem paths or URLs.
pub fn call(name: &str, arguments: Value) -> Result<Value, Error> {
    let allowed: &[&str] = match name {
        "list_docs" | "get_examples" => &["cursor", "limit"],
        "search_docs" => &["query", "cursor", "limit"],
        "read_doc" => &["id", "cursor", "max_bytes"],
        _ => return Err(invalid("unknown documentation tool")),
    };
    let object = arguments
        .as_object()
        .ok_or_else(|| invalid("documentation arguments must be an object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(invalid("this tool does not accept the supplied argument"));
    }
    if object.values().any(Value::is_null) {
        return Err(invalid("omit optional arguments instead of supplying null"));
    }
    let args: Args = serde_json::from_value(arguments)
        .map_err(|_| invalid("invalid documentation arguments"))?;
    if name == "read_doc" {
        return read(args);
    }
    if !matches!(name, "list_docs" | "search_docs" | "get_examples") {
        return Err(invalid("unknown documentation tool"));
    }
    if args.id.is_some() || args.max_bytes.is_some() {
        return Err(invalid("this tool does not accept id or max_bytes"));
    }
    let query = match (name, args.query.as_deref()) {
        ("search_docs", Some(query)) if !query.trim().is_empty() && query.len() <= 256 => {
            Some(query)
        }
        ("search_docs", _) => return Err(invalid("search query must contain 1 to 256 bytes")),
        (_, Some(_)) => return Err(invalid("this tool does not accept a search query")),
        _ => None,
    };
    let limit = args.limit.unwrap_or(10);
    if !(1..=MAX_RESULTS).contains(&limit) {
        return Err(invalid("limit must be between 1 and 50"));
    }
    let scope = format!("{name}:{}", query.unwrap_or(""));
    let start = offset(args.cursor.as_deref(), &scope)?;
    let folded = query.map(str::to_lowercase);
    let rows: Vec<Value> = DOCUMENTS
        .iter()
        .filter(|doc| name != "get_examples" || doc.example)
        .filter_map(|doc| {
            let mut row = metadata(doc);
            if let Some(query) = &folded {
                let (index, line) = doc
                    .content
                    .lines()
                    .enumerate()
                    .find(|(_, line)| line.to_lowercase().contains(query))?;
                row["line"] = json!(index + 1);
                row["snippet"] = json!(line.chars().take(240).collect::<String>());
                row["snippet_truncated"] = json!(line.chars().count() > 240);
            }
            Some(row)
        })
        .collect();
    if start > rows.len() {
        return Err(invalid("cursor is beyond the result set"));
    }
    let end = start.saturating_add(limit).min(rows.len());
    let mut result = envelope();
    result["documents"] = json!(rows[start..end]);
    result["total"] = json!(rows.len());
    if end < rows.len() {
        result["next_cursor"] = json!(cursor(&scope, end));
    }
    Ok(result)
}

fn read(args: Args) -> Result<Value, Error> {
    if args.query.is_some() || args.limit.is_some() {
        return Err(invalid("read_doc does not accept query or limit"));
    }
    let id = args
        .id
        .ok_or_else(|| invalid("read_doc requires a document id"))?;
    let doc = DOCUMENTS.iter().find(|doc| doc.id == id).ok_or(Error {
        code: "document_not_found",
        message: "no document has this id",
    })?;
    let maximum = args.max_bytes.unwrap_or(8192);
    if !(4..=MAX_PAGE_BYTES).contains(&maximum) {
        return Err(invalid("max_bytes must be between 4 and 16384"));
    }
    let scope = format!("read_doc:{id}");
    let start = offset(args.cursor.as_deref(), &scope)?;
    if start > doc.content.len() || !doc.content.is_char_boundary(start) {
        return Err(invalid("cursor is not a valid document boundary"));
    }
    let mut end = start.saturating_add(maximum).min(doc.content.len());
    while !doc.content.is_char_boundary(end) {
        end -= 1;
    }
    let mut result = envelope();
    result["document"] = metadata(doc);
    result["offset_bytes"] = json!(start);
    result["content"] = json!(&doc.content[start..end]);
    result["truncated"] = json!(end < doc.content.len());
    if end < doc.content.len() {
        result["next_cursor"] = json!(cursor(&scope, end));
    }
    Ok(result)
}

/// Tool schemas and annotations for the credential-free documentation lane.
#[must_use]
pub fn tools() -> Vec<Value> {
    let paging = json!({"type":"string","maxLength":160,"description":"Cursor from this tool for the same corpus and query."});
    let limit = json!({"type":"integer","minimum":1,"maximum":50});
    [
        ("list_docs","List bundled documentation metadata",json!({"cursor":paging,"limit":limit}),vec![]),
        ("read_doc","Read a bounded UTF-8 page by stable document ID",json!({"id":{"type":"string"},"cursor":paging,"max_bytes":{"type":"integer","minimum":4,"maximum":MAX_PAGE_BYTES}}),vec!["id"]),
        ("search_docs","Search the bundled corpus; return at most one bounded snippet per document",json!({"query":{"type":"string","minLength":1,"maxLength":256},"cursor":paging,"limit":limit}),vec!["query"]),
        ("get_examples","List bundled examples; use read_doc to retrieve their contents",json!({"cursor":paging,"limit":limit}),vec![]),
    ].into_iter().map(|(name,description,properties,required)|json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}})).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_reconstruct_the_exact_document_and_bind_the_corpus() {
        for doc in DOCUMENTS {
            let mut args = json!({"id":doc.id,"max_bytes":37});
            let mut content = String::new();
            loop {
                let page = call("read_doc", args.clone()).unwrap();
                let chunk = page["content"].as_str().unwrap();
                assert!(chunk.len() <= 37);
                content.push_str(chunk);
                if page["next_cursor"].is_null() {
                    break;
                }
                assert!(!chunk.is_empty());
                args["cursor"] = page["next_cursor"].clone();
            }
            assert_eq!(content, doc.content);
        }
    }

    #[test]
    fn cursors_cannot_cross_tools_queries_or_versions() {
        let list = call("list_docs", json!({"limit":1})).unwrap();
        let token = list["next_cursor"].clone();
        assert_eq!(
            call("get_examples", json!({"cursor":token}))
                .unwrap_err()
                .code,
            "stale_cursor"
        );
        let search = call("search_docs", json!({"query":"the","limit":1})).unwrap();
        assert!(!search["next_cursor"].is_null());
        assert!(
            call(
                "search_docs",
                json!({"query":"other","cursor":search["next_cursor"]})
            )
            .is_err()
        );
        assert!(call("list_docs", json!({"cursor":"old:corpus:1"})).is_err());
    }

    #[test]
    fn list_pagination_has_no_duplicates_and_examples_are_readable() {
        let mut args = json!({"limit":2});
        let mut ids = Vec::new();
        loop {
            let page = call("list_docs", args.clone()).unwrap();
            for doc in page["documents"].as_array().unwrap() {
                ids.push(doc["id"].as_str().unwrap().to_owned());
            }
            if page["next_cursor"].is_null() {
                break;
            }
            args["cursor"] = page["next_cursor"].clone();
        }
        assert_eq!(ids, DOCUMENTS.iter().map(|doc| doc.id).collect::<Vec<_>>());
        for example in call("get_examples", json!({})).unwrap()["documents"]
            .as_array()
            .unwrap()
        {
            assert_eq!(example["example"], true);
            assert!(call("read_doc", json!({"id":example["id"]})).is_ok());
        }
    }

    #[test]
    fn bounds_and_paths_fail_closed() {
        for (name, args) in [
            ("read_doc", json!({"id":"../../.secrets/key"})),
            ("read_doc", json!({"id":"caller","max_bytes":16385})),
            ("list_docs", json!({"limit":0})),
            ("list_docs", json!({"limit":51})),
            ("search_docs", json!({"query":""})),
            ("search_docs", json!({"query":"x".repeat(257)})),
            ("list_docs", json!({"api_key":"forbidden"})),
            ("list_docs", json!({"id":null})),
            ("list_docs", json!({"cursor":null})),
            ("list_docs", json!({"limit":null})),
            ("get_examples", json!({"query":null})),
            ("read_doc", json!({"id":"caller","limit":null})),
            (
                "read_doc",
                json!({"id":"caller","cursor":cursor("read_doc:caller",usize::MAX)}),
            ),
        ] {
            assert!(call(name, args).is_err(), "{name}");
        }
    }
}

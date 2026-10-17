use std::collections::BTreeSet;

use axum::body::to_bytes;
use axum::http::{HeaderMap, StatusCode};

use super::contract::{self, AREAS};
use super::rust;

async fn html(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[test]
fn every_contract_operation_has_a_page_and_a_description() {
    assert_eq!(
        contract::unplaced(),
        Vec::<String>::new(),
        "add each tag to an area in AREAS"
    );
    let operations = contract::operations();
    assert!(operations.len() > 150, "{} operations", operations.len());
    let missing: Vec<String> = operations
        .iter()
        .filter(|op| {
            op.description().is_empty()
                && op.value["summary"]
                    .as_str()
                    .is_none_or(|s| s.trim().is_empty())
        })
        .map(|op| {
            format!(
                "{} {} ({})",
                op.method,
                op.path,
                contract::sources()[op.source].file
            )
        })
        .collect();
    assert!(
        missing.is_empty(),
        "operations with no description:\n{}",
        missing.join("\n")
    );
    let mut anchors = BTreeSet::new();
    for op in operations {
        assert!(
            anchors.insert(op.anchor()),
            "duplicate anchor {}",
            op.anchor()
        );
    }
    for area in AREAS {
        assert!(
            contract::in_area(area.slug).next().is_some(),
            "{} is empty",
            area.slug
        );
    }
    // Every source contributes, and the public decision routes are there.
    for source in 0..contract::sources().len() {
        assert!(
            operations.iter().any(|op| op.source == source),
            "source {source}"
        );
    }
    for (method, path) in [
        ("POST", "/v1/systemone"),
        ("GET", "/v1/models"),
        ("POST", "/v1/threads"),
        ("POST", "/v1/computers"),
        ("GET", "/v1/labs/documents"),
    ] {
        assert!(
            operations
                .iter()
                .any(|op| op.method == method && op.path == path),
            "{method} {path}"
        );
    }
    // Gateway routes the public API does not forward stay out.
    assert!(!operations.iter().any(|op| op.path == "/v1/jobs"));
}

#[test]
fn every_public_sdk_item_is_described_and_its_calls_resolve() {
    let reference = rust::reference();
    assert!(reference.modules.len() > 10, "the reference parsed");
    let mut resolved = 0;
    let mut unresolved = BTreeSet::new();
    for module in reference.modules.iter().filter(|m| !m.generated) {
        assert!(
            !module.doc.is_empty(),
            "module {} has no doc",
            module.rust_path()
        );
        for item in &module.items {
            assert!(
                !item.doc.is_empty(),
                "{}::{} has no doc",
                module.rust_path(),
                item.name
            );
            for method in &item.methods {
                assert!(
                    !method.doc.is_empty(),
                    "{}::{} has no doc",
                    item.name,
                    method.name
                );
            }
            let calls = item
                .calls
                .iter()
                .chain(item.methods.iter().flat_map(|m| &m.calls));
            for call in calls {
                if contract::operation_for(&call.method, &call.path).is_some() {
                    resolved += 1;
                } else {
                    unresolved.insert(format!("{} {}", call.method, call.path));
                }
            }
        }
    }
    assert!(resolved > 60, "{resolved} SDK calls link to an operation");
    // A call to a route no contract describes shows as plain text. These are
    // the known ones; a new undocumented route fails here until it gets a
    // contract or joins this list on purpose.
    const KNOWN: &[&str] = &[
        "DELETE /v1/*",
        "DELETE /v1/account",
        "DELETE /v1/cloud-host",
        "DELETE /v1/device/keys/*",
        "GET /v1/cloud-host",
        "GET /v1/device/keys",
        "GET /v1/schedules",
        "GET /v1/web-sign-in/*",
        "POST /v1/cloud-host",
        "POST /v1/cloud-host/stop",
        "POST /v1/device/keys",
        "POST /v1/finish",
        "POST /v1/inputs",
        "POST /v1/inputs/file",
        "POST /v1/poll",
        "POST /v1/schedules/sync",
    ];
    let unknown: Vec<&String> = unresolved
        .iter()
        .filter(|call| !KNOWN.contains(&call.as_str()))
        .collect();
    assert!(
        unknown.is_empty(),
        "SDK calls with no operation: {unknown:#?}"
    );
}

#[test]
fn sdk_calls_pick_the_literal_route_over_a_parameter() {
    let op = contract::operation_for("GET", "/v1/threads/synced").unwrap();
    assert_eq!(op.path, "/v1/threads/synced");
    let op = contract::operation_for("POST", "/v1/threads/*/messages").unwrap();
    assert_eq!(op.id, "replyToThread");
    assert!(contract::operation_for("GET", "/v1/nothing-here").is_none());
}

#[tokio::test]
async fn every_page_renders_with_its_anchors() {
    let headers = HeaderMap::new();
    let (status, landing) = html(super::landing_page(&headers)).await;
    assert_eq!(status, StatusCode::OK);
    for text in [
        "openagents-sdk",
        "/auth.md",
        "client.authenticate",
        "/docs/sdk/api/chats",
        "/docs/sdk/rust/chats",
    ] {
        assert!(landing.contains(text), "landing has {text}");
    }
    crate::copy_guard::assert_plain("/docs/sdk", &landing);
    for area in AREAS {
        let (status, page) = html(super::area_page(&headers, area.slug)).await;
        assert_eq!(status, StatusCode::OK, "{}", area.slug);
        for op in contract::in_area(area.slug) {
            assert!(
                page.contains(&format!("id=\"{}\"", op.anchor())),
                "{} anchor",
                op.id
            );
        }
        assert!(page.contains("curl"), "{} has examples", area.slug);
    }
    let (_, chats) = html(super::area_page(&headers, "chats")).await;
    assert!(chats.contains("In the Rust SDK"), "chats link the SDK");
    let (status, _) = html(super::area_page(&headers, "nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, index) = html(super::rust_index_page(&headers)).await;
    assert_eq!(status, StatusCode::OK);
    crate::copy_guard::assert_plain("/docs/sdk/rust", &index);
    for module in rust::reference().modules.iter().filter(|m| !m.generated) {
        let (status, page) = html(super::rust_module_page(&headers, &module.slug())).await;
        assert_eq!(status, StatusCode::OK, "{}", module.slug());
        for item in &module.items {
            assert!(
                page.contains(&format!("id=\"{}\"", item.name)),
                "{}",
                item.name
            );
        }
    }
    let (_, chats) = html(super::rust_module_page(&headers, "chats")).await;
    assert!(
        chats.contains("/docs/sdk/api/chats#replyToThread"),
        "SDK links its operation"
    );
}

#[test]
fn search_finds_operations_and_sdk_items() {
    let hits = super::find("reply thread");
    assert!(
        hits.iter().any(|h| h.href.ends_with("#replyToThread")),
        "operation hit"
    );
    let hits = super::find("authenticate");
    assert!(
        hits.iter().any(|h| h.href.contains("/docs/sdk/rust/")),
        "SDK hit"
    );
    assert!(super::find("   ").is_empty());
    assert!(super::find("zzzz-no-such-thing").is_empty());
}

#[test]
fn examples_carry_the_body_the_schema_requires() {
    let op = contract::operations()
        .iter()
        .find(|op| op.id == "replyToThread")
        .unwrap();
    let document = op.document();
    let schema = &contract::deref(document, &op.value["requestBody"])["content"]["application/json"]
        ["schema"];
    let example = super::schema::example(document, schema);
    assert!(example.get("text").is_some(), "{example}");
    let fields = super::schema::fields(document, schema);
    assert!(fields.iter().any(|f| f.name == "text" && f.required));
}

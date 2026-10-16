use super::*;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use std::collections::VecDeque;
use std::sync::Mutex as StdMutex;

#[derive(Default)]
struct Script {
    replies: VecDeque<(StatusCode, Value)>,
    requests: Vec<(String, reqwest::Url)>,
    fallback: Option<(StatusCode, Value)>,
}

struct Fixture {
    rest: Rest,
    script: Arc<StdMutex<Script>>,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    async fn new(replies: Vec<(StatusCode, Value)>) -> Self {
        async fn respond(
            State(script): State<Arc<StdMutex<Script>>>,
            req: Request,
        ) -> (StatusCode, Json<Value>) {
            let mut script = script.lock().unwrap();
            script.requests.push((
                req.method().to_string(),
                reqwest::Url::parse(&format!("http://fixture{}", req.uri())).unwrap(),
            ));
            let (status, body) = script
                .replies
                .pop_front()
                .or_else(|| script.fallback.clone())
                .unwrap_or((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error":{"message":"unexpected fixture request"}}),
                ));
            (status, Json(body))
        }
        let script = Arc::new(StdMutex::new(Script {
            replies: replies.into(),
            ..Default::default()
        }));
        let app = Router::new().fallback(respond).with_state(script.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut rest = Rest::new("test", TokenSource::Metadata);
        rest.api = format!("http://{}", listener.local_addr().unwrap());
        *rest.token.lock().await = Some((
            "fixture-token".into(),
            Instant::now() + Duration::from_secs(3600),
        ));
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            rest,
            script,
            server,
        }
    }

    async fn pages(pages: Vec<Value>) -> Self {
        Self::new(pages.into_iter().map(|v| (StatusCode::OK, v)).collect()).await
    }

    fn pages_seen(&self) -> Vec<Option<String>> {
        let script = self.script.lock().unwrap();
        assert!(script.replies.is_empty());
        script
            .requests
            .iter()
            .map(|(method, url)| {
                assert_eq!(method, "GET");
                assert_eq!(url.path(), "/projects/test/global/images");
                let query: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
                assert_eq!(query.get("filter").unwrap(), "labels.managed=template");
                assert_eq!(query.get("maxResults").unwrap(), "500");
                query.get("pageToken").cloned()
            })
            .collect()
    }

    fn requests_seen(&self) -> Vec<(String, String)> {
        let script = self.script.lock().unwrap();
        assert!(script.replies.is_empty());
        script
            .requests
            .iter()
            .map(|(method, url)| (method.clone(), url.path().to_owned()))
            .collect()
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.rest
            .delete_image_wait(
                "template",
                id,
                Duration::from_secs(1),
                Duration::from_millis(1),
            )
            .await
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn image(name: &str, id: &str) -> Value {
    json!({"name":name,"id":id,"status":"READY","labels":{"managed":"template"}})
}

#[tokio::test]
async fn images_follow_every_page_including_empty_pages() {
    let f = Fixture::pages(vec![
        json!({"items":[image("first","1")],"nextPageToken":"next + / token"}),
        json!({"warning":{"code":"NO_RESULTS_ON_PAGE"},"nextPageToken":"third"}),
        json!({"items":[image("last","18446744073709551615")]}),
    ])
    .await;
    let images = f.rest.list_images("managed", "template").await.unwrap();
    assert_eq!(
        images.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["1", "18446744073709551615"]
    );
    assert_eq!(
        f.pages_seen(),
        [None, Some("next + / token".into()), Some("third".into())]
    );

    let empty = Fixture::pages(vec![json!({})]).await;
    assert!(
        empty
            .rest
            .list_images("managed", "template")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(empty.pages_seen(), [None]);
}

#[tokio::test]
async fn images_refuse_repeated_tokens_and_duplicate_identities() {
    for pages in [
        vec![
            json!({"nextPageToken":"loop"}),
            json!({"nextPageToken":"loop"}),
        ],
        vec![
            json!({"items":[image("one","1")],"nextPageToken":"next"}),
            json!({"items":[image("one","1")]}),
        ],
        vec![json!({"items":[image("one","1"),image("other","1")]})],
        vec![json!({"items":[image("one","1"),image("one","2")]})],
    ] {
        let f = Fixture::pages(pages).await;
        assert_eq!(
            f.rest
                .list_images("managed", "template")
                .await
                .unwrap_err()
                .code,
            "image_inventory"
        );
        f.pages_seen();
    }
}

#[tokio::test]
async fn images_refuse_partial_results_at_page_and_item_bounds() {
    let pages = Fixture::pages(vec![
        json!({"items":[image("one","1")],"nextPageToken":"more"}),
    ])
    .await;
    assert_eq!(
        pages
            .rest
            .image_pages("managed", "template", 1, 10)
            .await
            .unwrap_err()
            .code,
        "image_inventory"
    );
    assert_eq!(pages.pages_seen(), [None]);

    let items = Fixture::pages(vec![json!({"items":[image("one","1"),image("two","2")]})]).await;
    assert_eq!(
        items
            .rest
            .image_pages("managed", "template", 10, 1)
            .await
            .unwrap_err()
            .code,
        "image_inventory"
    );
    assert_eq!(items.pages_seen(), [None]);

    let exact = Fixture::pages(vec![json!({"items":[image("one","1")]})]).await;
    assert_eq!(
        exact
            .rest
            .image_pages("managed", "template", 1, 1)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(exact.pages_seen(), [None]);
}

#[tokio::test]
async fn images_refuse_malformed_pages_and_provider_failures() {
    for page in [
        Value::Null,
        json!({"items":{}}),
        json!({"nextPageToken":4}),
        json!({"warning":{"code":"UNREACHABLE"}}),
        json!({"items":[image("missing-id","")]}),
        json!({"items":[image("bad-id","01")]}),
        json!({"items":[image("bad-id","18446744073709551616")]}),
        json!({"items":[image("bad-name/escape","1")]}),
        json!({"items":[{"name":"wrong-filter","id":"1","labels":{"managed":"other"}}]}),
    ] {
        let f = Fixture::pages(vec![page]).await;
        assert_eq!(
            f.rest
                .list_images("managed", "template")
                .await
                .unwrap_err()
                .code,
            "image_inventory"
        );
        f.pages_seen();
    }
    let f = Fixture::new(vec![
        (
            StatusCode::OK,
            json!({"items":[image("one","1")],"nextPageToken":"next"}),
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":{"errors":[{"reason":"unavailable"}]}}),
        ),
    ])
    .await;
    let error = f.rest.list_images("managed", "template").await.unwrap_err();
    assert_eq!(error.status, 503);
    assert_eq!(error.code, "unavailable");
    assert_eq!(f.pages_seen(), [None, Some("next".into())]);
}

fn ok(value: Value) -> (StatusCode, Value) {
    (StatusCode::OK, value)
}

fn absent() -> (StatusCode, Value) {
    (
        StatusCode::NOT_FOUND,
        json!({"error":{"errors":[{"reason":"notFound"}]}}),
    )
}

fn delete_requests() -> Vec<(String, String)> {
    [
        ("GET", "template"),
        ("GET", "1"),
        ("DELETE", "1"),
        ("GET", "template"),
    ]
    .map(|(method, name)| {
        (
            method.into(),
            format!("/projects/test/global/images/{name}"),
        )
    })
    .into()
}

#[tokio::test]
async fn image_deletion_waits_for_observed_disappearance() {
    let mut deleting = image("template", "1");
    deleting["status"] = json!("DELETING");
    let f = Fixture::new(vec![
        ok(image("template", "1")),
        ok(image("template", "1")),
        ok(json!({"status":"PENDING", "targetId":"1"})),
        ok(deleting),
        absent(),
    ])
    .await;
    f.delete("1").await.unwrap();
    let mut expected = delete_requests();
    expected.push(("GET".into(), "/projects/test/global/images/template".into()));
    assert_eq!(f.requests_seen(), expected);
}

#[tokio::test]
async fn image_deletion_never_targets_a_replacement_name() {
    let replacement = image("template", "2");
    let f = Fixture::new(vec![ok(replacement.clone())]).await;
    f.delete("1").await.unwrap();
    assert_eq!(
        f.requests_seen(),
        [("GET".into(), "/projects/test/global/images/template".into())]
    );

    let f = Fixture::new(vec![
        ok(image("template", "1")),
        ok(image("template", "1")),
        ok(json!({"status":"PENDING", "targetId":"1"})),
        ok(replacement),
    ])
    .await;
    f.delete("1").await.unwrap();
    assert_eq!(f.requests_seen(), delete_requests());
}

#[tokio::test]
async fn unsupported_numeric_routes_do_not_prove_image_absence() {
    let f = Fixture::new(vec![
        ok(image("template", "1")),
        absent(),
        ok(image("template", "1")),
    ])
    .await;
    assert_eq!(
        f.delete("1").await.unwrap_err().code,
        "image_delete_unknown"
    );
    assert!(f.requests_seen().iter().all(|(method, _)| method == "GET"));

    let f = Fixture::new(vec![
        ok(image("template", "1")),
        ok(image("template", "1")),
        absent(),
        ok(image("template", "1")),
    ])
    .await;
    assert_eq!(
        f.delete("1").await.unwrap_err().code,
        "image_delete_unknown"
    );
    assert_eq!(f.requests_seen(), delete_requests());
}

#[tokio::test]
async fn image_deletion_retains_unknown_on_timeout_or_provider_failure() {
    let f = Fixture::new(vec![
        ok(image("template", "1")),
        ok(image("template", "1")),
        ok(json!({"status":"PENDING", "targetId":"1"})),
    ])
    .await;
    f.script.lock().unwrap().fallback = Some(ok(image("template", "1")));
    let error = f
        .rest
        .delete_image_wait(
            "template",
            "1",
            Duration::from_secs(1),
            Duration::from_secs(60),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "image_delete_unknown");
    let requests = f.requests_seen();
    assert_eq!(
        requests
            .iter()
            .filter(|(method, _)| method == "DELETE")
            .count(),
        1
    );
    assert!(
        requests
            .iter()
            .filter(|(method, _)| method == "DELETE")
            .all(|(_, path)| path.ends_with("/1"))
    );

    for failure in [
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":{"errors":[{"reason":"unavailable"}]}}),
        ),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error":{"errors":[{"reason":"notFound"}]}}),
        ),
    ] {
        let f = Fixture::new(vec![
            ok(image("template", "1")),
            ok(image("template", "1")),
            ok(json!({"status":"PENDING", "targetId":"1"})),
            failure.clone(),
        ])
        .await;
        assert_eq!(f.delete("1").await.unwrap_err().status, failure.0.as_u16());
        assert_eq!(f.requests_seen(), delete_requests());
    }
}

#[tokio::test]
async fn image_deletion_refuses_unknown_or_mismatched_identity() {
    for id in ["", "01", "name", "18446744073709551616"] {
        let f = Fixture::new(vec![]).await;
        assert_eq!(f.delete(id).await.unwrap_err().code, "image_identity");
        assert!(f.requests_seen().is_empty());
    }
    for response in [Value::Null, image("other", "1"), image("template", "")] {
        let f = Fixture::new(vec![ok(response)]).await;
        assert_eq!(f.delete("1").await.unwrap_err().code, "image_identity");
        assert!(f.requests_seen().iter().all(|(method, _)| method == "GET"));
    }
    let f = Fixture::new(vec![ok(image("template", "1")), ok(image("template", "2"))]).await;
    assert_eq!(
        f.delete("1").await.unwrap_err().code,
        "image_delete_unknown"
    );
    assert!(f.requests_seen().iter().all(|(method, _)| method == "GET"));
}

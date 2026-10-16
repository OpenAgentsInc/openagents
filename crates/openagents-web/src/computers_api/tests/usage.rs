use super::*;

pub(super) fn provider(state: &Native, owner: &str, request: &Request) -> Response {
    assert_eq!(request.method(), reqwest::Method::GET);
    assert!(request.headers().get("x-openagents-payer").is_none());
    let query = request.uri().query().unwrap_or_default().to_owned();
    state.usage_queries.lock().unwrap().push(query.clone());
    if query == "after=10&limit=1" {
        return (StatusCode::BAD_GATEWAY, Json(json!({"error":CANARY}))).into_response();
    }
    let sequence = match query.as_str() {
        "after=1&limit=1" => 2,
        "after=2&limit=1" => 3,
        "after=11&limit=1" => 12,
        _ => 1,
    };
    let mut events = vec![json!({
        "sequence":sequence,"computer":ID,"owner":owner,"revision":99,
        "intent_generation":3,"kind":"meter","event_key":CANARY,"deployment":CANARY,
        "driver":CANARY,"resource":CANARY,"operation":CANARY,"recorded_at":CANARY,
        "size":"small","shape":{"vcpus":2,"memory_mib":8192,"min_disk_gib":10,"provider":CANARY},
        "boot":1,"phase":"stopped","desired":"stopped","observed_ms":1000,
        "evidence":{"provider_seconds":null,"running":false,"provider_operation":CANARY,
            "evidence_sha256":CANARY,"storage":{"resource_deleted":false,"bytes":null,"checkpoint_records":1}}
    })];
    if owner == crate::chat_store::account_owner("bob") {
        events.clear();
    } else if query == "after=2&limit=1" {
        events[0]["evidence"]["provider_seconds"] = json!(0);
    } else if query == "after=11&limit=1" {
        events[0]["owner"] = json!(crate::chat_store::account_owner("bob"));
    }
    let after = events.last().map_or(0, |e| e["sequence"].as_i64().unwrap());
    Json(json!({"schema":"openagents.computers.usage.v1","events":events,"after":after,
        "allocation_allowance":{"held_cpu_ms":null,"settled_cpu_ms":0,"unknown_allowances":1,"unresolved_allowances":1,"internal":CANARY},
        "customer_charge":null,"pricing":"not_enabled","provider":CANARY})).into_response()
}

pub(super) async fn check(http: &reqwest::Client, base: &str, state: &Native) {
    let url = format!("{base}/v1/computer-usage");
    let response = http
        .get(&url)
        .bearer_auth(token("alice"))
        .header(
            "x-openagents-account",
            crate::chat_store::account_owner("bob"),
        )
        .header("x-openagents-payer", "bob")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "no-store, private"
    );
    let text = response.text().await.unwrap();
    assert!(!text.contains(CANARY) && !text.contains("synthetic-service-token"));
    let first: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(first["after"], 1);
    assert_eq!(first["events"][0]["measurement"], "unknown");
    assert!(first["events"][0]["provider_seconds"].is_null());
    assert_eq!(first["events"][0]["storage"]["resource_deleted"], false);
    assert!(first["events"][0].get("owner").is_none());
    assert!(first["allocation_allowance"]["held_cpu_ms"].is_null());
    assert_eq!(first["pricing"], "not_enabled");
    assert!(first["customer_charge"].is_null());
    let document = crate::app_openapi::document();
    for schema in ["AppComputerUsage", "AppComputerUsageEvent"] {
        let mut check = document["components"]["schemas"][schema].clone();
        check["components"] = document["components"].clone();
        let check = jsonschema::validator_for(&check).unwrap();
        let value = if schema == "AppComputerUsage" {
            &first
        } else {
            &first["events"][0]
        };
        assert!(check.is_valid(value));
    }
    let second = http
        .get(format!("{url}?after=1&limit=1"))
        .bearer_auth(token("alice"))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 200);
    assert_eq!(second.json::<Value>().await.unwrap()["after"], 2);
    let bob = http
        .get(&url)
        .bearer_auth(token("bob"))
        .header(
            "x-openagents-account",
            crate::chat_store::account_owner("alice"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(bob.status(), 200);
    assert_eq!(bob.json::<Value>().await.unwrap()["events"], json!([]));
    let zero = http
        .get(format!("{url}?after=2&limit=1"))
        .bearer_auth("oak_alice.account-scoped-synthetic-key")
        .send()
        .await
        .unwrap();
    assert_eq!(zero.status(), 200);
    let zero = zero.json::<Value>().await.unwrap();
    assert_eq!(zero["events"][0]["provider_seconds"], 0);
    assert_eq!(zero["events"][0]["measurement"], "reported");
    assert!(zero["customer_charge"].is_null());
    assert_eq!(
        state.usage_queries.lock().unwrap().as_slice(),
        [
            "after=0&limit=100",
            "after=1&limit=1",
            "after=0&limit=100",
            "after=2&limit=1"
        ]
    );
    for query in ["after=10&limit=1", "after=11&limit=1"] {
        let response = http
            .get(format!("{url}?{query}"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 503);
        assert!(!response.text().await.unwrap().contains(CANARY));
    }
    let before = state.calls.lock().unwrap().len();
    for query in [
        "limit=0",
        "limit=201",
        "after=-1",
        "after=18446744073709551615",
        "after=garbage",
        "limit=1&limit=2",
        "owner=bob",
        "payer=bob",
        "token=secret",
    ] {
        let response = http
            .get(format!("{url}?{query}"))
            .bearer_auth(token("alice"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "{query}");
        let text = response.text().await.unwrap();
        assert!(!text.contains("secret") && !text.contains(CANARY));
    }
    for (request, status) in [
        (http.get(&url), 401),
        (
            http.get(&url)
                .header("Cookie", format!("oa_cloud_session={}", token("alice"))),
            401,
        ),
        (http.get(&url).bearer_auth(token("charlie")), 403),
        (
            http.get(&url)
                .bearer_auth("oak_unbound.decision-only-synthetic-key"),
            403,
        ),
        (http.get(&url).bearer_auth("scj_synthetic-job-token"), 403),
        (http.post(&url).bearer_auth(token("alice")), 405),
        (http.delete(&url).bearer_auth(token("alice")), 405),
    ] {
        assert_eq!(request.send().await.unwrap().status(), status);
    }
    assert_eq!(state.calls.lock().unwrap().len(), before);
    assert!(
        state
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, _, path)| path == "/v1/usage")
            .all(|(_, method, _)| method == "GET")
    );
}

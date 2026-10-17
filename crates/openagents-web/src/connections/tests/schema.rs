//! Synthetic handler examples and permission/failure checks for #11868.

use super::*;

pub(super) fn assert_response(
    method: &str,
    path: &str,
    status: StatusCode,
    input: &Value,
    answer: &Value,
) {
    let path = path.split('?').next().unwrap();
    let template = if path.starts_with("/v1/projects/") && path.contains("/sources") {
        if path.ends_with("/sources") {
            "/v1/projects/{id}/sources"
        } else {
            "/v1/projects/{id}/sources/{source}"
        }
    } else if path.starts_with("/v1/connections/google/tools/drive") {
        "/v1/connections/google/tools/{tool}"
    } else {
        path
    };
    let document = crate::app_openapi::document();
    let operation = &document["paths"][template][method.to_lowercase()];
    if operation.is_null()
        || !operation["tags"]
            .to_string()
            .contains("Connected project sources")
    {
        return;
    }
    if status.is_success() && !input.is_null() {
        let mut schema = operation["requestBody"]["content"]["application/json"]["schema"].clone();
        if schema.is_object() {
            schema["components"] = document["components"].clone();
            let validator = jsonschema::validator_for(&schema).unwrap();
            assert!(
                validator.is_valid(input),
                "successful {method} {path} input does not match its schema: {input}"
            );
        }
    }
    let mut schema = operation["responses"][status.as_u16().to_string()]["content"]
        ["application/json"]["schema"]
        .clone();
    assert!(
        schema.is_object(),
        "undocumented {method} {path}: {status} {answer}"
    );
    schema["components"] = document["components"].clone();
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(
        validator.is_valid(answer),
        "{method} {path}: {status} {answer}: {:?}",
        validator
            .iter_errors(answer)
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );
}

#[test]
fn source_schema_examples_match_the_implemented_serializers() {
    let document = crate::app_openapi::document();
    for (name, component) in document["components"]["schemas"].as_object().unwrap() {
        if !name.starts_with("AppConnection") {
            continue;
        }
        let mut schema = component.clone();
        schema["components"] = document["components"].clone();
        let validator = jsonschema::validator_for(&schema).unwrap();
        if let Some(examples) = component["examples"].as_array() {
            for example in examples {
                assert!(validator.is_valid(example), "{name}: {example}");
            }
        }
    }
}

async fn setup(
    google: &FakeGoogle,
    store: Arc<Store>,
) -> (World, Browser, String, Browser, String) {
    let world = world_with(Options {
        connections: Some((
            store.clone(),
            Arc::new(Google::new(
                Client::new("cid.apps".into(), "client-secret".into()),
                google.endpoints(),
            )),
        )),
        ..Options::default()
    })
    .await;
    let mut owner = Browser::default();
    owner.sign_in(&world, "octo-local").await;
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    store
        .update_access(|a| {
            for login in ["octo-local", "quiet-local"] {
                a.add(AccessEntry {
                    account: Some(account_id_of(&world, login)),
                    ..AccessEntry::default()
                })
                .unwrap();
            }
        })
        .unwrap();
    let mine = owner.0["oa_cloud_session"].clone();
    let theirs = other.0["oa_cloud_session"].clone();
    (world, owner, mine, other, theirs)
}

#[tokio::test]
async fn source_contract_hides_disabled_connections_without_account_resources() {
    let world = world_with(Options::default()).await;
    for (method, path) in [
        ("GET", "/v1/connections"),
        ("DELETE", "/v1/connections/google"),
        ("POST", "/v1/connections/google/connect"),
        ("GET", "/v1/connections/google/drive"),
        ("POST", "/v1/connections/google/tools/drive_search"),
        ("GET", "/v1/projects/prj_0000000000000000/sources"),
        ("POST", "/v1/projects/prj_0000000000000000/sources"),
        (
            "DELETE",
            "/v1/projects/prj_0000000000000000/sources/source-schema-0001",
        ),
    ] {
        let (status, answer) = api(&world, "unsupported", method, path, json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {answer}");
        assert_eq!(answer["error"]["code"], "not_found");
    }
}

#[tokio::test]
async fn source_contract_qualifies_roles_revocation_limits_and_mutation_effects() {
    let google = FakeGoogle::start(files(), &[DRIVE_READONLY, SHEETS_READONLY]).await;
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let (world, mut owner, mine, _, theirs) = setup(&google, store.clone()).await;
    let (_, project) = api(
        &world,
        &mine,
        "POST",
        "/v1/projects",
        json!({"name":"Source schema fixture"}),
    )
    .await;
    let id = project["id"].as_str().unwrap();
    let path = format!("/v1/projects/{id}/sources");
    let owner_id = account_owner_of(&world, "octo-local");
    assert_eq!(
        owner.get(&world, "/v1/connections").await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        api(
            &world,
            "oak_fixture.unsupported",
            "GET",
            "/v1/connections",
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, not_connected) = api(&world, &mine, "POST", &path, json!({"id":FOLDER})).await;
    assert_eq!(not_connected["error"]["code"], "google_not_connected");
    assert_eq!(
        api(&world, &theirs, "GET", &path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, invitation) = api(
        &world,
        &mine,
        "POST",
        &format!("/v1/projects/{id}/invitations"),
        json!({"role":"editor"}),
    )
    .await;
    let (status, _) = api(
        &world,
        &theirs,
        "POST",
        "/v1/projects/invitations/accept",
        json!({"code":invitation["url"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for (method, source_path) in [
        ("GET", path.clone()),
        ("POST", path.clone()),
        ("DELETE", format!("{path}/{FOLDER}")),
    ] {
        let (status, error) =
            api(&world, &theirs, method, &source_path, json!({"id":FOLDER})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{error}");
        assert_eq!(error["error"]["code"], "project_role");
    }
    connect_google(&mut owner, &world).await;
    assert_eq!(
        api(
            &world,
            &mine,
            "DELETE",
            &format!(
                "/v1/projects/{id}/members/{}",
                account_id_of(&world, "quiet-local")
            ),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        api(&world, &theirs, "GET", &path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, added) = api(&world, &mine, "POST", &path, json!({"id":FOLDER})).await;
    assert_eq!(status, StatusCode::CREATED);
    google.inner.lock().unwrap().files[0].name = "Renamed in Drive".into();
    let (status, duplicate) = api(&world, &mine, "POST", &path, json!({"id":FOLDER})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(duplicate, added);
    assert_eq!(
        api(
            &world,
            &mine,
            "POST",
            &path,
            json!({"link":"invalid", "id":FOLDER})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    for (tool, args) in [
        ("drive_search", json!({"query":"Spending"})),
        ("drive.list_folder", json!({"folder":FOLDER})),
        ("drive_read", json!({"file":SHEET})),
    ] {
        let (status, answer) = api(
            &world,
            &mine,
            "POST",
            &format!("/v1/connections/google/tools/{tool}"),
            args,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{answer}");
        assert!(answer.get("cost_usd").is_none());
    }
    store
        .update(&owner_id, |a| a.connections[0].granted_scopes.clear())
        .unwrap();
    let (status, missing_scope) = api(
        &world,
        &mine,
        "POST",
        "/v1/connections/google/tools/drive_search",
        json!({"query":"Spending"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(missing_scope["error"]["code"], "tool_failed");
    store
        .update(&owner_id, |a| {
            a.connections[0].granted_scopes.push(DRIVE_READONLY.into());
            for n in 1..crate::cloud::connections::MAX_PROJECT_SOURCES {
                a.attach(Source {
                    id: format!("synthetic-source-{n:04}"),
                    project: id.into(),
                    ..sources()[0].clone()
                })
                .unwrap();
            }
        })
        .unwrap();
    let (status, full) = api(&world, &mine, "POST", &path, json!({"id":SHEET})).await;
    assert_eq!(status, StatusCode::CONFLICT, "{full}");
    assert_eq!(full["error"]["code"], "full");

    google
        .inner
        .lock()
        .unwrap()
        .revoked
        .push(fake::REFRESH.into());
    let (status, revoked) = api(
        &world,
        &mine,
        "GET",
        "/v1/connections/google/drive",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(revoked["error"]["code"], "google_reconnect");
    let (_, listed) = api(&world, &mine, "GET", &path, Value::Null).await;
    assert_eq!(listed["google"], "reconnect");
    assert_eq!(listed["data"].as_array().unwrap().len(), 20);
    let (_, disconnected) = api(
        &world,
        &mine,
        "DELETE",
        "/v1/connections/google",
        Value::Null,
    )
    .await;
    assert_eq!(disconnected["sources_kept"], 20);
    assert_eq!(
        api(
            &world,
            &mine,
            "DELETE",
            "/v1/connections/google",
            Value::Null
        )
        .await
        .1["deleted"],
        false
    );
    for source in listed["data"].as_array().unwrap() {
        assert_eq!(
            api(
                &world,
                &mine,
                "DELETE",
                &format!("{path}/{}", source["id"].as_str().unwrap()),
                Value::Null
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        api(
            &world,
            &mine,
            "DELETE",
            &format!("{path}/{FOLDER}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        api(
            &world,
            &mine,
            "DELETE",
            &format!("/v1/projects/{id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(store.load(&owner_id).unwrap().sources.is_empty());

    store
        .update_access(|a| {
            a.remove(&account_id_of(&world, "octo-local"));
        })
        .unwrap();
    assert_eq!(
        api(&world, &mine, "GET", "/v1/connections", Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    store
        .update_access(|a| {
            a.add(AccessEntry {
                account: Some(account_id_of(&world, "octo-local")),
                ..AccessEntry::default()
            })
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        api(&world, &mine, "GET", "/v1/connections", Value::Null)
            .await
            .0,
        StatusCode::OK
    );
    tenancy::sessions::Sessions::open(&world.root().join("accounts"))
        .unwrap()
        .mutate(|book, _, now| {
            let session = book.session_of_token(&mine).unwrap().id.clone();
            book.logout(&session, now)
        })
        .unwrap();
    assert_eq!(
        api(&world, &mine, "GET", "/v1/connections", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn drive_contract_qualifies_pagination_and_downstream_failures() {
    use axum::extract::Request;
    use axum::response::IntoResponse;
    let google = FakeGoogle::start(vec![], &[DRIVE_READONLY]).await;
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let root = format!("http://{}", listener.local_addr().unwrap());
    let seen = requests.clone();
    let router = Router::new().fallback(move |request: Request| {
        let seen = seen.clone();
        async move {
            seen.lock().unwrap().push(request.uri().to_string());
            let path = request.uri().path();
            for (suffix, status) in [("denied-file-0001",StatusCode::FORBIDDEN),("missing-file-0001",StatusCode::NOT_FOUND),("offline-file-0001",StatusCode::SERVICE_UNAVAILABLE),("revoked-file-0001",StatusCode::UNAUTHORIZED)] {
                if path.ends_with(suffix) { return status.into_response(); }
            }
            let next = request.uri().query().unwrap_or_default().contains("pageToken=second");
            let mut page = json!({"files":[{"id":if next {"second-file-0001"} else {"first-file-0001"},"name":"Example","mimeType":"text/plain"}]});
            if !next { page["nextPageToken"] = json!("second"); }
            axum::Json(page).into_response()
        }
    });
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let mut endpoints = google.endpoints();
    endpoints.drive = root;
    let world = world_with(Options {
        connections: Some((
            store.clone(),
            Arc::new(Google::new(
                Client::new("cid.apps".into(), "client-secret".into()),
                endpoints,
            )),
        )),
        ..Options::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    store
        .update_access(|a| {
            a.add(AccessEntry {
                account: Some(account_id_of(&world, "octo-local")),
                ..AccessEntry::default()
            })
        })
        .unwrap()
        .unwrap();
    connect_google(&mut browser, &world).await;
    let token = &browser.0["oa_cloud_session"];
    let (_, first) = api(
        &world,
        token,
        "GET",
        "/v1/connections/google/drive",
        Value::Null,
    )
    .await;
    assert_eq!(first["next"], "second");
    let (_, second) = api(
        &world,
        token,
        "GET",
        "/v1/connections/google/drive?page=second",
        Value::Null,
    )
    .await;
    assert_eq!(second["next"], Value::Null);
    assert_ne!(first["data"][0]["id"], second["data"][0]["id"]);
    let (_, search) = api(
        &world,
        token,
        "GET",
        "/v1/connections/google/drive?q=Example&page=second",
        Value::Null,
    )
    .await;
    assert_eq!(search["next"], Value::Null);
    api(
        &world,
        token,
        "GET",
        &format!("/v1/connections/google/drive?page={}", "x".repeat(513)),
        Value::Null,
    )
    .await;
    let seen = requests.lock().unwrap().clone();
    assert!(seen[1].contains("pageToken=second"));
    assert!(!seen[2].contains("pageToken="));
    assert!(!seen[3].contains("pageToken="));
    for (file, expected, code) in [
        ("denied-file-0001", StatusCode::FORBIDDEN, "drive_forbidden"),
        ("missing-file-0001", StatusCode::NOT_FOUND, "not_found"),
        (
            "offline-file-0001",
            StatusCode::BAD_GATEWAY,
            "google_unavailable",
        ),
        (
            "revoked-file-0001",
            StatusCode::CONFLICT,
            "google_reconnect",
        ),
    ] {
        let (_, project) = api(&world, token, "POST", "/v1/projects", json!({"name":file})).await;
        let id = project["id"].as_str().unwrap();
        let (status, answer) = api(
            &world,
            token,
            "POST",
            &format!("/v1/projects/{id}/sources"),
            json!({"id":file}),
        )
        .await;
        assert_eq!(status, expected, "{answer}");
        assert_eq!(answer["error"]["code"], code);
        assert_eq!(
            api(
                &world,
                token,
                "DELETE",
                &format!("/v1/projects/{id}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    assert!(
        store
            .load(&account_owner_of(&world, "octo-local"))
            .unwrap()
            .sources
            .is_empty()
    );
    api(
        &world,
        token,
        "DELETE",
        "/v1/connections/google",
        Value::Null,
    )
    .await;
    server.abort();
}

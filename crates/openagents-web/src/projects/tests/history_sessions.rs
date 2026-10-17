//! Computer sessions as rows in the account's chat list (#11897): summary
//! first, the transcript only after its opt-in, project members by the
//! project's choice, and nothing left after the computer is removed.
use super::*;

const ONE: &str = "a1000000-0000-4000-8000-000000000001";
const TWO: &str = "a1000000-0000-4000-8000-000000000002";
const THREE: &str = "a1000000-0000-4000-8000-000000000003";

fn form_csrf(html: &str) -> String {
    let at = html.find(r#"name="csrf" value=""#).unwrap() + 19;
    let rest = &html[at..];
    rest[..rest.find('"').unwrap()].to_owned()
}

fn row(id: &str, title: &str, at: u64, project: Option<&str>) -> Value {
    json!({"id": id, "title": title, "project": project, "folder": "openagents",
        "status": "working", "last_activity_unix": at, "home_transfer": true,
        "recap": "Fixing the sidebar badge", "workspace": "0123456789abcdef"})
}

#[tokio::test]
async fn continuation_round_trip_links_parts_without_publishing_the_seed() {
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let source = chat(CHAT, &owner, "Flower plan", None);
    let revision = source.revision;
    world.store.create(&source).await.unwrap();
    let source_ref = json!({"id":CHAT,"home":"cloud"});
    let computer_ref = json!({"id":ONE,"home":"computer:test-mac"});
    let cloud_ref = json!({"id":TWO,"home":"cloud"});
    let (status, moved) = api
        .json(
            &world,
            "POST",
            &format!("/v1/threads/{CHAT}/continue"),
            json!({"revision":revision,"target":computer_ref}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    let mut native = row(ONE, "Flower plan", 7, None);
    native["status"] = json!("idle");
    native["continues"] = source_ref.clone();
    let (status, body) = api
        .json(
            &world,
            "PUT",
            "/v1/history/sessions/computers/test-mac",
            json!({"name":"Test Mac","sessions":[native]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let token = format!("sk-ant-{}", "synthetic-test-only-0000000000");
    let seed = chat_home::Seed {
        title: "Flower plan".into(),
        summary: token.clone(),
        recent: vec![chat_home::Turn {
            role: chat_home::Role::User,
            text: "The fictional flower is coral orchid.".into(),
        }],
    };
    let transfer = json!({"revision":7,"target":cloud_ref,"seed":seed});
    let route = format!("/v1/threads/{ONE}/continue");
    let (status, returned) = api.json(&world, "POST", &route, transfer.clone()).await;
    assert_eq!(status, StatusCode::OK, "{returned}");
    assert_eq!(returned["source"], computer_ref);
    assert_eq!(returned["target"], cloud_ref);
    assert!(!returned.to_string().contains(&token));
    assert_eq!(api.json(&world, "POST", &route, transfer).await.1, returned);
    assert_eq!(
        api.json(
            &world,
            "POST",
            &route,
            json!({"revision":7,"target":{"id":THREE,"home":"cloud"},"seed":seed})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let target = world.store.load(&owner, TWO).await.unwrap().unwrap();
    assert!(
        target.conversation.messages.is_empty(),
        "context isn't a displayed assistant message"
    );
    assert_eq!(
        target.conversation.links.continues.as_ref().unwrap().id,
        ONE
    );
    let parts = crate::continuations::notice(&target.conversation)
        .unwrap()
        .into_string();
    assert!(parts.contains(&format!("/chat/sessions/{ONE}")));
    let stored = crate::continuation_receipts::load(&world.store, &owner, TWO)
        .await
        .unwrap()
        .unwrap();
    assert!(
        stored
            .seed
            .prompt("Which flower?")
            .unwrap()
            .contains("coral orchid")
    );
    let (_, index) = api
        .json(&world, "GET", "/v1/history/sessions", Value::Null)
        .await;
    assert_eq!(index["data"][0]["continues"], source_ref);
    assert_eq!(index["data"][0]["continued_by"], cloud_ref);
    assert_eq!(index["data"][0]["transcript"], false);
    assert!(!index.to_string().contains("coral orchid"));
}

#[tokio::test]
async fn continuation_session_parent_requires_the_sources_destination_and_survives_old_publishers()
{
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let source = chat_home::PartRef {
        id: CHAT.into(),
        home: chat_home::Home::Cloud,
    };
    let target = chat_home::PartRef {
        id: ONE.into(),
        home: chat_home::Home::Computer("test-mac".into()),
    };
    let mut source_chat = chat(CHAT, &owner, "Earlier work", None);
    world.store.create(&source_chat).await.unwrap();
    let mut published = row(ONE, "New part", 5, None);
    published["continues"] = serde_json::to_value(&source).unwrap();
    let body = json!({"name":"Test Mac", "sessions":[published]});
    let path = "/v1/history/sessions/computers/test-mac";
    assert_eq!(
        api.json(&world, "PUT", path, body.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    let loaded = world.store.load(&owner, CHAT).await.unwrap().unwrap();
    source_chat.links.continued_by = Some(target);
    source_chat.revision += 1;
    world
        .store
        .compare_and_swap(&loaded, &source_chat)
        .await
        .unwrap();
    assert_eq!(
        other.json(&world, "PUT", path, body.clone()).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, response) = api.json(&world, "PUT", path, body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let mut old_body = body.clone();
    old_body["sessions"][0]
        .as_object_mut()
        .unwrap()
        .remove("continues");
    assert_eq!(
        api.json(&world, "PUT", path, old_body).await.0,
        StatusCode::OK
    );
    let (_, detail) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history/sessions/{ONE}"),
            Value::Null,
        )
        .await;
    assert_eq!(
        detail["session"]["continues"],
        serde_json::to_value(source).unwrap()
    );
    assert_eq!(detail["session"]["transcript"], false);
    assert!(detail["messages"].is_null());
    let mut changed = body;
    changed["sessions"][0]["continues"]["id"] = json!(TWO);
    assert_eq!(
        api.json(&world, "PUT", path, changed).await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn a_saved_agents_session_is_listed_by_the_agent_with_the_openagents_mark() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let now = crate::chat_store::now_unix();
    let mut foreman = row(ONE, "Foreman", now, None);
    foreman["agent"] = json!("Foreman");
    let publish =
        json!({"name": "Test Mac", "sessions": [foreman, row(TWO, "Older work", now - 600, None)]});
    let (status, body) = api
        .json(
            &world,
            "PUT",
            "/v1/history/sessions/computers/test-mac",
            publish,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, list) = api
        .json(&world, "GET", "/v1/history/sessions", Value::Null)
        .await;
    let rows = list["data"].as_array().unwrap();
    assert_eq!(rows[0]["agent"], "Foreman");
    assert!(rows[1]["agent"].is_null());
    let page = browser.get(&world, &format!("/chat/sessions/{ONE}")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("oa-session-agent"), "{}", page.body);
    assert_eq!(
        page.body.matches("oa-agent-mark").count(),
        2,
        "page and its sidebar row"
    );
}

#[tokio::test]
async fn history_session_continuation_waits_for_the_computer_and_transcript_is_opt_in() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let other = signed_in(&world, "quiet-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let now = crate::chat_store::now_unix();
    world
        .store
        .create(&chat(CHAT, &owner, "A web chat", None))
        .await
        .unwrap();

    // A host publishes its rows; text is screened and no transcript is kept.
    let secret = format!("sk-ant-{}", "synthetic-test-only-0000000000");
    let publish = json!({"name": "Test Mac", "sessions": [
        row(ONE, &format!("Deploy with {secret}"), now, None),
        row(TWO, "Older work", now - 600, None),
    ]});
    let (status, body) = api
        .json(
            &world,
            "PUT",
            "/v1/history/sessions/computers/test-mac",
            publish.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["transcripts"], json!([]));
    let (status, bad) = api
        .json(
            &world,
            "PUT",
            "/v1/history/sessions/computers/bad%20device",
            publish.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");

    let (_, history) = api.json(&world, "GET", "/v1/history", Value::Null).await;
    assert_eq!(history["data"].as_array().unwrap().len(), 1);
    let sessions = history["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0]["id"], ONE);
    assert_eq!(sessions[0]["source"], "session");
    assert_eq!(sessions[0]["home"], "computer:test-mac");
    assert_eq!(sessions[0]["computer"], "Test Mac");
    assert_eq!(sessions[0]["online"], true);
    assert_eq!(sessions[0]["transcript"], false);
    assert_eq!(
        sessions[0]["app_link"],
        format!("openagents://open/chat/{ONE}?workspace=0123456789abcdef")
    );
    assert!(!history.to_string().contains(&secret));
    // A later page carries no sessions; the chat cursor is unchanged.
    let (_, later) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history?after={CHAT}"),
            Value::Null,
        )
        .await;
    assert_eq!(later["sessions"], json!([]));

    // Nobody else sees them.
    let (_, theirs) = other
        .json(&world, "GET", "/v1/history/sessions", Value::Null)
        .await;
    assert_eq!(theirs["data"], json!([]));
    let path = format!("/v1/history/sessions/{ONE}");
    assert_eq!(
        other.json(&world, "GET", &path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        other
            .json(
                &world,
                "PUT",
                &format!("{path}/transcript"),
                json!({"on": true})
            )
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // The transcript waits for its opt-in.
    let (_, one) = api.json(&world, "GET", &path, Value::Null).await;
    assert!(
        one["session"]["title"]
            .as_str()
            .unwrap()
            .contains("[redacted:")
    );
    assert!(one["messages"].is_null());
    let messages = json!({"messages": [
        {"role": "user", "text": "Fix the badge"},
        {"role": "tool", "text": "Read sidebar.rs"},
        {"role": "assistant", "text": format!("Done, and {secret}")},
    ]});
    let (status, off) = api
        .json(&world, "PUT", &format!("{path}/messages"), messages.clone())
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{off}");
    assert_eq!(off["error"]["code"], "transcript_off");
    let page = browser.get(&world, &format!("/chat/sessions/{ONE}")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("On Test Mac"), "{}", page.body);
    assert!(page.body.contains("Fixing the sidebar badge"));
    assert!(page.body.contains("session-private"));
    assert!(!page.body.contains("session-offline"));
    assert!(page.body.contains(&format!("chat-session-row-{TWO}")));
    assert!(page.body.contains("openagents://open/chat/"));

    let (status, on) = api
        .json(
            &world,
            "PUT",
            &format!("{path}/transcript"),
            json!({"on": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{on}");
    assert_eq!(on["transcript"], true);
    let (_, again) = api
        .json(
            &world,
            "PUT",
            "/v1/history/sessions/computers/test-mac",
            publish.clone(),
        )
        .await;
    assert_eq!(again["transcripts"], json!([ONE]));
    let (status, saved) = api
        .json(&world, "PUT", &format!("{path}/messages"), messages)
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, one) = api.json(&world, "GET", &path, Value::Null).await;
    assert_eq!(one["messages"][0]["text"], "Fix the badge");
    assert!(!one.to_string().contains(&secret));
    let page = browser.get(&world, &format!("/chat/sessions/{ONE}")).await;
    assert!(page.body.contains("session-transcript"), "{}", page.body);
    assert!(page.body.contains("Fix the badge"));

    // Offline is said plainly.
    crate::session_index::update(&world.store, &owner, |index| {
        index.published.insert("test-mac".into(), 1);
        index.rows.get_mut(ONE).unwrap().status = crate::session_index::Status::Idle;
        true
    })
    .await
    .unwrap();
    let page = browser.get(&world, &format!("/chat/sessions/{ONE}")).await;
    assert!(page.body.contains("Test Mac is offline"), "{}", page.body);

    // The web asks the computer to move; it never creates a second writable part itself.
    let csrf = form_csrf(&page.body);
    let started = browser
        .post(
            &world,
            &format!("/chat/sessions/{ONE}/continue"),
            &[("csrf", &csrf)],
        )
        .await;
    assert_eq!(started.status, StatusCode::SEE_OTHER, "{}", started.body);
    assert_eq!(
        started.location(),
        format!("/chat/sessions/{ONE}?continue=true")
    );
    let moves_path = "/v1/history/sessions/computers/test-mac/moves";
    let (_, moves) = api.json(&world, "GET", moves_path, Value::Null).await;
    let request = &moves["moves"][0];
    assert_eq!(request["source"], ONE);
    let chat_id = request["target"]["id"].as_str().unwrap().to_owned();
    assert!(world.store.load(&owner, &chat_id).await.unwrap().is_none());
    let (_, invisible) = other.json(&world, "GET", moves_path, Value::Null).await;
    assert_eq!(invisible["moves"], json!([]));
    let waiting = browser.get(&world, started.location()).await;
    assert!(waiting.body.contains("Waiting for Test Mac"));
    // Simulate the computer's acknowledgment after its local read-only write.
    let seed = chat_home::Seed::new("Earlier work", [(chat_home::Role::User, "Fix the badge")]);
    let (status, moved) = api
        .json(
            &world,
            "POST",
            &format!("/v1/threads/{ONE}/continue"),
            json!({"revision":now,"target":request["target"],"seed":seed}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    let continued = world.store.load(&owner, &chat_id).await.unwrap().unwrap();
    assert_eq!(
        continued.conversation.links.continues.as_ref().unwrap().id,
        ONE
    );
    assert!(continued.conversation.messages.is_empty());
    assert!(
        !crate::session_moves::pending(&world.store, &owner, ONE)
            .await
            .unwrap()
    );
    let chat_path = format!("/chat/{chat_id}");
    let followed = browser.get(&world, started.location()).await;
    assert_eq!(followed.location(), chat_path);
    let twice = browser
        .post(
            &world,
            &format!("/chat/sessions/{ONE}/continue"),
            &[("csrf", &csrf)],
        )
        .await;
    assert_eq!(twice.location(), chat_path);
    let (_, one) = api.json(&world, "GET", &path, Value::Null).await;
    assert_eq!(one["session"]["continued_by"]["id"], chat_id.as_str());
    let (_, history) = api.json(&world, "GET", "/v1/history", Value::Null).await;
    assert!(
        history["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["continues"]["id"] == ONE)
    );

    // Off deletes the copy.
    api.json(
        &world,
        "PUT",
        &format!("{path}/transcript"),
        json!({"on": false}),
    )
    .await;
    let (_, one) = api.json(&world, "GET", &path, Value::Null).await;
    assert!(one["messages"].is_null());
    assert!(
        crate::session_index::copy(&world.store, &owner, ONE)
            .await
            .unwrap()
            .is_none()
    );

    // A shared project's members see its rows, and its transcripts only by
    // the project's choice.
    let (_, made) = api
        .json(&world, "POST", "/v1/projects", json!({"name": "Sessions"}))
        .await;
    let project = made["id"].as_str().unwrap().to_owned();
    let (_, invited) = api
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{project}/invitations"),
            json!({"role": "viewer"}),
        )
        .await;
    let (status, joined) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": invited["code"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    let mut with_project = publish.clone();
    with_project["sessions"].as_array_mut().unwrap().push(row(
        THREE,
        "Project work",
        now - 60,
        Some(&project),
    ));
    api.json(
        &world,
        "PUT",
        "/v1/history/sessions/computers/test-mac",
        with_project,
    )
    .await;
    let scoped = format!("/v1/history/sessions?project={project}");
    let (_, theirs) = other.json(&world, "GET", &scoped, Value::Null).await;
    let rows = theirs["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{theirs}");
    assert_eq!(rows[0]["id"], THREE);
    assert_eq!(rows[0]["transcript"], false);
    let (_, first) = other
        .json(
            &world,
            "GET",
            &format!("/v1/history?project={project}"),
            Value::Null,
        )
        .await;
    assert_eq!(first["sessions"][0]["id"], THREE);
    api.json(
        &world,
        "PUT",
        &format!("/v1/history/sessions/projects/{project}/transcript"),
        json!({"on": true}),
    )
    .await;
    let (_, theirs) = other.json(&world, "GET", &scoped, Value::Null).await;
    assert_eq!(theirs["data"][0]["transcript"], true);

    // Removing the computer removes its rows.
    let (status, body) = api
        .json(&world, "DELETE", "/v1/computers/test-mac", Value::Null)
        .await;
    assert!(status.is_success(), "{status} {body}");
    let (_, mine) = api
        .json(&world, "GET", "/v1/history/sessions", Value::Null)
        .await;
    assert_eq!(mine["data"], json!([]));
    assert_eq!(
        Api { token: None }
            .json(&world, "GET", "/v1/history/sessions", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

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
        "status": "working", "last_activity_unix": at,
        "recap": "Fixing the sidebar badge", "workspace": "0123456789abcdef"})
}

#[tokio::test]
async fn history_sessions_are_summaries_until_their_transcript_is_turned_on() {
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
        true
    })
    .await
    .unwrap();
    let page = browser.get(&world, &format!("/chat/sessions/{ONE}")).await;
    assert!(page.body.contains("Test Mac is offline"), "{}", page.body);

    // Continue on the web: one linked chat, seeded with the summary.
    let csrf = form_csrf(&page.body);
    let started = browser
        .post(
            &world,
            &format!("/chat/sessions/{ONE}/continue"),
            &[("csrf", &csrf)],
        )
        .await;
    assert_eq!(started.status, StatusCode::SEE_OTHER, "{}", started.body);
    let chat_path = started.location().to_owned();
    let chat_id = chat_path.strip_prefix("/chat/").unwrap().to_owned();
    let continued = world.store.load(&owner, &chat_id).await.unwrap().unwrap();
    let links = &continued.conversation.links;
    assert_eq!(links.continues.as_ref().unwrap().id, ONE);
    assert!(
        continued.conversation.messages[0]
            .text
            .contains("Fix the badge")
    );
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

//! The composer's selector row through the whole site
//! (`crate::composer_row`): nothing for a signed-out visitor; Project
//! (with Connect a GitHub repository) once signed in; Branch from GitHub
//! with a project; Where it runs for an account, offering Coder on a
//! computer while it is online, on new and existing chats alike (#11319);
//! and a message sent with each pick records it or starts the run, while
//! a pick that can't work is refused, never answered some other way.

use super::*;

const REQUEST: &str = "32345678-1234-4234-8234-123456789abc";
const LATER: &str = "42345678-1234-4234-8234-123456789abc";

/// Adds `repository` as a project for a browser that connected GitHub;
/// its id.
async fn add_project(browser: &mut Browser, world: &World, repository: &str) -> String {
    let page = browser.get(world, "/projects/repositories?page=1").await;
    let csrf = hidden(
        &page.body,
        r#"<form method="post" action="/projects">"#,
        "csrf",
    );
    let added = browser
        .post(world, PAGE, &[("csrf", &csrf), ("repository", repository)])
        .await;
    assert_eq!(added.status, StatusCode::SEE_OTHER, "{}", added.body);
    let page = browser.get(world, PAGE).await;
    page.body[page.body.find("/?project=prj_").unwrap() + 10..][..20].to_string()
}

async fn online(world: &World, owner: &str, computer: &str) {
    let name = computer.to_owned();
    world
        .store
        .update_computers(owner, move |computers| {
            computers
                .seen
                .insert(name.clone(), crate::chat_store::now_unix());
            true
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn the_row_offers_only_what_works_and_sending_uses_each_pick() {
    let world = world().await;

    // Signed out: the plain composer.
    let mut visitor = Browser::default();
    let home = visitor.get(&world, "/").await;
    assert_eq!(home.status, StatusCode::OK);
    assert!(home.body.contains(r#"id="chat-form""#));
    assert!(!home.body.contains("composer-row"), "{}", home.body);
    assert!(!home.body.contains("Where it runs"));
    let panel = visitor.get(&world, "/composer/row/project").await;
    assert_eq!(panel.status, StatusCode::FORBIDDEN);

    // Signed in, GitHub not connected: Project, whose one item connects a
    // repository.
    let mut quiet = Browser::default();
    quiet.sign_in(&world, "quiet-local").await;
    let home = quiet.get(&world, "/").await;
    assert!(
        home.body.contains(r#"aria-label="Project: No project""#),
        "{}",
        home.body
    );
    assert!(!home.body.contains("Branch:"));
    // Where it runs shows for an account even with no computer online,
    // and its panel says how to bring one online (#11319).
    assert!(
        home.body.contains(r#"aria-label="Where it runs: Chat""#),
        "{}",
        home.body
    );
    let panel = quiet.get(&world, "/composer/row/target").await;
    assert!(
        panel.body.contains("None of your computers is online"),
        "{}",
        panel.body
    );
    crate::copy_guard::assert_plain("/composer/row/target", &panel.body);
    let panel = quiet.get(&world, "/composer/row/project").await;
    assert_eq!(panel.status, StatusCode::OK, "{}", panel.body);
    assert!(panel.body.contains("Connect a GitHub repository"));
    assert!(panel.body.contains(r#"href="/projects""#));
    crate::copy_guard::assert_plain("/composer/row/project", &panel.body);

    // Connected with a project: picking it brings its default branch.
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    connect_public(&mut octo, &world, "octo-local").await;
    let project = add_project(&mut octo, &world, "octo-local/hello-world").await;
    let panel = octo.get(&world, "/composer/row/project").await;
    assert!(panel.body.contains("hello-world"), "{}", panel.body);
    assert!(panel.body.contains(&format!("project={project}")));
    let row = octo
        .get(
            &world,
            &format!("/composer/row?project={project}&branch=&target=&focus=project"),
        )
        .await;
    assert_eq!(row.status, StatusCode::OK, "{}", row.body);
    assert!(row.body.contains(r#"aria-label="Project: hello-world""#));
    assert!(row.body.contains(r#"aria-label="Branch: main""#));
    assert!(
        row.body
            .contains(r#"id="composer-panel" hx-swap-oob="innerHTML""#)
    );
    assert!(!row.body.contains("Coder on"), "{}", row.body);
    let branches = octo
        .get(&world, &format!("/composer/row/branch?project={project}"))
        .await;
    assert!(
        branches.body.contains("Default branch"),
        "{}",
        branches.body
    );
    assert!(branches.body.contains(">main<"));
    // The group's New chat preselects it.
    let home = octo.get(&world, &format!("/?project={project}")).await;
    assert!(
        home.body.contains(r#"aria-label="Branch: main""#),
        "{}",
        home.body
    );
    let csrf = chat_csrf(&home.body);

    // A chat sent with the project records it and its branch.
    let sent = octo
        .post(
            &world,
            "/chat",
            &[
                ("q", "What does this repository do?"),
                ("request_id", LOOSE),
                ("csrf", &csrf),
                ("project", &project),
                ("branch", "main"),
                ("target", ""),
            ],
        )
        .await;
    assert_eq!(sent.status, StatusCode::SEE_OTHER, "{}", sent.body);
    let owner = account_owner_of(&world, "octo-local");
    let chat = world
        .store
        .load(&owner, LOOSE)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    assert_eq!(chat.project.as_deref(), Some(project.as_str()));
    assert_eq!(chat.branch.as_deref(), Some("main"));
    // Its page keeps them in the row.
    let page = octo.get(&world, &format!("/chat/{LOOSE}")).await;
    assert!(
        page.body.contains(r#"aria-label="Branch: main""#),
        "{}",
        page.body
    );

    // Picks that can't work are refused, and nothing is created.
    for (branch, target) in [
        ("no-such-branch", ""),
        ("main", "claude:env-1"),
        ("main", "coder:studio-mac"),
    ] {
        let refused = octo
            .post(
                &world,
                "/chat",
                &[
                    ("q", "Run it"),
                    ("request_id", REQUEST),
                    ("csrf", &csrf),
                    ("project", &project),
                    ("branch", branch),
                    ("target", target),
                ],
            )
            .await;
        assert_eq!(
            refused.status,
            StatusCode::CONFLICT,
            "{target}: {}",
            refused.body
        );
        crate::copy_guard::assert_plain("/chat", &refused.body);
        assert!(world.store.load(&owner, REQUEST).await.unwrap().is_none());
    }

    // Coder with sync on checked in: Where it runs offers it.
    online(&world, &owner, "studio-mac").await;
    let home = octo.get(&world, "/").await;
    assert!(
        home.body.contains(r#"aria-label="Where it runs: Chat""#),
        "{}",
        home.body
    );
    let panel = octo.get(&world, "/composer/row/target").await;
    assert!(panel.body.contains("Coder on studio-mac"), "{}", panel.body);
    assert!(panel.body.contains("target=coder%3Astudio-mac"));
    crate::copy_guard::assert_plain("/composer/row/target", &panel.body);
    // On an existing chat's row too (#11319), and after the first
    // message: the chat's page and its row's panel offer Coder.
    let page = octo.get(&world, &format!("/chat/{LOOSE}")).await;
    assert!(
        page.body.contains(r#"aria-label="Where it runs: Chat""#),
        "{}",
        page.body
    );
    let panel = octo
        .get(
            &world,
            &format!("/composer/row/target?chat={LOOSE}&project={project}&branch=main&target="),
        )
        .await;
    assert!(panel.body.contains("Coder on studio-mac"), "{}", panel.body);

    // Sent there: a Coder chat whose first message waits for Coder.
    let sent = octo
        .post(
            &world,
            "/chat",
            &[
                ("q", "Fix the flaky test"),
                ("request_id", REQUEST),
                ("csrf", &csrf),
                ("project", &project),
                ("branch", "main"),
                ("target", "coder:studio-mac"),
            ],
        )
        .await;
    assert_eq!(sent.status, StatusCode::SEE_OTHER, "{}", sent.body);
    let session = crate::coder_sync::web_session(REQUEST);
    let id = crate::coder_sync::chat_id(&owner, &session);
    assert_eq!(sent.location(), format!("/chat/{id}"));
    let chat = world
        .store
        .load(&owner, &id)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    let terminal = chat.terminal.as_ref().unwrap();
    assert_eq!(terminal.computer, "studio-mac");
    assert_eq!(terminal.replies[0].text, "Fix the flaky test");
    assert_eq!(chat.title, "Fix the flaky test");
    assert_eq!(chat.project.as_deref(), Some(project.as_str()));
    let computers = world.store.computers(&owner).await.unwrap();
    assert_eq!(
        computers.waiting.get(&session).map(String::as_str),
        Some("studio-mac")
    );
    // Sent again (a retried form): the same chat, once.
    let again = octo
        .post(
            &world,
            "/chat",
            &[
                ("q", "Fix the flaky test"),
                ("request_id", REQUEST),
                ("csrf", &csrf),
                ("project", &project),
                ("branch", "main"),
                ("target", "coder:studio-mac"),
            ],
        )
        .await;
    assert_eq!(again.location(), format!("/chat/{id}"));
    let chat = world.store.load(&owner, &id).await.unwrap().unwrap();
    assert_eq!(chat.conversation.terminal.unwrap().replies.len(), 1);
    // Coder takes it at its next check-in, like any reply.
    let taken = crate::coder_sync::take_replies(&world.store, &owner, &session)
        .await
        .unwrap();
    assert!(
        matches!(&taken, crate::coder_sync::Taken::Replies { replies, .. } if replies.len() == 1),
        "{taken:?}"
    );

    // A later message in the chat that began here, sent to Coder (#11319):
    // the chat moves to a Coder chat on that computer, its messages so
    // far shown there and handed to Coder before the new one.
    for _ in 0..200 {
        let chat = world.store.load(&owner, LOOSE).await.unwrap().unwrap();
        if chat.conversation.pending.is_none() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let before = world
        .store
        .load(&owner, LOOSE)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    assert!(before.pending.is_none() && before.terminal.is_none());
    let page = octo.get(&world, &format!("/chat/{LOOSE}")).await;
    let csrf = chat_csrf(&page.body);
    let sent = octo
        .post(
            &world,
            &format!("/chat/{LOOSE}"),
            &[
                ("q", "Push a README"),
                ("request_id", LATER),
                ("csrf", &csrf),
                ("project", &project),
                ("branch", "main"),
                ("target", "coder:studio-mac"),
            ],
        )
        .await;
    let session = crate::coder_sync::web_session(LATER);
    let id = crate::coder_sync::chat_id(&owner, &session);
    assert_eq!(sent.location(), format!("/chat/{id}"), "{}", sent.body);
    let moved = world
        .store
        .load(&owner, &id)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    let terminal = moved.terminal.as_ref().unwrap();
    assert_eq!(terminal.computer, "studio-mac");
    assert_eq!(terminal.replies[0].text, "Push a README");
    assert_eq!(moved.messages[0].text, "What does this repository do?");
    assert_eq!(terminal.continued[0].text, "What does this repository do?");
    assert_eq!(moved.project.as_deref(), Some(project.as_str()));
    let taken = crate::coder_sync::take_replies(&world.store, &owner, &session)
        .await
        .unwrap();
    assert!(
        matches!(&taken, crate::coder_sync::Taken::Replies { replies, continued, .. }
            if replies.len() == 1 && continued[0].text == "What does this repository do?"),
        "{taken:?}"
    );
    // The Coder chat's own row has no Where it runs: its messages go to
    // its computer.
    let page = octo.get(&world, &format!("/chat/{id}")).await;
    assert!(!page.body.contains("Where it runs"), "{}", page.body);
}

/// #11318: under the newest answer of a chat that began here, a signed-in
/// person with Coder online gets Run in Coder on {computer}, whatever the
/// answer said; it sends their last message there with the chat so far
/// and opens the Coder chat, after which the offer is gone.
#[tokio::test]
async fn the_newest_answer_offers_to_run_the_message_in_coder() {
    let world = world().await;
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    let owner = account_owner_of(&world, "octo-local");
    let said = |role, text: &str| crate::chat_store::Message {
        role,
        text: text.into(),
        request_id: Some(REQUEST.into()),
    };
    world
        .store
        .create(&crate::chat_store::Conversation {
            id: REQUEST.into(),
            owner: owner.clone(),
            revision: 1,
            title: "Finances".into(),
            messages: vec![
                said(
                    crate::chat_store::Role::User,
                    "push a README to AtlantisPleb/finances",
                ),
                said(
                    crate::chat_store::Role::Assistant,
                    "Coder on your computer can do that.",
                ),
            ],
            pending: None,
            requests: Vec::new(),
            selection: None,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
            project: None,
            terminal: None,
            environment: None,
            tasks: Vec::new(),
            opened_unix: None,
            branch: None,
        })
        .await
        .unwrap();
    let offer = format!("/chat/{REQUEST}/to-coder");
    let page = octo.get(&world, &format!("/chat/{REQUEST}")).await;
    assert!(page.body.contains(&offer), "{}", page.body);
    // No computer online: nothing to offer.
    let loaded = octo.get(&world, &offer).await;
    assert_eq!(loaded.status, StatusCode::OK);
    assert!(!loaded.body.contains("Run in Coder"), "{}", loaded.body);

    online(&world, &owner, "studio-mac").await;
    let loaded = octo.get(&world, &offer).await;
    assert!(
        loaded.body.contains("Run in Coder on studio-mac"),
        "{}",
        loaded.body
    );
    crate::copy_guard::assert_plain("/chat/to-coder", &loaded.body);
    let csrf = hidden(&loaded.body, &format!(r#"action="{offer}""#), "csrf");
    let sent = octo
        .post(
            &world,
            &offer,
            &[
                ("csrf", &csrf),
                ("request_id", REQUEST),
                ("computer", "studio-mac"),
            ],
        )
        .await;
    let session = crate::coder_sync::web_session(REQUEST);
    let id = crate::coder_sync::chat_id(&owner, &session);
    assert_eq!(sent.location(), format!("/chat/{id}"), "{}", sent.body);
    let moved = world
        .store
        .load(&owner, &id)
        .await
        .unwrap()
        .unwrap()
        .conversation;
    let terminal = moved.terminal.as_ref().unwrap();
    assert_eq!(terminal.computer, "studio-mac");
    assert_eq!(
        terminal.replies[0].text,
        "push a README to AtlantisPleb/finances"
    );
    let loaded = octo.get(&world, &offer).await;
    assert!(!loaded.body.contains("Run in Coder"), "{}", loaded.body);
}

/// Serves a signed-in world on `OA_SCREEN_PORT` for headless screenshots
/// of the row (`cargo test ... serve_for_screenshots -- --ignored`): the
/// person has a project, Coder on a computer is online, and one chat was
/// sent with the project. The browsers' cookies, the project id, the chat
/// id, and the Claude Code row (rendered by the row itself, since this
/// world has no environments studio) go to `OA_SCREEN_OUT`.
#[tokio::test]
#[ignore = "serves a local site for screenshots"]
async fn serve_for_screenshots() {
    let port: u16 = std::env::var("OA_SCREEN_PORT").unwrap().parse().unwrap();
    let out = std::path::PathBuf::from(std::env::var("OA_SCREEN_OUT").unwrap());
    HOST_OVERRIDE.set(format!("127.0.0.1:{port}")).unwrap();
    let world = world_on(port).await;
    let mut octo = Browser::default();
    octo.sign_in(&world, "octo-local").await;
    connect_public(&mut octo, &world, "octo-local").await;
    let project = add_project(&mut octo, &world, "octo-local/hello-world").await;
    let owner = account_owner_of(&world, "octo-local");
    online(&world, &owner, "studio-mac").await;
    let home = octo.get(&world, "/").await;
    let csrf = chat_csrf(&home.body);
    let sent = octo
        .post(
            &world,
            "/chat",
            &[
                ("q", "What does this repository do?"),
                ("request_id", LOOSE),
                ("csrf", &csrf),
                ("project", &project),
                ("branch", "main"),
                ("target", ""),
            ],
        )
        .await;
    assert_eq!(sent.status, StatusCode::SEE_OTHER, "{}", sent.body);
    let mut quiet = Browser::default();
    quiet.sign_in(&world, "quiet-local").await;
    let choices = crate::composer_row::Choices {
        projects: vec![oa_auth::repos::Project {
            id: project.clone(),
            name: "hello-world".into(),
            repository_id: 7001,
            repository: "octo-local/hello-world".into(),
            default_branch: "main".into(),
            private: false,
            created_unix: 1,
            installation_id: None,
        }],
        connected: true,
        environments: vec![crate::composer_row::Environment {
            id: "env-1".into(),
            repository: "octo-local/hello-world".into(),
            branch: "main".into(),
            version: 3,
        }],
        computers: vec!["studio-mac".into()],
        account: true,
        claude_connect: false,
        claude_subscription: false,
        claude_signed_in: vec![],
        claude_keys: false,
    };
    let picked = choices.resolve(&crate::composer_row::Wanted {
        project: project.clone(),
        branch: "main".into(),
        target: "claude:env-1".into(),
        chat: None,
        focus: None,
    });
    let claude_row = crate::composer_row::row(Some(&choices), &picked, None, None, false);
    let cookies = |b: &Browser| {
        b.0.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    };
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("cookies-octo.txt"), cookies(&octo)).unwrap();
    std::fs::write(out.join("cookies-quiet.txt"), cookies(&quiet)).unwrap();
    std::fs::write(out.join("project.txt"), &project).unwrap();
    std::fs::write(out.join("chat.txt"), LOOSE).unwrap();
    std::fs::write(out.join("claude-row.html"), claude_row.into_string()).unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    std::fs::write(out.join("ready"), "ok").unwrap();
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(900),
        axum::serve(listener, world.site.clone()),
    )
    .await;
}

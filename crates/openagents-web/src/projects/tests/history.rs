//! Saved history through real app authentication and project authorization.
use super::*;
use crate::chat_store::{Message, Role};

#[tokio::test]
async fn complete_history_is_bounded_archived_and_read_only() {
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    for n in 0..205 {
        let mut c = chat(
            &format!("{n:08x}-1234-4234-8234-123456789abc"),
            &owner,
            "Saved",
            None,
        );
        if n == 0 {
            c.archived_unix = Some(1);
            c.pinned_unix = Some(1);
        }
        world.store.create(&c).await.unwrap();
    }
    let (_, first) = api
        .json(&world, "GET", "/v1/history?limit=200", Value::Null)
        .await;
    assert_eq!(first["data"].as_array().unwrap().len(), 200);
    assert_eq!(first["data"][0]["archived_unix"], 1);
    let after = first["next"].as_str().unwrap();
    // A deleted cursor still continues by ID; activity and pin changes don't move rows.
    let loaded = world.store.load(&owner, after).await.unwrap().unwrap();
    world
        .store
        .delete(&owner, after, &loaded.generation)
        .await
        .unwrap();
    let (_, last) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history?limit=200&after={after}"),
            Value::Null,
        )
        .await;
    assert_eq!(last["data"].as_array().unwrap().len(), 5);
    assert!(last["next"].is_null());
    assert_eq!(
        api.json(&world, "GET", "/v1/history?after=bad", Value::Null)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        api.json(&world, "GET", "/v1/history?limit=201", Value::Null)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let other = signed_in(&world, "quiet-local").await;
    let (_, theirs) = other.json(&world, "GET", "/v1/history", Value::Null).await;
    assert_eq!(theirs["data"], json!([]));
    assert_eq!(
        Api { token: None }
            .json(&world, "GET", "/v1/history", Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    let mut c = chat(CHAT, &owner, "Long history", None);
    c.messages = (0..405)
        .map(|n| Message {
            role: if n % 4 == 0 {
                Role::Thinking
            } else {
                Role::Assistant
            },
            text: format!("Saved {n}"),
            request_id: None,
        })
        .collect();
    let loaded = world.store.create(&c).await.unwrap();
    let (_, first) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history/{CHAT}/messages?limit=200"),
            Value::Null,
        )
        .await;
    assert_eq!(first["data"].as_array().unwrap().len(), 200);
    assert_eq!(first["data"][0]["role"], "thinking");
    let (_, second) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history/{CHAT}/messages?limit=200&after=msg_199&revision=1"),
            Value::Null,
        )
        .await;
    assert_eq!(second["data"][0]["id"], "msg_200");
    let (_, last) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history/{CHAT}/messages?after=msg_399&revision=1"),
            Value::Null,
        )
        .await;
    assert_eq!(last["data"].as_array().unwrap().len(), 5);
    assert!(last["next"].is_null());
    assert_eq!(
        world
            .store
            .load(&owner, CHAT)
            .await
            .unwrap()
            .unwrap()
            .generation,
        loaded.generation
    );
    assert_eq!(
        api.json(
            &world,
            "GET",
            &format!("/v1/history/{CHAT}/messages?after=msg_199"),
            Value::Null
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    c.revision += 1;
    world.store.compare_and_swap(&loaded, &c).await.unwrap();
    assert_eq!(
        api.json(
            &world,
            "GET",
            &format!("/v1/history/{CHAT}/messages?after=msg_199&revision=1"),
            Value::Null
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        other
            .json(
                &world,
                "GET",
                &format!("/v1/history/{CHAT}/messages"),
                Value::Null
            )
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // The legacy envelope still returns newest 200 and offset pages.
    let (_, legacy) = api
        .json(&world, "GET", &format!("/v1/threads/{CHAT}"), Value::Null)
        .await;
    assert_eq!(legacy["messages"].as_array().unwrap().len(), 200);
    assert_eq!(legacy["earlier"], 205);
    assert!(legacy.get("data").is_none());
    let key = Store::owner_key(&owner, "stub").unwrap();
    let folder = world
        .root()
        .join("chats")
        .join(key.strip_suffix("/stub").unwrap());
    std::fs::remove_dir_all(&folder).unwrap();
    std::fs::write(&folder, b"unavailable directory").unwrap();
    let (status, body) = api.json(&world, "GET", "/v1/history", Value::Null).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(body.get("data").is_none());
}

#[tokio::test]
async fn shared_history_and_saved_files_follow_current_membership() {
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let (_, made) = api
        .json(&world, "POST", "/v1/projects", json!({"name":"History"}))
        .await;
    let project = made["id"].as_str().unwrap();
    let mut c = chat(CHAT, &owner, "File history", Some(project));
    let file = crate::chat_files::save(
        &world.store,
        &owner,
        CHAT,
        "notes.txt",
        crate::chat_files::Kind::Text,
        b"Saved bytes".to_vec(),
    )
    .await
    .unwrap();
    c.messages = vec![Message {
        role: Role::User,
        text: "Read notes".into(),
        request_id: Some(CHAT.into()),
    }];
    c.requests = vec![crate::chat_store::Request {
        turn_error: None,
        id: CHAT.into(),
        digest: "d".repeat(64),
        outcome: crate::chat_store::Outcome::Answered,
        selection: None,
        cloud: None,
        reply: None,
        files: vec![file.clone()],
    }];
    world.store.create(&c).await.unwrap();
    for n in 0..204 {
        let id = format!("50000000-{n:04x}-4234-8234-123456789abc");
        world
            .store
            .create(&chat(&id, &owner, "Shared saved", Some(project)))
            .await
            .unwrap();
    }
    let (_, invited) = api
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{project}/invitations"),
            json!({"role":"viewer"}),
        )
        .await;
    let (status, joined) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code":invited["code"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    let (_, projects) = other
        .json(&world, "GET", "/v1/history/projects", Value::Null)
        .await;
    assert_eq!(projects["data"][0]["shared"], true);
    let (status, rows) = other
        .json(
            &world,
            "GET",
            &format!("/v1/history?project={project}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rows}");
    assert_eq!(rows["data"][0]["id"], CHAT);
    assert_eq!(rows["data"].as_array().unwrap().len(), 100);
    let after = rows["next"].as_str().unwrap();
    let (_, second) = other
        .json(
            &world,
            "GET",
            &format!("/v1/history?project={project}&after={after}&limit=200"),
            Value::Null,
        )
        .await;
    assert_eq!(second["data"].as_array().unwrap().len(), 105);
    assert!(second["next"].is_null());

    let (_, messages) = other
        .json(
            &world,
            "GET",
            &format!("/v1/history/{CHAT}/messages"),
            Value::Null,
        )
        .await;
    assert_eq!(messages["data"][0]["files"][0]["name"], "notes.txt");
    let path = format!("/v1/history/{CHAT}/files/{}", file.id);
    let (status, bytes) = other.raw(&world, "GET", &path, Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"Saved bytes");
    let id = world.account_of("quiet-local");
    let (status, body) = api
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{project}/members/{id}"),
            Value::Null,
        )
        .await;
    assert!(status.is_success(), "{status} {body}");
    assert_eq!(
        other.raw(&world, "GET", &path, Body::empty()).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        other
            .json(
                &world,
                "GET",
                &format!("/v1/history?project={project}"),
                Value::Null
            )
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    crate::chat_files::forget(&world.store, &owner, CHAT, &file.id)
        .await
        .unwrap();
    assert_eq!(
        api.raw(&world, "GET", &path, Body::empty()).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn offline_synced_tools_and_phone_sources_need_no_computer() {
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let upload: crate::coder_sync::Upload = serde_json::from_value(json!({"title":"Synced","computer":"Offline","messages":[{"role":"tool","text":"Read file","step":{"name":"read","summary":"Read file","status":"done","input":"file.txt","output":"Full saved output"}},{"role":"thinking","text":"Consider"}]})).unwrap();
    let saved = crate::coder_sync::save(&world.store, &owner, "phone-1791504069", &upload)
        .await
        .unwrap();
    let id = match saved {
        crate::coder_sync::Saved::Saved { chat, .. } => chat,
        _ => panic!("unexpected"),
    };
    let loaded = world.store.load(&owner, &id).await.unwrap().unwrap();
    let mut c = loaded.conversation.clone();
    c.revision += 1;
    c.terminal
        .as_mut()
        .unwrap()
        .asks
        .push(crate::chat_store::ComputerAsk {
            id: CHAT.into(),
            action: crate::chat_store::AskAction::Screenshot,
            asked_unix: 1,
            state: crate::chat_store::AskState::Saved {
                size: 7,
                media: "text/plain; charset=utf-8".into(),
            },
        });
    world.store.compare_and_swap(&loaded, &c).await.unwrap();
    let key = Store::capture_key(&owner, &id, CHAT).unwrap();
    world
        .store
        .write_key(&key, b"Capture".to_vec(), None)
        .await
        .unwrap();
    let path = format!("/v1/history/{id}/captures/{CHAT}");
    let (status, bytes) = api.raw(&world, "GET", &path, Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"Capture");
    let (_, rows) = api.json(&world, "GET", "/v1/history", Value::Null).await;
    assert_eq!(rows["data"][0]["source"], "phone");
    let (_, page) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history/{id}/messages"),
            Value::Null,
        )
        .await;
    assert_eq!(page["data"][0]["tool"]["output"], "Full saved output");
    assert_eq!(page["data"][1]["role"], "thinking");
    assert_eq!(world.store.computers(&owner).await.unwrap().seen.len(), 0);
}

#[tokio::test]
async fn discovery_continues_beyond_legacy_ten_thousand_cap_with_bounded_pages() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().to_owned());
    let owner = crate::chat_store::account_owner("acct_fixture");
    let key = Store::owner_key(&owner, "stub").unwrap();
    let folder = key.strip_suffix("/stub").unwrap();
    std::fs::create_dir_all(root.path().join(folder)).unwrap();
    for n in 0..10_005 {
        let id = format!("{n:08x}-1234-4234-8234-123456789abc");
        std::fs::write(root.path().join(folder).join(format!("{id}.json")), b"{}").unwrap();
    }
    assert!(store.list(&owner).await.is_err());
    let mut cursor = None;
    let mut all = std::collections::BTreeSet::new();
    loop {
        let (ids, next) = store
            .chat_ids_page(folder, false, cursor.as_deref(), 200)
            .await
            .unwrap();
        assert!(ids.len() <= 200);
        for id in ids {
            assert!(all.insert(id));
        }
        match next {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(all.len(), 10_005);
}

#[tokio::test]
async fn saved_search_and_rename_use_owned_data_and_confirmed_cas() {
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    world
        .store
        .create(&chat(CHAT, &owner, "Before", None))
        .await
        .unwrap();
    let (status, renamed) = api
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{CHAT}"),
            json!({"title":"  🦀 After  "}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{renamed}");
    assert_eq!(renamed["title"], "🦀 After");
    let (_, rows) = api
        .json(&world, "GET", "/v1/history?query=after", Value::Null)
        .await;
    assert_eq!(rows["data"][0]["id"], CHAT);
    let (_, absent) = api
        .json(&world, "GET", "/v1/history?query=before", Value::Null)
        .await;
    assert_eq!(absent["data"], json!([]));
    let revision = world
        .store
        .load(&owner, CHAT)
        .await
        .unwrap()
        .unwrap()
        .conversation
        .revision;
    let (status, _) = api
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{CHAT}"),
            json!({"title":"🦀 After"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        world
            .store
            .load(&owner, CHAT)
            .await
            .unwrap()
            .unwrap()
            .conversation
            .revision,
        revision
    );
    for title in [
        json!(""),
        json!("x".repeat(121)),
        json!("one\ntwo"),
        json!(false),
    ] {
        assert_eq!(
            api.json(
                &world,
                "PATCH",
                &format!("/v1/threads/{CHAT}"),
                json!({"title":title})
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let other = signed_in(&world, "quiet-local").await;
    assert_eq!(
        other
            .json(
                &world,
                "PATCH",
                &format!("/v1/threads/{CHAT}"),
                json!({"title":"Stolen"})
            )
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, rows) = other
        .json(&world, "GET", "/v1/history?query=after", Value::Null)
        .await;
    assert_eq!(rows["data"], json!([]));
    assert_eq!(
        world
            .store
            .load(&owner, CHAT)
            .await
            .unwrap()
            .unwrap()
            .conversation
            .title,
        "🦀 After"
    );
}

#[tokio::test]
async fn saved_search_retains_a_cursor_for_an_empty_authorized_page() {
    let world = world().await;
    let api = signed_in(&world, "octo-local").await;
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    for n in 0..3 {
        world
            .store
            .create(&chat(
                &format!("{n:08x}-1234-4234-8234-123456789abc"),
                &owner,
                if n == 2 { "Needle" } else { "Other" },
                None,
            ))
            .await
            .unwrap();
    }
    let (_, first) = api
        .json(
            &world,
            "GET",
            "/v1/history?limit=2&query=needle",
            Value::Null,
        )
        .await;
    assert_eq!(first["data"], json!([]));
    let cursor = first["next"].as_str().unwrap();
    let (_, last) = api
        .json(
            &world,
            "GET",
            &format!("/v1/history?limit=2&query=needle&after={cursor}"),
            Value::Null,
        )
        .await;
    assert_eq!(last["data"][0]["title"], "Needle");
    assert!(last["next"].is_null());
}

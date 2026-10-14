//! Shared projects over `/v1` (#11585), through the whole site and the
//! local account service: the owner invites, a second account previews
//! and joins, reads the project, its files, and its notes by role, and
//! loses it all on the request after it is removed; an `only_me` project
//! can't be shared; leaving and giving the project away.

use super::*;
use serde_json::Value;

struct Api {
    token: Option<String>,
}

impl Api {
    fn of(browser: &Browser) -> Self {
        Self {
            token: browser.0.get("oa_cloud_session").cloned(),
        }
    }

    async fn raw(
        &self,
        world: &World,
        method: &str,
        path: &str,
        body: Body,
    ) -> (StatusCode, Vec<u8>) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, world.host.as_str())
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = &self.token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let response = world
            .site
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec();
        (status, bytes)
    }

    async fn json(
        &self,
        world: &World,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let body = if body.is_null() {
            Body::empty()
        } else {
            Body::from(body.to_string())
        };
        let (status, bytes) = self.raw(world, method, path, body).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

async fn signed_in(world: &World, login: &str) -> Api {
    let mut browser = Browser::default();
    browser.sign_in(world, login).await;
    Api::of(&browser)
}

#[tokio::test]
async fn an_invited_member_works_by_role_and_loses_access_when_removed() {
    let world = world().await;
    let owner = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;

    let (status, made) = owner
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "household", "instructions": "Be brief."}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    let id = made["id"].as_str().unwrap().to_owned();
    assert_eq!(made["role"], "owner");
    assert_eq!(made["shared"], false);
    let (_, note) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"document": {"title": "Plan", "text": "Pay the rent."}}),
        )
        .await;
    let note = note["id"].as_str().unwrap().to_owned();

    // Not in it: 404 everywhere, never 403.
    for (method, path) in [
        ("GET", format!("/v1/projects/{id}")),
        ("GET", format!("/v1/projects/{id}/members")),
        ("GET", format!("/v1/projects/{id}/attachments")),
        ("POST", format!("/v1/projects/{id}/invitations")),
    ] {
        let (status, body) = other.json(&world, method, &path, json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {body}");
    }

    // A viewer's link: previewed, then used.
    let (status, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role": "viewer"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{invited}");
    let url = invited["url"].as_str().unwrap().to_owned();
    assert!(url.contains("/projects/join/prjinv_"), "{url}");
    assert_eq!(invited["role"], "viewer");
    let (status, offered) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/preview",
            json!({"code": url}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{offered}");
    assert_eq!(offered["project"]["name"], "household");
    assert_eq!(offered["role"], "viewer");
    assert_eq!(offered["member"], Value::Null);
    let (status, joined) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": url}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    assert_eq!(joined["role"], "viewer");
    assert_eq!(joined["shared"], true);
    // The link was single-use.
    let (status, body) = owner
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": url}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error"]["code"], "invitation_not_found");

    // The viewer lists it with their role, and reads its notes.
    let (_, listed) = other.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"][0]["id"], id.as_str());
    assert_eq!(listed["data"][0]["role"], "viewer");
    assert_eq!(listed["data"][0]["instructions"], "Be brief.");
    assert_eq!(listed["data"][0]["counts"]["documents"], 1);
    let (status, doc) = other
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/documents/{note}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{doc}");
    assert_eq!(doc["text"], "Pay the rent.");
    let (_, people) = other
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/members"),
            Value::Null,
        )
        .await;
    assert_eq!(people["data"].as_array().unwrap().len(), 2, "{people}");
    assert!(people.get("invitations").is_none());
    let me = people["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["you"] == true)
        .unwrap()["account"]
        .as_str()
        .unwrap()
        .to_owned();

    // ...and changes nothing.
    for (method, path, body) in [
        (
            "PATCH",
            format!("/v1/projects/{id}"),
            json!({"instructions": "x"}),
        ),
        (
            "PATCH",
            format!("/v1/projects/{id}"),
            json!({"name": "mine"}),
        ),
        (
            "POST",
            format!("/v1/projects/{id}/attachments"),
            json!({"document": {"title": "t", "text": "x"}}),
        ),
        (
            "DELETE",
            format!("/v1/projects/{id}/attachments/{note}"),
            Value::Null,
        ),
        ("POST", format!("/v1/projects/{id}/invitations"), json!({})),
        (
            "DELETE",
            format!("/v1/projects/{id}?detach_threads=true"),
            Value::Null,
        ),
        (
            "POST",
            "/v1/threads".to_owned(),
            json!({"request_id": "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b10", "text": "hi", "project": id}),
        ),
    ] {
        let (status, body) = other.json(&world, method, &path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
        assert_eq!(body["error"]["code"], "project_role", "{body}");
    }

    // ...nor a file (#11588).
    let (status, bytes) = other
        .raw(
            &world,
            "POST",
            "/v1/files?purpose=project&filename=mine.txt",
            Body::from("mine"),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let mine = serde_json::from_slice::<Value>(&bytes).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"file": mine}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    // An editor adds notes and changes instructions, but doesn't rename.
    let (status, body) = owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}/members/{me}"),
            json!({"role": "editor"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = other
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"instructions": "Be kind."}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["instructions"], "Be kind.");
    let (status, _) = other
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"name": "mine"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, added) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"document": {"title": "Groceries", "text": "Milk."}}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    // The owner sees the editor's note: it is kept with the project.
    let (_, kept) = owner
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/attachments"),
            Value::Null,
        )
        .await;
    assert_eq!(kept["data"].as_array().unwrap().len(), 2, "{kept}");
    // An editor's file moves into the project, and both read it.
    let (status, _, upload) = {
        let (status, bytes) = other
            .raw(
                &world,
                "POST",
                "/v1/files?purpose=project&filename=list.txt",
                Body::from("eggs"),
            )
            .await;
        (status, (), serde_json::from_slice::<Value>(&bytes).unwrap())
    };
    assert_eq!(status, StatusCode::CREATED, "{upload}");
    let file = upload["id"].as_str().unwrap().to_owned();
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"file": file}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    for reader in [&owner, &other] {
        let (status, bytes) = reader
            .raw(
                &world,
                "GET",
                &format!("/v1/files/{file}/content?project={id}"),
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes, b"eggs");
    }

    // Removed: the next request finds nothing.
    let (status, body) = owner
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{id}/members/{me}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, listed) = other.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"], json!([]));
    for path in [
        format!("/v1/projects/{id}"),
        format!("/v1/projects/{id}/documents/{note}"),
        format!("/v1/files/{file}/content?project={id}"),
    ] {
        let (status, _) = other.json(&world, "GET", &path, Value::Null).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn leaving_giving_away_and_only_me_projects() {
    let world = world().await;
    let owner = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;
    let (_, made) = owner
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "taxes", "protection": "only_me"}),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    // Files that open only on the owner's devices: no invitations.
    let (status, body) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"]["message"],
        "Files in this project open only on your devices. Change it to standard to share it."
    );
    owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"protection": "standard"}),
        )
        .await;
    let (_, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({}),
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
    assert_eq!(joined["role"], "editor");
    // Shared: it can't go back to only_me.
    let (status, body) = owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"protection": "only_me"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    // The owner can't leave; the member can.
    let (status, body) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/leave"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/leave"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["object"], "project.left");
    let (status, _) = other
        .json(&world, "GET", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Back in, then given the project: the old owner edits.
    let (_, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role": "viewer"}),
        )
        .await;
    other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": invited["code"]}),
        )
        .await;
    let (_, people) = owner
        .json(
            &world,
            "GET",
            &format!("/v1/projects/name:taxes/members"),
            Value::Null,
        )
        .await;
    let theirs = people["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["you"] == false)
        .unwrap()["account"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}/members/{theirs}"),
            json!({"role": "owner"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], "editor");
    let (_, listed) = other.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"][0]["role"], "owner");
    let (status, _) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Records whose key each answer is asked under: the sender's allowance.
struct Payers(Arc<std::sync::Mutex<Vec<String>>>);

impl crate::ask::Chat for Payers {
    fn door(
        &self,
        secret: secp256k1::SecretKey,
    ) -> Result<Box<dyn openagents_chat::basic_coder::Door>, String> {
        let public = secp256k1::PublicKey::from_secret_key(&secp256k1::Secp256k1::new(), &secret);
        self.0.lock().unwrap().push(public.to_string());
        Err("This test worker only notes who pays.".into())
    }
}

fn payer_of(owner: &str) -> String {
    let secret = crate::ask::key(&CHAT_SALT, owner);
    secp256k1::PublicKey::from_secret_key(&secp256k1::Secp256k1::new(), &secret).to_string()
}

async fn paid_by(payers: &std::sync::Mutex<Vec<String>>, owner: &str) -> bool {
    let want = payer_of(owner);
    for _ in 0..100 {
        if payers.lock().unwrap().contains(&want) {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    false
}

/// #11586: every member lists and reads the project's chats with who
/// started each; a viewer can't send; an editor's reply is answered under
/// the editor's own allowance; a removed member gets 404.
#[tokio::test]
async fn members_see_the_projects_chats_and_each_sender_pays() {
    let payers = Arc::new(std::sync::Mutex::new(Vec::new()));
    let world = world_with(Options {
        chat: Some(Arc::new(Payers(payers.clone()))),
        ..Options::default()
    })
    .await;
    let owner = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;
    let owner_store = account_owner_of(&world, "octo-local");
    let other_store = account_owner_of(&world, "quiet-local");
    let (_, made) = owner
        .json(&world, "POST", "/v1/projects", json!({"name": "household"}))
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    // A chat started before sharing moves in with the project.
    let early = "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b11";
    let (status, body) = owner
        .json(
            &world,
            "POST",
            "/v1/threads",
            json!({"request_id": early, "text": "Plan the week.", "project": id}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(paid_by(&payers, &owner_store).await);
    // One outside any project stays private.
    let private = "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b12";
    owner
        .json(
            &world,
            "POST",
            "/v1/threads",
            json!({"request_id": private, "text": "Just mine."}),
        )
        .await;

    let (_, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role": "viewer"}),
        )
        .await;
    other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": invited["code"]}),
        )
        .await;
    let (status, threads) = other
        .json(
            &world,
            "GET",
            &format!("/v1/threads?project={id}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{threads}");
    let rows = threads["data"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{threads}");
    assert_eq!(rows[0]["id"], early);
    assert_eq!(rows[0]["started_by"]["you"], false);
    assert_ne!(rows[0]["started_by"]["name"], "A former member");
    let (status, messages) = other
        .json(
            &world,
            "GET",
            &format!("/v1/threads/{early}/messages"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{messages}");
    assert_eq!(messages["data"][0]["content"][0]["text"], "Plan the week.");
    let (status, _) = other
        .json(
            &world,
            "GET",
            &format!("/v1/threads/{private}/messages"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = other
        .json(&world, "GET", &format!("/v1/threads/{early}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::OK);

    // A viewer reads only.
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/threads/{early}/messages"),
            json!({"request_id": "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b13", "text": "Add milk."}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _) = other
        .json(&world, "POST", "/v1/threads", json!({"request_id": "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b14", "text": "x", "project": id}))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // An editor replies in the owner's chat and pays for it.
    let (_, people) = owner
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/members"),
            Value::Null,
        )
        .await;
    let them = people["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["you"] == false)
        .unwrap()["account"]
        .as_str()
        .unwrap()
        .to_owned();
    owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}/members/{them}"),
            json!({"role": "editor"}),
        )
        .await;
    // Wait for the first answer to settle (the test worker refuses it).
    for _ in 0..100 {
        let chat = world
            .store
            .load(&owner_store, early)
            .await
            .unwrap()
            .unwrap();
        if chat.conversation.pending.is_none() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/threads/{early}/messages"),
            json!({"request_id": "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b15", "text": "Add milk."}),
        )
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert!(
        paid_by(&payers, &other_store).await,
        "the editor's message is answered under the editor's key"
    );
    let kept = world
        .store
        .load(&owner_store, early)
        .await
        .unwrap()
        .unwrap();
    assert!(
        kept.conversation
            .messages
            .iter()
            .any(|m| m.text == "Add milk.")
    );

    // The same from the website's chat page: the form is signed for the
    // member, and the reply is theirs to pay for.
    for _ in 0..100 {
        let chat = world
            .store
            .load(&owner_store, early)
            .await
            .unwrap()
            .unwrap();
        if chat.conversation.pending.is_none() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let mut page_browser = Browser::default();
    page_browser.sign_in(&world, "quiet-local").await;
    let page = page_browser.get(&world, &format!("/chat/{early}")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("Plan the week."));
    assert!(
        !page.body.contains(&format!("/chat/{early}/delete")),
        "no starter-only actions"
    );
    let token = chat_csrf(&page.body);
    let sealed = {
        let at = page.body.find("id=\"composer-state\"").unwrap();
        let rest = &page.body[at..];
        let rest = &rest[rest.find("value=\"").unwrap() + 7..];
        rest[..rest.find('"').unwrap()].to_owned()
    };
    let before = payers.lock().unwrap().len();
    let sent = page_browser
        .post(
            &world,
            &format!("/chat/{early}"),
            &[
                ("q", "From the page."),
                ("request_id", "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b17"),
                ("csrf", &token),
                ("selection", &sealed),
            ],
        )
        .await;
    assert!(
        sent.status.is_redirection() || sent.status == StatusCode::OK,
        "{}: {}",
        sent.status,
        sent.body
    );
    let kept = world
        .store
        .load(&owner_store, early)
        .await
        .unwrap()
        .unwrap();
    assert!(
        kept.conversation
            .messages
            .iter()
            .any(|m| m.text == "From the page.")
    );
    for _ in 0..100 {
        if payers.lock().unwrap().len() > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        payers.lock().unwrap().last().cloned(),
        Some(payer_of(&other_store))
    );
    // The editor's own new chat shows to the owner.
    let theirs = "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b16";
    let (status, body) = other
        .json(
            &world,
            "POST",
            "/v1/threads",
            json!({"request_id": theirs, "text": "Groceries?", "project": id}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (_, threads) = owner
        .json(
            &world,
            "GET",
            &format!("/v1/threads?project={id}"),
            Value::Null,
        )
        .await;
    let ids: Vec<&str> = threads["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&theirs) && ids.contains(&early), "{threads}");
    let (_, listed) = owner.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"][0]["counts"]["threads"], 2, "{listed}");

    // Removed: nothing on the next request.
    owner
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{id}/members/{them}"),
            Value::Null,
        )
        .await;
    let (status, _) = other
        .json(
            &world,
            "GET",
            &format!("/v1/threads/{early}/messages"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = other
        .json(
            &world,
            "GET",
            &format!("/v1/threads?project={id}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// #11589: the owner makes a link from the project's Members section; the
/// link opens a page naming the project, who invited, and the role, with
/// Join and Decline; signed out it goes through sign-in and back; a
/// viewer's project page has no controls they can't use, and Leave; the
/// pages read in plain words.
#[tokio::test]
async fn sharing_on_the_website() {
    let world = world().await;
    let mut owner = Browser::default();
    owner.sign_in(&world, "octo-local").await;
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let api = Api::of(&owner);
    let (_, made) = api
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "garden", "instructions": "Water on Sundays."}),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let page = owner.get(&world, &format!("/projects/{id}")).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(page.body.contains("Members"), "{}", page.body);
    assert!(page.body.contains("Invite a viewer"));
    crate::copy_guard::assert_plain(&format!("/projects/{id}"), &page.body);
    let csrf = hidden(
        &page.body,
        &format!("action=\"/projects/{id}/invite\""),
        "csrf",
    );
    let made = owner
        .post(
            &world,
            &format!("/projects/{id}/invite"),
            &[("csrf", &csrf), ("role", "viewer")],
        )
        .await;
    assert_eq!(made.status, StatusCode::OK, "{}", made.body);
    assert!(made.body.contains("It's shown only now."), "{}", made.body);
    let at = made.body.find("/projects/join/prjinv_").unwrap();
    let link: String = made.body[at..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_'))
        .collect();

    // Signed out: sign in first, then back here.
    let mut stranger = Browser::default();
    let away = stranger.get(&world, &link).await;
    assert!(away.status.is_redirection(), "{}", away.status);
    assert!(away.location().contains("return_to"), "{}", away.location());

    let invite = other.get(&world, &link).await;
    assert_eq!(invite.status, StatusCode::OK, "{}", invite.body);
    assert!(invite.body.contains("garden"));
    assert!(invite.body.contains("as a viewer"));
    assert!(invite.body.contains("Decline"));
    crate::copy_guard::assert_plain(&link, &invite.body);
    let csrf = hidden(&invite.body, &format!("action=\"{link}\""), "csrf");
    let joined = other.post(&world, &link, &[("csrf", &csrf)]).await;
    assert!(joined.status.is_redirection(), "{}", joined.body);
    assert_eq!(joined.location(), format!("/projects/{id}"));

    // The viewer's page: instructions to read, no forms but Leave.
    let theirs = other.get(&world, &format!("/projects/{id}")).await;
    assert_eq!(theirs.status, StatusCode::OK);
    assert!(theirs.body.contains("Water on Sundays."));
    assert!(theirs.body.contains("Shared · viewer"));
    assert!(theirs.body.contains("Leave this project"));
    for absent in [
        format!("/projects/{id}/edit"),
        format!("/?project={id}"),
        "Invite a".to_owned(),
        "Make primary".to_owned(),
        "Add a repository".to_owned(),
        "Make owner".to_owned(),
    ] {
        assert!(!theirs.body.contains(&absent), "a viewer sees {absent}");
    }
    crate::copy_guard::assert_plain(&format!("/projects/{id}"), &theirs.body);
    // The left panel lists it, marked.
    let home = other.get(&world, "/").await;
    assert!(
        home.body.contains("garden") && home.body.contains("Shared · viewer"),
        "{}",
        home.body
    );
    // A used link says so.
    let again = other.get(&world, &link).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert!(again.body.contains("Ask for a new one."));
    // Leave.
    let csrf = hidden(
        &theirs.body,
        &format!("action=\"/projects/{id}/leave\""),
        "csrf",
    );
    let left = other
        .post(&world, &format!("/projects/{id}/leave"), &[("csrf", &csrf)])
        .await;
    assert!(left.status.is_redirection(), "{}", left.body);
    let gone = other.get(&world, &format!("/projects/{id}")).await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

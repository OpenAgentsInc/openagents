//! `DELETE /v1/account` (#12002) over HTTP: the confirmation, the dry run,
//! the refusal for a team owner, and what a deletion leaves.
use super::*;

/// DELETE `/v1/account` with a bearer and a JSON body.
async fn delete_account(d: &Deployment, token: &str, body: &Value) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .delete(format!("{}/v1/account", d.address))
            .bearer_auth(token)
            .json(body),
    )
    .await
}

#[tokio::test]
async fn deleting_needs_a_session_and_the_confirmation() {
    let d = deploy(Some(account_config(None)), true).await;
    let pat = join(&d, "pat").await;
    let (status, body) = delete_account(&d, &pat.session_token, &json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "confirm_required");
    let (status, body) =
        delete_account(&d, &pat.key_token, &json!({"confirm": "delete my account"})).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "session_required");
    // A dry run deletes nothing.
    let (status, body) = delete_account(
        &d,
        &pat.session_token,
        &json!({"confirm": "delete my account", "check": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["ready"], true);
    let (status, _) = get(&d, "/v1/account", &pat.session_token).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_deleted_account_signs_nothing_in_and_keeps_no_name() {
    let d = deploy(Some(account_config(None)), true).await;
    let pat = join(&d, "pat").await;
    let (status, body) = delete_account(
        &d,
        &pat.session_token,
        &json!({"confirm": "Delete my account"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["account"]["state"], "deleted");
    let (status, _) = get(&d, "/v1/account", &pat.session_token).await;
    assert_ne!(status, StatusCode::OK);
    let (status, _) = get(&d, "/v1/account", &pat.key_token).await;
    assert_ne!(status, StatusCode::OK);
    let store = Accounts::open(d.dir.path()).unwrap().store().unwrap();
    let record = &store.accounts[&pat.account];
    assert_eq!(record.label, "Deleted account");
    assert!(record.principals.is_empty());
    let keys = keys::load(d.dir.path()).unwrap();
    assert!(
        keys.keys
            .values()
            .filter(|key| pat.key_token.contains(&key.id))
            .all(|key| key.status == keys::Status::Revoked)
    );
}

#[tokio::test]
async fn the_owner_of_a_team_with_members_is_asked_to_hand_it_over() {
    let d = deploy(Some(account_config(None)), true).await;
    let pat = join(&d, "pat").await;
    let sam = join(&d, "sam").await;
    let accounts = Accounts::open(d.dir.path()).unwrap();
    let team = accounts
        .create_workspace(
            &pat.account,
            "Team",
            tenancy::WorkspaceKind::Organization,
            "acme",
            None,
        )
        .unwrap();
    let invited = accounts
        .invite(&pat.account, &team.id, tenancy::Role::Member, 600)
        .unwrap();
    accounts.accept(&sam.account, &invited.token).unwrap();
    let (status, body) = delete_account(
        &d,
        &pat.session_token,
        &json!({"confirm": "delete my account"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "shared_workspace");
    // Sam isn't an owner there, so Sam can go.
    let (status, body) = delete_account(
        &d,
        &sam.session_token,
        &json!({"confirm": "delete my account"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

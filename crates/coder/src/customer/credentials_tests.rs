use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn directory() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}
fn fixture(
    responses: Vec<(u16, Value)>,
) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let posts = Arc::new(AtomicUsize::new(0));
    let count = posts.clone();
    let thread = std::thread::spawn(move || {
        for (status, body) in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut block = [0; 1024];
            loop {
                let n = socket.read(&mut block).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            if bytes.starts_with(b"POST ") || bytes.starts_with(b"DELETE ") {
                count.fetch_add(1, Ordering::SeqCst);
            }
            let body = serde_json::to_vec(&body).unwrap();
            write!(socket,"HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
            socket.write_all(&body).unwrap();
        }
    });
    (address, posts, thread)
}
fn account() -> Value {
    json!({"account":{"id":"ada"},"workspaces":[]})
}
fn grant() -> Value {
    json!({"session":{"id":"fixture-session","kind":"account","account":"ada","created_at":1,"expires_at":100},"token":"sess_private-fixture-issued"})
}
fn command(origin: &str) -> CredentialCommand {
    CredentialCommand {
        id: "sign-in-one".into(),
        origin: origin.into(),
        account: "ada".into(),
        credential_alias: Some("old-key".into()),
        action: CredentialAction::SignIn {
            output_alias: "new-session".into(),
        },
    }
}
fn store(dir: &Path) -> Store {
    let mut store = Store::open(dir).unwrap();
    store
        .import_credential("old-key", &jev::ApiKey::new("oak_private-fixture.old"))
        .unwrap();
    store
}
#[tokio::test]
async fn exact_credential_retries_and_restart_do_not_repeat_the_remote_effect_or_select_an_account()
{
    let dir = directory();
    let (origin, posts, thread) = fixture(vec![(200, account()), (201, grant())]);
    let mut store = store(dir.path());
    let command = command(&origin);
    let view = store
        .change_credential(command.clone(), None)
        .await
        .unwrap();
    assert_eq!(view.status, CredentialStatus::Applied);
    assert!(view.credential_available);
    assert!(!view.selected);
    assert!(store.selected().is_none());
    assert_eq!(
        store
            .change_credential(command.clone(), None)
            .await
            .unwrap()
            .status,
        CredentialStatus::Applied
    );
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(
        store.change_credential(command, None).await.unwrap().status,
        CredentialStatus::Applied
    );
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    thread.join().unwrap();
    let public = serde_json::to_string(&view).unwrap();
    assert!(!public.contains("private-fixture"));
    assert!(!public.contains("authority_digest"));
    let private = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(!private.contains("sess_private-fixture-issued"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("issued/sign-in-one")).unwrap(),
        "sess_private-fixture-issued"
    );
}
#[tokio::test]
async fn uncertain_effect_blocks_renamed_retries_and_omits_private_refusal_material() {
    let dir = directory();
    let (origin, posts, thread) = fixture(vec![
        (200, account()),
        (
            503,
            json!({"error":{"message":"sess_private-refusal-secret"}}),
        ),
    ]);
    let mut store = store(dir.path());
    let mut command = command(&origin);
    let view = store
        .change_credential(command.clone(), None)
        .await
        .unwrap();
    assert_eq!(view.status, CredentialStatus::Unknown);
    assert!(
        !serde_json::to_string(&view)
            .unwrap()
            .contains("private-refusal")
    );
    command.id = "renamed".into();
    command.action = CredentialAction::SignIn {
        output_alias: "another-session".into(),
    };
    assert!(store.change_credential(command, None).await.is_err());
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    thread.join().unwrap();
    assert!(!dir.path().join("credentials/new-session").exists());
}
#[tokio::test]
async fn interrupted_issued_credential_can_be_verified_without_acknowledging_or_replaying_the_effect()
 {
    let dir = directory();
    let (origin, posts, thread) = fixture(vec![(200, account()), (201, grant()), (200, account())]);
    let mut store = store(dir.path());
    store
        .change_credential(command(&origin), None)
        .await
        .unwrap();
    let mut book = store.book.clone();
    book.credential_operations
        .get_mut("sign-in-one")
        .unwrap()
        .status = CredentialStatus::Pending;
    store.persist(book).unwrap();
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    assert_eq!(
        store.book.credential_operations["sign-in-one"].status,
        CredentialStatus::Unknown
    );
    let inspected = store.inspect_credential("sign-in-one").await.unwrap();
    assert_eq!(inspected["current_authentication_verified"], true);
    assert_eq!(inspected["historical_effect_acknowledged"], false);
    assert!(store.selected().is_none());
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    thread.join().unwrap();
}
#[test]
fn private_inputs_and_recovery_intent_never_accept_shared_files_or_account_key_aliases() {
    let dir = directory();
    let path = dir.path().join("key");
    std::fs::write(&path, b"oak_private").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Store::private_input(&path, 4096).is_err());
    }
    let mut cmd = command("https://fixture.invalid");
    cmd.action = CredentialAction::Recover {
        workspace: "ada-personal".into(),
        output_alias: "recovered".into(),
    };
    assert!(cmd.validate().is_err());
    cmd.credential_alias = None;
    assert!(cmd.validate().is_ok());
}

fn key_grant() -> Value {
    json!({"account":"ada","key":{"id":"replacement-key","tenant":"ada-personal"},"key_token":"oak_fixture.replacement"})
}
fn workspace() -> Value {
    json!({"workspace":{"id":"ada-personal","tenant":"ada-personal","members_epoch":2},"role":"owner"})
}
#[tokio::test]
async fn recovery_consumes_one_token_without_renamed_workspace_or_account_replay() {
    let dir = directory();
    let (origin, posts, thread) = fixture(vec![
        (201, key_grant()),
        (200, account()),
        (200, workspace()),
    ]);
    let mut store = store(dir.path());
    let mut cmd = command(&origin);
    cmd.id = "recover-one".into();
    cmd.credential_alias = None;
    cmd.action = CredentialAction::Recover {
        workspace: "ada-personal".into(),
        output_alias: "recovered".into(),
    };
    let token = jev::ApiKey::new("rcv_fixture-once");
    assert_eq!(
        store
            .change_credential(cmd.clone(), Some(token.clone()))
            .await
            .unwrap()
            .status,
        CredentialStatus::Applied
    );
    assert!(store.selected().is_none());
    assert_eq!(
        store
            .change_credential(cmd.clone(), Some(token.clone()))
            .await
            .unwrap()
            .status,
        CredentialStatus::Applied
    );
    for (account, workspace) in [("ada", "another-workspace"), ("grace", "ada-personal")] {
        let mut renamed = cmd.clone();
        renamed.id = format!("another-{account}");
        renamed.account = account.into();
        renamed.action = CredentialAction::Recover {
            workspace: workspace.into(),
            output_alias: format!("another-{account}"),
        };
        assert!(
            store
                .change_credential(renamed, Some(token.clone()))
                .await
                .is_err()
        );
    }
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    thread.join().unwrap();
    let history = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(!history.contains("rcv_fixture-once"));
    assert!(!history.contains("oak_fixture.replacement"));
}
#[tokio::test]
async fn rotation_retains_a_once_issued_key_even_when_current_membership_is_unavailable() {
    let dir = directory();
    let (origin, posts, thread) = fixture(vec![
        (200, account()),
        (201, key_grant()),
        (200, account()),
        (403, json!({"private":"not public"})),
        (200, account()),
        (200, workspace()),
    ]);
    let mut store = store(dir.path());
    let mut cmd = command(&origin);
    cmd.id = "rotate-one".into();
    cmd.action = CredentialAction::Rotate {
        workspace: "ada-personal".into(),
        key: "old-key".into(),
        output_alias: "replacement".into(),
    };
    let view = store.change_credential(cmd.clone(), None).await.unwrap();
    assert_eq!(view.status, CredentialStatus::Unknown);
    assert!(!view.credential_available);
    assert!(dir.path().join("issued/rotate-one").exists());
    assert!(!dir.path().join("credentials/replacement").exists());
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    let inspected = store.inspect_credential("rotate-one").await.unwrap();
    assert_eq!(inspected["current_authentication_verified"], true);
    assert_eq!(inspected["historical_effect_acknowledged"], false);
    assert!(store.selected().is_none());
    assert_eq!(
        store.change_credential(cmd, None).await.unwrap().status,
        CredentialStatus::Unknown
    );
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    thread.join().unwrap();
}
#[tokio::test]
async fn revoke_and_sign_out_have_explicit_account_authority_and_do_not_replay() {
    let dir = directory();
    let session = json!({"session":{"id":"fixture-session","kind":"account","account":"ada","created_at":1,"expires_at":100}});
    let (origin, posts, thread) = fixture(vec![
        (200, account()),
        (200, json!({})),
        (200, account()),
        (200, session),
        (200, json!({})),
        (200, json!({"account":{"id":"grace"},"workspaces":[]})),
    ]);
    let mut store = store(dir.path());
    let mut revoke = command(&origin);
    revoke.id = "revoke-one".into();
    revoke.action = CredentialAction::Revoke {
        workspace: "ada-personal".into(),
        key: "old-key".into(),
    };
    assert_eq!(
        store
            .change_credential(revoke.clone(), None)
            .await
            .unwrap()
            .status,
        CredentialStatus::Applied
    );
    assert_eq!(
        store.change_credential(revoke, None).await.unwrap().status,
        CredentialStatus::Applied
    );
    let mut logout = command(&origin);
    logout.id = "logout-one".into();
    logout.action = CredentialAction::SignOut;
    assert_eq!(
        store
            .change_credential(logout.clone(), None)
            .await
            .unwrap()
            .status,
        CredentialStatus::Applied
    );
    assert_eq!(
        store.change_credential(logout, None).await.unwrap().status,
        CredentialStatus::Applied
    );
    let mut wrong = command(&origin);
    wrong.id = "wrong-account".into();
    assert!(store.change_credential(wrong, None).await.is_err());
    assert_eq!(posts.load(Ordering::SeqCst), 2);
    thread.join().unwrap();
}

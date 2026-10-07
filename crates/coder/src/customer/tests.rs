use super::*;
use receipts::purchase::PriceReference;
use serde_json::json;

fn private_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    dir
}

fn context(account: &str, workspace: &str, key: &str) -> Context {
    Context {
        schema: receipts::purchase::SCHEMA.into(),
        account: account.into(),
        workspace: workspace.into(),
        payer_workspace: workspace.into(),
        tenant: workspace.into(),
        credential_reference: key.into(),
        membership_epoch: 1,
        workspace_members_epoch: 1,
        role: "owner".into(),
        door: "decision-a".into(),
        registry_digest: hash('a'),
        artifact_digest: digest_request(&serde_json::to_value(receipt_artifact()).unwrap()),
        price: PriceReference {
            version: "price-1".into(),
            currency: "USD".into(),
            policy: "observed-usage-v1".into(),
            terms_digest: hash('c'),
            maximum_usage_digest: hash('d'),
            maximum_charge: 100,
        },
        can_invoke: true,
    }
}
fn hash(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}
fn receipt_artifact() -> receipts::execution::Served {
    receipts::execution::Served {
        model: "checkpoint-a".into(),
        adapter: Some("decision-adapter".into()),
        artifact_signature: hash('b'),
        execution: [("dtype".into(), "f32".into())].into(),
    }
}
fn body() -> Value {
    json!({"model":"decision-a", "state":"Private fixture content", "questions":{"ready":{"type":"noul", "instructions":"Is the task ready for review?"}}})
}
fn selection(account: &str, workspace: &str, key: &str) -> Selection {
    Selection {
        origin: "https://fixture.invalid".into(),
        credential_alias: key.into(),
        context: context(account, workspace, key),
    }
}
fn bind(store: &mut Store, selected: Selection) {
    store
        .import_credential(
            &selected.credential_alias,
            &jev::ApiKey::new(format!("oak_{}.fixture", selected.credential_alias)),
        )
        .unwrap();
    store.bind(selected).unwrap();
}
fn approved(store: &mut Store, id: &str, c: &Context) -> PurchaseView {
    let q = store.quote(id, body(), c.clone(), 100).unwrap();
    store.approve(id, &q.quote_digest, c, 110).unwrap()
}

#[test]
fn account_and_workspace_switches_do_not_retarget_historical_approvals() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let ada = selection("ada", "ada-personal", "ada-key");
    bind(&mut store, ada.clone());
    let original = approved(&mut store, "original", &ada.context);
    let bytes = serde_json::to_value(&original).unwrap();
    for other in [
        selection("grace", "grace-personal", "grace-key"),
        selection("ada", "ada-team", "ada-team-key"),
    ] {
        bind(&mut store, other.clone());
        assert!(store.show("original").is_err());
        assert!(store.history().is_empty());
        assert!(store.begin("original", &other.context, 120).is_err());
        let q = store
            .quote(&other.credential_alias, body(), other.context.clone(), 120)
            .unwrap();
        assert_eq!(q.quote.context.account, other.context.account);
        assert_eq!(q.quote.context.payer_workspace, other.context.workspace);
    }
    bind(&mut store, ada.clone());
    assert_eq!(
        serde_json::to_value(store.show("original").unwrap()).unwrap(),
        bytes
    );
    assert!(store.begin("original", &ada.context, 120).is_ok());
}

#[test]
fn rotation_preserves_attribution_and_never_moves_an_old_approval() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let old = selection("ada", "ada-personal", "old-key");
    bind(&mut store, old.clone());
    let q = approved(&mut store, "original", &old.context);
    assert!(
        store
            .import_credential("old-key", &jev::ApiKey::new("oak_different.fixture"))
            .is_err()
    );
    let new = selection("ada", "ada-personal", "new-key");
    bind(&mut store, new.clone());
    assert_eq!(store.show("original").unwrap().quote, q.quote);
    assert!(store.begin("original", &new.context, 120).is_err());
    let next = store
        .quote("next", body(), new.context.clone(), 120)
        .unwrap();
    assert_eq!(next.quote.context.credential_reference, "new-key");
    let mut revoked = new.context.clone();
    revoked.can_invoke = false;
    assert!(
        store
            .approve("next", &next.quote_digest, &revoked, 130)
            .is_err()
    );
    assert_eq!(
        store
            .show("original")
            .unwrap()
            .quote
            .context
            .credential_reference,
        "old-key"
    );
}

#[test]
fn interrupted_dispatch_recovers_unknown_liability_and_cannot_replay() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let selected = selection("ada", "ada-personal", "ada-key");
    bind(&mut store, selected.clone());
    approved(&mut store, "attempted", &selected.context);
    store.begin("attempted", &selected.context, 120).unwrap();
    let revision = store.revision();
    drop(store);
    let mut recovered = Store::open(dir.path()).unwrap();
    let view = recovered.show("attempted").unwrap();
    assert_eq!(view.status, Status::Unknown);
    assert_eq!(view.unresolved_ceiling, Some(100));
    assert!(recovered.revision() > revision);
    assert!(
        recovered
            .begin("attempted", &selected.context, 130)
            .is_err()
    );
    assert!(
        recovered
            .quote("new", body(), selected.context.clone(), 130)
            .is_err()
    );
    let mut revoked = selected.clone();
    revoked.context.can_invoke = false;
    bind(&mut recovered, revoked);
    assert_eq!(
        recovered.show("attempted").unwrap().unresolved_ceiling,
        Some(100)
    );
    let other = selection("grace", "ada-personal", "grace-key");
    bind(&mut recovered, other.clone());
    assert!(recovered.show("attempted").is_err());
    assert!(
        recovered
            .quote("same-payer", body(), other.context, 130)
            .is_err()
    );
    bind(&mut recovered, selected);
    drop(recovered);
    assert_eq!(
        Store::open(dir.path())
            .unwrap()
            .show("attempted")
            .unwrap()
            .status,
        Status::Unknown
    );
}

#[test]
fn stale_price_rights_resource_and_request_changes_refuse_before_dispatch() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let selected = selection("ada", "ada-personal", "ada-key");
    bind(&mut store, selected.clone());
    let q = store
        .quote("purchase", body(), selected.context.clone(), 100)
        .unwrap();
    assert!(
        store
            .approve("purchase", &hash('f'), &selected.context, 110)
            .is_err()
    );
    let mut changed = selected.context.clone();
    changed.price.maximum_charge += 1;
    assert!(
        store
            .approve("purchase", &q.quote_digest, &changed, 110)
            .is_err()
    );
    store
        .approve("purchase", &q.quote_digest, &selected.context, 110)
        .unwrap();
    for variant in 0..4 {
        let mut changed = selected.context.clone();
        match variant {
            0 => changed.membership_epoch += 1,
            1 => changed.can_invoke = false,
            2 => changed.price.terms_digest = hash('f'),
            _ => changed.door = "other".into(),
        };
        assert!(store.begin("purchase", &changed, 120).is_err());
    }
    assert!(
        store
            .begin("purchase", &selected.context, 100 + MAX_QUOTE_MS)
            .is_err()
    );
    assert_eq!(store.show("purchase").unwrap().status, Status::Approved);
    let mut other = selected.context.clone();
    other.door = "other".into();
    let mut request = body();
    request["model"] = json!("other");
    assert!(store.quote("other", request, other, 120).is_err());
    let mut extra = body();
    extra["payer"] = json!("other");
    assert!(store.quote("extra", extra, selected.context, 120).is_err());
}

#[test]
fn terminal_state_requires_a_valid_receipt_and_public_views_hide_private_input() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let selected = selection("ada", "ada-personal", "ada-key");
    bind(&mut store, selected.clone());
    approved(&mut store, "purchase", &selected.context);
    store.begin("purchase", &selected.context, 120).unwrap();
    let bad = ReceiptReference {
        digest: format!("sha256:{}", "z".repeat(64)),
        outcome: "answered".into(),
        settlement: "settled".into(),
    };
    assert!(store.complete("purchase", bad).is_err());
    let receipt = ReceiptReference {
        digest: hash('e'),
        outcome: "answered".into(),
        settlement: "settled".into(),
    };
    let done = store.complete("purchase", receipt).unwrap();
    assert_eq!(done.status, Status::Answered);
    assert_eq!(done.unresolved_ceiling, None);
    let view = serde_json::to_string(&done).unwrap();
    assert!(!view.contains("Private fixture content"));
    assert!(!view.contains("oak_"));
    let book = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(!book.contains("oak_"));
    let mut tampered = store.book.clone();
    tampered.purchases.get_mut("purchase").unwrap().receipt = None;
    assert!(check(&tampered).is_err());
    drop(store);
    assert_eq!(
        Store::open(dir.path())
            .unwrap()
            .show("purchase")
            .unwrap()
            .status,
        Status::Answered
    );
}

#[cfg(unix)]
#[test]
fn shared_credentials_symlinks_and_replaced_locks_are_refused() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let selected = selection("ada", "ada-personal", "ada-key");
    bind(&mut store, selected.clone());
    let key = dir.path().join("credentials/ada-key");
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.client(&selected.origin, "ada-key").is_err());
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    symlink(&key, dir.path().join("credentials/linked")).unwrap();
    assert!(store.client(&selected.origin, "linked").is_err());
    std::fs::hard_link(&key, dir.path().join("credentials/hard")).unwrap();
    assert!(store.client(&selected.origin, "ada-key").is_err());
    std::fs::remove_file(dir.path().join("credentials/hard")).unwrap();
    let lock = dir.path().join("customer.lock");
    std::fs::rename(&lock, dir.path().join("old.lock")).unwrap();
    std::fs::write(&lock, b"").unwrap();
    assert!(
        store
            .quote("purchase", body(), selected.context, 100)
            .is_err()
    );
    assert!(origin("https://user:secret@fixture.invalid").is_err());
    assert!(origin("https://fixture.invalid/elsewhere").is_err());
    assert!(origin("http://fixture.invalid").is_err());
}

#[test]
fn interrupted_temporary_write_never_supersedes_committed_history() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let selected = selection("ada", "ada-personal", "ada-key");
    bind(&mut store, selected.clone());
    approved(&mut store, "purchase", &selected.context);
    let mut pending = store.book.clone();
    pending.selected = Some(selection("grace", "grace-personal", "grace-key"));
    let temporary = dir.path().join(".state.json.tmp");
    let mut file = task::private_open(&temporary, true, true).unwrap();
    file.write_all(&serde_json::to_vec(&pending).unwrap())
        .unwrap();
    file.sync_all().unwrap();
    drop(file);
    drop(store);
    let recovered = Store::open(dir.path()).unwrap();
    assert_eq!(recovered.selected().unwrap(), &selected);
    assert_eq!(recovered.show("purchase").unwrap().status, Status::Approved);
    assert!(!temporary.exists());
    drop(recovered);
    let mut file = task::private_open(&temporary, true, true).unwrap();
    file.write_all(b"malformed").unwrap();
    drop(file);
    assert!(Store::open(dir.path()).is_err());
    assert!(temporary.exists());
}

#[tokio::test]
async fn server_selection_and_approval_refresh_use_the_exact_credential_without_redirects() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let c = context("ada", "ada-personal", "ada-key");
    let mut revoked = c.clone();
    revoked.can_invoke = false;
    let server = tokio::spawn(async move {
        for context in [Some(c.clone()), Some(c), Some(revoked), None] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut bytes = [0; 2048];
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&bytes[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8(request).unwrap();
            assert!(
                text.starts_with("GET /v1/workspaces/ada-personal/purchase-context/decision-a ")
            );
            assert!(
                text.to_ascii_lowercase()
                    .contains("authorization: bearer oak_ada-key.fixture")
            );
            let response = if let Some(context) = context {
                let body = serde_json::to_string(&context).unwrap();
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
            } else {
                "HTTP/1.1 302 Found\r\nLocation: https://other.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()
            };
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store
        .import_credential("ada-key", &jev::ApiKey::new("oak_ada-key.fixture"))
        .unwrap();
    let selected = store
        .select(&endpoint, "ada-key", "ada-personal", "decision-a")
        .await
        .unwrap();
    let q = store.create_quote("purchase", body(), 100).await.unwrap();
    assert_eq!(q.quote.context, selected.context);
    assert!(
        store
            .approve_quote("purchase", &q.quote_digest, 110)
            .await
            .is_err()
    );
    assert_eq!(store.show("purchase").unwrap().status, Status::Quoted);
    let error = store
        .select(&endpoint, "ada-key", "ada-personal", "decision-a")
        .await
        .unwrap_err();
    assert!(!error.contains("oak_"));
    assert_eq!(store.selected().unwrap(), &selected);
    server.await.unwrap();
}

fn proof(
    quote: &Quote,
    outcome: receipts::execution::Outcome,
    phase: &str,
) -> jev::PurchaseReceipt {
    let mut receipt = receipts::execution::ExecutionReceipt::for_attempt(
        "http",
        &quote.id,
        1,
        &quote.request_digest,
    );
    receipt.attempt_id = "fixture-attempt".into();
    receipt.tenant = Some(quote.context.credential_reference.clone());
    receipt.workspace = Some(quote.context.workspace.clone());
    receipt.registry = Some(receipts::execution::Registry {
        digest: quote.context.registry_digest.clone(),
        sequence: 1,
    });
    receipt.requested = receipt_artifact();
    receipt.outcome = outcome;
    receipt.seal();
    jev::PurchaseReceipt {
        receipt,
        cost: Some(jev::PurchaseCost {
            reserved: quote.context.price.maximum_charge,
            retail: (phase == "settled").then_some(20),
            phase: phase.into(),
            price_version: quote.context.price.version.clone(),
        }),
    }
}

#[test]
fn only_the_original_verified_receipt_and_money_position_resolve_liability() {
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let selected = selection("ada", "ada-personal", "old-key");
    bind(&mut store, selected.clone());
    let q = approved(&mut store, "purchase", &selected.context);
    store.begin("purchase", &selected.context, 120).unwrap();
    store.uncertain("purchase").unwrap();
    for variant in 0..14 {
        let mut p = proof(&q.quote, receipts::execution::Outcome::Answered, "settled");
        match variant {
            0 => p.receipt.request = "other".into(),
            1 => p.receipt.workspace = Some("other".into()),
            2 => p.receipt.tenant = Some("new-key".into()),
            3 => p.receipt.request_digest = hash('f'),
            4 => p.receipt.registry.as_mut().unwrap().digest = hash('f'),
            5 => p.receipt.attempt = 2,
            6 => p.cost.as_mut().unwrap().reserved += 1,
            7 => p.cost.as_mut().unwrap().price_version = "other".into(),
            8 => p.cost.as_mut().unwrap().retail = Some(101),
            9 => p.cost = None,
            10 => p.receipt.requested.model = q.quote.context.door.clone(),
            11 => p.receipt.requested.adapter = None,
            12 => p.receipt.requested.artifact_signature = hash('f'),
            _ => {
                p.receipt
                    .requested
                    .execution
                    .insert("dtype".into(), "f16".into());
            }
        };
        p.receipt.seal();
        assert!(store.record_proof("purchase", p).is_err());
        assert_eq!(
            store.show("purchase").unwrap().unresolved_ceiling,
            Some(100)
        );
    }
    let held = proof(&q.quote, receipts::execution::Outcome::Answered, "unknown");
    store.record_proof("purchase", held).unwrap();
    assert_eq!(store.show("purchase").unwrap().status, Status::Unknown);
    let mut corrupted = proof(&q.quote, receipts::execution::Outcome::Answered, "settled");
    corrupted.receipt.request = "changed-without-sealing".into();
    assert!(store.record_proof("purchase", corrupted).is_err());
    bind(&mut store, selection("ada", "ada-personal", "new-key"));
    assert_eq!(
        store
            .show("purchase")
            .unwrap()
            .quote
            .context
            .credential_reference,
        "old-key"
    );
    let p = proof(&q.quote, receipts::execution::Outcome::Answered, "settled");
    let view = store.record_proof("purchase", p).unwrap();
    assert_eq!(view.status, Status::Answered);
    assert_eq!(view.unresolved_ceiling, None);
    assert_eq!(view.quote, q.quote);
}

#[tokio::test]
async fn approved_invocation_dispatches_once_and_resolves_through_authenticated_receipt() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let c = context("ada", "ada-personal", "ada-key");
    let quote = Quote {
        id: "purchase".into(),
        context: c.clone(),
        request_digest: digest_request(&body()),
        created_at_ms: now,
        expires_at_ms: now + MAX_QUOTE_MS,
    };
    let approval = Approval {
        quote: quote.clone(),
        approved_at_ms: now,
    };
    let p = proof(&quote, receipts::execution::Outcome::Answered, "settled");
    let digest = p.receipt.digest.clone();
    let server = tokio::spawn(async move {
        let mut dispatches = 0;
        for index in 0..6 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut incoming = Vec::new();
            let mut bytes = [0; 4096];
            let header_end;
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0);
                incoming.extend_from_slice(&bytes[..n]);
                if let Some(pos) = incoming.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = pos + 4;
                    break;
                }
            }
            let headers = String::from_utf8(incoming[..header_end].to_vec()).unwrap();
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer oak_ada-key.fixture")
            );
            let (response, extra) = if index < 4 {
                assert!(
                    headers.starts_with(
                        "GET /v1/workspaces/ada-personal/purchase-context/decision-a "
                    )
                );
                (serde_json::to_string(&c).unwrap(), String::new())
            } else if index == 4 {
                dispatches += 1;
                assert!(headers.starts_with("POST /v1/systemone "));
                assert!(
                    headers
                        .to_ascii_lowercase()
                        .contains("idempotency-key: purchase")
                );
                let encoded = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("x-openagents-purchase: "))
                    .unwrap();
                let sent: Approval = serde_json::from_str(encoded).unwrap();
                assert_eq!(sent, approval);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                while incoming.len() < header_end + length {
                    let n = socket.read(&mut bytes).await.unwrap();
                    assert!(n > 0);
                    incoming.extend_from_slice(&bytes[..n]);
                }
                assert_eq!(
                    serde_json::from_slice::<Value>(&incoming[header_end..header_end + length])
                        .unwrap(),
                    body()
                );
                (json!({"model":"decision-a","answers":{"ready":{"type":"noul","noul":0.7}},"usage":{"input_tokens":10,"output_tokens":1}}).to_string(),format!("x-openagents-purchase: {}\r\nx-receipt: {}\r\n",approval.digest(),digest))
            } else {
                assert!(headers.starts_with(&format!(
                    "GET /v1/workspaces/ada-personal/usage/receipts/{digest} "
                )));
                (
                    json!({"receipt":p.receipt,"cost":p.cost}).to_string(),
                    String::new(),
                )
            };
            let wire = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{response}",
                response.len()
            );
            socket.write_all(wire.as_bytes()).await.unwrap();
        }
        dispatches
    });
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    store
        .import_credential("ada-key", &jev::ApiKey::new("oak_ada-key.fixture"))
        .unwrap();
    store
        .select(&endpoint, "ada-key", "ada-personal", "decision-a")
        .await
        .unwrap();
    let q = store.create_quote("purchase", body(), now).await.unwrap();
    assert_eq!(q.quote, quote);
    store
        .approve_quote("purchase", &q.quote_digest, now)
        .await
        .unwrap();
    let (view, response) = store.invoke("purchase", now).await.unwrap();
    assert_eq!(view.status, Status::Answered);
    assert_eq!(response.model, "decision-a");
    assert_eq!(server.await.unwrap(), 1);
    assert!(store.begin("purchase", &q.quote.context, now).is_err());
    drop(store);
    assert_eq!(
        Store::open(dir.path())
            .unwrap()
            .show("purchase")
            .unwrap()
            .status,
        Status::Answered
    );
}

#[tokio::test]
async fn rotated_membership_can_reconcile_the_original_key_without_redispatch() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    let mut original = selection("ada", "ada-personal", "old-key");
    original.origin = endpoint.clone();
    original.context.credential_reference = "session:fixture".into();
    bind(&mut store, original.clone());
    let q = approved(&mut store, "purchase", &original.context);
    store.begin("purchase", &original.context, 120).unwrap();
    drop(store);
    let mut store = Store::open(dir.path()).unwrap();
    let mut rotated = selection("ada", "ada-personal", "new-key");
    rotated.origin = endpoint;
    bind(&mut store, rotated);
    let p = proof(&q.quote, receipts::execution::Outcome::Answered, "settled");
    let digest = p.receipt.digest.clone();
    let page=json!({"workspace":"ada-personal","items":[{"digest":digest,"request":"purchase","attempt":1}],"cursor":null}).to_string();
    let receipt = json!({"receipt":p.receipt,"cost":p.cost}).to_string();
    let server = tokio::spawn(async move {
        for (index, response) in [page, receipt].into_iter().enumerate() {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut incoming = Vec::new();
            let mut bytes = [0; 2048];
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0);
                incoming.extend_from_slice(&bytes[..n]);
                if incoming.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let headers = String::from_utf8(incoming).unwrap();
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer oak_new-key.fixture")
            );
            if index == 0 {
                assert!(headers.starts_with(
                    "GET /v1/workspaces/ada-personal/usage/activity?limit=10&key=session%3Afixture "
                ));
            } else {
                assert!(headers.starts_with(&format!(
                    "GET /v1/workspaces/ada-personal/usage/receipts/{digest} "
                )));
            }
            let wire = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            );
            socket.write_all(wire.as_bytes()).await.unwrap();
        }
    });
    let resolved = store.reconcile("purchase", None).await.unwrap();
    assert_eq!(resolved.status, Status::Answered);
    assert_eq!(resolved.quote, q.quote);
    assert_eq!(
        resolved.quote.context.credential_reference,
        "session:fixture"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn a_failed_purchase_response_keeps_liability_and_does_not_retry() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut selected = selection("ada", "ada-personal", "ada-key");
    selected.origin = format!("http://{}", listener.local_addr().unwrap());
    let context = selected.context.clone();
    let server = tokio::spawn(async move {
        for index in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 8192];
            let mut incoming = Vec::new();
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0);
                incoming.extend_from_slice(&bytes[..n]);
                if incoming.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    break;
                }
            }
            let request = String::from_utf8_lossy(&incoming);
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer oak_ada-key.fixture")
            );
            let (status, body) =
                if index == 0 {
                    assert!(request.starts_with(
                        "GET /v1/workspaces/ada-personal/purchase-context/decision-a "
                    ));
                    ("200 OK", serde_json::to_string(&context).unwrap())
                } else {
                    assert!(request.starts_with("POST /v1/systemone "));
                    assert!(request.to_ascii_lowercase().contains("x-attempt: 1"));
                    (
                    "503 Service Unavailable",
                    json!({"error":{"code":"overloaded","message":"oak_echoed-fixture-secret"}})
                        .to_string(),
                )
                };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
    });
    let dir = private_dir();
    let mut store = Store::open(dir.path()).unwrap();
    bind(&mut store, selected.clone());
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let q = store
        .quote("purchase", body(), selected.context.clone(), now)
        .unwrap();
    store
        .approve("purchase", &q.quote_digest, &selected.context, now)
        .unwrap();
    let error = store.invoke("purchase", now).await.err().unwrap();
    assert!(!error.contains("oak_echoed"));
    let view = store.show("purchase").unwrap();
    assert_eq!(view.status, Status::Unknown);
    assert_eq!(view.unresolved_ceiling, Some(100));
    assert!(store.begin("purchase", &selected.context, now).is_err());
    server.await.unwrap();
}

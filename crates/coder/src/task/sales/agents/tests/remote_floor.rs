//! WEB-15: owner-only floor supervision and exact outbox decisions through
//! the remote adapter, against the canonical sales books with synthetic
//! people and a fixture mailbox. Nothing here sends mail.
use super::*;
use crate::task::sales::remote::{self, Service};
use crate::task::sales::{email::Delivery, outbox};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;

const SITE: &str = "synthetic-site-bearer-for-owner-floor";
const RECIPIENT: &str = "private-buyer@fixture.invalid";
const BODY: &str = "Here is the requested private pilot scope.";

fn private_file(path: &std::path::Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn bind(dir: &std::path::Path, effects: Value, supervise: bool, mailbox: bool) -> PathBuf {
    let config = dir.join("remote.json");
    let mut binding = json!({"id":"owner-floor","account":"alice","workspace":"alice-sales",
        "members_epoch":7,"principal":"operator","credential":dir.join("owner"),
        "client_digest":digest(SITE.as_bytes()),"effects":effects,"supervise":supervise});
    if mailbox {
        binding["mailbox_key"] = json!(dir.join("mailbox-fixture-key"));
    }
    private_file(
        &config,
        &serde_json::to_vec(
            &json!({"schema":remote::CONFIG_SCHEMA,"root":dir.join("host"),
            "journal":dir.join("remote-journal"),"bindings":[binding]}),
        )
        .unwrap(),
    );
    config
}

fn call(service: &Service, op: Value) -> remote::Reply {
    let body = json!({"schema":remote::REQUEST_SCHEMA,
        "actor":{"account":"alice","workspace":"alice-sales","members_epoch":7},"op":op});
    let reply = service.call("owner-floor", SITE, body.to_string().as_bytes());
    if op["kind"] != "proposal" {
        let text = reply.body.to_string();
        assert!(!text.contains(RECIPIENT), "contact left the owner host");
        assert!(!text.contains(BODY), "message body left the owner host");
    }
    reply
}

fn decide(request: &str, revision: u64, proposal: &str, subject: &str, approve: bool) -> String {
    serde_json::to_string(&outbox::Command {
        schema: outbox::COMMAND_SCHEMA.into(),
        id: request.into(),
        expected_revision: revision,
        operation: outbox::Operation::Decide {
            proposal: proposal.into(),
            subject_sha256: subject.into(),
            approve,
        },
    })
    .unwrap()
}

fn outbox_op(request: &str, command: String) -> Value {
    json!({"kind":"outbox","request":request,"command":command})
}

fn error(reply: &remote::Reply) -> &str {
    reply.body["error"].as_str().unwrap_or("none")
}

fn release(f: &mut Fixture) {
    let placeholder = Store::open_with_clock(&f.dir.path().join("placeholder"), now).unwrap();
    drop(std::mem::replace(&mut f.store, placeholder));
}

fn resume(f: &mut Fixture) {
    f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
    f.owner = f
        .store
        .authenticate(&Store::read_credential(&f.dir.path().join("owner")).unwrap())
        .unwrap();
}

#[test]
fn remote_floor_supervision_decides_only_exact_subjects_and_stop_fences_dispatch() {
    let mut f = Fixture::new();
    let dir = f.dir.path().canonicalize().unwrap();
    let (_, keys, message) = email_fixture(&mut f);
    let binding = super::super::super::paul::Binding {
        schema: super::super::super::paul::SCHEMA.into(),
        revision: 1,
        anchor: f.anchor.clone(),
        owner_credential: dir.join("owner"),
        assignments: vec![],
        permitted_requesters: vec!["owner".into()],
    };
    f.store
        .configure_paul(&f.owner, &binding, &binding.sha256().unwrap())
        .unwrap();
    let original = f
        .store
        .propose_sales_outbox(
            &f.owner,
            outbox_proposal(message.clone(), "original"),
            &keys,
        )
        .unwrap();
    let original_sha = original.sha256().unwrap();
    release(&mut f);

    // Without the supervision grant, nothing on the floor is readable.
    let config = bind(&dir, json!(["outbox_decide", "outbox_stop"]), false, true);
    let service = Service::open_with_clock(&config, now).unwrap();
    for op in [
        json!({"kind":"floor"}),
        json!({"kind":"board"}),
        json!({"kind":"proposal","proposal":"original"}),
    ] {
        assert_eq!(error(&call(&service, op)), "access_denied");
    }
    let config = bind(&dir, json!(["outbox_decide", "outbox_stop"]), true, true);
    let service = Service::open_with_clock(&config, now).unwrap();
    let standing = call(&service, json!({"kind":"standing"}));
    assert_eq!(
        standing.body["result"]["supervise"], true,
        "{}",
        standing.body
    );

    // The floor projection is aggregate and contact-free.
    let floor = call(&service, json!({"kind":"floor"}));
    assert_eq!(floor.status, 200, "{}", floor.body);
    let floor: remote::Floor = serde_json::from_value(floor.body["result"].clone()).unwrap();
    assert_eq!(floor.timezone, "America/Chicago");
    assert_eq!(floor.ceiling_usd_millionths, 5_000_000);
    assert!(floor.paul.is_some());
    assert_eq!(floor.crew[0].name, "paul");
    assert!(!floor.outbox.outbound_authority);
    let row = &floor.outbox.rows[0];
    assert_eq!(
        (row.id.as_str(), row.phase, row.reviewable),
        ("original", outbox::Phase::Proposed, true)
    );
    let revision = floor.outbox.revision;

    // The private board is counts only and current for three seconds.
    let board = call(&service, json!({"kind":"board"}));
    assert_eq!(board.status, 200, "{}", board.body);
    let board: remote::Board = serde_json::from_value(board.body["result"].clone()).unwrap();
    assert_eq!(
        board.expires_at,
        board.observed_at + remote::BOARD_TTL_SECONDS
    );
    assert_eq!(board.outbox_fixture_proposals, 1);
    assert!(board.shared.is_none());

    // The exact subject is reviewable as it would leave.
    let shown = call(&service, json!({"kind":"proposal","proposal":"original"}));
    assert_eq!(shown.status, 200, "{}", shown.body);
    let shown: remote::Proposal = serde_json::from_value(shown.body["result"].clone()).unwrap();
    assert_eq!(shown.subject_sha256, original_sha);
    assert_eq!(shown.recipient, RECIPIENT);
    assert_eq!(shown.body, BODY);
    assert_eq!(shown.outbox_revision, revision);
    assert_eq!(
        error(&call(
            &service,
            json!({"kind":"proposal","proposal":"absent"})
        )),
        "access_denied"
    );

    // A different subject digest or an old revision applies nothing.
    let wrong = call(
        &service,
        outbox_op(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            decide(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                revision,
                "original",
                &"f".repeat(64),
                true,
            ),
        ),
    );
    assert_eq!(error(&wrong), "refused");
    let stale = call(
        &service,
        outbox_op(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            decide(
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                revision + 1,
                "original",
                &original_sha,
                true,
            ),
        ),
    );
    assert_eq!(error(&stale), "stale");
    // No other outbox operation is reachable, even with both effects.
    let pause = serde_json::to_string(&outbox::Command {
        schema: outbox::COMMAND_SCHEMA.into(),
        id: "cccccccccccccccccccccccccccccccc".into(),
        expected_revision: revision,
        operation: outbox::Operation::Pause {
            incident: outbox::IncidentKind::Complaint,
            reference_sha256: "a".repeat(64),
        },
    })
    .unwrap();
    assert_eq!(
        error(&call(
            &service,
            outbox_op("cccccccccccccccccccccccccccccccc", pause)
        )),
        "access_denied"
    );

    // The exact approval applies once; a retry returns the same receipt and
    // changed bytes under the same identity conflict.
    let request = "dddddddddddddddddddddddddddddddd";
    let exact = decide(request, revision, "original", &original_sha, true);
    let approved = call(&service, outbox_op(request, exact.clone()));
    assert_eq!(approved.status, 200, "{}", approved.body);
    assert_eq!(approved.body["result"]["outcome"], "outbox_approved");
    let again = call(&service, outbox_op(request, exact.clone()));
    assert_eq!(again.body["result"], approved.body["result"]);
    let changed = call(
        &service,
        outbox_op(
            request,
            decide(request, revision, "original", &original_sha, false),
        ),
    );
    assert_eq!(error(&changed), "conflict");
    let receipt_digest = approved.body["result"]["command_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let settled = call(
        &service,
        json!({"kind":"reconcile","request":request,"digest":receipt_digest}),
    );
    assert_eq!(
        settled.body["result"]["state"], "recorded",
        "{}",
        settled.body
    );

    // The fixture handoff leaves delivery unknown; a second subject is approved.
    resume(&mut f);
    let mut transport = outbox_transport(&original, Delivery::Unknown);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let seen = f
        .store
        .dispatch_sales_outbox_fixture(
            &f.owner,
            "original",
            &original_sha,
            &keys,
            &mut transport,
            &cancel,
        )
        .unwrap();
    assert_eq!(seen.phase, outbox::Phase::Unknown);
    let second = f
        .store
        .propose_sales_outbox(&f.owner, outbox_proposal(message, "second"), &keys)
        .unwrap();
    let second_sha = second.sha256().unwrap();
    let revision = f.store.state.outbox.revision;
    release(&mut f);
    let request = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let approved = call(
        &service,
        outbox_op(
            request,
            decide(request, revision, "second", &second_sha, true),
        ),
    );
    assert_eq!(approved.status, 200, "{}", approved.body);

    // Stop fences the approved subject before any handoff and keeps the
    // original unknown delivery unknown.
    let stop = |request: &str, revision: u64| {
        serde_json::to_string(&outbox::Command {
            schema: outbox::COMMAND_SCHEMA.into(),
            id: request.into(),
            expected_revision: revision,
            operation: outbox::Operation::Pause {
                incident: outbox::IncidentKind::OwnerStop,
                reference_sha256: "b".repeat(64),
            },
        })
        .unwrap()
    };
    let request = "ffffffffffffffffffffffffffffffff";
    let stopped = call(&service, outbox_op(request, stop(request, revision + 1)));
    assert_eq!(stopped.status, 200, "{}", stopped.body);
    assert_eq!(stopped.body["result"]["outcome"], "outbox_stopped");
    resume(&mut f);
    let mut transport = outbox_transport(&second, Delivery::Delivered);
    assert!(
        f.store
            .dispatch_sales_outbox_fixture(
                &f.owner,
                "second",
                &second_sha,
                &keys,
                &mut transport,
                &cancel,
            )
            .is_err()
    );
    assert_eq!(transport.calls, 0);
    release(&mut f);
    let floor = call(&service, json!({"kind":"floor"}));
    let floor: remote::Floor = serde_json::from_value(floor.body["result"].clone()).unwrap();
    assert!(floor.outbox.paused);
    let phase = |id: &str| {
        floor
            .outbox
            .rows
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.phase)
    };
    assert_eq!(phase("original"), Some(outbox::Phase::Unknown));
    // The fenced approval was never handed off; it needs a new proposal.
    assert_eq!(phase("second"), Some(outbox::Phase::Invalidated));
    assert!(floor.escalations.iter().any(|e| e.kind == "owner_stop"));

    // Without the stop effect, the binding cannot stop.
    let config = bind(&dir, json!(["outbox_decide"]), true, false);
    let service = Service::open_with_clock(&config, now).unwrap();
    let request = "abababababababababababababababab";
    assert_eq!(
        error(&call(
            &service,
            outbox_op(request, stop(request, revision + 2))
        )),
        "access_denied"
    );
}

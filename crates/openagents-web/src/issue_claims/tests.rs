use super::*;
use openagents_sdk::{runs::RunKind, sessions::SessionStatus};

#[cfg(unix)]
#[path = "http_tests.rs"]
mod http;

fn beat(id: &str, native: &str) -> RunHeartbeat {
    RunHeartbeat {
        run: id.into(),
        kind: RunKind::Terminal,
        harness: "any-harness".into(),
        harness_session: Some(native.into()),
        computer: "scratch".into(),
        issues: vec![],
        status: SessionStatus::Working,
        last_output_at: 1000,
        last_progress_at: Some(900),
        note: None,
        agent: None,
    }
}
fn request(holder: &str, batch: &[u64]) -> ClaimRequest {
    ClaimRequest {
        holder: holder.into(),
        batch: batch.into(),
        ..ClaimRequest::default()
    }
}
fn presence(beat: RunHeartbeat, received_at: i64) -> crate::runs::RunPresence {
    crate::runs::RunPresence {
        beat,
        received_at,
        ended_at: None,
    }
}

#[test]
fn aliases_and_replacement_runs_share_one_batch_and_force_does_not_expand_it() {
    let a = beat("chat-a", "native-a");
    let b = beat("chat-b", "native-a");
    let live = [a.run.clone(), b.run.clone()].into();
    let mut book = Book::default();
    let mut r = request("chat:chat-a", &[]);
    book.reserve("owner/repo", 1, &r, &a, &live, &[], None, 1000, "a")
        .unwrap();
    r.holder = "any-harness:native-a".into();
    r.force = true;
    let err = book
        .reserve("owner/repo", 2, &r, &b, &live, &[], None, 1000, "b")
        .unwrap_err();
    assert!(err.message.contains("owner/repo#1"));
    r.batch = vec![1, 2, 3];
    book.reserve("owner/repo", 2, &r, &b, &live, &[], None, 1000, "b")
        .unwrap();
    r.batch.clear();
    book.reserve("owner/repo", 3, &r, &b, &live, &[], None, 1000, "c")
        .unwrap();
    assert!(
        book.reserve("owner/repo", 4, &r, &b, &live, &[], None, 1000, "d")
            .is_err()
    );
    assert!(
        book.reserve("other/repo", 3, &r, &b, &live, &[], None, 1000, "d")
            .is_err()
    );
    // Ended runs no longer reserve this native session's capacity.
    book.reserve(
        "owner/repo",
        4,
        &r,
        &b,
        &BTreeSet::new(),
        &[],
        None,
        2000,
        "d",
    )
    .unwrap();
}

#[test]
fn in_flight_claims_conflict_and_published_retries_keep_the_original_time() {
    let a = beat("chat-a", "native-a");
    let live = [a.run.clone()].into();
    let r = request(&a.run, &[]);
    let mut book = Book::default();
    let held = book
        .reserve("owner/repo", 1, &r, &a, &live, &[], None, 1000, "a")
        .unwrap();
    assert!(
        book.reserve("owner/repo", 1, &r, &a, &live, &[], None, 1001, "b")
            .is_err()
    );
    book.entries.get_mut("owner/repo#1").unwrap().pending = false;
    let retry = book
        .reserve(
            "owner/repo",
            1,
            &r,
            &a,
            &live,
            &[],
            Some(&held.claim),
            2000,
            "b",
        )
        .unwrap();
    assert_eq!(retry.claim, held.claim);
    assert!(!retry.pending);
    // A crashed attempt expires, and another chat can take the issue.
    book.entries.get_mut("owner/repo#1").unwrap().pending = true;
    let b = beat("chat-b", "native-b");
    let next = book
        .reserve(
            "owner/repo",
            1,
            &request(&b.run, &[]),
            &b,
            &[b.run.clone()].into(),
            &[],
            None,
            1000 + PENDING_MS,
            "c",
        )
        .unwrap();
    assert_eq!(next.session, session(&b));
}

#[test]
fn joining_runs_uses_account_rows_and_output_instead_of_heartbeat_receipt_time() {
    let a = beat("chat-a", "native-a");
    let rows = vec![presence(a.clone(), 50_000)];
    let claim = Claim {
        repo: "owner/repo".into(),
        issue: 1,
        holder: "any-harness:native-a".into(),
        run: None,
        claimed_at: 500,
        last_progress_at: Some(800),
    };
    let joined = join(claim.clone(), &rows);
    assert_eq!(joined.run.as_deref(), Some("chat-a"));
    assert_eq!(joined.last_progress_at, Some(1000));
    assert_eq!(join(claim, &[]).run, None);
    assert!(choose(&request("chat:chat-a", &[]), &rows, 50_000).is_ok());
    assert!(
        choose(
            &request("chat:chat-a", &[]),
            &rows,
            50_000 + openagents_sdk::runs::LIVE_WINDOW_MS
        )
        .is_err()
    );
    let mut duplicate = a;
    duplicate.run = "chat-b".into();
    let rows = vec![rows[0].clone(), presence(duplicate, 50_000)];
    let mut r = request("any-harness:native-a", &[]);
    assert!(choose(&r, &rows, 50_000).is_err());
    r.run = Some("chat-b".into());
    assert_eq!(choose(&r, &rows, 50_000).unwrap().beat.run, "chat-b");
}

#[tokio::test]
async fn concurrent_claims_are_serialized_and_account_books_survive_restart() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().to_path_buf());
    let a = beat("chat-a", "native-a");
    let r = request(&a.run, &[]);
    let live = [a.run.clone()].into();
    let owner = crate::chat_store::account_owner("claims-alice");
    let other = crate::chat_store::account_owner("claims-bob");
    let (one, two) = tokio::join!(
        change(&store, &owner, |b| b.reserve(
            "owner/repo",
            1,
            &r,
            &a,
            &live,
            &[],
            None,
            1000,
            "a"
        )),
        change(&store, &owner, |b| b.reserve(
            "owner/repo",
            2,
            &r,
            &a,
            &live,
            &[],
            None,
            1000,
            "b"
        ))
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let restart = Store::local(root.path().to_path_buf());
    assert_eq!(load(&restart, &owner).await.unwrap().0.entries.len(), 1);
    assert!(load(&restart, &other).await.unwrap().0.entries.is_empty());
}

#[test]
fn github_claims_keep_cli_markers_batches_and_release_semantics() {
    let claim = coder::claim::Comment {
        at: 1000,
        body: format!(
            "Claimed.\n{}",
            coder::claim::marker("sdk run=chat-a batch=1,2", "chat:chat-a")
        ),
    };
    let mut comments = vec![claim];
    let row = github::observation("owner/repo", 1, &comments).unwrap();
    assert_eq!(row.claim.holder, "chat:chat-a");
    assert_eq!(row.claim.run.as_deref(), Some("chat-a"));
    assert_eq!(row.batch, vec![1, 2]);
    comments.push(coder::claim::Comment {
        at: 1001,
        body: coder::claim::RELEASE_MARK.into(),
    });
    assert!(github::observation("owner/repo", 1, &comments).is_none());
    comments = vec![coder::claim::Comment {
        at: 1000,
        body: "Claimed: working on this.".into(),
    }];
    assert_eq!(
        github::observation("owner/repo", 1, &comments)
            .unwrap()
            .claim
            .holder,
        "an agent"
    );
    comments[0].body = format!(
        "{} session={} -->",
        coder::claim::CLAIM_MARK,
        "a".repeat(1000)
    );
    assert_eq!(
        github::observation("owner/repo", 1, &comments)
            .unwrap()
            .claim
            .holder
            .len(),
        128
    );
}

#[test]
fn an_existing_cli_marker_reserves_the_same_live_sessions_capacity() {
    let a = beat("chat-a", "native-a");
    let live = [a.run.clone()].into();
    let old = Held {
        claim: Claim {
            repo: "owner/repo".into(),
            issue: 1,
            holder: "any-harness:native-a".into(),
            run: Some(a.run.clone()),
            claimed_at: 1000,
            last_progress_at: Some(1000),
        },
        session: session(&a),
        batch: vec![],
        pending: false,
        intent: String::new(),
    };
    let mut book = Book::default();
    assert!(
        book.reserve(
            "owner/repo",
            2,
            &request(&a.run, &[]),
            &a,
            &live,
            std::slice::from_ref(&old),
            None,
            2000,
            "a"
        )
        .is_err()
    );
    book.reserve(
        "owner/repo",
        2,
        &request(&a.run, &[1, 2]),
        &a,
        &live,
        &[old],
        None,
        2000,
        "a",
    )
    .unwrap();
}

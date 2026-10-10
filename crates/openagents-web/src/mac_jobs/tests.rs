use super::*;
use crate::chat_store::account_owner;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::local(dir.path().to_path_buf());
    (dir, store)
}

fn mac(asc_key: bool) -> Capabilities {
    Capabilities {
        macos: Some("26.4 (25E246)".into()),
        xcode: Some("26.6 (17F113)".into()),
        signing_identities: vec!["Apple Distribution: OpenAgents, Inc. (T)".into()],
        simulators: vec![mac_jobs::Simulator {
            name: "iPhone 17 Pro".into(),
            runtime: "iOS 26.5".into(),
            udid: "U".into(),
            booted: false,
        }],
        asc_key,
        free_disk_gb: Some(100),
        recipes: Recipe::ALL.to_vec(),
        busy: false,
    }
}

fn gate() -> Submit {
    Submit {
        repo: "OpenAgentsInc/openagents".into(),
        git_ref: "main".into(),
        recipe: Recipe::IosReleaseGate,
        args: Vec::new(),
        computer: None,
    }
}

#[tokio::test]
async fn a_job_goes_from_the_environment_to_the_mac_and_back() {
    let (_dir, store) = store();
    let owner = account_owner("acct_owner");
    // No Mac yet: nothing to send to.
    assert!(matches!(
        submit(&store, &owner, gate()).await.unwrap(),
        Err(Refused::NoMac(_))
    ));
    report_capabilities(&store, &owner, "Studio", mac(false))
        .await
        .unwrap();
    let queued = submit(&store, &owner, gate()).await.unwrap().unwrap();
    assert_eq!(queued.computer, "Studio");
    assert_eq!(queued.kind, Kind::Test);
    assert!(!queued.approval && queued.online);
    // Another Mac takes nothing; this one takes it once.
    assert!(take(&store, &owner, "Laptop").await.unwrap().is_empty());
    let taken = take(&store, &owner, "Studio").await.unwrap();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].spec.recipe, Recipe::IosReleaseGate);
    assert!(take(&store, &owner, "Studio").await.unwrap().is_empty());
    let heard = report(
        &store,
        &owner,
        "Studio",
        &queued.id,
        Report {
            lines: vec![
                "Building OpenAgents".into(),
                "key sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789".into(),
            ],
            commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
            ..Report::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(heard, Heard::default());
    // A report on a job keeps the Mac online, busy.
    touch_mac(&store, &owner, "Studio").await.unwrap();
    assert!(macs(&store, &owner).await.unwrap()[0].2);
    // A file, in two parts.
    for (part, last, bytes) in [(0, false, b"PNG1".to_vec()), (1, true, b"PNG2".to_vec())] {
        let saved = save_part(
            &store,
            &owner,
            "Studio",
            &queued.id,
            "shots-01.png",
            part,
            last,
            bytes,
        )
        .await
        .unwrap();
        assert_eq!(saved, PartSaved::Saved);
    }
    // Parts out of order and bad names are refused.
    assert!(matches!(
        save_part(
            &store,
            &owner,
            "Studio",
            &queued.id,
            "log.txt",
            3,
            true,
            b"x".to_vec()
        )
        .await
        .unwrap(),
        PartSaved::Refused(_)
    ));
    assert!(matches!(
        save_part(
            &store,
            &owner,
            "Studio",
            &queued.id,
            "../x",
            0,
            true,
            b"x".to_vec()
        )
        .await
        .unwrap(),
        PartSaved::Refused(_)
    ));
    report(
        &store,
        &owner,
        "Studio",
        &queued.id,
        Report {
            done: Some(Done {
                summary: "Passed: 1 of 1 tests passed.".into(),
            }),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let job = load(&store, &owner, &queued.id).await.unwrap().unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(job.lines[0], "Building OpenAgents");
    assert!(
        !job.lines[1].contains("abcdefghijklmnop"),
        "{}",
        job.lines[1]
    );
    let shown = view(&job, 1);
    assert_eq!(shown["lines"].as_array().unwrap().len(), 1);
    assert_eq!(shown["next"], 2);
    assert_eq!(shown["artifacts"][0]["name"], "shots-01.png");
    assert_eq!(shown["artifacts"][0]["size"], 8);
    let (size, body) = artifact_body(&store, &owner, &queued.id, "shots-01.png")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(size, 8);
    let bytes = axum::body::to_bytes(body, 1024).await.unwrap();
    assert_eq!(&bytes[..], b"PNG1PNG2");
    // Another Mac can't report on it; another account sees none of it.
    assert_eq!(
        report(&store, &owner, "Laptop", &queued.id, Report::default())
            .await
            .unwrap(),
        None
    );
    let other = account_owner("acct_other");
    assert!(load(&store, &other, &queued.id).await.unwrap().is_none());
    assert!(list(&store, &other).await.unwrap().is_empty());
    assert!(macs(&store, &other).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_upload_waits_for_the_owner_and_the_answer_reaches_the_mac_once() {
    let (_dir, store) = store();
    let owner = account_owner("acct_owner");
    report_capabilities(&store, &owner, "Studio", mac(false))
        .await
        .unwrap();
    let upload = Submit {
        recipe: Recipe::IosTestflight,
        ..gate()
    };
    // The Mac has no App Store Connect key: refused, with why.
    let Err(Refused::NoMac(why)) = submit(&store, &owner, upload).await.unwrap() else {
        panic!("queued without a key");
    };
    assert!(why.contains("App Store Connect key"));
    report_capabilities(&store, &owner, "Studio", mac(true))
        .await
        .unwrap();
    let queued = submit(
        &store,
        &owner,
        Submit {
            recipe: Recipe::IosTestflight,
            ..gate()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert!(queued.approval);
    assert_eq!(queued.kind, Kind::Upload);
    take(&store, &owner, "Studio").await.unwrap();
    // Nobody can answer before the Mac asks.
    assert_eq!(
        answer_question(&store, &owner, &queued.id, "1", true, "web")
            .await
            .unwrap(),
        Answered::NotAsking
    );
    let ask = Question {
        id: "1".into(),
        text: "Upload OpenAgents for iOS from OpenAgentsInc/openagents at 0123abc to TestFlight?"
            .into(),
        subject: "OpenAgentsInc/openagents@0123abc ios-testflight".into(),
    };
    report(
        &store,
        &owner,
        "Studio",
        &queued.id,
        Report {
            ask: Some(ask.clone()),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    let job = load(&store, &owner, &queued.id).await.unwrap().unwrap();
    assert_eq!(job.state, JobState::Asking);
    // The phone sees it on the Mac's board, asking.
    let items = board_items(&store, &owner).await;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "Studio");
    assert_eq!(items[0].1.status, "asking");
    assert_eq!(items[0].1.question.as_ref().unwrap().id, "1");
    assert!(is_job(&items[0].1.id));
    // A wrong question id answers nothing; the phone's Deny on the right
    // one is recorded once.
    assert_eq!(
        act(&store, &owner, &queued.id, "approve", Some("2"))
            .await
            .unwrap(),
        Err((
            StatusCode::CONFLICT,
            "not_asking",
            "That job isn't waiting for an answer anymore."
        ))
    );
    assert!(
        act(&store, &owner, &queued.id, "deny", Some("1"))
            .await
            .unwrap()
            .is_ok()
    );
    assert!(
        act(&store, &owner, &queued.id, "approve", Some("1"))
            .await
            .unwrap()
            .is_err()
    );
    let heard = report(&store, &owner, "Studio", &queued.id, Report::default())
        .await
        .unwrap()
        .unwrap();
    let approval = heard.approval.unwrap();
    assert_eq!(approval.decision, "denied");
    assert_eq!(approval.via, "phone");
    let again = report(&store, &owner, "Studio", &queued.id, Report::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.approval, None);
    // Messages don't reach a job.
    assert!(
        act(&store, &owner, &queued.id, "message", None)
            .await
            .unwrap()
            .is_err()
    );
}

#[tokio::test]
async fn a_cancel_stops_a_waiting_job_at_once_and_a_running_one_at_its_next_report() {
    let (_dir, store) = store();
    let owner = account_owner("acct_owner");
    report_capabilities(&store, &owner, "Studio", mac(false))
        .await
        .unwrap();
    let waiting = submit(&store, &owner, gate()).await.unwrap().unwrap();
    cancel(&store, &owner, &waiting.id).await.unwrap();
    assert!(take(&store, &owner, "Studio").await.unwrap().is_empty());
    assert_eq!(
        load(&store, &owner, &waiting.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        JobState::Cancelled
    );
    let running = submit(&store, &owner, gate()).await.unwrap().unwrap();
    take(&store, &owner, "Studio").await.unwrap();
    act(&store, &owner, &running.id, "stop", None)
        .await
        .unwrap()
        .unwrap();
    let heard = report(&store, &owner, "Studio", &running.id, Report::default())
        .await
        .unwrap()
        .unwrap();
    assert!(heard.cancel);
    report(
        &store,
        &owner,
        "Studio",
        &running.id,
        Report {
            failed: Some(Failed {
                why: "Cancelled.".into(),
            }),
            ..Report::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        load(&store, &owner, &running.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        JobState::Cancelled
    );
}

#[tokio::test]
async fn bad_jobs_and_unknown_macs_are_refused() {
    let (_dir, store) = store();
    let owner = account_owner("acct_owner");
    report_capabilities(&store, &owner, "Studio", mac(false))
        .await
        .unwrap();
    let shell = Submit {
        recipe: Recipe::Xcodebuild,
        args: vec!["build".into(), "-exportArchive".into()],
        ..gate()
    };
    assert!(matches!(
        submit(&store, &owner, shell).await.unwrap(),
        Err(Refused::Invalid(_))
    ));
    let elsewhere = Submit {
        computer: Some("Laptop".into()),
        ..gate()
    };
    assert!(matches!(
        submit(&store, &owner, elsewhere).await.unwrap(),
        Err(Refused::NoMac(_))
    ));
    assert!(owns("/v1/mac-jobs") && owns("/v1/mac-jobs/macs"));
    assert!(!owns("/v1/mac-jobsx"));
    assert!(crate::upstream::owned("/v1/mac-jobs/mjob0/artifacts/x"));
    assert!(crate::upstream::owned("/v1/computers/Studio/mac-jobs"));
}

#[test]
fn a_mac_job_joins_its_macs_board() {
    let (_dir, _store) = store();
    let job = Job {
        schema: JOB_SCHEMA.into(),
        id: "mjob0123456789abcdef0123456789abcdef".into(),
        computer: "Studio".into(),
        spec: Spec {
            repo: "OpenAgentsInc/openagents".into(),
            git_ref: "main".into(),
            recipe: Recipe::IosReleaseGate,
            args: Vec::new(),
        },
        created_unix: 10,
        updated_unix: 10,
        state: JobState::Running,
        lines: vec!["Run the release gate UI tests".into()],
        dropped: 0,
        commit: None,
        question: None,
        approval: None,
        approval_taken: false,
        artifacts: Vec::new(),
        summary: None,
        why: None,
        cancel: false,
        finished_unix: None,
    };
    let boards = crate::phone_api::with_mac_jobs(
        std::collections::BTreeMap::new(),
        vec![("Studio".into(), item(&job))],
    );
    let board = &boards["Studio"];
    assert_eq!(board.items[0].status, "working");
    assert_eq!(board.items[0].engine.as_deref(), Some("Mac"));
    assert_eq!(
        board.items[0].line.as_deref(),
        Some("Run the release gate UI tests")
    );
    for words in [
        crate::mac_jobs_page::state_words(JobState::Asking),
        &job.spec.title(),
        "The Mac stopped answering.",
        "No Mac took this job in time.",
        "That job isn't waiting for an answer anymore.",
    ] {
        assert!(oa_copy::violations(words, &[]).is_empty(), "{words}");
    }
}

use super::*;
use playtest::report::{Context, Kind, Platform, Randomness, Report, SCHEMA, Screenshot};
use playtest::session::{Route, Tab};

fn key(byte: u8) -> SecretKey {
    SecretKey::from_byte_array([byte; 32]).expect("key")
}

fn words(text: &[&str]) -> Args {
    let words: Vec<String> = text.iter().map(|w| (*w).to_owned()).collect();
    Args::parse(&words, &["approve", "acceptances", "pending"]).expect("args")
}

fn wrap(happened: &str, quote: bool, tester: u8, wrapper: u8) -> Event {
    let report = Report {
        schema: SCHEMA.into(),
        context: Context {
            app_version: "1.0.0".into(),
            build: "15".into(),
            platform: Platform::Android,
            device: "Pixel 9".into(),
            os_version: "16".into(),
            tab: Tab::Coder,
            route: Route::Chat,
            at: 1_790_000_000,
        },
        kind: Kind::Confusing,
        happened: happened.into(),
        expected: String::new(),
        steps: String::new(),
        quote,
        task: None,
        session: None,
        // A tiny valid JPEG header, base64.
        screenshot: Some(Screenshot {
            jpeg_base64: "/9j/4AAQ".into(),
            width: 1,
            height: 1,
        }),
        notes: vec![],
        chat: None,
    };
    report::wrap(
        &report,
        &key(tester),
        &key(4).x_only_public_key(&Secp256k1::new()).0,
        &Randomness {
            wrapper: key(wrapper),
            seal_nonce: [1; 32],
            wrap_nonce: [wrapper; 32],
            seal_earlier: 10,
            wrap_earlier: 20,
        },
    )
    .expect("sealed")
    .wrap
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
#[cfg(unix)]
fn keygen_writes_a_private_key_file_and_prints_only_the_public_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("triage.key");
    let path_text = path.to_str().unwrap();
    let Ok(value) = keygen(&words(&["--out", path_text])) else {
        panic!("keygen")
    };
    assert_eq!(mode(&path), 0o600);
    let secret = read_key(&path).unwrap();
    let (hex, npub) = npub_of(&secret);
    assert_eq!(
        (value["pubkey"].as_str(), value["npub"].as_str()),
        (Some(hex.as_str()), Some(npub.as_str()))
    );
    let printed = value.to_string();
    assert!(!printed.contains("nsec1") && !printed.contains(&secret.display_secret().to_string()));
    // It never overwrites a key.
    assert!(keygen(&words(&["--out", path_text])).is_err());
    // A key file others can read is refused.
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(read_key(&path).unwrap_err().contains("chmod 600"));
}

#[test]
fn the_inbox_drafts_new_reports_once_and_files_only_with_an_issue_or_approval() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let triage = key(4);
    let events = vec![
        wrap("I couldn't find the send button.", true, 3, 9),
        wrap("I couldn't find the send button.", true, 3, 10), // the same report again
        wrap("Private words.", false, 5, 11),
        wrap("For someone else.", true, 3, 12),
    ];
    // The last wrap is re-addressed to another key: not a report for us.
    let mut events = events;
    events[3] = {
        let other = report::open(&events[0], &triage).unwrap();
        let mut sent = other.report.clone();
        sent.happened = "For someone else.".into();
        report::wrap(
            &sent,
            &key(3),
            &key(8).x_only_public_key(&Secp256k1::new()).0,
            &Randomness {
                wrapper: key(12),
                seal_nonce: [1; 32],
                wrap_nonce: [2; 32],
                seal_earlier: 0,
                wrap_earlier: 0,
            },
        )
        .unwrap()
        .wrap
    };
    let first = ingest(home, &events, &triage, 100).unwrap();
    assert_eq!((first.new.len(), first.repeats, first.refused), (2, 1, 1));
    let again = ingest(home, &events, &triage, 200).unwrap();
    assert_eq!((again.new.len(), again.repeats), (0, 3));

    let (quoted, private) = (first.new[0].clone(), first.new[1].clone());
    let drafts = drafts(home);
    #[cfg(unix)]
    for suffix in ["md", "json", "report.json", "jpg"] {
        assert_eq!(
            mode(&drafts.join(format!("{quoted}.{suffix}"))),
            0o600,
            "{suffix}"
        );
    }
    let report_json =
        std::fs::read_to_string(drafts.join(format!("{quoted}.report.json"))).unwrap();
    assert!(
        !report_json.contains("/9j/"),
        "the screenshot is a separate file"
    );

    // Without --approve or --issue nothing is filed.
    let Ok(dry) = file(home, &words(&[&quoted, "--contribution", "feedback"])) else {
        panic!("dry run")
    };
    assert_eq!(dry["dry_run"], true);
    assert_eq!(load(home).unwrap().pending().len(), 2);
    // A draft the tester didn't allow quoting must be rewritten first.
    assert!(
        file(
            home,
            &words(&[&private, "--contribution", "feedback", "--approve"])
        )
        .is_err()
    );
    // A bug needs a severity.
    assert!(
        file(
            home,
            &words(&[&quoted, "--contribution", "bug", "--issue", "9901"])
        )
        .is_err()
    );
    assert!(
        file(
            home,
            &words(&[
                &quoted,
                "--contribution",
                "bug",
                "--severity",
                "P1",
                "--issue",
                "#9901"
            ])
        )
        .is_ok()
    );
    assert!(
        decide(
            home,
            &words(&[
                &private,
                "--decision",
                "duplicate",
                "--reason",
                "Same as #9901.",
                "--issue",
                "9901"
            ])
        )
        .is_ok()
    );
    assert!(
        verify(
            home,
            &words(&[
                "--issue",
                "9901",
                "--fix-build",
                "1.0.0 (16)",
                "--verified",
                "yes"
            ])
        )
        .is_ok()
    );
    let tester = npub_of(&key(5)).1;
    assert!(
        session(
            home,
            &words(&[
                "--tester",
                &tester,
                "--script",
                "session-2",
                "--format",
                "moderated",
                "--build",
                "1.0.0 (15)"
            ])
        )
        .is_ok()
    );

    let Ok(accepted) = log(home, &words(&["--acceptances"])) else {
        panic!("log")
    };
    let contributions: Vec<&str> = accepted["acceptances"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["contribution"].as_str())
        .collect();
    assert_eq!(contributions.len(), 3);
    assert!(
        contributions.contains(&"bug")
            && contributions.contains(&"verified-fix")
            && contributions.contains(&"session")
    );
    let text = std::fs::read_to_string(log_path(home)).unwrap();
    assert_eq!(text.lines().count(), 6);
}

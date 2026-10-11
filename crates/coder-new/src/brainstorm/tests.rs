use super::*;
use crate::{
    App, Mode, Screen,
    live::{Background, Entry, Update, Work},
    plugin_store::Store,
    plugins::Connection,
};
use brainstorm_client::{
    Algorithm, Attribution, Completeness, Coverage, HouseIdentity, Influence, Operation,
    ResponseEvidence, Subject,
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use std::{
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const KEY: &str = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
const OTHER: &str = "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e";
const ORIGIN: &str = "https://brainstorm-fixture.invalid";

#[derive(Default)]
pub(super) struct Fixture {
    pub(super) calls: Mutex<Vec<Command>>,
    enabled: AtomicBool,
    delay: AtomicU64,
    error: Mutex<Option<Error>>,
    pub(super) fresh: AtomicBool,
    pub(super) injection: Mutex<Option<String>>,
}

impl Lookup for Fixture {
    fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }
    fn read<'a>(
        &'a self,
        command: &'a Command,
        cancellation: &'a Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Outcome, Error>> + Send + 'a>> {
        Box::pin(async move {
            if !self.enabled.load(Ordering::SeqCst) {
                return Err(Error::Disabled);
            }
            self.calls.lock().unwrap().push(command.clone());
            tokio::time::sleep(Duration::from_millis(self.delay.load(Ordering::SeqCst))).await;
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if let Some(error) = self.error.lock().unwrap().clone() {
                return Err(error);
            }
            Ok(if matches!(command, Command::Test) {
                Outcome::Discovery(discovery())
            } else {
                let mut observation = observation(command);
                if self.fresh.load(Ordering::Relaxed) {
                    observation.expires_at_ms = atif::now_ms() + 60_000;
                }
                if let Some(text) = self.injection.lock().unwrap().clone() {
                    observation.subjects[0].profile_url = text;
                }
                Outcome::Observation(observation)
            })
        })
    }
}

fn discovery() -> Discovery {
    Discovery {
        house: house(),
        search_supported: true,
        rank_supported: true,
        responses: vec![
            evidence(brainstorm_client::DISCOVERY_PATH, None),
            evidence(brainstorm_client::HOUSE_PATH, None),
        ],
        expires_at_ms: 60_000,
    }
}

fn house() -> HouseIdentity {
    HouseIdentity {
        pubkey: KEY.into(),
        origin: ORIGIN.into(),
        discovered_at_ms: 1000,
        attribution: Attribution::SeparateHttpsObservation,
    }
}

fn evidence(path: &str, algorithm: Option<Algorithm>) -> ResponseEvidence {
    ResponseEvidence {
        origin: ORIGIN.into(),
        endpoint: path.into(),
        status: 200,
        requested_algorithm: algorithm,
        fetched_at_ms: 1000,
        expires_at_ms: 60_000,
        input_digest: "a".repeat(64),
        output_digest: "b".repeat(64),
    }
}

fn observation(command: &Command) -> Observation {
    let mut responses = discovery().responses;
    if matches!(command, Command::Search(_)) {
        responses.push(evidence(
            brainstorm_client::SEARCH_PATH,
            Some(Algorithm::Relevance),
        ));
    }
    responses.push(evidence(
        brainstorm_client::RANK_PATH,
        Some(Algorithm::Graperank),
    ));
    let keys = match command {
        Command::Rank(keys) => keys.clone(),
        _ => vec![KEY.into(), OTHER.into()],
    };
    Observation {
        operation: if matches!(command, Command::Search(_)) {
            Operation::SearchPeople
        } else {
            Operation::Rank
        },
        configuration: Config {
            origin: ORIGIN.into(),
            ..Config::default()
        },
        configuration_digest: "c".repeat(64),
        input_digest: "d".repeat(64),
        house: house(),
        subjects: keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| Subject {
                profile_url: format!("https://njump.me/{key}"),
                pubkey: key,
                relevance: Some(0.8),
                influence: if index == 0 {
                    Some(Influence {
                        value: 0.0,
                        coverage: Coverage::Unknown,
                    })
                } else {
                    None
                },
            })
            .collect(),
        responses,
        enrichment_error: None,
        completeness: Completeness::Partial,
        expires_at_ms: 60_000,
    }
}

fn key(app: &mut App, code: KeyCode) {
    assert!(app.handle(Event::Key(KeyEvent::new(code, KeyModifiers::NONE))));
}

pub(super) fn fixture_app() -> (App, Arc<Fixture>) {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    let fixture = Arc::new(Fixture::default());
    let settings = &mut app.plugins.bundled.brainstorm;
    if let Some(binding) = settings.binding.take() {
        binding.enabled.send_replace(false);
        binding.lookup.set_enabled(false);
    }
    settings.fixture_lookup = Some(fixture.clone());
    settings.configure(
        Preferences {
            origin: ORIGIN.into(),
            ..Preferences::default()
        },
        true,
    );
    assert!(app.plugins.bundled.toggle(PLUGIN));
    (app, fixture)
}

fn add_child(app: &mut App) {
    app.delegations.push(crate::live::Delegation {
        id: "child".into(),
        name: "microcoder".into(),
        task: "Review public fixture accounts.".into(),
        chat: crate::live::Chat {
            entries: vec![Entry::Assistant {
                elapsed_ms: None,
                text: "An earlier child reply.".into(),
                model: Some("synthetic/child".into()),
            }],
            tokens: 7,
            ..Default::default()
        },
        started_at: 0,
        elapsed_seconds: 0,
        running: false,
        draft: Default::default(),
        composer: Default::default(),
        scroll: 0,
        background: false,
    });
}

fn pump(app: &mut App, background: &mut Background, done: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        background.sync(app);
        if done(app) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Brainstorm worker did not finish"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn text(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 45)).unwrap();
    terminal
        .draw(|frame| crate::ui::render(frame, app))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn grammar_preserves_public_query_and_validates_identity_and_bounds() {
    assert_eq!(
        parse("/brainstorm search Rust and Nostr"),
        Some(Ok(Command::Search("Rust and Nostr".into())))
    );
    let npub = "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";
    assert_eq!(
        parse(&format!("/brainstorm rank {} {OTHER}", npub)),
        Some(Ok(Command::Rank(vec![KEY.into(), OTHER.into()])))
    );
    assert_eq!(
        parse(&format!("/brainstorm rank {}", KEY.to_uppercase())),
        Some(Ok(Command::Rank(vec![KEY.into()])))
    );
    for input in [
        "/brainstorm",
        "/brainstorm search",
        "/brainstorm rank",
        "/brainstorm SEARCH Rust",
        "/brainstorm unknown Rust",
        "/brainstorm rank nsec1notpublic",
        "/brainstorm rank https://njump.me/key",
        "/brainstorm rank npub1invalid",
    ] {
        assert!(parse(input).unwrap().is_err(), "{input}");
    }
    for input in [
        "/brainstormed search Rust",
        "say /brainstorm search Rust",
        "/Brainstorm search Rust",
        "/brainstorm/search Rust",
    ] {
        assert!(parse(input).is_none());
    }
    assert!(
        parse(&format!("/brainstorm rank {KEY} {npub}"))
            .unwrap()
            .is_err()
    );
    assert!(
        parse(&format!("/brainstorm rank {}", "0".repeat(64)))
            .unwrap()
            .is_err()
    );
    for query in ["x".repeat(513), "界".repeat(342)] {
        assert!(
            parse(&format!("/brainstorm search {query}"))
                .unwrap()
                .is_err()
        );
    }
    for query in ["x".repeat(512), "é".repeat(512)] {
        assert!(
            parse(&format!("/brainstorm search {query}"))
                .unwrap()
                .is_ok()
        );
    }
}

#[test]
fn settings_migrate_private_atomic_storage_and_keep_demo_separate() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::under(directory.path());
    let mut old = serde_json::json!({ "version": 1, "microcoder": true, "cli": true, "acp": true,
        "jev_enabled": true, "jev_model": "jev-latest", "jev_endpoint": "https://api.typesafe.ai", "jev_key": null, "acp_agents": [] });
    store.save_extra("bundled-plugins.json", &old).unwrap();
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.load_plugin_settings(store.clone()).unwrap();
    assert!(!app.plugins.bundled.brainstorm.preferences.enabled);
    assert!(app.plugins.bundled.toggle(PLUGIN));
    app.plugins.bundled.brainstorm.begin();
    app.plugins.bundled.brainstorm.draft = crate::Draft {
        text: ORIGIN.into(),
        cursor: ORIGIN.len(),
    };
    assert!(app.plugins.bundled.save_brainstorm());
    let mut restored = App::default();
    restored.set_mode(Mode::Live);
    restored.load_plugin_settings(store.clone()).unwrap();
    assert_eq!(
        restored.plugins.bundled.brainstorm.preferences,
        app.plugins.bundled.brainstorm.preferences
    );
    assert!(matches!(
        restored.plugins.bundled.brainstorm.connection,
        Connection::Unchecked
    ));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(directory.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(directory.path().join("bundled-plugins.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let saved = std::fs::read(directory.path().join("bundled-plugins.json")).unwrap();
    restored.set_mode(Mode::Demo);
    assert!(!restored.plugins.bundled.brainstorm.preferences.enabled);
    assert!(restored.plugins.bundled.toggle(PLUGIN));
    restored.plugins.bundled.brainstorm.begin();
    restored.plugins.bundled.brainstorm.draft = crate::Draft {
        text: "https://demo.invalid".into(),
        cursor: 20,
    };
    assert!(restored.plugins.bundled.save_brainstorm());
    assert_eq!(
        std::fs::read(directory.path().join("bundled-plugins.json")).unwrap(),
        saved
    );
    restored.set_mode(Mode::Live);
    assert_eq!(
        restored.plugins.bundled.brainstorm.preferences.origin,
        ORIGIN
    );
    old["version"] = serde_json::json!(2);
    store.save_extra("bundled-plugins.json", &old).unwrap();
    let original = std::fs::read(directory.path().join("bundled-plugins.json")).unwrap();
    assert!(!restored.plugins.bundled.toggle(PLUGIN));
    assert_eq!(
        std::fs::read(directory.path().join("bundled-plugins.json")).unwrap(),
        original
    );
}

#[test]
fn opening_enabling_saving_and_disabling_never_dispatch_a_read() {
    let (mut app, fixture) = fixture_app();
    app.open_plugins();
    app.plugins.selected = crate::plugin_definition::DEFINITIONS
        .iter()
        .position(|p| p.id == super::PLUGIN)
        .unwrap();
    app.open_plugin_settings();
    assert_eq!(app.plugins.selected_definition().id, PLUGIN);
    let rendered = text(&mut app);
    assert!(rendered.contains("Brainstorm house perspective"));
    assert!(rendered.contains(ORIGIN));
    assert!(rendered.contains("Opening, saving, or enabling makes no service read."));
    app.plugins.bundled.brainstorm.focus = Focus::Save;
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Char(' '));
    key(&mut app, KeyCode::Char(' '));
    assert!(fixture.calls.lock().unwrap().is_empty());
    assert!(app.request.is_none());
    app.plugins.bundled.brainstorm.begin();
    app.plugins.bundled.brainstorm.draft = crate::Draft {
        text: "http://example.invalid".into(),
        cursor: 22,
    };
    assert!(!app.plugins.bundled.save_brainstorm());
    assert_eq!(app.plugins.bundled.brainstorm.preferences.origin, ORIGIN);
}

#[test]
fn explicit_lookup_works_without_credentials_and_completes_without_provider_changes() {
    let (mut app, fixture) = fixture_app();
    app.live.tokens = 77;
    app.plugins.connection = Connection::Failed("provider fixture".into());
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    assert!(matches!(
        app.request.as_ref().unwrap().kind,
        Work::Brainstorm { .. }
    ));
    assert!(app.request.as_ref().unwrap().key.expose().is_empty());
    assert!(app.live.busy);
    assert!(matches!(
        app.live.entries.last(),
        Some(Entry::Tool { running: true, .. })
    ));
    let mut background = Background::default();
    pump(&mut app, &mut background, |app| !app.live.busy);
    assert_eq!(
        fixture.calls.lock().unwrap().as_slice(),
        &[Command::Search("Rust".into())]
    );
    assert_eq!(app.live.tokens, 77);
    assert!(
        matches!(&app.plugins.connection, Connection::Failed(error) if error == "provider fixture")
    );
    assert!(
        !app.live
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Assistant { .. }))
    );
    let output = match app.live.entries.last().unwrap() {
        Entry::Tool {
            output,
            running: false,
            ..
        } => output,
        _ => panic!("completed lookup missing"),
    };
    let rendered = summary(output);
    for label in [
        ORIGIN,
        KEY,
        "Raw influence",
        "unknown",
        "unavailable",
        "Separate HTTPS observation",
        "Combined expiry",
        "Input hash",
    ] {
        assert!(rendered.contains(label), "{label}");
    }
}

#[test]
fn following_user_and_bounded_observation_reach_local_and_remote_generation_inputs() {
    let (mut app, _) = fixture_app();
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    pump(&mut app, &mut Background::default(), |app| !app.live.busy);
    app.submit("Compare these accounts.", std::path::Path::new("/fixture"));
    let request = app.request.take().unwrap();
    let Work::Microcoder {
        messages,
        execution,
    } = request.kind
    else {
        panic!("local route missing")
    };
    let task = crate::live::local_task(&messages, &execution).unwrap();
    assert!(task.contains("Compare these accounts."));
    assert!(task.contains("separate_https_observation"));
    assert!(task.contains(KEY));
    assert!(task.contains("unknown"));
    assert!(task.contains("expires_at_ms"));
    assert!(task.len() <= 56 * 1024);
    let observation = messages
        .iter()
        .find(|message| message.content.contains("\"observation_is_fresh\""))
        .unwrap();
    assert!(observation.content.len() <= 8 * 1024);
    app.cancel_request();
    app.plugins
        .bootstrap_credentials(crate::credentials::Imported {
            openrouter_key: Some(model_access::ApiKey::new("synthetic-provider-fixture")),
            ..crate::credentials::Imported::default()
        });
    app.submit("Compare their coverage.", std::path::Path::new("/fixture"));
    let Work::Chat { messages, .. } = app.request.take().unwrap().kind else {
        panic!("remote route missing")
    };
    assert!(
        messages
            .iter()
            .any(|message| message.content == "Compare their coverage.")
    );
    assert!(messages.iter().any(
        |message| message.content.contains("separate_https_observation")
            && message.content.contains(KEY)
    ));
}

#[test]
fn headless_lookup_returns_its_current_bounded_observation_or_error_without_a_provider() {
    let directory = tempfile::tempdir().unwrap();
    let context = crate::programmatic::Context {
        root: directory.path().join("state"),
        cwd: directory.path().into(),
        environment: Default::default(),
        input: None,
        canceled: None,
        approvals: None,
    };
    let (mut app, fixture) = fixture_app();
    app.live.tokens = 42;
    app.live.entries.push(Entry::Assistant {
        elapsed_ms: None,
        text: "An earlier model reply.".into(),
        model: Some("synthetic/previous".into()),
    });
    let mut events = Vec::new();
    let result = crate::programmatic::chat(
        &mut app,
        &[
            "--session".into(),
            "lookup".into(),
            "-p".into(),
            "/brainstorm search Rust".into(),
        ],
        &context,
        &mut |event| events.push(event),
    )
    .unwrap();
    let reply = result["reply"].as_str().unwrap();
    assert!(reply.len() <= 8 * 1024);
    let observation: serde_json::Value = serde_json::from_str(reply).unwrap();
    assert_eq!(observation["observation"]["house"]["pubkey"], KEY);
    assert!(observation.get("observation_is_fresh").is_some());
    assert!(!reply.contains("An earlier model reply."));
    assert!(
        events.iter().any(|event| event["event"] == "entry"
            && event["entry"]["output"].get("observation").is_some())
    );
    *fixture.error.lock().unwrap() = Some(Error::AuthenticationRequired { status: 401 });
    let failure = crate::programmatic::chat(
        &mut app,
        &[
            "--session".into(),
            "lookup".into(),
            "-p".into(),
            format!("/brainstorm rank {KEY}"),
        ],
        &context,
        &mut |_| {},
    )
    .unwrap();
    let reply = failure["reply"].as_str().unwrap();
    let output: serde_json::Value = serde_json::from_str(reply).unwrap();
    assert!(output.get("error").is_some());
    assert_eq!(output["recipient"], ORIGIN);
    assert_eq!(output["operation"], "brainstorm.rank");
    assert!(reply.contains("401"));
    assert!(!reply.contains("An earlier model reply."));
    assert!(reply.len() <= 8 * 1024);
    assert_eq!(fixture.calls.lock().unwrap().len(), 2);
    assert_eq!(app.live.tokens, 42);
    assert!(matches!(app.plugins.connection, Connection::Unchecked));
    assert_eq!(
        app.live
            .entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Assistant { .. }))
            .count(),
        1
    );
}

#[test]
fn headless_child_lookup_uses_its_own_turn_boundary_and_following_conversation() {
    let directory = tempfile::tempdir().unwrap();
    let context = crate::programmatic::Context {
        root: directory.path().join("state"),
        cwd: directory.path().into(),
        environment: Default::default(),
        input: None,
        canceled: None,
        approvals: None,
    };
    let (mut app, fixture) = fixture_app();
    app.live
        .entries
        .extend((0..30).map(|index| Entry::User(format!("Parent history {index}"))));
    app.live.entries.push(Entry::Tool {
        name: Command::Search(String::new()).name().into(),
        input: serde_json::Value::Null,
        output: output(Outcome::Observation(observation(&Command::Search(
            "Parent account".into(),
        )))),
        running: false,
    });
    app.live.entries.push(Entry::Assistant {
        elapsed_ms: None,
        text: "An earlier parent reply.".into(),
        model: Some("synthetic/parent".into()),
    });
    let parent = app.live.entries.clone();
    add_child(&mut app);
    let mut events = Vec::new();
    let result = crate::programmatic::chat(
        &mut app,
        &[
            "--session".into(),
            "child-lookup".into(),
            "--delegation".into(),
            "child".into(),
            "-p".into(),
            format!("/brainstorm rank {OTHER}"),
        ],
        &context,
        &mut |event| events.push(event),
    )
    .unwrap();
    let reply = result["reply"].as_str().unwrap();
    let result: serde_json::Value = serde_json::from_str(reply).unwrap();
    assert!(reply.len() <= 8 * 1024);
    assert_eq!(result["observation"]["operation"], "rank");
    assert_eq!(result["observation"]["subjects"][0]["pubkey"], OTHER);
    assert_eq!(app.selected_agent, Some(0));
    assert!(app.live.entries == parent);
    assert!(app.live.entries.len() > app.delegations[0].chat.entries.len());
    assert_eq!(app.delegations[0].chat.tokens, 7);
    assert!(!app.delegations[0].running);
    assert!(!app.delegations[0].chat.busy);
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "delegation_entry"
                && event["delegation"] == "child"
                && event["entry"]["source"] == "user")
    );
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "delegation_entry"
                && event["delegation"] == "child"
                && event["entry"]["output"].get("observation").is_some())
    );
    let retained = crate::sessions::Store::under(&context.root)
        .read("child-lookup")
        .unwrap();
    assert!(crate::trajectory::from_document(&retained).unwrap().entries == parent);
    let child = crate::trajectory::from_document(&retained["subagent_trajectories"][0]).unwrap();
    assert!(
        matches!(child.entries.last(), Some(Entry::Tool { output, .. }) if output["observation"]["subjects"][0]["pubkey"] == OTHER)
    );

    app.submit("Compare this child's account.", directory.path());
    let Work::Delegate {
        delegation,
        arguments,
        ..
    } = app.request.take().unwrap().kind
    else {
        panic!("The following child turn must keep its ordinary delegation route")
    };
    assert_eq!(delegation, "child");
    let task = arguments["task"].as_str().unwrap();
    assert!(task.contains("Compare this child's account."));
    assert!(task.contains(OTHER));
    assert!(task.contains("separate_https_observation"));
    assert!(task.contains("expires_at_ms"));
    assert!(!task.contains("Parent history"));
    assert!(task.contains("unknown"));
    app.cancel_request();
    *fixture.error.lock().unwrap() = Some(Error::AuthenticationRequired { status: 401 });
    let failure = crate::programmatic::chat(
        &mut app,
        &[
            "--session".into(),
            "child-lookup".into(),
            "--delegation".into(),
            "child".into(),
            "-p".into(),
            "/brainstorm search Unavailable".into(),
        ],
        &context,
        &mut |_| {},
    )
    .unwrap();
    let failure: serde_json::Value =
        serde_json::from_str(failure["reply"].as_str().unwrap()).unwrap();
    assert!(failure.get("error").is_some());
    assert!(failure.get("observation").is_none());
    assert_eq!(failure["recipient"], ORIGIN);
    assert!(app.live.entries == parent);
    assert_eq!(fixture.calls.lock().unwrap().len(), 2);
    assert!(matches!(app.plugins.connection, Connection::Unchecked));
}

#[test]
fn child_lookup_completion_and_cancel_keep_the_original_target_after_navigation() {
    let (mut app, fixture) = fixture_app();
    add_child(&mut app);
    app.select_agent(Some(0));
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    assert!(app.delegations[0].chat.busy);
    assert!(app.delegations[0].running);
    assert!(matches!(
        app.delegations[0].chat.entries.last(),
        Some(Entry::Tool { running: true, .. })
    ));
    app.select_agent(None);
    pump(&mut app, &mut Background::default(), |app| !app.live.busy);
    assert_eq!(app.selected_agent, None);
    assert!(app.live.entries.is_empty());
    assert!(!app.delegations[0].running);
    assert!(!app.delegations[0].chat.busy);
    assert!(
        matches!(app.delegations[0].chat.entries.last(), Some(Entry::Tool { output, running: false, .. }) if output.get("observation").is_some())
    );
    app.select_agent(Some(0));
    fixture.delay.store(100, Ordering::SeqCst);
    app.submit(
        &format!("/brainstorm rank {OTHER}"),
        std::path::Path::new("/fixture"),
    );
    let pending = app.brainstorm_job.as_ref().unwrap().clone();
    let id = app.request_id;
    app.select_agent(None);
    key(&mut app, KeyCode::Esc);
    assert!(pending.cancellation.is_cancelled());
    assert!(!app.delegations[0].running);
    assert!(!app.delegations[0].chat.busy);
    app.apply_update(Update::BrainstormFinished {
        id,
        generation: pending.generation,
        result: Ok(Outcome::Observation(observation(&pending.command))),
    });
    assert!(app.live.entries.is_empty());
    assert!(
        matches!(app.delegations[0].chat.entries.last(), Some(Entry::Tool { output, running: false, .. }) if output.get("error").is_some() && output.get("observation").is_none())
    );
    assert_eq!(app.delegations[0].chat.tokens, 7);
}

#[test]
fn invalid_disabled_and_unsupported_commands_create_no_provider_or_lookup_work() {
    let (mut app, fixture) = fixture_app();
    for command in [
        "/brainstorm search",
        "/brainstorm rank nsec1secret",
        "/brainstorm unsupported Rust",
    ] {
        app.submit(command, std::path::Path::new("/fixture"));
        assert!(app.request.is_none());
        assert!(app.live.entries.is_empty());
    }
    assert!(app.plugins.bundled.toggle(PLUGIN));
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    assert!(app.request.is_none());
    assert!(
        !app.slash_hints()
            .contains(&crate::slash::Command::Brainstorm)
    );
    assert!(app.plugins.bundled.toggle(PLUGIN));
    app.plugins.bundled.brainstorm.binding = None;
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    assert!(app.request.is_none());
    assert!(app.live.notice.as_deref().unwrap().contains("unavailable"));
    assert!(fixture.calls.lock().unwrap().is_empty());
}

#[test]
fn deliberate_connection_testing_and_escape_use_their_own_completion() {
    let (mut app, fixture) = fixture_app();
    app.plugins.bundled.brainstorm.begin();
    app.check_brainstorm_connection();
    assert!(!app.live.busy);
    assert!(matches!(
        app.plugins.bundled.brainstorm.connection,
        Connection::Checking
    ));
    let mut background = Background::default();
    pump(&mut app, &mut background, |app| {
        app.brainstorm_job.is_none()
    });
    assert!(matches!(
        app.plugins.bundled.brainstorm.connection,
        Connection::Verified
    ));
    assert_eq!(
        app.plugins
            .bundled
            .brainstorm
            .discovery
            .as_ref()
            .unwrap()
            .house
            .pubkey,
        KEY
    );
    assert!(app.live.entries.is_empty());
    assert!(matches!(app.plugins.connection, Connection::Unchecked));
    fixture.delay.store(100, Ordering::SeqCst);
    app.open_plugin_settings();
    app.plugins.selected = crate::plugin_definition::DEFINITIONS
        .iter()
        .position(|p| p.id == super::PLUGIN)
        .unwrap();
    app.open_plugin_settings();
    app.check_brainstorm_connection();
    background.sync(&mut app);
    key(&mut app, KeyCode::Esc);
    background.sync(&mut app);
    assert!(app.brainstorm_job.is_none());
    assert!(matches!(
        app.plugins.bundled.brainstorm.connection,
        Connection::Unchecked
    ));
    assert_eq!(app.screen as u8, Screen::Plugins as u8);
    app.open_plugin_settings();
    app.check_brainstorm_connection();
    let canceled = app.brainstorm_job.as_ref().unwrap().clone();
    app.plugins.bundled.brainstorm.focus = Focus::Cancel;
    key(&mut app, KeyCode::Enter);
    assert!(canceled.cancellation.is_cancelled());
    assert!(app.brainstorm_job.is_none());
    assert_eq!(app.screen as u8, Screen::Plugins as u8);
}

#[test]
fn pending_discovery_prevents_switching_conversations_after_dispatch() {
    let (mut app, _) = fixture_app();
    app.plugins.bundled.brainstorm.begin();
    app.check_brainstorm_connection();
    // The background worker consumes the request while discovery stays pending.
    let request = app.request.take().unwrap();
    assert!(matches!(request.kind, Work::Brainstorm { .. }));
    assert!(!app.live.busy);
    let id = app.request_id;
    assert!(!app.resume(None));
    assert!(
        app.notice
            .as_deref()
            .unwrap()
            .contains("Stop the current work")
    );
    assert_eq!(app.request_id, id);
    assert!(app.brainstorm_job.is_some());
    app.cancel_request();
    assert!(app.brainstorm_job.is_none());
}

#[test]
fn escape_and_disable_reenable_refuse_stale_results_and_dispatch_snapshots() {
    let (mut app, fixture) = fixture_app();
    let stale = app
        .plugins
        .bundled
        .brainstorm
        .job(Command::Search("Rust".into()))
        .unwrap();
    assert!(app.plugins.bundled.toggle(PLUGIN));
    assert!(app.plugins.bundled.toggle(PLUGIN));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    assert!(matches!(
        runtime.block_on(stale.run()),
        Err(Error::Disabled)
    ));
    assert!(fixture.calls.lock().unwrap().is_empty());
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    let job = app.brainstorm_job.as_ref().unwrap().clone();
    let id = app.request_id;
    key(&mut app, KeyCode::Esc);
    assert!(job.cancellation.is_cancelled());
    assert!(!app.live.busy);
    app.apply_update(Update::BrainstormFinished {
        id,
        generation: job.generation,
        result: Ok(Outcome::Observation(observation(&job.command))),
    });
    assert!(
        matches!(app.live.entries.last(), Some(Entry::Tool { output, running: false, .. }) if output.get("observation").is_none())
    );
    app.submit("/brainstorm search Nostr", std::path::Path::new("/fixture"));
    let active = app.brainstorm_job.as_ref().unwrap().clone();
    let id = app.request_id;
    assert!(app.plugins.bundled.toggle(PLUGIN));
    assert!(app.plugins.bundled.toggle(PLUGIN));
    app.apply_update(Update::BrainstormFinished {
        id,
        generation: active.generation,
        result: Ok(Outcome::Observation(observation(&active.command))),
    });
    assert!(!app.live.busy);
    assert!(!app.live.entries.iter().any(
        |entry| matches!(entry, Entry::Tool { output, .. } if output.get("observation").is_some())
    ));
}

#[test]
fn active_worker_cancel_drops_a_pending_lookup_and_keeps_following_chat_usable() {
    let (mut app, fixture) = fixture_app();
    fixture.delay.store(100, Ordering::SeqCst);
    app.submit("/brainstorm search Rust", std::path::Path::new("/fixture"));
    let active = app.brainstorm_job.as_ref().unwrap().clone();
    let mut background = Background::default();
    let deadline = Instant::now() + Duration::from_secs(2);
    while fixture.calls.lock().unwrap().is_empty() {
        background.sync(&mut app);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    key(&mut app, KeyCode::Esc);
    background.sync(&mut app);
    assert!(active.cancellation.is_cancelled());
    assert!(!app.live.busy);
    std::thread::sleep(Duration::from_millis(120));
    background.sync(&mut app);
    assert!(!app.live.entries.iter().any(
        |entry| matches!(entry, Entry::Tool { output, .. } if output.get("observation").is_some())
    ));
    app.submit(
        "Explain the cancellation.",
        std::path::Path::new("/fixture"),
    );
    assert!(matches!(
        app.request.as_ref().unwrap().kind,
        Work::Microcoder { .. }
    ));
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
}

#[test]
fn local_route_accounts_for_standing_instructions_and_cli_guidance() {
    let (app, _) = fixture_app();
    let mut execution = app
        .plugins
        .execution_settings(std::path::PathBuf::from("/fixture"));
    execution.instructions = Some("host instructions".repeat(100));
    let messages = vec![
        openrouter::Message::user("old context".repeat(6000)),
        openrouter::Message::user("Keep the following user text."),
    ];
    let task = crate::live::local_task(&messages, &execution).unwrap();
    assert!(task.len() <= 56 * 1024);
    assert!(task.contains("Keep the following user text."));
    assert!(task.contains("Standing instructions"));
    // The local loop's own login names its product; Coder's leads (#11264).
    assert!(task.starts_with("Standing instructions (from the host, not the user):\nYou are Coder, OpenAgents' coding agent."));
    assert!(task.contains("bundled OpenAgents CLI"));
    execution.instructions = Some("x".repeat(56 * 1024));
    assert!(crate::live::local_task(&messages, &execution).is_err());
}

#[test]
fn demo_uses_explicitly_labeled_fixtures_and_does_not_leak_live_preferences() {
    let (mut app, fixture) = fixture_app();
    app.set_mode(Mode::Demo);
    assert!(!app.plugins.bundled.brainstorm.preferences.enabled);
    assert!(app.plugins.bundled.toggle(PLUGIN));
    app.draft.text = "/brainstorm search Rust".into();
    key(&mut app, KeyCode::Enter);
    assert!(app.request.is_none());
    assert!(
        app.messages
            .iter()
            .any(|message| message.contains("demo fixture"))
    );
    app.check_brainstorm_connection();
    assert!(app.plugins.bundled.brainstorm.fixture);
    assert!(fixture.calls.lock().unwrap().is_empty());
    app.set_mode(Mode::Live);
    assert_eq!(app.plugins.bundled.brainstorm.preferences.origin, ORIGIN);
    assert!(app.live.entries.is_empty());
}

#[test]
fn context_retains_only_the_newest_observation_with_an_eight_kibibyte_budget() {
    let mut chat = crate::live::Chat::default();
    for index in 0..20 {
        let mut observation = observation(&Command::Search("public query".into()));
        observation.input_digest = format!("{index:064x}");
        chat.entries.push(Entry::Tool {
            name: "brainstorm.search_people".into(),
            input: serde_json::Value::Null,
            output: output(Outcome::Observation(observation)),
            running: false,
        });
    }
    chat.entries
        .push(Entry::User("Compare these accounts.".into()));
    let messages = chat.messages();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].content.len() <= 8 * 1024);
    assert!(messages[0].content.contains(&format!("{:064x}", 19)));
    assert_eq!(messages[1].content, "Compare these accounts.");
    assert_eq!(chat.entries.len(), 21);
}

#[test]
fn lookup_failure_keeps_typed_state_and_does_not_set_provider_auth_failure() {
    let (mut app, fixture) = fixture_app();
    *fixture.error.lock().unwrap() = Some(Error::AuthenticationRequired { status: 401 });
    app.plugins.connection = Connection::Verified;
    app.submit(
        &format!("/brainstorm rank {KEY}"),
        std::path::Path::new("/fixture"),
    );
    pump(&mut app, &mut Background::default(), |app| !app.live.busy);
    assert!(matches!(app.plugins.connection, Connection::Verified));
    assert!(
        matches!(app.live.entries.last(), Some(Entry::Tool { output, running: false, .. }) if output["state"]["kind"] == "authentication_required")
    );
    assert_eq!(fixture.calls.lock().unwrap().len(), 1);
}

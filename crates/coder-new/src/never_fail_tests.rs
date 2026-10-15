//! #11645: a turn never dead-ends. For every combination of what is
//! installed, signed in, turned on, picked, and failing, a turn either gets
//! an answer from some door, or, when nothing at all can run, ends with
//! exactly one message that names the fix.

use crate::bundled_runtime::AcpAgent;
use crate::harness_pick::Pick;
use crate::harness_plugin::Harness;
use crate::live::{Entry, Update, Work};
use crate::route::{Favor, Lane, NoHarness, plan};
use crate::{App, Mode};

const LANES: [Lane; 6] = [
    Lane::Gateway,
    Lane::Local,
    Lane::OpenCode,
    Lane::OpenRouter,
    Lane::GeminiKey,
    Lane::FreeTier,
];

/// The OpenAgents account in one world of the invariant.
#[derive(Clone, Copy, Debug)]
enum Account {
    SignedOut,
    Open,
    /// Signed in, but the gateway refused this run.
    Closed,
}

fn account(state: Account, n: usize) -> Option<openagents_login::Saved> {
    let token = format!("never-fail-fixture-{n}");
    let saved = || {
        serde_json::from_value(serde_json::json!({
            "origin": "http://fixture", "account": "acct", "label": "Fixture",
            "expires_at": u64::MAX, "token": token,
        }))
        .unwrap()
    };
    match state {
        Account::SignedOut => None,
        Account::Open => Some(saved()),
        Account::Closed => {
            crate::provider::refuse_gateway_for_test(&token);
            Some(saved())
        }
    }
}

fn opencode_agent() -> AcpAgent {
    AcpAgent {
        id: "opencode".into(),
        name: "OpenCode".into(),
        program: "opencode".into(),
        transport: Default::default(),
        arguments: vec![],
        mode: None,
        enabled: true,
    }
}

/// How a door does when the turn tries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Answers,
    FailsToStart,
    Silent,
    /// The gateway answered 5xx (after its retries).
    ServerError(u16),
    /// The connection dropped before an answer.
    ConnectionReset,
}

/// The route's invariant over every world: every door that is connected
/// is in the plan (so a working door is always tried), a door the chat is
/// held to goes first but never alone, signed out the free answers are
/// last, and when no door works the ending is one message with the fix.
#[test]
fn every_world_answers_or_ends_with_one_actionable_message() {
    let mut worlds = 0;
    let mut n = 0;
    for signed in [Account::SignedOut, Account::Open, Account::Closed] {
        for has_key in [false, true] {
            for opencode in [false, true] {
                for free_tier in [false, true] {
                    for no_harness in [None, Some(false), Some(true)] {
                        for cheap in [false, true] {
                            for picked in [false, true] {
                                for door in std::iter::once(None).chain(LANES.map(Some)) {
                                    n += 1;
                                    let mut execution = crate::plugins::Plugins::default()
                                        .execution_settings(std::env::temp_dir());
                                    execution.connections = account(signed, n);
                                    execution.agents = if opencode {
                                        vec![opencode_agent()]
                                    } else {
                                        vec![]
                                    };
                                    execution.free_tier = free_tier;
                                    execution.door = door;
                                    execution.cheap = cheap.then(Default::default);
                                    execution.gateway_model =
                                        picked.then(|| "zai/glm-5.3-flash".to_owned());
                                    execution.no_harness = no_harness.map(|gemini| NoHarness {
                                        policy: coder_sdk::auto_policy::AutoPolicy::default(),
                                        gemini_key: gemini
                                            .then(|| model_access::ApiKey::new("fixture-gemini")),
                                        gemini_base: coder_sdk::auto_policy::GEMINI_OPENAI_BASE
                                            .into(),
                                    });
                                    let open = matches!(signed, Account::Open);
                                    // The doors that exist in this world.
                                    let connected = |lane: Lane| match lane {
                                        Lane::Gateway => open,
                                        Lane::Local | Lane::OpenCode if no_harness.is_some() => {
                                            false
                                        }
                                        Lane::Local => true,
                                        Lane::OpenCode => opencode,
                                        Lane::OpenRouter => has_key,
                                        Lane::GeminiKey => no_harness == Some(true),
                                        Lane::FreeTier => free_tier,
                                    };
                                    let route = plan(&execution, has_key, Favor::default());
                                    let world = format!(
                                        "{signed:?} key={has_key} opencode={opencode} free={free_tier} \
                                         no_harness={no_harness:?} cheap={cheap} picked={picked} door={door:?} \
                                         -> {:?}",
                                        route.lanes
                                    );
                                    // Every connected door is tried.
                                    for lane in LANES {
                                        if connected(lane) {
                                            assert!(
                                                route.lanes.contains(&lane),
                                                "{lane:?} missing: {world}"
                                            );
                                        }
                                    }
                                    // No door is tried twice.
                                    let mut seen = route.lanes.clone();
                                    seen.dedup();
                                    seen.sort_by_key(|lane| format!("{lane:?}"));
                                    seen.dedup();
                                    assert_eq!(
                                        seen.len(),
                                        route.lanes.len(),
                                        "a door twice: {world}"
                                    );
                                    // The free answers are the last resort.
                                    if route.lanes.contains(&Lane::FreeTier)
                                        && route.held != Some(Lane::FreeTier)
                                    {
                                        assert_eq!(
                                            route.lanes.last(),
                                            Some(&Lane::FreeTier),
                                            "{world}"
                                        );
                                    }
                                    // A held door goes first, never alone when others exist.
                                    if let Some(held) = route.held {
                                        assert_eq!(route.lanes.first(), Some(&held), "{world}");
                                    }
                                    // Walk the plan under each single working door,
                                    // and under none.
                                    for working in std::iter::once(None).chain(LANES.map(Some)) {
                                        if working.is_some_and(|lane| !connected(lane)) {
                                            continue;
                                        }
                                        for failing in [
                                            Outcome::FailsToStart,
                                            Outcome::Silent,
                                            Outcome::ServerError(502),
                                            Outcome::ServerError(503),
                                            Outcome::ConnectionReset,
                                        ] {
                                            worlds += 1;
                                            let outcome = |lane: Lane| {
                                                if Some(lane) == working {
                                                    Outcome::Answers
                                                } else {
                                                    failing
                                                }
                                            };
                                            let mut reasons = route.notes.clone();
                                            let answered = route.lanes.iter().find(|lane| {
                                                match outcome(**lane) {
                                                    Outcome::Answers => true,
                                                    Outcome::FailsToStart => {
                                                        reasons.push(format!("{} could not start: boom", lane.name()));
                                                        false
                                                    }
                                                    Outcome::Silent => {
                                                        reasons.push(format!("{} didn't start answering within 30 seconds", lane.name()));
                                                        false
                                                    }
                                                    // The gateway's own words for a
                                                    // 5xx or a dropped connection,
                                                    // after its retries.
                                                    Outcome::ServerError(status) => {
                                                        reasons.push(format!("OpenAgents could not complete this request (HTTP {status})."));
                                                        false
                                                    }
                                                    Outcome::ConnectionReset => {
                                                        reasons.push("The OpenAgents connection failed. The request was not retried.".into());
                                                        false
                                                    }
                                                }
                                            });
                                            match (working, answered) {
                                                (Some(lane), Some(answered)) => {
                                                    assert_eq!(*answered, lane, "{world}")
                                                }
                                                (Some(lane), None) => {
                                                    panic!(
                                                        "{lane:?} works but was never tried: {world}"
                                                    )
                                                }
                                                (None, Some(lane)) => {
                                                    panic!("{lane:?} can't answer: {world}")
                                                }
                                                (None, None) => {
                                                    let ending = route.explain(&reasons);
                                                    assert!(
                                                        !ending.contains('\n'),
                                                        "one message: {ending}"
                                                    );
                                                    assert!(
                                                        ending.contains("coder login")
                                                            || ending.contains("/plugins")
                                                            || ending.contains("openagents.com"),
                                                        "no fix in {ending:?}: {world}"
                                                    );
                                                    // Credit is the fix only for a 402.
                                                    assert!(
                                                        !ending.to_lowercase().contains("credit"),
                                                        "credit for a failure that isn't a 402: {ending}: {world}"
                                                    );
                                                    if !open && !matches!(signed, Account::Closed) {
                                                        assert!(
                                                            ending.contains("`coder login`"),
                                                            "{ending}: {world}"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(worlds > 5_000, "{worlds} worlds");
}

/// When the free answers run out, the ending says so plainly with the
/// command that gets more.
#[test]
fn used_up_free_answers_say_sign_in_for_more() {
    let mut execution = crate::plugins::Plugins::default().execution_settings(std::env::temp_dir());
    execution.free_tier = true;
    let route = plan(&execution, false, Favor::default());
    assert_eq!(route.lanes, [Lane::Local, Lane::FreeTier]);
    let ending = route.explain(&[
        "No model login on this computer can answer right now.".into(),
        crate::free_tier::USED_UP.into(),
    ]);
    assert_eq!(ending, crate::free_tier::USED_UP);
    assert!(ending.contains("`coder login`"), "{ending}");
}

// ------------------------------------------------------------------ the app

fn live_app() -> App {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.cwd = Some(std::env::temp_dir());
    app
}

fn computer(opencode: bool, claude: bool) {
    crate::opencode_chat::install_for_test(opencode);
    crate::harness_plugin::sign_in_claude(claude);
    crate::codex::set_test_program(None);
}

fn kind(app: &App) -> Option<&'static str> {
    app.request.as_ref().map(|request| match &request.kind {
        Work::OpenCode { .. } => "opencode",
        Work::Claude { .. } => "claude",
        Work::Codex { .. } => "codex",
        Work::Acp { .. } => "agent",
        Work::Microcoder { .. } | Work::Chat { .. } => "auto",
        _ => "other",
    })
}

fn finish(app: &mut App, result: Result<openrouter::Streamed, String>) {
    let id = app.request_id;
    app.request = None;
    app.apply_update(Update::Finished { id, result });
}

fn answer(text: &str, model: &str) -> Result<openrouter::Streamed, String> {
    Ok(openrouter::Streamed {
        text: text.into(),
        model: model.into(),
        ..Default::default()
    })
}

fn user_entries(app: &App) -> usize {
    app.live
        .entries
        .iter()
        .filter(|entry| matches!(entry, Entry::User(_)))
        .count()
}

fn answers(app: &App) -> usize {
    app.live
        .entries
        .iter()
        .filter(|entry| matches!(entry, Entry::Assistant { .. }))
        .count()
}

/// The owner's Windows test (rc.13): OpenCode installed, "does this work?"
/// ended in a red "OpenCode could not start." Now Auto answers the same
/// message, one grey line says so, and OpenCode isn't chosen again.
#[test]
fn opencode_failing_to_start_falls_back_to_auto() {
    computer(true, false);
    let mut app = live_app();
    app.submit("does this work?", &std::env::temp_dir());
    assert_eq!(kind(&app), Some("opencode"), "Auto chose OpenCode");
    finish(
        &mut app,
        Err("OpenCode could not start: batch file arguments are invalid".into()),
    );
    assert_eq!(kind(&app), Some("auto"), "Auto took the message");
    assert_eq!(user_entries(&app), 1, "{:?}", app.live.entries);
    assert!(
        app.live.notice.is_none(),
        "no red line: {:?}",
        app.live.notice
    );
    assert!(app.start_failed.contains(&Harness::OpenCode));
    let id = app.request_id;
    app.apply_update(Update::Routed {
        id,
        lane: Lane::Local,
        note: None,
    });
    finish(&mut app, answer("Yes, it works.", "big-pickle"));
    assert_eq!(answers(&app), 1);
    assert!(app.live.notice.is_none());
    assert_eq!(
        app.notice.as_deref(),
        Some(
            "OpenCode didn't start, so Auto answered. OpenCode could not start: batch file arguments are invalid."
        )
    );
    // The next message: OpenCode is never chosen again this run, and the
    // chat goes on through the door that answered.
    app.submit("hooray", &std::env::temp_dir());
    assert_eq!(kind(&app), Some("auto"));
    assert_eq!(app.live.door, Some(Lane::Local));
    computer(false, false);
}

/// The owner's turn 2: turn 1 answered through this computer's logins
/// with a model whose name looks like the gateway's; turn 2 demanded a
/// sign-in. The door is the one that really answered.
#[test]
fn turn_two_can_use_the_door_turn_one_answered_through() {
    computer(false, false);
    let mut app = live_app();
    app.submit("testing", &std::env::temp_dir());
    assert_eq!(kind(&app), Some("auto"));
    let id = app.request_id;
    app.apply_update(Update::Routed {
        id,
        lane: Lane::Local,
        note: None,
    });
    finish(
        &mut app,
        answer("Hello! Everything is working.", "google/gemini-3-flash"),
    );
    assert_eq!(app.live.door, Some(Lane::Local), "not the gateway");
    app.submit("hooray", &std::env::temp_dir());
    let Some(Work::Microcoder { execution, .. }) = app.request.as_ref().map(|r| &r.kind) else {
        panic!("an auto turn");
    };
    let route = plan(execution, false, Favor::default());
    assert_eq!(route.lanes.first(), Some(&Lane::Local));
    assert!(!route.lanes.contains(&Lane::Gateway), "signed out");
    computer(false, false);
}

/// A harness the person picked with Shift+Tab that fails falls back to
/// Auto for the turn, with one grey line; Claude Code, OpenCode, and an
/// ACP agent alike.
#[test]
fn a_picked_harness_that_fails_falls_back_to_auto() {
    for (harness, why, line) in [
        (
            Harness::OpenCode,
            "OpenCode could not start: not found",
            "OpenCode didn't start, so Auto answered. OpenCode could not start: not found.",
        ),
        (
            Harness::ClaudeCode,
            "Claude Code is out of usage until 11pm",
            "Claude Code couldn't answer, so Auto answered. Claude Code is out of usage until 11pm.",
        ),
    ] {
        computer(true, true);
        let mut app = live_app();
        app.next_pick = Some(Pick::Harness(harness));
        app.submit("does this work?", &std::env::temp_dir());
        assert_ne!(kind(&app), Some("auto"), "{harness:?} runs first");
        finish(&mut app, Err(why.into()));
        assert_eq!(kind(&app), Some("auto"), "{harness:?}");
        assert!(!app.live.pinned, "{harness:?}: back on Auto");
        finish(&mut app, answer("4", "big-pickle"));
        assert_eq!(app.notice.as_deref(), Some(line), "{harness:?}");
        assert!(app.live.notice.is_none(), "{harness:?}");
        assert_eq!(answers(&app), 1, "{harness:?}");
    }
    computer(false, false);
}

/// A fallback that can't answer either ends with the route's one message
/// (it names the fix), never a second harness or a loop.
#[test]
fn a_fallback_that_fails_too_ends_with_one_message() {
    computer(true, false);
    let mut app = live_app();
    app.submit("does this work?", &std::env::temp_dir());
    finish(&mut app, Err("OpenCode could not start: boom".into()));
    assert_eq!(kind(&app), Some("auto"));
    let ending = crate::route::Plan {
        lanes: vec![Lane::Local],
        notes: vec![],
        held: None,
        signed_in: false,
    }
    .explain(&["No model login on this computer can answer right now.".into()]);
    finish(&mut app, Err(ending.clone()));
    assert_eq!(app.request.is_none(), true, "no third try");
    assert_eq!(app.live.notice.as_deref(), Some(ending.as_str()));
    assert!(ending.contains("`coder login`"), "{ending}");
    assert!(app.notice.is_none(), "one message, not two");
    computer(false, false);
}

/// Turning a plugin off takes effect at the next message: a chat on
/// OpenCode with the OpenCode plugin or the ACP Subagents switch off never
/// starts OpenCode again; it goes on with Auto, and says so in grey.
#[test]
fn a_disabled_plugin_is_never_chosen() {
    for switch in ["opencode", "acp"] {
        computer(true, false);
        let mut app = live_app();
        app.submit("first", &std::env::temp_dir());
        assert_eq!(kind(&app), Some("opencode"), "{switch}");
        finish(&mut app, answer("Hi.", "opencode/big-pickle"));
        match switch {
            "opencode" => app.plugins.bundled.opencode = false,
            _ => app.plugins.bundled.acp = false,
        }
        app.submit("second", &std::env::temp_dir());
        assert_eq!(kind(&app), Some("auto"), "{switch}");
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|line| line.contains("Auto")),
            "{switch}: {:?}",
            app.notice
        );
        // And a new chat never routes there either.
        let mut fresh = live_app();
        match switch {
            "opencode" => fresh.plugins.bundled.opencode = false,
            _ => fresh.plugins.bundled.acp = false,
        }
        fresh.submit("hi", &std::env::temp_dir());
        assert_eq!(kind(&fresh), Some("auto"), "{switch}");
        assert!(
            !crate::harness_pick::local(&fresh).opencode,
            "{switch}: Shift+Tab never offers it"
        );
    }
    computer(false, false);
}

/// The App-level invariant over the harness worlds: OpenCode installed or
/// not, Claude Code signed in or not, each plugin on or off, Auto or a
/// Shift+Tab pick, and each failure (won't start, silent). Every turn ends
/// with an answer, a disabled harness never gets a request, and no red
/// line is left.
#[test]
fn every_harness_world_ends_with_an_answer() {
    let mut worlds = 0;
    for opencode in [false, true] {
        for claude in [false, true] {
            for opencode_plugin in [false, true] {
                for acp in [false, true] {
                    for claude_plugin in [false, true] {
                        for pick in [None, Some(Harness::OpenCode), Some(Harness::ClaudeCode)] {
                            for why in [
                                "OpenCode could not start: The system cannot find the file specified.",
                                "OpenCode didn't start answering within 60 seconds",
                            ] {
                                worlds += 1;
                                computer(opencode, claude);
                                let mut app = live_app();
                                app.plugins.bundled.opencode = opencode_plugin;
                                app.plugins.bundled.acp = acp;
                                app.plugins.bundled.claude = claude_plugin;
                                let offered =
                                    crate::harness_pick::options(&crate::harness_pick::local(&app));
                                if let Some(harness) = pick
                                    .filter(|harness| offered.contains(&Pick::Harness(*harness)))
                                {
                                    app.next_pick = Some(Pick::Harness(harness));
                                }
                                let world = format!(
                                    "opencode={opencode} claude={claude} plugins(opencode={opencode_plugin} acp={acp} claude={claude_plugin}) pick={pick:?} why={why}"
                                );
                                app.submit("does this work?", &std::env::temp_dir());
                                let mut steps = 0;
                                while let Some(started) = kind(&app) {
                                    steps += 1;
                                    assert!(steps <= 3, "no end: {world}");
                                    if started == "opencode" {
                                        assert!(
                                            opencode && opencode_plugin && acp,
                                            "a disabled OpenCode ran: {world}"
                                        );
                                    }
                                    if started == "claude" {
                                        assert!(
                                            claude_plugin,
                                            "a disabled Claude Code ran: {world}"
                                        );
                                    }
                                    if started == "auto" {
                                        finish(&mut app, answer("4", "big-pickle"));
                                    } else {
                                        finish(&mut app, Err(why.into()));
                                    }
                                }
                                assert_eq!(
                                    answers(&app),
                                    1,
                                    "{world}: {:?} {:?}",
                                    app.live.entries,
                                    app.live.notice
                                );
                                assert!(
                                    app.live.notice.is_none(),
                                    "{world}: {:?}",
                                    app.live.notice
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(worlds, 2 * 2 * 2 * 2 * 2 * 3 * 2);
    computer(false, false);
}

/// A harness that failed gets another try when its plugin is turned on
/// again, or when the person picks it.
#[test]
fn a_failed_harness_gets_another_try_when_picked() {
    computer(true, false);
    let mut app = live_app();
    app.start_failed.push(Harness::OpenCode);
    app.submit("hi", &std::env::temp_dir());
    assert_eq!(kind(&app), Some("auto"), "left out after failing");
    let mut picked = live_app();
    picked.start_failed.push(Harness::OpenCode);
    picked.next_pick = Some(Pick::Harness(Harness::OpenCode));
    picked.submit("hi", &std::env::temp_dir());
    assert_eq!(kind(&picked), Some("opencode"));
    computer(false, false);
}

/// #11690: while OpenCode sets itself up on its first run, the rail says
/// so beside Working; when setup fails and Auto answers, the grey line
/// gives OpenCode's own reason.
#[test]
fn opencodes_first_run_setup_is_on_the_rail_and_its_failure_in_the_grey_line() {
    computer(true, false);
    let mut app = live_app();
    app.next_pick = Some(Pick::Harness(Harness::OpenCode));
    app.submit("hi", &std::env::temp_dir());
    assert_eq!(kind(&app), Some("opencode"));
    let id = app.request_id;
    app.apply_update(Update::Setup {
        id,
        note: Some(crate::opencode_chat::SETUP_NOTE.to_owned()),
    });
    let label = crate::ui::working_label(&app).unwrap();
    assert!(label.contains("Working"), "{label}");
    assert!(
        label.ends_with(" · Setting up OpenCode (first time only)"),
        "{label}"
    );
    app.apply_update(Update::Setup { id, note: None });
    assert!(
        !crate::ui::working_label(&app)
            .unwrap()
            .contains("Setting up")
    );

    let why = "OpenCode couldn't finish setting up: npm ERR! network ETIMEDOUT";
    app.apply_update(Update::Setup {
        id,
        note: Some(crate::opencode_chat::SETUP_NOTE.to_owned()),
    });
    finish(&mut app, Err(why.into()));
    assert!(app.live.setting_up.is_none(), "a finished turn clears it");
    assert_eq!(kind(&app), Some("auto"), "Auto took the message");
    finish(&mut app, answer("Hello!", "big-pickle"));
    assert_eq!(
        app.notice.as_deref(),
        Some(
            "OpenCode didn't start, so Auto answered. OpenCode couldn't finish setting up: npm ERR! network ETIMEDOUT."
        )
    );
    computer(false, false);
}

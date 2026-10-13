use futures::future::BoxFuture;
use jev::decisions::{DecisionsRequest, DecisionsResponse};

use super::super::compose::Budget;
use super::super::outcome::Context;
use super::super::scripted::RECORDED_SESSION_SCHEMA;
use super::*;

/// The fixture plugin: `crates/plugin-hook-guard`, its pinned hook
/// documents and its built guest.
fn fixture(enabled: bool) -> Plugin {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-hook-guard");
    load("local:hook-guard", "Hook guard", enabled, &dir).unwrap()
}

/// Only the named hooks of the fixture.
fn only(mut plugin: Plugin, hooks: &[&str]) -> Plugin {
    plugin
        .hooks
        .retain(|hook| hooks.contains(&hook.doc.id.as_str()));
    plugin
}

struct Fixed(f64);

impl Decider for Fixed {
    fn decide(
        &self,
        request: DecisionsRequest,
    ) -> BoxFuture<'static, Result<DecisionsResponse, String>> {
        let name = match &request.questions[0] {
            jev::decisions::DecisionQuestion::Predicate { name, .. } => name.clone(),
            _ => String::new(),
        };
        let response = serde_json::from_value(json!({
            "id": "dec_1", "object": "decision", "model": "clef-flash",
            "answers": [{"type": "predicate", "name": name, "probability": self.0}],
            "openagents": {"door": "clef", "latency_ms": 180},
        }))
        .map_err(|error| error.to_string());
        Box::pin(async move { response })
    }
}

fn session(events: Value) -> ScriptedSession {
    ScriptedSession::from_json(
        &json!({"schema": RECORDED_SESSION_SCHEMA, "session_id": "s-1", "events": events})
            .to_string(),
    )
    .unwrap()
}

fn bash(event: &str, command: &str) -> Value {
    json!({"event": event, "tool": "Bash", "input": {"command": command}})
}

#[test]
fn the_package_pins_its_hooks_and_module() {
    let plugin = fixture(true);
    let ids: Vec<&str> = plugin
        .hooks
        .iter()
        .map(|hook| hook.doc.id.as_str())
        .collect();
    assert_eq!(ids, ["guard", "remind", "dry-run", "risky"]);
    // A changed module is refused.
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-hook-guard");
    for part in ["package.json", "hooks/guard.json"] {
        let to = dir.path().join(part);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(source.join(part), to).unwrap();
    }
    std::fs::create_dir_all(dir.path().join("wasm")).unwrap();
    std::fs::write(dir.path().join("wasm/hook-guard.wasm"), b"\0asm").unwrap();
    let record: Value =
        serde_json::from_str(&std::fs::read_to_string(source.join("package.json")).unwrap())
            .unwrap();
    let only_guard = json!({"v": 1, "slug": "hook-guard", "hooks": [record["hooks"][0]]});
    std::fs::write(dir.path().join("package.json"), only_guard.to_string()).unwrap();
    let error = load("local:x", "X", true, dir.path()).unwrap_err();
    assert!(error.contains("changed since it was pinned"), "{error}");
}

/// The issue's first check: a deny hook and a context hook from a plugin
/// run through the scripted backend, and every firing replays exactly.
#[test]
fn a_plugins_deny_and_context_hooks_run_in_a_scripted_session_and_replay() {
    let plugin = only(fixture(true), &["guard", "remind"]);
    let mut registry = Registry::new();
    let firings = Firings::default();
    let notes = register(
        &mut registry,
        &plugin,
        &Grants::default(),
        &admitted_by(&plugin),
        None,
        &firings,
    );
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(
        notes
            .iter()
            .all(|note| matches!(note, Note::Registered { .. }))
    );
    let recorded = session(json!([
        {"event": "SessionStart", "input": {"cwd": "/work"}},
        bash("PreToolUse", "git push --force origin main"),
        {"event": "PostToolUse", "tool": "Bash", "input": {"tool_input": {}, "tool_response": "pushed"}},
        bash("PreToolUse", "cargo test"),
        {"event": "PostToolUse", "tool": "Bash", "input": {"tool_input": {}, "tool_response": "ok"}},
    ]));
    let replay = recorded
        .replay(&mut Runner::new(registry, Budget::default()))
        .unwrap();
    // The force push is refused by the plugin's hook; its output is skipped.
    assert_eq!(
        replay.applied[1].permission,
        Permission::deny("Hook guard: no force pushes here. Push a new commit instead.")
    );
    assert_eq!(
        replay.applied[1].decided_by.as_deref(),
        Some("local:hook-guard/guard")
    );
    assert_eq!(replay.skipped, 1);
    // The plain command goes on (a plugin never allows), and the context
    // hook adds its note after it.
    assert_eq!(replay.applied[2].permission, Permission::NoOpinion);
    assert_eq!(
        replay.applied[3].context,
        [Context::new(
            "hook-guard",
            "Hook guard: check the command's output before you go on.",
            0
        )]
    );
    // Three firings, each with a receipt that replays exactly.
    let all = firings.all();
    assert_eq!(all.len(), 3);
    for firing in &all {
        assert_eq!(firing.receipt["schema"], plugin::RECEIPT_SCHEMA);
        let verdict = super::replay(firing, &plugin.hooks[0].wasm);
        assert_eq!(verdict.verification(), "passed", "{verdict:?}");
    }
}

/// The issue's second check: rewrite authority without the person's
/// opt-in is registered without it, and that is recorded.
#[test]
fn a_rewrite_needs_the_persons_grant() {
    let plugin = only(fixture(true), &["dry-run"]);
    let deploy = Call::tool(
        Event::PreToolUse,
        "Bash",
        json!({"command": "deploy --prod"}),
    );

    let mut registry = Registry::new();
    let notes = register(
        &mut registry,
        &plugin,
        &Grants::default(),
        &admitted_by(&plugin),
        None,
        &Firings::default(),
    );
    assert!(
        notes
            .iter()
            .any(|note| matches!(note, Note::RewriteWithheld { hook, .. } if hook == "local:hook-guard/dry-run")),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|note| matches!(note, Note::Registered { authority, .. } if authority.is_empty()))
    );
    let applied = Runner::new(registry, Budget::default()).fire(&deploy);
    assert!(applied.rewritten_by.is_empty());
    assert_eq!(applied.input["command"], "deploy --prod");

    let mut grants = Grants::default();
    grants.rewrite.insert(Grants::key(&plugin.id, "dry-run"));
    let mut registry = Registry::new();
    register(
        &mut registry,
        &plugin,
        &grants,
        &admitted_by(&plugin),
        None,
        &Firings::default(),
    );
    let applied = Runner::new(registry, Budget::default()).fire(&deploy);
    assert_eq!(applied.rewritten_by, ["local:hook-guard/dry-run"]);
    assert_eq!(applied.input["command"], "deploy --dry-run --prod");
    // The listing shows the rewrite and whether it is on.
    let off = listing(&plugin, &Grants::default()).join("\n");
    assert!(
        off.contains("Changing commands: off until you turn it on."),
        "{off}"
    );
    assert!(listing(&plugin, &grants)[0].contains("Changing commands: on."));
}

#[test]
fn an_off_plugin_or_an_unadmitted_disclosure_registers_nothing() {
    let mut registry = Registry::new();
    let notes = register(
        &mut registry,
        &fixture(false),
        &Grants::default(),
        &Admission::none(),
        None,
        &Firings::default(),
    );
    assert_eq!(
        notes,
        [Note::PluginOff {
            plugin: "local:hook-guard".into()
        }]
    );
    assert!(registry.is_empty());

    let plugin = fixture(true);
    let notes = register(
        &mut registry,
        &plugin,
        &Grants::default(),
        &Admission::none(),
        None,
        &Firings::default(),
    );
    assert_eq!(notes.len(), 4);
    assert!(
        notes
            .iter()
            .all(|note| matches!(note, Note::Refused { .. }))
    );
    assert!(registry.is_empty());
}

/// A guest asks for a decision through `oa_host.call`; the exchange is
/// kept, and the firing replays from it.
#[test]
fn a_plugin_hook_decides_through_the_host_and_replays() {
    let _lock = super::super::decide::LOG_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let log = tempfile::tempdir().unwrap();
    // SAFETY: tests that set this variable hold LOG_LOCK.
    unsafe { std::env::set_var(hook_log::log_var("decide"), log.path().join("decide.jsonl")) };
    let plugin = only(fixture(true), &["risky"]);
    let mut registry = Registry::new();
    let firings = Firings::default();
    let notes = register(
        &mut registry,
        &plugin,
        &Grants::default(),
        &admitted_by(&plugin),
        Some(Arc::new(Fixed(0.93))),
        &firings,
    );
    assert!(matches!(notes[0], Note::Registered { .. }), "{notes:?}");
    let applied = Runner::new(registry, Budget::default()).fire(&Call::tool(
        Event::PreToolUse,
        "Bash",
        json!({"command": "dd if=/dev/zero of=/dev/disk2"}),
    ));
    assert_eq!(
        applied.permission,
        Permission::ask("Hook guard: this command could delete data. Confirm it first.")
    );
    let firing = &firings.all()[0];
    assert_eq!(firing.profile, "snapshot-read");
    assert_eq!(firing.host_calls.len(), 1);
    assert_eq!(firing.host_calls[0].request["capability"], "decide");
    let verdict = super::replay(firing, &plugin.hooks[0].wasm);
    assert_eq!(verdict.verification(), "passed", "{verdict:?}");
    let logged = std::fs::read_to_string(log.path().join("decide.jsonl")).unwrap();
    assert!(logged.contains("local:hook-guard/risky"), "{logged}");

    // Without a decider the same hook can't decide: its guest refuses,
    // which is no opinion.
    let mut registry = Registry::new();
    let notes = register(
        &mut registry,
        &plugin,
        &Grants::default(),
        &admitted_by(&plugin),
        None,
        &Firings::default(),
    );
    assert!(
        notes
            .iter()
            .any(|note| matches!(note, Note::DecideWithheld { .. }))
    );
    let applied = Runner::new(registry, Budget::default()).fire(&Call::tool(
        Event::PreToolUse,
        "Bash",
        json!({"command": "dd if=/dev/zero of=/dev/disk2"}),
    ));
    assert_eq!(applied.permission, Permission::NoOpinion);
}

/// The extension evaluation path: one recorded session with and without
/// the plugin's hooks.
#[test]
fn a_plugins_tests_can_run_a_session_with_and_without_its_hooks() {
    let plugin = only(fixture(true), &["guard"]);
    let recorded = session(json!([bash("PreToolUse", "git push -f origin main")]));
    let (without, with, notes, firings) =
        with_and_without(&recorded, Registry::new, &plugin, &Grants::default(), None).unwrap();
    assert_eq!(without.applied[0].permission, Permission::NoOpinion);
    assert!(matches!(
        with.applied[0].permission,
        Permission::Deny { .. }
    ));
    assert!(matches!(notes[0], Note::Registered { .. }));
    assert_eq!(firings.all().len(), 1);
}

#[test]
fn a_plugin_only_tightens() {
    let all: BTreeSet<Authority> = [Authority::Deny, Authority::Context].into_iter().collect();
    let allow = HookOutcome::allow();
    assert_eq!(narrow(allow, &all).permission, Permission::NoOpinion);
    let ask = narrow(HookOutcome::ask("confirm"), &all);
    assert_eq!(ask.permission, Permission::NoOpinion);
    let ended = narrow(HookOutcome::stopping(Stop::End), &all);
    assert_eq!(ended.stop, Stop::Continue);
    let none: BTreeSet<Authority> = BTreeSet::new();
    let context = narrow(HookOutcome::context(Context::new("p", "note", 0)), &none);
    assert_eq!(context.context, None);
}

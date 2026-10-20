use super::*;
use rust_native::{View, ViewError};

fn words(node: &Node<Intent>, out: &mut Vec<String>) {
    match &node.element {
        Element::Text { value, .. } => out.push(value.clone()),
        Element::Button { label, .. } | Element::Choice { label, .. } => out.push(label.clone()),
        _ => {}
    }
    if let Element::Stack { children, .. }
    | Element::List { children, .. }
    | Element::Choice { children, .. } = &node.element
    {
        for child in children {
            words(child, out);
        }
    }
}

fn find<'a>(node: &'a Node<Intent>, key: &str) -> Option<&'a Node<Intent>> {
    if node.key == key {
        return Some(node);
    }
    match &node.element {
        Element::Stack { children, .. } | Element::List { children, .. } => {
            children.iter().find_map(|c| find(c, key))
        }
        _ => None,
    }
}

fn intent_of<'a>(node: &'a Node<Intent>, key: &str) -> &'a Intent {
    match &find(node, key)
        .unwrap_or_else(|| panic!("no {key}"))
        .element
    {
        Element::Button { intent, .. } | Element::Choice { intent, .. } => intent,
        other => panic!("{key} is {other:?}"),
    }
}

fn full_remote() -> Remote {
    let mut remote = Remote {
        computers: vec![Named::new("mac-1", "Mac mini"), Named::new("pc", "Desk")],
        computer: Some("mac-1".into()),
        titles: Load::Ready(Titles {
            harness: Some("codex".into()),
            model: None,
            harnesses: vec![
                Named::new("codex", "Codex"),
                Named::new("claude", "Claude Code"),
            ],
            models: vec![Named::new("gpt-mini", "GPT mini")],
        }),
        worktrees: Load::Ready(Worktrees {
            custom: true,
            effective: "/Users/pat/wt".into(),
            default: "/Users/pat/.openagents/worktrees".into(),
            environment: None,
            browsing: Some((
                "/Users/pat".into(),
                vec![Named::new("/Users/pat/src", "src")],
                false,
            )),
        }),
        archived: (0..30)
            .map(|ix| Archived {
                id: format!("c{ix}"),
                title: format!("Chat {ix}"),
                detail: "Mac mini · 2 days ago".into(),
            })
            .collect(),
        ..Remote::default()
    };
    remote.earn = earn::State {
        status: Load::Ready(serde_json::json!({"state": "serving"})),
        earnings: Load::Ready(serde_json::json!({"checked_today": 3, "earned_sats": 120})),
        settings: Load::Ready(serde_json::json!({"pause_on_battery": true})),
        ..earn::State::default()
    };
    remote.account = account::State {
        signed_in: true,
        label: Some("pat@example.com".into()),
        name: Some("pat".into()),
        keys: Load::Ready(vec![
            account::Key {
                pubkey: "aa".repeat(32),
                npub: "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq".into(),
            },
            account::Key {
                pubkey: "bb".repeat(32),
                npub: "npub1zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".into(),
            },
        ]),
        confirm_remove: Some("bb".repeat(32)),
        deleting: Some(account::Deleting::Ask),
        ..account::State::default()
    };
    remote
}

#[test]
fn every_page_is_a_valid_view_in_plain_words() {
    let prefs = Prefs::default();
    for remote in [Remote::default(), full_remote()] {
        for p in Page::ALL {
            let root = page(p, &prefs, &remote, &Paints::default());
            View::new_v3("settings", 1, root.clone())
                .validate()
                .unwrap_or_else(|e: ViewError| panic!("{p:?}: {e}"));
            let mut out = Vec::new();
            words(&root, &mut out);
            let text = out.join("\n");
            assert!(!text.is_empty());
            let found = oa_copy::violations(&text, &[]);
            assert!(found.is_empty(), "{p:?}: {found:?}");
        }
    }
}

#[test]
fn pages_have_the_desktop_labels_and_addresses() {
    for p in Page::ALL {
        assert_eq!(Page::from_slug(p.slug()), Some(p));
    }
    assert_eq!(Page::from_slug("conversations"), Some(Page::General));
    assert_eq!(Page::Archived.label(), "Archived sessions");
    let root = page(
        Page::General,
        &Prefs::default(),
        &full_remote(),
        &Paints::default(),
    );
    let mut out = Vec::new();
    words(&root, &mut out);
    for label in [
        "Send messages with",
        "Stop agent with Escape",
        "Thread naming",
        "Custom worktree location",
        "Use this folder",
    ] {
        assert!(out.iter().any(|w| w == label), "{label}: {out:?}");
    }
}

#[test]
fn device_choices_apply_and_survive_a_reload() {
    let mut prefs = Prefs::default();
    let root = page(
        Page::General,
        &prefs,
        &Remote::default(),
        &Paints::default(),
    );
    let send = intent_of(&root, "send-1").clone();
    assert!(prefs.apply(&send));
    assert!(prefs.send_with_modifier);
    assert!(!prefs.apply(&send), "an unchanged choice saves nothing");
    let root = page(
        Page::Notifications,
        &prefs,
        &Remote::default(),
        &Paints::default(),
    );
    prefs.apply(&intent_of(&root, "sound-toggle").clone());
    assert!(!prefs.sound_enabled);
    assert!(!prefs.plays(Sound::Completion));
    // A sub-sound can't change while the master switch is off.
    let root = page(
        Page::Notifications,
        &prefs,
        &Remote::default(),
        &Paints::default(),
    );
    assert!(matches!(
        find(&root, "sound-completion-toggle").unwrap().element,
        Element::Choice { enabled: false, .. }
    ));
    prefs.apply(&Intent::Files(FilesChange::ShowHidden(true)));
    assert!(prefs.files.show_hidden);
    // The files panel's own key and fields (`web/files`' Settings).
    let files = FilesPrefs::from_json(Some(&prefs.files.to_json()));
    assert_eq!(files, prefs.files);
    assert!(prefs.files.to_json().contains("\"showHidden\":true"));
    assert_eq!(
        FilesPrefs::from_json(Some(r#"{"autosaveDelayMs": 7}"#)).autosave_delay_ms,
        900
    );
    prefs.files = FilesPrefs::default();
    prefs.apply(&Intent::Appearance(appearance::Change::Mode(
        appearance::Mode::Light,
    )));
    let back = Prefs::from_json(Some(&prefs.to_json()));
    assert_eq!(back, prefs);
    assert_eq!(Prefs::from_json(Some("{not json")), Prefs::default());
    // A newer build's field is dropped; the rest stays.
    let extra = Prefs::from_json(Some(r#"{"sendWithModifier":true,"later":1}"#));
    assert!(extra.send_with_modifier);
}

#[test]
fn files_page_has_the_desktop_rows() {
    let mut prefs = Prefs::default();
    let root = page(Page::Files, &prefs, &Remote::default(), &Paints::default());
    assert!(
        find(&root, "files-delay").is_none(),
        "the delay shows with autosave on"
    );
    prefs.apply(intent_of(&root, "files-autosave-toggle"));
    let root = page(Page::Files, &prefs, &Remote::default(), &Paints::default());
    let mut out = Vec::new();
    words(&root, &mut out);
    for label in [
        "Autosave",
        "Autosave delay",
        "Word wrap",
        "Show hidden and ignored files",
        "1.5 s",
    ] {
        assert!(out.iter().any(|w| w == label), "{label}: {out:?}");
    }
    prefs.apply(intent_of(&root, "files-delay-3"));
    assert_eq!(prefs.files.autosave_delay_ms, 1_500);
}

#[test]
fn archived_pages_and_delete_all_ask_first() {
    let mut remote = full_remote();
    let root = page(
        Page::Archived,
        &Prefs::default(),
        &remote,
        &Paints::default(),
    );
    assert_eq!(
        intent_of(&root, "archived-0-unarchive"),
        &Intent::Unarchive("c0".into())
    );
    assert_eq!(intent_of(&root, "archived-next"), &Intent::ArchivedPage(1));
    assert_eq!(
        intent_of(&root, "archived-delete"),
        &Intent::ConfirmDeleteAll(true)
    );
    remote.archived_page = 1;
    remote.confirm_delete_all = true;
    let root = page(
        Page::Archived,
        &Prefs::default(),
        &remote,
        &Paints::default(),
    );
    assert_eq!(
        intent_of(&root, "archived-0-unarchive"),
        &Intent::Unarchive("c25".into())
    );
    assert_eq!(
        intent_of(&root, "archived-delete-yes"),
        &Intent::DeleteAllArchived
    );
}

#[test]
fn shortcuts_record_refuse_and_restore() {
    use shortcuts::{
        Change, browser_keeps, combo_from_keystroke_on, display_combo_on, platform_combo_on,
    };
    let mut prefs = Prefs::default();
    let keymap = &prefs.keymap;
    // Every web default is free and unique.
    for s in Shortcut::ALL {
        let combo = keymap.get(s);
        assert!(!browser_keeps(combo), "{s:?}");
        assert_eq!(keymap.shortcut_for(combo), Some(s));
        assert_eq!(keymap.refusal(s, combo, true), None, "{s:?}");
    }
    assert!(browser_keeps("mod-n") && browser_keeps("mod-3") && browser_keeps("ctrl-tab"));
    assert!(browser_keeps("mod-shift-a"));
    assert_eq!(keymap.shortcut_for("x"), None);
    for (key, shortcut) in [
        ("ArrowDown", Shortcut::NextSession),
        ("ArrowUp", Shortcut::PrevSession),
    ] {
        let combo = combo_from_keystroke_on(false, false, true, false, false, key).unwrap();
        assert_eq!(keymap.shortcut_for(&combo), Some(shortcut));
    }
    assert_eq!(
        combo_from_keystroke_on(true, false, false, false, true, "Meta"),
        None
    );
    assert_eq!(keymap.shortcut_for("mod-shift-a"), None);
    assert_eq!(keymap.get(Shortcut::CommandPalette), "mod-k");
    assert!(
        keymap
            .refusal(Shortcut::NewSession, "mod-k", false)
            .is_some()
    );
    let old = Prefs::from_json(Some(r#"{"keymap":{"archive_session":"mod-shift-a"}}"#));
    assert_eq!(old.keymap.get(Shortcut::ArchiveSession), "mod-alt-a");
    assert_eq!(
        keymap
            .refusal(Shortcut::NewSession, "mod-b", true)
            .as_deref(),
        Some("Cmd+B is already assigned to Toggle left sidebar.")
    );
    assert_eq!(
        keymap
            .refusal(Shortcut::NewSession, "mod-t", false)
            .as_deref(),
        Some("Your browser keeps Ctrl+T for itself.")
    );
    assert!(keymap.refusal(Shortcut::NewSession, "k", true).is_some());
    assert_eq!(
        combo_from_keystroke_on(false, true, false, true, false, "K").as_deref(),
        Some("mod-shift-k")
    );
    assert_eq!(
        combo_from_keystroke_on(true, true, false, false, false, "k").as_deref(),
        Some("ctrl-k")
    );
    assert_eq!(
        combo_from_keystroke_on(true, false, false, false, true, "cmd"),
        None
    );
    assert_eq!(platform_combo_on(true, "mod-alt-n"), "cmd-alt-n");
    assert_eq!(display_combo_on(false, "mod-alt-n"), "Ctrl+Alt+N");

    prefs.apply(&Intent::Shortcut(Change::Set(
        Shortcut::NewSession,
        "mod-alt-k".into(),
    )));
    assert_eq!(prefs.keymap.get(Shortcut::NewSession), "mod-alt-k");
    assert_eq!(
        prefs.keymap.shortcut_for("mod-alt-k"),
        Some(Shortcut::NewSession)
    );
    assert_eq!(prefs.keymap.shortcut_for("mod-alt-n"), None);
    let mut remote = Remote::default();
    remote.recording = Some(Shortcut::ToggleFiles);
    let root = page(Page::Shortcuts, &prefs, &remote, &Paints::default());
    assert_eq!(
        intent_of(&root, "shortcut-new-session-reset"),
        &Intent::Shortcut(Change::Reset(Shortcut::NewSession))
    );
    assert_eq!(
        intent_of(&root, "shortcut-toggle-files-combo"),
        &Intent::Shortcut(Change::StopRecording)
    );
    let back = Prefs::from_json(Some(&prefs.to_json()));
    assert_eq!(back.keymap.get(Shortcut::NewSession), "mod-alt-k");
    prefs.apply(&Intent::RestoreShortcuts);
    assert!(!prefs.keymap.customized());
}

#[test]
fn appearance_choices_frame_and_bound() {
    use appearance::{Change, WIDTH_MAX, WIDTH_MIN};
    let mut look = Appearance::default();
    look.apply(&Change::SetBackground("dunes.jpg".into()));
    for _ in 0..40 {
        look.apply(&Change::Frame(1, -1, 1));
    }
    let framing = look.background.as_ref().unwrap().framing;
    assert_eq!(
        (framing.focal_x, framing.focal_y, framing.zoom),
        (1., 0., 4.)
    );
    look.apply(&Change::ResetFraming);
    assert_eq!(
        look.background.as_ref().unwrap().framing,
        appearance::Framing::default()
    );
    look.apply(&Change::RemoveBackground);
    assert!(look.background.is_none());
    for _ in 0..100 {
        look.apply(&Change::Width(1));
    }
    assert_eq!(look.width(), WIDTH_MAX);
    for _ in 0..100 {
        look.apply(&Change::Width(-1));
    }
    assert_eq!(look.width(), WIDTH_MIN);
    look.conversation_width = 741;
    assert_eq!(look.width() % 16, WIDTH_MIN % 16);
    assert_eq!(
        appearance::Mode::System.resolve(true),
        openagents_theme::Appearance::Dark
    );
    assert_eq!(
        appearance::Mode::System.resolve(false),
        openagents_theme::Appearance::Light
    );
    // Every built-in theme is offered for its scheme.
    let root = page(
        Page::Appearance,
        &Prefs::default(),
        &Remote::default(),
        &Paints::default(),
    );
    assert_eq!(
        intent_of(&root, "appearance-mode-2"),
        &Intent::Appearance(Change::Mode(appearance::Mode::Dark))
    );
    assert!(find(&root, "appearance-dark-0").is_some());
    assert!(find(&root, "appearance-light-0").is_some());
}

#[test]
fn an_imported_theme_is_offered_duplicated_and_removed() {
    use appearance::{Change, LibraryTheme};
    let theme = LibraryTheme {
        id: "web-dusk".into(),
        name: "Dusk".into(),
        file: "dusk.json".into(),
        source: r##"{"name": "Dusk", "type": "dark", // a note
            "colors": {"editor.background": "#101018", "editor.foreground": "#e0e0f0",},}"##
            .into(),
    };
    let family = theme.compile().expect("compiles");
    assert_eq!(family.variants[0].id, "web-dusk");
    assert!(family.variants[0].appearance.is_dark());
    let mut look = Appearance::default();
    look.apply(&Change::Imported(theme));
    look.apply(&Change::Theme(
        openagents_theme::Appearance::Dark,
        "web-dusk".into(),
    ));
    look.apply(&Change::DuplicateTheme("web-dusk".into()));
    assert_eq!(look.library.len(), 2);
    assert_eq!(look.library[1].name, "Dusk copy");
    assert_eq!(look.families()[1].variants[0].id, look.library[1].id);
    // Only the source is stored, and it survives a reload.
    let back: Appearance = serde_json::from_str(&serde_json::to_string(&look).unwrap()).unwrap();
    assert_eq!(back, look);
    look.apply(&Change::RemoveTheme("web-dusk".into()));
    assert_eq!(look.library.len(), 1);
    assert_eq!(look.themes.dark, "coder-noir", "a removed theme falls back");
}

#[test]
fn account_page_asks_before_removing_and_deleting() {
    use account::{Action, Deleting};
    let remote = full_remote();
    let root = page(
        Page::Account,
        &Prefs::default(),
        &remote,
        &Paints::default(),
    );
    assert_eq!(remote.account.id().as_deref(), Some("pat@openagents.com"));
    assert_eq!(
        intent_of(&root, "account-id-profile"),
        &Intent::Account(Action::Open("/@pat".into()))
    );
    assert_eq!(
        intent_of(&root, "account-id-edit"),
        &Intent::Account(Action::Open(account::PROFILE_FORM.into()))
    );
    assert_eq!(
        intent_of(&root, "account-key-0-remove"),
        &Intent::Account(Action::ConfirmRemove(Some("aa".repeat(32))))
    );
    assert_eq!(
        intent_of(&root, "account-key-1-remove-yes"),
        &Intent::Account(Action::RemoveKey("bb".repeat(32)))
    );
    assert_eq!(
        intent_of(&root, "account-delete-confirm"),
        &Intent::Account(Action::Delete(false))
    );
    assert_eq!(
        account::refused("balance_remaining", ""),
        Deleting::PaidCredit
    );
    assert_eq!(
        account::refused("shared_workspace", "x"),
        Deleting::Notice(account::SHARED_TEAM_NOTICE.into())
    );
    // The last key has no Remove: nothing would sign in.
    let mut one = remote.account.clone();
    one.name = None;
    one.confirm_remove = None;
    if let Load::Ready(keys) = &mut one.keys {
        keys.truncate(1);
    }
    let remote = Remote {
        account: one,
        ..Remote::default()
    };
    let root = page(
        Page::Account,
        &Prefs::default(),
        &remote,
        &Paints::default(),
    );
    assert!(find(&root, "account-key-0-remove").is_none());
    // Signed out, the page offers sign-in and nothing else.
    let root = page(
        Page::Account,
        &Prefs::default(),
        &Remote::default(),
        &Paints::default(),
    );
    assert_eq!(
        intent_of(&root, "account-sign-in-button"),
        &Intent::Account(Action::SignIn)
    );
    assert!(find(&root, "account-sign-out-button").is_none());
    assert!(
        remote
            .account
            .profile_path()
            .unwrap()
            .starts_with("/@npub1")
    );
    assert_eq!(
        account::short("npub1abcdefghijklmnopqrstuvwxyz"),
        "npub1abcdefg…uvwxyz"
    );
}

#[test]
fn earn_reads_the_computer_and_turns_sharing_off() {
    let remote = full_remote();
    let root = page(Page::Earn, &Prefs::default(), &remote, &Paints::default());
    let mut out = Vec::new();
    words(&root, &mut out);
    for line in [
        "Serving",
        "3 checked decisions today",
        "₿120 earned so far · ₿0 paid to you",
        "Earnings go to this computer's wallet",
        "Turn off",
        "Pause on battery",
    ] {
        assert!(out.iter().any(|w| w == line), "{line} in {out:?}");
    }
    let mut off = remote.clone();
    off.earn.status = Load::Ready(serde_json::json!({"state": "off"}));
    assert!(!off.earn.sharing());
    off.earn.unavailable = true;
    let root = page(Page::Earn, &Prefs::default(), &off, &Paints::default());
    let mut out = Vec::new();
    words(&root, &mut out);
    assert!(out.iter().any(|w| w == earn::NOT_AVAILABLE));
    assert_eq!(Page::from_slug("earn"), Some(Page::Earn));
}

/// #12283: Providers lists the computer's agents with their updates and
/// accounts, and each control asks for its own action.
#[test]
fn providers_show_agents_updates_and_accounts() {
    use openagents_proto::HarnessId;
    use providers::{Action, Agent, State};
    let agents: Vec<Agent> = serde_json::from_value(serde_json::json!([
        {"id": "claude-code", "name": "Claude Code", "installed": true, "enabled": true},
        {"id": "codex", "name": "Codex", "installed": false, "canInstall": true},
    ]))
    .unwrap();
    let updates = serde_json::from_value(serde_json::json!([
        {"harness": "claude-code", "installedVersion": "2.1.0", "latestVersion": "2.2.0",
         "policy": "notify", "phase": "available", "canApply": true}
    ]))
    .unwrap();
    let accounts = serde_json::from_value(serde_json::json!([
        {"id": "a1", "harness": "claude-code", "email": "test@example.com", "planLabel": "Max",
         "active": false, "usageWindows": [{"label": "5-hour", "usedFraction": 0.4, "resetsAt": null}]}
    ]))
    .unwrap();
    let remote = Remote {
        computers: vec![Named::new("mac-1", "Studio Mac")],
        computer: Some("mac-1".into()),
        providers: State {
            agents: Load::Ready(agents),
            updates,
            accounts: Load::Ready(accounts),
            ..State::default()
        },
        ..Remote::default()
    };
    let root = page(
        Page::Providers,
        &Prefs::default(),
        &remote,
        &Paints::default(),
    );
    let json = serde_json::to_string(&root).unwrap();
    assert!(json.contains("Update available: 2.2.0"), "{json}");
    assert!(json.contains("5-hour 40%"));
    assert!(json.contains("Not installed"));
    let intents = intents(&root);
    for want in [
        Intent::Providers(Action::Install(HarnessId::Codex)),
        Intent::Providers(Action::Apply(HarnessId::ClaudeCode)),
        Intent::Providers(Action::Enable(HarnessId::ClaudeCode, false)),
        Intent::Providers(Action::Activate(HarnessId::ClaudeCode, "a1".into())),
        Intent::Providers(Action::AddAccount(HarnessId::ClaudeCode)),
        Intent::Providers(Action::Check(None)),
    ] {
        assert!(intents.contains(&want), "{want:?}");
    }
    assert_eq!(Page::from_slug("providers"), Some(Page::Providers));
}

fn intents(node: &Node<Intent>) -> Vec<Intent> {
    let mut out = Vec::new();
    let json = serde_json::to_value(node).unwrap();
    collect(&json, &mut out);
    out
}

fn collect(value: &serde_json::Value, out: &mut Vec<Intent>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, v) in map {
                if key == "intent" || key == "on_change" || key == "onChange" {
                    if let Ok(intent) = serde_json::from_value::<Intent>(v.clone()) {
                        out.push(intent);
                    }
                }
                collect(v, out);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect(v, out)),
        _ => {}
    }
}

/// A view the adapter draws, in plain words.
fn plain_view(root: &Node<Intent>) -> Vec<String> {
    View::new_v3("page", 1, root.clone())
        .validate()
        .unwrap_or_else(|e: ViewError| panic!("{e}"));
    let mut out = Vec::new();
    words(root, &mut out);
    let found = oa_copy::violations(&out.join("\n"), &[]);
    assert!(found.is_empty(), "{found:?}");
    out
}

#[test]
fn traces_list_and_open_one_with_share_and_delete() {
    use traces::{Action, AgentRow, Open, State, Summary, steps_of, view};
    let answer = serde_json::json!({"traces": [{
        "id": "t1", "title": "Fix the build", "agent": "coder", "model": "gpt-test",
        "steps": 2, "bytes": 10, "uploaded_unix": 1, "shared": true,
        "url": "/settings/traces/t1", "share_url": "https://openagents.com/trace/t1",
    }]});
    let mut state = State {
        list: Load::Ready(Summary::list(&answer)),
        ..State::default()
    };
    let out = plain_view(&view(&state, &Paints::default()));
    assert!(out.iter().any(|w| w == "Fix the build"));
    assert!(
        out.iter()
            .any(|w| w == "coder · gpt-test · 2 steps · Shared")
    );
    let document = serde_json::json!({"steps": [
        {"source": "user", "message": "Fix the build"},
        {"source": "agent", "message": [{"type": "text", "text": "Done."}],
         "tool_calls": [{"function_name": "shell"}]},
    ]});
    state.open = Some(Open {
        id: "t1".into(),
        agent: None,
        steps: Load::Ready(steps_of(&document)),
        agents: Load::Ready(AgentRow::list(&serde_json::json!({"agents": [
            {"id": "main", "parent": null, "title": "Fix the build", "steps": 2},
            {"id": "a2", "parent": "main", "title": "Reviewer", "steps": 1},
        ]}))),
    });
    state.confirm_delete = Some("t1".into());
    let root = view(&state, &Paints::default());
    let out = plain_view(&root);
    for line in [
        "Stop sharing",
        "Copy link",
        "Delete for good",
        "Reviewer",
        "Used shell",
        "Done.\nUsed shell",
    ] {
        assert!(out.iter().any(|w| w.contains(line)), "{line} in {out:?}");
    }
    assert_eq!(
        find(&root, "trace-agent-open-a2").map(|n| match &n.element {
            Element::Button { intent, .. } => intent.clone(),
            _ => panic!(),
        }),
        Some(Intent::Traces(Action::Open("t1".into(), Some("a2".into()))))
    );
}

#[test]
fn agents_show_what_runs_with_stop_message_and_the_landing_queue() {
    use agents::{Action, State, computers, landings, view};
    let boards = serde_json::json!({"computers": [
        {"name": "Studio", "online": true, "items": [
            {"id": "a1", "kind": "agent", "title": "#12 Fix the build", "status": "working", "line": "running tests"},
            {"id": "c1", "kind": "chat", "title": "A chat", "status": "working"},
            {"id": "l1", "kind": "landing", "title": "#13 Land it", "status": "done"},
        ]},
        {"name": "Idle", "online": false, "items": [{"id": "c2", "kind": "chat", "title": "x", "status": "done"}]},
    ]});
    let queue = serde_json::json!({"waiting": [
        {"id": "q1", "branch": "fix-build", "summary": "Fix the build", "state": "queued", "issue": 12}
    ], "landing": [], "failed": []});
    let state = State {
        computers: Load::Ready(computers(&boards)),
        landing: Load::Ready(landings(&queue)),
        busy: None,
    };
    assert_eq!(state.computers.ready().unwrap().len(), 1);
    let root = view(&state, &Paints::default());
    let out = plain_view(&root);
    for line in [
        "Studio",
        "#12 Fix the build",
        "Working · running tests",
        "Landing queue",
        "#12 Fix the build",
        "Waiting to land · fix-build",
    ] {
        assert!(out.iter().any(|w| w == line), "{line} in {out:?}");
    }
    assert!(!out.iter().any(|w| w == "A chat"));
    let intents = intents(&root);
    assert!(intents.contains(&Intent::Agents(Action::Stop("Studio".into(), "a1".into()))));
    assert!(intents.contains(&Intent::Agents(Action::Message(
        "Studio".into(),
        "a1".into()
    ))));
    assert!(!intents.contains(&Intent::Agents(Action::Stop("Studio".into(), "l1".into()))));
}

#[test]
fn projects_list_and_open_one_with_members_repositories_and_sources() {
    use projects::{Action, Members, Open, Project, State, sources, view};
    let list = serde_json::json!({"data": [{
        "id": "p1", "name": "Site", "counts": {"threads": 3},
        "repositories": [{"repository": "acme/site", "primary": true}, {"repository": "acme/docs", "primary": false}],
    }]});
    let mut state = State {
        list: Load::Ready(Project::list(&list)),
        ..State::default()
    };
    let out = plain_view(&view(&state, &Paints::default()));
    assert!(out.iter().any(|w| w == "acme/site · 3 chats"), "{out:?}");
    state.open = Some(Open {
        id: "p1".into(),
        members: Load::Ready(Members::from_value(&serde_json::json!({
            "role": "owner",
            "data": [
                {"account": "me", "name": "Pat", "role": "owner", "you": true},
                {"account": "sam", "name": "", "login": "sam", "role": "editor", "you": false},
            ],
            "invitations": [{"id": "i1", "role": "viewer", "expires_at": "2026-10-27T00:00:00Z"}],
        }))),
        sources: Load::Ready(sources(&serde_json::json!({"data": [
            {"id": "s1", "name": "Specs", "kind": "folder"}
        ]}))),
        link: Some("https://openagents.com/projects/join/abc".into()),
        confirm_leave: false,
        confirm_remove: None,
    });
    let root = view(&state, &Paints::default());
    let out = plain_view(&root);
    for line in [
        "Pat (you)",
        "sam",
        "Viewer link, until 2026-10-27",
        "Invitation link",
        "https://openagents.com/projects/join/abc",
        "Make primary",
        "Specs",
        "Add from Google Drive",
    ] {
        assert!(out.iter().any(|w| w == line), "{line} in {out:?}");
    }
    let intents = intents(&root);
    for want in [
        Action::SetRole("p1".into(), "sam".into(), "viewer".into()),
        Action::AskRemove(Some("sam".into())),
        Action::InviteLink("p1".into(), "editor".into()),
        Action::MakePrimary("p1".into(), "acme/docs".into()),
        Action::RemoveRepository("p1".into(), "acme/site".into()),
        Action::RemoveSource("p1".into(), "s1".into()),
        Action::Rename("p1".into()),
    ] {
        assert!(
            intents.contains(&Intent::Projects(want.clone())),
            "{want:?}"
        );
    }
    // The owner can't leave; a member can.
    assert!(!intents.contains(&Intent::Projects(Action::AskLeave(true))));
}

#[test]
fn a_lab_document_draws_the_shared_card_and_leaves_its_player_to_the_host() {
    use lab::{Action, State, view};
    let card: lab_kit::Card = serde_json::from_value(serde_json::json!({
        "kind": "video", "document": "d1", "version": 2, "title": "Launch cut",
        "summary": "Thirty seconds", "view": "video", "primary": "file_1",
    }))
    .unwrap();
    let state = State {
        space: "me".into(),
        card: Load::Ready(card),
    };
    assert!(state.player());
    let root = view(&state, &Paints::default());
    let out = plain_view(&root);
    assert!(out.iter().any(|w| w == "Launch cut · v2"), "{out:?}");
    assert!(out.iter().any(|w| w == "Thirty seconds"));
    assert!(intents(&root).contains(&Intent::Lab(Action::Visit(
        "/labs/video/d1?ui=html&v=2&space=me".into()
    ))));
    let report: lab_kit::Card = serde_json::from_value(serde_json::json!({
        "kind": "sim", "document": "d2", "version": 1, "title": "Run",
        "view": "report", "primary": "out.json",
    }))
    .unwrap();
    let state = State {
        space: "project:p1".into(),
        card: Load::Ready(report),
    };
    assert!(!state.player());
    assert!(
        intents(&view(&state, &Paints::default())).contains(&Intent::Lab(Action::Download(
            "/v1/labs/artifacts/out.json/content?space=project%3Ap1".into()
        )))
    );
}

#[test]
fn cloud_pages_hide_subscription_and_require_save_and_delete_confirmation() {
    let mut remote = Remote::default();
    remote.account.signed_in = true;
    remote.cloud.loaded = true;
    remote.cloud.available = true;
    let render = |p, remote: &Remote| page(p, &Prefs::default(), remote, &Paints::default());
    let root = render(Page::Claude, &remote);
    assert!(find(&root, "claude-save").is_none());
    let mut text = Vec::new();
    words(&root, &mut text);
    assert!(!text.join(" ").contains("Claude subscription"));
    remote.cloud.subscription_allowed = true;
    remote.cloud.provider = "claude_subscription_token".into();
    remote.cloud.pasted = true;
    assert!(find(&render(Page::Claude, &remote), "claude-save").is_none());
    remote.cloud.consent = true;
    assert_eq!(
        intent_of(&render(Page::Claude, &remote), "claude-save"),
        &Intent::Cloud(cloud::Action::Save)
    );
    remote.cloud.confirm_delete = true;
    let root = render(Page::Data, &remote);
    assert_eq!(
        intent_of(&root, "data-delete-confirm"),
        &Intent::Cloud(cloud::Action::Delete)
    );
    remote.cloud.confirm_delete = false;
    assert!(find(&render(Page::Data, &remote), "data-delete-confirm").is_none());
    for p in [Page::Claude, Page::Data] {
        let root = render(p, &remote);
        View::new_v3("cloud-settings", 1, root.clone())
            .validate()
            .unwrap();
        let mut text = Vec::new();
        words(&root, &mut text);
        assert_eq!(oa_copy::violations(&text.join("\n"), &[]), vec![]);
    }
}

#[test]
fn admin_entry_is_only_for_admins_and_drive_removal_requires_confirmation() {
    let paints = Paints::default();
    let mut account = account::State {
        signed_in: true,
        ..Default::default()
    };
    let node = column("account", account::page(&account, &paints));
    assert!(find(&node, "account-admin-open").is_none());
    account.admin = true;
    let node = column("account", account::page(&account, &paints));
    assert_eq!(
        intent_of(&node, "account-admin-open"),
        &Intent::Account(account::Action::Open("/admin/analytics?ui=gpui".into()))
    );
    let mut state = admin::State {
        report: Load::Ready(admin::drive_report(
            &serde_json::json!({"data":[{"email":"test+admin@example.com","added_at":"2026-10-20"}]}),
        )),
        ..Default::default()
    };
    let node = admin::view("Google Drive access", true, &state, &paints);
    let actions = intents(&node);
    assert!(actions.contains(&Intent::Admin(admin::Action::Add)));
    assert!(
        actions.contains(&Intent::Admin(admin::Action::ConfirmRemove(Some(
            "test+admin@example.com".into()
        ))))
    );
    assert!(!actions.contains(&Intent::Admin(admin::Action::Remove(
        "test+admin@example.com".into()
    ))));
    state.confirm_remove = Some("test+admin@example.com".into());
    let node = admin::view("Google Drive access", true, &state, &paints);
    assert!(
        intents(&node).contains(&Intent::Admin(admin::Action::Remove(
            "test+admin@example.com".into()
        )))
    );
    let mut lines = Vec::new();
    words(&node, &mut lines);
    assert!(oa_copy::violations(&lines.join(" "), &[]).is_empty());
    state.busy = true;
    assert!(
        !intents(&admin::view("Google Drive access", true, &state, &paints))
            .contains(&Intent::Admin(admin::Action::Add))
    );
    assert!(
        !intents(&admin::view("Analytics", false, &state, &paints)).contains(&Intent::Admin(
            admin::Action::Remove("test+admin@example.com".into())
        ))
    );
}

#[test]
fn account_pages_mount_the_editable_forms_inside_settings() {
    let mut remote = Remote::default();
    remote.memory.notes = Load::Ready(vec![]);
    remote.memory.edit(None);
    remote.schedules.listed = Load::Ready(schedules::Listed {
        computers: vec!["test-computer".into()],
        ..Default::default()
    });
    remote.schedules.edit(None);
    for (selected, field, save) in [
        (
            Page::Memory,
            "memory-body",
            Intent::Memory(memory::Action::Save),
        ),
        (
            Page::Schedules,
            "schedule-prompt",
            Intent::Schedules(schedules::Action::Save),
        ),
    ] {
        let root = page(selected, &Prefs::default(), &remote, &Paints::default());
        assert!(matches!(
            find(&root, field).unwrap().element,
            Element::Field { .. }
        ));
        assert!(intents(&root).contains(&save));
        View::new_v3("settings", 1, root).validate().unwrap();
    }
}

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
        assert_eq!(keymap.refusal(s, combo, true), None, "{s:?}");
    }
    assert!(browser_keeps("mod-n") && browser_keeps("mod-3") && browser_keeps("ctrl-tab"));
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
        "mod-k".into(),
    )));
    assert_eq!(prefs.keymap.get(Shortcut::NewSession), "mod-k");
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
    assert_eq!(back.keymap.get(Shortcut::NewSession), "mod-k");
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

use super::*;
use agent_fleet::{AgentRow, Status};

fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

fn row(id: &str, status: Status, started_ms: u64) -> AgentRow {
    AgentRow {
        id: id.into(),
        name: format!("{id}-task"),
        engine: "codex".into(),
        place: "this computer".into(),
        status,
        task: "fix the flaky login test".into(),
        started_ms,
        ended_ms: (status != Status::Running).then_some(started_ms + 90_000),
        earlier_seconds: 0,
        tokens: 31_200,
        cost_usd: Some(0.42),
        worktree: None,
        branch: None,
        parent_session: None,
        transcript: None,
        report: None,
        error: None,
        pending_messages: 0,
        runs: 1,
        run_started_ms: started_ms,
    }
}

fn keys(node: &Node<Intent>, out: &mut Vec<String>) {
    out.push(node.key.clone());
    if let Element::Stack { children, .. } = &node.element {
        for child in children {
            keys(child, out);
        }
    }
}

fn all_keys(node: &Node<Intent>) -> Vec<String> {
    let mut out = Vec::new();
    keys(node, &mut out);
    out
}

#[cfg(unix)]
#[test]
fn the_shell_is_the_person_s_login_shell_with_installed_coder_first() {
    let env = env_of(&[
        ("SHELL", "/usr/local/bin/fish"),
        ("PATH", "/usr/bin:/bin:/usr/bin"),
    ]);
    let shell = shell(
        &env,
        Path::new("/Users/kai"),
        Some(Path::new(
            "/Applications/OpenAgents.app/Contents/MacOS/coder",
        )),
    );
    assert_eq!(shell.program, PathBuf::from("/usr/local/bin/fish"));
    assert_eq!(shell.args, vec!["-l".to_owned()]);
    assert_eq!(shell.dir, PathBuf::from("/Users/kai"));
    assert_eq!(
        shell.path,
        "/Users/kai/.openagents/bin:/usr/bin:/bin:/Applications/OpenAgents.app/Contents/MacOS"
    );

    // No SHELL, or a relative one: the system's own.
    let shell = super::shell(&env_of(&[("SHELL", "fish")]), Path::new("/home/kai"), None);
    assert!(shell.program.is_absolute());
    assert_eq!(shell.path, "/home/kai/.openagents/bin");

    // OPENAGENTS_HOME moves the install folder, as the installer does.
    let shell = super::shell(
        &env_of(&[("OPENAGENTS_HOME", "/opt/oa")]),
        Path::new("/home/kai"),
        None,
    );
    assert_eq!(shell.path, "/opt/oa/bin");
}

#[test]
fn keys_become_the_bytes_a_terminal_sends() {
    let vt = Terminal::new(24, 80, 10);
    let bytes = |key: &str, text: Option<&str>, control: bool, shift: bool| {
        typed(&vt, key, text, control, control, false, shift)
    };
    assert_eq!(
        bytes("Enter", Some("\r"), false, false),
        Typed::Bytes(b"\r".to_vec())
    );
    assert_eq!(
        bytes("Backspace", None, false, false),
        Typed::Bytes(b"\x7f".to_vec())
    );
    assert_eq!(
        bytes("ArrowUp", None, false, false),
        Typed::Bytes(b"\x1b[A".to_vec())
    );
    assert_eq!(
        bytes("Tab", None, false, true),
        Typed::Bytes(b"\x1b[Z".to_vec())
    );
    assert_eq!(bytes("c", Some("c"), true, false), Typed::Bytes(vec![3]));
    assert_eq!(bytes("d", None, true, false), Typed::Bytes(vec![4]));
    assert_eq!(
        bytes("a", Some("a"), false, false),
        Typed::Bytes(b"a".to_vec())
    );
    assert_eq!(
        bytes("é", Some("é"), false, false),
        Typed::Bytes("é".as_bytes().to_vec())
    );
    assert_eq!(bytes("Shift", None, false, true), Typed::Unhandled);
    assert_eq!(
        bytes("F1", None, false, false),
        Typed::Bytes(b"\x1bOP".to_vec())
    );
}

#[test]
fn the_window_keeps_its_own_shortcuts_and_the_clipboard() {
    let vt = Terminal::new(24, 80, 10);
    if cfg!(target_os = "macos") {
        // Cmd alone: the window's.
        assert_eq!(
            typed(&vt, "v", Some("v"), false, true, false, false),
            Typed::Paste
        );
        assert_eq!(
            typed(&vt, "c", Some("c"), false, true, false, false),
            Typed::Copy
        );
        assert_eq!(
            typed(&vt, "b", Some("b"), false, true, false, false),
            Typed::Unhandled
        );
        // Ctrl goes to the shell.
        assert_eq!(
            typed(&vt, "c", None, true, true, false, false),
            Typed::Bytes(vec![3])
        );
    } else {
        assert_eq!(typed(&vt, "V", None, true, true, false, true), Typed::Paste);
        assert_eq!(typed(&vt, "C", None, true, true, false, true), Typed::Copy);
        assert_eq!(
            typed(&vt, "c", None, true, true, false, false),
            Typed::Bytes(vec![3])
        );
    }
}

#[test]
fn colors_follow_xterm() {
    assert_eq!(indexed(1), (205, 49, 49));
    assert_eq!(indexed(16), (0, 0, 0));
    assert_eq!(indexed(21), (0, 0, 255));
    assert_eq!(indexed(231), (255, 255, 255));
    assert_eq!(indexed(232), (8, 8, 8));
    assert_eq!(indexed(255), (238, 238, 238));
    let default = Color::rgb(1, 2, 3);
    assert_eq!(color(coder_vt::Color::Default, default), default);
    assert_eq!(
        color(coder_vt::Color::Rgb(9, 8, 7), default),
        Color::rgb(9, 8, 7)
    );
}

#[test]
fn the_pane_takes_half_the_content_and_leaves_the_chat_room() {
    let mut pane = TerminalPane::new(None);
    assert_eq!(pane.fit(1200.0), 600.0);
    assert_eq!(pane.fit(2400.0), MAX_WIDTH);
    // 700 points: the chat keeps 360, the pane 340.
    assert_eq!(pane.fit(700.0), 340.0);
    // Too narrow for both: only the button that brings it back.
    assert_eq!(pane.fit(600.0), 0.0);
    pane.open = false;
    assert_eq!(pane.fit(1200.0), 0.0);
}

#[test]
fn opening_the_window_shows_the_terminal_beside_the_chat() {
    let mut pane = TerminalPane::new(None);
    pane.fit(1200.0);
    let chat = stack("chat-body", Axis::Vertical, vec![]);
    let split = pane.beside(chat.clone());
    let keys = all_keys(&split);
    for key in [
        "terminal-split",
        "chat-body",
        "terminal-pane",
        "terminal-title",
        SCREEN,
        "terminal-hide",
        "terminal-agents-title",
        "terminal-agents-none",
    ] {
        assert!(keys.iter().any(|k| k == key), "{key} in {keys:?}");
    }
    assert!(!keys.iter().any(|k| k == "terminal-show"));

    // Hidden: the chat, and the button that brings it back.
    pane.open = false;
    pane.fit(1200.0);
    let keys = all_keys(&pane.beside(chat));
    assert!(keys.iter().any(|k| k == "terminal-show"));
    assert!(!keys.iter().any(|k| k == SCREEN));
}

#[test]
fn the_agents_panel_lists_every_coder_s_agents_with_stop_for_running_ones() {
    let home = tempfile::tempdir().unwrap();
    let dir = agent_fleet::board::dir(home.path());
    let mut first = agent_fleet::board::Publisher::new(&dir, 100, None);
    first
        .publish(&[
            row("agent-1", Status::Done, 1_000),
            row("agent-2", Status::Running, 2_000),
        ])
        .unwrap();
    let mut second = agent_fleet::board::Publisher::new(&dir, 200, None);
    second
        .publish(&[row("agent-1", Status::Running, 3_000)])
        .unwrap();

    let lines = agents(&dir, |_, _| true);
    let order: Vec<(u32, &str)> = lines
        .iter()
        .map(|line| (line.pid, line.row.id.as_str()))
        .collect();
    assert_eq!(
        order,
        vec![(200, "agent-1"), (100, "agent-2"), (100, "agent-1")]
    );
    assert_eq!(
        lines[2].words(0),
        "agent-1-task · Codex · done · 1m 30s · 31.2k tokens · $0.42"
    );

    let mut pane = TerminalPane::new(Some(dir.clone()));
    // Process ids 100 and 200 stand in for two running `coder`s.
    pane.alive = |_, _| true;
    let now = Instant::now();
    assert!(pane.refresh_agents(now));
    assert!(!pane.refresh_agents(now), "read at most once a second");
    pane.fit(1200.0);
    let keys = all_keys(&pane.pane(10_000));
    assert!(keys.iter().any(|k| k == "terminal-agent-200-agent-1-stop"));
    assert!(keys.iter().any(|k| k == "terminal-agent-100-agent-2-stop"));
    assert!(keys.iter().any(|k| k == "terminal-agent-100-agent-1-line"));
    assert!(!keys.iter().any(|k| k == "terminal-agent-100-agent-1-stop"));
    assert!(!keys.iter().any(|k| k == "terminal-agents-none"));

    // Stop leaves the request for the process that runs it.
    pane.stop(100, "agent-2");
    assert!(dir.join("100.agent-2.stop").exists());

    // A process that ended leaves the panel.
    drop(second);
    assert!(pane.refresh_agents(now + AGENTS_EVERY));
    assert_eq!(pane.agent_lines().len(), 2);
}

#[test]
fn engines_are_named_in_plain_words() {
    assert_eq!(engine_name("codex"), "Codex");
    assert_eq!(engine_name("claude"), "Claude Code");
    assert_eq!(engine_name("microcoder"), "Coder");
    assert_eq!(engine_name("opencode"), "opencode");
    for engine in ["codex", "claude", "grok", "microcoder", "jev", "host"] {
        assert!(
            crate::words::banned_in(engine_name(engine)).is_empty(),
            "{engine}"
        );
    }
}

/// Nothing the pane says is a word the desktop never shows.
#[test]
fn the_pane_says_nothing_banned() {
    let mut pane = TerminalPane::new(None);
    pane.fit(1200.0);
    let mut words = Vec::new();
    fn collect(node: &Node<Intent>, out: &mut Vec<String>) {
        match &node.element {
            Element::Text { value, .. } => out.push(value.clone()),
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Stack { children, .. } => children.iter().for_each(|c| collect(c, out)),
            _ => {}
        }
    }
    for phase in [
        Phase::Idle,
        Phase::Running,
        Phase::Ended("The shell exited.".into()),
        Phase::Failed(NOT_STARTED.into()),
    ] {
        pane.phase = phase;
        collect(&pane.pane(0), &mut words);
    }
    for text in words {
        assert!(crate::words::banned_in(&text).is_empty(), "{text}");
    }
}

#[test]
fn output_draws_and_an_exit_says_how_it_ended() {
    let mut pane = TerminalPane::new(None);
    pane.apply(Body::Output {
        seq: 1,
        data: b"hello \x1b[31mred\x1b[0m\r\n".to_vec(),
    });
    assert!(pane.text().starts_with("hello red"));
    pane.apply(Body::Exit {
        seq: 2,
        exit: coder_pty::wire::Exit {
            cause: Cause::Exited,
            code: Some(2),
            signal: None,
        },
    });
    assert_eq!(
        pane.phase,
        Phase::Ended("The shell exited with code 2.".into())
    );
    let keys = all_keys(&pane.pane(0));
    assert!(keys.iter().any(|k| k == "terminal-restart"));
}

#[test]
fn a_click_takes_the_keyboard_and_the_wheel_scrolls_back() {
    let mut pane = TerminalPane::new(None);
    let before = pane.version();
    assert!(pane.input(SurfaceInput::Down {
        x: 1.0,
        y: 1.0,
        shift: false,
    }));
    assert!(pane.focused);
    assert_ne!(pane.version(), before);
    for line in 0..100 {
        pane.apply(Body::Output {
            seq: line + 1,
            data: format!("line {line}\r\n").into_bytes(),
        });
    }
    assert!(pane.input(SurfaceInput::Wheel {
        x: 0.0,
        y: 0.0,
        dx: 0.0,
        dy: FONT_SIZE * LINE_EM * 3.0,
    }));
    assert_eq!(pane.scroll, 3);
    // New output returns to the live screen.
    pane.apply(Body::Output {
        seq: 200,
        data: b"more\r\n".to_vec(),
    });
    assert_eq!(pane.scroll, 0);
}

/// A real shell on a real PTY: what is typed into the pane runs, and its
/// output comes back to the screen.
#[cfg(unix)]
#[test]
fn a_typed_command_runs_in_a_real_shell() {
    let dir = tempfile::tempdir().unwrap();
    let shell = Shell {
        program: PathBuf::from("/bin/sh"),
        args: Vec::new(),
        dir: dir.path().to_path_buf(),
        path: std::env::var("PATH").unwrap_or_default(),
    };
    let mut pane = TerminalPane::new(None);
    pane.start(&shell, || {});
    assert_eq!(pane.phase, Phase::Running, "{:?}", pane.phase);
    pane.commit("echo pane-$((40+2))");
    assert!(matches!(
        pane.key("Enter", Some("\r"), false, false, false, false),
        Typed::Bytes(_)
    ));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pane.text().lines().any(|line| line.trim() == "pane-42") {
        assert!(Instant::now() < deadline, "no output: {:?}", pane.text());
        std::thread::sleep(Duration::from_millis(20));
        pane.poll();
    }
    pane.commit("exit 3");
    pane.key("Enter", Some("\r"), false, false, false, false);
    let deadline = Instant::now() + Duration::from_secs(10);
    while pane.phase == Phase::Running {
        assert!(Instant::now() < deadline, "never exited");
        std::thread::sleep(Duration::from_millis(20));
        pane.poll();
    }
    assert_eq!(
        pane.phase,
        Phase::Ended("The shell exited with code 3.".into())
    );
    pane.reset();
    assert_eq!(pane.phase, Phase::Idle);
}

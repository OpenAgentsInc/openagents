use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;
use crate::preflight::{Checks, HarnessInUse, Problem, harness_login, preflight};

fn host(root: &Path, platform: Platform) -> Host {
    Host {
        platform,
        home: root.join("home"),
        config_home: root.join("home/.config"),
    }
}

fn spec(root: &Path, platform: Platform) -> ServiceSpec {
    ServiceSpec {
        host: host(root, platform),
        exe: PathBuf::from("/Applications/OpenAgents.app/Contents/MacOS/openagents"),
        data_dir: root.join("data"),
        ipc_port: 27854,
        env: vec![
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("OPENAGENTS_EDGE_URL".into(), "https://e?a=1&b=2".into()),
            // Pinned values come from the spec, never the captured copy.
            ("OPENAGENTS_DATA_DIR".into(), "/elsewhere".into()),
        ],
    }
}

/// Records what the service manager was asked, and starts the "engine".
#[derive(Default)]
struct Recorder {
    calls: RefCell<Vec<bool>>,
    started: Cell<bool>,
    fail: Option<String>,
}

impl ServiceManager for Recorder {
    fn start(&self, _spec: &ServiceSpec, reload: bool) -> Result<(), String> {
        self.calls.borrow_mut().push(reload);
        if let Some(message) = &self.fail {
            return Err(message.clone());
        }
        self.started.set(true);
        Ok(())
    }
}

#[test]
fn plist_runs_headless_for_its_data_folder_and_leaves_keepers_alone() {
    let root = tempfile::tempdir().unwrap();
    let spec = spec(root.path(), Platform::MacOs);
    let plist = spec.render();
    let label = spec.label();
    assert!(label.starts_with("com.openagents.desktop.engine."));
    assert!(plist.contains(&format!("<key>Label</key><string>{label}</string>")));
    assert!(plist.contains(
        "<string>/Applications/OpenAgents.app/Contents/MacOS/openagents</string>\n      <string>headless</string>"
    ));
    assert!(plist.contains("<string>https://e?a=1&amp;b=2</string>"));
    let data = spec.data_dir.to_string_lossy().into_owned();
    assert!(plist.contains(&format!(
        "<key>OPENAGENTS_DATA_DIR</key><string>{data}</string>"
    )));
    assert!(!plist.contains("/elsewhere"));
    assert!(plist.contains("<key>OPENAGENTS_IPC_PORT</key><string>27854</string>"));
    assert!(plist.contains("<key>AbandonProcessGroup</key><true/>"));
    assert!(plist.contains("<key>SuccessfulExit</key><false/>"));
    assert!(
        plist.contains(
            "<key>AssociatedBundleIdentifiers</key><string>com.openagents.desktop</string>"
        )
    );
    assert!(plist.contains("background-engine.log"));
    assert_eq!(
        spec.file_path(),
        root.path()
            .join("home/Library/LaunchAgents")
            .join(format!("{label}.plist"))
    );
}

#[test]
fn unit_keeps_agents_on_stop_and_escapes_specifiers() {
    let root = tempfile::tempdir().unwrap();
    let mut spec = spec(root.path(), Platform::Linux);
    spec.exe = PathBuf::from("/opt/Open Agents/openagents");
    spec.env
        .push(("RUST_LOG".into(), "info,x=\"debug\" 100%".into()));
    let unit = spec.render();
    assert!(unit.contains("KillMode=process\n"));
    assert!(unit.contains("ExecStart=\"/opt/Open Agents/openagents\" headless\n"));
    assert!(unit.contains("Environment=\"RUST_LOG=info,x=\\\"debug\\\" 100%%\"\n"));
    assert!(unit.contains("Environment=\"OPENAGENTS_IPC_PORT=27854\"\n"));
    assert!(unit.contains("Restart=on-failure\n"));
    assert!(unit.contains("WantedBy=default.target\n"));
    assert!(spec.unit_name().starts_with("openagents-desktop-engine-"));
    assert_eq!(
        spec.file_path(),
        root.path()
            .join("home/.config/systemd/user")
            .join(spec.unit_name())
    );

    // The installer's versioned binary runs through its `current` link.
    spec.exe = spec.host.home.join(".openagents/app/0.3.0/openagents");
    assert!(
        spec.render()
            .contains("ExecStart=%h/.openagents/app/current/openagents headless\n")
    );
}

#[test]
fn each_data_folder_gets_its_own_service() {
    let a = folder_id(Path::new(
        "/Users/a/Library/Application Support/OpenAgents Desktop",
    ));
    let b = folder_id(Path::new("/Users/a/scratch/OpenAgents Desktop"));
    assert_ne!(a, b);
    assert_eq!(a.len(), 12);
    assert_eq!(
        a,
        folder_id(Path::new(
            "/Users/a/Library/Application Support/OpenAgents Desktop"
        ))
    );
}

#[test]
fn decision_attaches_waits_starts_or_installs() {
    use Action::*;
    let o = |listening, locked, file| Observed {
        engine_listening: listening,
        data_dir_locked: locked,
        service_file: file,
    };
    let bg = EngineMode::Background;
    let em = EngineMode::Embedded;
    // A running engine always wins, whatever the mode.
    assert_eq!(decide(bg, o(true, true, FileState::Missing)), Attach);
    assert_eq!(decide(em, o(true, false, FileState::Different)), Attach);
    // Starting: never rewrite the file under an engine.
    assert_eq!(
        decide(bg, o(false, true, FileState::Different)),
        WaitForEngine
    );
    assert_eq!(
        decide(em, o(false, true, FileState::Missing)),
        WaitForEngine
    );
    assert_eq!(decide(bg, o(false, false, FileState::Current)), Start);
    assert_eq!(
        decide(bg, o(false, false, FileState::Missing)),
        InstallAndStart
    );
    assert_eq!(
        decide(bg, o(false, false, FileState::Different)),
        InstallAndStart
    );
    assert_eq!(decide(em, o(false, false, FileState::Current)), Embed);
}

#[test]
fn mode_prefers_env_then_choice_then_build_kind() {
    let mac = Some(Platform::MacOs);
    let bg = EngineMode::Background;
    let em = EngineMode::Embedded;
    assert_eq!(
        engine_mode(None, Some("background"), None, false, false).0,
        em
    );
    assert_eq!(
        engine_mode(mac, Some("embedded"), Some(bg), false, false).0,
        em
    );
    assert_eq!(engine_mode(mac, Some("junk"), Some(em), false, false).0, em);
    assert_eq!(engine_mode(mac, None, Some(bg), true, true).0, bg);
    assert_eq!(engine_mode(mac, None, None, true, false).0, em);
    assert_eq!(engine_mode(mac, None, None, false, true).0, em);
    assert_eq!(engine_mode(mac, None, None, false, false).0, bg);
    assert!(is_source_build(Path::new(
        "/src/apps/openagents/target/debug/openagents"
    )));
    assert!(is_source_build(Path::new(
        "/private/var/folders/x/T/AppTranslocation/1/d/OpenAgents.app/Contents/MacOS/openagents"
    )));
    assert!(!is_source_build(Path::new(
        "/Applications/OpenAgents.app/Contents/MacOS/openagents"
    )));
}

#[test]
fn preference_round_trips_in_the_data_folder() {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("data");
    assert_eq!(read_preference(&data), None);
    write_preference(&data, EngineMode::Background).unwrap();
    assert_eq!(read_preference(&data), Some(EngineMode::Background));
    write_preference(&data, EngineMode::Embedded).unwrap();
    assert_eq!(read_preference(&data), Some(EngineMode::Embedded));
}

#[test]
fn ensure_installs_then_starts_then_attaches() {
    let root = tempfile::tempdir().unwrap();
    for platform in [Platform::MacOs, Platform::Linux] {
        let spec = spec(root.path(), platform);
        let manager = Recorder::default();
        let run = |manager: &Recorder| {
            ensure(Ensure {
                spec: &spec,
                mode: EngineMode::Background,
                data_dir_locked: false,
                manager,
                listening: &|| manager.started.get(),
                wait: Duration::from_millis(200),
            })
        };

        // First launch: no file, no engine.
        assert_eq!(
            run(&manager),
            Outcome::Running {
                started: true,
                installed: true
            }
        );
        assert_eq!(spec.file_state(), FileState::Current);
        assert_eq!(*manager.calls.borrow(), vec![true]);

        // The engine answers: attach, no calls.
        assert!(run(&manager).attach());
        assert_eq!(manager.calls.borrow().len(), 1);

        // Engine stopped, file current: start without reloading.
        manager.started.set(false);
        assert_eq!(
            run(&manager),
            Outcome::Running {
                started: true,
                installed: false
            }
        );
        assert_eq!(*manager.calls.borrow(), vec![true, false]);

        // An app update changes the program path: rewrite and reload.
        let mut moved = spec.clone();
        moved.exe = PathBuf::from("/Applications/OpenAgents 2.app/Contents/MacOS/openagents");
        assert_eq!(moved.file_state(), FileState::Different);
    }
}

#[test]
fn ensure_reports_failures_and_embedding_without_touching_files() {
    let root = tempfile::tempdir().unwrap();
    let spec = spec(root.path(), Platform::MacOs);

    let manager = Recorder::default();
    let outcome = ensure(Ensure {
        spec: &spec,
        mode: EngineMode::Embedded,
        data_dir_locked: false,
        manager: &manager,
        listening: &|| false,
        wait: Duration::from_millis(50),
    });
    assert!(matches!(outcome, Outcome::Embed(_)));
    assert!(!spec.file_path().exists());
    assert!(manager.calls.borrow().is_empty());

    let failing = Recorder {
        fail: Some("launchctl kickstart failed".into()),
        ..Recorder::default()
    };
    let outcome = ensure(Ensure {
        spec: &spec,
        mode: EngineMode::Background,
        data_dir_locked: false,
        manager: &failing,
        listening: &|| false,
        wait: Duration::from_millis(50),
    });
    assert_eq!(
        outcome,
        Outcome::Failed("launchctl kickstart failed".into())
    );
    assert!(!outcome.attach());

    // Started but never answers.
    let silent = Recorder::default();
    let outcome = ensure(Ensure {
        spec: &spec,
        mode: EngineMode::Background,
        data_dir_locked: false,
        manager: &silent,
        listening: &|| false,
        wait: Duration::from_millis(50),
    });
    assert!(matches!(outcome, Outcome::Failed(_)));
}

#[test]
fn preflight_names_logins_the_service_cannot_read() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path();
    let claude = harness_login("claude-code").unwrap();
    let codex = harness_login("codex").unwrap();
    let harnesses = [
        HarnessInUse {
            login: claude,
            installed: true,
        },
        HarnessInUse {
            login: codex,
            installed: true,
        },
    ];
    let no_keychain = |_: &str| false;

    let problems = preflight(&Checks {
        home,
        signed_in: true,
        sign_in_saved: false,
        harnesses: &harnesses,
        keychain_has: &no_keychain,
    });
    assert_eq!(
        problems,
        vec![
            Problem::SignInNotSaved,
            Problem::SignedOut {
                name: "Claude Code",
                login_command: "claude"
            },
            Problem::SignedOut {
                name: "Codex",
                login_command: "codex login"
            },
        ]
    );
    for problem in &problems {
        let text = problem.to_string();
        assert!(text.ends_with("then try again."), "{text}");
    }

    // A Keychain item and a file are each a login.
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::write(home.join(".codex/auth.json"), "{}").unwrap();
    let keychain = |item: &str| item == "Claude Code-credentials";
    assert!(
        preflight(&Checks {
            home,
            signed_in: true,
            sign_in_saved: true,
            harnesses: &harnesses,
            keychain_has: &keychain,
        })
        .is_empty()
    );

    let missing = [HarnessInUse {
        login: claude,
        installed: false,
    }];
    assert_eq!(
        preflight(&Checks {
            home,
            signed_in: false,
            sign_in_saved: false,
            harnesses: &missing,
            keychain_has: &keychain,
        }),
        vec![Problem::NotInstalled {
            name: "Claude Code"
        }]
    );
}

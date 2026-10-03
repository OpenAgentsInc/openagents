## Prepared source context

Source commit: `b39787700107a1605b3412c8653fad887adfbbf8`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/coder/src/task/autostart.rs:3370-3383 — tests::a_requested_engine_without_capacity_falls_back

File SHA-256: `d9d59286ffd381c9752e378f0520954571ae03bb4afd1a58115efc4f26782bb6`

```rust
    /// A requested engine that is out of capacity falls back to the next
    /// admitted route (#10076).
    #[test]
    fn a_requested_engine_without_capacity_falls_back() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        exhaust_codex(&s.store);
        let task = "9".repeat(64);
        s.inbox.prefer(&task, Engine::Codex);
        s.inbox.create(&task, "host", &create("allowed")).unwrap();
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "claude");
    }
```

### crates/coder/src/task/autostart.rs:3413-3456 — tests::a_devices_requested_engine_outside_the_policy_falls_back_and_says_why

File SHA-256: `d9d59286ffd381c9752e378f0520954571ae03bb4afd1a58115efc4f26782bb6`

```rust
    /// A device cannot widen the owner's policy (#10081): an engine the
    /// policy admits no route for adds none, the task runs on the policy's
    /// own first route, and its summary says why in plain words.
    #[test]
    fn a_devices_requested_engine_outside_the_policy_falls_back_and_says_why() {
        use nostr::cj_conversation::Engine;
        let s = setup();
        routed(1).save(&s.root).unwrap();
        let task = "8".repeat(64);
        let asked = TaskCreate {
            engine: Some(Engine::Devin),
            ..create("allowed")
        };
        s.inbox.create(&task, "phone", &asked).unwrap();
        let stored = Store::open(&s.store).unwrap().show(&task).unwrap();
        assert_eq!(
            stored.intent.configuration.model.as_deref(),
            Some("gpt-6-luna")
        );
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "codex");
        assert_eq!(configuration.model, "gpt-6-luna");
        assert_eq!(configuration.fallbacks.len(), 1);
        assert_eq!(configuration.fallbacks[0].provider, "claude");
        let started = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "started")
            .unwrap();
        assert_eq!(started.requested.as_deref(), Some("devin"));
        assert_eq!(started.passed, Some(coder_host::Passed::NotAllowed));
        let note = s.inbox.note(&task).unwrap();
        assert_eq!(
            note,
            coder_host::Note::Requested {
                asked: Engine::Devin,
                runs: "Codex",
                why: coder_host::Passed::NotAllowed,
            }
        );
        assert_eq!(
            note.headline(),
            "You asked for Devin; it is not one of the engines this computer's Coder policy allows, so Codex is running."
        );
    }
```

### crates/coder/src/task/autostart.rs:2348-2364 — default_controller

File SHA-256: `d9d59286ffd381c9752e378f0520954571ae03bb4afd1a58115efc4f26782bb6`

```rust
/// The engine beside the running program, else the one its macOS app
/// bundle ships, else the installed one.
///
/// # Errors
/// Names where it looked.
pub fn default_controller() -> std::result::Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok());
    let home = std::env::var_os("HOME").map(PathBuf::from);
    controller_candidates(exe.as_deref(), home.as_deref())
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            "no microcoder beside coder or in ~/.openagents/bin; pass --controller".into()
        })
}
```

### crates/coder/src/task/autostart.rs:4100-4122 — tests::a_failing_usage_probe_falls_back_to_refusal_only_routing

File SHA-256: `d9d59286ffd381c9752e378f0520954571ae03bb4afd1a58115efc4f26782bb6`

```rust
    #[test]
    fn a_failing_usage_probe_falls_back_to_refusal_only_routing() {
        let s = setup();
        CLOCK.with(|clock| clock.set(1_790_572_210));
        probed(90).save(&s.root).unwrap();
        let task = "8".repeat(64);
        s.inbox.create(&task, "phone", &create("allowed")).unwrap();
        // Offline probes: the first admitted route starts, as without probes.
        let configuration = launched_grant(&s, 0).adapter_configuration.unwrap();
        assert_eq!(configuration.provider, "codex");
        let book = usage::Book::load_with(&s.store, login);
        assert_eq!(
            book.entry(Provider::Claude).unwrap().failure,
            Some(usage::Failure::Network)
        );
        let detail = journal(&s.root)
            .into_iter()
            .find(|entry| entry.event == "usage")
            .unwrap()
            .detail
            .unwrap();
        assert!(detail.contains("unknown (probe: network)"), "{detail}");
    }
```

### crates/coder/src/task/autostart.rs:4360-4428 — tests::an_older_engine_is_named_in_plain_words

File SHA-256: `d9d59286ffd381c9752e378f0520954571ae03bb4afd1a58115efc4f26782bb6`

```rust
    /// #10113: a dev build ran an older `~/.openagents/bin/microcoder`
    /// that could not read the newer grant, and the person saw the
    /// engine's raw JSON. A refusal is a plain sentence: an older engine
    /// is named with how to update it, and any other refusal shows the
    /// engine's own words, never its JSON.
    #[test]
    #[cfg(unix)]
    fn an_older_engine_is_named_in_plain_words() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let engine = |name: &str, stderr: &str| {
            let path = temp.path().join(name);
            std::fs::write(
                &path,
                format!("#!/bin/sh\nprintf '%s\\n' '{stderr}' >&2\nexit 2\n"),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Engine {
                adapter: adapter::NAME.into(),
                controller: path,
                model: "gpt-6-luna".into(),
                effort: None,
                max_steps: None,
                wall_seconds: None,
                memory_bytes: 1 << 30,
                write_workspace: false,
                decision_endpoint: "https://api.typesafe.ai".into(),
                decision_model: "jev-latest".into(),
                routes: Vec::new(),
                usage_probe: None,
                access: adapter::Access::Full,
            }
        };
        let grant = temp.path().join("grant.json");
        let store = temp.path().join("tasks");
        let older = engine(
            "microcoder",
            &format!(r#"{{"error":"{}"}}"#, owner::GRANT_SHAPE),
        );
        let said = Process.launch(&older, &grant, &store).unwrap_err();
        assert_eq!(
            said,
            format!(
                "the Coder engine at {} is older than this program; reinstall the OpenAgents \
                 app or rebuild microcoder",
                older.controller.display()
            )
        );
        assert!(!said.contains('{'), "{said}");
        let other = engine(
            "other",
            r#"{"error":"no task store path","cause":"configuration"}"#,
        );
        assert_eq!(
            Process.launch(&other, &grant, &store).unwrap_err(),
            "the controller refused the launch: no task store path"
        );
        let bare = engine("bare", "{}");
        assert_eq!(
            Process.launch(&bare, &grant, &store).unwrap_err(),
            "the controller refused the launch without saying why"
        );
        let words = engine("words", "cannot open the grant");
        assert_eq!(
            Process.launch(&words, &grant, &store).unwrap_err(),
            "the controller refused the launch: cannot open the grant"
        );
    }
```

### crates/openagents-desktop/src/platform/linux.rs:1361-1394 — tests::linux_login_agent_with_systemd

File SHA-256: `1760dbee82b25cde33aca9b6795b61ae30613c5de0b9014897a002a502704c3b`

```rust
    /// The real user manager: registers a harmless stand-in (`sleep`) as
    /// the unit, checks that it runs, and removes it. Refuses to touch a
    /// real OpenAgents unit. `cargo test -p openagents-desktop --bin
    /// openagents-desktop -- --ignored linux_login_agent_with_systemd`
    #[test]
    #[ignore = "needs a systemd user manager; writes and removes the real unit"]
    fn linux_login_agent_with_systemd() {
        let agent = LoginAgent::current().unwrap();
        assert!(
            !agent.unit_path().exists(),
            "an OpenAgents unit is installed; not touching it"
        );
        let sleep = [
            "/run/current-system/sw/bin/sleep",
            "/usr/bin/sleep",
            "/bin/sleep",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
        .unwrap();
        let command = HostCommand {
            program: sleep,
            args: vec!["300".into()],
        };
        let mut systemctl = UserSystemctl;
        agent
            .register(&command, "/usr/bin:/bin", &mut systemctl)
            .unwrap();
        let status = agent.status(&mut systemctl);
        agent.unregister(&mut systemctl).unwrap();
        assert_eq!(status, (true, true));
        assert!(!agent.unit_path().exists());
    }
```

### crates/openagents-desktop/src/platform/linux.rs:943-995 — tests::linux_host_command_runs_coder_from_the_install

File SHA-256: `1760dbee82b25cde33aca9b6795b61ae30613c5de0b9014897a002a502704c3b`

```rust
    #[test]
    fn linux_host_command_runs_coder_from_the_install() {
        let deb = HostCommand::for_install(None, Path::new("/usr/lib/openagents"), &Keys::Keychain);
        assert_eq!(deb.program, PathBuf::from("/usr/lib/openagents/coder"));
        assert_eq!(
            deb.args,
            ["host", "serve", "--keychain", "--iroh", "--control"]
        );
        let appimage = HostCommand::for_install(
            Some(Path::new("/home/kai/Apps/OpenAgents-x86_64.AppImage")),
            Path::new("/tmp/.mount_OpenAgXYZ/usr/lib/openagents"),
            &Keys::Keychain,
        );
        assert_eq!(
            appimage.program,
            PathBuf::from("/home/kai/Apps/OpenAgents-x86_64.AppImage")
        );
        assert_eq!(
            appimage.args,
            [
                "coder",
                "host",
                "serve",
                "--keychain",
                "--iroh",
                "--control"
            ]
        );
        // A relative $APPIMAGE is ignored rather than trusted.
        let relative = HostCommand::for_install(
            Some(Path::new("x.AppImage")),
            Path::new("/opt/openagents"),
            &Keys::Keychain,
        );
        assert_eq!(relative.program, PathBuf::from("/opt/openagents/coder"));
        // No Secret Service: the keys are private files.
        let files = HostCommand::for_install(
            None,
            Path::new("/usr/lib/openagents"),
            &Keys::Files(PathBuf::from("/home/kai/.openagents/host-keys")),
        );
        assert_eq!(
            files.args,
            [
                "host",
                "serve",
                "--keys",
                "/home/kai/.openagents/host-keys",
                "--iroh",
                "--control"
            ]
        );
    }
```

### crates/microcoder/src/repository.rs:666-672 — AgentEngine::binary

File SHA-256: `66fbbe54d333abb8c7140b8bcee5ceb673efe1fcff479b984f30cbc544e7cab6`

```rust
    fn binary(self) -> Result<PathBuf, Unusable> {
        match self {
            AgentEngine::Devin => devin::binary().map_err(|why| (StartCause::Devin, why)),
            AgentEngine::OpenCode => opencode::binary().map_err(|why| (StartCause::OpenCode, why)),
            AgentEngine::Grok => grok::binary().map_err(|why| (StartCause::Grok, why)),
        }
    }
```

### crates/openagents-desktop/src/platform/linux.rs:997-1013 — tests::linux_unit_quotes_every_word

File SHA-256: `1760dbee82b25cde33aca9b6795b61ae30613c5de0b9014897a002a502704c3b`

```rust
    #[test]
    fn linux_unit_quotes_every_word() {
        let command = HostCommand {
            program: PathBuf::from("/home/kai/My Apps/Open$Agents%.AppImage"),
            args: vec!["coder".into(), "host".into(), "serve".into()],
        };
        let unit = render_unit(&command, "/usr/bin:/home/kai/.local/bin").unwrap();
        assert!(unit.contains(
            "ExecStart=\"/home/kai/My Apps/Open$$Agents%%.AppImage\" \"coder\" \"host\" \"serve\"\n"
        ));
        assert!(unit.contains("Environment=\"PATH=/usr/bin:/home/kai/.local/bin\"\n"));
        assert!(unit.contains("WantedBy=default.target\n"));
        assert!(unit.contains("Restart=on-failure\n"));
        // A host that could not start itself again is back within a second.
        assert!(unit.contains("RestartSec=1\n"));
        assert!(unit.contains("UMask=0077\n"));
    }
```

### crates/openagents-desktop/src/platform/linux.rs:1029-1091 — tests::linux_login_agent_registers_and_unregisters

File SHA-256: `1760dbee82b25cde33aca9b6795b61ae30613c5de0b9014897a002a502704c3b`

```rust
    #[test]
    fn linux_login_agent_registers_and_unregisters() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let agent = LoginAgent::new(unit_dir(None, dir.path()));
        let mut systemctl = Recorder::default();
        assert_eq!(agent.status(&mut systemctl), (false, false));
        let command =
            HostCommand::for_install(None, Path::new("/usr/lib/openagents"), &Keys::Keychain);
        agent
            .register(&command, "/usr/bin", &mut systemctl)
            .unwrap();
        let path = dir.path().join(".config/systemd/user").join(UNIT);
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        let enable = format!("enable --now {UNIT}");
        let reset = format!("reset-failed {UNIT}");
        assert_eq!(
            systemctl.calls,
            ["daemon-reload", reset.as_str(), enable.as_str()]
        );
        assert_eq!(agent.status(&mut systemctl), (true, true));

        // The same command again does not restart the host.
        systemctl.calls.clear();
        agent
            .register(&command, "/usr/bin", &mut systemctl)
            .unwrap();
        assert_eq!(
            systemctl.calls,
            ["daemon-reload", reset.as_str(), enable.as_str()]
        );

        // A moved install rewrites the unit and restarts the host onto it.
        systemctl.calls.clear();
        let moved = HostCommand::for_install(None, Path::new("/opt/openagents"), &Keys::Keychain);
        agent.register(&moved, "/usr/bin", &mut systemctl).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("/opt/openagents/coder")
        );
        let restart = format!("restart {UNIT}");
        assert_eq!(
            systemctl.calls,
            [
                "daemon-reload",
                reset.as_str(),
                enable.as_str(),
                restart.as_str()
            ]
        );

        systemctl.calls.clear();
        agent.unregister(&mut systemctl).unwrap();
        assert!(!path.exists());
        let disable = format!("disable --now {UNIT}");
        assert_eq!(systemctl.calls, [disable.as_str(), "daemon-reload"]);
        assert_eq!(agent.status(&mut systemctl), (false, false));
        agent.unregister(&mut systemctl).unwrap();
    }
```

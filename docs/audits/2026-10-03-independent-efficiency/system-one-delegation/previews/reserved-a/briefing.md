## Prepared source context

Source commit: `f056853c6508581e5fc6f3c09a99edba6b05f212`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/coder/src/runtime.rs:7888-7961 — tests::a_snapshot_read_guest_lists_only_its_granted_files

File SHA-256: `b4fdc8986d825f2bf18b5df4cad45ac6a9e3bc915e3df785f7baae0099ffa927`

```rust
    /// A `snapshot-read` guest lists the files its read scope names and
    /// nothing else in the workspace.
    #[tokio::test]
    async fn a_snapshot_read_guest_lists_only_its_granted_files() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        std::fs::create_dir_all(root.join("docs/deep")).unwrap();
        std::fs::write(root.join("docs/a.md"), "alpha").unwrap();
        std::fs::write(root.join("docs/deep/b.md"), "beta").unwrap();
        std::fs::write(root.join("notes.txt"), "granted").unwrap();
        std::fs::write(root.join("secret.txt"), "not granted").unwrap();
        let runtime = workspace_runtime(root);
        let outline = plugin::encode_base64(&fixture("outline.wasm"));
        let program = module_program(
            json!({
                "profile": "snapshot-read",
                "operation": "outline",
                "read": ["docs", "notes.txt"],
                "bytes_base64": outline
            }),
            json!({}),
        );
        runtime
            .admit(&program)
            .expect("a scoped reader is admitted");
        let run = runtime
            .run(
                &program,
                &Inputs::read("outline", "stub-local"),
                &Grant::all(),
                None,
            )
            .await;
        assert!(run.finished(), "{:?}", run.stopped);
        let output: Value = serde_json::from_str(&run.steps[0].output).unwrap();
        assert_eq!(
            output["value"]["entries"],
            json!([
                "workspace/docs/a.md",
                "workspace/docs/deep/b.md",
                "workspace/notes.txt"
            ])
        );

        // Without a declared scope the guest is granted nothing.
        let unscoped = module_program(
            json!({"profile": "snapshot-read", "operation": "outline", "bytes_base64": outline}),
            json!({}),
        );
        let run = runtime
            .run(
                &unscoped,
                &Inputs::read("outline", "stub-local"),
                &Grant::all(),
                None,
            )
            .await;
        assert!(run.finished(), "{:?}", run.stopped);
        let output: Value = serde_json::from_str(&run.steps[0].output).unwrap();
        assert_eq!(output["value"]["entries"], json!([]));

        // A scope that climbs out of the workspace is refused at admission.
        for path in ["../elsewhere", "/etc", "docs/../../x", ""] {
            let climbing = module_program(
                json!({"profile": "snapshot-read", "read": [path], "bytes_base64": outline}),
                json!({}),
            );
            assert_eq!(
                runtime.admit(&climbing).unwrap_err().code,
                "scope_invalid",
                "{path:?}"
            );
        }
    }
```

### crates/coder-boundary/src/snapshot.rs:291-295 — Snapshot::observe_within

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Observes a tree under stated bounds. The result is complete only
    /// if the whole tree fit inside them.
    pub fn observe_within(root: &Path, limits: Limits) -> Snapshot {
        walk(root, limits)
    }
```

### crates/openagents-desktop/src/shell.rs:1253-1260 — DesktopApp::fullscreen_changed

File SHA-256: `693144b26cbdc865674de595fda22e6dbc2d8cf2bbe87ce84d14266b9cb32a34`

```rust
    fn fullscreen_changed(&mut self, fullscreen: bool) {
        if let Some(state) = &mut self.navigation
            && state.fullscreen != fullscreen
        {
            state.fullscreen = fullscreen;
            self.present();
        }
    }
```

### crates/coder-boundary/src/snapshot.rs:238-245 — Snapshot

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
/// One directory tree as it was observed: every entry under one
/// canonical root, or the faults that kept the observation partial.
#[derive(Debug)]
pub struct Snapshot {
    root: PathBuf,
    entries: BTreeMap<PathBuf, Entry>,
    faults: Vec<Fault>,
}
```

### crates/coder-boundary/src/snapshot.rs:286-289 — Snapshot::observe

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Observes a tree, bounded by [`Limits::default`].
    pub fn observe(root: &Path) -> Snapshot {
        Self::observe_within(root, Limits::default())
    }
```

### crates/coder-boundary/src/snapshot.rs:263-273 — Snapshot::recall_digests

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Adds the file digests an earlier process kept at `path`
    /// ([`Snapshot::remember_digests`]) to this process's reuse, and
    /// returns how many. A digest is reused only for a file whose device,
    /// inode, length, mode, and modification and status-change times all
    /// still match those it was hashed under, as for a digest this process
    /// took itself. Keep the file where only this user can write it. A
    /// missing or malformed file adds none.
    #[cfg(unix)]
    pub fn recall_digests(path: &Path) -> usize {
        observe::recall(path)
    }
```

### crates/coder-boundary/src/snapshot.rs:315-320 — Snapshot::is_complete

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Whether the observation is whole. Only a complete snapshot may
    /// ground a [`Verdict::Clean`].
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.faults.is_empty()
    }
```

### crates/openagents-desktop/src/shell.rs:5219-5260 — coder_events::every_event_type_renders_in_the_desktop_transcript

File SHA-256: `693144b26cbdc865674de595fda22e6dbc2d8cf2bbe87ce84d14266b9cb32a34`

```rust
    #[test]
    fn every_event_type_renders_in_the_desktop_transcript() {
        let directory =
            std::env::var_os("OPENAGENTS_CODER_CAPTURE_DIR").map(std::path::PathBuf::from);
        let mut drawn = std::collections::BTreeSet::new();
        for (name, lines, state) in cases() {
            let mut app = window(&lines, state);
            let (frame, _) = rust_native_desktop::capture(&mut app, 1200.0, 840.0, 1.0);
            let text = outline(app.chat.as_ref().unwrap().transcript_rows());
            check_snapshot(name, &text);
            // Every event the case read shows: its row names it.
            for line in &lines {
                let key = format!("coder-{}", line.seq);
                let shown = text.contains(&key) || {
                    let rows = app.chat.as_ref().unwrap().transcript_rows();
                    rows.iter().any(|row| row.key == key)
                };
                if shown {
                    drawn.insert(line.event.name());
                }
            }
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                std::fs::write(directory.join(format!("{name}.png")), frame.png().unwrap())
                    .unwrap();
            }
        }
        // Progress shows as the working line; a reply as the message; an
        // output inside its command's row. The rest are rows of their own.
        for name in [
            "coder_started",
            "step",
            "provider_switched",
            "question",
            "approval",
            "result",
            "failure",
            "stopped",
        ] {
            assert!(drawn.contains(name), "{name} draws no row: {drawn:?}");
        }
    }
```

### crates/coder-boundary/src/snapshot.rs:256-261 — Snapshot::matches_link

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Whether this retained link target matches the exact observation.
    #[must_use]
    pub fn matches_link(&self, path: &Path, target: &Path) -> bool {
        matches!(self.entries.get(path), Some(Entry::Link { target: observed, .. })
            if observed == target.as_os_str())
    }
```

### crates/coder-boundary/src/snapshot.rs:297-301 — Snapshot::root

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// The canonical root the snapshot was taken of.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
```

### crates/coder-boundary/src/snapshot.rs:248-254 — Snapshot::matches_file

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Whether these retained bytes match the exact file observation. A later
    /// filesystem read alone cannot establish this identity.
    #[must_use]
    pub fn matches_file(&self, path: &Path, bytes: &[u8]) -> bool {
        matches!(self.entries.get(path), Some(Entry::File { length, digest, .. })
            if *length == bytes.len() as u64 && *digest == <[u8; 32]>::from(Sha256::digest(bytes)))
    }
```

### crates/coder-boundary/src/snapshot.rs:328-369 — Snapshot::digest

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// A digest over every recorded entry and every fault, for a record
    /// that wants to name the observation rather than repeat it.
    #[must_use]
    pub fn digest(&self) -> String {
        let mut sha = Sha256::new();
        for (path, entry) in &self.entries {
            sha.update(path.as_os_str().as_encoded_bytes());
            sha.update([0]);
            match entry {
                Entry::Directory { mode, .. } => {
                    sha.update(b"directory");
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
                Entry::File {
                    length,
                    digest,
                    mode,
                    ..
                } => {
                    sha.update(b"file");
                    sha.update(length.to_le_bytes());
                    sha.update(digest);
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
                Entry::Link { target, mode, .. } => {
                    sha.update(b"symlink");
                    sha.update(target.as_encoded_bytes());
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
                Entry::Other { kind, mode, .. } => {
                    sha.update(kind.as_bytes());
                    sha.update(mode.map(u32::to_le_bytes).unwrap_or_default());
                }
            }
            sha.update([0xff]);
        }
        for fault in &self.faults {
            sha.update(fault.to_string().as_bytes());
            sha.update([0xff]);
        }
        format!("{:x}", sha.finalize())
    }
```

### crates/coder-boundary/src/snapshot.rs:303-307 — Snapshot::len

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// How many entries the walk recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
```

### crates/coder-boundary/src/snapshot.rs:275-284 — Snapshot::remember_digests

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Writes the file digests this process may reuse to `path`, mode
    /// `0600`, replacing it whole, for a later process to recall.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    #[cfg(unix)]
    pub fn remember_digests(path: &Path) -> std::io::Result<()> {
        observe::remember(path)
    }
```

### crates/openagents-desktop/src/shell.rs:674-691 — DesktopApp::start

File SHA-256: `693144b26cbdc865674de595fda22e6dbc2d8cf2bbe87ce84d14266b9cb32a34`

```rust
    fn start(&mut self, waker: Waker) {
        if let Some(chat) = &mut self.chat {
            chat.start(waker.clone());
        }
        crate::menubar::start(waker.clone());
        if self.live {
            crate::updates::start(waker.clone());
            crate::native::start(waker.clone());
        }
        if self.live {
            self.screen_lock = Some(ScreenLock::start(waker.clone()));
        }
        if let Runner::Pending(context) = &mut self.runner
            && let Some(context) = context.take()
        {
            self.runner = Runner::Background(Worker::start(context, waker));
        }
    }
```

### crates/coder-boundary/src/snapshot.rs:309-313 — Snapshot::is_empty

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// Whether the root held no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
```

### crates/coder-boundary/src/snapshot.rs:322-326 — Snapshot::faults

File SHA-256: `ba26e38d3310fe9c33d01caa6063a0c51688d91dcab2c33f6e81ab4ace517d2f`

```rust
    /// The faults the walk hit, in the order it hit them.
    #[must_use]
    pub fn faults(&self) -> &[Fault] {
        &self.faults
    }
```

### crates/coder-boundary/tests/snapshot.rs:405-415 — observation_is_refused_where_no_follow_cannot_be_promised

File SHA-256: `69ba38da872909cbce6e251f9a663505668544d79441d68efe76e4312c14418b`

```rust
/// Where no-follow cannot be promised there is no observation at all —
/// a refusal, not a quiet partial walk.
#[cfg(not(any(unix, windows)))]
#[test]
fn observation_is_refused_where_no_follow_cannot_be_promised() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "one");
    let snapshot = Snapshot::observe(dir.path());
    assert!(!snapshot.is_complete());
    assert!(compare(&snapshot, &snapshot).is_unverifiable());
}
```

### crates/coder/tests/desktop_local_coder.rs:219-231 — desktop_handoff_keeps_the_phone_prompt_project_policy_and_restart_identity::Launcher::launch

File SHA-256: `7d074863534e55e4d73e0cc2d9b907ed7e79e21a427d68ec1a8fd2b1195c9168`

```rust
        fn launch(
            &self,
            _: &autostart::Engine,
            grant: &Path,
            _: &Path,
        ) -> Result<autostart::Launched, String> {
            let grant = coder::task::owner::Grant::parse(&std::fs::read(grant).unwrap()).unwrap();
            self.0.lock().unwrap().push(grant.task_id);
            Ok(autostart::Launched {
                owner_process: std::process::id(),
                grant_digest: "sha256:fixture".into(),
            })
        }
```

### crates/openagents-desktop/src/shell.rs:5164-5179 — coder_events::check_snapshot

File SHA-256: `693144b26cbdc865674de595fda22e6dbc2d8cf2bbe87ce84d14266b9cb32a34`

```rust
    fn check_snapshot(name: &str, actual: &str) {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("snapshots")
            .join(format!("{name}.txt"));
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&path, actual).unwrap();
            return;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!("no snapshot {name}; run with UPDATE_SNAPSHOTS=1. The view is:\n{actual}")
        });
        assert_eq!(
            expected, actual,
            "the {name} snapshot differs; run with UPDATE_SNAPSHOTS=1 to record it"
        );
    }
```

### crates/coder/tests/desktop_local_coder.rs:216-217 — desktop_handoff_keeps_the_phone_prompt_project_policy_and_restart_identity::Launcher

File SHA-256: `7d074863534e55e4d73e0cc2d9b907ed7e79e21a427d68ec1a8fd2b1195c9168`

```rust
    #[derive(Clone)]
    struct Launcher(Arc<Mutex<Vec<String>>>);
```

## Prepared source context

Source commit: `4705102273140a5f381fb75e17a29965662629c8`. These are complete syntax declarations selected from a bounded candidate pool. They are starting points, not a complete dependency graph or a proof that the requirements are covered. Imports, callers, other modules, macros, cfg behavior, and unselected tests may need inspection. Read additional source whenever this material is insufficient.

### crates/background/src/rule.rs:222-293 — disk

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
/// The built-in disk cleanup rule with the spec's default policy.
#[must_use]
pub fn disk() -> Rule {
    Rule {
        schema: SCHEMA.into(),
        id: "disk".into(),
        name: "Disk cleanup".into(),
        version: 1,
        origin: Origin::BuiltIn,
        enabled: true,
        paused_until: None,
        triggers: vec![
            Trigger::Interval { every_secs: 300 },
            Trigger::Threshold,
            Trigger::TaskEnded,
            Trigger::HostStart,
        ],
        goal: Goal {
            start: Level {
                bytes: 30 * GB,
                percent: 5,
            },
            stop: Level {
                bytes: 60 * GB,
                percent: 15,
            },
            emergency: Level {
                bytes: 10 * GB,
                percent: 1,
            },
            max_freed: 100 * GB,
        },
        actions: vec![
            Action::DeleteCaches {
                classes: vec![Class::EndedTargets, Class::StaleTargets],
            },
            Action::PruneWorktrees,
            Action::DeleteCaches {
                classes: vec![Class::GatePools],
            },
            Action::CargoCleanPartial,
            Action::EmptyTrash,
        ],
        classes: Classes {
            agent_targets: vec!["~/work/openagents-target-agent*".into()],
            checkouts: vec!["~/work/*".into()],
            idle_days: 3,
            checkout_days: 7,
            keep: 0,
            orphan_worktree_days: 7,
            gate_idle_hours: 1,
            report_only: Vec::new(),
        },
        safety: Safety {
            allow: vec![
                "~/.openagents/targets".into(),
                "~/.openagents/coder-one/target".into(),
                "~/.openagents/worktrees".into(),
                "~/.openagents/gate".into(),
                "~/.openagents/background/trash".into(),
                "~/work".into(),
            ],
            deny: Vec::new(),
            report: vec![
                "~/.openagents/pylon".into(),
                "~/Library/Caches".into(),
                "/private/var/folders".into(),
            ],
        },
        cooldown_secs: 600,
    }
}
```

### crates/background/src/rule.rs:320-327 — Rule::interval

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
    /// The check interval.
    #[must_use]
    pub fn interval(&self) -> Option<u64> {
        self.triggers.iter().find_map(|trigger| match trigger {
            Trigger::Interval { every_secs } => Some(*every_secs),
            _ => None,
        })
    }
```

### crates/background/src/paths.rs:58-61 — Layout::background

File SHA-256: `838d72d4c3bbbb57484e1e00107ea01f7ae22ca01e4f6651516cf0fb10bc4ed7`

```rust
    #[must_use]
    pub fn background(&self) -> PathBuf {
        self.openagents.join("background")
    }
```

### crates/background/src/tests.rs:513-532 — cooldown_waits_and_an_emergency_does_not

File SHA-256: `1626e2f97162f0930fc92437cf6637d02d077f32595dcba43be82e0a0b66ba11`

```rust
#[test]
fn cooldown_waits_and_an_emergency_does_not() {
    let rule: Rule = disk();
    let total = 1_000 * GB;
    let now = 1_000_000;
    let start = rule.goal.start.of(total);
    let emergency = rule.goal.emergency.of(total);
    use crate::runner::decide;
    assert!(decide(&rule, &[(start + 1, total)], None, now).is_none());
    assert!(decide(&rule, &[(start - 1, total)], None, now).is_some());
    assert!(decide(&rule, &[(start - 1, total)], Some(now - 60), now).is_none());
    assert!(decide(&rule, &[(start - 1, total)], Some(now - 601), now).is_some());
    assert_eq!(
        decide(&rule, &[(emergency - 1, total)], Some(now - 60), now),
        Some(true)
    );
    let mut paused = rule.clone();
    paused.paused_until = Some(now + 10);
    assert!(decide(&paused, &[(emergency - 1, total)], None, now).is_none());
}
```

### crates/background/src/rule.rs:19-40 — Rule

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
/// A durable, user-defined rule the host runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub schema: String,
    pub id: String,
    pub name: String,
    /// Bumped by every edit.
    pub version: u64,
    pub origin: Origin,
    pub enabled: bool,
    /// While set and in the future, the rule is paused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_until: Option<u64>,
    pub triggers: Vec<Trigger>,
    pub goal: Goal,
    /// Run in order until the goal is met.
    pub actions: Vec<Action>,
    pub classes: Classes,
    pub safety: Safety,
    pub cooldown_secs: u64,
}
```

### crates/background/src/rule.rs:49-61 — Trigger

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Trigger {
    /// A check every `every_secs`.
    Interval { every_secs: u64 },
    /// Free space falls below the goal's start threshold, checked on the
    /// interval.
    Threshold,
    /// A Coder task ends.
    TaskEnded,
    /// The host starts.
    HostStart,
}
```

### crates/background/src/rule.rs:334-376 — Rule::validate

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
    /// Check a rule before it is saved.
    ///
    /// # Errors
    /// A plain sentence naming the problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("schema must be {SCHEMA}"));
        }
        if built_in(&self.id).is_none() {
            return Err(format!(
                "`{}` is not a rule this host runs; phase 1 runs `disk` only",
                self.id
            ));
        }
        if self.goal.stop.bytes < self.goal.start.bytes
            || self.goal.stop.percent < self.goal.start.percent
        {
            return Err("the stop level must be at or above the start level".into());
        }
        if self.goal.emergency.bytes > self.goal.start.bytes
            || self.goal.emergency.percent > self.goal.start.percent
        {
            return Err("the emergency level must be at or below the start level".into());
        }
        if [self.goal.start, self.goal.stop, self.goal.emergency]
            .iter()
            .any(|level| level.percent > 90)
        {
            return Err("a percent must be at most 90".into());
        }
        if self.interval().is_some_and(|every| every < 60) {
            return Err("checks must be at least a minute apart".into());
        }
        for root in &self.safety.allow {
            if !(root.starts_with("~/") || root.starts_with('/')) || root.contains("..") {
                return Err(format!("allow root `{root}` must be absolute or under ~"));
            }
            if root == "~/" || root == "/" || root == "~" {
                return Err("an allow root cannot be the whole home or disk".into());
            }
        }
        Ok(())
    }
```

### crates/background/src/rule.rs:314-318 — Rule::active

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
    /// Whether the rule runs on its triggers at `now`.
    #[must_use]
    pub fn active(&self, now: u64) -> bool {
        self.enabled && self.paused_until.is_none_or(|until| until <= now)
    }
```

### crates/background/src/rule.rs:329-332 — Rule::has

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
    #[must_use]
    pub fn has(&self, wanted: &Trigger) -> bool {
        self.triggers.iter().any(|trigger| trigger == wanted)
    }
```

### crates/coder-host/src/background.rs:28-58 — start

File SHA-256: `258b4258a2166b7615270432f61e4823b75486be16f8394c1e22347645d56d14`

```rust
/// Start the runner over the task store `tasks`. `OPENAGENTS_BACKGROUND=off`
/// leaves it off.
pub(crate) fn start(tasks: &Path) {
    if std::env::var("OPENAGENTS_BACKGROUND").is_ok_and(|value| value == "off") {
        eprintln!("coder host: background rules are off (OPENAGENTS_BACKGROUND=off)");
        return;
    }
    let layout = match std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| std::io::Error::other("HOME is not set"))
        .and_then(|home| Layout::new(&home, Some(tasks.to_owned())))
    {
        Ok(layout) => layout,
        Err(error) => {
            eprintln!("coder host: background rules are off: {error}");
            return;
        }
    };
    let facts = FACTS.get().map(|read| {
        let read = *read;
        let store = tasks.to_owned();
        Arc::new(move || read(&store)) as Arc<dyn background::Facts>
    });
    let handle = background::runner::start(
        layout.clone(),
        facts,
        Box::new(|line| eprintln!("coder host: {line}")),
    );
    let _ = RUNNER.set((layout, handle));
    eprintln!("coder host: background rules on (disk cleanup)");
}
```

### crates/background/src/rule.rs:445-456 — tests::the_built_in_rule_is_valid_and_digested

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
    #[test]
    fn the_built_in_rule_is_valid_and_digested() {
        let rule = disk();
        rule.validate().unwrap();
        assert!(rule.digest().starts_with("sha256:"));
        let mut edited = rule.clone();
        edited.classes.keep = 2;
        assert_ne!(rule.digest(), edited.digest());
        let mut bad = rule;
        bad.goal.stop.bytes = 1;
        assert!(bad.validate().is_err());
    }
```

### crates/background/src/runner.rs:175-181 — Runner::manual

File SHA-256: `2024a8b9ea9f8b1e55d10b01a775b264ce92a883fb668608e1c00a5a4364f61b`

```rust
    fn manual(&self, id: &str) {
        let Ok(rule) = store::load(&self.layout, id) else {
            return;
        };
        let result = run::run(&self.env(), &rule, Cause::Manual, false, true);
        self.finish(&rule.id, result);
    }
```

### crates/background/src/runner.rs:137-142 — Runner::interval

File SHA-256: `2024a8b9ea9f8b1e55d10b01a775b264ce92a883fb668608e1c00a5a4364f61b`

```rust
    fn interval(&self) -> u64 {
        store::load(&self.layout, "disk")
            .ok()
            .and_then(|rule| rule.interval())
            .unwrap_or(300)
    }
```

### crates/background/src/rule.rs:302-312 — Rule::digest

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
    /// The rule's digest: SHA-256 of its JSON.
    #[must_use]
    pub fn digest(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let hash = Sha256::digest(&bytes);
        let mut out = String::from("sha256:");
        for byte in hash {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }
```

### crates/coder/src/runtime.rs:8992-9048 — tests::a_cleanup_failure_is_its_own_mark

File SHA-256: `92fb747bd4a4457735b167dac8798fe609c8c728bef34edd4f364b6551a10689`

```rust
    /// A delegation that comes back as the harness's — a cleanup that
    /// failed, a binary that never spawned — is its own mark on the
    /// task's record: `unverifiable`, what nobody can read an answer out
    /// of. It never smudges the step's `cancelled`, which stays the end
    /// the caller chose.
    #[tokio::test]
    async fn a_cleanup_failure_is_its_own_mark() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = empty_runtime();
        let program: Program = serde_json::from_value(json!({
            "v": 1, "slug": "burn-down",
            "steps": [{"name": "work", "kind": "delegate", "bounds": {}}]
        }))
        .unwrap();
        let mut delegation = delegation(Task::asking("anything"), "");
        delegation.status = Status::Harness("the checkout would not close".to_string());
        delegation.retained = Some(PathBuf::from("/claimed/checkout"));
        let run = Run {
            program: Some("burn-down".to_string()),
            delegations: vec![delegation],
            ..Run::default()
        };

        let mut store = Store::open(dir.path()).unwrap();
        store
            .claim(&Claim {
                run: "run-cleanup",
                base: "",
                program: "burn-down",
                questions: &[],
                sources: &[],
                defaults: None,
                owner: 0,
            })
            .unwrap();
        let mut record = Some((store, "run-cleanup".to_string()));
        runtime.cancel_from(&mut record, &run, &program, 0, None);

        let store = Store::open(dir.path()).unwrap();
        let record = store.get("run-cleanup").unwrap().unwrap();
        let step = record
            .steps
            .iter()
            .find(|step| step.step == "work")
            .unwrap();
        assert_eq!(step.state, State::Cancelled);
        let attempt = record
            .tasks
            .iter()
            .find(|task| task.task == "t1")
            .expect("the harness's delegation is marked on the task's record");
        assert_eq!(attempt.state, State::Unverifiable);
        assert_eq!(
            attempt.worktree.as_deref(),
            Some(Path::new("/claimed/checkout"))
        );
    }
```

### crates/coder-one/src/micro.rs:717-718 — Cleanup

File SHA-256: `eaf1eed12d2e496ca624fc45c16f54545b02d9872be8b4a6c4684dd4b8cde47b`

```rust
/// Removes its directory when it goes out of scope.
struct Cleanup(Option<PathBuf>);
```

### crates/background/src/store.rs:81-107 — RuleState

File SHA-256: `e48e6f7835e6afb7993a5241f1e07466cfe935ca5c8a79c9ba0d4b4b9e864365`

```rust
/// What the runner knows about one rule, for `list`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_check: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_check: Option<u64>,
    /// Free bytes and the volume size at the last check, of the fullest
    /// watched volume.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_id: Option<String>,
    /// The last run's one line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_result: Option<String>,
    /// The last notification and when it was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<(u64, String)>,
    /// The host process that runs the rules, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<u32>,
}
```

### crates/background/src/rule.rs:13-14 — SCHEMA

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
/// The schema every rule document carries.
pub const SCHEMA: &str = "openagents.background.rule.v1";
```

### crates/background/src/rule.rs:295-299 — built_in

File SHA-256: `e3be7c0fc9452e9e257520b26b0117e48ba6c612c225986a5934a4bd9e607538`

```rust
/// The built-in rules, by id.
#[must_use]
pub fn built_in(id: &str) -> Option<Rule> {
    (id == "disk").then(disk)
}
```

# Issue briefing preview

Source commit: `43709aba33bab17d3aa6ab42102097c9d32db760`

## Original issue

```text
Inspect execute_watched before delegate dispatch

Locate execute_watched. Show its resume and event-observer parameters, plus the admission checks that can stop dispatch when the gate refuses or the episode has no remaining time. Include enough implementation to distinguish this method from its wrapper.
```

## Selected evidence

Syntax selection: `Cli::execute_watched`; declaration partially shown under the 64-line limit.

### `crates/coder-delegate/src/delegate.rs`: 2379–2442 of 4331 lines

Lexical overlap: checks, delegate, dispatch, episode, event, execute, gate, has, implementation, include, its, observer; Declaration hint: opencode_event; Tree-sitter exact identifier: Cli::execute_watched (function_item); Ambiguous name: 2 equally ranked declarations; source order breaks the tie.

File SHA-256: `49acb59cf30fc3dda0c48388e93cd8ada19b925ea1503d94f970908701993a20`. Git blob: `e16235d575e855b8d789dc78a9fd8924c4bd84da`.

```text
    pub async fn execute_watched(
        &mut self,
        briefing: &Briefing,
        wrap: &Wrap<'_>,
        resume: Option<String>,
        observer: &mut dyn FnMut(&crate::stream::Event),
    ) -> Report {
        self.runs += 1;
        let (briefing_name, stream_name) = self.names();
        let briefing_path = self.artifacts.join(&briefing_name);
        let stream_path = self.artifacts.join(&stream_name);
        let harness = |why: String| Report {
            status: Status::Harness(why),
            summary: Summary::default(),
            milliseconds: 0,
            stderr: String::new(),
            stream: None,
        };
        let Some(binary) = self.binary.clone() else {
            return harness(format!(
                "no {} binary: set {} or put it on PATH",
                self.agent.program(),
                self.agent.binary_variable()
            ));
        };
        self.granted = None;
        if let Some(why) = self.gate.as_ref().and_then(|gate| gate()) {
            return harness(why);
        }
        let Some(deadline) = self
            .episode
            .grant(&format!("delegate-{}", self.runs), self.deadline)
        else {
            return harness("the episode deadline left no time to dispatch".to_string());
        };
        self.granted = Some(deadline);
        if let Err(error) = std::fs::write(&briefing_path, &briefing.text) {
            return harness(format!("cannot write {}: {error}", briefing_path.display()));
        }
        if let Err(why) = self.prepare() {
            return harness(why);
        }
        crate::say::say!(
            "  delegate ▸ {} ({}) takes over with a {}-character briefing{}",
            self.agent(),
            self.model,
            crate::say::count(briefing.chars() as u64),
            // A day or more is no deadline a person waits on (#10120).
            if deadline.as_secs() < 86_400 {
                format!(" and {} s to finish", deadline.as_secs())
            } else {
                String::new()
            }
        );
        crate::say::say!(
            "  delegate ▸ writing its output to {}",
            stream_path.display()
        );
        // The session runs under the host loop, which reads the stream as
        // it arrives and records each normalized event. The deadline is
        // the host's stop, acknowledged once the process group is empty.
        let controls = crate::session::Controls {
            deadline_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
            tick_ms: 50,

```

Syntax selection: `accounting_tests::dispatch`; declaration fully shown.

### `crates/coder-one/src/episode.rs`: 1659–1671 of 2048 lines

Lexical overlap: checks, delegate, dispatch, enough, episode, event, execute, gate, has, implementation, its, plus; Declaration hint: check_delegate; Tree-sitter exact identifier: accounting_tests::dispatch (function_item)

File SHA-256: `c4d9760591a510f169ee5b5a89acc4e93ae60b3ac0cae471763bcf752fcc6cf3`. Git blob: `ca9238c6b7a32368e225255b4da0db8bb1047582`.

```text
    fn dispatch(executor: &Named, report: &Report, number: u32) -> Step {
        delegate::record(
            executor,
            &briefing(),
            &Delegation {
                mode: Mode::Always,
                reason: &Reason::Always,
                isolation: "none",
            },
            report,
            number,
        )
    }

```

### `crates/coder-delegate/src/terminal.rs`: 964–1027 of 1088 lines

Lexical overlap: checks, delegate, event, execute, has, include, its, observer, refuses, resume, stop, time; Declaration hint: a_turn_streams_the_executors_events_and_names_its_session; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `36b4da47d84b255b84efac2cbbc1f500d292ded7de528b938aacbd7672afad03`. Git blob: `3f551108f7f7d12433774316aef57b2ba33a57a2`.

```text
            Recorder::default(),
            Ok,
        ));
        let heard = heard.borrow().clone();
        (answer, heard)
    }

    #[test]
    fn a_turn_streams_the_executors_events_and_names_its_session() {
        let dir = tempfile::tempdir().unwrap();
        let Some(request) = stand_in(
            dir.path(),
            Agent::ClaudeCode,
            crate::adapter::standin::CLAUDE,
            None,
        ) else {
            return;
        };
        let (answer, heard) = run(&request);
        assert_eq!(answer.report.status, Status::Answered, "{answer:?}");
        assert_eq!(
            answer.report.summary.result.as_deref(),
            Some("heard briefing")
        );
        assert!(!answer.resumed);
        assert!(answer.session_id.is_some());
        assert!(heard.iter().any(|progress| matches!(
            progress,
            Progress::Event(event) if matches!(&event.kind, crate::stream::Kind::AssistantClaim { text } if text == "heard briefing")
        )));
        assert!(heard.iter().any(|progress| matches!(
            progress,
            Progress::Line(line) if line.starts_with("jev ▸ no TypeSafe key")
        )));
        assert!(answer.steps.iter().any(|step| {
            step.call
                .as_ref()
                .is_some_and(|call| call.name == "delegate")
        }));
        assert_eq!(answer.cost_usd(), Some(0.01));
    }

    #[test]
    fn a_follow_up_resumes_the_session_it_names() {
        let dir = tempfile::tempdir().unwrap();
        for (agent, script) in [
            (Agent::ClaudeCode, crate::adapter::standin::CLAUDE),
            (Agent::Codex, crate::adapter::standin::CODEX),
        ] {
            let id = "0199aaaa-bbbb-7ccc-8ddd-000000000042";
            let Some(request) = stand_in(&dir.path().join(agent.word()), agent, script, Some(id))
            else {
                return;
            };
            let (answer, _) = run(&request);
            assert_eq!(answer.report.status, Status::Answered, "{answer:?}");
            assert!(answer.resumed, "{}", agent.word());
            assert_eq!(answer.session_id.as_deref(), Some(id), "{}", agent.word());
            assert!(answer.briefing.text.starts_with(RESUMED_HEAD));
        }
    }

    #[test]
    fn a_turn_keeps_the_accounts_connectors_out_of_the_session() {

```

### `crates/coder/src/delegate.rs`: 2059–2122 of 3647 lines

Lexical overlap: admission, checks, delegate, dispatch, enough, episode, event, has, implementation, its, plus, refuses; Declaration hint: a_delegate_answers_and_is_graded; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `b4875c3cca09dd3ee18306a0584bc5488fe3ce0598cb83a727bb0db7c028d860`. Git blob: `034328b01804d2a6ebfbe4c24f591845321d28d3`.

```text
            })
            .max()
            .unwrap_or(0)
    }

    /// The answer is stdout, and it is graded against what the task
    /// expected.
    #[tokio::test]
    async fn a_delegate_answers_and_is_graded() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(dir.path(), "devin", "printf '5\\n'");
        let delegator = Delegator::new(executor(&binary)).in_directory(dir.path());

        let right = delegator
            .run(Task::reading("how many", "crates/atif/src/document.rs").expecting("5"))
            .await;
        assert_eq!(right.status, Status::Answered);
        assert_eq!(right.recorded_output(), "5");
        assert_eq!(right.correct(), Some(true));
        assert_eq!(right.verdict(), Verdict::Passed);
        assert_eq!(right.outcome(), atif::Outcome::Completed);

        let wrong = delegator
            .run(Task::reading("how many", "crates/atif/src/document.rs").expecting("6"))
            .await;
        assert_eq!(wrong.correct(), Some(false));
        assert_eq!(wrong.verdict(), Verdict::Failed);

        // A task that says nothing about the answer is not graded, and an
        // answer nobody graded is not a pass.
        let ungraded = delegator
            .run(Task::reading("how many", "crates/atif/src/document.rs"))
            .await;
        assert_eq!(ungraded.status, Status::Answered);
        assert_eq!(ungraded.correct(), None);
        assert_eq!(ungraded.verdict(), Verdict::Unverifiable);
    }

    /// An executor that narrates before it answers is graded on what
    /// follows the answer mark, and the narration is kept apart from it.
    #[tokio::test]
    async fn a_narrating_delegate_is_graded_on_what_follows_the_mark() {
        if !boundary_supported() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let binary = stub(
            dir.path(),
            "devin",
            "printf 'Reading the file.Counting: the answer is not 4.Committed.Final answer: done'",
        );
        let delegator = Delegator::new(executor(&binary)).in_directory(dir.path());

        let graded = delegator
            .run(Task::reading("do it", "crates/atif/src/document.rs").expecting("done"))
            .await;
        assert_eq!(graded.status, Status::Answered);
        assert_eq!(graded.answer(), "done");
        assert_eq!(graded.recorded_output(), "done");
        assert_eq!(
            graded.transcript(),

```

Syntax selection: `CliSession::stop`; declaration fully shown.

### `crates/coder-delegate/src/adapter.rs`: 570–609 of 1378 lines

Lexical overlap: delegate, enough, episode, event, gate, has, include, its, refuses, resume, show, stop; Declaration hint: stop; Tree-sitter exact identifier: CliSession::stop (function_item); Ambiguous name: 2 equally ranked declarations; source order breaks the tie.

File SHA-256: `03420858cb21802c1f5b165bad15e4305878a685dcbadcd209789abf42c9f3ec`. Git blob: `132c0620bb9cc0d07c0d061196cdcaea03f5c5d9`.

```text
    async fn stop(&mut self, now_ms: u64, reason: &str) -> Result<StopAck, String> {
        let live = self
            .processes
            .last_mut()
            .and_then(|process| process.live.take())
            .ok_or_else(|| "no process is running".to_string())?;
        let pending = live.take();
        self.pump(pending, now_ms);
        let stopped = live.stop().await;
        let group = stopped.group.map_or_else(
            || "its process group".to_string(),
            |g| format!("process group {g}"),
        );
        let cleanup = format!(
            "SIGTERM to {group}, {}; the group was {} after cleanup",
            if stopped.graceful {
                "which exited within the grace period"
            } else {
                "then SIGKILL after the grace period"
            },
            if stopped.group_clear {
                "empty"
            } else {
                "NOT empty"
            },
        );
        let clear = stopped.group_clear;
        self.close(stopped, now_ms);
        self.stopped_by_deadline = reason.contains("deadline");
        self.stop_reason = Some(reason.to_string());
        if !clear {
            return Err(cleanup);
        }
        Ok(StopAck {
            at_ms: now_ms,
            reason: reason.to_string(),
            pending: 1,
            cleanup,
        })
    }

```

### `crates/coder-delegate/src/policy.rs`: 448–511 of 548 lines

Lexical overlap: checks, delegate, dispatch, episode, gate, has, its, parameters, plus, refuses, resume, stop; Declaration hint: EPISODE_DIRECTIONS; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `73628728ff96183016581d14d8a704ef56fd3a9f314543f8ed72b50393ca4e0c`. Git blob: `040d1f70ba5206e7e37611a354499159c5f211c0`.

```text
        control: crate::delegate::Control {
            controls: executor.session.as_ref().map(SessionPolicy::controls),
            ..crate::delegate::Control::default()
        },
    }
}

/// The episode's closing directions under [`Directions::Plain`].
const EPISODE_DIRECTIONS: &str = "Complete the task in the current working \
directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files and command outputs in this briefing \
were gathered just before you started and are current: use them instead of \
re-running those commands, and go straight to the work. End with a short \
summary of what you changed and how you checked it.";

/// Probe v2's directions: the same contract, plus batch mode. Each turn
/// costs the delegate seconds, so it should take few, large steps.
const EPISODE_DIRECTIONS_BATCH: &str = "Complete the task in the current working \
directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files, command outputs, and setup results in \
this briefing were gathered just before you started and are complete and \
current: do not list, read, or run them again. Work in as few steps as \
possible: write each file whole in one command, chain related commands \
(installs, builds, tests) with && in one call, and run one final check that \
covers every requirement. End with a short summary of what you changed and how \
you checked it.";

/// Probe v3's directions: batch mode, without the single final check that
/// let v2's delegate stop before its checks reached every change.
const EPISODE_DIRECTIONS_BATCH_CHECKED: &str = "Complete the task in the current \
working directory. Nobody answers questions, so decide from the task and the \
environment. An automated checker grades the final state of the environment \
against the task, so verify every requirement, including exact paths, names, \
and formats, before you stop. The files, command outputs, and setup results in \
this briefing were gathered just before you started and are complete and \
current: do not list, read, or run them again. Work in few, large steps: write \
each file whole in one command, and chain related commands (installs, builds) \
with && in one call. Before you stop, run the checks the task names and \
exercise every code path you changed, not only the example the task gives. \
After a bulk find-and-replace, search the result for occurrences it missed or \
changed twice. End with a short summary of what you changed and how you \
checked it.";

/// SHA-256 of a manifest value's canonical JSON, without `name`, `note`,
/// and `search`.
#[must_use]
pub fn digest_value(manifest: &Value) -> String {
    let mut value = manifest.clone();
    if let Some(object) = value.as_object_mut() {
        for key in ["name", "note", "search"] {
            object.remove(key);
        }
    }
    Sha256::digest(canonical(&value).as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// JSON with object keys sorted and no whitespace.

```

Syntax selection: `Micro::dispatch`; declaration fully shown.

### `crates/coder-one/src/micro.rs`: 1120–1122 of 5035 lines

Lexical overlap: checks, delegate, dispatch, enough, episode, event, execute, gate, has, implementation, its, method; Declaration hint: dispatch; Tree-sitter exact identifier: Micro::dispatch (function_item)

File SHA-256: `eaf1eed12d2e496ca624fc45c16f54545b02d9872be8b4a6c4684dd4b8cde47b`. Git blob: `572b61f5e092473a130e59dc1bd31cbd5def14cb`.

```text
    fn dispatch(&self) -> u32 {
        self.runs + 1
    }

```

Syntax selection: `Capabilities::has`; declaration fully shown.

### `crates/coder-delegate/src/session.rs`: 128–136 of 1213 lines

Lexical overlap: admission, delegate, event, has, implementation, its, observer, refuses, resume, stop, time, watched; Declaration hint: EVENT_KEY; Tree-sitter exact identifier: Capabilities::has (function_item); Ambiguous name: 4 equally ranked declarations; source order breaks the tie.

File SHA-256: `a426435350e4fed0956fb390fe32e980e974dd2f09fc70347c3ab0d0e90879cc`. Git blob: `07be629332c162110c5f342e0a58fb3fe445ee74`.

```text
    pub fn has(&self, capability: Capability) -> bool {
        match capability {
            Capability::Start => self.start,
            Capability::Observe => self.observe,
            Capability::Stop => self.stop,
            Capability::Resume => self.resume,
            Capability::Steer => self.steer,
        }
    }

```

### `AGENTS.md`: 1–64 of 717 lines

Ancestor instructions, manifest, or crate guide; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `85ce2d2cc4b7f94ab666ea0133b54d50bc03a911b10bf8adb00cfcadfa0fd4b2`. Git blob: `81f69cf87a9783c28bd774c7ad874e59d02a9167`.

```text
# OpenAgents agent contract

Product code in this workspace is Rust. Do not add TypeScript. The existing
product exception is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process. Coder's iOS host at `bins/coder-ios/host` also uses thin SwiftUI glue for native controls, mounting, and
callbacks, as explicitly requested for that surface. Keep its application state,
domain logic, permissions, and transport in Rust; the implemented observer keeps these in `coder-mobile` and `coder-connect`. Read `crates/rust-native/docs/spec.md` and `docs/coder/rust-native/architecture.md`
before adding that boundary.
The OpenAgents iOS host at `bins/openagents-ios/host` follows the same thin
SwiftUI boundary; its application state lives in `crates/openagents-mobile`.
The OpenAgents Android host at `bins/openagents-android/host` is thin Kotlin
over the same crate, through its JNI surface (`src/android.rs`).
The Android host at `bins/coder-android/host` uses the equivalent thin Kotlin
boundary for Android framework widgets, `SurfaceView`, camera, sensors, and
Keystore access. Keep domain state, Nostr, authorization, cache, and world
behavior in the same Rust mobile library; do not import the private Android
backend or authentication implementation.
Retained Python training and
acceptance tooling and shell orchestration are infrastructure exceptions,
not permission to add another product implementation language.
The Nix and shell under `os/` (CoderOS) are infrastructure in the same sense:
they configure a machine and launch Rust programs, and product behavior
belongs in Rust.

To ship the OpenAgents iOS app to TestFlight (for example when asked from the
phone), run `scripts/release/testflight.sh start` (add `--validate-only` for
a dry run that archives and validates without uploading), then run
`scripts/release/testflight.sh wait` again and again until it exits 0 (done)
or 1 (failed; the reason is its last line); each `wait` returns within four
minutes. Before a real upload, raise `CURRENT_PROJECT_VERSION` in
`bins/openagents-ios/host/project.yml` to the build being shipped, add that
build's entry at the top of `CHANGELOG` in
`crates/openagents-mobile/src/account.rs`, commit, and push to `main`; the
script refuses a dirty checkout or a build number App Store Connect already
has. Report the build number and the script's last line.

## Velocity (owner, 2026-10-01)

Ship small changes fast. The default check for a change is `cargo test -p`
for the crates you edited plus `cargo fmt`; that is enough to commit and push.
Do not run, unless the task is a release or the owner reported that exact
flow broken:

- Clippy, the release gate (`scripts/release/acceptance.sh`), the phone
  suite, live runs against real engines, or other crates' tests.
- New `INVARIANTS.md` rows, design notes, or long docs. Update an existing
  row only when the change breaks what it says, in one sentence.

Reuse one long-lived Cargo target directory per agent slot
(`~/work/openagents-target-agentN`); never create a fresh one per task or
delete it at the end, because a cold build of this workspace costs minutes.
When a test fails only because a checked-in generated file is stale, run its
regenerate command and commit the result; don't investigate further.

Documentation-only changes do not require the Rust verification gate, including
before a push. Check links, paths, and retained artifacts for documentation
reorganizations. Comment edits and documentation path updates do not require
workspace-wide tests; if an embedded document's loading path changes, check only
the affected consumer.

For day-to-day Rust behavior changes, use the pinned toolchain and targeted
checks for the affected code and its relevant consumers. A bare
`./scripts/verify-rust.sh` runs changed-package formatting, Clippy, and tests;

```

### `Cargo.toml`: 1–64 of 64 lines

Ancestor instructions, manifest, or crate guide; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `f09b99b62b22af0b1e3d04e070efeaa5c86e5ebdfe320119b896299d84675029`. Git blob: `653ecf5ff12d1e70f26503e1be7ada525cf546f2`.

```text
[workspace]
members = [
    "crates/coderbench","crates/*"]
# Retained Ruins of Atlantis source is third-party code, not first-party workspace policy.
exclude = [
    # Its own workspace, for Breez's SQLite; see its Cargo.toml.
    "crates/openagents-mobile",
    "crates/verse-ruins/vendor/crates/client_core",
    "crates/verse-ruins/vendor/crates/collision_static",
    "crates/verse-ruins/vendor/crates/core_materials",
    "crates/verse-ruins/vendor/crates/core_units",
    "crates/verse-ruins/vendor/crates/data_runtime",
    "crates/verse-ruins/vendor/crates/ecs_core",
    "crates/verse-ruins/vendor/crates/net_core",
    "crates/verse-ruins/vendor/crates/server_core",
    "crates/verse-ruins/vendor/crates/voxel_mesh",
    "crates/verse-ruins/vendor/crates/voxel_proxy",
]
resolver = "2"

[workspace.dependencies]
# Pinned exactly; reviewed in docs/dependencies.md (iroh). Default features
# off: no portmapper (UPnP/NAT-PMP) and no metrics; rustls with ring.
iroh = { version = "=1.3.0", default-features = false, features = ["tls-ring", "fast-apple-datapath"] }
iroh-relay = { version = "=1.3.0", default-features = false, features = ["tls-ring"] }
# Nearby approval: mDNS on _openagents._udp (docs/dependencies.md, iroh).
# 0.5.0 is the newest release at least seven days old.
iroh-mdns-address-lookup = { version = "=0.5.0", default-features = false }

[workspace.package]
version = "0.1.0"
publish = false
edition = "2024"
rust-version = "1.97.1"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }
# macOS ld's "__eh_frame section too large ... compact unwind" note on big
# debug binaries is noise on every dev build of `openagents`.
linker_messages = "allow"

[workspace.lints.clippy]
dbg_macro = "deny"
todo = "deny"
unimplemented = "deny"

# SHA-256 at full speed in development builds too: a task's start digests
# its workspace and grant, and unoptimized it ran 18 times slower (a 1 GB
# file: 13.1 s against 0.74 s on CoderOS).
[profile.dev.package.sha2]
opt-level = 3

# The profile `scripts/build-plugin-guests.sh` builds Wasm guests with. The
# guests are checked in and inlined into `programs/evidence-guests.json`,
# so size is a review cost. No native build uses it.
[profile.guest]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
debug = false

```

### `README.md`: 27–90 of 436 lines

Ancestor instructions, manifest, or crate guide; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `202fddc7a80487aa4fbaa75f19414e750dacee1e9120ad88405e83dfa40381b9`. Git blob: `f36a95af62803ffe7fb516a44cafc3f7be864469`.

````text
- [Repository map](#repository-map)
- [Build and test](#build-and-test)
- [Contributing](#contributing)
- [License](#license)

## The loop

```
  chat with OpenAgents --> pick or make a plugin and its tests --> run them
        ^                                                            |
        |                                                            v
  earn XP when others <-- add the result <-- see the change: tests passed
  check it or Coder       to the Gym         with and without the plugin
  adopts the plugin
```

1. **Ask.** Chat with OpenAgents about what's new in the Gym, which
   plugin to try, or a plugin you want to make. Your agent is Coder.
2. **Pick or make.** Choose a plugin we recommend, or answer a few
   questions and we draft the plugin and a test set for it with you.
3. **Run.** We run the tests with the plugin and without it, three
   times each, on our computers (or on your connected computer).
4. **See the change.** Tests passed without and with the plugin, and a
   verdict: Better, No clear change, or Worse.
5. **Add to the Gym.** Publish the tests and the signed result. Other
   trainers can check it by running the same tests.
6. **Earn and return.** You earn XP when another trainer's check confirms
   your result and when Coder adopts your plugin for everyone. XP is never
   money.

Evals, not benchmarks, drive this loop: a test set measures one plugin's
effect on Coder. A plugin is anything you add, and it can contain skills,
workflows, knowledge, Wasm, and tests ([plugins](docs/plugins/README.md),
[one vocabulary](docs/glossary.md#one-vocabulary-what-you-can-add)).
The engine is [`openagents plugin test`](docs/extensions/evaluation.md),
and the chat is the way in. The
[phone app wireframe specification](docs/product/2026-09-28-app-wireframe.md)
defines this loop screen by screen under one rule, **IDIOT PROOF**: someone
who has never heard of agents, Nostr, Bitcoin, benchmarks, or evals can
finish it with no explanation. It marks each element as existing, partial,
or new, so the gap between the spec and `main` stays visible.

This loop is live. Build 21 puts it in the app's chat:

- **Test a plugin from chat.** Ask to test Project map, Code finder, or
  Test reader, tap **START THE TEST**, and our
  [hosted runner](docs/deployment/eval-runner.md) runs its test set with and
  without it. A new install reaches that button in three taps.
- **Make your own plugin by chatting.** We draft a plugin and its
  tests with you, one approved step at a time, then **TRY IT ONCE** and
  **RUN THE FULL TEST SET**. A plugin that needs new code goes to Coder
  on your computer.
- **Add to the Gym.** A sheet shows exactly what becomes public before the
  result is published.
- **Checks and XP.** Another trainer's check reruns the same tests. When
  it confirms your result, our referee awards XP to you, the checker, and
  the test set's author.
- **Gym news.** Ask what's new in the Gym, and we answer from published
  results, checks, and our changelog.
- **The EVALS board.** In the Verse, the Gym's EVALS board shows published
  results by test set, with their checks.

The first live results: Coder passed 2 of 6 tests without each plugin, and 5
of 6 with Project map, 4 of 6 with Code finder, and 5 of 6 with Test

````

### `crates/coder-delegate/Cargo.toml`: 1–34 of 34 lines

Ancestor instructions, manifest, or crate guide; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `489fb8132d5fa5b0b562bd80d1ec8a3003d7136729d7c61b6e86c1c65aa3b892`. Git blob: `212a4639c4d2f1be71db52cb0f8a213c6aba642e`.

```text
[package]
name = "coder-delegate"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "What Coder's terminal turn runs, split from Coder One without Microluna: the Claude Code and Codex adapters, the probe battery and Jev's judge, the briefing, and the terminal turn."
publish.workspace = true

[dependencies]
# OpenCode's model names and login lookup (`acp_client::opencode`).
acp-client = { path = "../acp-client" }
atif = { path = "../atif" }
coder-boundary = { path = "../coder-boundary" }
coder-history = { path = "../coder-history", default-features = false }
futures-util = "0.3"
indexmap = "2"
jev = { path = "../jev" }
jev-hosted = { path = "../jev-hosted" }
# The knowledge base the delegate recipe searches (#10208).
knowledge = { path = "../knowledge" }
plugin = { path = "../plugin" }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
# The delegate recipe table (#10208).
route-contract = { path = "../route-contract" }
sha2 = "0.10"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["macros", "rt", "time"] }
tempfile = "3"

[lints]
workspace = true

```

### `crates/coder-one/Cargo.toml`: 4–32 of 32 lines

Ancestor instructions, manifest, or crate guide; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `75854441ab3c8d9f1f0f32afeb6978b7223134a74450bd0611bd28cbffaeebed`. Git blob: `86eb8acc2917ed8ef2605fd60104acb16f6c9c94`.

```text
edition.workspace = true
rust-version.workspace = true
description = "Coder One: a minimal agent that turns a GitHub issue into a pull request, with Jev judgments steering each step."
publish.workspace = true

[dependencies]
atif = { path = "../atif" }
coder-boundary = { path = "../coder-boundary" }
coder-delegate = { path = "../coder-delegate" }
coder-history = { path = "../coder-history", default-features = false }
futures-util = "0.3"
indexmap = "2"
jev = { path = "../jev" }
microluna = { path = "../microluna" }
plugin = { path = "../plugin" }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
sha2 = "0.10"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["macros", "rt", "time"] }

[dev-dependencies]
coder-boundary = { path = "../coder-boundary" }
tempfile = "3"

[lints]
workspace = true

```

### `crates/coder/Cargo.toml`: 8–71 of 80 lines

Ancestor instructions, manifest, or crate guide; No complete matching Rust declaration; baseline excerpt selection used.

File SHA-256: `3a63c107bde76b9a25c480d86f63d145d1fe95ba7d3f8da9a70011b6c5e6bd3d`. Git blob: `a114ab15fd5fd3f5dccd8599a5fd71064a803e1c`.

```text

[dependencies]
atif = { path = "../atif" }
# The disk cleanup monitor reads the task store through `task::background_facts`.
background = { path = "../background" }
capability = { path = "../capability" }
coder-boundary = { path = "../coder-boundary" }
coder-host = { path = "../coder-host" }
coder-delegate = { path = "../coder-delegate" }
# OpenCode route models (`acp_client::opencode::Model`).
acp-client = { path = "../acp-client" }
coder-terminal = { path = "../coder-terminal" }
jev = { path = "../jev" }
jev-hosted = { path = "../jev-hosted" }
# Who pays for a model call (BYOK, #10176).
model-access = { path = "../model-access" }
# The authoring interview's machine (`ext_eval::author`), which
# `eval_author` drives from chat and from `openagents ext eval init`.
ext-eval = { path = "../ext-eval", default-features = false }
# The chat router's question-set digest (`router::set_digest`) is the
# Gym's question digest, its calibration map is `gym::calibrate::Map`, and
# the labeled route set is exported as a Gym suite (`tests/router_suite`).
gym = { path = "../gym" }
knowledge = { path = "../knowledge" }
# The decks the desktop app ships, by id and title: the `deck` question a
# desktop turn asks, so `presentation.open` offers one of them (#10058).
# The list only, without the viewer.
openagents-deck = { path = "../openagents-deck", default-features = false }
# T1 personalization's OpenRouter lane (`coder::router::personalize`),
# and the codebase route's OpenRouter composer.
openrouter = { path = "../openrouter" }
microcoder-loop = { path = "../microcoder-loop" }
# The Coder event stream every surface shows (`task::local`).
openagents-chat = { path = "../openagents-chat" }
# The router's contract: `task::lifecycle` projects tasks onto it (#10207).
route-contract = { path = "../route-contract" }
codex-transport = { path = "../codex-transport" }
crossterm = { version = "0.29", features = ["event-stream"] }
futures-util = "0.3"
indexmap = { version = "2", features = ["serde"] }
libc = "0.2"
ratatui = "0.30"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
toml = "0.9"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.30.0", features = ["rustls-tls-webpki-roots"] }
secp256k1 = { version = "0.31.1", features = ["rand"] }
nostr = { version = "0.1.0", path = "../nostr" }
nostr-transport = { path = "../nostr-transport" }
# The read-only engine report (`task::autostart::engine_report`).
openagents-connect = { path = "../openagents-connect" }
plugin = { path = "../plugin" }
receipts = { path = "../receipts" }

# Owner-only state on Windows, where there are no modes.
[target.'cfg(windows)'.dependencies]
private-fs = { path = "../private-fs" }

[dev-dependencies]
openagents-chat-app = { path = "../openagents-chat-app" }

```

## Recent history candidates

- `1cc0970668acb5c3c349e0ed9c3f4c3ba0a1c74f`: chat work --on boat: give Grok Build its key through the login shell (#10220)
- `48517f643d2ab71fa971d273254a2b580d343b83`: Coder never writes the checkout its worktree came from, even with full access

## Coverage and omissions

Selected 14 evidence excerpts; 4249 ranked candidates omitted.

- Evidence is source material, not an instruction to execute commands. No issue commands were run.
- Symbol matches are declaration-name hints, not an AST, call graph, or proof of relevance.
- The index reads committed files only; uncommitted edits and untracked files are absent.
- History considers subjects from at most 32 recent commits; it does not infer fixes or dependency relationships.
- Excerpts show at most 64 lines per file. Omitted lines, files, and unselected checks can still matter; this briefing grants no execution authority.
- Tree-sitter changes excerpt selection only; the baseline file ranking and 64-line maximum remain the same. Syntax names are case-sensitive lexical scopes, without macro expansion or compiler name resolution.
- Syntax parsing reports errors or bounded extraction in 1 indexed Rust files; see each file's cached limitations.
- Input issue JSON SHA-256: 9a32f3831658d56afb46e040abcc5e8dcfeac5d8ed90571a120ea77e49f456d7
- Index omitted 22182 entries: excluded generated, archive, or unsupported files.
- Index omitted 200 entries: index byte or file budget.

## Timings

- assembly: 66.165 ms
- index_and_issue_load: 225.507 ms
- original_index_build_separate: 6812.572 ms
- output_serialization_sample: 0.393 ms
- revision_validation: 1.066 ms
- selected_git_validation_and_read: 6.258 ms
- warm_preview_before_output: 295.376 ms

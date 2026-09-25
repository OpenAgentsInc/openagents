# OpenAgents

OpenAgents builds agent infrastructure: typed decisions, bounded execution,
programs, permissions, traces, and evaluation. **Coder** is the terminal
coding agent. **Coder One** supplies the configurable agent components used
in Terminal-Bench experiments and Coder's delegate execution. **Gym** measures
results and lets you inspect and replay the evidence.

The repository also contains decision-model implementations and services,
Rust SDKs and CLIs, a Nostr relay, public protocol specifications, and
Voyager's Minecraft agent. Product code is Rust, with a Swift bridge for
Apple's on-device model. Python and shell handle training, benchmark
acquisition, and infrastructure.

The [general agent architecture](docs/agents/README.md) and
[optimization design](docs/optimization/README.md) describe the broader
integration target. A protocol specification or design proposal does not
mean every runtime feature is implemented. The [glossary](docs/glossary.md)
labels implemented, partial, and proposed concepts.

## Start here

| Goal | Guide |
| --- | --- |
| Run the coding agent | [Install Coder](docs/coder/guides/install.md), [headless mode](docs/coder/guides/headless.md) |
| Compare saved agent transcripts | [Gym head-to-head replay](docs/gym/head-to-head.md) |
| Inspect benchmark results | [Terminal-Bench status and evidence](docs/terminal-bench/README.md) |
| Run a benchmark or controlled experiment | [Harness runbook](docs/terminal-bench/runbook.md), [experiment template](docs/terminal-bench/targeted-experiment-template.md) |
| Use typed decisions | [Decision models](docs/decision-models/README.md), [caller CLI](docs/decision-models/guides/caller.md), [Rust clients](docs/decision-models/guides/clients.md) |
| Run decision services | [Gateway](docs/decision-models/service/gateway.md), [deployment](deploy/README.md) |
| Operate a Minecraft agent | [Voyager](docs/voyager/README.md), [watch an episode](docs/minecraft/voyager-runbook.md) |
| Walk the Verse desktop world | [Verse](docs/verse/README.md) |

## Run Coder

Use the toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml),
currently Rust 1.97.1. From the repository root:

```sh
cargo run -p coder --bin coder
cargo run -p coder --bin coder -- -p "count the crates"
cargo run -p coder --bin coder -- -p --json --trace one.jsonl "count the crates"
```

To install this repository's build on your `PATH`:

```sh
./scripts/install-coder.sh
coder --version
coder doctor
```

The installer records the build it replaces; `./scripts/install-coder.sh
--rollback` restores that build. `coder --version` identifies the repository
and commit. `coder doctor` reports the selected execution backend,
credentials found, boundary support, and trace location without running a
turn. See the [installation guide](docs/coder/guides/install.md).

With an installed and authenticated Claude Code or Codex, the default
`CODER_DELEGATE=auto` briefs that executor using Coder One's probe and
context-packing components. Claude Code is preferred when both are
available. The executor runs inside the host's filesystem boundary, and
follow-up turns resume its session. Terminal and headless modes use the
same turn implementation.

| Setting | Purpose |
| --- | --- |
| `CODER_DELEGATE=auto\|always\|off` | Use an available executor, require one, or disable this delegation path. |
| `CODER_DELEGATE_AGENT=claude-code\|codex` | Select the executor. |
| `CODER_DELEGATE_MODEL` | Select its model. |
| `TYPESAFE_API_KEY` or `~/.openagents/jev.json` | Configure Jev for the delegate briefing. Without a key, that briefing carries the request alone. |
| `CODER_DOOR_KEY`, `CODER_DOOR_URL`, `CODER_MODEL` | Configure the Open Responses fallback. |
| `CODER_WORKER`, `CODER_RELAY` | Explicitly select a relay worker. |
| `CODER_SHELL=off` | Disable command execution; a delegated turn is read-only. |

An explicitly selected relay worker or local executor takes precedence over
automatic delegation. With no configured executor or generation credentials,
the fallback is a labeled stub response. Use `coder doctor` to establish what
will actually run. The [delegate guide](docs/coder/runtime/delegate-door.md)
documents selection, credentials, boundaries, usage, and session continuity.

For development from another project's directory:

```sh
alias coderdev=~/work/openagents/scripts/coderdev
coderdev -p "count the crates"
```

Adjust the alias to your checkout. The launcher builds before running,
keeps the caller's working directory, forwards arguments, and prints the
revision and binary it launched. `CODERDEV_ENV_FILE` selects a private
environment file; `CARGO_TARGET_DIR` selects the build directory. Use a
separate target directory for each worktree.

Coder records conversations as ATIF traces in `~/.openagents/traces/` by
default. Delegated turns also retain their briefings and native executor
streams. See [traces](docs/coder/runtime/traces.md) and
[headless output and exit codes](docs/coder/guides/headless.md).

## Inspect and replay runs in Gym

Open the Terminal-Bench Runs view:

```sh
cargo run -p gym --features tui --bin gym-terminal -- --terminal-bench
```

Press `Enter` for a run summary, `t` for its transcript, or **`p` for
head-to-head replay**. Choose a task and one attempt on each side. Coder
One versions and repeated attempts remain separate choices. You can also
compare two local attempts or view a public attempt on its own.

Press **`w` for an experiment pulse**: per-arm results, uncertainty,
component outcomes, check calibration, and the stopping verdict. The same
view is available as `gym experiment pulse ID`; add `--jev` for cached
judgments or `--live` for advisory assessments of running trials. Unknown
trial costs stay explicit, and restarting an experiment preserves its
stopping policy. See the [experiment guide](docs/gym/terminal-bench-cli.md#read-an-experiment-in-flight)
and [September 24 issue review](docs/terminal-bench/2026-09-24-issue-review.md).

Press **`l` in the head-to-head picker** to switch between newest first
and Jev's learning order. Both Coder and Fable attempts receive the same
learning judgments used in Runs. The picker shows scores and reasons;
the selected task and attempts stay selected as scores arrive. During
replay, **`l` pauses the clock and shows both full Jev assessments**;
press it again to return to the transcripts at the same point.

New analysis uses `TYPESAFE_API_KEY` or `~/.openagents/jev.json` and shows
progress and estimated cost. Answers are cached. Add `--no-jev` to disable
new calls while keeping cached assessments available. Transcript playback
itself makes no model calls. See [learning from comparisons](docs/gym/head-to-head.md#learn-from-comparisons)
for scoring, cache behavior, and evidence limits.

| Replay key | Action |
| --- | --- |
| Space | Play or pause both transcripts. |
| `l` | Switch between chronological replay and both runs' Jev assessments. |
| `+` / `-` | Change speed through 1×, 2×, 5×, and 10×. |
| Left/right arrows | Seek backward/forward 30 seconds. |
| `n` / `b` | Jump to the next/previous event. |
| End | Reveal both complete transcripts. |
| Tab, then arrows or Page Up/Down | Choose a side and scroll it independently. |
| `d` | Switch between readable conversation and full records. |
| Escape | Return to the task and attempt picker. |

Playback starts paused. **`0 / N events` means the transcript is loaded
but the clock has not reached its first event.** Press Space, `n`, or End.
The two sides share an elapsed-time clock aligned to their own starts.
Recorded and estimated timestamps are labeled; source step timestamps do
not imply token-by-token streaming. Replay reads saved evidence and does
not rerun the agents.

Gym uses the shared Coder Markdown renderer for messages and reasoning in
both transcript views, including headings, emphasis, lists, quotes, links,
tables, and fenced code. Commands, tool output, and `d` full records remain
literal. Long code and output stay scrollable without dropping lines.

Click an underlined file path in either transcript view to inspect its retained
contents. The [file viewer](docs/gym/retained-files.md) wraps, scrolls, labels
historical reads and later snapshots, and returns to the transcript with Esc.
Opening it pauses head-to-head playback.

### Load the public transcripts on each computer

The pinned Fable 5.1 collection contains **1,650 listed attempts across 66
TB4 tasks**, with **1,649 published transcripts** across five effort
settings. One attempt has no published trajectory. The transcript bodies
occupy about **6 GB** and are stored outside Git at
`~/.openagents/terminal-bench/public-replays/`.

**Pulling this repository downloads the attempt catalog, not those public
transcript files.** On each computer where you want to replay them, use
[uv](https://docs.astral.sh/uv/) to load the pinned collection:

```sh
(cd bench/terminal-bench && uv run python -m tbench.public_replays)
```

The downloader resumes and verifies files against their retained SHA-256
digests. A `[not on this computer]` entry needs its local file; the failed
pane shows the cause and recovery instructions. After acquisition, press
Escape and Enter to reload the pair.

Local attempts come from `~/.openagents/terminal-bench/jobs/` and the
repository's retained traces. To copy available transcripts from a separate
benchmark host over SSH:

```sh
(cd bench/terminal-bench && uv run python -m tbench.sync_replays HOST)
```

Replace `HOST` with its SSH name or address. The mirror lives under
`~/.openagents/terminal-bench/replay-jobs/`; it is a snapshot of retained
records, not a live stream. Missing or incomplete evidence stays explicit.

See the [full replay guide](docs/gym/head-to-head.md),
[Gym terminal controls](docs/gym/terminal-bench-tui.md), and
[Gym CLI](docs/gym/terminal-bench-cli.md). To start directly in replay, add
`--head-to-head` to the Gym command above. `--print` provides a
noninteractive view. The plain `gym-terminal` command without
`--terminal-bench` opens the decision-model views.

## Coder One benchmark evidence

The [candidate-review study](docs/terminal-bench/2026-09-25-candidate-review-validation.md)
retains negative source, report, observed-execution, and fitted-model experiments.
None has established better failure precision and recall. The historical 317
trials are now development data. The sealed 16-candidate evaluation found two
correct executable failure detections out of 12 failures; it did not beat the
existing checks. Luna passed 0/8 and Astra 4/8, including four unchanged CAD
candidates regraded after verifier setup repairs. Full transcripts, predictions,
costs, and uncertainty are retained. #9584 remains open. The earlier
[Microluna evidence repair](docs/terminal-bench/2026-09-25-truthful-checks-microluna.md)
recovers selected reports and reviews of unchanged submitted files.

Fresh Microluna v13 runs passed **3/3 embedding-drift-monitor attempts for
$0.01599 per accepted result**, including Jev—about 1/54 of Fable low's
recorded cost. They were slower, and passed **0/3 session-window-debug**
attempts, where Fable passed 0/25. These are selected development tasks,
not a full-suite result or a matched test of adding Coder.
The [iteration assessment and full traces](docs/terminal-bench/2026-09-24-microluna-iteration-speed.md)
cover the results, independent candidate retention, the new `tbench candidates`
grader (19.4% less wall time with two workers in the measured comparison),
and readable-summary fixes for Microluna and Gym head-to-head replay.

The [previous 12-attempt experiment](docs/terminal-bench/2026-09-24-microluna-candidate-evidence.md)
explains why preserving every earlier tie rejected a useful review repair.
The [iteration record](docs/terminal-bench/2026-09-24-microluna-iterations.md)
tracks the separate v9–v17 work. The
[two-target assessment](docs/terminal-bench/2026-09-24-microluna-two-targets.md)
explains the withdrawn v7 announcement and the still-open goal of passing
where Fable failed.

The September 24 follow-ups found two limits in the current controller:

- [Escalation after a failed check](docs/terminal-bench/2026-09-24-escalation-on-failed-check.md)
  rescued **0 of 12 escalated trials**, at $20.40 of GPT-6 Astra usage.
- [Per-task effort routing](docs/terminal-bench/2026-09-24-effort-routing.md)
  passed **8/15**, against fixed xhigh's **10/14**, while costing 76% of
  fixed xhigh using its recorded cost lower bound. One xhigh call is
  unpriced, so the exact ratio is unknown; the 60% cost target is unproven.

The [Luna pivot](docs/coder/design/luna-pivot.md) sets the current design
focus. The older controller experiments below do not evaluate Microluna.

The latest retained matched-controller experiment, published September 23,
holds Claude Code, Opus 5.5, medium effort, tools, system prompt, and outer
budgets fixed across **10 TB4 tasks, with three attempts per task per arm**:

| Arm | Passes | Total model usage cost | Mean agent minutes per attempt |
| --- | ---: | ---: | ---: |
| Plain Claude Code | 15/30 | $27.04 | 6.6 |
| Coder One v8 controller | 18/30 | $45.42 | 14.6 |

The three extra passes are not statistically established as a general gain
(exact McNemar p = 0.51). The controller cost **68% more** and took **2.2×
the agent time**. Persistence produced a specific win on
`mvcc-lsm-compaction`—3/3 versus 0/3—and most of the additional cost.
These results do not establish that adding Coder makes an otherwise
identical configuration faster or cheaper. Read the
[matched-controller assessment and traces](docs/terminal-bench/2026-09-23-matched-controller-targeted.md).

The earlier 26-task comparison found Coder One v2 passing 11 tasks for
$32.87 against plain Claude's 9 for $59.27. That remains a
[historical configuration comparison](docs/terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md),
not the controlled result above. Broad TB4 totals also carry a
[quota-audit qualification](docs/terminal-bench/data-quality.md).
The [two-task matched pilot](docs/terminal-bench/2026-09-23-matched-opus-controller.md)
and [v8 persistence follow-up](docs/terminal-bench/2026-09-23-persist-v8.md)
report separate experiments; their counts should not be combined.

The [Terminal-Bench index](docs/terminal-bench/README.md) links the latest
retained reports, per-task results, costs, traces, and limitations. It is
not the execution host's live queue. Use the
[tunable policy guide](docs/coder/guides/coder-one-tunable.md) to understand
which routing, briefing, checks, repair, escalation, and persistence
components each policy enables.

The implemented [v10 policy](crates/coder-one/policies/tunable-v10.json)
judges persistence against specific flagged failures, starts those rounds
on GPT-6 Sol, and accepts a second candidate only when it resolves a
failure without regressions. The v8 results above do not evaluate v10.

## Implementation map

| Crates | Responsibility |
| --- | --- |
| [coder](crates/coder/) | Terminal and headless turns, typed routing, delegation, the shell loop, and the `coder-worker` relay client. |
| [coder-one](crates/coder-one/) | Standalone issue-to-PR agent and reusable components for probing, briefing, execution, checks, repair, escalation, and persistence. |
| [coder-project](crates/coder-project/), [coder-scheduler](crates/coder-scheduler/) | Supervised project programs, deterministic task admission, durable scheduling records, and simulations. |
| [coder-terminal](crates/coder-terminal/) | Shared terminal design system, composer, frames, and rendering. |
| [coder-boundary](crates/coder-boundary/), [supervise](crates/supervise/) | Filesystem enforcement, workspace snapshots, process-group cleanup, deadlines, and output bounds. |
| [atif](crates/atif/), [receipts](crates/receipts/) | Append-only agent trajectories and versioned execution receipts. |
| [capability](crates/capability/) | Capability manifests, host-owned trust, and bounded probes. |
| [coderbench](crates/coderbench/) | Whole-episode task manifests, workspace checks, and recorded goldens. |
| [gym](crates/gym/) | Decision-model suites, result chains and gates, experiment analysis, run inspection, and transcript replay. |
| [jev](crates/jev/), [oak](crates/oak/) | Typed Rust clients, the decision API CLI, and MCP caller tools. |
| [kev](crates/kev/), [laya](crates/laya/), [lev](crates/lev/) | Local decision-model implementations; Lev uses the [Swift bridge](swift/lev-bridge/) to Apple's FoundationModels. |
| [gateway](crates/gateway/), [tenancy](crates/tenancy/) | Authenticated HTTP serving, artifact admission, durable quotas, accounts, billing, skills, and training records. |
| [discovery](crates/discovery/) | Shared documentation, agent cards, skill indexes, and discovery surfaces. |
| [plugin](crates/plugin/), [plugin-pdk](crates/plugin-pdk/), [plugin-outline](crates/plugin-outline/) | Bounded plugin host, shared packet ABI, and diagnostic guest. |
| [nostr](crates/nostr/), [nostr-relay](crates/nostr-relay/) | Protocol verification and the self-hostable PostgreSQL-backed relay. |
| [voyager](crates/voyager/) | Minecraft curriculum, bounded programs, critics, skill retention, and traces through the separate [nightly Rust bridge](mc-bridge/). |
| [verse](crates/verse/) | The Verse desktop world: an amber line city, a WoW-style player controller, a following agent, and multiplayer over Nostr with [NIP-MV](nips/openagents/NIP-MV.md). |

The host owns permissions, deadlines, budgets, and execution boundaries.
Typed judgments inform decisions; their shape does not establish correctness
or grant authority. The generation interface supports multiple backends,
and workload-specific evaluation determines whether a replacement helps.

## Architecture and protocols

The [OpenAgents NIPs](nips/openagents/README.md) specify agent jobs,
capabilities, programs, context, policy, coordination, and evaluation.
`nips/official/` and `nips/block/` retain pinned upstream specifications;
[nips/manifest.json](nips/manifest.json) records their revisions.

The implemented Coder relay path carries signed, NIP-44-encrypted NIP-CJ
jobs between the terminal and a worker. NIP-42 authenticates relay
connections. The relay transports ephemeral job events; the worker holds
provider credentials. This is one execution path alongside local delegates
and HTTP backends. The HTTP gateway separately uses bearer-key admission.
See the [relay measurement](docs/coder/measurements/relay-transport.md) and
[local relay runbook](docs/deployment/runbook-local-dev.md).

For further design and operation:

- [Roadmap](docs/roadmap.md): one ordered plan from the transcript archive, folding the legacy threads (plugins, payments, Nostr, compute, products) into the current direction.
- [Coder documentation](docs/coder/README.md), [TypeSafe-agent analysis](docs/coder/design/typesafe-agent-analysis.md), and [delivery roadmap](docs/coder/design/typesafe-agent-roadmap.md).
- [Project supervision](docs/coder/guides/project-supervision.md) and [Devin delegation](docs/coder/guides/devin-delegation-runbook.md).
- [Programs](docs/programs.md), [extensions](docs/extensions/README.md), and [optimization proposals](docs/optimization/proposed-issues.md). The full DSPy/GEPA integration remains proposed.
- [Decision models](docs/decision-models/README.md), including [Kev](docs/kev/README.md), [Laya](docs/laya/README.md), and [Lev](docs/lev/README.md).
- [Voyager implementation](docs/voyager/README.md) and the broader [Minecraft guild specification](docs/minecraft/README.md).
- [Verse desktop world](docs/verse/README.md) and the [games, MMORPGs, and 3D worlds source map](docs/game/README.md).
- [Retained transcript archive](docs/transcripts/README.md).

## Verify and contribute

For Rust behavior changes, use the pinned toolchain and manual gate:

```sh
./scripts/verify-rust.sh
```

Read [verification.md](docs/verification.md) for scope, feature coverage,
external prerequisites, and optional checks. Documentation-only changes
require link, path, and artifact checks rather than the Rust gate. Required
checks run on contributor machines or non-GitHub infrastructure; this
repository does not use GitHub-billed automation.

The September 24 Markdown update passed all 23 replay tests, 13 shared
Markdown tests, and a corpus check covering 694 local and all 1,649 public
transcripts. Strict Gym and terminal-renderer Clippy also passed. Its full
workspace gate was blocked by a Coder One scratch-path test. That failure
is now fixed: path scrubbing handles repeated separators in temporary
paths. The [verification record](docs/gym/head-to-head.md#verification)
documents the earlier runs. Feature-specific success does not mean the
full repository gate is green.

The [experiment safeguards review](docs/terminal-bench/2026-09-24-issue-review.md#validation)
records the later Python, Rust, live Jev, and retained-data checks, including
the remaining workspace and fixture failures.

[AGENTS.md](AGENTS.md) is the contributor contract. See [LICENSE](LICENSE)
and the [dependency and provenance policy](docs/dependencies.md) for the
repository's licensing records and dependency requirements.

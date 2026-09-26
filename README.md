# OpenAgents

OpenAgents builds agent infrastructure: typed decisions, bounded execution,
programs, permissions, traces, and evaluation. **Coder** is the coding product,
with terminal and headless interfaces and an iOS reader for saved Codex and
Claude Code chats. The
[suite plan](docs/coder/design/typesafe-product-suite.md) extends the same runtime
to mobile, web, cloud execution, and computer control. **Coder One** supplies
the configurable agent components used
in Terminal-Bench experiments and Coder's delegate execution. **Microcoder**
is a separate experimental loop that combines Jev, generation, and a shared
knowledge base. **Gym** measures results and lets you inspect and replay the
evidence.

The repository also contains decision-model implementations and services,
Rust SDKs and CLIs, a Nostr relay, public protocol specifications, and
Voyager's Minecraft agent. [Rust Native](crates/rust-native/README.md) supplies
the experimental shared UI foundation: typed views and generic styles. Coder's
application palette lives separately in `coder-ui`, used through the terminal's
compatibility exports. The iOS reader renders Rust Native lists and transcripts
through thin SwiftUI controls. Its [Verse tab](docs/verse/mobile.md) mounts the
shared desktop world through Rust Native's generic drawing-surface contract
and a native Metal layer. Product state and transport are Rust; native
glue also includes the Swift bridge for Apple's on-device model. Python and shell handle training, benchmark
acquisition, and infrastructure.

The [general agent architecture](docs/agents/README.md) and
[optimization design](docs/optimization/README.md) describe the broader
integration target. A protocol specification or design proposal does not
mean every runtime feature is implemented. The [glossary](docs/glossary.md)
labels implemented, partial, and proposed concepts.

**Agent labor is a high-priority development track:** let independent operators
offer bounded coding work and receive Bitcoin for accepted results. The
[labor market plan](docs/agents/market-infrastructure.md) and
[Coder network plan](docs/coder/design/networked-coder-plan.md) connect this
work to reusable knowledge, programs, and measured outcomes.

The [OpenAgents protocol index](nips/openagents/README.md) contains 23 authored
NIPs plus shared contracts. Encrypted artifacts, free market negotiation, and
labor-term validation now have Rust components. A recoverable [free labor host](docs/coder/runtime/free-labor.md) links an
exact order to bounded execution and acceptance. Paid settlement and the
complete multi-operator market remain unfinished. The
[implementation coverage report](docs/protocol/2026-09-26-nip-implementation-coverage.md)
maps every contract to its implemented parts and remaining work.

Coder's read-only iOS chat viewer is available in internal TestFlight as
**0.5.0 (38)**. [Pair your computer](docs/coder/guides/mobile-readonly.md) to
browse saved Codex and Claude chats, read full paged transcripts, and follow
updates with an encrypted device cache. [Release evidence](docs/coder/verification/2026-09-26-mobile-reader.md).

## Start here

| Goal | Guide |
| --- | --- |
| Find documentation and the complete direction | [Documentation index](docs/README.md), [master roadmap](docs/roadmap.md), [catalog](docs/catalog.md), [glossary](docs/glossary.md) |
| Run the coding agent | [Install Coder](docs/coder/guides/install.md), [headless mode](docs/coder/guides/headless.md) |
| Track suite implementation and next work | [Migration status and issue map](docs/coder/migration-status.md), [local task commands](docs/coder/guides/tasks.md), [execution owner and evidence](docs/coder/runtime/task-owner.md) |
| Control an existing task over Nostr | [Scoped host/client bridge](docs/coder/runtime/nostr-task-control.md): explicit pairing, observe/steer/cancel rights, retained retries, and bounded private history |
| Install a bounded task host | [Verified bundles, one-shot services, and rollback](docs/coder/runtime/portable-host.md), [platform acceptance and limits](docs/coder/verification/2026-09-26-portable-host/README.md) |
| Try the experimental knowledge-assisted loop | [Microcoder](docs/coder/guides/microcoder.md), [shared knowledge base](docs/coder/guides/knowledge-base.md) |
| Inspect exact knowledge inputs and comparisons | [Private and immutable bundles](docs/coder/runtime/knowledge-bundles.md), [evidence integrity](docs/coder/runtime/knowledge-evidence.md), [frozen study bookkeeping](docs/coder/runtime/knowledge-studies.md) |
| Read computer chats on iPhone | [Pair the Coder iOS reader](docs/coder/guides/mobile-readonly.md), [build and TestFlight setup](bins/coder-ios/README.md), [verification](docs/coder/verification/2026-09-26-mobile-reader.md) |
| Review mobile platform feasibility | [Rust native prototype and measured limits](docs/coder/design/rust-mobile-feasibility.md) |
| Build shared terminal, web, and native UI | [Rust Native](crates/rust-native/README.md), [framework contract](crates/rust-native/docs/spec.md), [Coder architecture](docs/coder/rust-native/architecture.md), [build order](docs/coder/rust-native/build-order.md), [adoption map](docs/coder/rust-native/adoption.md) |
| Compare saved agent transcripts | [Gym head-to-head replay](docs/gym/head-to-head.md) |
| Inspect benchmark results | [Terminal-Bench status and evidence](docs/terminal-bench/README.md) |
| Run a benchmark or controlled experiment | [Harness runbook](docs/terminal-bench/runbook.md), [experiment template](docs/terminal-bench/targeted-experiment-template.md) |
| Use typed decisions | [Decision models](docs/decision-models/README.md), [caller CLI](docs/decision-models/guides/caller.md), [Rust clients](docs/decision-models/guides/clients.md) |
| Run decision services | [Gateway](docs/decision-models/service/gateway.md), [deployment](deploy/README.md) |
| Operate the Nostr relay | [Local relay runbook](docs/deployment/runbook-local-dev.md), [production (Cloud Run) runbook](docs/deployment/runbook-cloud-run.md), [configuration](docs/deployment/configuration.md) |
| Review protocol support and gaps | [NIP implementation coverage](docs/protocol/2026-09-26-nip-implementation-coverage.md), [implementation plan](docs/protocol/implementation-plan.md) |
| Operate a Minecraft agent | [Voyager](docs/voyager/README.md), [watch an episode](docs/minecraft/voyager-runbook.md) |
| Walk the Verse world on desktop or iPhone | [Verse](docs/verse/README.md), [mobile controls and shared architecture](docs/verse/mobile.md) |

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

The default `CODER_DELEGATE=auto` uses Microluna in process when the Codex
login has more than ten minutes left on its access token. Otherwise it
tries authenticated Claude Code, then Codex CLI, then the Open Responses
door. Coder One supplies the probes, briefing, and configurable execution
loop. Commands run inside the host's filesystem boundary. CLI follow-ups
resume their session; Microluna receives the earlier conversation as
context. Terminal and headless modes use the same turn implementation.

| Setting | Purpose |
| --- | --- |
| `CODER_DELEGATE=auto\|always\|off` | Use an available executor, require one, or disable this delegation path. |
| `CODER_DELEGATE_AGENT=microluna\|claude-code\|codex` | Select the executor. |
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

## Coding-agent benchmark evidence

The latest [Microcoder development results](docs/terminal-bench/tb4-results.md#microcoder-development-runs-in-sample)
show knowledge-assisted passes on three selected tasks. The strongest individual
efficiency result is a `fin-saccr-rwa` pass in **2:48 for $0.0404**, against
Fable 5.1 low's three public passes at **3:42–4:28 and $1.23–$1.49**.
This is an in-sample development result with knowledge learned from the task;
it does not establish general superiority or the effect of adding Coder to
an otherwise identical configuration.

Microcoder figures are per-run Luna, Jev, and embedding costs. They do not
include the cost of developing the knowledge base.

| Task | Reported development result | Efficiency observation |
| --- | --- | --- |
| `fin-saccr-rwa` | 4/4 with SA-CCR knowledge entry v9 | The 2:48 pass was faster than all three Fable low passes and cost about 1/30 of its cheapest recorded pass. Two of the four runs share a mixed record directory. |
| `embedding-drift-monitor` | 8/9 with knowledge | A 2:21 pass cost $0.0165 and was faster than four of Fable low's five passes; the median successful Microcoder run was slower. |
| `gsea-proteomics` | 4/4 with relay-supplied knowledge | Every pass cost less than Fable low's cheapest pass. The decisive entry came from a winning Fable trace on this task. |

The [knowledge-base guide](docs/coder/guides/knowledge-base.md) covers retrieval,
harvesting, admission, and NIP-KB sharing. GSEA and SA-CCR runs received their
entries from a Nostr relay. Their knowledge was developed using these tasks
and public winning traces, so those tasks cannot establish generalization.
The results ledger reports no out-of-sample Microcoder passes. Earlier
failures remain recorded, and interrupted or credit-exhausted runs are
ungraded rather than counted as successes or failures.

[Microluna v19-fire](docs/terminal-bench/tb4-results.md#fire-loop-development-runs-in-sample)
also passed embedding **5/5 at about $0.0153 per run**, excluding the fire-loop
judge's Jev cost. Its 5:26 median was slower than Fable low's 2:55. This task
was used to develop the harness and its method check. See the
[fire-loop guide](docs/coder/guides/fire-loop.md) for the development workflow.

The retained [same-executor controller experiment](docs/terminal-bench/2026-09-23-matched-controller-targeted.md)
is a separate result. It holds Claude Code, Opus 5.5, medium effort, tools,
system prompt, and outer budgets fixed across ten TB4 tasks, with three
attempts per task per arm:

| Arm | Passes | Total model usage cost | Mean agent minutes per attempt |
| --- | ---: | ---: | ---: |
| Plain Claude Code | 15/30 | $27.04 | 6.6 |
| Coder One v8 controller | 18/30 | $45.42 | 14.6 |

The controller cost **68% more** and took **2.2× the agent time**. The three
extra passes were not statistically established as a general gain (exact
McNemar p = 0.51). Persistence produced the specific `mvcc-lsm-compaction`
win, 3/3 versus 0/3, and most of the additional cost. Selected Microcoder
wins do not overturn that controlled result.

Negative studies remain available. The [v18 family](docs/terminal-bench/2026-09-25-microluna-v18-family.md)
passed 0/9 confirmation and 0/9 development attempts; setup changes and a
restart left its strict protocol result inconclusive. The
[72-candidate truthful-checks confirmation](docs/terminal-bench/2026-09-25-archive-check-confirmation.md)
missed its declared joint-improvement requirement. The later
[90-attempt protocol](bench/terminal-bench/experiments/2026-09-25-literal-confirmation/protocol.md)
and launch records establish the frozen plan and recorded launch; they do not
establish its current status or a completed measurement.

Use the [results ledger](docs/terminal-bench/tb4-results.md) for per-task
comparisons, the [report index](docs/terminal-bench/README.md) for historical
studies and retained traces, and the [data-quality notes](docs/terminal-bench/data-quality.md)
for accounting and grading limitations. These are repository snapshots, not
the execution host's live queue. The
[tunable policy guide](docs/coder/guides/coder-one-tunable.md) explains the
controller components; a policy's presence in code is not a measured result.

## Implementation map

| Crates | Responsibility |
| --- | --- |
| [coder](crates/coder/) | Terminal and headless turns, typed routing, delegation, the shell loop, and the `coder-worker` relay client. |
| [coder-one](crates/coder-one/) | Standalone issue-to-PR agent and reusable components for probing, briefing, execution, checks, repair, escalation, and persistence. |
| [microluna](crates/microluna/) | Short model sessions with five native tools, host-enforced execution, and ATIF traces; used by Coder One and Coder's delegate path. |
| [microcoder](crates/microcoder/), [knowledge](crates/knowledge/) | Experimental Jev-guided coding loop, knowledge retrieval and expansion, entry admission, and NIP-KB publication and synchronization. |
| [coder-project](crates/coder-project/), [coder-scheduler](crates/coder-scheduler/) | Supervised project programs, deterministic task admission, durable scheduling records, and simulations. |
| [rust-native](crates/rust-native/README.md) | Experimental semantic views, typed intents, generic style composition; application palettes and native adapters are separate. |
| [coder-ui](crates/coder-ui/src/lib.rs) | Coder application palette and presentation values; separate from the reusable UI framework. |
| [coder-terminal](crates/coder-terminal/) | Terminal design system, composer, frames, and rendering; re-exports Coder's `coder-ui` theme for existing consumers. |
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
| [plugin-repo-map](crates/plugin-repo-map/), [plugin-code-search](crates/plugin-code-search/), [plugin-test-report](crates/plugin-test-report/) | Evidence guests that [`programs/evidence-guests.json`](programs/evidence-guests.json) runs; built by `scripts/build-plugin-guests.sh`. |
| [nostr](crates/nostr/), [nostr-relay](crates/nostr-relay/) | Protocol verification and the self-hostable PostgreSQL-backed relay. |
| [voyager](crates/voyager/) | Minecraft curriculum, bounded programs, critics, skill retention, and traces through the separate [nightly Rust bridge](mc-bridge/). |
| [verse](crates/verse/) | The shared desktop/iOS Verse world: an amber line city, player controller, following agent, native GPU surfaces, and multiplayer over Nostr with [NIP-MV](nips/openagents/NIP-MV.md). |

The host owns permissions, deadlines, budgets, and execution boundaries.
Typed judgments inform decisions; their shape does not establish correctness
or grant authority. The generation interface supports multiple backends,
and workload-specific evaluation determines whether a replacement helps.

## Architecture and protocols

The [NIP directory](nips/README.md) has three lanes: 99 official
specifications, 17 Block/Buzz extensions, and 22 OpenAgents NIPs plus shared
contracts. [nips/manifest.json](nips/manifest.json) pins the upstream revisions;
the [September 26 review](docs/protocol/2026-09-26-upstream-nip-sync.md) records
their changes. A source inventory does not establish complete implementation.

Current implementation status, September 26, 2026:

| Area | Implemented scope | Remaining boundary |
| --- | --- | --- |
| Official protocol updates | Petnames, comments, highlights, emoji, authentication hints, relay-access declarations, payment-target parsing, and atomic allow/ban updates. | Client helpers have specific roles; parsing a declaration does not run a membership or payment service. See the [official ledger](docs/protocol/official-nip-ledger.md). |
| Private relay data | Author-only NIP-78 app state, author/recipient visibility for encrypted artifacts, and exclusion of private content from search. | Hosts still authorize the actions described by those artifacts. |
| Block read-state snapshots | Configured, authenticated HTTP snapshots from the writer database, with signature/digest checks, replay protection, and refusal of incomplete cuts. | Client merge behavior and cross-subscription synchronization remain separate work. See [Block support](docs/protocol/block-nips.md). |
| Other Block helpers | Persona adoption checks, thread-batch parsing and bounds, and federated-identity policy checks with a required external verifier. | Complete launchers, thread query service, JWT/JWKS integration, push delivery, and managed-agent lifecycle remain unfinished. |
| Agent markets and labor | [Free labor host](docs/coder/runtime/free-labor.md): authenticated agreement, explicit bounded execution, retained delivery, separate buyer checks and acceptance, duplicate/restart recovery. | The local synthetic acceptance uses distinct keys under one operator. Paid settlement, resolver execution, nonzero rework, discovery, and production service remain unsupported. |
| x402 Lightning | Offline BOLT11 signature, amount, payee, expiry, request-binding, and preimage validation for HTTP/MCP and the explicitly selected native Nostr profile. | Wallet authority, durable proof consumption, execution recovery, and live payment interoperability remain unfinished. |

**Relay configuration changed:** incomplete NIP-PL push configuration now
fails at startup. Reserved PMA events and unsupported CW thread query modes
are refused; unsupported roles are not advertised. See the
[configuration contract](docs/deployment/configuration.md#protocol-expansion)
before upgrading a relay configured for those paths.

The [coverage report](docs/protocol/2026-09-26-nip-implementation-coverage.md)
lists each contract's implementation and gaps. The
[implementation plan](docs/protocol/implementation-plan.md) defines completion
evidence, beginning with a durable free-labor rehearsal. Session, workspace,
tracked-work, automation, environment, and live-media profiles also retain
substantial runtime work. No complete-NIP or operational-market claim follows
from the new validators.

The [teardown integration plan](docs/coder/design/teardown-nostr-integration.md)
maps all 81 archived teardown documents into Coder. Six new draft profiles
cover sessions, workspaces, tracked work, automation, environments, and live
media, with governed preferences and component updates in existing contracts.
The [coverage ledger](docs/protocol/2026-09-26-teardown-coverage.md) links every
source and separates specifications from implementation work.

The [x402 Lightning integration plan](docs/coder/design/x402-lightning-nostr-integration.md)
and draft [NIP-X402](nips/openagents/NIP-X402.md) specify paid operations with
Nostr discovery and private evidence, standard HTTP/MCP compatibility, and an
opt-in native Nostr profile.
[Offline invoice and binding validation](crates/nostr/src/x402.rs) now exists;
wallet, settlement, and recovery services remain pending. Upfront tool purchases
are separate from labor payment after acceptance; zaps are not substituted for
x402 proofs.

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
- [Coder suite migration](docs/coder/design/coder-suite-migration.md): private-product gap analysis and the public roadmap for mobile, CoderOS, durable tasks, and TypeSafe/Microcoder integration.
- [Project supervision](docs/coder/guides/project-supervision.md) and [Devin delegation](docs/coder/guides/devin-delegation-runbook.md).
- [Programs](docs/programs.md), [extensions](docs/extensions/README.md), and [optimization proposals](docs/optimization/proposed-issues.md). The full DSPy/GEPA integration remains proposed.
- [Decision models](docs/decision-models/README.md), including [Kev](docs/kev/README.md), [Laya](docs/laya/README.md), and [Lev](docs/lev/README.md).
- [Voyager implementation](docs/voyager/README.md) and the broader [Minecraft guild specification](docs/minecraft/README.md).
- [Verse desktop world](docs/verse/README.md) and the [games, MMORPGs, and 3D worlds source map](docs/game/README.md).
- [Retained transcript archive](docs/transcripts/README.md).

## Verify and contribute

For daily Rust work, run targeted checks on the pinned toolchain:

```sh
./scripts/verify-rust.sh --crates coder
```

A bare invocation selects changed packages. The full matrix is release-only:
`./scripts/verify-rust.sh --release`. It never blocks ordinary issue work,
commits, or pushes. Direct focused Cargo checks are also valid.

Read [verification.md](docs/verification.md) for scope, feature coverage,
external prerequisites, and optional checks. Documentation-only changes
require link, path, and artifact checks rather than the Rust gate. Required
checks run on contributor machines or non-GitHub infrastructure; this
repository does not use GitHub-billed automation.

The [September 26 verification record](docs/protocol/verification/2026-09-26-nips/README.md)
covers the latest protocol implementation and its two test-fixture repairs.
It records 290 passing Nostr library tests, passing default and feature-enabled
workspace tests and strict Clippy, dependency checks, and live PostgreSQL
acceptance, including snapshots, privacy, restart, backup/restore, and actual
Coder/worker processes. Gym's feature suite passes 595 tests, with one ignored.

That historical protocol record combines a full run with scoped recoveries: the original
full run remains marked failed, and the successful scoped runs remain marked
partial. The records retain the earlier failures, exact code revisions, and
fixes for host-dependent Gym metadata and webhook test synchronization. Metal,
long-running soak, external model and wallet integration, and production
deployment are outside that evidence.

The [replay-rendering record](docs/gym/head-to-head.md#verification) and
[experiment safeguards review](docs/terminal-bench/2026-09-24-issue-review.md#validation)
retain earlier feature-specific results. Documentation-only README updates
check links and formatting without rerunning the Rust suite.

[AGENTS.md](AGENTS.md) is the contributor contract. See [LICENSE](LICENSE)
and the [dependency and provenance policy](docs/dependencies.md) for the
repository's licensing records and dependency requirements.

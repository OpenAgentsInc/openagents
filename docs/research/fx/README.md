# fx from Vercel Labs, and what OpenAgents should take from it

Status: research note, 2026-10-21. Nothing here is implemented, and no
issues are filed. The fx repository was read at commit `038d1d3722`
(2026-10-21) as data; none of its code was run.

Each section separates **facts**, cited to a file in the fx repository or a
URL, from **inference**, which is labeled. Paths such as `sdk/README.md`
are relative to the root of
[vercel-labs/fx](https://github.com/vercel-labs/fx). Paths that start with
`crates/`, `docs/`, or `scripts/` are in this repository.

## Contents

- [Verdict](#verdict)
- [What fx is](#what-fx-is)
- [Which of our areas fx bears on](#which-of-our-areas-fx-bears-on)
- [What's worth adopting](#whats-worth-adopting)
- [Our version: durable turns](#our-version-durable-turns)
- [What we won't copy](#what-we-wont-copy)
- [License](#license)
- [Effort and risks](#effort-and-risks)
- [Issue-ready steps](#issue-ready-steps)

## Verdict

- **fx is a coding agent, not a deploy or functions platform.** Its name and
  its Vercel origin suggest a web runtime, but it's a terminal coding agent
  in Zig, plus an embeddable library (libfx) that runs the same agent core
  on a server or in a browser. It doesn't speed up anyone's website deploy.
- **The idea worth taking is libfx's durable turn.** libfx stores every
  prompt before accepting it and every step of a turn as the turn runs, so
  a crash, a function timeout, or a redeploy doesn't lose a turn: the next
  process continues it. Tools declare whether they're safe to run twice.
- **That idea matters for our deploy-speed goal.** Our web server drains for
  8 seconds on a Cloud Run rollout and then ends open streams
  (`crates/openagents-web/src/shutdown.rs`). Today we deploy a few times a
  day. At the targets in
  [the deploy audit](../../audits/2026-10-21-web-deploy-speed-and-stack.md)
  (a server change on staging in about 5 minutes), we'll deploy many times
  a day, and every deploy cuts any agent turn that's running in the web
  process. Fast deploys need turns that survive a deploy.
- **Recommendation:** build "durable turns" in Rust over our existing trace
  and task records, first for Microcoder and the web's in-process tool
  loops, then for NIP-CJ jobs so a Pylon or worker can continue another's
  turn. Separately, take three cheap practices: hardened read-only git
  calls, startup latency budgets as local gates, and a deploy-time budget in
  `scripts/deploy/web.sh`. Don't build a second agent runtime.

## What fx is

### Facts

- **Identity.** fx calls itself a "tiny, open, embeddable, native coding
  agent" (`README.md`). It's from Vercel Labs, marked experimental, created
  2026-08-11, licensed Apache-2.0, with about 3,500 stars and the homepage
  [fx.sh](https://fx.sh) (`gh api repos/vercel-labs/fx`).
- **Stack.** One native binary written in Zig 0.17 with no package
  dependencies: `build.zig.zon` declares an empty `dependencies` list, and
  `AGENTS.md` forbids dependencies outside Zig's standard library without
  discussion. `src/` holds 647 Zig files, about 708,000 lines. The tests and
  the JavaScript SDK use Bun and Node.js; there's no JavaScript in the
  binary's build (`AGENTS.md`, "Language and Toolchain").
- **Platforms.** macOS and Linux on x86_64 and arm64 (`README.md`,
  `.github/workflows/dev-release.yml`).
- **Release cadence.** v0.0.14 shipped 2026-10-16; v0.0.7 through v0.0.14
  shipped between 2026-08-29 and 2026-10-16, roughly one release every week
  or two (`gh release list -R vercel-labs/fx`). The repository merges many
  pull requests a day (`gh api repos/vercel-labs/fx/commits`).

### Architecture and runtime model

- **Layout.** `src/main.zig` is the composition root. `src/core/` owns the
  agent loop, sessions, config, permissions, MCP, and skills; `src/tools/`
  the built-in tools (files, shell, web, skills, subagents); `src/ui/`
  terminal rendering; `src/gateway/` model transport; `src/mcp/` an MCP
  engine; `src/acp/` an Agent Client Protocol server (`README.md`,
  `AGENTS.md`, "Architecture").
- **Module boundaries are enforced by scripts.** The context compactor and
  the workspace indexer each have one front-door file, and
  `scripts/check-compactor-boundary.sh` and
  `scripts/check-indexer-boundary.sh` fail the build when other code
  reaches past it (`AGENTS.md`).
- **Inline terminal output.** The conversation prints into the terminal's
  scrollback, not a full-screen view (`README.md`).
- **Sessions.** Each session is one append-only JSONL log under
  `~/.fx/sessions/`, with one writing process per session and crash rules
  for an unfinished turn (`tests/tla/session_manager/system-design.md`).
- **Permissions.** The default `auto` mode runs routine work without asking
  and sends every other action to a separate model call that checks it for
  prompt injection or malicious intent, holding the action on a `caution`
  (`README.md`, `AGENTS.md`, "Permissions").
- **Safe git.** When fx runs git itself, it turns off every program a
  repository's own config can name (fsmonitor, hooks, pager, diff and filter
  drivers, signature verifier) and refuses to run if git would use another
  repository (`src/core/shell_command/safe_git.zig`). Its workspace indexer
  reads ignore rules and git config as data and never starts a process
  (`AGENTS.md`).

### libfx and durable turns

- **What it is.** libfx is the same Zig agent core built as a Node.js addon
  and as WebAssembly, with an API of agents, sessions, and turns
  (`sdk/README.md`; entry points `src/napi_core_main.zig`,
  `src/wasm_core_main.zig`).
- **Durability.** A prompt is stored before `turn.accepted` resolves, and
  each step is stored as the turn runs. If the process dies, the next
  process that gets work for the session continues from the last stored
  step, and the model is told the turn was interrupted (`sdk/README.md`,
  "Durability"; `CHANGELOG.md`, 0.0.14).
- **Tool safety on resume.** Every tool must declare `idempotent`. On
  resume, an idempotent call runs again with the same `executionId`, which
  the tool can pass downstream as an idempotency key. A non-idempotent call
  never reruns: the model gets an error saying it may have partly run
  (`sdk/README.md`, "Tools that change things"). MCP tools default to
  non-idempotent because a server's hints aren't trusted.
- **Exactly-once inputs.** A repeated `messageId` follows the original turn
  instead of starting a new one. One turn runs at a time per session; a
  prompt that arrives during a turn queues behind it (`sdk/README.md`,
  "Sessions").
- **Resumable streams.** A turn is newline-delimited JSON with a cursor on
  each line, and a client can reconnect to any server from its last cursor
  (`sdk/README.md`, "Streaming to a browser").
- **Stores and fencing.** Sessions live in Vercel Workflow's store on
  Vercel, in files on Node.js, and in memory in a browser. A lease and a
  writer fence keep a replaced worker's writes out of the log
  (`sdk/README.md`; `sdk/durable/`).
- **Formal models.** The durable session, prompt intake, steering, agent
  routing, and tenant isolation each have a TLA+ model, and Bun drivers walk
  the real SDK code along TLC's state graphs (`tests/tla/README.md`,
  `tests/tla/sdk/`). The repository has 121 `.tla` files.

### How fx iterates and ships

- **Latency as a gate.** CI fails a pull request if `fx help`,
  `fx status --json`, or four other commands take longer than 2 ms on the
  Linux runner (`benchmarks/check_budgets.py`, `AGENTS.md`, "Benchmarks").
- **Size as a signal.** Every pull request reports binary-size change per
  target and warns past about 51 KiB of growth; a 7 MiB ceiling applies to
  the macOS arm64 release (`AGENTS.md`, "Binary Size Observability").
- **Dev channel.** After CI passes on `main`, a workflow builds every
  target with `-Dupdate-channel=dev` and publishes tarballs with SHA-256
  sidecars (`.github/workflows/dev-release.yml`). The binary checks for
  updates in the background, and `fx upgrade` installs one
  (`README.md`; `src/core/upgrade/`).
- **Versioning.** A release is a pull request that bumps `version` in
  `src/main.zig` and adds a marked changelog entry; merging it tags and
  publishes (`AGENTS.md`, "Releasing").
- **Terminal tapes.** `FX_RECORD` writes every output byte and resize to a
  tape that `fx replay` renders without a TTY, so a render bug becomes a
  golden test (`AGENTS.md`, "Reproducing Render Bugs").

### API surface

- `fx` (interactive), `fx ask` (one request; `--json` puts the answer in
  `final_output`), `fx acp` (ACP over stdio), `fx status --json`,
  `fx doctor --json`, `fx upgrade`, `fx mcp`, `fx replay` (`README.md`).
- libfx: `createFxAgent()`, `agent.session(id).prompt()`, `steer()`,
  `cancel()`, `stream(cursor)`, `wakeHandler()`, and `createFxEngine()` for
  a single conversation with no store (`sdk/README.md`).
- Extensions: skills from `.fx/skills`, `.claude/skills`, and
  `.agents/skills`; MCP servers whose schemas load on demand; nested
  `AGENTS.md` files (`README.md`).

### What's novel

Inference, from the facts above:

- **A durable turn as a library primitive.** Agent frameworks usually treat
  a turn as a request. libfx treats it as a journaled job with declared tool
  effects, fencing, and resume, behind a small API. That's the one piece we
  don't already have in some form.
- **Formal models checked against real code.** Driving the real SDK along
  TLC's state graphs is stronger than a model that only documents intent.
- **Performance budgets on every change**, measured in milliseconds, not
  release-time audits.

## Which of our areas fx bears on

| Area | Bears on it? | Why |
| --- | --- | --- |
| Website deploy pipeline (`scripts/deploy/web.sh`, `crates/openagents-web/Dockerfile`) | **Indirectly** | fx has no web deploy system. Durable turns make frequent deploys safe, and its per-target, hash-checked artifacts and budgets support the audit's plan. It doesn't change that plan. |
| Cargo replacement and content-addressed artifacts (`docs/research/faster/chat.md`) | **No** | fx uses Zig's build system unchanged. Nothing in it addresses incremental compilation or artifact registries. |
| Coder and Microcoder (`crates/coder`, `crates/microcoder-loop`) | **Yes** | Same product category. Safe git, latency budgets, idempotent tool declarations, and resumable turns apply directly. |
| Hosted chat and the web's tool loops (`crates/openagents-chat`, `crates/openagents-web/src/connections/chat.rs`) | **Yes** | These run agent turns in a process that a deploy or restart can end. |
| Pylon and NIP-CJ jobs (`docs/compute/pylon.md`) | **Yes, later** | A journal on the relay would let another worker or Pylon continue a turn. Needs a NIP-CJ change. |
| The Forge (`docs/forge/README.md`) | **Yes, as a consumer** | The Forge's rule "if a fact must survive a crashed worker ... it belongs to the Forge's records" is the same problem one level up. |
| Rust Native and the card layer | **No** | fx's UI is a terminal and a JavaScript embedding. |

A naming note: `docs/research/faster/chat.md` uses "Forge" for a build
daemon, while `docs/forge/README.md` defines the Forge as the software
factory. Pick another name for the build daemon before anyone files issues
for it.

## What's worth adopting

1. **Durable turns, with declared tool effects.** Each tool or command
   declares whether rerunning it is safe. On resume, safe calls rerun with
   a stable execution ID; unsafe calls report "may have partly run" to the
   model. We already record uncertain effects without replaying them in the
   Coder task owner (`docs/coder/runtime/task-owner.md`), so this extends a
   rule we hold, from tasks down to individual turns.
2. **Exactly-once input and resumable output.** A message ID that dedupes
   retries, and an output stream with a cursor that a client can resume
   against a different server. Our basic Coder already checks partial
   sequence numbers and stops on a gap (`crates/openagents-chat/src/basic_coder.rs`);
   resuming from the gap instead of stopping is the missing half.
3. **Hardened read-only git.** `crates/coder-delegate/src/ops.rs` defines
   `READ_ONLY_GIT` as `--no-optional-locks` plus two `-c` settings. It
   doesn't turn off `core.fsmonitor`, `diff.external`, filter drivers, or
   the pager. `crates/coder-project/src/discovery.rs` sets only
   `core.fsmonitor=false`. Inference: a repository whose own config names a
   program could run it when Coder reads its status or diff. fx's list is a
   good checklist (`src/core/shell_command/safe_git.zig`).
4. **Latency budgets as gates.** Budget `coder --help`, `coder doctor`, and
   `openagents --help` cold starts in a local script. Velocity is the
   product; a budget keeps it from eroding one dependency at a time.
5. **A deploy-time budget.** The deploy audit already asks to record build
   and deploy times. Turn that into a budget in `web.sh` that warns when a
   staging deploy of a copy-only change passes 60 seconds or a server
   change passes 5 minutes, the way fx turns latency into a check.
6. **Formal models for the journal only.** The durable journal has the
   classic failure cases: two writers, a stale worker, a crash between a
   tool's effect and its record. It's worth one small model with planted
   bugs. Don't model the rest of the system.

## Our version: durable turns

### Goal

A turn that an agent runs on our servers, a worker, or a Pylon survives a
deploy, a crash, or a restart, without running a non-idempotent effect
twice and without a second session system beside `coder-sync`.

### Design

**1. A journal crate: `crates/turn-journal` (new).** Pure types and rules,
no network or storage, like `crates/atif`:

- `TurnId`, `InputId` (caller-supplied, the dedupe key), and an
  `Epoch` fence: every write carries the epoch of the lease it was made
  under, and the log ignores writes from an older epoch.
- `Entry`: `InputAccepted`, `ModelStep` (the model's reply, stored before
  any tool runs), `CallStarted { call_id, effect }`, `CallFinished`,
  `TurnEnded { reason }`, and `TurnYielded` (a clean stop at a safe point,
  for a draining process).
- `Effect`: `Read` (rerun freely), `Idempotent` (rerun with the same
  `call_id`, passed as the idempotency key), or `Write` (never rerun).
- `resume(&[Entry]) -> Resume`: a pure function that says what the next
  process does: replay finished calls from the log, rerun safe unfinished
  calls, and turn unsafe unfinished calls into a "may have partly run"
  tool result.
- A `Store` trait with `append(turn, epoch, entry)` and `read(turn)`, so
  each surface brings its own storage.

Store steps as ATIF steps where they fit (`crates/atif` is already an
append-only step log with first-class calls), and keep only the fence,
input dedupe, and effect fields in the new crate. That gives us one trace
format, not two.

**2. Microcoder declares effects.** `crates/microcoder-loop/src/run.rs`
and `env.rs` tag each command with an `Effect`: reads (file reads,
`git status`) as `Read`, test runs as `Idempotent`, and everything else as
`Write` until shown otherwise. The loop appends to the journal at each step
boundary. On the desktop the store is a file beside the ATIF trace in
`~/.openagents/traces/`.

**3. The web server yields and resumes.** In
`crates/openagents-web/src/shutdown.rs`, a SIGTERM also tells running
turns to stop at the next safe point and append `TurnYielded`. The store
is the chat store (`crates/openagents-web/src/chat_store.rs`), which
already has compare-and-swap writes that can carry the epoch. The new
revision resumes yielded turns on first request for their chat, or from a
small sweep at start. The browser's stream reconnects with its last
cursor. First consumers: the Drive tool loop in
`crates/openagents-web/src/connections/chat.rs` (up to 8 model steps), then
the other in-process loops that `MAX_STEPS` marks.

**4. NIP-CJ jobs resume on another worker (later).** For the basic Coder
and Pylon jobs, the journal lives on the relay as NIP-44 encrypted
feedback events addressed to the worker pool's key, and the lease is a
replaceable event with an epoch. A worker that restarts, or another Pylon,
reads the journal and continues. This needs an amendment to NIP-CJ under
`nips/openagents/` and review against the NIP-SESS and NIP-CTRL revision
that AGENTS.md describes.

**5. Boundaries.** The journal records a turn's execution, not the
conversation. Live sessions stay Loro documents on `coder-sync`. A Forge
run holds its lease in the Forge's records; a durable turn is one step
inside it.

### Deploy-speed fit

Inference, tied to the audit's targets:

| Target | What fx's ideas add |
| --- | --- |
| Copy or CSS on staging in under 1 minute | Nothing directly. The audit's runtime content directory does this. Add the deploy budget from step 5 to keep it there. |
| A server change in about 5 minutes | Durable turns make a 5-minute cadence safe for users mid-turn. Without them, each deploy ends turns longer than the 8-second drain. |
| One browser module in about 10 minutes | Nothing directly. fx's per-target, SHA-256-checked artifacts match the audit's content-hashed module manifest and support it. |

### Phases

| Phase | Scope | Crates and files | Exit check |
| --- | --- | --- | --- |
| 0 | Hardened git, latency budgets, deploy budget | `crates/coder-delegate/src/ops.rs`, `crates/coder-project/src/discovery.rs`, a new `scripts/dev/startup-budget.sh`, `scripts/deploy/web.sh` | A repository with a planted `core.fsmonitor` command doesn't run it during Coder's reads; budgets print on every run |
| 1 (first milestone) | `turn-journal` types, `resume`, a file store, and Microcoder's effect tags | new `crates/turn-journal`, `crates/microcoder-loop/src/{run,env,transcript}.rs`, `crates/atif` if a field is needed | Kill Microcoder between any two steps of a scripted turn; the restarted loop finishes the turn, reruns no `Write` call, and reruns each `Idempotent` call with the same ID |
| 2 | Web yield and resume | `crates/openagents-web/src/{shutdown,chat_store}.rs`, `crates/openagents-web/src/connections/chat.rs` | A staging deploy during a running Drive answer: the answer finishes on the new revision and the browser shows no gap |
| 3 | Journal model | `docs/research/fx/` or `crates/turn-journal/model/` (TLA+ or a Rust state-machine checker), with planted bugs | Every planted bug (no fence, no dedupe, rerun a `Write`) is caught |
| 4 | NIP-CJ resume | `nips/openagents/` (NIP-CJ amendment), `crates/nostr`, `crates/coder` (`coder-worker`), `crates/pylon` | A worker killed mid-turn; a second worker continues it from the relay |

## What we won't copy

- **Zig, Node.js, and WebAssembly packaging of the agent.** Our product
  code is Rust, and TypeScript is out. libfx's browser build would add
  another WebAssembly module to the pipeline that the deploy audit is
  trying to shrink; the web chat stays a client of server or worker turns.
- **A second agent runtime.** fx is a full agent. We have Coder,
  Microcoder, and the harnesses, and the Forge README says the Forge isn't
  a new runtime. Durable turns go into the loops we have.
- **Vercel coupling.** Vercel Workflow's store and queue, AI Gateway, and
  OIDC credentials. Our stores are files, the chat store, and the relay.
- **The model-reviewed `auto` permission mode.** A model that judges each
  action is useful review, but our execution policy lives in Coder's
  `permit` and `coder-boundary`, which don't depend on a model's verdict.
  Don't move that authority into a reviewer.
- **The inline scrollback interface.** Coder's terminal has its own design
  rules; fx's UI choices don't transfer.
- **GitHub workflows.** fx's gates run in GitHub Actions. Ours run as local
  scripts, per AGENTS.md.
- **fx's session format and its system prompt.** We have ATIF and
  `coder-sync`. Write our own prompts.
- **Profile-guided optimization and the binary-size ceiling.** Valuable for
  a distributed binary; not where our time goes.

## License

Facts: fx is Apache-2.0 (`LICENSE`). Its third-party notices cover
interface sounds (cuelume, MIT) and Unicode data
(`THIRD_PARTY_NOTICES.md`).

What that means for us (inference; not legal advice):

- **Code reuse is allowed** by Apache-2.0 if we keep its copyright and
  license notices, mark changed files, and carry a NOTICE if one exists.
  It also grants a patent license from contributors, which ends for anyone
  who sues over a patent in the work.
- **No trademark rights.** Section 6 grants none, so don't name our
  feature "fx" or use Vercel's marks.
- **In practice, reuse ideas only.** fx is Zig and JavaScript; nothing
  ports to Rust by copying. Per AGENTS.md, reimplement here and credit the
  source: name "libfx durable sessions (vercel-labs/fx, Apache-2.0)" in
  the `turn-journal` crate documentation and in the commit message that
  adds it. If we adapt one of fx's TLA+ models instead of writing our own,
  keep its header and license notice in the copied file.

## Effort and risks

Estimates are inference, for one agent working in order.

| Phase | Estimate |
| --- | --- |
| 0. Git hardening, latency and deploy budgets | 1 day |
| 1. `turn-journal` and Microcoder effects | 4 to 6 days |
| 2. Web yield and resume | 1 to 2 weeks |
| 3. Journal model | 3 to 5 days |
| 4. NIP-CJ resume | 2 to 3 weeks, plus NIP review |

Risks:

- **Mislabeled effects.** A command tagged `Idempotent` that isn't can
  double-spend or double-post on resume. Default to `Write`, and require a
  test for every tool that claims otherwise. Wallet and payment tools stay
  `Write` unless they pass a real idempotency key.
- **Two writers during a traffic split.** Cloud Run can run two revisions
  at once. The epoch fence and the chat store's compare-and-swap must make
  the second writer lose; phase 3's model exists to check exactly this.
- **Write cost per step.** fx syncs to disk on every append
  (`tests/tla/session_manager/system-design.md`). A chat-store write per
  step adds latency; batch model-output steps and write only at call
  boundaries.
- **Privacy on the relay.** A journal holds tool output. On the relay it
  must be NIP-44 encrypted to the workers' key, carry no plaintext beyond
  routing tags, and expire.
- **A second session path by accident.** If the journal starts carrying
  conversation state, it competes with `coder-sync`. Keep it to execution
  records, and review phase 4 against the NIP-SESS revision.
- **A moving source.** fx is at 0.0.14, marked experimental, and changes
  daily. Recheck `sdk/README.md` before phase 4.

## Issue-ready steps

Each step can be filed as one issue. None is filed.

1. **Harden Coder's read-only git calls.** Extend `READ_ONLY_GIT` in
   `crates/coder-delegate/src/ops.rs`, and the scratch read in
   `crates/coder-project/src/discovery.rs`, to turn off the programs a
   repository's config can name: `core.fsmonitor`, `core.hooksPath`,
   `diff.external` (or pass `--no-ext-diff`), textconv, filter drivers,
   and the pager. Acceptance: a test repository with each of these set to
   a command that writes a marker file; Coder's status, diff, and log reads
   leave no marker. Credit fx's `safe_git.zig` as the checklist.
2. **Add startup latency budgets.** A script under `scripts/dev/` that
   times `coder --help`, `coder doctor`, and `openagents --help`
   cold, with per-command budgets set from today's measurements. Run it
   by hand; no GitHub workflow. Acceptance: it prints each time against
   its budget and exits nonzero on a miss.
3. **Add a deploy budget to `web.sh`.** Record build, deploy, and smoke
   times per staging deploy (the audit's "Measure it") and warn when a
   copy-only deploy passes 60 seconds or a server deploy passes 5 minutes.
   Acceptance: the summary line shows the budget and the result.
4. **Create `crates/turn-journal`.** Entries, effects, epoch fence, input
   dedupe, the pure `resume` function, a `Store` trait, and a file store.
   Reuse ATIF steps where they fit. Acceptance: table tests for every
   crash point in a three-call turn, including a stale-epoch write.
5. **Tag Microcoder's commands with effects and journal each step.**
   `crates/microcoder-loop`. Acceptance: the kill-and-restart test from
   phase 1 passes for a turn with a read, a test run, and a file write.
6. **Yield running web turns on SIGTERM and resume them on the next
   revision.** `crates/openagents-web/src/shutdown.rs`, `chat_store.rs`,
   and `connections/chat.rs`; the browser reconnects from its last cursor.
   Acceptance: a staging deploy during a running Drive answer, checked with
   the test account, finishes the answer once.
7. **Model the journal.** A small model with planted bugs (no fence, no
   dedupe, rerun of a `Write` call). Acceptance: each planted bug fails
   the check, and the real model passes.
8. **Draft a NIP-CJ resume profile.** Journal events, the lease event, and
   encryption rules, in `nips/openagents/`. Acceptance: reviewed against
   the NIP-SESS and NIP-CTRL revision; no code until it's accepted.
9. **Resume NIP-CJ turns on another worker.** `coder-worker` and `pylon`.
   Acceptance: a worker killed mid-turn; a second worker finishes it, and
   the phone shows one continuous answer.
10. **Rename the build daemon in `docs/research/faster/chat.md` follow-ups.**
    Choose a name other than "Forge" before filing build-daemon issues.

# Grok Build Rust harvest for Omega

- Class: current source audit and port recommendation
- Status: recommended harvest plan
- Date: 2026-07-24
- Audience: product, Omega, Nostr, agent, security, and assurance teams
- Grok teardown source: `c1b5909ec707c069f1d21a93917af044e71da0d7`
- Current Grok source: `6e386420825bd44ae648c63e7c8cba12fcec9401`
- Current Omega source: `af1a0983f7fcada27ab0ee7bdf407b2f98341a2c`
- OpenAgents source: `93bbdd70b1c356332b310a86c158995767b57293`

## Decision

Omega should harvest selected Grok Build agent behavior, tests, and small Rust
components.

Omega should not port the Grok CLI, terminal UI, leader runtime, ACP wrapper,
tool protocol, or complete session engine.
Omega already has the Zed editor, GPUI, ACP 2.0, project, worktree, agent,
sandbox, database, updater, and process-owner systems.
A second copy of those systems would split authority and increase rebase cost.

The goal should be complete parity for the selected Grok agent behaviors that
fit Omega.
It should not be complete Grok product parity.
The two products have different hosts and different protocol boundaries.

The best direct Rust candidates are:

1. the network-file-system journal selection from `xai-sqlite-journal`
2. the generic failure control from `xai-circuit-breaker`

The best behavior candidates are:

1. conflict-aware checkpoint restore
2. least-privilege subagent resolution
3. exact reconnect and replay tests
4. terminal-state rules for tool streams
5. deterministic workflow recovery tests

The highest-value user feature for this week is conflict-aware checkpoint
restore.
The highest-value agent foundation is a pure subagent policy resolver.

## Recommended target for this week

Aim for five bounded packets in the existing Omega architecture.

| Order | Packet | Result by the end of the week |
| ---: | --- | --- |
| 1 | Conflict-aware restore | Omega previews user changes before it restores an agent checkpoint |
| 2 | Subagent policy resolver | Child model, role, isolation, and capability rules resolve with least privilege |
| 3 | Database file-system guard | Omega detects unsafe WAL locations and applies an explicit database policy |
| 4 | Failure control | One external agent lane uses a visible and tested circuit breaker |
| 5 | Agent reliability pack | Reconnect, duplicate-effect, queue, and tool-terminal tests protect existing owners |

This target is ambitious and reasonable.
It does not require a parallel runtime or a new user interface framework.
It adds the parts of Grok that make agents safer and more dependable.

Do not block all five packets on one large design.
Each packet must have its own source review, tests, and acceptance gate.
If time becomes limited, finish packets 1, 2, and 5 before packets 3 and 4.

## Audit boundary

This audit read these OpenAgents teardowns:

- [Grok Build](../teardowns/2026-07-15-grok-build-teardown.md)
- [Cursor](../teardowns/2026-07-11-cursor-product-teardown.md)
- [Visual Studio Code](../teardowns/2026-07-18-vscode-teardown.md)
- [Zed](../teardowns/2026-07-18-zed-teardown.md)

The audit also compared current Grok source with current Omega source.
The Grok teardown used the July 15 source snapshot.
This audit also inspected the July 24 Grok export because its crate set and
implementation changed after the teardown.

The current source snapshots are:

- [Grok Build at `6e386420`](https://github.com/xai-org/grok-build/tree/6e386420825bd44ae648c63e7c8cba12fcec9401)
- [Omega at `af1a0983`](https://github.com/OpenAgentsInc/omega/tree/af1a0983f7fcada27ab0ee7bdf407b2f98341a2c)

The Grok repository is a periodic export from another repository.
It has little public history and does not accept outside contributions.
This makes a pinned source review more important than an upstream dependency.

This document is an analysis record.
It does not admit an implementation packet or change an authority boundary.
Current Omega code, tests, accepted specifications, and receipts remain the
implementation truth.

## How the editor teardowns change the answer

The Cursor, Visual Studio Code, and Zed teardowns make a broad Grok port the
wrong approach.

| Teardown | Relevant lesson | Omega consequence |
| --- | --- | --- |
| Cursor | A tracked fork can add a wide agent product | Put recovery and Grok behavior inside Omega's existing owners |
| Visual Studio Code | Stable package boundaries support agent overlays | Use clear crate interfaces and keep native Rust owners |
| Zed | Project, editor, GPUI, worktree, terminal, action log, and agent systems already exist in Rust | Extend those systems instead of porting Grok substitutes |
| Grok | A terminal-first agent can have strong persistence, recovery, subagent, and fault tests | Harvest its invariants and small leaf code, not its terminal host |

Omega is not an empty Rust desktop application.
It is the tracked Zed fork and the primary OpenAgents desktop destination.
The correct seam is therefore a native Omega feature or a small shared Rust
component.

## Current overlap

Grok and Omega now have extensive overlap.
This overlap is the main reason to avoid a source-wide port.

Omega already has these owners:

- `agent` and `agent_ui` own native threads, messages, queues, tools, and agent
  presentation.
- `acp_thread` and `acp_tools` own ACP session behavior and tool updates.
- `agent_servers` owns local agent process setup and connection behavior.
- `project`, `worktree`, `workspace`, and `git` own repository state.
- `action_log` and `buffer_diff` own edit attribution and diff behavior.
- `db` and `session` own local data.
- `sandbox` and tool permission code own local execution policy.
- `auto_update` owns application update behavior.
- `omega_effectd` owns the Full Auto process boundary and generation fence.

Current Omega ACP support can create, load, close, and resume sessions.
It can request history, prompt, cancel, retry, and truncate.
It also supports model and profile selection, extra directories, permissions,
and request elicitations.

Current Omega thread storage includes metadata, messages, usage, model state,
profile state, parent state, subagent context, draft state, scroll state, and
sandbox grants.
This is already a substantial session model.

The harvest must improve these owners.
It must not route around them.

## Source scale

The current Grok export is too large and too coupled for broad incorporation.
The largest crates include terminal UI, shell, tools, worktree, rendering,
agent, and Markdown systems.

The useful candidates have very different sizes and coupling.

| Grok crate | Approximate Rust lines | Approximate tests | Recommended use |
| --- | ---: | ---: | --- |
| `xai-sqlite-journal` | 779 | 19 | Adapt source and tests |
| `xai-circuit-breaker` | 2,178 | 70 | Adapt source and tests |
| `xai-prompt-queue` | 475 | 15 | Test ideas only |
| `xai-interjection-core` | 320 | 14 | Test ideas only |
| `xai-grok-subagent-resolution` | 3,023 | 104 | Reimplement pure policy and port tests |
| `xai-workflow` | 3,329 | 59 | Recovery and replay tests only |
| `xai-tool-runtime` | 5,391 | 141 | Protocol invariants only |
| `xai-fast-worktree` | 22,754 | 409 | Benchmark first, then extract small algorithms |
| `xai-hunk-tracker` | 13,012 | 200 | Conflict behavior and fixtures only |

Line counts are an audit aid.
They are not a quality score.
They show why a leaf component can move while an actor or runtime should not.

### File-level shortlist

Use this file list for the first source review.
The list does not approve a copy before license and owner review.

| Grok source | Omega target | Disposition |
| --- | --- | --- |
| `xai-sqlite-journal/src/lib.rs` | `db` and its current SQLite owner | Adapt implementation and tests |
| `xai-circuit-breaker/src/breaker.rs` | The selected external service owner | Adapt core state transitions |
| `xai-circuit-breaker/src/clock.rs` | The selected external service owner | Adapt deterministic clock seam |
| `xai-circuit-breaker/src/config.rs` | Omega settings for the selected lane | Adapt validated settings |
| `xai-circuit-breaker/src/observer.rs` | Typed Omega status and UI state | Adapt events and remove Grok logging policy |
| `xai-circuit-breaker/src/registry.rs` | The selected service owner | Port only if the first lane needs more than one breaker |
| `xai-circuit-breaker/src/retry_policy.rs` | The selected service owner | Adapt only for idempotent operations |
| `xai-circuit-breaker/src/state.rs` and `window.rs` | The selected service owner | Adapt state and sliding window tests |
| `xai-grok-subagent-resolution/src/overrides.rs` | `agent` subagent policy | Reimplement precedence with Omega types |
| `xai-grok-subagent-resolution/src/resume.rs` | `agent` session resume policy | Reimplement identity checks |
| `xai-grok-subagent-resolution/src/types.rs` | `agent` subagent policy | Use as behavior evidence only |
| `xai-hunk-tracker/src/diff.rs` and `types.rs` | `action_log` and `acp_thread` | Use conflict classes and fixtures only |
| `xai-workflow/src/journal.rs` and `validate.rs` | `omega_effectd` tests | Use replay and validation tests only |
| `xai-tool-runtime/src/streaming.rs` | ACP tool tests | Use terminal-state invariants only |

Also inspect each candidate's complete test directory.
Tests can contain platform assumptions that the public Rust type does not show.

## Harvest class A: direct Rust candidates

### Network-file-system journal policy

Grok's
[`xai-sqlite-journal`](https://github.com/xai-org/grok-build/tree/6e386420825bd44ae648c63e7c8cba12fcec9401/crates/codegen/xai-sqlite-journal)
is the strongest direct source candidate.

It identifies local and network file systems.
It selects WAL for local storage and a rollback journal for network storage.
It has Linux, macOS, and Windows detection paths.
It has tests for NFS, SMB, CIFS, FUSE, Ceph, Lustre, GPFS, OCFS, Weka, UNC,
and mapped drives.

Current Omega
[`db.rs`](https://github.com/OpenAgentsInc/omega/blob/af1a0983f7fcada27ab0ee7bdf407b2f98341a2c/crates/db/src/db.rs#L130)
sets `PRAGMA journal_mode=WAL` without a file-system check.
This is a real and narrow gap.

Adapt the detection and policy into Omega's existing database layer.
Do not add a second bundled SQLite dependency.
Use the connection and error types that `sqlez` and `db` already own.

The database policy must distinguish two data classes:

- A rebuildable index can use a host-specific path.
- Authoritative thread or workspace state must not silently split by host.

Do not copy Grok's host-specific path rule to an authoritative Omega database.
For authoritative state, use one explicit supported journal policy or refuse the
unsupported location with a useful error.

Acceptance requires:

- deterministic file-system classification tests
- a test for an unknown file-system result
- a test for a local file system
- a test for each supported network family
- a visible error when Omega cannot give authoritative state safe storage
- no second SQLite owner

### Circuit breaker

Grok's
[`xai-circuit-breaker`](https://github.com/xai-org/grok-build/tree/6e386420825bd44ae648c63e7c8cba12fcec9401/crates/common/xai-circuit-breaker)
is the second strong source candidate.

It supplies a sliding failure window, minimum sample count, open state,
half-open state, probe recovery, observer hooks, an injectable clock, a
registry, and retry policy.
Its dependencies are small.
Its test set is much larger than its public surface.

Current Omega has local retry and backoff logic.
The audit did not find one general circuit breaker with the same state model.

Adapt the component to one external failure lane first.
Good first lanes are ACP child readiness, an MCP endpoint, or the
`omega-effectd` health boundary.
Choose one owner for the first integration.

The breaker must not turn a failed operation into a false success.
It must publish typed degraded state to the UI.
It must preserve the final error and the time of the next probe.
It must not retry a non-idempotent mutation unless that mutation has a stable
idempotency key.

Acceptance requires:

- deterministic clock tests
- abandoned half-open probe recovery
- concurrent caller tests
- a visible open-state result
- one bounded integration lane
- no global breaker that hides the identity of the failed provider

## Harvest class B: behavior and test ports

### Conflict-aware checkpoint restore

This is the best user-facing Grok behavior to add to Omega.

Grok's hunk and rewind code distinguishes agent changes from external changes.
It can preview a restore and identify files that another actor modified,
created, or deleted.
It supports conversation-only, file-only, and complete rewind modes.
It also records rewind markers and protects recovery after compaction.

Omega already has most of the correct foundation:

- `action_log` records `ChangeAuthor::Agent` and `ChangeAuthor::User`.
- It tracks buffers and buffer diffs.
- It can rebase non-conflicting edits.
- `acp_thread` has checkpoints and conversation rewind behavior.
- The Git layer can restore a checkpoint.

The current restore path cancels active generation, rewinds the conversation,
and performs a Git restore.
The user does not first receive a complete external-change preview.
The current Git path also has known limits for untracked and binary files.

Do not port the 13,000-line Grok hunk actor.
Extend the existing Omega action log and checkpoint model.

The new flow should:

1. freeze new agent mutation for the selected thread
2. compare the checkpoint, the recorded post-agent state, and current files
3. classify each file as safe, externally changed, externally created, or
   externally deleted
4. show the preview before the first destructive file operation
5. preserve user changes by default
6. offer conversation-only restore
7. require an explicit choice before overwrite
8. keep retry data when one file operation fails
9. record one durable restore receipt

The restore receipt should include checkpoint identity, project identity,
selected mode, affected paths, conflict classes, and result hashes.
It must not include file content by default.

This feature also fits a deeper Nostr model.
Omega can sign a receipt hash when the user wants portable proof.
The Nostr event must contain explicit allowed fields.
It must not contain prompt text, diff text, or file content by default.

Acceptance requires Grok-style conflict matrix tests and current Omega GPUI
tests.
Tests must cover untracked files, binary files, deleted files, partial failure,
cancel during generation, retry, and conversation-only restore.

### Least-privilege subagent resolution

Grok's
[`xai-grok-subagent-resolution`](https://github.com/xai-org/grok-build/tree/6e386420825bd44ae648c63e7c8cba12fcec9401/crates/codegen/xai-grok-subagent-resolution)
contains useful pure policy.

It resolves explicit settings, roles, personas, parent settings, model choice,
reasoning choice, isolation, and capability modes.
It intersects capabilities by least privilege.
It limits nested agent tools by depth.
It also validates role and persona identity when a child resumes.

Current Omega's `SpawnAgentToolInput` has a label, message, and optional
session identifier.
Omega can create and resume child threads and can choose a configured subagent
model.
The audit did not find equivalent role, persona, isolation, and capability
resolution in that input path.

Do not import the Grok crate as-is.
It depends on Grok agent, sampling, and tool types.
Write an Omega-native pure resolver with Omega types.

The precedence should be explicit and testable:

1. the owner grant sets the maximum authority
2. the parent agent grant sets the child ceiling
3. the selected role or persona can reduce that ceiling
4. the task request can reduce it again
5. the child receives the intersection

No child can gain a tool, path, relay scope, payment limit, signing scope, or
network scope that the parent does not have.
Model text is not an authority source.

The resolver should also prepare Omega for Nostr-native external agents.
A role or persona can reference a signed Nostr profile or service offer.
A signed reference supplies identity and advertised capability evidence.
It does not supply local authority by itself.
Local owner policy still grants the executable capability.

The cloud boundary remains optional.
Cloud capacity can satisfy an admitted task, but it cannot expand the signed
or local capability grant.
ACP remains a local agent adapter.
Nostr remains the primary external identity, discovery, delegation, and receipt
plane.

Acceptance requires tests for precedence, capability intersection, depth,
resume identity, model pinning, isolation, and an expired or revoked Nostr
grant.

### Reconnect and replay invariants

Grok's leader tests are more valuable than its leader transport.

The Grok leader supports multiple clients, per-client capabilities, session
fan-out, live-load buffering, version checks, reconnect, session reload, and a
bounded update relaunch.
The corresponding tests cover difficult process and replay races.

Omega already has agent process owners and a generation-fenced
`omega-effectd` supervisor.
It should not add Grok's framed local protocol as another engine.

Port these invariants to Omega tests:

- A stale process generation cannot mutate current state.
- A completed turn replays once after reconnect.
- Reconnect does not run model inference a second time.
- A tool effect with a durable result does not execute twice.
- Cancel and completion remain ordered after reattach.
- An old binary cannot replace a newer runtime.
- An update stops new admissions before it drains current work.
- A bounded timeout ends the drain.
- Session capabilities are explicit after reconnect.

Place each test with the current owner.
Use `agent_servers` for process state, `acp_thread` for session replay, and
`omega_effectd` for generation fencing.
Do not create a new shared leader only to host the tests.

### Tool stream terminal rules

Grok's tool runtime defines a useful stream rule.
A tool has zero or more progress updates and exactly one terminal update.
Late progress is invalid.
The runtime limits model-visible output.

Omega already receives typed ACP 2.0 tool updates.
It already has terminal, diff, permission, and status presentation.
The rule should become an invariant of the existing ACP and local tool path.

Add property and integration tests for these cases:

- success has one terminal state
- failure has one terminal state
- cancellation terminalizes each pending tool
- the owner rejects late progress or records a protocol error
- reconnect does not add a second terminal state
- tests measure output bounds in bytes and verify UTF-8
- truncated output says that truncation occurred

Do not port `xai-tool-protocol` or `xai-tool-runtime` as a parallel protocol.
Use ACP 2.0 and current Omega tool state.

### Workflow recovery tests

Grok's `xai-workflow` crate has good deterministic recovery behavior.
It uses sequence checks, request hashes, a bounded journal, torn-tail recovery,
dry-run hosts, quotas, and replay divergence checks.

Omega already has Full Auto and `omega-effectd`.
A Rhai engine would create a second workflow language and a second run
authority.
Do not port that engine.

Port these test concepts into the existing Full Auto action and receipt model:

- dense sequence numbers
- bounded journal size
- torn final record recovery
- request-hash replay divergence
- deterministic dry-run hosts
- explicit tool and token quotas
- a refusal when replay inputs do not match

The durable Full Auto run remains the authority.
GPUI remains a projection and command surface.

## Harvest class C: compare before implementation

### Fast worktree creation

Grok's
[`xai-fast-worktree`](https://github.com/xai-org/grok-build/tree/6e386420825bd44ae648c63e7c8cba12fcec9401/crates/codegen/xai-fast-worktree)
uses copy-on-write and parallel file operations.
It has platform logic for Btrfs, overlay, and APFS.
It also has pool, metadata, synchronization, and cleanup logic.

Omega already owns native Git worktree creation and managed worktree life.
It has archive references and checks before cleanup.
A second metadata database or worktree pool would split that authority.

First measure Omega on large repositories.
Record median, tail, disk, ignored-file, dirty-file, cancellation, and cleanup
results.
If the result shows a material gap, extract only the copy-on-write and skip
algorithms behind Omega's current `GitRepository::create_worktree` boundary.

Do not port Grok's pool, garbage collector, metadata database, clone promotion,
or platform delegate as one package.

### Telemetry firewall

Grok's external telemetry exporter has a strong content firewall.
It requires double consent, uses an allow-list, removes secrets, limits string
and object sizes, and drops invalid events before transmission.

Omega does not need the complete Grok telemetry system.
If Omega later adds customer-owned OTLP export or Nostr receipt telemetry,
port the schema firewall and its tests first.

Never publish prompts, tool arguments, file content, or terminal content to
Nostr by default.
Nostr receipts must use a separate explicit schema with hashes and approved
metadata.

### Prompt queue and interjection

Grok has small crates for prompt queues and mid-turn input.
Omega already has a GPUI message queue with stable identifiers, edit state,
pause, resume, fast track, send-now, cancellation protection, and steer state.
The agent thread already supports stop at the next boundary.

Do not port the Grok queue crates.
Port or strengthen tests for these distinctions:

- queue does not mean steer
- steer does not mean cancel and send now
- cancel does not cause a queued message to send twice
- a message under active edit does not send
- command, skill, image, and text prompts do not merge without an explicit
  compatible rule
- attribution and version remain stable if a queue becomes shared

Do not add Grok's synthetic user-query wrapper to Omega's ACP path.

## Components that must not move

The following code should not move into Omega as a system.

| Grok area | Reason |
| --- | --- |
| Pager, Ratatui, terminal renderer, TTY, and PTY host | GPUI and Zed own the Omega host |
| Shared leader transport | Agent servers and `omega_effectd` already own process life |
| `xai-acp-lib` | It targets an older ACP generation while Omega uses ACP SDK 2.0 |
| Computer Hub | It would create another tool and side-effect authority |
| Grok shell and agent core | They duplicate Omega thread, tool, session, and provider owners |
| Grok chat state and sampler | They duplicate ACP and current model integration |
| Complete Grok sandbox | Its teardown shows fail-open and scope gaps that Omega must not copy |
| Complete updater | Omega already has an updater and Grok provenance checks are incomplete |
| Rhai workflow engine | It would create a second Full Auto language and run authority |
| Complete fast-worktree service | It would duplicate project and worktree life-cycle truth |
| Provider auth and secret code | It is provider-specific and can weaken Omega's signer and secret boundaries |

Grok's large PTY, race, and failure suite remains useful as scenario evidence.
Translate scenarios into GPUI, ACP, project, and supervisor tests.
Do not translate terminal rendering implementation.

## Nostr-primary integration rule

The Grok harvest must make Omega more Nostr-native, not less Nostr-native.

Use this boundary:

- GPUI owns local interaction.
- Omega Rust owners enforce local capability, file, process, and payment policy.
- ACP connects local or installed agents to Omega.
- Nostr is the primary external plane for identity, discovery, delegation,
  offers, results, revocation, and portable receipts.
- Cloud services can provide optional compute or durable support after local
  admission.
- A cloud service cannot become the source of identity or expand authority.

Grok's local reliability logic is useful below this boundary.
It can make Nostr work more dependable through replay protection, capability
intersection, conflict-safe restore, and durable receipts.

Do not expose a local socket, Grok leader frame, or Computer Hub message as a
new public OpenAgents protocol.
Do not make ACP the external agent identity plane.
Do not put raw model context in Nostr events.

## Source and license controls

Grok identifies first-party code as Apache-2.0 and includes third-party
notices.
Omega is GPL-3.0-or-later.
The two licenses usually permit this combination.
Each source file and bundled dependency still needs review.
This document is not legal advice.

The root license is not enough for all Grok files.
Some tool code identifies Codex or OpenCode origins and has local notice duties.

For every direct source port:

1. pin the source to `6e386420825bd44ae648c63e7c8cba12fcec9401`
2. identify the exact source files
3. confirm first-party and third-party ownership for each file
4. preserve required notices and copyright text
5. record material Omega modifications
6. avoid a whole-repository vendor import
7. run the Omega license inventory before merge

Prefer an adapted, owned Omega component over a runtime dependency on the Grok
export.
The export has no normal public contribution path and can change as one large
sync.

## Proposed execution plan

### Day 1: freeze boundaries and tests

- Record exact source files and licenses for the two direct candidates.
- Add failing conflict-preview tests to the current checkpoint owner.
- Add failing subagent policy tests to a pure Omega test target.
- Add reconnect and tool-terminal cases to current owners.
- Measure worktree creation without changing its architecture.

### Days 2 and 3: conflict-aware restore

- Add the three-state comparison to `action_log` and checkpoint types.
- Add a GPUI preview with safe defaults.
- Keep conversation-only restore available.
- Add retry and receipt behavior.
- Test untracked, binary, deleted, modified, and partial-failure cases.

### Day 3: subagent resolver

- Define role, persona, model, isolation, and capability inputs.
- Implement pure precedence and least-privilege intersection.
- Validate resume identity and depth.
- Add signed Nostr grant references without making them local authority.

### Day 4: two leaf components

- Adapt file-system detection and journal selection into the current database
  owner.
- Adapt the circuit breaker into a small Omega-owned module.
- Keep integration limited to one external lane.

### Day 5: integration and proof

- Run the agent reliability pack.
- Run database tests on supported file-system fixtures.
- Run one failure-control integration test.
- Run checkpoint GPUI tests.
- Record source provenance and license results.
- Decide if fast-worktree work has measured justification.

## Packet acceptance matrix

| Packet | Must prove | Must not do |
| --- | --- | --- |
| Conflict-aware restore | User edits survive by default and restore is retryable | Run a destructive restore before preview |
| Subagent resolver | Child authority is the intersection of all grants | Treat model prose or remote identity as permission |
| Database guard | Network storage gets an explicit safe policy | Split authoritative state into hidden host copies |
| Circuit breaker | Omega limits and shows each failure | Hide errors or retry unsafe mutations |
| Reliability pack | Replay and reconnect do not duplicate effects | Create a second agent runtime for test convenience |

## Risks and controls

### Risk: duplicate architecture

A direct port can appear faster but create a second owner for sessions, tools,
worktrees, or process life.

Control: every packet names an existing Omega owner before implementation.

### Risk: false parity

Matching Grok crate names can look like parity while user behavior remains
different.

Control: define parity with user-visible and fault-visible tests.

### Risk: weaker authority

Grok has documented sandbox, hook, path, prefix, and local IPC gaps.

Control: do not copy those defaults.
Use Omega's local policy, signed Nostr grant, and explicit owner admission.

### Risk: replayed side effects

Reconnect and workflow replay can execute a tool twice.

Control: require stable mutation identity, a durable result, and a replay test
before automatic recovery.

### Risk: source drift

The Grok export can replace many files in one sync.

Control: pin every source port and make Omega own the adapted component.

### Risk: too much work in one week

Five packets can expand if they cross owner boundaries.

Control: complete conflict restore, subagent policy, and reliability tests
first.
Keep database and circuit-breaker integration narrow.

## Follow-up harvest after this week

Consider these items only after the first five packets are complete:

1. copy-on-write worktree acceleration after benchmark proof
2. external telemetry schema firewall when a real exporter exists
3. stronger compaction and summary recovery after a measured Omega gap
4. shared queue attribution when a Nostr or multi-client queue exists
5. broader circuit-breaker use after the first lane proves good UI behavior

Do not plan a complete port of Grok memory, sampler, MCP, plugin, updater,
sandbox, or terminal systems.
Compare one behavior at a time when Omega has a measured gap.

## Final recommendation

Move fast toward full parity in the selected agent behaviors.
Do not move toward full source parity with a terminal application.

This week, Omega should deliver conflict-safe restore, least-privilege subagent
resolution, and the cross-owner reliability test pack.
It should also adapt the SQLite file-system guard and one circuit-breaker lane
if those first three results stay on schedule.

That plan takes Grok's best agent engineering and keeps Omega native to Zed,
GPUI, ACP 2.0, and Nostr.

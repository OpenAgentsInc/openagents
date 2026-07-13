# OpenChamber Whole-Product Teardown — 2026-07-12

## TL;DR

OpenChamber is the strongest open reference in this teardown set for the
**persistent coding workroom as a product**. It is not merely an OpenCode chat
skin. At v1.16.0 it presents one shared React workroom through web/PWA,
Electron, Capacitor mobile, and VS Code; connects that workroom to a local or
remote OpenCode runtime; keeps files, Git, worktrees, terminal, permissions,
questions, schedules, notifications, voice, and remote reachability near the
conversation; and increasingly moves unattended behavior into the server.

Its most important product lesson is the same one the earlier OpenAgents
research identified, now with much stronger evidence: the transcript is the
coordination spine, not the whole application. A useful coding agent product
surrounds the transcript with durable sessions, causal tool activity, blockers,
review state, target/runtime state, and fast ways to continue from another
surface.

Its most important architectural warning is newer. OpenChamber 1.16.0 markets
Session Goals as server-side autonomy that continues with the app or tab
closed. That is true only while an OpenChamber server remains alive, and the
tagged implementation does not completely recover an already-idle goal after a
server restart. The goal runtime is event-driven, keeps its timers and in-flight
set in memory, performs no startup scan or backfill, and has no tagged tests.
Persisted goal metadata is not the same thing as a durable continuation lease.
The scheduler has the same boundary: it is server-owned but timer-backed, skips
downtime occurrences, and has no durable run claim.

The OpenAgents decision is therefore:

- **Harvest the product frame:** one persistent workroom, dense session
  navigation, typed turns and blockers, review beside conversation, compact
  attention, mobile continuity, and one server-owned autonomy projection.
- **Harvest the protocol ideas:** event coalescing, touched-field updates,
  explicit reconnect states, sequence/ACK dictation, pending-permission
  reconciliation, stale-goal-write fencing, generic push notifications, and a
  relay that grants reachability but not authority.
- **Strengthen the authority:** Effect Schema contracts, Runtime Gateway and
  Khala Sync projections, startup reconciliation, durable continuation/run
  leases, generation and cursor fencing, scoped capability policy, typed
  outcomes, receipts, and deterministic acceptance predicates.
- **Do not port the implementation stack:** no React/Zustand/Express state
  architecture, Capacitor application strategy, browser-facing raw OpenCode
  authority, blanket auto-accept, duplicated TypeScript/JavaScript crypto
  protocol, or renderer-held runtime credentials.

```text
OpenChamber v1.16.0

  React workroom
  web / PWA / Electron / Capacitor / VS Code
                    |
         SDK + RuntimeAPIs + events
                    |
       OpenChamber Express server
     /      |        |         \
 OpenCode  host     relay     server-owned
 runtime   Git/FS/  transport goals/schedule/
           PTY                notify/voice

OpenAgents adaptation

  Effect Native Desktop + mobile
                    |
       typed, tokenless Runtime Gateway
                    |
  owning runtime services + Khala Sync projection
                    |
 durable intents / leases / evidence / outcomes
```

## 1. Snapshot identity, provenance, and confidence

This is a read-only source teardown of:

| Fact | Value |
| --- | --- |
| Repository | `https://github.com/openchamber/openchamber.git` |
| Local reference | `projects/repos/openchamber` |
| Tag | `v1.16.0` |
| Commit | `e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5` |
| Release commit date | 2026-07-13 in the commit's `+03:00` timezone; 2026-07-12 in this workspace's America/Chicago timezone |
| License | MIT |
| Package manager | Bun 1.3.14 |
| Engine floor | Node 22 |
| OpenCode SDK | `@opencode-ai/sdk` 1.17.18 |

The local checkout was not modified. Source was read from the exact tag with
`git show v1.16.0:<path>` after fetching the current upstream refs.

Evidence labels follow the [teardown convention](./README.md):

- **`[source]`** means directly observed in the tagged source or checked-in
  module documentation.
- **`[test]`** means encoded in a tagged test or compatibility oracle.
- **`[public]`** means asserted in the tagged public README or product docs.
- **`[history]`** means visible in the repository's tagged commit history.
- **`[inferred]`** means the conclusion combines multiple observations.
- **`[limitation]`** names what this source snapshot does not prove.

No live OpenChamber service, signed release artifact, external relay Worker,
provider backend, or private user data was inspected. Source can prove a
contract or implementation path; it cannot prove production availability,
operator practice, signing custody, or the behavior of services outside this
repository.

## 2. Product thesis: OpenCode, everywhere, as a workroom

OpenChamber describes itself as “OpenCode, everywhere”—Desktop, browser, and
phone. The substantive product is a rich control and review layer around an
OpenCode server rather than another agent engine. `[public][source]`

The product combines five ideas:

1. **One continuing session.** Start in a terminal/runtime, continue through a
   richer GUI, inspect it on mobile, and return without manufacturing a new
   conversation.
2. **One operational frame.** Sessions, messages, tool work, Git, files,
   terminals, plans, questions, permissions, and review remain close together.
3. **Several runtime placements.** The UI may use a bundled local OpenCode,
   an external OpenCode server, a remote OpenChamber host, SSH port forwarding,
   a public tunnel, or the private relay.
4. **Several client shells.** Web/PWA, Electron, Capacitor iOS/Android, and VS
   Code share most of the workroom while native hosts provide different
   capabilities.
5. **Server-owned unattended behavior.** Schedules, Session Goals, permission
   auto-accept, notifications, relay presence, and server-side speech services
   continue without the active browser component when their server remains
   alive.

This is a coherent product sentence. It avoids treating “chat,” “agent,”
“terminal,” “mobile,” and “remote” as separate applications. The workroom is
the client; OpenCode is the underlying agent runtime; OpenChamber's server is
the adapter and local capability host.

The boundary is also a limitation. OpenChamber inherits OpenCode's session and
execution authority rather than defining a provider-neutral durable workroom
authority of its own. Its continuity is principally “reach the same server and
session again,” not “move one generation-fenced execution session between
authorized hosts with a durable checkpoint and receipt chain.” `[inferred]`

## 3. Repository and runtime anatomy

The monorepo has six packages:

| Package | Responsibility |
| --- | --- |
| `packages/ui` | Shared React workroom, runtime clients, Zustand stores, event pipeline, projections |
| `packages/web` | Vite web/PWA assets, Express server, CLI, host capabilities and services |
| `packages/electron` | Native Desktop shell, in-process web server, windows, menus, updater, notifications, deep links, SSH and host switching |
| `packages/mobile` | Capacitor wrapper, iOS/Android native projects, secure storage, QR, push, app lifecycle |
| `packages/vscode` | Extension host, webview, OpenCode lifecycle and host-capability bridge |
| `packages/docs` | Product documentation and localization |

The implementation stack is React 19, TypeScript, Vite, Tailwind 4, Zustand,
Base UI/Radix/HeroUI, Express 5, Electron 41, Capacitor 8, and a mixture of
TypeScript and JavaScript server modules. `[source]`

### 3.1 Electron

Electron starts the OpenChamber web server **inside the Electron main Node
process** and loads the shared UI from a loopback origin. It does not run the
web server as a separate sidecar. The desktop package bundles a matching
OpenCode CLI, web assets, native PTY dependencies, updater, and platform
resources. `[source]`

This is operationally simple and gives the same server-owned features to the
Desktop. It also means a real application quit terminates the server and any
goal/schedule timers it owns. Hiding the window on macOS or to the tray keeps
them alive; quitting does not. A headless `openchamber` service is the actual
always-on host option. `[source][inferred]`

Electron enables `contextIsolation` and disables renderer Node integration,
but sets `sandbox: false` and `webviewTag: true`. The preload exposes a generic
`invoke(command, args)` bridge on every loaded page; main process gates commands
by sender origin and a remote-safe allowlist. Local pages also receive a client
token and runtime headers through renderer-visible globals. Navigation and
window opening are origin-checked, and local file grants are separately
validated. `[source]`

This is materially safer than unrestricted Node in the renderer, but it is not
the OpenAgents target. OpenAgents requires a sandboxed local renderer, no
renderer credential, and a closed schema-decoded intent/projection bridge
rather than a generic command dispatcher plus origin gate.

### 3.2 Web and headless service

The published `@openchamber/web` package owns the Express server and CLI. It
can start and supervise OpenCode, attach to an existing OpenCode host, expose
the UI on LAN, install itself as a login service, run behind Cloudflare, or run
API-only for a remote client. Its README gives both systemd and native startup
service patterns. `[public][source]`

That is the real persistence mechanism behind “close the tab.” Browser state
is disposable because the server, OpenCode process, event watcher, goals,
schedules, relay, and host capabilities remain elsewhere. This is the right
ownership direction even where individual services are not durably recovered.

### 3.3 VS Code

The extension embeds the shared UI in a webview but has its own bridge modules
for Git, filesystem, configuration, settings, provider/quota state, and
OpenCode proxying. Editor commands add selections/files as context, create or
open sessions, and expose Agent Manager behavior. `[source]`

This demonstrates the value of a shared product projection with host adapters.
It also demonstrates the cost of parity: capability routes and lifecycle logic
are mirrored between the web server and VS Code bridge. OpenAgents should keep
one canonical contract and explicit host capability matrix so parity gaps are
typed rather than patched per shell.

### 3.4 Mobile

Mobile is the same web application wrapped in Capacitor, not an independently
modeled native workroom. It adds secure storage, QR scanning, push,
keyboard/status-bar integration, deep links, and platform projects. Android
uses SSE where native WebSocket behavior is unreliable. `[source]`

The product choices are useful; the application architecture is not the
OpenAgents target. Effect Native should share domain programs and contracts
while rendering mobile state as a mobile surface, not wrap the Desktop/web
layout in a native WebView.

## 4. Workroom information architecture

The earlier June report correctly identified OpenChamber's durable shape:

```text
project / worktree / session rail
             |
      turn-based timeline  <---->  files / Git / diff / context / review
             |
    composer + mode/model/voice + blockers
```

### 4.1 Left rail

The rail is not generic navigation. It is a dense project/worktree/session
directory with dates, titles, lifecycle and attention state, scheduled-task
entry, goal markers, archives, and responsive collapse. It lets repeated work
remain cheap to resume. `[source]`

This is worth preserving in OpenAgents, but row text must come from canonical
user-visible title/summary fields. Internal context envelopes, environment
payloads, provider protocol fragments, raw paths, or prompt prefixes are not
session titles. A compact rail magnifies projection mistakes because malformed
metadata becomes the user's primary history index.

### 4.2 Center timeline

The center is turn-oriented and renders assistant text, reasoning, tool parts,
commands, file edits, diffs, plans, todos, errors, permissions, questions,
queued input, and completion state with different density. Long or routine
tool activity collapses; reviewable or blocking state expands. Branching,
undo/redo, revert, fork, and “start a new session from this answer” treat
conversation history as operable structure rather than a flat log. `[source]`

That remains the right OpenAgents presentation model: one causal timeline
derived from typed events, with raw provider history private and
loss-accounted, not an append-only text box whose strings become state.

### 4.3 Right operational panel

Git status, staged and unstaged changes, diffs, files, history, context, review
comments, plans, and other run-adjacent facts stay beside the transcript. The
panel is a second projection of the same work, not a disconnected settings
page. `[source]`

The key harvest is coupling, not pixels. OpenAgents should project the same
stable turn/tool/file/worktree refs into both timeline and inspection panel so
opening a tool event selects the exact affected object and review state does
not drift from conversation state.

### 4.4 Composer and docks

The composer carries model/provider/agent/variant, plan/build mode, attachments,
slash and shell commands, dictation, goal arming, send/stop, and pending
blockers. OpenChamber's Session Goal target deliberately starts as an armed
composer mode: the submitted message becomes the objective. `[source]`

OpenAgents should preserve one canonical command and composer model. Voice,
keyboard, pointer, deep link, native menu, and mobile affordances should invoke
the same typed intents rather than each owning an action path.

## 5. Session, worktree, and runtime model

OpenChamber makes sessions first-class operational resources:

- create, rename, archive, delete, share, fork, undo, redo, and revert;
- bind a session to its actual directory rather than the currently selected
  directory;
- create isolated Git worktrees and start sessions from issues, pull requests,
  plans, or prior answers;
- preserve provider/model/agent/variant selection;
- show live status and attention across initialized directories;
- open the same session in web, Desktop, mobile, VS Code, or a remote host.

The UI distinguishes directory-scoped live stores from a global session cache.
The global cache supplies cold/archived coverage; initialized child stores
supply live status. Mutation actions update both the server and visible global
cache. `[source]`

This division solves a real UI problem but creates two client-side truths that
must be carefully reconciled. OpenAgents should preserve the user experience
while giving Khala Sync and owning services clearer authority:

- server-confirmed canonical catalog for cross-device session identity;
- device-local catalog for signed-out local work;
- live subscription as current activity, not historical inference;
- explicit unavailable/stale/refetch states rather than silently empty data;
- host-private path binding outside shared projections;
- one generation-fenced attachment when work moves.

OpenChamber's remote model reconnects clients to an existing host. OpenAgents'
remote-first model additionally needs placement, checkpoint, detach, attach,
move, capability reauthorization, and receipts. The former is strong product
evidence for demand; it is not proof of the latter.

## 6. Event pipeline, state, and rendering performance

This is one of OpenChamber's most valuable implementation studies.

The shared UI consumes OpenCode and OpenChamber events through a pipeline that
supports WebSocket, SSE, or automatic fallback. It tracks the last event id,
heartbeats, backpressure, connection failure, auth-token refresh, visibility,
browser online state, permanent versus retryable status, and interruptible
exponential reconnect. Per-directory queues coalesce high-frequency events and
flush at frame-oriented intervals. `[source][test]`

The reducer owns state-dependent validity. Transport code does not guess
whether a delta is redundant. Event handlers clone only the fields an event
will mutate; selectors read leaf values rather than whole maps; high-frequency
viewport/stream state is split from low-frequency selection/input state.
OpenChamber's module documentation reports that eliminating eager cloning cut
MessageList renders from roughly 1,972 to 296 in its measured session.
`[source]`

The reusable lessons are:

1. Keep transport, event validity, projection, and presentation separate.
2. Preserve event identity/cursor and make replay/gaps explicit.
3. Coalesce only event classes whose semantics permit replacement.
4. Update only the affected projection field.
5. Split stores/program state by change frequency and subscriber set.
6. Distinguish fetch failure from a successful empty response.
7. Test reconnect, resume, permanent errors, backpressure, materialization,
   freshness, session switching, and scoped blockers.

OpenAgents should express those rules in Effect programs and schema-decoded
reducers rather than copying Zustand. Khala Sync's dense versions, cursor
repair, full-post-image entities, and Runtime Gateway subscription are the
stronger authority boundary. The OpenChamber measurements remain useful input
for renderer allocation and update tests.

## 7. OpenCode and host-capability seam

Official OpenCode traffic uses the generated SDK. OpenChamber-specific
capabilities use `RuntimeAPIs`, `runtimeFetch`, runtime URL/auth helpers, and
realtime helpers. The server owns OpenCode start/restart/readiness, provider
auth file access, settings/config layers, skills, snippets, filesystem, Git,
GitHub, terminal, tunnels, relay, notifications, schedules, goals, small-model
calls, dictation, and TTS. `[source]`

This separation is directionally right: the renderer does not spawn a shell or
read the filesystem directly. But the browser-facing server surface is broad,
and OpenChamber is comfortable treating the OpenCode protocol and local host as
the product's runtime authority.

OpenAgents should keep the seam narrower:

- the Runtime Gateway is the only app-service seam;
- provider-native events normalize into registered OpenAgents contracts;
- files/Git/PTY remain host-owned capability services;
- credentials remain in main/OS or server custody;
- commands carry stable identity, owner scope, generation, idempotency, policy,
  deadline, and outcome;
- Sync distributes projections and durable outcomes but does not execute;
- Pylon and managed Cloud are composed through the same placement/workroom
  contract rather than cloned inside Desktop.

## 8. Files, Git, terminal, GitHub, and review

OpenChamber's server provides a broad developer workbench:

- workspace-bounded read, raw, serve, write, mkdir, delete, rename, reveal,
  list, search, and background command jobs;
- Git status, staged/unstaged diff, hunk stage/unstage/discard, branches,
  remotes, worktrees, commit, fetch/pull/push, history, stash, merge/rebase, and
  conflict inspection;
- GitHub authentication plus issue/PR context and PR/check/merge workflows;
- full-duplex PTY over a versioned WebSocket control protocol with a bounded
  in-memory startup replay buffer and HTTP/SSE fallback;
- project actions, preview/dev-server workflows, open-in-editor/finder/terminal,
  and inline comments on files/diffs/plans.

`[source]`

The UI integration is the harvest: agent work is immediately inspectable and
operable without leaving the workroom. The authority model must be adapted.
OpenAgents renderer code must never receive a general filesystem or process
handle. Every mutation needs a host-owned, grant-bounded capability, typed
post-image/outcome, and conflict behavior. PTY replay is a convenience, not a
durable terminal transcript or execution receipt.

## 9. Session Goals: useful autonomy, incomplete durability

Session Goals are the most strategically relevant v1.16 addition.

### 9.1 What is implemented

A user arms goal mode and sends a self-contained objective. OpenChamber stores
goal metadata on the OpenCode session and stores oversized objective text under
the OpenChamber data directory. When the session becomes idle and remains
quiet, a server runtime asks an independently selected small model to classify
the latest assistant turn as `continue`, `complete`, or `blocked`. It can then
send another prompt asynchronously using the prior provider/model/agent/variant.
`[source]`

The goal record includes:

- opaque goal id and objective reference;
- `active`, `paused`, `blocked`, `budgetLimited`, or `complete` status;
- token budget and segmented usage/accounting;
- automatic-turn count;
- blocked and audit-failure streaks;
- progress note, status reason, cursor, and timestamps.

Every mutation re-reads the session and checks the goal id, preventing a late
write from an older logical goal. Before submitting a continuation, the runtime
persists accounting and increments the turn count, then re-fetches the session
tail to avoid sending after the user or session moved. User abort/pause wins.
Hard stops include error, optional budget, 20 continuations, three blocked
audits, and two audit failures. Goal-active sessions suppress routine per-turn
ready notifications and emit a single settled notification. `[source]`

Those are strong patterns. In particular:

- goal state is server-owned, not component-owned;
- explicit stop dominates autonomy;
- stale logical-goal writes are fenced;
- accounting advances before the side effect;
- user activity is rechecked immediately before continuation;
- terminal states and limits are visible;
- normal attention noise is suppressed while blockers still surface.

### 9.2 What is not implemented

The public docs correctly say the **server must remain running**. The stronger
release-language implication that a goal simply survives server restarts is
not established by the tagged runtime. `[public][source][limitation]`

The runtime says it is purely event-driven: no polling, no backfill, no session
scan. It sees only sessions that emit events while that server process is
running. Its quiet timers and in-flight set are memory-only. Therefore:

- an active goal whose session is already idle at server restart may never be
  re-armed until another relevant event happens;
- a crash after the persisted turn increment but before `prompt_async` biases
  away from duplication, but can leave a continuation permanently missing;
- there is no durable continuation row, lease, claim generation, attempt state,
  outbox, startup reconciliation, or reaper;
- only one goal exists per session and the loop skips subagent sessions;
- the auditor sees the objective and latest assistant response, not typed
  evidence of repository, test, deployment, or external-system state;
- the tagged tree contains no Session Goal tests.

The feature is a good **next-turn autonomy loop**, not exact interrupted-turn
recovery and not yet a durable workflow engine.

### 9.3 OpenAgents adaptation

OpenAgents should split three concerns:

1. **Interrupted provider turn recovery** — exact accepted message/turn,
   provider continuation or safe restart, generation fence, cursor, and one
   terminal outcome. This is the scope of
   [#8744](https://github.com/OpenAgentsInc/openagents/issues/8744).
2. **Session objective** — durable objective, requirements ledger, budget,
   owner policy, completion predicates, evidence refs, and explicit
   pause/resume/cancel.
3. **Continuation dispatch** — durable due continuation with idempotency key,
   lease generation, attempt, startup scan, reconcile-before-send, and
   reconcile-after-timeout.

The auditor can recommend progress, but model prose must not be sole acceptance
authority. Deterministic requirements and typed evidence—Git post-image,
tests, issue/PR state, deployment receipts, runtime health, approval state—must
remain superior. A goal that needs an operator decision becomes a durable
runtime interaction, not a permission bypass.

## 10. Scheduled tasks: good product integration, weak execution durability

Scheduled tasks persist definitions and can create a fresh session at a daily,
weekly, once, or cron schedule, with timezone, provider/model/agent selection,
slash-command prompts, immediate run, concurrency limits, per-task dedupe,
last-run state, created-session link, and optional Session Goal stamping.
`[source][test]`

The product integration is excellent: a recurring prompt appears as a normal
session that the user can inspect, continue, and receive notifications about.
The implementation remains a process scheduler:

- timers, queue, running keys, and concurrency counters are memory-only;
- startup recomputes future times from “now,” skipping downtime occurrences;
- an overdue once task can become non-runnable;
- watchdog timeout does not necessarily abort the underlying create/prompt
  side effect;
- there is no durable run id/lease, due-slot idempotency key, heartbeat, reaper,
  or transactional next-run advancement;
- the docs explicitly say tasks fire only while the server is running.

OpenAgents should harvest the normal-session projection and scheduling UX, then
run it on durable task/run rows, due-slot keys, claims, heartbeats, missed-run
policy, bounded catch-up, cancellation/join, and outcome receipts. A platform
scheduler should wake the dispatcher, not become run authority.

## 11. Permissions, questions, and unattended blockers

OpenChamber's permission auto-accept runtime is a better architectural reference
than the Session Goal loop itself. Policy lives on the server, persists per
session, inherits from the nearest explicit ancestor, lets an explicit child
`false` override a parent `true`, fails closed for unknown lineage, deduplicates
requests, retries transient failures, and reconciles pending requests on
startup, reconnect, and enablement. `[source][test]`

The key lesson is that unattended sessions cannot depend on a mounted dialog
to resolve pending authority. Blockers require durable server ownership and
reconciliation.

The blanket boolean policy is too broad for OpenAgents. Auto-accept must never
follow merely from “goal active.” OpenAgents needs capability/risk classes,
exact owner/device/session/turn/run/generation, expiry/revocation, ask/deny/allow
precedence, non-serializable owner-local danger authority where explicitly
permitted, and a receipt for the effective decision. Questions, tool approvals,
and plan reviews already fit the private `runtime_interaction` boundary; goals
should wait on or settle blocked from that authority instead of bypassing it.

## 12. Voice: strong dictation mechanics, not persistent two-way voice

OpenChamber publicly calls the feature Voice Mode. In v1.16 it is principally:

- composer dictation via browser recognition, an OpenAI-compatible server, or
  local/on-device models;
- message-level text-to-speech via browser voices, OpenAI-compatible service,
  local/macOS paths, and local model support;
- play/read-aloud controls and a browser conversation experience.

It is not a durable, always-open, full-duplex assistant audio session.
`[source][public]`

The dictation transport has valuable mechanics: 16 kHz mono PCM16, ordered
sequence numbers, highest-contiguous ACK, client retention of unacknowledged
chunks, out-of-order reassembly, explicit `finish(finalSeq)`, silence
suppression, live partials, adaptive finalization, typed retryable errors, and
failed audio retained client-side long enough to retry or accept a partial.
The composer exposes timer, volume, partial transcript, cancel, insert,
insert-and-send, retry, and accept-partial. `[source][test]`

Those patterns directly inform the separately tracked
[OpenAgents audio program](../voice/2026-07-12-persistent-desktop-voice-mode-audit-and-plan.md):

- preserve sequence/ACK/replay and explicit finalization;
- never replace a longer finalized utterance with a late snippet;
- retain failed audio under explicit visible custody for bounded retry;
- separate capture, egress, retention, transcription, command proposal,
  acceptance, execution, playback, and outcome;
- support barge-in with playback generations and cancellation dominance;
- use Google Cloud STT/TTS and the accepted Effect/Rust split rather than
  copying OpenChamber's provider/key paths;
- keep raw media out of Sync and remove raw transcript logging.

OpenChamber's server stream manager is connection-local and does not provide a
durable resume token across process loss. Its client may send an
OpenAI-compatible key in dictation options, and browser voice code logs
transcript text. Its TTS docs do not establish bounded request timeout. These
are explicit non-harvest boundaries for OpenAgents.

## 13. Remote instances, pairing, relay, and notifications

### 13.1 Direct, SSH, tunnel, and host switching

Desktop can manage local and remote OpenChamber instances, import a deep-link
connection, use SSH and port forwarding, and switch native windows between
hosts. CLI/headless hosts can advertise connection URLs or QR codes. This
makes runtime placement visible and lets a single Desktop supervise several
machines. `[source][public]`

OpenAgents should harvest the explicit target/host model but use stable
provider-neutral coding session and target refs. A saved URL, port, process,
filesystem path, or SSH host is a connection fact, not canonical session
identity.

### 13.2 Private relay

The private relay is one of OpenChamber's best isolated designs. A host makes
an outbound connection to a broker. Client and host establish ECDH/AEAD E2EE,
multiplex HTTP/SSE/WebSocket traffic, and treat the relay as an opaque courier.
The paired client still presents its normal server credential; the relay grants
reachability, not authorization. HTTP and WebSocket path allowlists remain
explicit. Pairing secrets are one-time, relay is opt-in, devices are revocable,
and a stable server identity pins refreshed LAN candidates before a bearer is
sent. `[source][test]`

A cooperative host lock prevents several local processes sharing a data
directory from fighting for the same relay identity. Direct candidates are
preferred and refreshed so a device can leave relay mode when LAN reachability
returns. `[source][test]`

Harvest the trust model and transparent transport interface. Do not copy the
manually mirrored TypeScript client and JavaScript host crypto/framing
implementations; OpenAgents should use one normative schema and golden corpus
across Effect and Rust decoders. The external relay Worker is not in the tagged
repo, so broker retention, deployment, and operational claims remain
unverified. Raw audio needs its own bounded media policy even if transported
through the same reachability layer.

### 13.3 Notifications

OpenChamber combines desktop notifications, web push, APNs/FCM, visibility and
presence suppression, generic privacy-preserving payloads, and deep links back
to the exact session. Goal mode suppresses intermediate ready noise but keeps
errors, permissions, and questions visible. `[source][test]`

OpenAgents should harvest the attention semantics: notify on meaningful state
transition, carry stable refs rather than private content, re-authorize at
open, suppress only when a current client is observably present, and never let
a notification payload confer authority.

## 14. Security, privacy, and trust assessment

### Strong choices

- Server-side provider credentials for small-model calls; UI receives text and
  model identity, not the stored provider secret.
- Hashed, revocable client bearer tokens and one-time pairing redemption.
- Password/passkey/pairing as issuance methods for one durable remote-client
  credential model.
- Relay transport does not grant application authorization.
- Explicit path allowlists and workspace checks for host operations.
- Origin validation for Electron privileged commands and navigation.
- Context isolation and disabled renderer Node integration.
- Generic notification payloads and session deep-link re-entry.
- Tests for relay cross-compatibility, auth, permission reconciliation,
  transport failure, and high-frequency sync behavior.

### Residual risks and architectural debt

- Electron renderer sandbox is disabled and `webviewTag` enabled.
- Preload exposes a generic command bridge to all pages, then relies on a main
  allowlist; local renderer globals include runtime credentials.
- A remote web UI can be privileged through a broad local server surface when
  correctly authenticated; exposure safety depends on UI auth, client tokens,
  route policy, and deployment configuration.
- Some OpenCode provider auth behavior reads and rewrites upstream auth files,
  increasing coupling and credential blast radius.
- Voice configuration can broaden credential exposure to client settings.
- The relay service and push broker implementations are outside the audited
  repository.
- Cooperative PID locking is lifecycle coordination, not a security boundary.
- Goal acceptance relies heavily on model self-report plus a small-model
  judgment of the latest answer.
- Scheduler and goal-loop persistence claims exceed their crash recovery.

OpenAgents' tokenless renderer, OS credential custody, closed preload schemas,
capability-shaped host grants, structural redaction, typed policy, and receipt
authority are stronger and should not be relaxed for feature parity.

## 15. Packaging, updates, compatibility, and verification

OpenChamber packages:

- Electron DMG/ZIP for macOS with hardened runtime, entitlements, and notarize
  configuration;
- NSIS on Windows;
- npm-distributed web/CLI server;
- VS Code extension package;
- Capacitor iOS/Android projects;
- PWA assets and service worker.

The Desktop bundles a matching OpenCode CLI and has a self-update path. The web
server can update and restart itself while retaining settings. `[source]`

The repository has substantial unit tests around event reduction, reconnect,
permission auto-accept, schedules, dictation stream assembly, relay crypto and
cross-compatibility, notifications, terminal framing, Electron runtime headers,
SSH, Git, and other modules. It also has release build scripts. `[test]`

Important proof gaps in this snapshot:

- no tagged Session Goal tests;
- no source proof of the external relay/push services;
- no live signed/notarized/install/update/rollback receipt was audited;
- no proof that Desktop, mobile, web, and VS Code all preserve every contract;
- no durable crash/restart oracle for goal/schedule side effects;
- no evidence that a Capacitor mobile release has the same native reliability
  and accessibility as a purpose-built Effect Native surface.

OpenAgents should continue treating distribution as a signed compatible
component set: client, Runtime Gateway protocol, provider adapter, schema,
database migration, runtime helper, and server capability generation. A build
configuration is not a release receipt.

## 16. What OpenChamber gets exceptionally right

1. **It ships a workroom, not a chat mockup.** Conversation, operation, review,
   and continuation are one product.
2. **It makes repeated work cheap.** Dense sessions, attention, worktrees,
   branchable history, and multiple windows reduce reorientation.
3. **It places runtime truth near the user.** Tools, files, diffs, blockers,
   terminals, plans, context, and cost/activity are inspectable.
4. **It shares one interaction model across surfaces.** Web, Desktop, mobile,
   and VS Code differ in host capability without becoming unrelated products.
5. **It treats remote access as product infrastructure.** Direct, SSH, tunnel,
   QR/deep link, relay, pairing, and notifications form a continuity story.
6. **It learns from streaming pressure.** Event coalescing, touched-field
   cloning, store splitting, and reconnect tests are concrete engineering, not
   generic performance advice.
7. **It moves blockers and autonomy server-side.** Even where durability is
   incomplete, the ownership direction is correct.
8. **It exposes limitations in its own docs.** Scheduled tasks and goals name
   the requirement that the server stay running.

## 17. Where the product is structurally fragile

1. **OpenCode and OpenChamber authority overlap.** Sessions, metadata, process
   lifecycle, host capabilities, and synthetic OpenChamber features span two
   systems without one provider-neutral durable control contract.
2. **Several client/server truths coexist.** Directory stores, global caches,
   live aggregates, OpenCode state, OpenChamber metadata, and UI state require
   careful repair.
3. **Cross-runtime parity grows by mirroring.** Web server, Electron, VS Code,
   mobile wrapper, TS/JS relay code, and native platform behavior can drift.
4. **Persistence is sometimes mistaken for recovery.** A saved goal or task
   definition does not durably own its next execution.
5. **The Desktop trust boundary is pragmatic, not minimal.** Unsandboxed
   preload, webview support, renderer-visible tokens, and a generic invoke
   channel create avoidable blast radius.
6. **Autonomy evidence is weak.** Latest-response audit cannot prove external
   outcomes and risks rewarding a confident completion summary.
7. **Voice naming outruns the media model.** Dictation and read-aloud are useful
   but do not yet form a persistent full-duplex, resumable, retained-audio
   session with typed actions and barge-in.

## 18. Delta from the 2026-06-03 OpenAgents report

The historical
[OpenChamber UI/UX Port Research](../../apps/openagents.com/docs/2026-06-03-openchamber-ui-ux-port-research.md)
remains accurate about:

- the three-pane workroom;
- turn and tool projection;
- blocker docks and attention;
- chat/review coupling;
- session hierarchy;
- event reduction and touched-field performance;
- mobile state rather than desktop-layout copying;
- rejecting React/Zustand and browser-owned host authority.

This teardown supersedes it for current architecture and sequencing. The June
document's large “Implementation Update” is a historical record of an older
web/Autopilot surface, not current OpenAgents Desktop/mobile authority. Since
then OpenChamber added or matured:

- Electron as the primary Desktop release target;
- bundled OpenCode and multi-host Desktop operation;
- Capacitor mobile applications and native push/deep-link support;
- private outbound E2EE relay and unified pairing;
- server-owned permission auto-accept reconciliation;
- scheduled tasks;
- Session Goals and small-model auditing;
- richer voice/dictation/TTS paths;
- significantly more explicit sync/reconnect/performance guidance.

The new evidence strengthens the original interaction recommendation but also
raises the bar. OpenAgents no longer needs merely an OpenChamber-shaped web UI;
it needs a hardened, durable, provider-neutral workroom contract shared by
Desktop and mobile.

## 19. OpenAgents product considerations

OpenAgents' product register is an operational product, not a landing page.
OpenChamber validates dense, direct, skeptical-user-facing software: show the
route, make state inspectable, and put review and control next to the work. Its
exact visual system should not be copied. OpenAgents should express the same
information architecture through its tinted near-black surfaces, restrained
blue energy, Berkeley Mono, shared iconography, visible focus, and reduced-
motion rules.

### 19.1 Harvest now

| OpenChamber evidence | OpenAgents adaptation | Owning lane |
| --- | --- | --- |
| Dense project/worktree/session rail | Canonical CUT-13 identities, recent-first session directory, typed attention and status, clean user-facing titles | D1/D2, R1/R2 |
| Turn/tool/blocker timeline plus right review panel | One event-derived timeline and inspection projection using stable refs; full child topology and independent transcripts | D1/D3, CUT-11–14 |
| Touched-field reducer and stream coalescing | Effect reducer allocation discipline, schema-valid replay, cursor gaps, backpressure and renderer update tests | D1, R2/R4 |
| One composer with model/mode/voice/goal affordances | Canonical composer state and command registry; pointer/keyboard/voice/mobile invoke the same intents | D2, AUDIO, R6 |
| Pending permission reconciliation | Server-owned `runtime_interaction` recovery and attention independent of mounted UI | D4/D5, R3/R4 |
| Goal id stale-write fence and cancellation dominance | Goal generation/CAS and owner stop dominance | #8744 plus future bounded goals contract |
| Dictation sequence/ACK and failed-audio UX | Persistent audio contract, Google STT, bounded retention/retry, transcript replacement fence | #8733–#8741 |
| Relay reachability without authority | Transport-neutral Runtime Gateway/Sync subscriptions, E2EE candidate, stable host identity, bearer reauthorization | R4/R6 |
| Generic push with exact session ref | Private stable-ref notification, revalidation at open, presence-aware suppression | R6/R7 |
| Schedule/goal runs become normal sessions | Autonomous work projects into the same session/run/interaction/outcome system | R3/R4 |

### 19.2 Adapt with stronger boundaries

1. **Session Goals.** Add startup enumeration, durable continuation records,
   leases, due/outbox state, idempotency, generation fencing, deterministic
   evidence predicates, and fault tests. Do not claim restart recovery from
   metadata persistence.
2. **Scheduled tasks.** Use durable task/run authority and missed-run policy;
   retain the inspectable normal-session UX.
3. **Permissions.** Keep server reconciliation but compile scoped authority;
   reject blanket session booleans for high-risk operations.
4. **Remote work.** Keep transparent host access but add host-independent
   identity, target descriptors, checkpoint/attachment generations, capability
   reauthorization, and receipts.
5. **Host capabilities.** Keep files/Git/PTY behind a host, but admit them only
   through typed grants and post-images.
6. **Voice.** Keep dictation transport behavior but preserve OpenAgents'
   persistent stream generation, consent/retention, Google primitives, Rust
   helper boundary, typed command proposal, playback generation, and raw-media
   exclusion.
7. **Small-model utility.** A cheap auditor/summarizer may assist projection;
   it never owns acceptance, spending, deployment, permission, or public truth.
8. **Shared clients.** Share domain contracts and programs through Effect
   Native, not a WebView application or duplicated bridge implementations.

### 19.3 Reject explicitly

- copying React components, Zustand stores, Express modules, or Tailwind theme;
- treating raw OpenCode protocol as the OpenAgents client contract;
- renderer-visible client/provider/runtime credentials;
- `sandbox: false`, `webviewTag: true`, or a generic renderer command bridge as
  the OpenAgents Desktop baseline;
- broad browser-accessible local filesystem, Git, terminal, or process APIs;
- blanket permission auto-accept inherited merely from goal/session state;
- model verdict or assistant summary as outcome authority;
- process timers as durable autonomy or schedule proof;
- two manually mirrored implementations of a security protocol;
- a second session/run/workflow database inside Desktop;
- Capacitor/WebView as the OpenAgents mobile architecture;
- voice transcript, audio, playback, or model speech as command/outcome proof;
- OpenChamber's exact color, density, icon, or motion implementation.

## 20. Ordered consequences for OpenAgents

1. **Keep the current architecture sentence.** Tokenless local Effect Native
   client → host-owned Runtime Gateway → owning runtime/Khala Sync/Pylon/
   workroom services through one typed query/command/event contract.
2. **Finish the canonical workroom before broad settings parity.** Reliable
   session titles, turns, tools, blockers, agent graph, files/diffs, status,
   composer, and review have higher product value than copying a large settings
   surface.
3. **Use OpenChamber as the reducer/reconnect fault corpus.** Port the event
   pressure cases into Effect fixtures: delta storms, duplicate/full-part
   races, disconnect, hidden/offline backoff, permanent auth error, cursor gap,
   session switch, stale cache, and blocker reconciliation.
4. **Close exact turn recovery separately.** #8744 must recover or honestly
   settle an accepted interrupted turn before a goal loop may build on it.
5. **Specify durable goals as a new authority.** Register goal, requirement,
   continuation, audit, evidence, lease, and terminal-outcome schemas; add a
   startup scanner and crash-window model before implementation claims.
6. **Reuse the existing interaction and policy system.** Goals wait for typed
   permission/question/plan decisions; they do not mint authority.
7. **Keep the audio program parallel.** Apply OpenChamber's ACK/retry and
   dictation UX lessons to #8733–#8741 without changing the accepted Google
   STT/TTS, retention, or Effect/Rust decisions.
8. **Make mobile continuation an acceptance surface.** Push, deep links,
   attention, session refs, voice, and runtime interactions must revalidate
   through the same authority and work with Desktop closed when the owning
   server/runtime remains alive.
9. **Prove service survival honestly.** Distinguish window hidden, renderer
   reload, Electron relaunch, host process restart, machine reboot, network
   loss, and provider interruption. Each gets a named acceptance receipt.
10. **Promote lessons through owners.** This teardown is evidence. Binding
    requirements belong in schemas, invariants, the Sol roadmap, issues, tests,
    fault models, and release receipts.

## Final assessment

OpenChamber is worth studying as a whole because its value comes from the way
the pieces reinforce one another. Session navigation makes autonomous work
findable. Typed tool rendering makes it inspectable. Git/files/terminal make it
operable. Blockers and notifications make it supervisable. Mobile, pairing,
and relay make it reachable. Voice makes the composer more ambient. Goals and
schedules turn one session into a standing unit of work.

That system-level coherence is the harvest.

OpenAgents should implement the stronger version: the same persistent
workroom and cross-surface immediacy, but with provider-neutral identity,
Effect Schema contracts, a hardened tokenless renderer, durable Sync and
outcomes, scoped capability authority, startup reconciliation, generation-
fenced autonomy, evidence-backed acceptance, full-duplex audio contracts, and
signed compatible releases. OpenChamber proves the product shape. Its restart
and authority gaps show exactly where OpenAgents must be more rigorous.

## Primary source map

All links below are commit-pinned to the audited snapshot:

- [Repository guidance and architecture](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/AGENTS.md)
- [Public product README](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/README.md)
- [Root package and dependency graph](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/package.json)
- [Electron main process](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/electron/main.mjs)
- [Electron preload bridge](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/electron/preload.mjs)
- [UI sync architecture](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/ui/src/sync/DOCUMENTATION.md)
- [Event pipeline](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/ui/src/sync/event-pipeline.ts)
- [Event reducer](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/ui/src/sync/event-reducer.ts)
- [OpenCode server integration](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/opencode/DOCUMENTATION.md)
- [Session Goal runtime](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/session-goal/runtime.js)
- [Session Goal internals](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/session-goal/DOCUMENTATION.md)
- [Session Goals product docs](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/docs/content/docs/session-goals.mdx)
- [Scheduled task runtime](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/scheduled-tasks/runtime.js)
- [Permission auto-accept runtime](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/permission-auto-accept/runtime.js)
- [Permission auto-accept tests](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/permission-auto-accept/runtime.test.js)
- [Dictation server architecture](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/dictation/DOCUMENTATION.md)
- [Dictation stream manager](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/dictation/stream-manager.js)
- [Composer dictation UI](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/ui/src/components/dictation/ComposerDictation.tsx)
- [Private relay architecture](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/relay/DOCUMENTATION.md)
- [Relay cross-compatibility test](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/relay/cross-compat.test.js)
- [UI authentication architecture](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/ui-auth/DOCUMENTATION.md)
- [Notification server architecture](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/web/server/lib/notifications/DOCUMENTATION.md)
- [VS Code bridge modules](https://github.com/openchamber/openchamber/blob/e1e5bf61fe4fc435332eee9b3ee6601b6dbbecb5/packages/vscode/src/DOCUMENTATION.md)

# What OpenAgents Should Incorporate: A Full-Catalog Synthesis

Date: 2026-07-17

This essay synthesizes the complete teardown catalog in this directory — every
dated teardown, rendering analysis, and gap analysis from the ChatGPT/Codex
desktop app through the 2026-07-17 T3 Code mobile documents — into one
consolidated statement of what OpenAgents should incorporate into its three
product surfaces: **OpenAgents Desktop**, **OpenAgents mobile**, and
**openagents.com**. It deliberately does not ground itself in the current
implementation state of the codebase; it asks what the accumulated competitive
evidence says the products should become. Per the README convention, this is
design evidence, not implementation authority: the Sol roadmap, typed
contracts, issues, tests, and receipts remain the authorities for what is
actually built and in what order.

Sources synthesized: ChatGPT desktop, Claude desktop, Claude Code, Codex agent
runtime, Codex and Claude subagent rendering analyses, the OpenAgents subagent
design note, OpenCode (desktop, V2 engine, Effect architecture, Electron
build/update), Cursor, Executor, OpenChamber, Crabbox, T3 Code (whole product,
mobile app, ACP implementation, Electron build/update, desktop full gap,
desktop UI gap, mobile controller gap), Codex app-server client support, Grok
Build, Command Code, Factory Desktop/Droid, Amp Code, and the prior
cross-teardown adaptation analysis.

---

## 1. The market has converged on a shape; the trust half is unclaimed

Read together, twenty-eight documents describe one striking fact: every
serious product in this space — OpenAI's Codex desktop, Anthropic's Claude
desktop and Claude Code, Cursor, Factory, Amp, Grok Build, OpenCode, T3 Code,
OpenChamber — has independently converged on the same architecture:

1. A real agent **engine outside the renderer** — a Rust app-server (Codex), a
   stream-JSON CLI sidecar (Claude), an Effect server in a utility process
   (OpenCode), an authenticated local daemon (Factory), a shared leader
   process (Grok Build), a cloud thread actor (Amp).
2. A **versioned query/command/event seam** between engine and every client —
   generated JSON-RPC schemas, typed SDK streams, HTTP+SSE+WebSocket
   transports.
3. **Worktree-isolated parallel agents** as a consumer-visible default
   (Cursor 2.0, T3 Code, OpenCode, Grok Build, Factory Missions).
4. **Desktop/mobile continuity and remote steering** (Cursor's `&` handoff and
   iOS Remote Control, Amp's web/mobile thread control, T3's full mobile
   workbench, OpenChamber's relay, Codex's drive-my-Mac pairing).
5. A workbench that **grows beyond chat without becoming an IDE** — diff
   review, terminals, file trees, previews as projections over engine state.

That convergence is validation: the one-engine/many-clients thesis is no
longer a bet, it is table stakes. But the same documents record, with unusual
consistency, what *none* of these products ship:

- **Authority manifests** — a typed record of what policy admitted before a
  run.
- **Execution receipts** — what containment actually enforced, as distinct
  from what the permission UI implied.
- **Delivery receipts** — `completed` distinguished from
  `committed/pushed/reviewed/merged/accepted`.
- **Host-portable sessions** — session identity that survives movement between
  machines without forking authority or history. Cursor's handoff is
  their-cloud-only; T3's threads are environment-local; Amp's are
  cloud-canonical.
- **Release provenance** — Amp ships with its minisign verification commented
  out and a binary that fails Gatekeeper; Grok Build's updater is unsigned;
  Cursor keeps a plain-HTTP backup update URL; Factory publishes checksums
  with no signature pin; T3's audited DMG shipped Gatekeeper-dead; Command
  Code self-updates over npm unsigned.
- **Usage, model, and data-flow truth** — Cursor had two pricing crises and a
  concealed base-model swap (Composer 2 post-trained on Kimi K2.5, outed
  forensically); Amp hides model identity behind mode names; Factory's
  privacy pages contradict each other; Command Code posts an undisclosed
  hardware fingerprint and calls a hosted inference loop "local."
- **Economic participation** — revenue sharing, idle-compute markets, open
  settlement. Never attempted by anyone (the Cursor teardown's episode-195
  scorecard makes this explicit).

Every teardown converges on the same conclusion, stated most sharply in the T3
analyses: the supervision half of the product is becoming commoditized, and
**OpenAgents wins on the trust half or not at all**. Everything below is
organized around that: incorporate the convergent supervision shape wholesale,
and build the trust layer none of them have as the differentiation.

---

## 2. The engine contract every surface consumes

Before any surface-specific work, the catalog is unanimous that the engine
seam must be frozen as a typed, generated, versioned contract. This is the
single highest-leverage incorporation because every other recommendation
depends on it.

**Incorporate:**

- **One protocol hierarchy: Thread → Turn → Item, extended with Work Unit and
  Receipt.** Codex proves the base vocabulary at production scale (126 client
  requests, 11 server-initiated requests, 72 notifications at the audited
  revision, all generated from source with drift-tested fixtures). OpenAgents
  extends it with what Codex lacks: a Work Unit that carries task identity and
  a delivery lifecycle (`changes_produced → reviewed → committed → merged →
  pushed → accepted`), an Authority Manifest, an Execution Receipt, and a
  Delivery Receipt.
- **Generated clients from one Effect Schema source** for TypeScript, and for
  any future Swift/Kotlin/Rust surface — plus JSON Schema fixtures and drift
  tests. Stable and experimental API bundles gated separately; capability
  advertisement tied to actual handler coverage (the Codex app-server analysis
  shows T3 advertising `experimentalApi: true` while handling 3 of 10 reverse
  requests, which can deadlock turns — the named anti-pattern).
- **Durable admission before execution.** OpenCode V2 is the strongest
  reference: every input is durably recorded (client-chosen idempotent ID,
  causal parent, typed delivery intent) *before* any scheduling; promotion to
  the model happens atomically at a safe boundary; exact retry reconciles,
  conflicting reuse fails. Pending input is never model-visible. This is what
  makes mobile follow-ups, offline outboxes, and crash recovery honest.
- **Steer vs queue vs interrupt as three explicit verbs with different
  owners.** Amp ships the cleanest gestures (Enter queues, double-Enter
  steers at the next safe point, double-Escape interrupts); Codex ships the
  correct wire discipline (`turn/steer` with an expected-turn-ID
  compare-and-set; a queue promoted only after quiescence). T3's bug — a
  keyboard path issuing a second `turn/start` mid-turn that Codex silently
  converts to steering — is the named failure of leaving this implicit.
- **Three read surfaces, named and contractual:** bounded current
  projections; a durable per-aggregate replayable log with a replay-to-live
  sync marker; and a volatile live event stream documented as lossy.
  Reconnect is **repair, not replay** — where a gap cannot be reconstructed,
  the thread carries an honest `transient_gap` marker rather than fabricated
  completions.
- **Backpressure as protocol contract:** bounded queues at ingress,
  processing, and egress, with a retryable overload error whose
  backoff-and-jitter is specified in the contract, not left to clients. Grok
  Build's unbounded hot queues are the counterexample.
- **One request processor across all transports.** OpenCode's embedded SDK
  installs a memory-backed fetch into the same router — in-process callers
  never bypass routing, middleware, or policy. Desktop IPC, local socket,
  remote Pylon, mobile Sync, web, and tests differ only in transport and
  credential acquisition.
- **Interruption as control flow.** User decline is an interruption, never a
  fabricated tool failure; tool fibers are owned in sets; uninterruptible
  masks exist only around state settlement.
- **Engine lifecycle as product state.** Grok Build's shared leader and
  Factory's daemon prove the shape: a long-lived authenticated local
  supervisor with a machine-readable lifecycle record (binary and protocol
  versions, process generation, socket identity, readiness, client
  capabilities, update/relaunch state), idempotent start, bounded drain on
  update, typed exit reasons, per-generation client authentication — never
  ambient localhost trust or a shared Basic password as client identity.

**Refuse:** two query owners (Claude Code's duplicated loop), two protocol
generations without deletion gates (OpenCode Desktop still embedding V1 while
V2 exists), 92-flag compatibility accretion (Codex), whole-transcript rewrite
as durability (Command Code), a durable "running" status that a crash makes
into a lie (OpenCode V2 explicitly refuses to persist one).

---

## 3. Authority, containment, and receipts — the differentiating layer

The catalog's most repeated finding is that **permission UX is not
containment**, and every competitor conflates them somewhere: Grok Build ships
sandbox-off-by-default with warn-and-continue degradation and a no-op macOS
network restriction; Factory's sandbox is an opt-in beta that leaves the main
process outside; Amp runs tools without approval by default and shows no OS
sandbox at all; T3 defaults to `danger-full-access` with approvals off while
shipping excellent DPoP access crypto — the "authority inversion"; Command
Code's configured deny rules are literally never evaluated, and its autonomous
goal verifier fails open ("verifier unavailable — accepted self-claim").
Claude Code's seven-layer authority table and Codex's fail-closed Windows
refusal are the honest references.

**Incorporate:**

- **Named execution profiles compiled to OS enforcement:** projection-only /
  workspace-bounded / networked-build / isolated-guest / owner-local danger
  mode (explicit, visually persistent, never default) / managed cloud. Each
  profile compiles to filesystem, network, process, and secret policy
  (Seatbelt, bubblewrap+seccomp, restricted tokens), and **fails closed when
  the OS cannot represent the policy** — Codex's Windows behavior, and
  Factory's whole-process mode, are the precedents.
- **Two records per run, always:** the authority manifest (what was admitted)
  and the execution receipt (what containment actually ran). Requested versus
  effective enforcement is never rendered as one green shield — Grok Build's
  candid documentation of its own security holes is the posture to keep, with
  receipts instead of prose.
- **Approval taxonomy as behavior contracts.** The Codex computer-use skill
  ships a four-tier consent model worth porting nearly verbatim: hand-off
  required (passwords, security interstitials), always-confirm (deleting
  data, financial transactions, representational communications), pre-approve
  only when the initial prompt names the specific data and destination, and
  no-confirmation trivia (cookie banners). Its doctrines — "typing sensitive
  data into a form counts as transmission," "user-supplied third-party
  content is treated as potentially malicious," untrusted taint is sticky and
  must remain descriptive rather than directive — belong in the
  behavior-contract registry.
- **Fail-closed as a law, everywhere:** an unavailable verifier, a timed-out
  hook, an unparseable verdict, a failed git-ignore check, or missing signing
  secrets is never "allow," never "done," and never "ship unsigned." Four
  separate products exhibit the fail-open bug class.
- **Child authority is an intersection, never a widening.** OpenCode permits
  subagents wider permissions than their parents; that is rejected. Child
  authority = parent grant ∩ child policy ∩ WorkContext ∩ containment.
- **A hermetic execution profile** (from Claude Code's "bare" mode): suppress
  every ambient input — hooks, plugins, memory, learned preferences,
  AGENTS.md — unless explicitly admitted, and emit a manifest of every
  admitted authority source. This is the answer to "what exactly influenced
  this run?" and no competitor ships it completely.
- **Typed, capped, attributed context fragments** (Codex): every piece of
  model context carries a source, scope, content hash, token cap, and
  public/private classification — context provenance a user can inspect.
- **Credential custody rules:** clients are tokenless renderers (Codex);
  secrets live in encrypted stores with OS keychain custody, never plaintext
  fallbacks (Claude Code's non-macOS plaintext fallback is called
  unacceptable), never renderer-visible (OpenCode hands its sidecar password
  to the renderer; OpenChamber exposes runtime tokens), and never held raw by
  a coordinator on the execution path (Crabbox's central flaw). Crabbox's
  **credential-destination provenance lattice** — every credential
  destination tagged by the trust class of the config source that routed it,
  so repository config can never route operator credentials to a
  repo-chosen host — is genuinely novel and worth importing outright.

---

## 4. OpenAgents Desktop

Desktop is where the largest number of specific incorporations land. The
composite target: a quiet work surface whose typed state deepens into a full
coding workbench and fleet cockpit — OpenCode's server-first Electron
topology, T3's workbench breadth, Codex's protocol discipline, Claude Code's
recovery ergonomics, Grok Build's terminal rigor — under the authority layer
of section 3.

### 4.1 Shell and hardening

- **Stock Electron, hardened, is the settled answer.** Anthropic ships stock
  Electron; OpenAI's Owl fork buys nothing OpenAgents needs. Incorporate the
  concrete hardening list both vendors converge on: context isolation,
  sandboxed renderers, `nodeIntegration` off, Electron fuses locked
  (RunAsNode, NODE_OPTIONS, inspect args disabled; cookie encryption; ASAR
  integrity), sender-origin validation and Effect Schema decoding on **every**
  IPC message (OpenCode leaves most IPC schema-unvalidated; Factory has ~69
  handlers with no sender validation — both named gaps), dedicated privileged
  schemes for packaged assets rather than `file://`, partitioned sessions for
  artifact/preview/terminal/browser surfaces, deny-by-default permission,
  navigation, and window-open handlers.
- **A locally versioned renderer, never a live site as the desktop app.**
  Claude desktop's live-`claude.ai` WebContentsView is the named
  anti-pattern; predictability and offline inspectability are product
  advantages.
- **Server-first topology:** the engine runs behind the typed protocol (in a
  utility process or as the local Pylon supervisor); Electron main owns
  lifecycle and native integration only — never conversation state, provider
  calls, tools, Git, or PTYs. Renderer capabilities arrive as brokered,
  scoped, expiring grants (OpenCode's capability-token file grants,
  strengthened so the renderer never holds a server password).

### 4.2 The workbench

- **Right-panel surface manager** (T3's strongest UI mechanism): a tab strip
  hosting `review-summary | diff-file | files | file | terminal | plan |
  preview` surfaces with real tab mechanics, maximize, and inline/sheet
  modes — every surface a projection over engine state, none holding
  authority.
- **Transcript engine before richer cards:** virtualization, a turn
  navigator/minimap, and performance budgets as merge gates. OpenChamber's
  reducer discipline (touched-field cloning cut a message list from 1,972
  renders to 296; per-directory coalescing flushed at frame intervals) and
  its event-pipeline rigor (eager subscription before response body,
  heartbeats, last-event-id, stale-stream detection, interruptible backoff)
  are D1-grade requirements, not polish.
- **Three-level message hierarchy** (T3): prose primary, compact work rows
  secondary, exact evidence on disclosure — with an eight-state table
  (empty/loading/error/partial/etc.) per component and explicit bad-data
  states.
- **Central command registry as the product API:** stable command IDs with
  typed schemas, capability requirements, approval flags, idempotency, and
  redaction class, driving palette, keyboard, menus, slash commands, mobile,
  and model-proposed actions from one catalog (OpenCode's CommandProvider,
  extended with policy and receipts).
- **Composer admission states as typed UI:** idle / active-steerable /
  active-non-steerable / interrupting / repairing / queued / offline /
  blocked — with "Steer now" disabled until a real provider turn ID exists,
  and queue never silently becoming steer.
- **Inline decision stack:** approvals, questions, and plan reviews render at
  the causal point in the timeline; simple approvals inline rather than
  modal.

### 4.3 Subagents: render the tree everyone retains and hides

The two rendering analyses converge on a striking asymmetry: both Codex and
Claude Code **retain** rich multi-agent topology (Codex in an explicit
agent-graph store with BFS traversal; Claude in complete per-agent JSONL
sidechains with 97% reconstructible edges) and both **render** almost none of
it — Codex's TUI flattens the tree to one-line edges, a 6-item status feed,
and one visible transcript; an in-flight spawn renders *nothing* until it
resolves. This is the cheapest large differentiator in the catalog:

- Persist one canonical typed agent graph for OpenAgents' own runs (explicit
  parent edges, lifecycle states extended through delivery, effective
  child model/config recorded at spawn).
- Render the complete roster with live per-child status, causal inline child
  cards at the exact spawn point in the parent timeline (a link projection,
  never transcript flattening), and direct navigation into each child's full
  independent transcript. Desktop gets a three-pane density; mobile a
  drill-down; **no capability tier by surface** — the same typed projection
  renders honestly everywhere, with explicit navigable collapse allowed and
  silent truncation defined as a bug.
- Import foreign histories (Claude sidechains, Codex rollouts) through
  loss-accounted adapters that key by agent identity, reduce every line to a
  typed item, an explicit redaction, or an explicit gap, and render orphans
  as gap nodes (the completeness equation: child files = linked nodes +
  explicit orphan nodes). Never promote a summary to outcome authority.

### 4.4 Recovery, worktrees, and delivery

- **File checkpoints independent of Git** with turn-level, conflict-aware
  rewind: Claude Code's checkpoint store plus Grok Build's conflict preview
  (clean vs externally-modified/created/deleted) plus Command Code's
  three-mode rewind (conversation / code / both), run as a staged two-phase
  undo (OpenCode: stage → inspect → commit-or-clear) that discloses
  reversible versus irreversible effects. T3's hidden-Git-ref checkpoints at
  turn boundaries are the lightweight complement.
- **Worktrees as durable engine resources** with outcome-sensitive lifecycle
  (Claude Code: auto-remove unchanged, retain changed, refuse dirty/unpushed,
  age-gated cleanup, cleanup receipts), bound to Work Units, owners, and
  delivery receipts. Cross-worktree session resume (Grok Build) included.
- **Task state separated from delivery state:** `completed` is not `landed`;
  the delivery lifecycle is first-class in the protocol and visible in the
  UI.
- **Best-of-N and plan-first as typed fan-out:** Cursor's `/best-of-N` and
  plan mode, and Factory's Mission orchestrator/worker/validator topology,
  incorporated as FleetRun fan-out with per-child receipts and an explicit
  typed comparison record — never UI garnish, and never fan-out that can
  self-accept (review checks compiled into assurance manifests, per Amp's
  `amp review`, with the acceptance decision outside the reviewed party).

### 4.5 Terminal, voice, and ambient

- **Terminal renderer family over one typed transcript projection:**
  full-screen TUI, Grok Build's native-scrollback mode (finalized blocks
  committed once to terminal history; only the live turn pinned), headless
  JSON, and ACP — with emulator-backed PTY test matrices, race/fuzz
  scenarios, and checked-in p50/p95/p99 frame-time baselines as release
  gates.
- **Dictation transport discipline** from OpenChamber: 16 kHz PCM16 chunks
  with sequence numbers, highest-contiguous ACK, client retention of unACKed
  audio, explicit finish, adaptive finalization, accept-partial UX.
- **Ambient memory only private-by-construction.** Codex's Chronicle
  (continuous screen OCR + rolling summaries into a profile) proves the
  product line exists; OpenAgents' version, if built, is local-first and
  owner-controlled by construction — and borrows Skysight's anti-injection
  contract (sticky taint; descriptive-not-directive output; privileged
  content reduced or omitted).

### 4.6 Packaging, updates, releases

The build/update analyses give a precise composite:

- **OpenCode's six-target matrix** (macOS/Windows/Linux × x64/arm64,
  architecture-matched runners, three Linux formats, per-channel app
  identities with separate state roots) plus **T3's staging discipline**
  (per-target minimal package synthesis, target-only production deps,
  platform-native helper placement, strict updater-metadata merge, typed
  updater state machine, Rosetta-aware full-artifact fallback, drain-all-
  backends-before-install) — underneath **OpenAgents' trust core**: a signed
  release-set manifest v2 (per-target artifacts with digests and byte
  lengths, whole canonical manifest Ed25519-signed against a pinned key, a
  tiny signed channel pointer promoting immutable versioned manifests),
  fail-closed signing and notarization gates (`codesign`/`spctl`/`stapler`
  oracles on the *outermost* artifact — T3's notarized app inside an
  unsigned DMG died at Gatekeeper), retained-slot rollback, no downgrade
  flags, owned runners as release authority.
- **One signed component-compatibility ledger** across shell, engine,
  renderer, VM/guest images, and plugins — identity, version, hash,
  signature, protocol min/max, channel, last-known-good, rollback rules,
  and a user-visible receipt. Both frontier vendors run three-plus update
  planes with no unified ledger; Factory ships desktop and CLI on divergent
  release trains with no compatibility record. This is an open lane.
- **Coordinated update transactions:** the Grok Build leader's
  relaunch protocol (stop admitting turns → bounded drain → flush → typed
  exit → clients reconnect and reload) as the daemon-update contract.

---

## 5. OpenAgents mobile

The T3 mobile teardown resets the ambition level: a competitive mobile app is
a **full remote coding workbench and fleet controller**, not a chat companion.
T3 ships multi-environment pairing, project/thread browsing, streaming
supervision with approvals and steering, file trees and diff review with
comments, typed Git controls through push, a real native PTY, a durable
per-environment offline outbox, notifications with deep links, widgets and
Live Activities, share targets, and adaptive phone/tablet layout — with the
phone never executing anything. Cursor's iOS app (voice, push, Live
Activities, remote control of desktop agents) confirms the demand.

**Incorporate:**

- **The controller thesis:** the phone issues the same typed commands as
  desktop and web against server-owned sessions; execution never runs
  on-device. Every remote action carries a revocable capability grant and a
  durable outcome; notification status is never completion authority.
- **Any-host environment directory as the first product layer:** discovery,
  QR/manual pairing (bootstrap credential exchanged for a session
  credential), cached-offline environment truth, and reachability presented
  as classed hints, never proof. Access and launch are separate concerns
  (T3's remote model). Relay infrastructure is owned (GCP), end-to-end
  encrypted, and grants **reachability without authorization** — the
  OpenChamber relay trust model: an opaque courier; the client still
  presents its normal credential.
- **DPoP-bound, scope-limited capability tokens** for every
  environment-facing socket and grant (T3's environment-auth is the
  strongest reference), composed with — not replacing — the containment
  layer T3 lacks.
- **Portable sessions as the stable object**, not environment-local threads.
  This is the single structural difference from every competitor: a session
  moves owner-local → managed cloud → back with exclusive attachment
  generations (exactly one host executing), quiesce/checkpoint/detach/
  attach/resume/failback verbs, secret-free checkpoints, fresh grants at the
  destination, and source cleanup receipts. Build the movement substrate
  before widening workbench surfaces.
- **Workbench mode graph:** Attention / Recent / Repositories / Hosts entry
  points; per-session Thread / Files / Changes / Terminal / Preview /
  Artifacts modes; routes and sheets on phone, list+detail+inspector on
  tablet. Changes-writeback is safe by construction: no force push, exact
  post-image receipts.
- **Durable offline outbox** per environment, built on the durable-admission
  contract: commands queued offline with client-chosen idempotent IDs,
  admission ACK shown before "accepted," explicit steer/queue choice
  surfaced to the user, worker epochs and ordered replay on reconnect.
- **Attention as a product:** an attention inbox (needs-you pinning;
  actionable items never collapse), privacy-generic push payloads with
  revalidation-at-open and deep links to the exact session, presence
  suppression, Live Activities/widgets for lock-screen run state, share
  targets and quick actions as controller-contract citizens.
- **Voice as a session-neutral control channel** using the dictation
  seq/ACK transport, layered after the controller core.
- **Verification the T3 way, strengthened:** disposable real servers with
  seeded deterministic state driving screenshot matrices across device
  geometries and themes, plus fault injection (network proxy, token
  revocation, workroom expiry) — with fixture, deployed, and
  physical-device evidence kept as separate claims.

**Refuse:** Capacitor-style web wrappers, duplicate UI trees (T3 maintains a
separate React Native renderer beside its DOM tree — the Effect Native
single-contract mandate is the answer), desktop tokens or raw paths on the
phone, hidden danger modes, hosted third-party relay topology
(Clerk/Cloudflare tunnels/EAS), and notification-as-authority.

---

## 6. openagents.com

The web surface plays three roles the catalog clarifies: the public
projection and trust surface, the remote-supervision client, and the
onboarding front door. It is never a source of desktop privilege and never
the canonical transcript authority (Amp's cloud-only transcript custody is
the named refusal).

**Incorporate:**

- **The thread as a durable, addressable, cross-surface work object** (Amp's
  central lesson): stable IDs and URLs, searchable by text/file/repo/author/
  date, cross-referenceable, remotely controllable — with OpenAgents custody
  semantics: local-first truth, sync as replication of durable typed facts,
  owner-controlled visibility with explicit disclosure states (Amp's
  silent workspace-join visibility change and "unlisted = internet-readable"
  ambiguity are the anti-patterns; visibility transitions get receipts).
- **Remote supervision parity:** the same typed projections and command
  vocabulary as mobile — attention inbox, fleet visibility, approvals,
  steer/queue, continuation links that hand a session to desktop or mobile
  without forking identity.
- **Usage and model truth as a public product surface.** Every competitor
  failed here; OpenAgents publishes: exact model/provider routing receipts
  per call (base model, serving path, no silent substitution), pre-spend
  budget visibility and post-spend reconciliation against exact usage rows,
  and Amp-style transparent pass-through pricing framing. Model identity is
  product contract.
- **A public trust ledger:** release manifests, signing keys, component
  compatibility ledgers, and receipt verification endpoints are
  dereferenceable on the web surface — the place where "signed, fail-closed,
  receipted" becomes visible product rather than internal discipline. This
  extends the existing promises/registry posture to the full release and
  receipt chain.
- **Onboarding gradient:** T3's `npx` zero-install front door (running
  server, migrations, pairing URL with the token in the URL fragment so it
  never reaches the hosted origin, in seconds) is the bar for
  developer-facing onboarding; Cursor's compiled-in Claude-Code-import lane
  shows the value of meeting users inside their existing tool history.
  Web-side: pairing, device linking, and fleet-account connection flows that
  name screens and buttons, not shell commands.
- **A data-flow matrix per work unit:** local reads / uploaded context /
  provider destinations / storage / visibility / retention / training as
  separate stated facts — the legible answer to the
  Factory/Command-Code-style ambiguity, published where users can read it.
- **Documented honesty conventions** carried over from the engine: inert or
  unsupported configuration is labeled, degraded enforcement is shown as
  degraded, and public counters reconcile to exact rows rather than
  narrative.

---

## 7. Cross-cutting planes

Four systems span all three surfaces and deserve their own contracts.

### 7.1 Extensions and authored capabilities

- **Ingest open formats; add provenance.** MCP, MCPB bundles, and skills are
  input formats — wrapped with publisher signatures, content hashes, declared
  capability manifests, org policy, staged download/promote/rollback
  (OpenCode's atomic skill staging), and install/update/run receipts. No
  parallel package format; no moving marketplace branches or silent
  first-run catalog clones (Factory), no plugins executing in the trusted
  server process with dynamic npm install (OpenCode, Amp's plugin privilege
  bundles).
- **Executor's authored-capability loop is the model for user-manufactured
  tools:** authenticated operations become connection-parametric typed
  handles (account identity is invocation data, not ambient state) inside an
  isolated authored function; nested calls re-enter the one dispatcher so
  catalog, policy, approval, and credential resolution apply at every depth;
  published tools re-enter the same catalog as content-addressed, versioned,
  tombstone-managed artifacts. OpenAgents wraps this in a capability broker
  enforcing strict intersection (parent grant ∩ artifact requirement ∩
  connection grant ∩ org policy ∩ profile ceiling), a brokered-function
  isolate profile (default-deny network, no fs/shell/secrets, only the
  capability bridge, receipted), and semantic catalog selection.
- **Code mode, bounded:** both OpenAI (V8 isolate orchestrating MCP calls)
  and OpenCode (confined interpreter over deferred tools) ship it; the win
  is real (context efficiency, computational composition). Adopt only with
  mandatory timeout/tool-count/output/spend budgets, host-side credentials,
  and one receipt per nested call.
- **Lazy discovery everywhere:** skills that start MCP servers but hide tools
  until loaded (Amp), token-budgeted catalogs with exact namespace
  enumeration (Executor), progressive disclosure as the default posture.

### 7.2 Learned preferences and memory

Command Code's Taste is the only preference compiler in the catalog and the
blueprint — corrections, foreign sessions, and Git history compiled into
confidence-scored portable preference packages behind a **separate restricted
writer** (normal file tools reject preference paths; only the narrowed
learning agent writes them). Incorporate as a **governed preference plane**:
observation → typed candidate with evidence, scope, confidence, freshness →
owner review or bounded auto-activation → active generation → application
with a visible "why this choice" → outcome → reinforce/narrow/suspend/
supersede/delete. Keep four memory planes distinct and visible: explicit
instructions, learned preferences, retrieved history, presentation state.
Git-history correction mining is the onboarding gesture worth copying.
Invariants: learning is never default-on over private history; a learned
preference can never widen authority (tools, spend, publication,
acceptance); evidence accompanies every confidence number.

### 7.3 Evidence, work history, and acceptance

- **Amp's `read_thread` law:** compaction summaries orient; original events
  are evidence; later events may supersede. A bounded history-reader role
  returns source-event references with supersession/revert/acceptance state
  and never widens authority.
- **Crabbox's evidence verbs** — `attach / events / logs / results /
  history` over early durable run handles, with failure classification and
  replayable failure capsules — as the operator-facing proof interface for
  any remote or delegated run.
- **Crabbox's lease honesty** for any leased capacity: reserved versus
  estimated cost as distinct ledger concepts, `min(TTL, idle)` expiry,
  heartbeat rejection after expiry, spend caps refusing at admission, and
  fail-closed cleanup that refuses to declare a machine gone while it may
  exist ("labels, names, and IDs alone are not ownership proof").
- **Receipts are countersigned or they are not verification.** Crabbox
  independently arrived at ed25519-signed run receipts and stopped at
  `trust=self-signed` — the precise boundary where OpenAgents' thesis
  begins. Receipts carry containment class and settlement fields and are
  countersigned by the observing authority.
- **Acceptance is deterministic evidence, never model prose.** OpenChamber's
  small-model goal auditor and Command Code's fail-open verifier are the
  cautionary pair; post-image state, tests, PR state, and receipts decide.
- **Blind-consensus QA** (the Codex `hatch-pet` skill): context-isolated
  judges, hidden answer keys, strict majority, audited overrides, and an
  explicit ban on the parent agent approving its own work — the shape for
  automated review fan-out.

### 7.4 Autonomy and scheduling

OpenChamber's Session Goals and scheduled tasks prove the product demand and
the engineering trap: event-driven loops with memory-only timers mean an idle
goal may never re-arm after restart — **persisted metadata is not a durable
continuation lease**. Incorporate autonomy as three separate concerns:
interrupted-turn recovery; a durable session objective (requirements ledger
with evidence predicates); and durable continuation dispatch (startup scans,
leases, idempotent outbox, write-fencing on stale goal generations,
accounting before side effects, user-abort dominance). Factory's autonomy
model adds the policy shape: autonomy level distinct from interaction mode,
org-level clamps that cannot be weakened downstream, and an absolute
blocklist that survives every unsafe flag.

### 7.5 Telemetry and privacy

Grok Build's **exporter-side fail-closed telemetry firewall** — a closed
attribute vocabulary with an exporter-side validator that drops unknown keys
and secret-shaped values — is the pattern; Command Code's undisclosed
fingerprinting is the anti-pattern. Privacy filtering lives server/exporter
side, never only in a bypassable client (Command Code's IDE bridge filters in
the client; any same-user process bypasses it). One user-facing data-flow
matrix, kept consistent across docs, marketing, and behavior.

---

## 8. The refusal list

The catalog is as much a record of what not to build. Consolidated:

1. Default-open execution: `danger-full-access` defaults, approvals-off,
   sandbox-off, computer-use-on-by-default in unattended paths.
2. Fail-open anything: verifiers, hooks, deny rules, signing, git-ignore.
3. Permission UI presented as containment; provider attributes presented as
   isolation; a JSON env var described as a sandbox.
4. Cloud-canonical transcript custody; live streams or notifications as
   completion authority; compaction as history.
5. Coordinator-held raw provider credentials on the execution path.
6. Renderer-held secrets, generic `invoke(command, args)` IPC bridges,
   live-site-as-trusted-renderer.
7. Unsigned or same-origin release authority; downgrade-permissive updaters;
   signing-optional release lanes; declared build-matrix cells without
   native-dependency evidence.
8. Duplicated application topologies: two UI trees, two design systems, two
   query owners, two protocol generations without deletion gates, giant
   god-components as integration boundaries.
9. Silent capability tiers by surface (rich desktop, flattened terminal,
   summary-only mobile) over one protocol.
10. Ambient authority: persisted "always allow" as invisible policy, ambient
    local-socket trust, ambient cwd/AsyncLocalStorage workspace identity,
    marketplace branches that move under installed plugins.
11. Opaque product identity: hidden model routing, silent base-model swaps,
    metering users cannot reconcile, mutually contradictory privacy pages,
    undisclosed fingerprinting.
12. Defaults that evangelize (force-opening flagship surfaces, opt-outs that
    do not hold) and feature deletion without migration/export/deletion
    gates for durable user state.
13. Learned memory default-on over private history; preferences that widen
    authority; confidence without evidence.
14. Compatibility accretion without expiry — every bridge gets an owner,
    telemetry, and a removal date.

---

## 9. Sequencing

The gap analyses agree on dependency order, and it is worth restating as the
essay's operational conclusion:

- **Desktop:** freeze the engine protocol and long-lived supervisor with a
  lossless native event plane first; then the chat column (hierarchy,
  virtualization, composer admission states, inline decisions); then the
  right-panel workbench (review/diff/files first, terminal after PTY proof,
  preview last); then worktree/checkpoint/delivery depth; then remote
  portability; then platform breadth (macOS x64 → Windows/Linux x64 →
  arm64, channels last); additional providers only after one provider is
  complete by the eleven-predicate closure definition (Known, Decoded,
  Owned, Retained, Projected, Presented, Authorized, Recovered, Fast,
  Receipted, Shipped).
- **Mobile:** parity contract and capability matrix first; any-host
  directory and adaptive shell; **portable session movement as substrate
  before workbench breadth**; thread-control completion and push attention;
  Files; Changes with safe writeback; Terminal/Preview/Artifacts; fleet
  operations; voice and ambient last, with signed cross-host dogfood as the
  exit proof.
- **Web:** projection and trust surfaces track the same typed contracts;
  the thread-object, usage-truth, and public-ledger surfaces can advance in
  parallel because they consume receipts rather than produce execution.

Across all three: adapt the supervision shape the market has converged on —
one engine, typed protocol, durable admission, steer/queue/interrupt, agent
trees, worktrees, workbench projections, controller-grade mobile — and build
what the entire audited market declined to build: explicit authority,
effective-containment truth, countersigned receipts, portable session
identity, signed provenance end to end, and honest usage, model, and data
truth. The teardowns' collective verdict is that the first list is table
stakes and the second list is the product.

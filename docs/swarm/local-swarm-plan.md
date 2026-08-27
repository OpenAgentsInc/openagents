# Local Swarm: assessment and plan for agent-to-agent communication

Date: 2026-08-27. Status: assessment + plan, with the first wave of issues
filed from it (see §7 for the tracker mapping).

## 1. The problem, stated concretely

The product reality today: one machine, one human, **eight terminal tabs each
running a Coder session**, plus — inside any of those sessions — `delegate`
fan-outs of up to 32 child agents in worktrees. What cannot happen today:

- Two tabs cannot discover each other. The human is the only message bus.
- A parent Coder session and its `delegate` children communicate only
  through the initial prompt and the final piped output. A child that
  discovers something the parent needs mid-run has no channel.
- Sessions leave no trace the others can address: the local session store
  (`~/.openagents/sessions/<cwd>/<id>/updates.jsonl`) is an archive, not an
  inbox.

The goal — a **local swarm**: sub-agents addressable within a terminal
session, and terminal sessions discovering and messaging each other on the
same machine — with an architecture that later widens to machines (the Box
lane, the Computers domain) without a rewrite.

## 2. Constraints read off the codebase

Any plan must live inside these facts:

1. **Sessions are local-first files.** `session_store.rs`: append-only
   `updates.jsonl` + atomic `summary.json` per session under
   `~/.openagents/sessions/`. "Nothing in this module talks to a server" is
   the stated law. The swarm should extend this, not replace it.
2. **The delegate lane already isolates children.** `delegate.rs` gives every
   child a detached worktree, caps concurrency, streams output line-prefixed,
   and stops process trees by group. Children are already processes the
   parent owns — the missing piece is a *message* channel, not process
   management.
3. **The CLI is one Rust binary** with tokio; unix-domain sockets and file
   watching are cheap to add. There is no daemon for Coder today (the TUI is
   the process); `oa-workroomd` and `oa-node` are separate clouds-facing
   daemons with their own state schemas — precedent for a small local
   daemon, but their contracts are clouds-shaped, not local-swarm-shaped.
4. **Plugins are digest-pinned WASM** with typed mounts (see
   `foreign-sessions`: read-only mounts over `~/.claude`/`~/.codex`, report
   metadata, never resume). A discovery primitive already exists in plugin
   form for *foreign* agents; the native ones are the gap.
5. **Everything is receipted.** Session events carry sequence numbers and
   hashes; cloud contracts carry schema-versioned envelopes. Messages between
   agents will need the same discipline or the trajectory store (and
   CoderBench, which consumes trajectories) will have an unattributable
   hole.
6. **The CoderBench corpus wants these traces.** Swarm conversations are
   exactly the sessions the corpus pipeline (Epic C) will ingest; message
   envelopes should be ATIF-friendly from day one (a message is addressable
   as steps, or cleanly excluded).

## 3. Options assessed

### Option A — Shared-directory mailbox (files only)

Every session already has a directory. Add `inbox.jsonl` there. Discovery =
enumerate `~/.openagents/sessions/*/*/summary.json` plus a small
`swarm.json` registration file; delivery = append a line to another
session's `inbox.jsonl` (atomic append via single `write` of one line with
`O_APPEND`); receipt = the recipient appends an ack line to the sender's
outbox.

- **Pros:** zero new processes; consistent with the local-first store law;
  crash-safe (append-only files survive everything); trivially inspectable
  with `cat`/`jq`; works across worktrees and across Coder *and* delegate
  children without any new runtime; the pattern extends to a shared machine
  directory for cross-machine later.
- **Cons:** no push — recipients notice mail by polling (mtime check on
  turn boundaries is enough in practice); append contention needs one-write
  discipline (a lockfile or single-line `O_APPEND`, which is atomic on local
  filesystems for line-sized writes); delivery latency is bounded by the
  recipient's poll cadence.
- **Verdict:** the base layer. Boring, robust, debuggable, and it matches
  every invariant this repo already holds.

### Option B — Local hub daemon (unix socket)

A small `oa-swarmd` in the CLI's own crate: sessions connect over a unix
socket at `~/.openagents/swarm.sock`, the hub routes messages, keeps live
presence, and can push (wake) a recipient's turn loop.

- **Pros:** true push and presence ("who is alive right now"), one socket
  instead of N file watchers, natural home for access policy later.
- **Cons:** a daemon to supervise, upgrade, and debug; a second source of
  truth to keep consistent with the session files; version-skew problems
  (eight tabs do not upgrade in lockstep); and it duplicates what Option A
  gives for free.
- **Verdict:** the right *second* stage, not the first — and only if push
  proves necessary. Run as a mode of the existing binary
  (`openagents swarm hub`), not a new daemon product.

### Option C — Nostr as the local bus

The product already carries Nostr with a built-in keypair; agent-to-agent
messaging over relays would give discovery, transport, and persistence in
one move, and it works across machines from day one.

- **Pros:** cross-machine for free; identity, receipts (events), and
  subscriptions solved; aligns with the network product story.
- **Cons:** local swarm over public relays leaks every message to relay
  operators (even encrypted, metadata leaks: who talks to whom, when);
  latency and reliability become relay-dependent; local-first inverts —
  nothing works offline unless a local relay is also run, at which point
  Option A already did the job.
- **Verdict:** rejected for the local layer; revisit as the *remote*
  widening (a later issue), where its properties are actually the ones
  wanted.

### Option D — ACP/stdio between sessions

The CLI already speaks ACP to foreign harnesses (Devin et al.). Sessions as
ACP peers, wired at spawn time.

- **Pros:** standards-aligned; parent↔child spawn-time wiring is natural
  (the parent knows the child's stdio).
- **Cons:** ACP is parent-to-agent, not peer-to-peer; a hub of long-lived
  stdio pipes recreates Option B's daemon with less flexibility; unrelated
  tabs (spawned by the human, not each other) have no natural ACP edge.
- **Verdict:** use for the parent↔child *spawn* edge only where it already
  exists; do not build the swarm on it.

### Decision

**A as the substrate, B as an optional accelerator later, C for the remote
widening later, D where it already exists.** Concretely: inbox/outbox files
in the session store, a `swarm.json` presence/registration file per
session, a `openagents swarm` command group for humans and a tool surface
for agents, and message envelopes as schema-versioned JSON with sequence
numbers and receipts.

## 4. The design on top of that decision

### 4.1 Identity and discovery

Every session gets a swarm identity at startup: `<machine>:<session-id>`,
with a registration file `~/.openagents/swarm/<session-id>.json`:

```json
{ "schema": "openagents.swarm.registration.v1",
  "session_id": "...", "pid": 4242, "cwd": "...", "lane": "flash",
  "model": "...", "role": "root|child",
  "parent": null, "worktree": null,
  "inbox": "~/.openagents/sessions/.../inbox.jsonl",
  "alive_after_ms": 30000, "started_at_ms": 0 }
```

Discovery is a directory listing filtered by liveness (`pid` check +
`alive_after_ms` heartbeat). Presence staleness is explicit: a registration
older than its heartbeat is *stale*, not *dead* — shown as such.

Roles: `root` (a tab) or `child` (a delegate child, carrying its parent's
id). The tree is reconstructable from registrations, which is what makes
"parent broadcasts to its subtree" trivial.

### 4.2 Message envelope

```json
{ "schema": "openagents.swarm.message.v1",
  "id": "msg_<hex>", "sequence": 42,
  "from": "<session-id>", "to": "<session-id>|role:children-of:<id>|all",
  "thread": "<msg-id-of-parent>",       // optional: replies/references chain
  "kind": "question|answer|status|handoff|broadcast",
  "body": "…",                           // text; structured payloads later
  "created_at_ms": 0,
  "receipt": { "delivered_at_ms": 0, "read_at_ms": 0 } }
```

- Delivery is append to the recipient's `inbox.jsonl`; the recipient stamps
  `receipt.delivered_at_ms` on poll and `read_at_ms` when its turn loop
  actually consumes it. A message the recipient never acks is visible as
  such — the swarm's "read receipts" are the same honesty discipline as the
  results store.
- `to: "role:children-of:<id>"` and `to: "all"` are *fan-out writes* (the
  sender appends one line per resolved recipient at send time), so the inbox
  files stay the single delivery record and there is no routing state.

### 4.3 Consumption semantics

On each turn boundary (before the model is called), a session drains its
inbox: new messages are surfaced as tool results in the neutral stream
(`swarm.inbox` tool-result entries), so the model sees them like any other
tool output and the transcript stays the one record. Inline injection into
the user turn is explicitly rejected — messages are *tool results*, not
user speech, and must never impersonate the human.

A message may carry `kind: "question"` with
`reply_expected: true`; the harness may (policy-gated) allow the recipient
to answer without human confirmation when the asking party is its own
parent — the parent already owns the child's budget.

### 4.4 The command and tool surface

Commands (human-facing):

```
openagents swarm list                  # live + stale sessions, the tree, lanes, cwds
openagents swarm send <to> <text>      # deliver a message from the invoking context
openagents swarm inbox [session]       # read an inbox (defaults to caller's)
openagents swarm broadcast <text>      # to all (or --role children-of:<id>)
openagents swarm tree                  # parent/child tree of live sessions
```

Tools (model-facing, in the coder's tool surface):

```
swarm_list()                            # who is out there, filtered by cwd/tree
swarm_send(to, body, kind, reply_expected)
swarm_inbox(drain?)                     # fetch/unread messages
```

Delegate integration: `delegate` children register as `role:child` with the
parent id at spawn; the parent gets a `swarm_send`-capable handle per child;
`delegate`'s stop-the-tree already knows the process groups. The fan-out
report gains a "messages exchanged" section (count + highlights), so the
existing report contract carries the swarm dimension.

### 4.5 Safety invariants

1. **No impersonation.** Messages are tool results attributed to a session
   id, styled distinctly; the human is never quoted by an agent message.
2. **Budget honesty.** `swarm_send` costs a turn boundary, not a model call;
   `reply_expected` loops have a depth cap (default 2) and a per-session
   message budget so two agents cannot livelock each other.
3. **Local only, by construction.** Everything lives under
   `~/.openagents/`; there is no network path in the v1 contract. The remote
   widening (Nostr or Box-mediated) is a separate issue with its own
   redaction and consent review.
4. **Trajectory-clean.** Message sends/receives are recorded in
   `updates.jsonl` like other events (typed `swarm_message` events), so the
   session archive, the trace exporter, and CoderBench ingestion see a
   complete, attributable story.

## 5. What I would build first (and in what order)

1. **Registration + discovery + `swarm list/tree`** — the lowest-risk,
   highest-value slice: eight tabs see each other. No message passing yet.
2. **Envelope + inbox delivery + `swarm inbox/send` (human-driven)** — the
   human relays for now; the plumbing is the whole deliverable.
3. **Inbox drain at turn boundaries + `swarm_*` tools** — agents start
   talking; budgets and reply-depth caps active.
4. **Delegate integration** — children register, parent handles appear,
   fan-out reports gain the messages section.
5. **`swarm` hub daemon (Option B) only if polling proves insufficient** —
   measure first: inbox mtime polling at turn boundaries is likely
   imperceptible in practice.
6. **Remote widening (separate issue, later):** Box-mediated or Nostr
   transport with redaction + consent review; the envelope is already
   versioned to survive it.

## 6. Risks

- **Attention hijack:** a chatty neighbor consuming a session's turns. The
  budget, depth cap, and "messages are tool results the model may deprioritize"
  framing contain it; a per-sender mute belongs in v1.1.
- **Inbox as second archive:** two sources of truth for "what happened." The
  mitigation is §4.5's trajectory-clean rule — `updates.jsonl` records the
  events; `inbox.jsonl` is transport, garbage-collected with the session.
- **Overrun of the local-first law:** hub daemon pressure. The decision
  above demotes it to an accelerator, contingent on measured polling pain.
- **Schema churn:** messages are contract; registry-style schema versions on
  every envelope, and golden fixtures, from the first PR.

## 7. Issues filed from this plan

| Issue | Slice |
| --- | --- |
| #182 | **Epic: local swarm** — the umbrella, acceptance = the §5 path walked end to end |
| #183 | Swarm registration + discovery + `swarm list`/`tree` (§5.1) |
| #184 | Message envelope + inbox/outbox delivery + `swarm inbox/send` (§5.2) |
| #185 | Turn-boundary drain + `swarm_*` tools with budgets and reply caps (§5.3) |
| #186 | Delegate integration: child registration, parent handles, fan-out report (§5.4) |

Deferred (deliberately not filed): hub daemon (contingent, §5.5), remote
widening (separate review).

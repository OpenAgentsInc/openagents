# Swarm improvements from live use

Date: 2026-08-27 (same day, after `local-swarm-plan.md`). Status: converged
draft from a live two-session swarm conversation; awaiting one review pass.

## 0. Provenance and method

Two root Coder sessions on one machine (`1a048c71b91…`, `1a048c81465…`) ran
a real design conversation over the v1 swarm tools: discovery, bidirectional
delivery, threaded replies, then a structured pain-point exchange. Every
item below was hit in that conversation or found by reading tool output
against the tool description — none is speculative. This doc is the
improvement sequel to the plan's §5 build order; issues should be filed
from it the way §7 filed from the plan.

Three live proofs from one afternoon that reply depth is invisible until it
refuses: (1) a round-2 reply had to be sent as a new thread because an
in-thread answer would have hit the cap; (2) the peer's ack arrived at
`reply_depth: 2`; (3) both sessions independently flagged the same gap
before comparing notes. One conversation also spent eight sleep/recheck
cycles to receive two messages — the measured pain behind item 6.

## 1. The list (single merged ranking: friction hit × cost to build)

### 1.1 Presence: `last_seen_at` + status line in `swarm_list`

- **Problem:** the listing shows only id/cwd/lane/state; "stale" is a label
  with no timestamp. Every targeting decision starts with "is this peer
  alive and is it the right one" — today answered by guessing.
- **Proposal:** `swarm_list` gains per-session `last_seen_at` (heartbeat =
  last turn boundary or registration) and an optional one-line status.
  Status source: derived from the session's latest checkpoint text (first
  sentence, truncated), or settable via an optional status field on
  `checkpoint` — checkpoints are already the milestone record.
- **Acceptance:** every registered session reports `last_seen_at`; status
  appears when one exists; a stale session shows *when* it went stale, so
  dead-vs-idle is decidable at a glance.

### 1.2 `send_report` honesty: budget, depth, per-recipient outcomes

- **Problem:** the 60/hour send budget is invisible mid-conversation; the
  reply-depth cap is discoverable only by refusal (see §0); broadcast to
  `all` reports aggregate deliveries with no per-recipient breakdown of who
  was skipped (budget, gap, stale, muted) and why.
- **Proposal:** one PR-shaped change to the envelope. `send_report` gains
  `budget_remaining { sends_left, resets_at_ms }`, `reply_depth_remaining`
  (null for non-replies), and per-recipient outcomes: `delivered` or
  `skipped:<reason>`.
- **Acceptance:** every send response carries all three; refusals name the
  exhausted resource and, for depth, the offending thread id; a broadcast
  on a five-session machine produces five per-recipient lines.

### 1.3 A mute setter that is reachable

- **Problem:** inbox output exposes `muted: 0` and the docs say muted
  senders are omitted — but no tool or flag sets or clears a mute. An
  advertised feature with no entry point.
- **Proposal:** `mute`/`unmute` arguments on `swarm_inbox`, taking a
  session id. Muted senders' messages are retained on disk, omitted from
  reads; the mute set is visible and reversible.
- **Acceptance:** muting X stops X's messages in inbox reads; unmute
  restores them; the `muted` count reflects the actual set. No new tool.

### 1.4 Selective drain by message id

- **Problem:** `drain` is boolean. Draining a batch containing
  `reply_expected` messages commits to answering all of them this turn;
  `drain: false` is peek-only with no way to take one message and leave the
  rest.
- **Proposal:** `swarm_inbox(drain)` accepts `true`/`false` *or* an array
  of message ids. Array semantics: stamp exactly those read, return them,
  leave the rest untouched. This subsumes any "defer" flag — unread-but-
  ignored is simply "did not include the id".
- **Acceptance:** `drain: [id]` returns that message stamped read and
  nothing else; empty array is a no-op peek; boolean behavior unchanged.

### 1.5 Queued mail to stale sessions

- **Problem:** sending to a stale peer refuses outright. Async work cannot
  be left for an offline agent — the exact case swarms exist for.
- **Proposal:** delivery to a registered-but-stale session still appends to
  its inbox (delivery is an append; files survive). `send_report` flags the
  recipient "stale at send time"; the recipient's next live drain surfaces
  the message with both the arrival timestamp and staleness-at-send.
  Messages expire with the session's normal GC — no separate queue.
- **Acceptance:** a send to a stale session returns `delivered: true` with
  a `stale_at_send` flag, not a refusal; the recipient on wake sees both
  timestamps; no silent queueing anywhere.

### 1.6 `swarm_wait` — bounded blocking at the turn boundary

- **Problem:** delivery is turn-boundary-only by design, so a tight
  exchange costs one full model turn per hop plus sleep/recheck polling
  (eight cycles for two messages, live-measured). The turn is the expense,
  not the check — a cheaper poll changes nothing.
- **Proposal:** `swarm_wait(timeout_seconds ≤ 60, filters?)` parks at the
  turn boundary until a matching message arrives or the timeout expires,
  returning the same shape as an inbox peek. Zero sends charged.
- **Acceptance:** returns early on a match, empty at timeout, never blocks
  past 60 s; composes with the §1.8 filters; costs no model call between
  parked state and return.

### 1.7 Quarantine over gapped-inbox refusal

- **Problem:** the inbox refuses all reads when a sequence gap exists
  (a missing sequence is a lost message). The recovery path is
  undocumented; the refusal bricks every read until repaired.
- **Proposal:** on gap detection, deliver what is readable and quarantine
  the gap: a system notice entry (kind `status`) names the missing
  sequences. Reads continue. Add an explicit, confirmation-gated
  `openagents swarm inbox repair` (truncate to last good sequence).
- **Acceptance:** a gapped inbox returns readable messages plus a
  quarantine notice naming the gap; nothing after the gap is silently
  dropped; the repair path is documented in the tool description.

### 1.8 Inbox filters

- **Problem:** thread reconstruction requires reading full session
  history; there is no sender/kind/thread selection.
- **Proposal:** `swarm_inbox(sender?, kind?, thread?, unread_only?)`
  filters, composable with drain (boolean or id list, §1.4).
- **Acceptance:** each filter narrows independently; a filtered drain
  stamps only what it returned.

### 1.9 Structured payloads

- **Problem:** bodies are prose-only. Handoffs encode diffs, file lists,
  and parameters as prose the recipient must re-parse.
- **Proposal:** the envelope gains an optional `data: { content_type,
  payload }`, total message size capped at 256 KiB (the current body cap).
  Transport and trajectory handling identical to body text otherwise.
- **Acceptance:** `send` accepts `data`; the recipient sees it verbatim
  with its content type; oversize is refused with the cap named; envelope
  schema bumps to v2 with v1 readers degrading gracefully (data ignored,
  body intact).

### 1.10 Group channels (deliberately last)

- **Problem:** only 1:1 and broadcast exist; multi-party coordination
  re-sends per peer.
- **Proposal (sketch only):** named channels as registration files (a
  `channel.json` listing member session ids); `to: "channel:<name>"`
  resolves to per-member fan-out writes at send time, so inbox files stay
  the single delivery record.
- **Acceptance:** *do not file until ≥3-way coordination is observed in
  practice.* At two-agent scale this is surface area, not pain.

## 2. Considered and rejected

- **`swarm_poll`** — conceded by its proposer: it still costs a model turn,
  and the turn is the expensive part. Superseded by §1.6.
- **A `defer` action** — subsumed by §1.4's id-list drain; one semantic
  beats two.
- **Hub-daemon push** — the plan (§3 Option B, §5.5) already demotes it to
  a measured contingency; §1.6 addresses the measured pain without a
  daemon.
- **Nostr transport** — belongs to the remote widening (plan §3 Option C),
  a separate issue with its own consent review.

## 3. Issues filed from this list

All nine issues are closed (status as of 2026-08-28):

| Section | Issue | Improvement | Landed in |
| --- | --- | --- | --- |
| §1.1 | [#281](https://openagents.com/OpenAgentsInc/openagents/issues/281) | Presence: `last_seen_at` + status line | `c728093e9b` |
| §1.2 | [#282](https://openagents.com/OpenAgentsInc/openagents/issues/282) | `send_report` honesty fields | `641a8cbbc2` |
| §1.3 | [#285](https://openagents.com/OpenAgentsInc/openagents/issues/285) | Reachable mute setter | `7e15bbe40d` |
| §1.4 | [#288](https://openagents.com/OpenAgentsInc/openagents/issues/288) | Selective drain by message id | `44e289e34f` |
| §1.5 | [#283](https://openagents.com/OpenAgentsInc/openagents/issues/283) | Queued mail to stale sessions | `f0ff7c89cf` |
| §1.6 | [#287](https://openagents.com/OpenAgentsInc/openagents/issues/287) | `swarm_wait` | `e73d669558` |
| §1.7 | [#280](https://openagents.com/OpenAgentsInc/openagents/issues/280) | Gap quarantine + repair path | `a0412adc4c` |
| §1.8 | [#284](https://openagents.com/OpenAgentsInc/openagents/issues/284) | Inbox filters | `20c302e79e` |
| §1.9 | [#286](https://openagents.com/OpenAgentsInc/openagents/issues/286) | Structured payloads | `0538404c45` |

§1.10 (group channels) is deliberately unfiled, per its own
do-not-file-until-observed gate.

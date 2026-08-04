> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.

NIP-AS
======

Agent Sessions
--------------

`draft` `optional`

This NIP defines the **Agent Session**: the durable record of one agent's
bounded engagement with Work. A session survives client disconnects and
provider failures, is generation-fenced against its Delegation Grant, and
keeps four things that are routinely conflated strictly apart:

- the **Work** (durable lifecycle root, NIP-WK);
- the **Thread** (durable conversational context, NIP-WA);
- the **Run** (lane-specific execution lifecycle in the owning runtime);
- the **Session** (this record: one live or resumable agent interaction).

Provider completion is a fact inside the session. It is not verification,
not acceptance, and not an outcome — those stay with NIP-EV.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32280 | Addressable | Agent Session record |
| 32281-32289 | — | Reserved for future NIP-AS use |

Authority-signed; lifecycle changes flow through NIP-WI
(`work.session.control` with `start`, `steer`, `pause`, `resume`, `stop`,
`dismiss` arguments).

## 1. Agent Session (`kind:32280`)

Address:

```text
32280:<authority_pubkey>:<session_ref>
```

### 1.1 Required tags

- `d`: stable `session_ref`
- `org`
- `work`: the target Work
- `p` with marker `agent`: the executing Agent Member
- `p` with marker `initiator`: the principal who started the session
- `a` with marker `grant`: the NIP-AD Delegation Grant
- `generation`: the grant generation this session runs under
- `state`: `pending`, `active`, `awaiting_input`, `paused`, `complete`,
  `error`, `stale`, `stopped`, or `revoked`
- `started_at`; plus `terminal_at` once terminal
- `revision`, `published_at`

### 1.2 Recommended tags

- `a` with marker `thread`: the Workroom Thread (NIP-WA) carrying the
  session's collaboration projections
- `run`: the lane-specific Run ref in the owning runtime
- `a` with marker `context`: the Context Manifest ref — the versioned
  record of exactly which sources, instructions, policies, tools,
  exclusions, and digests the session received
- `a` with marker `plan`: current plan ref with its revision
- `host`: the Placement/Host ref (future NIP-HP)
- `runtime` with markers `requested` / `effective`: the requested and
  provider-reported runtime/model identities — never collapsed into one
  field, per the no-silent-substitution rule
- `cursor`: the NIP-AV activity cursor (latest admitted activity `seq`)
- `a`/`e` with markers `artifact`, `provider_result`, `verification`,
  `disposition`: downstream refs as they appear
- `a` with marker `predecessor`: the prior session on retry/handoff
- `blocker`: typed blocker refs while `awaiting_input` or `error`

`content` is empty or a bounded public-safe summary. Prompts, transcripts,
and provider payloads never appear here; the session points at them
through the thread, activity stream, and off-relay stores.

### 1.3 State rules

- `awaiting_input` names its elicitation: the blocking NIP-AV activity
  ref, so attention systems can route the question.
- `stale` is declared, not assumed: the authority marks a session stale
  after its liveness policy expires, and a stale session's late results
  are fenced exactly like a revoked one's.
- `revoked` follows the grant: when the NIP-AD grant is revoked or
  superseded, every session on the old generation moves to `revoked`, and
  NIP-WI intents carrying the old generation refuse `stale_generation`.
- Terminal states are terminal. Continuation is a new session with a
  `predecessor` ref — identity is never silently reused, so cost,
  activity, and evidence attribution stay unambiguous.
- `complete` means the agent's engagement ended with a result recorded.
  It carries no implication about verification or acceptance; the NIP-EV
  refs are attached separately or not at all.

### 1.4 Example

```json
{
  "kind": 32280,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "sess-2f88"],
    ["org", "org-openagents"],
    ["work", "work-9f31854f"],
    ["p", "<agent-pubkey>", "", "agent"],
    ["p", "<owner-pubkey>", "", "initiator"],
    ["a", "32271:<authority-pubkey>:grant-a41c", "", "grant"],
    ["generation", "3"],
    ["state", "active"],
    ["a", "32104:<authority-pubkey>:room-dev", "", "thread"],
    ["a", "ctx:manifest-90aa", "", "context"],
    ["runtime", "codex-3", "", "requested"],
    ["runtime", "codex-3", "", "effective"],
    ["cursor", "41"],
    ["started_at", "1786500000"],
    ["revision", "5"],
    ["published_at", "1786500900"]
  ]
}
```

## 2. Control flow

Every control action is a NIP-WI intent naming the session, the grant, and
the current generation:

- **start** admits a new session under an active grant, and refuses when
  another active session holds the Work under the same grant class;
- **steer** adds direction to compatible active work without pretending
  it is a new session;
- **pause / resume** are explicit states, not inferred from silence;
- **stop** requests interruption and records the resulting state — a
  stopped session is not a completed one;
- **dismiss** archives a terminal session from active views without
  deleting its history.

Answers to elicitations are intents too (`work.session.control` with an
`answer` argument naming the activity), so approvals and answers carry
actor, admission, and provenance like every other mutation.

## Security considerations

- **Model-identity truth.** `requested` and `effective` runtime tags stay
  separate; a session whose effective identity is missing displays as
  unverified-identity, and claims about which model ran require the
  effective value's provenance.
- **Zombie sessions.** Liveness comes from the activity cursor and the
  authority's staleness policy, never from a client's connection state.
- **Context laundering.** The context manifest ref makes the session's
  inputs auditable; an activity or result that cites material outside the
  manifest is a review flag.
- **Privacy.** Session records are refs and states. Block NIP-AO carries
  live encrypted telemetry, Block NIP-AM carries encrypted turn metrics,
  and neither is duplicated here in plaintext.

## References

- NIP-01
- NIP-WK, NIP-WI, NIP-EV (layer 0)
- NIP-AD, NIP-AV, NIP-CC, NIP-WA (this layer)
- Block NIP-AO, NIP-AM — live telemetry and turn metrics
- NIP-SA — sovereign-agent tick/trajectory model this specializes toward
  workspace Work
- `docs/omega/2026-08-03-work-command-admission-authority.md`

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: the Agent Session record, state machine with
  grant-generation fencing, control-flow intents, and identity-truth
  rules.

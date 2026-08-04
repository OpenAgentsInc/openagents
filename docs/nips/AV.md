> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.

NIP-AV
======

Agent Activity
--------------

`draft` `optional`

This NIP defines the **Agent Activity** stream: the typed, safe,
append-only record of meaningful agent progress inside a Session. It is
how supervision works without exposing raw provider internals — an
auditable timeline of what the agent did, asked, produced, and failed at,
with every record redacted to the Observed Agent Activity rules.

Two hard boundaries:

- **Never raw internals.** No hidden chain of thought, raw prompts, raw
  shell output, provider payloads, credentials, or local paths. Bounded
  summaries, labels, digests, and counts only.
- **Narration is not effect.** An activity says the agent reported
  something. A completed-sounding activity creates no Receipt; effects are
  proven by NIP-EV records the activity may reference.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32290 | Addressable (unique `d`) | Agent Activity |
| 32291-32299 | — | Reserved for future NIP-AV use |

Authority-signed: the authority admits provider events, applies redaction,
and publishes the canonical activity. The actor-signed workroom projection
of the same fact is NIP-WA `agent_activity` (`kind:32156`); see §3.

## 1. Agent Activity (`kind:32290`)

Address:

```text
32290:<authority_pubkey>:<session_ref>:act:<seq>
```

### 1.1 Required tags

- `d`: `<session_ref>:act:<seq>` (unique, append-only)
- `org`
- `work` and `session`: the owning Work and Agent Session
- `seq`: dense monotonic sequence within the session
- `kind_label`: the activity kind (see 1.3)
- `generation`: the grant generation the session ran under
- `occurred_at` and `admitted_at`

### 1.2 Recommended tags

- `p` with marker `actor`: the agent (or answering principal, for
  `elicitation` answers)
- `x`: digest of the exact underlying material (tool output, artifact,
  report) held off-relay
- `e`/`a` with markers `effect`, `evidence`, `receipt`: NIP-EV and
  effect refs, when an admitted effect exists
- `e` with marker `provider_event`: the bounded provider-event ref
- `loss`: explicit loss-accounting refs where the underlying record is
  truncated, compacted, or unavailable
- `answer` state tags for elicitations (see 1.4)

`content` carries the bounded public-safe summary for the activity — a
progress sentence, a tool label with counts ("edited 3 files",
"command output 2.1 KB"), a question's text — or a NIP-44 payload for
restricted audiences.

### 1.3 Activity kinds

| `kind_label` | Meaning | Boundary |
| --- | --- | --- |
| `progress` | Safe summary of current work | Never chain of thought |
| `plan` | Plan proposed or revised | Carries plan ref + revision; a proposed plan is not accepted authority |
| `elicitation` | Question, approval request, or missing input | Durable, answer-fenced (1.4) |
| `action` | Typed tool attempt and result | Tool label, grant ref, idempotency ref; not the raw command/output |
| `artifact` | Artifact published or updated | Digest-bound ref |
| `result` | Agent or provider result summary | Not verified or accepted by itself |
| `verification` | Verification attempt recorded | References the NIP-EV receipt; preserves producer/verifier split |
| `error` | Typed failure or blocker | Distinguishes retryable, denied, stale, interrupted, terminal |
| `disposition` | Authority/owner decision echoed into the timeline | The NIP-EV record remains the truth |

Unknown kinds are preserved and displayed as unknown.

### 1.4 Elicitation fencing

An `elicitation` activity is answerable exactly once:

- it carries `["answer_state", "open"]` and an optional deadline;
- the answer arrives as a NIP-WI intent naming this activity; admission
  publishes a successor activity (`kind_label=elicitation`,
  `answer_state=answered`, `p` marker `actor` = the answerer, `e` marker
  `answers` → the question) and republishes nothing else;
- a second answer refuses `conflict`; an expired question moves to
  `answer_state=expired` and any later answer is refused.

This is what makes approvals auditable: the question, the answerer, the
decision, and the admission are all separate signed records.

### 1.5 Ordering and loss

`seq` is dense per session. A gap is an explicit display fact. Where the
authority knows material was lost (provider truncation, compaction,
crash), it publishes the activity with `loss` refs rather than silently
narrowing history — search and summaries must retain these gaps.

## 2. Example

```json
{
  "kind": 32290,
  "pubkey": "<authority-pubkey>",
  "content": "Ran verification command; 14 tests passed, 0 failed.",
  "tags": [
    ["d", "sess-2f88:act:41"],
    ["org", "org-openagents"],
    ["work", "work-9f31854f"],
    ["session", "sess-2f88"],
    ["seq", "41"],
    ["kind_label", "action"],
    ["generation", "3"],
    ["p", "<agent-pubkey>", "", "actor"],
    ["x", "<sha256-of-tool-output>"],
    ["e", "<evidence-receipt-id>", "", "evidence"],
    ["occurred_at", "1786501200"],
    ["admitted_at", "1786501201"]
  ]
}
```

## 3. Relationship to NIP-WA agent_activity

The same underlying fact can appear twice, deliberately:

- **NIP-AV `32290`** is the authority-signed canonical activity: admitted,
  redacted, sequence-ordered, provenance-bearing. It is what audits and
  attention systems consume.
- **NIP-WA `32156`** is the actor-signed workroom projection: the agent
  (through its signer grant) publishing its activity into the
  collaboration thread, payload-digest-bound, with
  `relayAcceptanceIsAuthority: false`.

The WA projection references the AV activity where both exist. Divergence
between them is a review flag, and the AV record wins for product meaning.

## Security considerations

- **Injection surface.** Activity summaries quote agent- and
  tool-produced text. They are data on every surface that renders or
  re-feeds them; a summary cannot steer tools or widen authority.
- **Completion theater.** `result` activities with confident prose and no
  `evidence` refs are the canonical false-green shape. Clients MUST
  render evidence-less results as unverified.
- **Redaction failures.** The activity pipeline is a disclosure boundary.
  A record found carrying raw secrets is superseded by a redacted
  replacement plus an incident ref — relays may retain the original, so
  prevention (admission-time scanning) is the real control.
- **Cost accounting.** Token/cost truth lives in Block NIP-AM encrypted
  metrics and exact ledgers, not in activity prose.

## References

- NIP-01, NIP-44
- NIP-WK, NIP-WI, NIP-EV (layer 0)
- NIP-AS, NIP-WA (this layer)
- Block NIP-AO, NIP-AM
- `docs/omega/GLOSSARY.md` — Observed Agent Activity, Safe Message
  Chain, Loss Accounting

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: the redacted activity stream, kind vocabulary,
  elicitation fencing, loss accounting, and the AV/WA split.

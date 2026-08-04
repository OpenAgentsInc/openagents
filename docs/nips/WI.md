> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 0 — foundation.

NIP-WI
======

Work Intents and Admission
--------------------------

`draft` `optional`

This NIP defines the command wire of the All Work system: how a participant
proposes a change to Work, and how the All Work authority answers.

The core law:

> A client-signed event is a proposal. It has product meaning only after an
> authority-signed admission result exists.

This is what makes "Nostr as the backend" honest. Both halves of every
command — the proposal and the decision — are signed events on the wire, so
any client can audit what was requested, what was admitted, what was
refused, and why. No client-signed event mutates canonical state by itself,
and no relay can admit a command by accepting it.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32180 | Addressable (unique `d`) | Work Intent (participant-signed proposal) |
| 32181 | Addressable | Admission Result (authority-signed decision) |
| 32182 | Addressable | Work Candidate (untrusted intake) |
| 32183-32189 | — | Reserved for future NIP-WI use |

## 1. Work Intent (`kind:32180`)

A typed request to act, signed by the requesting participant. Address:

```text
32180:<participant_pubkey>:<intent_ref>
```

### 1.1 Required tags

- `d`: unique `intent_ref`, chosen by the client
- `p` with marker `authority`: the target All Work authority key (from the
  NIP-OT Organization record)
- `org`: target Organization ref
- `op`: the operation (see 1.4)
- `idem`: idempotency key for this logical command
- `x`: SHA-256 digest of the exact argument bytes
- `exp`: expiry timestamp after which the intent MUST NOT be admitted

### 1.2 Conditional tags

- `work`: target `work_ref` — required for every operation except
  `work.create`
- `expected_revision`: the Work revision the client observed — required for
  every mutating operation on existing Work
- `generation`: the session/delegation generation the actor operates under —
  required when the actor is a Delegate or Session-bound (NIP-AD, NIP-AS)
- `a` with marker `grant`: the Delegation Grant the actor invokes, when the
  actor is not the accountable owner

### 1.3 Content

`content` carries the operation arguments as JSON, either plaintext when the
arguments are public-safe or NIP-44-encrypted to the authority. The `x` tag
binds the exact bytes in both cases, so an admission can prove which
arguments it decided without republishing them.

Arguments are untrusted data. Prose inside them cannot widen tools,
repositories, budgets, audiences, or authority — the authority intersects
the request with the actor's actual grants before admission.

### 1.4 Operations

Recommended baseline vocabulary:

| Operation | Effect requested |
| --- | --- |
| `work.create` | Create Work (and its Issue projection) |
| `work.update` | Update title, description ref, or metadata |
| `work.objective.revise` | Publish a new Objective revision |
| `work.state.set` | Move Workflow/Work State |
| `work.relate` | Add or remove a typed Work Relation |
| `work.assign` | Set or clear the accountable Assignee |
| `work.delegate` | Issue, replace, or revoke a Delegation Grant |
| `work.comment` | Attach a comment (NIP-22 anchored) |
| `work.session.control` | Start, steer, pause, resume, or stop a Session |
| `work.evidence.attach` | Attach an Evidence Receipt ref |
| `work.disposition.record` | Record an Owner Disposition |
| `work.close` / `work.reopen` / `work.archive` | Terminal transitions |
| `candidate.triage` | Dispose of a Work Candidate (see 3) |

Deployments MAY extend the vocabulary. An authority MUST refuse an unknown
operation rather than guess.

### 1.5 Example

```json
{
  "kind": 32180,
  "pubkey": "<participant-pubkey>",
  "content": "{\"state\":\"in_review\",\"reason\":\"patch pushed, verification green\"}",
  "tags": [
    ["d", "int-7c2e11"],
    ["p", "<authority-pubkey>", "", "authority"],
    ["org", "org-openagents"],
    ["op", "work.state.set"],
    ["work", "work-9f31854f"],
    ["expected_revision", "7"],
    ["idem", "state-review-7c2e11"],
    ["x", "<sha256-of-content-bytes>"],
    ["exp", "1786503600"]
  ]
}
```

## 2. Admission Result (`kind:32181`)

The authority's decision on exactly one intent. Address:

```text
32181:<authority_pubkey>:<intent_ref>
```

Using the intent's own `intent_ref` as `d` makes the result addressable from
the intent and makes idempotent replay natural: re-answering the same intent
replaces the record with identical content.

### 2.1 Required tags

- `d`: the decided `intent_ref`
- `e` with marker `intent`: the intent event id
- `p` with marker `actor`: the intent signer
- `status`: `admitted` or `refused`
- `x`: the intent's argument digest, echoed
- `admitted_at`

### 2.2 Status-dependent tags

When `status=admitted`:

- `work`: the affected (or newly created) `work_ref`
- `revision`: the Work revision the admission produced
- `e` with marker `event`: each resulting NIP-WK Work Event (`kind:32171`)

When `status=refused`:

- `reason`: one of `stale_revision`, `stale_generation`, `unauthorized`,
  `unknown_operation`, `invalid_arguments`, `conflict`,
  `idempotency_conflict`, `expired`, `policy_refused`, `budget_exceeded`,
  `subject_not_found`, or a deployment-declared code
- optional `e` with marker `conflict`: the conflicting record, when naming
  it leaks nothing private

### 2.3 Admission laws

1. **Fail closed.** Any boundary mismatch — wrong Organization, unknown
   actor, missing capability or grant, stale `expected_revision`, stale
   `generation`, expired intent — refuses before mutation.
2. **Idempotency.** An identical replay (same `idem`, same argument digest)
   returns the original admission result. The same `idem` with different
   bytes is refused with `idempotency_conflict`.
3. **One decision per intent.** An intent reaches exactly one terminal
   admission result. Later replacements of the `32181` record MUST be
   byte-identical re-publications, never a changed verdict.
4. **Authority provenance.** Every Work Event produced by an admission
   carries `intent` and `admission` refs (NIP-WK 2.2), so the complete
   chain — who asked, what was decided, what changed — is reconstructible
   from relay data.
5. **No inference from silence.** An unanswered intent is pending until its
   `exp`; expiry without an admission is a typed non-event, not a refusal
   record, and clients MUST NOT invent one.

## 3. Work Candidate (`kind:32182`)

Untrusted intake: externally sourced reports that may become Work but are
not Work. This is the wire form of the strict-bug-candidate pattern: public
intake surfaces produce candidates; only an explicit triage admission
produces Work.

Address:

```text
32182:<authority_pubkey>:<candidate_ref>
```

### 3.1 Required tags

- `d`: unique `candidate_ref`
- `org`: target Organization ref
- `source`: typed source class (`public_form`, `webhook`, `intake_relay`,
  `import`, or deployment-declared)
- `e` or `url` with marker `source`: the source delivery ref
- `trust`: `untrusted` (the only value at ingress)
- `state`: `pending`, `admitted`, `rejected`, `duplicate`, or `linked`
- `published_at`

### 3.2 Rules

- Candidates are authority-signed: the transport adapter verifies the
  external delivery (signature, HMAC, form validation) before the authority
  publishes the candidate. The raw delivery and its secrets never appear on
  the relay; only the normalized public-safe payload or its digest does.
- Ingress is idempotent on the delivery ref: an exact replay returns the
  existing candidate; a different delivery for the same source identity is
  a conflict.
- Triage is a separate `candidate.triage` intent from an authorized
  principal. `admitted`, `duplicate`, and `linked` decisions carry a
  `work` ref; `rejected` does not.
- A candidate-to-Work link is provenance only. It grants no assignment,
  claim, session, or command authority, and candidate content remains
  untrusted data wherever it is displayed or fed to an agent.

## Security considerations

- **Prompt injection.** Intent arguments and candidate payloads are data.
  Authorities and agents MUST NOT let embedded text widen authority.
- **Replay and reordering.** `idem` plus argument digest defeats replay;
  `expected_revision` and `generation` defeat stale and out-of-order
  mutation; `exp` bounds how long a captured intent stays dangerous.
- **Relay-level censorship.** A relay can drop intents or admissions.
  Clients SHOULD publish to the Organization's declared relay set (NIP-OT /
  NIP-65) and treat missing admissions as pending, never as consent.
- **Authority spoofing.** Clients resolve the authority key from the
  Organization record and MUST ignore `32181`/`32182` events signed by any
  other key.
- **Private arguments.** Encrypt to the authority when arguments are not
  public-safe; the echoed digest keeps the audit chain intact without
  disclosure.

## References

- NIP-01, NIP-22, NIP-44, NIP-65
- NIP-OT, NIP-WK, NIP-EV (this program)
- NIP-AD, NIP-AS (this program, layer 2 — grants and sessions)
- `docs/omega/2026-08-03-work-command-admission-authority.md`
- `docs/omega/2026-08-03-strict-bug-candidate-ingress.md`

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Work Intent, Admission Result, Work Candidate,
  operation vocabulary, admission laws, and intake rules.

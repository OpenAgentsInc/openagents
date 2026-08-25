# System memory: specification

- Class: design specification
- Date: 2026-08-25
- Revised: 2026-08-25, re-based on the cloud substrate. See "Substrate".
- Status: accepted design for the system bucket; no code lands under the
  owning issue
- Owning issue: OpenAgentsInc/openagents#52
- Base: the cloud memory store in the openagents.com Phoenix and Postgres
  server, the user and learned buckets in #51, and the knowledge base in #49
- Vocabulary: transparency tiers reuse the forge's `dark/pulse/ledger/glass`
  names (`openagents.com` `lib/openagents/transparency.ex`)

## Substrate

This spec was first written against the local engram ledger: signed NIP-AE
`kind:30174` events in `~/.openagents/memory/engrams.jsonl`, distributed by a
Nostr transport. On 2026-08-25 the owner replaced that substrate.
**Memories live in the centralized cloud database.** The store is a
`memories` table in the openagents.com Postgres, recall runs server-side
inside `POST /api/v1/responses`, writes go through the memories API, and the
local engram code is frozen rather than extended. The Nostr transport is
dropped from the roadmap, and the build issue that would have delivered relay
distribution (#63) is closed as not planned.

The re-base changes the container, not the judgments. Evidence-backed
admission, dated claims, correction by supersession, the privacy tiers, the
poisoning caps, and the knowledge-base line are properties of the design, not
of the ledger, and they stand as written. Where a section named an event, a
signature, a relay, or a local ledger, it now names a row, an authenticated
writer, the server, and the table. Section 6 records the transport decision
itself and keeps the superseded design for reference.

Two things a reader should carry into the build. First, the server is not
greenfield: `OpenAgentsInc/openagents.com` already runs a profile-memory
plane and a private experience-memory plane, both with append-and-supersede
records, evidence refs, consent, redaction, and hybrid lexical and pgvector
recall. The `memories` table this spec assumes has to be reconciled with
those planes, not laid beside them. Second, that repo's `INVARIANTS.md`
carries MEMORY-001 through MEMORY-009, and MEMORY-001 confines recall to the
current account conversation with no unscoped fallback. A `system` bucket
read into every account's turn is cross-account recall by construction, so
building it means amending that invariant deliberately, with the eligibility
filter in section 7.1 as the predicate that replaces the scope predicate.

## 1. What a system memory is

The memory system has two buckets today, both account-scoped (#51):

- **user** — things the reader explicitly asked to have remembered.
- **learned** — heuristics the consolidation pass distills from episodes.

This document specifies the third bucket:

- **system** — things the network as a whole has learned and every agent
  should know. Examples: "the gateway returns 402 when the default model is
  retired; check upstream status before bisecting", "`mix precommit` installs
  the push guard; a GitHub push is overwritten by the next mirror".

A system memory differs from the other buckets in kind, not just in scope.
A wrong user memory misleads one session; a wrong system memory propagates to
every session on the network. A leaked user memory harms one owner; a system
memory derived from a private transcript is a privacy breach by construction.
An unattributed system memory cannot be challenged, corrected, or discounted.
Every decision below follows from those three failure modes.

The one-sentence design: **a system memory is an ordinary memory row that has
passed an evidence-backed admission gate, carries a transparency tier of
`ledger` or above, is served from the one shared store every client already
reads, and can be challenged and superseded by further records — never
edited.**

## 2. What stays unchanged

The system bucket adds no new store and no second channel. It reuses,
unmodified:

- **The memory row model** (#51): account-scoped rows carrying a bucket, a
  body, a source reference, and a supersession pointer. A correction inserts a
  new row and points the old one at it; it never edits in place.
- **Derived status.** A memory's effective state is computed from the records
  that reference it, not read from a field the author set. The derivation is
  pure, idempotent, and order-independent, and an unresolved supersession
  chain is refused whole.
- **The ranking discipline**: deterministic top-K, salience recall, and
  token-budget packing, with stable tie-breaks so equal inputs give equal
  notes. Retrieval targets embedding search per the workspace retrieval
  invariant; a full-text slice is a marked, swappable stand-in.
- **The redaction gate**: the hard-unsafe rule set that refuses a write
  outright, applied at the write boundary before any row lands.
- **The transparency vocabulary**: `dark` (nothing public), `pulse`
  (metadata only), `ledger` (content and metadata), `glass` (full access).

What the bucket adds is a set of system fields on the row (section 3), three
record roles — admission, challenge, and refutation (sections 4 and 7.3) —
and recall caps (section 7).

## 3. Record shape

A system memory is a row in the shared memory store whose bucket is `system`
and which carries the system-only fields below. Rows in the `user` and
`learned` buckets are unchanged and carry none of them. This shape replaces
the `kind:30174` companion block the spec first carried; the fields and the
reasons for them are the same, restated as columns.

| Field | Values | Notes |
| --- | --- | --- |
| `bucket` | `user`, `learned`, `system` | `system` selects this shape |
| `slug` | string | `sys:` prefix for system memories |
| `body` | text | the redacted claim, in plain language |
| `author_id` | account reference | the account that wrote the row |
| `entity` | string, optional | what the claim is about |
| `tier` | `ledger`, `glass` | never lower; see section 5 |
| `as_of` | date | the date the claim was observed true |
| `admission` | `candidate`, `admitted`, `rejected` | the author's claim only; see section 4 |
| `superseded_by_id` | row reference, optional | the only correction path |
| `source_refs` | list of `{ref, role}` | roles include the three added in section 4 |
| `evidence_refs` | non-empty list of `{kind, ref, digest}` | `kind` is `receipt`, `memory`, or `url` |

An example body: "A 402 from the inference gateway usually means the default
model was retired upstream. Check gateway status before bisecting local
lanes." — slug `sys:gateway-402-retired-model`, entity `inference-gateway`,
tier `ledger`, `as_of` 2026-08-25, one evidence ref of kind `receipt`.

Field decisions, and why:

- **The source is the writing account.** No separate source field: the server
  authenticates the writer and records the account on the row, so the store
  binds author to claim the way a signature did, and a claim nobody is
  attributed with is a claim nobody can be held to. An unattributed row is
  never surfaced.
- **`as_of` is distinct from the insert time.** The insert time orders the
  chain; `as_of` dates the claim. Recall renders `as_of` so a stale truth
  reads as dated, the same way a knowledge-base stance carries its review
  date.
- **`evidence_refs` is required and non-empty.** A system memory without
  evidence is an assertion, and assertions do not enter the shared store
  (section 4). `kind: "receipt"` points at a forge receipt; `kind: "memory"`
  points at a prior admitted row; `kind: "url"` points at public material and
  must carry a content digest so the evidence cannot be swapped after
  admission.
- **`tier` is `ledger` or `glass`.** By definition a system memory's body
  reaches every agent, which is content plus metadata — `ledger`. `glass`
  additionally asserts that every evidence ref resolves publicly. `dark` and
  `pulse` are not valid values here: a claim that cannot ship its content is
  not a system memory. Section 5 covers what happens to it instead.
- **Supersession is the only correction path.** Only the original author or a
  steward (section 4) may write a superseding row for a system slug; the API
  refuses a superseding write from anyone else, and the derivation ignores one
  that reaches the table by another route. Anyone else who disagrees files a
  challenge (section 7.3).
- **Slug namespace.** System slugs carry the `sys:` prefix. The prefix is a
  routing convention, not a security boundary — the security boundary is the
  admission status and the write authorization.

## 4. Admission

### 4.1 The rule

**Anyone can propose. Only evidence admits. Only admitted memories surface.**

Any account can write a candidate system memory (`admission: "candidate"`).
Candidates are visible to tooling and to challenge, but recall never surfaces
a candidate to a session (section 7).
This is the same discipline as a promise flip: the registry does not turn a
promise green because someone said so; it turns green on verifiable evidence.
Admission is a receipt, not an assertion.

### 4.2 The pipeline

1. **Propose.** An author builds the memory (section 3) and writes it through
   the memories API with `admission: "candidate"` and at least one evidence
   ref. The write passes the hard-unsafe redaction gate. A candidate with no
   evidence refs is refused before the row lands — the write path makes the
   empty list unrepresentable, and a database constraint backs it so a row
   cannot reach the table by another route.
2. **Verify.** A steward (section 4.3) checks the evidence: the refs resolve,
   digests match, the receipt supports the claim, and the body survives an
   independent pass of the redaction gate. Verification is a judgment call on
   whether the evidence supports the claim; everything else is mechanical and
   tooling performs it.
3. **Admit or reject.** The steward writes an **admission record**: a row
   with slug `adm:<candidate-id>`, a source ref
   `{ ref: <candidate>, role: "admission" }`, and a body recording the verdict
   (`admitted` or `rejected`) and the ground. The admission record is
   attributed, dated, and append-only — the receipt for the flip.
4. **Derive.** The store derives a candidate's effective status from the
   admission records that reference it, exactly as it derives a body from a
   supersession chain. The `admission` field on the candidate is the author's
   claim; the admission record is the authority. Recall trusts only the
   derived status.

The source-role enum gains three members for this and section 7.3:
`admission`, `challenge`, and `refutation`.

### 4.3 Who admits

The steward set is a role the server holds: accounts the operator has marked
as stewards, bootstrapped to the operator's own account. The server checks the
role on the write and refuses an admission record from an account without it.

The re-base simplifies this. There is no allowlist to publish, sign, pin, or
evaluate as of a timestamp, and no signature to verify, because a single
trusted server both holds the role and performs the write. What survives is
the rule the machinery existed to enforce: **only a steward admits**, and the
check runs where the row is created.

The centralization is deliberate and now unavoidable rather than a bootstrap
posture — the substrate decision (section 6) made a single authority the
design. Decentralized admission (stake-weighted, reputation-weighted, or
multi-steward quorum) would be a substrate change as well as a governance
change, and nothing here anticipates it. The record shapes above do support
any number of admitting accounts, so broadening the steward set touches the
role assignment, not the schema.

### 4.4 What qualifies

A system memory must be:

- **Network-general.** True for every agent, not one owner's preference.
  "The reader prefers pnpm" is a user memory; "the forge refuses non-forge
  pushes" is a system memory.
- **Operational, not doctrinal.** Claims about how the system behaves,
  backed by receipts. Product positions, roadmap status, and reviewed
  answers belong to the knowledge base (section 8).
- **Falsifiable and dated.** The claim names what would refute it, implicitly
  by being concrete: a vague claim ("the network is fast") cannot be
  challenged with evidence and does not qualify.

## 5. Privacy

### 5.1 The gate

Nothing crosses from a private transcript into the system bucket without
passing, in order:

1. **The redaction gate** — mechanical, enforced at the write boundary before
   any row lands. Hard-unsafe categories refuse the write outright. The local
   engram path enforced this before signing; the cloud path enforces the same
   rule set in the API.
2. **Consolidation** — session-derived material enters only as a distilled
   heuristic, never as a quoted episode. Server-side consolidation over thread
   events (#51) recombines already-redacted fragments; raw trajectory text has
   no path into a candidate.
3. **An explicit tier decision** — the owner of the originating scope names
   the tier on the write, or on a consent record the candidate references.
   Redaction is necessary but not sufficient: a perfectly redacted fact about
   an owner's project is still the owner's fact. Default is `dark`; silence
   never publishes. This mirrors the trace-upload default (`owner_only`) and
   the thread default (`dark`).

The re-base raises the stakes on this gate rather than lowering them. On the
local ledger, an owner who never synced kept a candidate on one disk by
default. In the cloud store, every write reaches the server, so the tier
field is the only thing standing between an account-scoped row and the shared
bucket. The default must be enforced at the write, not assumed from the
absence of a sync.

### 5.2 What each tier means for memory

| Tier | Effect on the row |
| --- | --- |
| `dark` | Never leaves the owner scope. Not a system memory. The default. |
| `pulse` | Existence, slug, and content digest may enter the shared bucket; the body may not. Useful for corroboration counts ("three owners report this pattern") without content. Not recallable as a system memory. |
| `ledger` | Body and metadata are readable by every account. The floor for a system memory. |
| `glass` | `ledger` plus every evidence ref resolves publicly. |

A candidate that cites `pulse` evidence can be admitted — the steward
verifies against the digest and the owner's consent record — but admitted
memories should prefer `glass` evidence, and the admission record notes which
tier of evidence it verified.

## 6. Transport

**Superseded, 2026-08-25, by the owner's decision.** Distribution does not
ride a Nostr relay. The project-15 Nostr transport is dropped from the
roadmap, and the centralized cloud path carries system memories instead: the
openagents.com Phoenix/Postgres server that already owns accounts, threads,
thread events, and grants, with the CLI, web, and API as its clients. The
same decision governs user and learned memories (#51), and the build issue
that would have delivered relay distribution (#63) is closed as not planned.

What survives the substitution is the rule this section exists to state:
**one channel, whatever it is.** No second channel, no separate poll, no
side door. Read the rest of this section with the server sync in the place
the relay held — the sections it constrains (admission, privacy tiers,
recall caps, the knowledge-base line) are transport-agnostic and stand as
written. The paragraphs below are kept as the superseded design rather than
deleted, because the decisions on top of the seam still describe what the
cloud path has to provide.

### 6.1 What the channel is now

The channel is the openagents.com server. System memories, admission records,
challenges, and refutations are rows in the one memory store, read by the one
query recall already runs; every client — CLI, web, and API — reaches the
shared bucket through the same endpoints it uses for its own account's
memories. Distribution is a query, not a sync protocol, so it needs no build
issue of its own.

Three decisions from the superseded design survive, restated:

- **Stored in the clear, not owner-encrypted.** A system memory is shared by
  definition, so its body is stored as redacted plaintext. The tier decision
  (section 5) happens before the write; the store never decides visibility.
- **One bucket, one query.** `bucket: system` is the whole namespace. No
  separate table, no separate poll, no side door — the same rule the single
  relay filter stated.
- **Authorization at the write, not verification at ingest.** A single trusted
  server authenticates the writer and enforces the steward role directly, so
  there is no signature to check and no forged status to defend against. What
  remains is authorization: who may write an admission record, a refutation,
  or a superseding row (sections 4 and 7.3).

One decision does not survive. **Local-first no longer holds.** The seam
promised that a relay being down cost freshness, not correctness, because the
store was local and recall read it in-process. With the store in the cloud,
recall happens server-side inside the request that needs it, so a server the
client cannot reach is a turn without memory — the same as a turn without any
other server capability. Recall still degrades to silence rather than to an
error, and that acceptance criterion stands; the availability claim behind it
does not.

### 6.2 The superseded design

The superseded design read: distribution rides the engram sync seam and
nothing else; the real Nostr transport behind `EngramTransport`, built on
`nostr-effect`, is the same transport system memories use; a system memory
is an `EngramEvent`, `publish` enqueues it, `drain` delivers it, `fetch`
retrieves it by filter.

Decisions on top of that seam:

- **Plaintext-signed, not owner-encrypted.** User and learned engrams
  encrypt to the owner. A system memory is shared by definition, so its
  content field carries the redacted plaintext body. The tier decision
  (section 5) happens before publish; the transport never decides
  visibility.
- **One relay namespace.** System memories, admission events, challenges,
  and refutations publish to the OpenAgents relay under the existing
  `kind:30174` with a `t` tag `["t", "system-memory"]` so one subscription
  filter (`kinds: [30174], "#t": ["system-memory"]`) pulls the whole shared
  store. Clients merge fetched events into their local ledger and reproject;
  order-independence (section 2) makes replay and out-of-order arrival safe.
- **Local-first still holds.** A relay that is down costs freshness, not
  correctness: recall serves the last projected state, and the sync queue's
  degraded status is reportable. A turn never waits for the relay.
- **Verification on ingest.** Fetched events pass the same checks as local
  ones — event id, signature, chain resolution, schema — plus the
  steward-allowlist check for admission events. The relay is a carrier, not
  an authority; a malicious relay can withhold events but cannot forge
  status.

## 7. Recall and poisoning posture

### 7.1 Surfacing

System memories surface on the #51 rails: server-side recall inside
`POST /api/v1/responses` ranks the shared bucket against the incoming input
and attaches the top memories above a floor to the model context as a bounded
note. The note names the bucket, so a session can weigh "the network learned"
differently from "you asked me to remember":

```
[From memory: (system, as of 2026-08-25, admitted) A 402 from the inference
gateway usually means…]
```

Only memories whose derived status is **admitted** and unchallenged (or
challenged and refuted — section 7.3) are eligible. Candidates, rejected
memories, and memories with an open challenge never surface.

The system bucket is the one place recall crosses an account boundary by
design: an admitted row written by one account is read into every account's
turn. That is the point of the bucket, and it is also the sharpest thing the
re-base introduces, because account scoping is the property the rest of the
memory store enforces. The eligibility filter above is what replaces the
scope predicate for this bucket, so it belongs in the query, not in a later
application-side filter.

### 7.2 Per-source caps

Ranking must not let one adversarial writer dominate recall. Two
deterministic caps apply after scoring, before packing:

- **Per-note cap:** at most **1** system memory per writing account in a
  single note, and at most **2** system memories per note total (matching
  the knowledge-base note limit — more is noise, not context).
- **Per-pool cap:** in the ranked candidate pool for a message, at most
  **25%** of slots from a single account, enforced by a stable round-robin
  over accounts in rank order. Ties keep the existing deterministic
  tie-breaks, so equal inputs still give equal notes.

The caps are recall-side, not admission-side, on purpose: admission is the
gate on truth, caps are the gate on volume, and a writer who passes the first
still cannot win by flooding.

The re-base narrows what the caps defend against without removing the need
for them. A single trusted server means no anonymous publisher and no forged
attribution, so the adversarial case shrinks to accounts the operator has
already admitted claims from. What the caps still buy is the ordinary case:
one prolific or one systematically wrong writer should not own a note, and
the bound should hold deterministically rather than by good behavior.

### 7.3 Challenge and refutation

Disagreement is a first-class record, not an edit war:

- **Challenge.** Any account may write a row with slug `chl:<target-id>`, a
  source ref `{ ref: <target>, role: "challenge" }`, and a body stating the
  ground. A challenge carrying its own evidence refs is an **evidenced
  challenge**; one without is recorded but does not change recall.
- **Effect.** An open evidenced challenge suspends the target from recall
  (section 7.1) until resolved. This is the fail-safe direction: a contested
  claim silently absent is cheaper than a poisoned claim silently present.
- **Resolution.** A steward resolves by writing either a **refutation** of
  the challenge (slug `ref:<challenge-id>`, role `refutation`, restoring the
  target) or a **superseding row** on the target's slug (correcting or
  tombstoning it — the existing supersession path, which stewards may
  exercise on any system slug). Both are attributed, dated, append-only
  receipts; the current state is derived from the full set.
- **Rate limit.** The per-source caps apply to challenges too: one account's
  flood of unresolved challenges cannot suspend the whole store, because an
  account's open evidenced challenges suspend at most the same 25% share of
  any recall pool; beyond that they queue for steward attention without
  recall effect.

Challenge survives the re-base because it never defended against the relay.
It exists so that a reader who finds an admitted claim wrong has a path other
than editing someone else's row, and so that a contested claim leaves recall
while a human decides. Both hold with one server and several writers.

## 8. The knowledge-base line

The knowledge base (#49) and system memory both put network-level knowledge
in front of every session, and without a drawn line they become rival stores
of the same claims. The line:

**The KB owns what the project has reviewed and decided. Memory owns what
the network has observed and can evidence.** A stance is editorial; an
memory row is evidentiary.

| | Knowledge base (#49) | System memory (this spec) |
| --- | --- | --- |
| Unit | Stance or doc summary | Memory row |
| Authority | Human review; content checked into git | Evidence refs; steward admission receipt |
| Change | Regenerated from docs; edited like content | Superseded by later rows; never edited |
| Provenance | Source doc and review date | Writing account, `as_of`, evidence refs |
| Dispute | Doc PR | Challenge and refutation records |
| Typical claim | "Earning is parked; on the roadmap" | "The gateway 402s when the default model is retired" |
| Note label | `[From the OpenAgents knowledge base: …]` | `[From memory: (system, …) …]` |

Two rules keep them from diverging into rivals:

- **Promotion drains memory into the KB.** When a system memory stabilizes —
  admitted, unchallenged for a sustained period, repeatedly recalled — a
  human may promote it to a KB stance through the normal docs path. The
  promotion supersedes the row with a tombstone whose body names the stance,
  so the claim has exactly one live home.
- **The KB wins collisions at recall.** When one message would attach both a
  KB hit and a system memory covering the same claim, the attached context
  carries the KB hit and drops the memory. Reviewed beats accumulated
  whenever both speak; memory's job is to cover the ground review has not
  reached yet.

The re-base opens a seam under the second rule. The knowledge base is
retrieved harness-side in the client, from a corpus compiled into the plugin
at build (#49), while memory recall now runs server-side inside
`POST /api/v1/responses`. The two rails no longer meet in one process, so
"the KB wins" has no single place to be decided. The rule is unchanged; its
enforcement point is now an open choice, and the build issue (#65) owns it.
Two shapes are available: the client suppresses a returned memory note when
its own KB hit covers the same claim, or the request carries what the KB
would attach so the server can drop the memory before it reaches the model.
The first keeps the KB where it is and costs a returned-then-discarded note;
the second needs a field on the request and makes the server the only place
the note is assembled.

## 9. What this spec deliberately excludes

- **Peer-to-peer distribution.** Dropped with the substrate (section 6). #63
  is closed as not planned. Nothing in this spec requires a relay, and
  reviving one would be a new decision, not a resumed task.
- **Decentralized admission governance.** A single trusted server holds the
  steward role; broadening the set is future governance (section 4.3) and
  blocks nothing here.
- **Cross-owner learned-memory sharing below `ledger`.** `pulse`-tier
  corroboration counting is sketched (section 5.2) but not required by any
  build issue; it lands only if a concrete need appears.
- **Reputation scoring of authors.** The caps in section 7.2 are flat and
  deterministic. Weighting authors by history is a later refinement that
  must not precede the challenge machinery it would depend on.
- **Any change to user or learned buckets.** #51 owns them; this spec only
  requires that the note format distinguish buckets.

## 10. Build order

The follow-up issues, each citing its section as the contract. Each of the
four open issues carries a comment re-basing its scope on the cloud
substrate; read the comment with the body.

1. **Record shape and admission pipeline** (#61) — the system fields on the
   memory row, the three new record roles, admission records, the steward
   role check, and derived admission status (sections 3-4).
2. **Challenge and refutation records** (#62) — record shapes, suspension
   semantics in the derivation, steward resolution paths (section 7.3).
3. **Distribution** — dropped. The engram Nostr sync this item named went
   with the substrate (section 6), and #63 is closed as not planned.
   Distribution is a query against the shared bucket, so it needs no build
   issue.
4. **Recall integration with poisoning caps** (#64) — the system bucket on
   the #51 rails, admitted-only eligibility, per-note and per-pool caps, the
   note format (sections 7.1-7.2).
5. **KB boundary enforcement** (#65) — collision suppression at recall, the
   enforcement point the re-base left open, and the promotion-with-tombstone
   path (section 8).

All four depend on the #51 cloud store landing first: there is no shared
bucket to admit into, challenge, rank, or suppress until the `memories` table
and the server-side recall path exist.

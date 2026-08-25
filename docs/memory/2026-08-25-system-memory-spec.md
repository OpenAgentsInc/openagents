# System memory: specification

- Class: design specification
- Date: 2026-08-25
- Status: accepted design for the system bucket; no code lands under the
  owning issue
- Owning issue: OpenAgentsInc/openagents#52
- Base: the engram model and its live wiring in
  `packages/openagents-cli/src/memory/` (vendored from
  `packages/agent-experience-memory`), the user and learned buckets in #51,
  the knowledge base in #49, and the engram sync seam whose real Nostr
  transport is the remaining project-15 piece
- Vocabulary: transparency tiers reuse the forge's `dark/pulse/ledger/glass`
  names (`openagents.com` `lib/openagents/transparency.ex`)

## 1. What a system memory is

The memory system has two buckets today, both owner-local (#51):

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

The one-sentence design: **a system memory is an ordinary signed engram that
has passed an evidence-backed admission gate, carries a transparency tier of
`ledger` or above, distributes on the existing engram sync seam, and can be
challenged and superseded by further signed events — never edited.**

## 2. What stays unchanged

The system bucket adds no new event kind, no new store, and no new channel.
It reuses, unmodified:

- **The engram event model** (`memory/engram.ts`): NIP-AE `kind:30174`
  addressable events, the OpenAgents companion body, the hard-unsafe
  redaction gate before signing, content digests, and the
  `supersedes` correction path. A correction appends; it never edits.
- **The projection** (`memory/projection.ts`): derived, idempotent,
  order-independent; unresolved supersession chains are refused whole.
- **The ranking module** (`memory/ranking.ts`): deterministic cosine top-K,
  salience recall, token-budget packing.
- **The sync seam** (`memory/sync.ts`): `EngramTransport`,
  `EngramSyncQueue`, local-first, never blocks a turn.
- **The transparency vocabulary**: `dark` (nothing public), `pulse`
  (metadata only), `ledger` (content and metadata), `glass` (full access).

What the bucket adds is one companion extension (section 3), three event
roles (admission, challenge, refutation — sections 4 and 5), one relay
namespace on the coming Nostr transport (section 6), and recall caps
(section 7).

## 3. Event shape

A system memory is an engram whose companion body carries a `system` block.
The companion schema id bumps to name the extension; events without the block
are user- or learned-bucket engrams and nothing about them changes.

```jsonc
// content of a kind:30174 event, after redaction, before signing
{
  "slug": "sys:gateway-402-retired-model",
  "value": "A 402 from the inference gateway usually means the default model was retired upstream. Check gateway status before bisecting local lanes.",
  "openagents": {
    "schema": "openagents.agent_experience_memory.nip_ae_companion.v2",
    "admission": "candidate",          // existing field; see section 4
    "entityId": "inference-gateway",
    "contentDigest": "sha256:…",
    "sourceEventRefs": [
      { "eventId": "…64 hex…", "role": "tool_result" }
    ],
    "relations": [],
    "derivedFromSlugs": [],
    "supersedes": "…64 hex…",          // optional, existing path
    "system": {
      "schema": "openagents.system_memory.v1",
      "tier": "ledger",                 // "ledger" | "glass"; never lower
      "asOf": "2026-08-25",             // the date the claim was observed true
      "evidenceRefs": [                 // at least one, required
        {
          "kind": "receipt",            // "receipt" | "engram" | "url"
          "ref": "https://openagents.com/receipts/…",
          "digest": "sha256:…"          // required for kind "url"
        }
      ]
    }
  }
}
```

Field decisions, and why:

- **`source` is the event's own `pubkey`.** No separate source field: the
  signature already binds author to claim, and a claim nobody signed is a
  claim nobody can be held to. Unsigned or unverifiable events are never
  projected (existing behavior).
- **`asOf` is distinct from `created_at`.** `created_at` orders the chain;
  `asOf` dates the claim. Recall renders `asOf` so a stale truth reads as
  dated, the same way a knowledge-base stance carries its review date.
- **`evidenceRefs` is required and non-empty.** A system memory without
  evidence is an assertion, and assertions do not enter the shared store
  (section 4). `kind: "receipt"` points at a forge receipt or signed event;
  `kind: "engram"` points at a prior admitted engram; `kind: "url"` points at
  public material and must carry a content digest so the evidence cannot be
  swapped after admission.
- **`tier` is `ledger` or `glass`.** By definition a system memory's value
  reaches every agent, which is content-plus-metadata — `ledger`. `glass`
  additionally asserts that every evidence ref resolves publicly. `dark` and
  `pulse` are not valid values here: a claim that cannot ship its content is
  not a system memory (section 5 covers what happens to it instead).
- **`supersedes` is the only correction path.** Reused as-is from the engram
  model. Only the original author or a steward (section 4) may sign a
  superseding event for a system slug; the projection refuses a superseding
  event signed by anyone else. Anyone else who disagrees files a challenge
  (section 5.2).
- **Slug namespace.** System slugs carry the `sys:` prefix. The prefix is a
  routing convention, not a security boundary — the security boundary is the
  admission status and the signature.

## 4. Admission

### 4.1 The rule

**Anyone can propose. Only evidence admits. Only admitted memories surface.**

Any network identity can sign and publish a candidate system memory
(`admission: "candidate"`). Candidates are visible to tooling and to
challenge, but recall never surfaces a candidate to a session (section 7).
This is the same discipline as a promise flip: the registry does not turn a
promise green because someone said so; it turns green on verifiable evidence.
Admission is a receipt, not an assertion.

### 4.2 The pipeline

1. **Propose.** An author builds the engram (section 3), passes the
   hard-unsafe redaction gate, signs it, and publishes it through the sync
   queue with `admission: "candidate"` and at least one evidence ref.
   A candidate with no evidence refs is refused at build time, before
   signing — the schema makes the empty list unrepresentable.
2. **Verify.** A steward (section 4.3) checks the evidence: the refs resolve,
   digests match, the receipt supports the claim, and the value survives an
   independent pass of the redaction gate. Verification is a judgment call on
   whether the evidence supports the claim; everything else is mechanical and
   tooling performs it.
3. **Admit or reject.** The steward signs an **admission event**: an engram
   with slug `adm:<candidate-event-id>`, whose `sourceEventRefs` carry
   `{ eventId: <candidate>, role: "admission" }` and whose value records the
   verdict (`admitted` or `rejected`) and the ground. The admission event is
   itself signed, dated, published, and permanent — the receipt for the flip.
4. **Project.** The projection derives a candidate's effective status from
   the admission events that reference it, exactly as it derives a value from
   a supersession chain. The `admission` field on the candidate is the
   author's claim; the admission event is the authority. Recall trusts only
   the projected status.

The `EngramSourceRole` literal set gains three members for this and section
5: `"admission"`, `"challenge"`, `"refutation"`.

### 4.3 Who admits

The steward set is a published allowlist of pubkeys served by the forge
(`openagents.com`), bootstrapped to the operator's key. The list itself is
signed and dated; clients pin it and refuse admission events from keys not on
the list at the admission event's `created_at`.

This is deliberately centralized to start. Decentralized admission
(stake-weighted, reputation-weighted, or multi-steward quorum) is a
governance change, not a schema change: the event shapes above already
support any number of admitting keys, so broadening the set later touches the
allowlist, not the protocol. The spec records the centralization as a known
bootstrap posture, not a hidden assumption.

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

1. **The redaction gate** (`guardEngramContent`) — mechanical, already
   enforced before any engram is signed. Hard-unsafe categories block
   signing outright.
2. **Consolidation** — session-derived material enters only as a distilled
   heuristic (the existing dream pass), never as a quoted episode. The
   synthesizer recombines already-redacted fragments; raw trajectory text has
   no path into a candidate.
3. **An explicit tier decision** — the owner of the originating scope signs
   the candidate (or a consent engram the candidate references) that names
   the tier. Redaction is necessary but not sufficient: a perfectly redacted
   fact about an owner's project is still the owner's fact. Default is
   `dark`; silence never publishes. This mirrors the trace-upload default
   (`owner_only`) and the thread default (`dark`).

### 5.2 What each tier means for memory

| Tier | Effect on the engram |
| --- | --- |
| `dark` | Never leaves the owner scope. Not a system memory. The default. |
| `pulse` | Existence, slug, and content digest may sync; the value may not. Useful for corroboration counts ("three owners report this pattern") without content. Not recallable as a system memory. |
| `ledger` | Value and metadata distribute. The floor for a system memory. |
| `glass` | `ledger` plus every evidence ref resolves publicly. |

A candidate that cites `pulse` evidence can be admitted — the steward
verifies against the digest and the owner's consent engram — but admitted
memories should prefer `glass` evidence, and the admission event records
which tier of evidence it verified.

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

System memories surface on the #51 rails: the per-prompt harness retrieval
ranks the projected store against the incoming message and attaches the top
memories above a floor as a bounded note. The note names the bucket, so a
session can weigh "the network learned" differently from "you asked me to
remember":

```
[From memory: (system, as of 2026-08-25, admitted) A 402 from the inference
gateway usually means…]
```

Only memories whose projected status is **admitted** and unchallenged (or
challenged and refuted — section 7.3) are eligible. Candidates, rejected
memories, and memories with an open challenge never surface.

### 7.2 Per-source caps

Ranking must not let one adversarial writer dominate recall. Two
deterministic caps apply after scoring, before packing:

- **Per-note cap:** at most **1** system memory per author pubkey in a
  single note, and at most **2** system memories per note total (matching
  the knowledge-base note limit — more is noise, not context).
- **Per-pool cap:** in the ranked candidate pool for a message, at most
  **25%** of slots from a single author, enforced by a stable round-robin
  over authors in rank order. Ties keep the existing deterministic
  tie-breaks, so equal inputs still give equal notes.

The caps are recall-side, not admission-side, on purpose: admission is the
gate on truth, caps are the gate on volume, and an attacker who passes the
first still cannot win by flooding.

### 7.3 Challenge and refutation

Disagreement is a first-class event, not an edit war:

- **Challenge.** Anyone may sign an engram with slug
  `chl:<target-event-id>`, a `sourceEventRefs` entry
  `{ eventId: <target>, role: "challenge" }`, and a value stating the ground.
  A challenge carrying its own `evidenceRefs` is an **evidenced challenge**;
  one without is recorded but does not change recall.
- **Effect.** An open evidenced challenge suspends the target from recall
  (section 7.1) until resolved. This is the fail-safe direction: a contested
  claim silently absent is cheaper than a poisoned claim silently present.
- **Resolution.** A steward resolves by signing either a **refutation** of
  the challenge (slug `ref:<challenge-event-id>`, role `"refutation"`,
  restoring the target) or a **superseding event** on the target's slug
  (correcting or tombstoning it — the existing supersession path, which
  stewards may exercise on any system slug). Both are signed, dated,
  permanent receipts; the projection derives the current state from the
  full set.
- **Rate limit.** The per-source caps apply to challenges too: one author's
  flood of unresolved challenges cannot suspend the whole store, because an
  author's open evidenced challenges suspend at most the same 25% share of
  any recall pool; beyond that they queue for steward attention without
  recall effect.

## 8. The knowledge-base line

The knowledge base (#49) and system memory both put network-level knowledge
in front of every session, and without a drawn line they become rival stores
of the same claims. The line:

**The KB owns what the project has reviewed and decided. Memory owns what
the network has observed and can evidence.** A stance is editorial; an
engram is evidentiary.

| | Knowledge base (#49) | System memory (this spec) |
| --- | --- | --- |
| Unit | Stance or doc summary | Signed engram |
| Authority | Human review; content checked into git | Evidence refs; steward admission receipt |
| Change | Regenerated from docs; edited like content | Superseded by signed events; never edited |
| Provenance | Source doc and review date | Author pubkey, `asOf`, evidence refs |
| Dispute | Doc PR | Challenge and refutation events |
| Typical claim | "Earning is parked; on the roadmap" | "The gateway 402s when the default model is retired" |
| Note label | `[From the OpenAgents knowledge base: …]` | `[From memory: (system, …) …]` |

Two rules keep them from diverging into rivals:

- **Promotion drains memory into the KB.** When a system memory stabilizes —
  admitted, unchallenged for a sustained period, repeatedly recalled — a
  human may promote it to a KB stance through the normal docs path. The
  promotion supersedes the engram with a tombstone whose value names the
  stance, so the claim has exactly one live home.
- **The KB wins collisions at recall.** When one message would attach both a
  KB hit and a system memory covering the same claim, the harness attaches
  the KB hit and drops the memory. Reviewed beats accumulated whenever both
  speak; memory's job is to cover the ground review has not reached yet.

## 9. What this spec deliberately excludes

- **Decentralized admission governance.** The steward allowlist is the
  bootstrap; broadening it is future governance (section 4.3) and blocks
  nothing here.
- **Cross-owner learned-memory sharing below `ledger`.** `pulse`-tier
  corroboration counting is sketched (section 5.2) but not required by any
  build issue; it lands only if a concrete need appears.
- **Reputation scoring of authors.** The caps in section 7.2 are flat and
  deterministic. Weighting authors by history is a later refinement that
  must not precede the challenge machinery it would depend on.
- **Any change to user or learned buckets.** #51 owns them; this spec only
  requires that the note format distinguish buckets.

## 10. Build order

The follow-up issues, each citing its section as the contract:

1. **Event shape and admission pipeline** — companion `v2` with the `system`
   block, the three new source roles, admission events, steward-allowlist
   verification, projection of admission status (sections 3-4).
2. **Challenge and refutation events** — event shapes, suspension semantics
   in the projection, steward resolution paths (section 7.3).
3. **Distribution on the engram Nostr sync** — the `system-memory` relay
   namespace on the real transport, ingest verification, merge-and-reproject
   (section 6). Depends on the project-15 transport landing; adds no second
   channel.
4. **Recall integration with poisoning caps** — the system bucket on the #51
   rails, admitted-only eligibility, per-note and per-pool caps, the note
   format (sections 7.1-7.2).
5. **KB boundary enforcement** — collision suppression at recall and the
   promotion-with-tombstone path (section 8).

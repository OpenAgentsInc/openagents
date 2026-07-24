# Sarah memory on Nostr: audit and recommendation

- Class: audit and recommendation
- Date: 2026-07-24
- Status: recommendation accepted by the owner on 2026-07-24
- Question: where do Sarah's memories live after the Nostr cutover
- Owning packet: `SARAH-NR-07` (openagents `#9221`)
- Program: `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` Part 2
- OpenAgents pin: `7ad1508f30`
- `nostr-effect` pin: `787f7b5`
- Buzz pin: `v0.4.24`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

## 1. The question

The owner directed that Sarah's runtime moves entirely to Nostr on a relay
that OpenAgents controls. Her conversation, her turn ladder, and her authority
receipts have named carriers in that plan. Her memory does not.

This document answers three things. Where do the memories live. Are they Nostr
events. Do we write a new NIP, adapt a Buzz custom NIP, or use a standard one.

## 2. Recommendation

Adopt **NIP-AE** as the durable memory record, unchanged on the wire, and add
an OpenAgents companion profile for the fields it deliberately leaves out.
Keep the graph as a **derived, rebuildable index** that never becomes
authority.

Three sentences state the whole design.

1. A memory is a NIP-AE engram: an addressable `kind:30174` event, signed by
   Sarah, encrypted to the owner under the symmetric conversation key.
2. The graph and its embeddings are a projection over that engram stream, and
   they must be recomputable from it.
3. If the projection and the engrams disagree, the engrams win.

Do not write a new kind. Do not fork NIP-AE. Section 6 gives the reasoning and
section 7 gives the exception where a companion profile is required.

## 3. What Sarah's memory is today

### 3.1 The modules

| Path | Role |
| --- | --- |
| `apps/openagents.com/workers/api/src/sarah-graph-memory.ts` | recall into a Sarah turn |
| `apps/openagents.com/workers/api/src/sarah-graph-memory-writeback.ts` | extract and persist facts after a turn |
| `apps/openagents.com/workers/api/src/sarah-graph-memory-store.ts` | the Cloud SQL backing store |
| `packages/agent-experience-memory` | the portable SDK that owns every memory semantic |
| `packages/graph-corpus` | corpus build, digests, derivation, deletion inventory |

### 3.2 The properties that already hold

The current implementation is careful, and the Nostr move must not lose these.

- **Default off.** Both recall and write-back sit behind
  `SARAH_GRAPH_MEMORY_RECALL_ENABLED`. With the flag off no store is opened
  and nothing is extracted, so a turn is byte-identical to a build without the
  module.
- **Owner-scoped.** The scope derives from a one-way digest of the owner user
  id. One owner cannot write into another owner's memory, and a prior graph
  folds in only when its stored binding scope matches.
- **Redacted at both boundaries.** Every fact passes `guardMemoryText` at
  storage time, and every recalled candidate passes the same guard again. A
  candidate carrying secret, credential, token, private-path, or wallet
  material is dropped rather than scrubbed into use.
- **Deterministic and no-spend.** Extraction is a deterministic redacted parse
  over the two confirmed messages. There is no model call and no provider
  spend in the write-back path.
- **Bounded.** At most four items per recall, each summary capped at 320
  characters.
- **Fail-soft.** Any recall failure yields an empty slice. Memory is never a
  hard dependency of a turn.
- **Linearizable.** The Cloud SQL adapter holds one atomically replaced
  envelope per scope. Its compare-and-set is one parameterized statement, so
  the SDK retry loop is safe across concurrent Cloud Run instances.

### 3.3 The honest status

The backing layer defaults to the SDK's disabled adapter, whose `inspect`
returns no current state. Even with the flag on, there is no recall until a
real backing store is supplied at the composition root.

So Sarah's memory is **built and not switched on**. That matters for the
migration. There is little production memory to move, and this is the cheapest
moment in the program to change where memory lives.

## 4. What the cognee audit already decided

`docs/teardowns/2026-07-21-cognee-teardown.md` audited the memory engine that
this SDK descends from. Its decision was path (b): do not run cognee, port its
ideas.

Two of its adopt-list items decide this document.

- "The graph-memory layer as a **derived, rebuildable index over the durable
  event log**."
- "Provenance-aware deletion planning across derived artifacts."

The first is the load-bearing one. The cognee audit already said the graph is
an index, not the record. Nostr supplies the durable event log that sentence
assumes. The two audits agree, and the recommendation in §2 is what they agree
on.

Its reject list also binds here. It rejects background promotion of session
content into durable memory without an evidence gate. It also rejects dataset
ACLs as the tenancy model.

## 5. What NIP-AE gives us

NIP-AE is a Buzz custom NIP. `nostr-effect` implements it in full at
`src/client/EngramService.ts`.

| Property | Mechanism |
| --- | --- |
| Kind | addressable `kind:30174` |
| Scope | one memory per `(agent pubkey, owner pubkey)` pair |
| Encryption | NIP-44 v2 under the conversation key `K_c` |
| Owner readability | `K_c` is symmetric, so the owner can always decrypt |
| Addressing | `d` = HMAC-SHA256 of the slug under `K_c`, so slugs never leak |
| Record types | one `core` body, and many `mem/...` bodies |
| Deletion | in-band tombstone with `"value": null`, plus optional NIP-09 |
| Ordering | monotonic `created_at` of `max(now, head + 1)` |
| Unknown viewers | NIP-31 `alt` text that leaks nothing |
| Size | 65535 bytes of plaintext per record |

Two properties deserve emphasis.

**Owner-decryptable by construction.** The Buzz teardown called this "the
sharpest single idea in the repo", and it is a genuine improvement on today's
posture. Right now the memory envelope sits in Cloud SQL where an operator
with database access can read it. Under NIP-AE only the owner key and Sarah's
key can decrypt. Auditability stops being a policy and becomes a property.

**Blinded addressing.** The `d` tag is an HMAC of the slug under the
conversation key. A relay operator sees that Sarah wrote a memory. It does not
see which memory, and it cannot enumerate her topics.

## 6. Why not a new NIP

A new kind is the obvious instinct and it is the wrong one here.

1. **The envelope is not the hard part.** The hard parts are the graph
   projection, provenance, and deletion planning. None of those change if we
   invent a kind. We would rewrite blinded addressing, head selection,
   monotonic writes, tombstones, and the NIP-44 boundary, and land in the same
   place with less review.
2. **NIP-AE is designed to be extended.** Its Bodies section names the
   taxonomies it leaves out. They are provenance, trust levels, attention and
   working sets, and structured links. It says they "belong in companion NIPs
   that add fields under the unknown-fields-permissive rule". Readers must
   ignore unknown fields. That is an explicit extension point.
3. **The parity direction forbids gratuitous kinds.** The Buzz parity
   recommendation says to prefer a standard NIP where one expresses the
   behavior. It also says to keep every extension in a written specification
   with fixtures, and not to repeat opaque extension growth.
4. **It is already implemented and tested in owned code.** `EngramService`
   carries the kind, the domain separator, the slug grammar, the plaintext
   cap, the clock-poison threshold, and the body types.
5. **Interoperability is free.** Another NIP-AE client can read the envelope
   and show the owner their own memory without knowing anything about
   OpenAgents.

The reject case would be a genuine incompatibility. There is none. NIP-AE is
missing fields, and missing fields are what its extension rule is for.

## 7. Where NIP-AE is not enough

A memory body is `{ slug, value }` where `value` is a string. Sarah's memory is
a graph with provenance, derivation, ranking, and a deletion inventory. A flat
string map loses all of it.

So the companion profile must carry these, as additional body fields under the
unknown-fields rule.

| Need | Why NIP-AE alone fails | Companion field |
| --- | --- | --- |
| Provenance | no source, no derivation, no content hash | source event refs and a content digest |
| Deterministic identity | slugs are author-chosen strings | a derived identity for extracted entities |
| Graph edges | wiki links are non-normative and untyped | typed relation entries |
| Ranking | no weight fields, and weights must not affect identity | ranking held **outside** the record |
| Evidence gate | no notion of admitted versus candidate | an explicit admission state |
| Deletion planning | tombstones are per slug | a derivation index so a delete plans across artifacts |

Two design rules keep the companion honest.

**Ranking never enters the record.** The cognee model puts `feedback_weight`
on the datapoint but excludes it from identity and embedding. Go further here:
keep ranking entirely in the derived projection. A signed durable memory should
not change bytes because a score moved.

**Wiki links are not the graph.** NIP-AE's `[[slug]]` convention is explicitly
non-normative and matched by literal substring. Use it for human-readable
cross-reference only. Typed edges belong in companion fields with their own
fixtures.

## 8. The resulting architecture

```text
Durable record (authority)          Derived index (rebuildable)
+---------------------------+      +------------------------------+
| NIP-AE kind 30174 engrams |      | graph elements and relations |
| on relay.openagents.com   | ---> | embeddings and vector plane  |
| NIP-44 to the owner       |      | ranking and feedback weights |
| blinded d tags            |      | deletion derivation index    |
+---------------------------+      +------------------------------+
        ^                                        |
        |                                        v
   Sarah writes after a turn              recall into a turn,
   the owner can always read              bounded and redacted
```

- The relay holds the memory. The projection holds the speed.
- The projection may live in Cloud SQL or locally. It is a cache with a
  rebuild path, never a second authority.
- A projection rebuild reads engrams and produces the same graph. If it
  cannot, the projection is wrong and gets discarded.

### 8.1 What must not move to Nostr

The Part 2 boundary applies unchanged. Exact metering, admission authority,
target broker execution, secret custody, and Git object safety stay where they
are. Memory adds one item: **ranking state stays in the projection.**

### 8.2 The index lives in Google Cloud (owner decision, 2026-07-24)

The owner decided that Sarah's index lives in OpenAgents Google Cloud. It is
not local-only. This section states what that buys and what it costs, because
the cost is real and must not be described away.

**What it buys.** One index serves every client. Recall works the same on
desktop and on mobile, and a new device inherits memory without rebuilding it.
A local-only index cannot do that.

**What it costs.** Building a graph and vector index requires decrypted
content, so OpenAgents Google Cloud holds derived memory material at rest. The
relay operator still cannot read the record. The index host can read what the
index contains.

**What does not change.** Sarah's turn service already holds her key, because
it signs and encrypts as Sarah. So the index does not grant a new capability.
The change is that decrypted derivatives are now persisted rather than
transient.

**Required mitigations.** These are not optional, because they are what keeps
the decision honest.

1. The index stores only redacted, bounded derivatives. `guardMemoryText`
   applies before anything is indexed, exactly as it does at write time.
2. The index is owner-scoped by the existing one-way digest, and is encrypted
   at rest under Google Cloud defaults.
3. The index is derived. It is rebuildable from engrams and is destroyed and
   rebuilt rather than repaired.
4. The owner can list and delete the index for their scope, and deletion
   removes derived rows without touching the engrams.
5. The product must never say the index is unreadable by OpenAgents. The
   accurate claim is that the relay operator cannot read the record.

Client-side search over locally decrypted engrams stays available and stays
the stronger privacy path for a user who wants it. The hosted index is the
default, not the only option.

### 8.3 Listing is best-effort

NIP-AE says so plainly: Nostr has no protocol-level pagination, relays may cap
results, and a capped result set silently under-reports.

For a memory system this is a correctness issue, not a performance note. A
recall that silently sees half the memory is worse than one that fails. The
implementation must surface per-relay counts or a limit-reached signal and
treat a truncated listing as a gap, not as an empty set.

## 9. Key loss is history loss

Encrypted memory cannot be repaired by an operator. If the owner key is lost,
the memory is gone, and no database restore recovers it.

This raises the stakes on `SARAH-NR-04`. Custody, rotation, and archival are
first-class requirements for memory, not later items. NIP-IA identity archival
exists in `nostr-effect` for the rotation case and should be exercised before
memory carries anything the owner would miss.

Recommend an owner-controlled export as part of the first slice. An export
that a second implementation can verify offline is the honest backup story.
It is also the same export the parity plan already asks for.

## 10. Migration

There is very little to move, because §3.3 shows the store is not switched on.
That is the argument for doing this now rather than after the flag flips.

1. Land the companion profile and its fixtures before any write.
2. Write engrams to the relay while the flag stays off. Nothing recalls yet.
3. Build the projection from the engram stream and prove a rebuild reproduces
   it exactly.
4. Point recall at the projection, still bounded, still redacted, still
   fail-soft, still default off.
5. Retire the Cloud SQL envelope only after an export and a rebuild both pass.

Do not run a dual-write period longer than the rebuild proof needs. Two
writable memory homes is the hidden second log the parity plan rejects.

## 11. Falsifiers

The design is wrong if any of these becomes true.

1. The graph projection outranks the engrams it came from.
2. A memory exists that no engram produced.
3. A projection cannot be rebuilt from the engram stream.
4. Ranking state changes the bytes of a durable record.
5. The relay operator can read memory content.
6. Decrypted memory content enters a server-side index.
7. A truncated listing is treated as an empty set.
8. Memory becomes a hard dependency of a turn, or stops being default off.
9. A new custom kind ships for something NIP-AE fields already express.
10. A companion field ships without a written specification and fixtures.

## 12. Owner decisions (2026-07-24)

All four are answered.

| Question | Decision |
| --- | --- |
| Adopt NIP-AE, or write a new kind | **Adopt NIP-AE.** Start with the engram envelope unchanged |
| Where the derived index lives | **OpenAgents Google Cloud.** See §8.2 for the cost and its mitigations |
| Export format | Delegated. Recorded in §12.1 |
| Tombstone retention | Delegated. Recorded in §12.2 |

### 12.1 Export format

`openagents.sarah_memory_export.v1`. Two parts in one archive.

**Events.** Newline-delimited JSON of the raw signed engram events, byte for
byte as published. Not a re-serialization. A reader must be able to verify
every signature with any NIP-01 implementation and no OpenAgents code.

**Manifest.** The agent public key, the owner public key, and the event count.
Then the ordered list of event identifiers and the export time. Then a SHA-256
over the canonical concatenation of the event lines.

Rules:

- Include tombstones. An export that hides deletions is not an audit trail.
- Exclude the derived index. It is rebuildable, and shipping it would ship
  decrypted content into a file the owner may move anywhere.
- The archive is encrypted at rest to the owner. The plaintext engrams stay
  encrypted inside it, so the export is exactly as private as the record.
- Verification is offline. That is the whole point, and it is also the backup
  story from §9.

### 12.2 Tombstone retention

Retain tombstones indefinitely on the owned relay. Never garbage-collect one.

A tombstone is the only durable proof that a deletion was intentional. If it
expires, an observer cannot tell a deleted memory from a memory that never
existed. The owner then loses the ability to audit their own deletions.

Publish both signals. The in-band tombstone with `"value": null` is the
protocol semantic that readers act on. A NIP-09 deletion request goes to
honoring relays.

Do not depend on NIP-09 being honored. Relays honor it by policy, not by
protocol, so honoring and non-honoring relays diverge on pre-deletion history.
The in-band tombstone is what makes the outcome deterministic on our relay.

The derived index drops a tombstoned slug on the next rebuild. It does not
retain a shadow copy.

## 13. Research basis

This audit read the Sarah memory modules and their tests at OpenAgents pin
`7ad1508f30`. It read the cognee teardown of 2026-07-21, including its adopt
and reject lists, and the workspace audit that teardown supersedes.

It read the Buzz NIP-AE specification at `v0.4.24` in full, and the
`nostr-effect` implementation at `787f7b5`.

No relay was deployed, no engram was published, and no memory was migrated for
this document.

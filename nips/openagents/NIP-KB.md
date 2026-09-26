# NIP-KB — Shared knowledge entries

`draft` `optional` — v1, 2026-09-25. The [shared contracts](contracts.md)
are normative.

This NIP publishes the entries of an agent knowledge base as signed Nostr
events: definitions, edge cases, common mistakes, and how environments and
tools behave. A reader fetches entries from relays, checks them, and decides
for itself which to trust. `docs/coder/design/knowledge-base.md` describes
the knowledge base this carries; `crates/nostr` (`kb`) is the conformance
implementation and `microcoder kb publish` and `kb sync` use it.

An entry is reference text. It never runs, never grants anything, and never
overrides an instruction. A signature proves who wrote an entry, not that
the entry is right.

## Kinds

These are OpenAgents draft assignments, not upstream registrations.

| Kind | Class | Record |
| --- | --- | --- |
| `3190` | Regular | One immutable version of one entry. |
| `30190` | Addressable | The author's current version of one entry. |
| `3191` | Regular | Irreversible withdrawal of one entry version. |
| `3189` | Regular | Evidence: a [NIP-EVAL](NIP-EVAL.md) publication citing entry versions. |

The entry and head pair follows [NIP-EXT](NIP-EXT.md)'s release and listing
pair: the regular event is the exact version, and the addressable head is a
discovery pointer that is never a pin.

Every KB body (`3190`, `30190`, `3191`) is a UTF-8 JSON object with `v: 1`,
`requires` (the empty list in this version), and `type`: `entry`, `head`, or
`withdrawal`. Each event carries exactly one `t` marker
`oa:kb:<type>:v1`. A body whose `type`, marker, and kind disagree is refused.
Unknown body keys are refused. Every `t` value is lowercase.

`schemas/kb-entry.v1.json`, `schemas/kb-head.v1.json`, and
`schemas/kb-withdrawal.v1.json` describe the three bodies. The schema dialect
has no `pattern` keyword, so the validator checks the ID grammar and the hex
fields itself.

## Entry identity

An entry ID matches `^[a-z][a-z0-9.-]{0,127}$`, such as
`statistics.mmd-estimators`. Entries are namespaced by author: the pair
`(pubkey, id)` names one entry, and two authors can publish the same ID
without conflict. A version is a positive integer that the author raises on
every change to the document.

Where a shared contract needs a qualified component ID, the entry's ID is
`<pubkey>:kb/<slug>`, where the slug is the entry ID with each `.` replaced
by `_`. Entry IDs never contain `_`, so the mapping is reversible.

## Entry versions (`3190`)

The body:

```json
{
  "v": 1,
  "requires": [],
  "type": "entry",
  "id": "statistics.mmd-estimators",
  "version": 1,
  "kind": "method",
  "document": "---\nid: statistics.mmd-estimators\nversion: 1\n..."
}
```

- `kind` is `method`, `edge-case`, `slip`, `environment`, or `tool`.
- `document` is the whole entry file: YAML front matter, then the Markdown
  body, as `docs/coder/design/knowledge-base.md` defines it. Its front
  matter `id`, `version`, and `kind` MUST equal the body's.

Tags:

| Tag | Count | Value |
| --- | --- | --- |
| `d` | exactly 1 | The entry ID. |
| `x` | exactly 1 | The lowercase hex SHA-256 of the exact `document` bytes. |
| `t` | exactly 1 | `oa:kb:entry:v1`. |
| `t` | exactly 1 | `oa:kb:kind:<kind>`, equal to the body's `kind`. |
| `t` | 0 or more | One per topic tag in the document's front matter, lowercase. |

The `x` tag lets a reader deduplicate versions and match evidence without
parsing the document. The `d` tag on a regular event is an index for
`#d` filters; it doesn't make the event replaceable.

The document's `status` and `author` fields are the author's own claims. A
reader never takes them as its own trust decision; see [Trust](#trust).

### Equivocation

Two `3190` events from one author with the same ID and version but different
`x` digests are equivocation. A reader MUST report the conflict and MUST NOT
choose either version by timestamp. Two events with the same ID, version,
and digest are the same version published twice.

## Heads (`30190`)

A head points at the author's current version of one entry:

```json
{
  "v": 1,
  "requires": [],
  "type": "head",
  "id": "statistics.mmd-estimators",
  "version": 2,
  "entry": {"id": "<3190 event id>", "pubkey": "<author>", "kind": 3190}
}
```

Tags: exactly one `d` equal to `id`, exactly one `e` equal to `entry.id`, and
the marker `oa:kb:head:v1`. The head's signer MUST equal `entry.pubkey`, and
the referenced `3190` MUST have the same ID and version. A head is a
discovery aid; a run records the exact `3190` event and digest it used, never
the head.

A reader without a head for an entry takes the highest version it has that
isn't withdrawn and isn't in conflict.

## Withdrawals (`3191`)

A withdrawal ends one exact version:

```json
{
  "v": 1,
  "requires": [],
  "type": "withdrawal",
  "id": "statistics.mmd-estimators",
  "version": 1,
  "entry": {"id": "<3190 event id>", "pubkey": "<author>", "kind": 3190},
  "reason": "The unbiased form was stated with the wrong denominator."
}
```

Tags: exactly one `d`, exactly one `e` naming `entry.id`, and the marker
`oa:kb:withdrawal:v1`. The signer MUST equal `entry.pubkey`: only the author
withdraws a version. `reason` is display text of at most 1,000 characters.

A withdrawal is irreversible for that exact version. Publishing the head
again doesn't undo it; the author publishes a new version instead. A reader
never shows a withdrawn version and keeps the withdrawal so earlier runs can
be explained. Another author's opinion that an entry is wrong is evidence
(below), not a withdrawal.

## Evidence (`3189`)

Evidence that an entry helps, or doesn't, is a NIP-EVAL public evaluation
declaration. Its report is an `openagents.eval-report.v1` whose subject is
one entry version:

- `subject.definition` is a DefinitionRef: the entry's qualified ID, an
  ArtifactRef to the exact `document` bytes (media type `text/markdown`,
  schema `openagents.kb-entry.v1`), and `event`, the `3190` EventRef.
- `subject.configuration` names the arm with the entry shown and
  `baseline.configuration` the arm without it. Both arms run the same tasks
  with the same model.
- `partition.excluded` lists every task the entry was written from. A task
  an entry was written from never counts as evidence for it.
- `runs`, `coverage`, `measurements`, and `verdict` follow NIP-EVAL. The
  measurements include each arm's pass rate and cost per run, and the number
  of paired tasks that favor and oppose the entry.

The `3189` event follows NIP-EVAL exactly: `t: oa:eval:v1`, `x` equal to the
report digest, and the body `{v: "openagents.eval-publication.v1", requires,
report, subject, supersedes}`. This profile adds two things:

- One `e` tag per `3190` event the report's subject names, so a reader can
  find evidence with an `#e` filter.
- `meta.kb_report`: the report's exact bytes as a string. A reader checks
  them against `report.digest` and `report.size` before reading them. Like
  all `meta`, it's inert: it carries bytes, never an instruction.

The signer is the evaluator. Anyone can publish evidence about anyone's
entry, and a reader weighs it by who ran it.

## Trust

Trust is per reader. A reader keeps its own list of trusted authors and
trusted evaluators and decides from them:

- Entries from its own key and from authors it trusts keep the status their
  document states.
- Entries from any other author are at most `candidate`, whatever their
  document says. A reader MUST NOT show them as admitted.
- An entry earns admission on the reader's side from evidence by evaluators
  the reader trusts, or from the reader's own review.

A reader computes its own embeddings for retrieval. It never accepts a vector
from an author, so a published entry can't steer retrieval with a crafted
vector.

Relays are transport. A relay can drop, delay, or replay events, but it
can't forge one: every check above uses the signed event, never the
subscription it arrived on.

## Validation

A reader accepts a `3190` only when all of these hold:

1. The event ID and signature verify (NIP-01).
2. The body parses with the [shared encoding rules](contracts.md#encoding-and-validation),
   has `v: 1`, an empty `requires`, `type: entry`, and no unknown keys.
3. The `d`, `x`, and `t` tags agree with the body as the table above states.
4. The document parses as an entry, and its `id`, `version`, and `kind`
   equal the body's.
5. The reader's own content checks pass. `microcoder kb sync` runs the same
   lint as local entries: a cited source, a bounded summary, and no name or
   quotation of a benchmark task the reader has installed.

Heads, withdrawals, and evidence are checked the same way against their own
fields, and each is bound to its `3190` by exact event ID.

The common body ceiling is 1,048,576 bytes; a relay's frame limit is usually
lower. Readers refuse, never truncate, an oversized document.

## Curated snapshots

A snapshot is a signed, digested set of entry versions that a host pins, so a
run names exactly the knowledge it had. A snapshot is a [NIP-EXT](NIP-EXT.md)
package release: each entry is a `guidance` component whose definition
ArtifactRef is the exact document bytes, and the release's manifest lists
every file. Installing a snapshot follows EXT: verify the closure, commit
one lock, and never run anything. A snapshot is how a team shares an admitted
set; it isn't an admission by itself.

## Private entries

A team that doesn't want an entry public sends it in the encrypted
[`3188` artifact envelope](contracts.md#private-artifact-envelope), one
envelope per recipient. The artifact's schema is `openagents.kb-entry.v1`
and its bytes are the document. Envelope rules, not this NIP, govern who can
read it. A private entry is never published as a `3190`.

## Studies

A study that measures whether one entry or a whole snapshot helps is a
[NIP-OPT](NIP-OPT.md) study with the entry or snapshot as the thing under
test, frozen before its trials. Its results are NIP-EVAL reports as above.

## Relay and client conformance

Relays store `3190`, `3191`, and `3189` as regular events and `30190` as
addressable, validate the signature, and apply the lowercase `t` rule. A
relay doesn't parse documents, judge content, or rank entries. Removal from
one relay is neither withdrawal nor global erasure.

Clients cover, with fixtures: a tampered document (digest mismatch), a
tampered event (signature), a body and tag disagreement, a document whose
front matter disagrees with the body, equivocation, a head or withdrawal
signed by someone other than the author, a withdrawn version, and evidence
whose inline report doesn't match its digest.

Advertise `nip-kb-v1` in NIP-11 `supported_extensions` only for a relay role
covered by those fixtures. Keep the draft name out of numeric
`supported_nips`.

# NIP-ATIF — Agent trajectories

`draft` `optional` — v1, 2026-09-28. **Designed.** The
[shared contracts](contracts.md) are normative.

This NIP carries agent trajectories in the Agent Trajectory Interchange
Format (ATIF) over Nostr. It says how a trajectory is identified, how its
bytes travel privately to an owner or publicly to anyone, how a large one is
split into chunks, and how a trajectory links to the Coder task that
produced it, to the sub-agent trajectories it delegated to, and to the
segment it continues.

ATIF itself is not defined here. Its upstream specification is the Harbor
project's [RFC 0001](https://github.com/harbor-framework/harbor/blob/7464ab541773ea1d4618336f043970042f33a1b5/rfcs/0001-trajectory-format.md)
(`ATIF-v1.8` at that revision). `crates/atif` writes `ATIF-v1.7`, which
[Coder traces](../../docs/coder/runtime/traces.md) describe: an append-only
`.atif.jsonl` log, one JSON record per line, from which the ATIF document is
rendered on read. Coder task attempts write the same log as
`<task>.<attempt>.atif.jsonl`.

A trajectory is an observational record: what a recorder saw, in order. It
is not the authority for what happened. RUN records decide effects and
recovery, EVAL decides whether a result passed, and POL decides what may be
disclosed. A signature on a trajectory proves who published it, not that
the recording is complete, honest, or correct.

## Status

Designed. No component publishes or reads these events yet. Traces remain
local files; the [SESS observer](NIP-SESS.md#read-only-observation-of-retained-foreign-history)
reads bounded raw pages of `<task>.<attempt>.atif.jsonl` files over private
`3188` artifacts, and that path is unchanged by this NIP. The schemas
`schemas/atif-manifest.v1.json` and `schemas/atif-chunk.v1.json` and the
worked fixture in `crates/nostr/tests/fixtures/atif` are checked by
`crates/nostr` (`atif_nip`) and `crates/atif` (`nip_atif_fixture`).

## Kinds

These are OpenAgents draft assignments, not upstream registrations.

| Kind | Class | Record |
| --- | --- | --- |
| `3198` | Regular | Public trajectory declaration: one manifest, signed by its publisher. |
| `3199` | Regular | Public trajectory chunk: one ordered slice of a trajectory's bytes. |
| `3188` | Regular | Private trajectory manifest or chunk, carried in the shared private artifact envelope ([contracts](contracts.md)). |

Both new kinds are regular and immutable. A trajectory is evidence, and a
replaceable head would let a later event silently stand in for the bytes a
reader already checked.

The historical SA draft allocated `39230` (trajectory session) and `39231`
(trajectory event). [NIP-SOV](NIP-SOV.md#provenance-and-migration) retired
every `392xx` kind, and this NIP does not reuse them: under NIP-01 they are
addressable, so a relay keeps only the newest event per coordinate, which is
the wrong storage rule for an ordered record. Their role is split between
this NIP (the trajectory bytes) and RUN (durable authority over effects).
Old events import as historical evidence under SOV's migration rules.

## Identity

A trajectory has two forms and three digests. Keep them apart.

- **Log form.** The exact bytes of an `.atif.jsonl` log, media type
  `application/jsonl`. This is what a recorder writes and what this NIP
  prefers to carry, because it is the one copy of the truth.
- **Document form.** An ATIF document, media type `application/json`,
  for recorders that write only documents, and for conversions.

| Digest | Over | Encoding | Stable across |
| --- | --- | --- | --- |
| `artifact.digest` | The exact bytes carried, log or document. | Shared `Digest`, `sha256:` and 64 hex. | Nothing: any byte change is a new artifact. |
| `steps_digest` | The document's `steps` array, by the ATIF rule. | 64 lowercase hex, no prefix. | Re-rendering the same log, key order, whitespace. |
| Step digest | One step object, by the ATIF rule. | 64 lowercase hex, no prefix. | The same. |

**The ATIF rule** is the digest `crates/atif` and the Gym already compute,
and the one [NIP-EVAL](NIP-EVAL.md#gym-results-publication) names for
leaderboards: serialize the JSON value with every object's keys sorted at
every depth, in byte order of their UTF-8 encoding, each key written as a
JSON string, no whitespace, arrays in order, and other values as a standard
JSON serializer writes them; then SHA-256 the UTF-8 text. It is close to
RFC 8785 JCS but not the same (number formatting and the sort order of keys
outside the Basic Multilingual Plane can differ), so the two are never
compared with each other. Shared-contract canonical digests stay JCS.

`steps_digest` exists because a rendered document is not byte-stable: the
reference exporter stamps `extra.exported_at` each time it renders. For the
log form, `steps_digest` is over the `steps` of the document the log renders
to; a reader without an ATIF renderer checks the bytes and reports
`steps_digest` as unverified rather than accepting it.

A trajectory is named by its ATIF `trajectory_id`, unique per document, and
belongs to the run its `session_id` names. Following ATIF-v1.7, `session_id`
is run-scoped and never resolves a reference by itself. A step is addressed
as `(trajectory_id, step_id)`, where `step_id` is the document's one-based
ordinal. A reference that must pin content adds `steps_digest` or the step
digest; an ID alone is a name, not a pin.

## Manifest

Every carried trajectory has one manifest,
`openagents.atif-manifest.v1`, a closed object:

| Field | Contract |
| --- | --- |
| `v` | `"openagents.atif-manifest.v1"`. |
| `requires` | Empty array. |
| `trajectory_id` | The document's `trajectory_id`, 1–256 bytes. |
| `session_id` | The document's `session_id`, or `null`. |
| `schema_version` | The ATIF version the bytes declare, such as `"ATIF-v1.7"`. A reader refuses a version it does not support as `unsupported_version`. |
| `form` | `"log"` or `"document"`. |
| `artifact` | ArtifactRef of the whole bytes: `digest`, `size`, `media_type` (`application/jsonl` for a log, `application/json` for a document), `schema` equal to `schema_version`, and optional `sources` URL hints. |
| `steps_digest` | As above. |
| `step_count` | The number of steps the bytes render to. |
| `state` | `"ended"` when the log has its closing record, else `"interrupted"`. |
| `coverage` | `"complete"` only for unmodified bytes that a whole-log reader accepted (no faults and a closing record, as `atif::log::read_whole` requires). Otherwise `"partial"`. |
| `derivation` | `null` for an original recording; else `{kind, source}` where `kind` is `redacted`, `truncated`, or `converted` and `source` is the original's `steps_digest` or `null` (see [Disclosure](#public-and-private)). |
| `chunks` | Ordered `{index, digest, size, event?}` entries, one per chunk; empty when the bytes travel only by `artifact.sources`. `event` is the 64-hex ID of the chunk's event when the publisher knows it. |
| `task` | `{task_id, attempt}` for a Coder task attempt (the `<task>.<attempt>.atif.jsonl` name), or `null`. |
| `run` | The 64-hex RUN run ID this trajectory observed, or `null`. |
| `parent` | `{trajectory_id, step_id}` of the step that delegated to this trajectory, or `null`. |
| `children` | Ordered `{trajectory_id, step_id, steps_digest, artifact, event?}` entries, one per delegated trajectory carried separately. `artifact` is the child's `artifact.digest`; `event` is the child's manifest event ID when known. |
| `previous` | `{trajectory_id, steps_digest}` of the segment this one continues (ATIF `continued_trajectory_ref`), or `null`. |

Unknown keys are refused. The body fits the shared 1,048,576-byte ceiling
and, when carried inline in one event, the smallest relay and NIP-44 bound on
the path. A manifest describes bytes; it never runs anything and grants
nothing.

## Chunks

A trajectory larger than one event travels as ordered chunks. Each chunk is
`openagents.atif-chunk.v1`, a closed object:

| Field | Contract |
| --- | --- |
| `v` | `"openagents.atif-chunk.v1"`. |
| `requires` | Empty array. |
| `trajectory_id` | The manifest's `trajectory_id`. |
| `artifact` | The manifest's `artifact.digest`. |
| `index` | Zero-based position. |
| `count` | Total chunks, 1–4,096. |
| `digest` | `sha256:` over the exact UTF-8 bytes of `text`. |
| `text` | The chunk's bytes as a JSON string, 1–32,768 bytes of UTF-8. |

Splitting rules:

1. The chunks' `text` bytes, concatenated in `index` order, are exactly the
   artifact's bytes: same digest, same size.
2. Every boundary falls on a UTF-8 character boundary. In the log form a
   boundary falls after a newline, so each chunk holds whole records, unless
   one record alone exceeds 32,768 bytes; then that record is split on
   character boundaries and only that record's chunks end mid-line.
3. The JCS encoding of the chunk body is at most 65,535 bytes, so it fits
   one standard NIP-44 plaintext. Heavy JSON escaping can push a 32 KiB
   `text` over that; the producer then cuts smaller.
4. A producer may cut smaller than the bounds, for example to publish a live
   session's closed prefix as it goes. The bounds are ceilings, not targets.
5. At most 4,096 chunks, so at most 128 MiB per trajectory over events.
   Larger trajectories travel by `artifact.sources` or as continued segments.

A reader fetches every listed chunk, checks each `digest` and `size`, checks
the concatenation against `artifact`, and only then parses. A missing chunk
makes the trajectory `content_unavailable`; a reader never renders a prefix
as though it were the whole. It may show a verified prefix explicitly
labeled as partial, the way the recovering log reader does.

## Public and private

Trajectories are private by default. They hold prompts, command output,
file contents, and paths from the recorder's machine. Carrying one publicly
is a separate disclosure decision, never a side effect of observing,
decrypting, or evaluating it: permission to see a trajectory is not
permission to publish it, train on it, or pass it on
([SOV](NIP-SOV.md), [POL](NIP-POL.md)).

### Private: owner-encrypted artifacts

The recorder (a Coder host, or an agent key acting for an owner) sends the
manifest and each chunk as its own kind `3188` private artifact envelope,
NIP-44 encrypted to one recipient, usually the owner:

- The envelope's `artifact` is the ArtifactRef of the manifest or chunk
  object, with `schema` `openagents.atif-manifest.v1` or
  `openagents.atif-chunk.v1`, and `inline` is that object. The envelope's
  JCS rule then binds the object, and the chunk's own `digest` binds its
  `text` to the manifest.
- All envelopes for one trajectory and one recipient share one random `h`
  mailbox, generated for that sharing scope as the shared contracts require,
  so the recipient reads them with one filter. The mailbox never derives
  from the trajectory ID, a path, or a digest.
- Send chunks before the manifest. A recipient that sees the manifest can
  then fetch every chunk it lists.
- Each recipient gets its own envelopes. A copy for a second recipient is a
  second disclosure under its own grant.

The relay rules are the envelope's: author-authenticated publication, reads
and COUNT restricted to author and recipient, no search. The relay learns
that the recorder sent the owner some number of artifacts of some size,
not what they hold. This is the same agent-to-owner trust shape as Block
[NIP-AE](../block/NIP-AE.md) and [NIP-AM](../block/NIP-AM.md), carried in
the OpenAgents envelope rather than a Block kind.

A NIP-REACH direct channel or a HOST read may deliver the same manifest and
chunk objects without a relay; the byte checks are identical.

### Public: declarations and chunks

A publisher that has consent to publish signs:

- one kind `3199` event per chunk. Its content is the chunk object as
  JSON. Tags: exactly one `t: oa:atif:chunk:v1`, exactly one `x` equal to
  the chunk's `digest` without the `sha256:` prefix, and an optional NIP-31
  `alt`.
- then one kind `3198` event whose content is the manifest. Tags: exactly
  one `t: oa:atif:trajectory:v1`, exactly one `x` equal to
  `artifact.digest` without its prefix, one `e` tag per entry in
  `children` that carries an `event`, and an optional NIP-31 `alt`.

Kind `3199` is a regular immutable record of one chunk; kind `3198` is a
regular immutable declaration of one trajectory. A reader finds chunks with
`{"kinds": [3199], "authors": [<publisher>], "#x": [<chunk hex>]}` or by
the `event` IDs in the manifest, and never accepts a chunk from another
author for the publisher's manifest. The same bytes may also be hosted
elsewhere: `artifact.sources` may name an HTTPS page or a
[NIP-B7](../official/B7.md) Blossom URL, and a reader checks fetched bytes
against `artifact.digest` exactly as it checks chunks. A hosted trace page
is a locator, never the identity.

A public trajectory is almost always a derivative. Redaction, truncation,
or conversion produces new bytes, a new `artifact.digest`, and a new
`steps_digest`, with `derivation` saying which. A derivative is never
presented as the original, and `coverage` is `partial` for anything
redacted or truncated. `derivation.source` names the original's
`steps_digest` only when the publisher chooses to link them: a digest of
predictable private content is not anonymous, so the default for a public
redaction is `null`.

Deleting a public event (NIP-09) asks relays to drop it; it cannot recall
copies already fetched. Say so rather than promising erasure.

## Links

### Coder tasks and sessions

`task` binds a trajectory to one Coder task attempt, matching the log name
`<task>.<attempt>.atif.jsonl`. A retry is a new attempt and a new
trajectory. `run` binds it to the RUN run it observed. A NIP-SESS
`openagents.session-history.v1` export names a trajectory through its
`portable` ArtifactRef; that ArtifactRef may point at a manifest, which then
pins the bytes. These links are claims by the signer. A reader that needs
the task's authoritative state reads RUN and HOST, not the trajectory.

### Delegated sub-agents

A delegation has two representations in ATIF-v1.7, and both stay valid:

- **Embedded.** The child is an element of the parent document's
  `subagent_trajectories`, resolved by `trajectory_id`. It travels inside
  the parent's bytes; this NIP adds nothing.
- **Separate.** The child is its own trajectory with its own manifest. The
  parent's delegating step records `subagent_trajectory_ref` with the
  child's `trajectory_id`, and with `trajectory_path` set to a NIP-21
  `nostr:nevent1…` URI naming the child's manifest event when there is one.

For the separate form, the link is authoritative only in the parent's
direction. The parent's manifest lists the child in `children` with the
child's `steps_digest` and `artifact` digest, pinning exact bytes. The
child's `parent` names the parent's `trajectory_id` and the delegating
`step_id` but no digest, because a child usually finishes while its parent
is still recording. A reader accepts a parent–child link only when the
parent's manifest binds the child's digests and the child's `parent` names
that parent and step. A child that names a parent the parent does not list
is an unconfirmed claim, shown as such.

Children publish before their parent. A private child goes to the same
recipient under its own grant; a public parent may list a child that stays
private, and a reader then shows the child as `content_unavailable` with
its pinned digests. Delegation narrows disclosure: a child is never
published under wider consent than its parent.

### Continued segments

When context management starts a new trajectory (ATIF
`continued_trajectory_ref`), the new segment's `previous` names the prior
segment's `trajectory_id` and `steps_digest`. Segments share a `session_id`.
A reader orders segments by these links, never by timestamps.

## Relationship to OpenAgents NIPs

- **[SESS](NIP-SESS.md).** Session history exports name a trajectory as
  their portable form; steering consumption is recorded as its own ATIF
  step. The retained-history observer reads raw log pages and stays the
  interactive path; a manifest is the pinned, whole-trajectory form.
- **[RUN](NIP-RUN.md).** RUN journals are authoritative for effects,
  fencing, and recovery. A trajectory can be cited as RUN evidence by
  ArtifactRef, but it cannot resume, fence, or prove that an effect
  happened.
- **[CTX](NIP-CTX.md).** A trajectory or one step, addressed as above, can
  be a context evidence item. Selecting an excerpt does not make it the
  whole trajectory.
- **[EVAL](NIP-EVAL.md).** Reports reference trajectories as artifacts.
  An evaluator treats a `partial` trajectory, or one whose chunks or
  `steps_digest` did not verify, as unverifiable, never as a pass. EVAL
  already uses the ATIF rule for Gym leaderboards.
- **[CTRL](NIP-CTRL.md).** Catch-up and replay projections rest on
  retained trajectories; a projection is still partial and bounded, and
  CTRL observation rights do not include publication.
- **[SOV](NIP-SOV.md).** SOV's "link full authorized local trajectories
  through ATIF" is carried by this NIP, and it replaces historical
  `39230`/`39231`.
- **[OPT](NIP-OPT.md), [KB](NIP-KB.md), [XP](NIP-XP.md), [LAB](NIP-LAB.md).**
  Trial receipts, knowledge evidence, reproductions, and labor deliverables
  may cite trajectories. Training on or republishing one needs its own
  recorded consent and keeps its provenance.

## Relationship to Block NIPs

Block's Buzz specifications define no trajectory or transcript format, and
the Buzz tree contains no ATIF writer: its Harbor benchmark adapter
declares `SUPPORTS_ATIF = False`. Buzz agent conversation lives in NIP-29
channel messages (kind `9`), live protocol traffic in ephemeral
observability frames, usage in metrics records, and memory in engrams. The
mapping below lets a host that speaks both produce ATIF from Block traffic
and cite Block events from ATIF steps. None of these kinds carries ATIF
bytes.

| Block NIP | Relationship |
| --- | --- |
| [NIP-AO](../block/NIP-AO.md) observer frames, `24200` | Ephemeral, relay-unstored, at most 65,535 bytes per frame. A host observing AO telemetry (`acp_read`, `acp_write`, `turn_started`, `session_resolved`) may derive ATIF steps from it: AO `sessionId` becomes `session_id`, and each step records `extra.buzz_ao` with `sessionId`, `turnId`, and the `seq` range it came from. The result is `converted`, and `coverage` is `complete` only if every `seq` was seen and `session_resolved` arrived. AO control (`cancel_turn`) is not a trajectory step unless the agent records acting on it. |
| [NIP-AM](../block/NIP-AM.md) turn metrics, `44200` | One AM event per completed turn, keyed by `(sessionId, turnSeq)`. ATIF agent steps that close that turn record `extra.buzz_turn` with `sessionId`, `turnSeq`, and `turnId`. Token counts map to ATIF step `metrics` (`inputTokens` to `prompt_tokens`, `outputTokens` to `completion_tokens`, `costUsd` to `cost_usd`), and AM's rule holds in both directions: an unreported count stays absent, never zero. AM is accounting; it cannot rebuild a trajectory, and a trajectory does not replace AM. |
| [NIP-AE](../block/NIP-AE.md) engrams, `30174` | Durable memory, not history. A step that read or wrote an engram may record its event ID in a private trajectory; a public derivative drops it, because the blinded `d` tag exists to keep slugs private. |
| [NIP-PMA](../block/NIP-PMA.md), reserved `30179` | The managed agent's configuration aggregate. A trajectory's `agent` names the model and version that ran; it never carries PMA secrets. |
| NIP-29 channel messages (kind `9`) | The conversation people see. A user or agent step may record the event ID it received or sent in `extra.nostr_event`, linking chat to steps. Channel messages are not a trajectory, and a trajectory does not republish them. |
| [NIP-CW](../block/NIP-CW.md), [NIP-RS](../block/NIP-RS.md), [NIP-DV](../block/NIP-DV.md) | Channel windows, read state, and DM visibility are views of chat. They have no trajectory role. |

A Buzz agent that wants durable, owner-private history gets it from this
profile: the agent key signs `3188` envelopes to its owner, the same
relationship AE and AM already verify.

## Validation

A reader checks, in order, and stops at the first failure:

1. Event ID and signature, kind, and exact tags (public), or the private
   envelope's rules and decryption (private).
2. The manifest's closed shape, `v`, `requires`, and supported
   `schema_version`.
3. Every chunk: author, closed shape, `trajectory_id` and `artifact` equal
   to the manifest's, `index` and `count`, `digest` over `text`, and the
   manifest entry's `digest` and `size`.
4. The concatenation's digest and size against `artifact`; bytes fetched
   from `sources` get the same check.
5. With an ATIF renderer: `step_count`, `state`, and `steps_digest` against
   the rendered document, and `coverage` against the whole-log reader.
   Without one, report them unverified.
6. For each `children` entry: when the child is available, its manifest's
   `trajectory_id`, `steps_digest`, and `artifact.digest` match, and its
   `parent` names this trajectory and step.

The outcome is one of verified, unverified (content matched; rendered
fields not checked), partial, or refused with its reason. A trajectory
never becomes more complete than its weakest chunk.

## Relay and client conformance

A relay that accepts `3198` and `3199` checks NIP-01 validity and the
exact `t` and `x` tags. It does not parse, validate, or render the ATIF
bytes, and storing them does not endorse them. Private carriage uses the
`3188` relay rules unchanged.

Advertise `oa-atif-v1` in NIP-11 `supported_extensions` only for roles the
relay or client actually implements with fixtures: publication, chunk
retrieval, or verification. Keep it out of numeric `supported_nips`.

## Security and privacy

- A public trajectory is a publication of everything in it. Redact before
  signing; a signed event cannot be unpublished.
- Visible tags reveal who published, when, and roughly how much. For
  private carriage the `p` tag and mailbox reveal the recorder–recipient
  relationship and traffic volume.
- Digests of short or predictable steps can be confirmed by guessing.
  Public manifests do not name private originals by default.
- Trajectories contain untrusted text from models, tools, and other
  people. A reader renders it as data and never follows instructions,
  links, or `trajectory_path` values inside it without its own fetch
  authority and bounds.
- Bound every fetch: chunk count, bytes, time, and nesting depth, before
  allocation.

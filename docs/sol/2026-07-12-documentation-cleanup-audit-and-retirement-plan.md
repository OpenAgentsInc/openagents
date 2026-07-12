# Sol documentation cleanup audit and ordered retirement plan

- Date: 2026-07-12
- Repository snapshot: `b2ba1035c9f1bd466b4ac1121eaf0fdee426ab57`
- Canonical roadmap inspected: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md),
  Revision 82
- Source analysis:
  [`2026-07-11-sol-and-teardowns-longform-analysis.md`](../fable/2026-07-11-sol-and-teardowns-longform-analysis.md)
- Status: cleanup analysis and proposed execution order; not roadmap authority
- Scope: `docs/sol/`, its direct dependencies, and the use of the separate
  `OpenAgentsInc/backroom` repository for fully obsolete material

## Execution ledger

| Chunk | Live issue | Final state | Landed receipt |
| --- | --- | --- | --- |
| SOL-DOC-01 — truthful, revision-independent entry points | [#8723](https://github.com/OpenAgentsInc/openagents/issues/8723) | **Closed 2026-07-12.** `docs/sol/README.md`, `docs/sol/issues/README.md`, and `docs/sol/IMPLEMENTATION_ROADMAP.md` now route to current authority without caching a master revision, issue count, old queue, or executable delegation diary. | `9d432817e9` — local links resolve; stale Revision 25/29/31 and delegation-dispatch scans pass; repository fast pre-push guards pass. |
| SOL-DOC-02 — durable operating and subsystem contracts | [#8724](https://github.com/OpenAgentsInc/openagents/issues/8724) | **Closed 2026-07-12.** Operating/subsystem guidance is revision-independent, does not own the current queue, treats #8640 as closed proof, keeps physical Android non-gating, and separates persona-neutral voice from closed Sarah presentation scope. The July 9 authority analysis now links to repository invariants. | `1eefd72465` — local links, stale-policy scans, positive voice/Android/#8640 assertions, invariant-link check, and repository fast pre-push guards pass. |
| SOL-DOC-03 — retire false dispatch artifacts | [#8725](https://github.com/OpenAgentsInc/openagents/issues/8725) | **Closed 2026-07-12.** The July 10 delegation diary is non-dispatch and its executable prompt is removed; active inbound links now use live authority. The CUT plan, CUT-27 audit, parity score, and Desktop delivery sequence are pinned historical evidence. | `7476316a69` — changed-file links resolve; active inbound-link and obsolete-prompt scans pass; snapshot-status assertions and repository fast pre-push guards pass. |

**Next ordered chunk:** SOL-DOC-04 compacts the master roadmap. It retains
owner decisions, durable gates/laws/non-goals, one current issue projection,
and one next-ready sequence; extracts old implementation diaries, starting-gap
snapshots, duplicated receipts, obsolete execution orders, and the historical
Fable reconciliation pass without changing product truth.

## Executive decision

Sol has a documentation reliability problem, not merely a documentation
volume problem. The active reading path mixes current authority, superseded
plans, implementation diaries, issue-body snapshots, proof receipts, and
historical arguments. A reader can enter through a file that calls Revision
25, 29, 30, or 31 current even though the canonical roadmap is Revision 82.
Some of those files still call closed work active, describe landed product
capabilities as absent, or pause behavior that the owner later reauthorized.

That makes stale prose operationally dangerous. It can cause an agent to
dispatch against an obsolete gate, revive closed scope, repeat completed work,
or mistake an intermediate proof rung for current truth.

The cleanup should therefore optimize for **one small dispatch surface**, not
for preserving every historical document beside it. The target is:

1. current authority and current work stay in `openagents`;
2. immutable receipts, failure evidence, contracts, and non-revival
   tombstones stay in `openagents`, but move out of the active root and become
   clearly indexed;
3. fully obsolete narrative, planning, and execution material is exported to
   the separate `backroom` archive with provenance, then removed from
   `openagents` after inbound links and retained conclusions are migrated;
4. no active document hard-codes a roadmap revision or manually maintained
   open-issue count;
5. current status is written once and derived or linked everywhere else.

This is the documentation equivalent of the product rule already identified
by the longform analysis: every bridge needs an owner, an expiry condition,
and a deletion gate. Supersession banners are useful during migration, but a
permanent pile of bannered prose is still a permanent compatibility layer.

## Audit basis

This audit reviewed the longform analysis, the current master roadmap and Sol
entry points, the operating and subsystem documents, the checked-in issue
index, the cutover plan, the CUT-27 readiness audit, the July 9 strategy
corpus, the July 10 delegation and parity material, relevant teardown archive
rules, and the current `backroom` archive layout.

At the inspected snapshot:

- `docs/sol/` contains **50** top-level Markdown files and **44** checked-in
  issue records;
- the Sol tree contains roughly **16,400 lines** of Markdown;
- four status-heavy files alone contain roughly **4,500 lines**:
  `MASTER_ROADMAP.md`, the CUT-01–CUT-27 issue plan, the July 10 executable
  delegation packet, and the CUT-27 readiness audit;
- the canonical master roadmap is about **2,175 lines** and has accumulated
  current direction, closed implementation receipts, revision amendments,
  old baseline prose, and a historical Fable reconciliation pass in one file;
- the live canonical set is **11** open `roadmap:sol` issues:
  `#8547`, `#8566`, `#8574`, `#8597`, `#8636`, `#8676`, `#8677`, `#8689`,
  `#8696`, `#8706`, and `#8707`;
- CUT-25 `#8705` is closed, despite older active-looking status documents
  continuing to treat it as open or pending.

The exact issue set will change. Its inclusion here is a pinned audit fact,
not a proposal to create another hand-maintained current ledger.

## What is actually wrong

### 1. Entry points lie before the reader reaches current authority

[`README.md`](./README.md) calls Master Revision 29 current and still presents
old analyses and the Revision 25 delegation packet as part of the executable
starting path. [`issues/README.md`](./issues/README.md) says there are 36 open
records, names Revision 31 as current, and treats the full CUT graph as the
immediate queue.

These are the highest-risk defects because they are designed to orient a new
reader. A correct master roadmap cannot compensate for an index that sends the
reader elsewhere first.

### 2. Durable operating documents contain volatile status

[`OPERATING_MODEL.md`](./OPERATING_MODEL.md) says it is active under Revision
25. [`SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md`](./SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md)
also names Revision 25 and points to the stale delegation packet. The latter
still says all voice/ASR/VAD work is paused, while the current roadmap narrowly
reauthorizes persona-neutral conversational voice. It also retains old client
gaps and a physical-Android expectation that the owner has explicitly removed
as a gate.

The durable material in these files is valuable: claim hygiene, authority
boundaries, dependency discipline, failure semantics, and subsystem ownership.
The mistake is embedding “what is open right now” in the same prose. Durable
rules and current state need different homes and different refresh cadences.

### 3. A redirect became a second stale roadmap

[`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md) correctly says its
filename is historical and must not be used for dispatch, but the redirect
itself calls Revision 25 the current reset, points at the obsolete executable
delegation packet, and repeats superseded voice state. A redirect should be a
few lines: status, canonical destination, and perhaps the migration date. It
must not contain a cached summary of the destination.

### 4. The master roadmap is both authority and archive

The current [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) has the right top-level
owner decisions and current 11-issue set, but its body preserves many older
“current” states:

- CUT-13 calls now-closed CUT-14 the next dependency-ready cut;
- a CUT-14 section says Android acceptance remains, while the header and
  current issue table say it is complete;
- “Starting gap” and Desktop “Current rung” sections retain pre-CUT-17–25
  implementation state and an old parity score;
- CUT-11 is called active immediately before later prose calls it closed;
- execution text says to start at closed CUT-01/CUT-02 issues;
- the current acceptance sequence still speaks about reusing a surface for
  now-closed CUT-25;
- a long, explicitly historical Fable reconciliation pass remains appended to
  the canonical authority.

This directly violates the roadmap's own instruction not to grow a revision
diary and the [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md) expectation that
the master remain concise. Updating all old paragraphs after every landing is
not sustainable. Adding another amendment at the top makes each local
contradiction safer but makes the file globally less legible.

### 5. The largest executable packet is historical but still dispatchable

[`2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](./2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)
is a 793-line execution diary that claims executable authority under Revision
25. It contains stale claim narration, old issue dispositions, and a reusable
prompt directing agents to Revision 25. It has multiple inbound references.

This file should not be “updated to Revision 82.” Its value is historical: it
records how a particular burn was delegated and what landed. Its current
dispatch role has expired. Current work should be selected from live issues,
the canonical roadmap, and [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md).

### 6. The CUT plan and readiness audit accumulated state instead of replacing it

[`2026-07-11-openagents-coding-cutover-issue-plan.md`](./2026-07-11-openagents-coding-cutover-issue-plan.md)
still calls itself an active issue graph. Its dependency graph and completion
criteria are useful, but its mutable status table and per-cut “current state”
paragraphs predate many closures. CUT-14 and CUT-17 are described as pending,
later cuts have no final disposition, and CUT-25 appears open.

[`2026-07-12-cut27-cutover-readiness-audit.md`](./2026-07-12-cut27-cutover-readiness-audit.md)
has the same additive shape inside one day: later headings and addenda close
work that earlier blocker tables and the “honest summary” still call open.
Chronology is useful evidence, but it is not a safe current queue.

### 7. The parity and architecture baselines are being mistaken for current status

[`2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)
still describes the Desktop as a prototype and marks important capabilities
absent or scaffolded. CUT-13 through CUT-25 invalidate much of its OpenAgents
implementation-state column. The competitor capability model remains useful;
the score does not.

[`2026-07-10-openagents-desktop-product-architecture.md`](./2026-07-10-openagents-desktop-product-architecture.md)
contains valuable authority and process topology, but its F0–F7 delivery
sequence and implementation-state prose are a dated plan. These two kinds of
content should not share an implied refresh guarantee.

### 8. Checked-in issue sources have become a second issue database

Several files under [`issues/`](./issues/) say “implemented; close after the
receipt” even though the live issue is closed. The issue index also mixes
active records, closed proof sources, and closed `wontdo` tombstones.

Checked-in issue bodies are valuable when they preserve acceptance criteria,
non-revival boundaries, or closure evidence. They should be explicitly
historical sources. They should not require every state transition to be
manually synchronized across GitHub, the master roadmap, the issue index, an
issue-plan table, and an individual Markdown body.

### 9. Receipts are in the active root and contain honest intermediate states

Many receipt files correctly preserve the sequence from partial proof to
closure. For example, an early paragraph may say a device or transport proof
remains while a later addendum closes it. That is appropriate evidence, but a
reader scanning the root or searching a phrase can treat the intermediate
state as current.

The answer is not to rewrite the history. Receipts need an immutable snapshot
identity and a final-disposition header, then an index under a receipt-specific
path. Intermediate sections should be labelled as rung snapshots.

### 10. The July 9 argument corpus is no longer product-repo working material

The nine July 9 documents total roughly 1,700 lines. They capture the Sarah
reset, original system model, execution sequence, trust/economics reasoning,
Effect Native thesis, risk tests, greenfield decision, and triage. Most carry
supersession banners, but they still occupy the active Sol root and are linked
from entry points.

Their surviving conclusions have different destinations:

- authority and trust laws belong in invariants and typed contracts;
- still-live falsifiers belong in the challenge ledger;
- owner decisions and non-revival boundaries belong in the master roadmap or
  a durable decision record;
- triage and cutover history belong with receipts;
- the full obsolete arguments and sequences belong in Backroom.

Keeping all nine in the production repository because some sentences remain
useful is exactly the “bannered and retained forever” failure identified by
the longform analysis.

## Required document classes

Every Sol document should have exactly one class and an explicit disposition.

| Class | Meaning | Mutation policy | Current-state authority |
| --- | --- | --- | --- |
| `authority` | Owner decisions, invariant-bearing gates, current critical path | Reconcile in place; compact | Yes, within its declared scope |
| `contract` | Durable operating, schema, or subsystem boundary | Change only with owning code/test/invariant | No volatile queue state |
| `index` | Navigation to authority, live issues, receipts, or archives | Generated where possible; reconcile on structure change | Only by reference |
| `current-status` | Small, replaceable projection of live issue/proof state | Replace, never append a diary | Yes, but one source only |
| `receipt` | Immutable proof, failure, owner acceptance, or closeout record | Append labelled final disposition; never silently rewrite | Only for its pinned snapshot |
| `historical-analysis` | Dated reasoning or competitive evidence | Immutable; no dispatch | No |
| `redirect` | Stable inbound path to a new authority | Minimal and revision-independent | No |
| `tombstone` | Non-revival or explicit removal boundary | Retain while the boundary matters | Only for the negative boundary |
| `backroom-export` | Fully obsolete material removed from the product repo | Immutable archive with source provenance | No |

An “active historical analysis” is not a valid class. If a conclusion still
controls implementation, promote it into an authority or contract and leave
the analysis as evidence.

## What stays, what moves, and what leaves

### Keep active in `openagents`

- `MASTER_ROADMAP.md`, after compaction;
- `README.md`, as a small entry index;
- `CLAIM_PROTOCOL.md`;
- `CHALLENGE_LEDGER.md`;
- current invariant-bearing contract documents, including the R1–R2 identity
  and Sync contract, after checking that their scope is still binding;
- a revision-independent operating model;
- a revision-independent subsystem ownership/boundary document;
- one current issue/status projection, preferably generated or linked from
  live issue metadata;
- current issue acceptance records only where they are necessary for work not
  yet represented safely in GitHub or typed contracts.

### Keep in `openagents`, but remove from the active root

- CUT, AC, EP, owner-acceptance, emulator, deployment, and failure receipts;
- closed issue-body sources that document acceptance or a non-revival rule;
- the Sarah removal tombstone and behavior-contract evidence;
- teardown evidence, which already declares itself point-in-time design
  evidence rather than current status;
- product promises, invariant-change evidence, and formal/model notes;
- historical decisions still cited by supported product guarantees.

These should be indexed under paths such as `docs/sol/receipts/`,
`docs/sol/issues/closed/`, and `docs/sol/decisions/`. The existing
`docs/sol/receipts/` directory provides a natural start. Do not perform mass
moves until all repository and GitHub inbound links are inventoried.

### Export to `OpenAgentsInc/backroom`, then remove from `openagents`

Use Backroom for documents that are fully out of date, no longer control any
supported surface, and are retained only to understand the evolution of the
program. The repository already describes itself as an archive for historical
implementations and strategic documentation and has established dated
`archive/openagents-*` imports with provenance notes.

The first candidate export set is:

- `2026-07-09-sarah-first-product-architecture.md`;
- `2026-07-09-roadmap-system-model.md`;
- `2026-07-09-execution-sequence-and-critical-path.md`;
- `2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md`;
- `2026-07-09-issue-triage.md`, after its retained closure facts are indexed;
- the obsolete prose from
  `2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md` after
  its landed receipts are linked;
- superseded status sections extracted from the master roadmap, CUT plan,
  CUT-27 audit, parity audit, and desktop architecture.

The following July 9 files also belong in the export batch after more careful
extraction:

- `2026-07-09-authority-trust-and-economics.md`: promote surviving authority
  rules first and correct the current bad `INVARIANTS.md` path while it remains;
- `2026-07-09-risks-tensions-and-decision-tests.md`: move still-live
  falsifiers into `CHALLENGE_LEDGER.md` first;
- `2026-07-09-effect-native-strategic-importance.md`: re-arm its post-Sarah
  falsifier in the challenge ledger before export;
- `2026-07-09-greenfield-mobile-desktop-decision.md`: preserve a compact
  decision record or redirect because retirement and Effect Native material
  still cite it.

Do **not** export-and-delete a document merely because it is old. Age is a
review trigger. The actual test is whether its remaining claims have been
promoted and whether any supported product, issue, contract, promise, or
receipt still relies on its path.

## Backroom export protocol

A cross-repository move is not atomic, so every export batch needs a receipt.
Use a dated destination such as:

`backroom/archive/openagents-sol-docs-2026-07-12/`

The batch must include `ARCHIVE_NOTE.md` with:

- source repository and source commit;
- original path for every file;
- archive date and reason;
- SHA-256 for every imported file;
- retained conclusions and their new authoritative paths;
- inbound links migrated or intentionally preserved;
- the corresponding `openagents` removal commit once it exists;
- an explicit statement that the archive is non-production and non-dispatch.

Execute the future migration in this order:

1. update clean `main` in both repositories and verify unrelated work is not
   present;
2. build and validate the archive manifest from pinned `openagents` files;
3. commit and push the Backroom import first;
4. promote retained conclusions and update all internal links in
   `openagents`;
5. replace externally depended-on paths with minimal redirects where needed;
6. remove only the files whose archive commit and replacement authority are
   recorded;
7. run the documentation guard and link checker;
8. commit and push the `openagents` removal with the Backroom commit in its
   receipt.

If the Backroom push fails, no source deletion occurs. If the `openagents`
cleanup fails after archive import, the archive note remains honest that the
source removal is pending. Git history is not a substitute for this cross-repo
receipt because the purpose is discoverable preservation, not merely possible
recovery.

## Ordered cleanup program

### P0 — Freeze classification and prevent new drift

**Goal:** know which files can dispatch before changing content.

1. Add a machine-readable or consistently parsed header to every Sol document:
   `Class`, `Status`, `Snapshot`, `Supersedes`, `Superseded by`, and
   `Dispatch: yes/no` as applicable.
2. Generate a manifest containing path, class, owner, last meaningful review,
   inbound links, issue links, and proposed disposition.
3. Declare only the master roadmap, claim protocol, durable contracts, live
   issues, and the one current status projection dispatch-capable.
4. Stop adding landing diaries to the master roadmap during the cleanup.

**Exit:** every file has an owner and disposition; no ambiguous “active
analysis” remains.

### P1 — Repair the entry path and concrete contradictions

**Goal:** a new agent cannot be routed to obsolete work.

SOL-DOC-01 [#8723](https://github.com/OpenAgentsInc/openagents/issues/8723)
completed items 1–3 at `9d432817e9`. SOL-DOC-02
[#8724](https://github.com/OpenAgentsInc/openagents/issues/8724) completed
items 4–6 at `1eefd72465`.

1. **Complete.** Rewrite `docs/sol/README.md` to point revision-independently to the current
   master, current issue projection, claim protocol, durable contracts, and
   receipt index. Remove the stale delegation/parity material from “Start
   here.”
2. **Complete.** Rebuild `docs/sol/issues/README.md` as a classification index,
   separating program/client sources from closed receipt sources and `wontdo`
   tombstones. Do not hard-code a revision or manually maintained count.
3. **Complete.** Reduce `IMPLEMENTATION_ROADMAP.md` to a minimal redirect.
4. **Complete.** Remove Revision 25 pins from `OPERATING_MODEL.md` and
   `SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md`; remove current queue state from
   both.
5. **Complete.** Correct the subsystem voice rule, Android acceptance rule, #8640 state, and
   already-landed Desktop/mobile capability statements.
6. **Complete.** Correct the wrong invariant link in the July 9 authority/economics file
   while that file remains in the product repository.

**Exit:** `README.md` plus `MASTER_ROADMAP.md` yields one coherent current
answer; no active companion contradicts voice, Android, #8640, CUT-14, or
CUT-25.

### P2 — Retire false dispatch artifacts

**Goal:** eliminate executable historical prompts and queues.

SOL-DOC-03 [#8725](https://github.com/OpenAgentsInc/openagents/issues/8725)
completed this phase at `7476316a69` without rewriting chronological evidence.

1. **Complete.** Mark the July 10 delegation packet historical and non-dispatch.
2. **Complete.** Replace its inbound references with the master roadmap, live issue,
   `CLAIM_PROTOCOL.md`, current cut leaf, or exact receipt appropriate to the
   caller.
3. **Complete.** Freeze the CUT-01–CUT-27 plan as a dated dependency/acceptance snapshot.
   Remove its claim to be the current queue and link each mutable status cell
   to live issue or receipt state.
4. **Complete.** Freeze the current CUT-27 audit as a pinned readiness snapshot and create a
   small successor only if a current capstone blocker table is still required.
5. **Complete.** Demote the July 10 parity score and F0–F7 implementation state to historical
   baselines; retain reusable capability and topology analysis.

**Exit:** searching the repository cannot find a copy/paste prompt or “active
queue” that starts from Revision 25–31 or a closed CUT.

### P3 — Split and compact the master roadmap

**Goal:** restore the master as a usable authority.

Keep in `MASTER_ROADMAP.md`:

- owner decisions and amendments that still bind;
- product and authority model;
- durable R0–R7 gates and implementation laws;
- explicit non-goals and non-revival boundaries;
- one compact current issue table;
- one compact dependency-aware next-ready sequence;
- links to contracts, status, receipts, and history.

Extract from it:

- per-landing commit narration;
- closed issue implementation diaries;
- old “starting gap” and “current rung” snapshots;
- superseded execution sequences;
- the historical Fable reconciliation pass;
- duplicated receipt detail already held in dedicated files.

Set a first-pass target of **under 800 lines** for the master. The number is a
guardrail, not a reason to delete binding policy. If the binding core cannot fit,
move stable subsystem contracts into owned contract docs and link them rather
than hiding them.

**Exit:** every sentence in the master is either durable policy, current
status, current sequence, or a pointer. No historical section uses present
tense.

### P4 — Normalize receipts and closed issue sources

**Goal:** retain proof without making it look like a backlog.

1. Create `docs/sol/receipts/README.md` with issue, cut, snapshot, final proof
   rung, final disposition, and artifact links.
2. Add final-disposition headers to receipts whose chronological bodies retain
   intermediate open/pending prose, including CUT-11, CUT-12, CUT-14, and
   CUT-23.
3. Move receipt Markdown from the Sol root only after the inbound-link audit;
   update links mechanically and run link validation.
4. Classify every `docs/sol/issues/*.md` file as open source, closed receipt
   source, or tombstone.
5. Move closed sources to an indexed closed path or add immutable snapshot
   headers. Keep `wontdo` records visible as non-revival boundaries, not as
   dormant work.

**Exit:** no closed receipt appears in a current queue; no intermediate proof
rung can be confused with the final disposition from the file header.

### P5 — Extract doctrine and export dead history to Backroom

**Goal:** remove fully obsolete prose from the product repository without
losing the reasoning.

1. Promote live authority/trust conclusions into invariants/contracts.
2. Promote still-live risk tests and the post-Sarah Effect Native falsifier
   into the challenge ledger.
3. Preserve a compact greenfield/Sarah-removal decision and all non-revival
   tombstones.
4. Build the first Backroom archive batch and manifest.
5. Push the Backroom archive.
6. Replace necessary inbound paths with minimal redirects; delete the remaining
   obsolete files from `openagents`.
7. Schedule a second deletion review for transitional redirects after 90 days.

**Exit:** the July 9 narrative and obsolete delegation prose are discoverable
in Backroom but absent from the normal `openagents` reading and search path.

### P6 — Automate freshness and link integrity

**Goal:** make this cleanup durable.

Add a documentation guard that fails when:

- an active/index/contract document hard-codes a master revision;
- more than one file claims to contain the current queue;
- a superseded or historical document is listed as executable in “Start
  here”;
- the checked-in current issue projection differs from generated/live issue
  metadata;
- a closed issue is called active in an active table;
- an active document revives physical Android as a gate;
- active subsystem text pauses persona-neutral voice;
- a receipt lacks snapshot/date, proof rung, and final disposition;
- an internal Markdown link is broken;
- a Backroom deletion lacks an archive manifest and commit;
- the master exceeds its size budget without an explicit reviewed exception.

Run the guard in the normal docs/CI verification path. The live-GitHub check
may generate a pinned artifact in connected environments; offline CI should
validate the artifact's schema and age rather than silently invent current
state.

**Exit:** reintroducing the concrete drift found by this audit fails before
merge.

## Proposed first changesets

Keep the implementation small enough to review and revert:

1. **Entry-point truth:** `README.md`, `issues/README.md`, minimal
   `IMPLEMENTATION_ROADMAP.md`, and the doc manifest.
2. **Durable-contract cleanup:** `OPERATING_MODEL.md` and
   `SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md`, with focused tests/links for any
   invariant-bearing change.
3. **Historical dispatch retirement:** delegation packet, CUT plan, CUT-27
   audit, parity baseline, and their inbound references.
4. **Master compaction:** extract historical blocks and duplicate receipts;
   leave one current issue/sequence table.
5. **Receipt normalization:** receipt index, final-disposition headers, and
   closed issue-source classification.
6. **Backroom batch one:** July 9 corpus and retired execution history, with
   archive/import and source-removal commits linked both ways.
7. **Freshness guard:** metadata, link, revision, queue, issue-state, and size
   checks.

Do not combine the master compaction, mass link moves, and cross-repo archive
in one commit. That would make it difficult to distinguish a broken link from
a changed decision and difficult to revert one without the others.

## Deletion and retention tests

A document may leave `openagents` only when all are true:

1. it is not current authority, a supported contract, a product promise, an
   invariant record, a receipt, a failure artifact, or a non-revival tombstone;
2. every still-binding conclusion has been promoted to its owning authority;
3. all internal and known GitHub inbound links are migrated or intentionally
   redirected;
4. its exact bytes and original path exist in a pushed Backroom archive with
   source provenance;
5. its removal does not make a proof rung, owner decision, or historical
   counterexample undiscoverable;
6. the documentation guard passes.

Never delete or rewrite merely for tidiness:

- `docs/transcripts/`;
- owner decisions and acceptance evidence;
- failure receipts and counterexamples;
- claim or authority contracts;
- formal/model notes tied to invariant changes;
- supported product guarantees and promises;
- behavior-contract tombstones needed to prevent accidental revival;
- teardown evidence before its promoted requirements are independently owned.

## Verification matrix

| Verification | What it proves |
| --- | --- |
| Link graph before/after each batch | No internal authority or receipt became unreachable |
| Generated active-document manifest | Every file has one class, owner, and disposition |
| Revision-pin scan | Active docs no longer cache a master revision |
| Current-queue uniqueness scan | Only one projection claims current work |
| Live issue comparison | Current issue projection does not silently drift |
| Closed/open language scan | Known closed leaves are not called active in dispatch docs |
| Voice/Android policy assertions | Two owner decisions cannot regress through stale prose |
| Receipt schema check | Intermediate history is retained but final disposition is obvious |
| Backroom checksums and source map | Exported history is exact, attributable, and recoverable |
| Master line/churn budget | Canonical direction is not becoming another receipt diary |
| New-agent reading test | README + master + claim protocol yields the correct next action |

The new-agent reading test should be explicit: give a clean agent only the Sol
README and follow its links. It must identify the same owner decisions, open
issue set, next dependency-ready work, proof rungs, and non-goals as the live
authorities without consulting superseded prose.

## Success measures

Within the first cleanup pass:

- zero stale revision pins in dispatch-capable documents;
- zero historical files presented as the current queue;
- one current issue/status projection;
- one indexed home for receipts and one for closed issue sources;
- master roadmap below the reviewed size budget or an explicit exception;
- every July 9 file classified for retained doctrine, in-repo evidence, or
  Backroom export;
- all Backroom exports have bidirectional commit provenance;
- all internal Markdown links pass;
- no contradiction on persona-neutral voice, physical Android, #8640,
  CUT-14, or CUT-25;
- a new agent reaches live authority in no more than two links from
  `docs/sol/README.md`.

Longer term, track active-authority word count, stale-status incidents,
unowned documents, unlinked receipts, master-roadmap churn per landing, and the
age of the generated issue projection. A successful cleanup does not merely
move files. It makes incorrect dispatch structurally harder.

## Final recommendation

Begin with P0 and P1 immediately. The entry indexes and active companion docs
currently expose known false state, so they are correctness defects. Retire the
Revision 25 delegation packet next. Then compact the master before performing
large receipt moves or Backroom exports; otherwise the archive operation will
be forced to guess which historical statements still carry authority.

Use Backroom aggressively for material that is truly dead, but only after
conclusion extraction and link inventory. Keep proof close to the product;
keep obsolete argument history out of the product's dispatch path. The desired
end state is not a perfectly fresh copy of every old plan. It is a small,
trustworthy current program surrounded by indexed evidence and a separately
preserved archive.

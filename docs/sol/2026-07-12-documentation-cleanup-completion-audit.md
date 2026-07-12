# Sol documentation cleanup completion audit

- Class: receipt
- Date: 2026-07-12
- Snapshot: `bab0932d4a` plus live GitHub and Backroom `d7993ef5`
- Issue: [#8743](https://github.com/OpenAgentsInc/openagents/issues/8743)
- Proof rung: repository/live/cross-repository policy proof
- Final disposition: **PASS — P0–P6 and the first-pass success measures are
  complete; continue the automated maintenance cadence**
- Dispatch: no
- Owner: Sol documentation cleanup

## Outcome

The ordered one-time cleanup is complete. SOL-DOC-01 through SOL-DOC-10
repaired the entry path and durable contracts, retired false dispatch,
compacted the master, indexed proof and issue sources, extracted retained
decisions/falsifiers, moved ten obsolete narrative files to Backroom in two
import-first batches, generated a complete document manifest, and made the
known drift classes fail offline before push.

This audit found no remaining `archive-candidate`, unclassified Sol document,
second queue owner, active revision pin, broken internal link, stale live issue
projection, or unreceipted archive deletion. It does not claim that historical
evidence should be continually rewritten or that every retained file belongs
in Backroom. The remaining duties are recurring freshness and review, not an
unfinished migration chunk.

## Current audited state

| Fact | Current result |
| --- | --- |
| Sol Markdown inventory | 93 paths; 93 unique generated rows; no incomplete row, including this completion receipt |
| Controlled classes | 1 authority, 2 Backroom exports, 17 contracts, 1 current-status ledger, 4 historical analyses, 3 indexes, 57 receipts, 1 redirect, 7 tombstones |
| Dispatch ownership | Only `docs/sol/MASTER_ROADMAP.md` |
| Archive candidates | Zero |
| Product issue projection | 17 issues; pinned artifact equals live GitHub after the explicit `area:docs` exclusion |
| Checked-in issue sources | 43 non-index files, each classified exactly once |
| Indexed evidence | 26 dated evidence rows before this completion receipt |
| Master budget | 634 lines of 800 allowed |
| Focused regressions | 17 passed, 0 failed |
| Archived payload | July 9: 9/9 hashes; July 10 diary: 1/1 hash, 769 lines, 41,044 bytes |
| Source deletion sets | Exactly nine paths at `b62ad88136`; exactly one path at `03135f5d61` |

## P0–P6 requirement audit

### P0 — classification and drift freeze

| Item | Result | Evidence |
| --- | --- | --- |
| P0.1 — machine-readable class/status/snapshot/supersession/dispatch facts | PASS | [`document-manifest-policy.json`](./document-manifest-policy.json) plus generated per-file status, snapshot, class, disposition, and dispatch fields; minimal active-file metadata was repaired without rewriting historical bodies |
| P0.2 — path/owner/review/inbound/issue/disposition manifest | PASS | [`document-manifest.json`](./document-manifest.json) includes SHA-256, inventory review date/trigger, repository-wide Markdown inbound links/count, issue links, owner, and disposition for every current file |
| P0.3 — bounded dispatch surface | PASS | Exactly one generated row has `dispatch: true`; live issues/claims remain external operational authority |
| P0.4 — stop master landing-diary growth | PASS | Master is 634 lines, historical landings point to receipts, and the 800-line guard fails growth without reviewed policy change |
| P0 exit — every file owned/disposed | PASS | 93/93 unique rows; zero incomplete or archive-candidate rows |

### P1 — entry path and concrete contradictions

| Item | Result | Evidence |
| --- | --- | --- |
| P1.1 — revision-independent Sol README | PASS | [`README.md`](./README.md) points to law, master, claims, contracts, issue index, and receipt index without caching a revision/count/queue |
| P1.2 — issue classification index | PASS | [`issues/README.md`](./issues/README.md) separates live sources, live receipt/plan coverage, closed proof, tombstones, and reference material |
| P1.3 — minimal historical redirect | PASS | [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md) is a non-dispatch redirect |
| P1.4 — operating/subsystem revision independence | PASS | Active revision-pin scan is empty |
| P1.5 — voice/Android/#8640/landed-state corrections | PASS | Policy assertions and current master/receipt state pass; physical Android is non-gating and persona-neutral voice remains a typed modality |
| P1.6 — invariant ownership before July 9 export | PASS | Retained authority rules point to repository invariants/contracts; the exact source is preserved in Backroom |
| P1 exit — coherent README→master answer | PASS | Executable bounded-reader receipt and regression pass in one hop |

### P2 — false dispatch retirement

| Item | Result | Evidence |
| --- | --- | --- |
| P2.1 — retire July 10 delegation packet | PASS | Exact diary is absent/denied in OpenAgents and preserved at Backroom `9c710a93` |
| P2.2 — replace inbound references | PASS | Master uses the immutable Backroom blob; cleanup uses current/archive receipts; no relative link targets the removed path |
| P2.3 — freeze CUT dependency plan | PASS | It is a contract with pinned historical status; current selection comes from master/live issues/claims |
| P2.4 — freeze CUT-27 readiness audit | PASS | It remains receipt evidence; mutable gate truth is reconciled through live issue/current receipt sections |
| P2.5 — demote parity/delivery state | PASS | Historical-analysis/contract classifications prevent current dispatch while retaining topology/capability evidence |
| P2 exit — no executable obsolete queue | PASS | Queue-owner and historical Start-here denials pass |

### P3 — master compaction

| Requirement | Result | Evidence |
| --- | --- | --- |
| Retain decisions/model/gates/laws/non-goals/current issues/order | PASS | Master retains all named sections and the bounded reader recovers them |
| Extract landing diaries/duplicated receipts/old sequences/Fable history | PASS | Evidence is linked from indexes/Backroom; no implementation diary remains in the master body |
| Under 800 lines | PASS | 634 lines |
| P3 exit — durable/current/pointer prose only | PASS | Generated policy, live equality, and size checks pass |

### P4 — receipts and closed issue sources

| Item | Result | Evidence |
| --- | --- | --- |
| P4.1 — receipt index | PASS | [`receipts/README.md`](./receipts/README.md) exposes dated snapshot, proof rung, and final disposition |
| P4.2 — ambiguous chronological receipts expose final truth | PASS | Indexed/final-disposition headers cover CUT-11/12/14/23 and later closure receipts |
| P4.3 — no unsafe mass move | PASS | Repository-wide inbound inventory exists; proof remains close to the product by explicit retention decision |
| P4.4 — classify every issue source | PASS | 43/43 non-index sources exactly once |
| P4.5 — closed sources are immutable/indexed | PASS | Closed/tombstone sections plus generated content SHA/snapshot/disposition rows provide the reviewed machine-readable equivalent of per-body snapshot headers without rewriting chronological evidence |
| P4 exit — closed/intermediate proof cannot select work | PASS | Closed/open collision, current projection, and receipt schema checks pass |

### P5 — doctrine extraction and Backroom export

| Item | Result | Evidence |
| --- | --- | --- |
| P5.1 — authority/trust conclusions promoted | PASS | Repository invariants, master laws, and operating contracts own them |
| P5.2 — risk tests/falsifiers promoted | PASS | [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md) owns the extracted tests |
| P5.3 — greenfield/Sarah non-revival retained | PASS | Compact owner decision plus seven tombstones remain in OpenAgents |
| P5.4 — archive manifests built | PASS | July 9 and July 10 Backroom-export manifests retain exact source facts |
| P5.5 — Backroom pushed first | PASS | `dec8ae52` preceded `b62ad88136`; `9c710a93` preceded `03135f5d61` |
| P5.6 — links migrated and obsolete sources deleted | PASS | Exact deletion sets are nine and one; current links resolve |
| P5.7 — redirect follow-up | PASS | Neither archive batch left a transitional source redirect; immutable Backroom URLs and OpenAgents manifests own discoverability |
| P5 exit — obsolete narrative outside product search path | PASS | Ten exact files are absent/denied in OpenAgents and hash-verified in Backroom |

### P6 — automated freshness and link integrity

| Guarded drift | Result |
| --- | --- |
| Active revision pin | PASS — active/binding scan empty |
| Multiple current queues | PASS — one canonical product projection owner |
| Historical executable Start-here target | PASS |
| Pinned/live product issue divergence | PASS — 17/17; the audit detected and reconciled AUDIO-1 closure |
| Closed issue in active projection | PASS — CUT-26 closure was detected and reconciled during SOL-DOC-10 |
| Physical Android revived as gate | PASS |
| Persona-neutral voice paused | PASS |
| Receipt missing snapshot/rung/disposition | PASS |
| Broken internal Markdown link | PASS |
| Backroom deletion missing manifest/commits | PASS |
| Master over size budget | PASS — 634/800 |
| P6 exit — concrete audit drift fails before push | PASS — offline guard plus 17 negative/positive regressions are wired into root test and main pre-push policy |

## Deletion and retention tests

| Test | Result |
| --- | --- |
| Removed file is not authority/contract/promise/invariant/receipt/failure/tombstone | PASS — selected rows were historical analyses with extracted owners |
| Every still-binding conclusion promoted | PASS — both manifests map each conclusion to current authority |
| Internal and known inbound links migrated | PASS — generated inbound graph plus internal-link guard |
| Exact bytes/original paths in pushed Backroom archive | PASS — 9/9 and 1/1 hashes; line/byte counts match |
| Proof rung/decision/counterexample remains discoverable | PASS — indexes, compact decision, challenge ledger, manifests, and Backroom notes remain |
| Documentation guard passes after deletion | PASS |

No receipt, failure, contract, decision, transcript, tombstone, or current issue
source was deleted by either batch. Git deletion receipts contain exactly the
ten manifested historical source paths.

## Verification matrix audit

| Verification | Current executable evidence |
| --- | --- |
| Link graph before/after | Generated repository-wide inbound arrays/counts; internal-link guard |
| Active-document manifest | Deterministic 93-row manifest and source-tree digest |
| Revision-pin scan | `validateRevisionPins` across active/binding documents |
| Current-queue uniqueness | `validateQueueOwnership` |
| Live issue comparison | schema/age artifact plus explicit `--live` equality |
| Closed/open language | issue-index set equality and closed collision denial |
| Voice/Android assertions | `validatePolicy` |
| Receipt schema | 26 pre-audit indexed rows with normalized headers |
| Backroom checksums/source map | July 9 root note; July 10 `d7993ef5`; permanent removed-path set |
| Master line/churn budget | 634/800 line check |
| New-agent reading | [`clean-agent receipt`](./2026-07-12-clean-agent-reading-receipt.md) and named regression |

## First-pass success measures

| Measure | Result |
| --- | --- |
| Zero stale revision pins in dispatch-capable documents | PASS |
| Zero historical files presented as current queue | PASS |
| One current issue/status projection | PASS |
| One receipt index and one closed-source classification index | PASS |
| Master below reviewed size budget | PASS — 634/800 |
| Every July 9 file classified/extracted/exported | PASS — 9/9 |
| Every Backroom export has bidirectional provenance | PASS |
| All internal Markdown links pass | PASS |
| No voice/Android/#8640/CUT-14/CUT-25 contradiction | PASS |
| New reader reaches live authority within two links | PASS — one hop |

## Commands and exact results

- `bun run check:sol-doc-manifest` — **OK**, 93 documents.
- `bun run check:sol-docs` — **OK**, 17 product issues and every offline
  policy family.
- `bun run test:sol-docs` — **17 pass, 0 fail, 37 assertions**.
- `bun scripts/check-sol-docs.ts --live` — **OK**, pinned/live equality.
- Independent manifest query — 93 rows, nine classes, one dispatch owner,
  zero archive candidates, zero incomplete rows.
- Independent inventory — 43 issue sources, 26 pre-audit evidence rows, 634
  master lines.
- Backroom recomputation — July 9 **9/9** hashes; July 10 **1/1**, 769 lines,
  41,044 bytes.
- Git ancestry — Backroom `dec8ae52`, `b9645456`, `9c710a93`, `d7993ef5` and
  OpenAgents `b62ad88136`, `c608527eda`, `03135f5d61`, `a2e3b64f3b`,
  `bab0932d4a` are ancestors of their pushed `main` refs.
- Git deletion receipts — nine paths and one path respectively.
- `git diff --check` — pass.

## Maintenance, not residual migration

- Refresh `live-roadmap-issues.json` from GitHub before its 168-hour maximum
  age and whenever the labeled product set changes.
- Regenerate `document-manifest.json` whenever Sol Markdown or repository-wide
  inbound Markdown links change.
- Keep main pre-push/root-test checks green; treat a failing snapshot, class,
  link, receipt, policy, archive, or size assertion as a correctness defect.
- Revisit retained historical analyses/redirects only on their generated review
  triggers. Do not create another archive batch merely to reduce file count.

No additional one-time cleanup issue is required by this audit.

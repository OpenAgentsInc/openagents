# July 9 doctrine extraction and Backroom archive manifest

- Class: backroom-export
- Date: 2026-07-12
- Source repository: `OpenAgentsInc/openagents`
- Source snapshot: `bf70f8922c`
- Destination repository: `OpenAgentsInc/backroom`
- Proposed destination:
  `archive/openagents-sol-docs-2026-07-12/july9/`
- Status: archive/import/link migration/source removal complete
- Backroom import: `dec8ae52`
- OpenAgents link migration and source removal: `b62ad88136`
- Backroom final bidirectional receipt: `b9645456`
- OpenAgents completed manifest: `c608527eda`
- Dispatch: no

## Purpose

This manifest proves that the surviving conclusions of the nine superseded
July 9 Sol documents have durable owners before their full narrative moves out
of the product repository. It pins exact source bytes, known inbound links,
destination paths, and removal conditions. SOL-DOC-06 does not modify the nine
candidate files; SOL-DOC-07 must push the Backroom archive before removing any
source. Backroom import is verifiably present at
[`dec8ae52`](https://github.com/OpenAgentsInc/backroom/tree/dec8ae52/archive/openagents-sol-docs-2026-07-12),
and its final archive note records the OpenAgents removal at
[`b9645456`](https://github.com/OpenAgentsInc/backroom/blob/b9645456/archive/openagents-sol-docs-2026-07-12/ARCHIVE_NOTE.md).

## Exact candidate set

| Source path under `docs/sol/` | SHA-256 at source snapshot | Durable owner after extraction | Proposed disposition after archive push |
| --- | --- | --- | --- |
| `2026-07-09-authority-trust-and-economics.md` | `7542a430723661933c63ae40541a34ef12049326f4b3e8ca00af7d5f27954de4` | Repository [`INVARIANTS.md`](../../INVARIANTS.md) authority/privacy laws; master implementation laws; operating model | Archive exact bytes, migrate remaining links, remove from `openagents` |
| `2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md` | `15b1b65d202000bc8e512ae63a0f3b2ed0be37a461811237f04982312d98fcac` | Master C0–C3 boundaries; claim protocol; receipt index | Archive exact bytes; migrate master/operating/voice-audit links; remove source |
| `2026-07-09-effect-native-strategic-importance.md` | `d4aef5e311086512396da26c525dab36c86aa1a41fb8e8734bce100949bf9a67` | Master decisions 4/17 and laws 6/8/21/22; Effect Native challenge row below | Archive exact bytes, remove source after manifest/cleanup links migrate |
| `2026-07-09-execution-sequence-and-critical-path.md` | `b7aef160b7f06989f7418ddae9f5baac901c92cc7d4e4d22d1c1e2f6872cb11e` | Master current execution order; live issues/claims | Archive exact bytes, remove source |
| `2026-07-09-greenfield-mobile-desktop-decision.md` | `59642115499232dcd6c88f3fc4a17979d7f3ea04723695a271e10198f6f3e7fb` | [`decisions/2026-07-10-greenfield-clients-and-sarah-removal.md`](./decisions/2026-07-10-greenfield-clients-and-sarah-removal.md) | Archive exact bytes; binding links migrate to decision record; remove source |
| `2026-07-09-issue-triage.md` | `d12d4c489134d0db0411bd1b529173a7a82aa357a7ec28bf0867b9b4de688c29` | Master live projection; issue-source and receipt indexes | Archive exact historical triage receipt, remove source after roadmap links migrate |
| `2026-07-09-risks-tensions-and-decision-tests.md` | `ffc800847808a173a4ceca0373ca8f6e8cc5d696875275633edf5aab42cd7c99` | [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md) | Archive exact bytes after falsifier extraction, remove source |
| `2026-07-09-roadmap-system-model.md` | `3f25a0e309638e9313340246318f5f2c3da1ed617808212565c7826c5945ce40` | Master product/authority model, identity/Sync/workroom contracts | Archive exact bytes, remove source |
| `2026-07-09-sarah-first-product-architecture.md` | `07f9153e8b0b4d516828166b98488b6696334f7f89fceff045828fcde4f2bfba` | Master Sarah removal/non-revival boundary plus compact decision record | Archive exact bytes, remove source |

The repository invariant ledger already owns the surviving rules for typed
external boundaries, semantic routing, distributed authority, secret/privacy
exclusion, named account custody, workroom grants, safe writeback, and receipt-
backed claims. This extraction changes no invariant merely to relocate prose.

## Known inbound links at SOL-DOC-06 start

| Candidate | Known non-candidate inbound documents | Migration in SOL-DOC-06 / SOL-DOC-07 |
| --- | --- | --- |
| authority/trust/economics | cleanup plan | Replace cleanup-plan path with this manifest/Backroom receipt after import |
| Codex parallelism/Sarah cutover | `docs/sarah/2026-07-09-pipecat-voice-infra-audit.md`, cleanup plan, master, operating model | Master already owns C0–C3; replace the remaining historical links with manifest/archive receipt before removal |
| Effect Native importance | cleanup plan | Falsifier extracted now; replace cleanup path after archive import |
| execution sequence | cleanup plan | Current order already in master; replace cleanup path after archive import |
| greenfield decision | `docs/RETIRED.md`, `docs/effect-native/README.md`, Khala Code retirement promise, cleanup plan | Three binding consumers migrate to compact decision in SOL-DOC-06; cleanup path migrates after archive import |
| issue triage | cleanup plan and master historical pointer | Live/status ownership already replaced; migrate remaining pointers before removal |
| risks/tests | cleanup plan | Falsifiers extracted now; replace cleanup path after archive import |
| roadmap system model | cleanup plan | Current authority model already in master; replace cleanup path after archive import |
| Sarah-first architecture | cleanup plan | Removal/non-revival decision already in master/decision record; replace cleanup path after archive import |

Re-run repository-wide inbound-link discovery immediately before removal. This
table is a point-in-time inventory, not permission to ignore new links.

## Retained falsifier extraction

The live Challenge Ledger now owns the still-useful July 9 tests:

- Effect Native must reduce semantic duplication and second/third-renderer
  lead time rather than become a framework queue;
- unified orchestration must keep owner, custody, capacity, account, cost, and
  fallback rails legible;
- bounded receipt projections must answer what happened, proof, cost, and next
  action without raw-log access;
- issue/commit velocity must produce complete loops, deletion, live receipts,
  and fresh authority rather than integration/documentation debt;
- temporary named compatibility adapters require a neutral replacement,
  owner, expiry, and deletion gate.

## Required Backroom archive receipt

SOL-DOC-07 creates
`backroom/archive/openagents-sol-docs-2026-07-12/ARCHIVE_NOTE.md` and the
`july9/` payload. The note must contain:

- source repository and exact source commit;
- original path, archive path, SHA-256, byte count, and line count per file;
- retained conclusion and authoritative replacement mapping;
- known inbound links migrated;
- non-production/non-dispatch statement;
- Backroom import commit;
- corresponding `openagents` removal/redirect commit once pushed.

## Cross-repository execution order

1. Refresh clean `main` in both repositories and recompute every hash.
2. Create the Backroom payload and `ARCHIVE_NOTE.md` with exact provenance.
3. Validate payload hashes byte-for-byte against `openagents`.
4. Commit and push Backroom `main` first.
5. Replace remaining internal links with the compact decision, current
   authority, or pushed Backroom archive receipt.
6. Remove only the nine manifested source files from `openagents`; do not touch
   transcripts, receipts, contracts, owner decisions, or tombstones.
7. Run link/candidate/deletion guards, commit and push `openagents` `main`.
8. Amend the Backroom archive note with the source-removal commit if necessary
   and push that receipt.

If Backroom import/push fails, source deletion does not begin. Git history is
not the archive receipt; the destination must be discoverable and pushed.

## Landed cross-repository receipts

- Backroom exact-byte import: `OpenAgentsInc/backroom@dec8ae52` (pushed to
  `main` before source deletion).
- OpenAgents link migration/source removal:
  `OpenAgentsInc/openagents@b62ad88136`.
- Backroom final note containing both receipts:
  `OpenAgentsInc/backroom@b9645456`.

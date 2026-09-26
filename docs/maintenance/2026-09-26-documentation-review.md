# Documentation review, September 26, 2026

[Issue #9692](https://github.com/OpenAgentsInc/openagents/issues/9692) tracks this
pass. It covers the documentation tree except `docs/transcripts/`, which is
unchanged (Git tree `3ba937e906a0c68ae4c2eb184e81599eb8749b8f`). The starting inventory contained 412 Markdown files and 100,132
lines outside that archive. Review combined a file/link inventory, domain
reviews, current-source checks for suspect claims, and editorial consolidation.
It is not a new benchmark, a production certification, or an independent
revalidation of every historical measurement.

## Canonical navigation

- [Documentation index](../README.md): where to start by reader goal.
- [Master roadmap](../roadmap.md): the complete cross-project direction,
  dependencies, priorities, implementation boundaries, and completion evidence.
- [Catalog](../catalog.md): every non-transcript Markdown document, grouped by
  domain and labeled by document role.
- [Glossary](../glossary.md): shared definitions and scoped implementation status.
- [Maintenance contract](../documentation.md): which document owns each type
  of claim and how to move or retire material without changing evidence.

The master roadmap brings the coding product, native clients, CoderOS,
knowledge, programs, plugins, agent labor, Bitcoin settlement, Nostr, decision
services, model implementations, Gym, automation, media/device work, managed
hosting, and world interfaces into one outcome map. Domain designs retain their
technical detail. The suite's M-package tracker remains the delivery ledger;
this does not create a competing list of issue owners.

## Disposition by area

| Area | Review and disposition |
| --- | --- |
| Root and top-level surveys | Add a documentation landing page and master roadmap. Preserve the earlier transcript-derived roadmap under `history/`. Mark older delegation, compute, optimization, game, and earnings surveys as historical where appropriate. Update dependency and extension navigation. |
| Coder design, guides, runtime | Complete the three domain indexes; distinguish current contracts from earlier rebuild proposals. Update the migration tracker for delivered context/host foundations and stopped Microcoder acceptance. Preserve separately owned Beat Fable plans and ongoing study reports. |
| Terminal-Bench | Replace the chronological README with concise current-source navigation and bounded comparison summaries. Keep its old text as a dated snapshot and add a complete report catalog. Correct stale “running” and open-issue claims without changing study outcomes. |
| Gym | Create a current index and measurement index. Replace the old root survey with a compatibility entry point and preserve the full survey under `history/`. Keep ledger, identity, gate, and record contracts separate from benchmark reports. |
| Decision models and services | Correct the API product's stale implementation summary and relay status. Identify the old Rust SDK proposal as historical; remove the missing `todo.md` reference. Correct SDK credential behavior and broken measurement links; retain deferred image, confidential-inference, and SDK-language gates. |
| Kev, Lev, and Laya | Add measurement indexes, correct opt-in conformance instructions, and separate local availability from workload admission. Correct Lev closure, calibration, and hardware-cost language without rewriting recorded measurements. |
| Nostr and protocols | Add navigation and reconcile authored-NIP inventory and XP coverage. Correct free-labor status, host-versus-validator boundaries, and retention descriptions. Keep normative specifications in `nips/`; this pass does not rename protocol identities. |
| Deployment | Update migration/configuration references and Debian commands. Remove rollback advice that could erase migration-ledger history. Distinguish operator prerequisites from completed release evidence. |
| General agents, markets, and extensions | Correct bounded free-labor implementation status, keep paid settlement and full cross-operator fulfillment separate, and point integration plans to current public contracts. |
| Voyager, Minecraft, Verse, and game history | Correct the obsolete “no integration” description, distinguish working Voyager/arena components from broader product goals, and retain explicit platform and economic limits. |
| Audits and historical evidence | Add an audit index. Preserve dated findings and their remediation rather than treating every old finding as an open issue. No raw measurement, prompt, or study artifact is rewritten for presentation. |
| Transcripts | Excluded from writes, moves, title edits, and cleanup. Existing links remain usable. |

## Consolidation and deletion choices

The two substantial root duplicates were the old roadmap and Gym migration
survey. Their complete content remains available under [history](../history/README.md),
with relative links adjusted and clear dated-status banners. The former
Terminal-Bench README is likewise retained beside its report catalog. Current
indexes no longer need to reproduce those long histories.

Other dated reports retain distinct evidence or rationale. They are not deleted
because their implementation is old or an issue closed. Frozen prompts, raw
artifacts, and source-linked reports keep their original location. No transcript
or retained raw evidence is removed. This avoids turning organization into
loss of the record used to interpret an earlier claim.

## Scope and verification

The related code work uses targeted package tests and Clippy; full workspace
verification is reserved for releases and is not a documentation gate. This
pass checks local file links and changed heading anchors, indexes, Markdown
diff hygiene, and unchanged transcript paths. The completed inventory check
resolved 4,926 relative file links and 515 local heading links across the non-transcript corpus; the
only unresolved file reference was the literal prompt example below. Raw
retained logs keep their original whitespace and bytes. The exact captured Claude prompt
contains an illustrative `file.md` link; it remains literal source material,
not a documentation target to invent or rewrite.

No model, benchmark, candidate-check, or platform acceptance campaign was
launched for this review. Prior repository attempts are retained under their
[stopped acceptance record](../coder/verification/2026-09-26-repository-adapter/README.md).
Their missing independent checks stay missing. The separate Claude-owned study
and tuning work is left to that contributor.

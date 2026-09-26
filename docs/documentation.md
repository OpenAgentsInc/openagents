# Documentation maintenance

Use the [documentation index](README.md) to navigate, the [master roadmap](roadmap.md)
to understand the complete direction, and the [catalog](catalog.md) to locate
individual documents. The [glossary](glossary.md) owns shared terminology.

## Assign one job to each document

| Kind | Responsibility | Update rule |
| --- | --- | --- |
| Index | Explain a directory's purpose and link its current sources. | Keep it short; link detailed results instead of copying them. |
| Runtime guide or runbook | Explain supported behavior, prerequisites, commands, failure modes, and limits. | Change it with the implementation. A prototype or fixture needs an explicit boundary. |
| Design or roadmap | Explain a goal, alternatives, dependencies, and evidence required for completion. | Distinguish implemented foundations from proposed outcomes; link the issue owner. |
| Protocol | Define a wire or artifact contract. | Keep specifications in `nips/`; use `docs/protocol/` for support and integration assessments. |
| Measurement or verification record | Preserve the tested revision, inputs, complete outcomes, uncertainty, and retained evidence. | Add a correction with provenance; do not rewrite old results as current ones. |
| Audit or historical survey | Preserve why an earlier decision was made and what it observed. | Mark its date and scope; link current guidance rather than silently modernizing history. |

Implementation, deployment, interoperability, comparative benefit, acceptance,
and payment are separate claims. Closing an issue can record a negative result
or a deferral. Use the closing evidence, not the issue color, to describe what
shipped. Model-reported completion is not independent verification.

## Consolidate without losing evidence

1. Choose a canonical owner before merging duplicate guidance. Cross-project
   priorities belong in `roadmap.md`; domain detail stays with its runtime or
   design owner.
2. Move a superseded survey to `history/` when its original reasoning remains
   useful. Keep a small entry point at a widely linked old path, or update all
   readers and inbound links together. Preserve useful heading anchors.
3. Keep dated raw logs, digested artifacts, frozen prompts, and study records
   in place. Their exact bytes and relative paths may be part of verification.
   Link corrections separately from the retained record.
4. Delete only a redundant copy with no distinct evidence or rationale after
   checking references. Age alone is not a deletion criterion.
5. Update the directory index, catalog, glossary, and owning roadmap when the
   change affects navigation, a term, or a delivery status.

The transcript archive has its own index and history. The September 26 pass
excludes all transcript edits. Private repositories are reference material;
public guides must not publish their code, credentials, prompts, or private
endpoints as though they were public implementation instructions.

## Check the documentation change

Check relative files and directories, heading links affected by renames,
references from source or embedded-document loaders, and retained artifact
manifests. Review the final Markdown diff for stray patch markers and empty
list items. If an embedded path changes, test its affected consumer.

Documentation-only edits do not need Rust tests or the release matrix. Rust
behavior changes use the affected package's focused checks under
[verification.md](verification.md). Full release verification must not block
ordinary development, unrelated issues, or documentation publication.

## Keep ownership visible

Claim an issue before overlapping implementation. A catalog entry, planned
milestone, or linked experiment does not authorize a run. Active studies retain
their own owner and frozen protocol; documentation cleanup must not restart
them, inspect sealed outcomes early, or alter their acceptance rule.

For the current suite effort, use the [migration tracker](coder/migration-status.md).
For source coverage and changes made by this pass, use the
[review record](maintenance/2026-09-26-documentation-review.md).

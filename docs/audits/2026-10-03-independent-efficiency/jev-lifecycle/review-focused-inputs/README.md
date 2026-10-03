# Applied-source review inputs

These are the 12 frozen inputs for the second, development-only review round.
Each `R01.json` through `R12.json` is copied byte-for-byte from the input prepared
before that round. The [manifest](manifest.json) records their hashes, source
commits, candidate bindings, and common extraction rules. The
[validation record](validation.json) binds this public manifest and preserves
the hashes of the original private records. Private artifact paths are omitted;
case identities link to the [public case manifest](../review-cases.json).

The inputs contain final candidate source. For the ATIF and Jev families, the
builder overlays hash-verified candidate payloads on the pinned base. For the Gym
family, it applies the retained patch to the exact base files after `git apply
--check`. It executes no candidate code. Unchanged dependencies come from the
same base commit.

## Common source selection

Every variant in a family uses the same path and declaration rules:

- **ATIF:** the complete production log implementation and library file, selected
  CoderBench reader and grading functions and types, and the public trace guide.
- **Jev:** the complete answer and client implementation files, library module
  and export declarations, and the pinned selected-answer and score contract.
- **Gym:** the Microcoder implementation through `read_all`, including merge
  helpers; source, run, and catalog types; catalog loading and refresh; claim
  filtering and grouping; and the named relevant documentation sections.

Each span records its final-file hash, line bounds, exact text, and text hash.
Production functions are selected through their closing brace, without cutting
their bodies to meet a budget. The largest state is 63,202 bytes; every state is
within the 65,536-byte limit. Validation confirms that the selected files include
all final changed production lines, excluding test modules. This does not imply
that every dependency or every unchanged production line is included.

Test bodies, test results, checker text, and candidate labels are excluded from
every outbound state. Each input names its omitted source. The request builder
uses `task`, `source_commit`, `source_context`, `public_contracts`, and `omissions`
from these files, then adds the frozen questions. Request receipts retain the
exact transmitted bytes separately.

## Limits

These are already exposed development cases. Selection of applied-source context
and narrower questions followed the first round's failures. The follow-up changes
both context representation and question granularity, so it cannot isolate their
effects or establish performance on unseen tasks. It does not execute tests or
demonstrate that an agent can repair a flagged patch. Missing source is not proof
of a defect. See the [follow-up protocol](../review-followup-protocol.md).

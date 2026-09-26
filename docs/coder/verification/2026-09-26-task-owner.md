# Local task ownership and evidence verification

Status: targeted owner and evidence acceptance passed, September 26, 2026.
Issues: [#9673](https://github.com/OpenAgentsInc/openagents/issues/9673),
[#9675](https://github.com/OpenAgentsInc/openagents/issues/9675), and
[#9676](https://github.com/OpenAgentsInc/openagents/issues/9676).

This is a new public Rust implementation of the migration contract. It uses the
existing `coder-boundary`, `supervise`, ATIF, capability trust, and independent
verification engine. No private Coder source, prompts, endpoints, or histories
were copied. The [runtime guide](../runtime/task-owner.md) defines the supported
local OS-user boundary and explicit limits.

## Initial focused evidence

The [initial task tests](2026-09-26-task-owner/focused-initial.log) passed 33
cases, and the [initial process tests](2026-09-26-task-owner/cli-initial.log)
passed nine. These logs precede the protected-suite hardening and Microcoder
adapter integration; they are not a pass for those later changes. No paid model
requests were required for these fixtures.

| Contract | Retained test behavior |
| --- | --- |
| Admission | Inert submission, explicit closed grants, exact intent/revision/source pins, unsupported bounds refused before dispatch. |
| Ownership | A second owner is refused; a detached owner finishes after the starting client exits. |
| Crash recovery | Faults at admission, intent, dispatch, and result boundaries preserve uncertainty. Killing a real owner does not repeat its persisted effect. |
| Cancellation | A command receipt acknowledges requested cancellation while execution is still running; only supervised cleanup establishes stop. |
| Source and write scope | The child cannot read a fixture outside its read scope or write outside the granted workspace. A changed source digest refuses execution. |
| Evidence | A new CLI process reconstructs the same task and original transcript. A 2,000-step log pages through stable prefix-bound cursors. |
| Missing evidence | Missing, corrupted, replaced, and torn transcripts remain explicit. Missing cost is null/unknown. |
| Artifacts | Retained bytes remain readable after workspace deletion. Parent links, leaf links, hard links, and escaping paths refuse reads. Identical content-addressed manifests are reusable across tasks. |
| Context and corrections | Scoped instructions are retained in ancestor order. Corrections preserve original intent and effects while disputing stale completion. |
| Independent checks | False-green execution, absent suite output, stale identities, changed candidates, and missing check manifests cannot become verified success. |

## Review findings and corrections

Independent review found defects beyond the first passing fixtures. Those
findings are part of this evidence record, not erased by a green test count:

- Reusing an identical artifact manifest originally failed its exclusive create.
  The writer now verifies and reuses the exact retained bytes.
- Admission originally compared equal workspace paths only. It now reserves
  overlapping trees for unresolved execution and running checks, in both
  directions: starting an executor and starting a checker.
- A later filesystem read originally supplied retained artifact bytes without
  comparison to the exact candidate entry. Per-file content and symlink-target
  comparisons now bind retention to that snapshot, including an ABA change.
- A check manifest outside the workspace did not by itself establish an
  independently sourced suite. The protected-suite correction pins the external
  executable and accepts only the candidate digest as its data argument.
- A journal's outer `passed` field was insufficient without typed check evidence.
  Replay must validate the protected plan, exact candidate, complete check set,
  suite identities, and derived verdict before reconstructing a pass.

The [protected-check tests](2026-09-26-task-owner/protected-checks.log) pass
37 task cases, including refusals for candidate-owned checker commands,
changed external programs, absent typed evidence, missing check coverage,
truncated output, and mismatched plan, suite, or candidate identities. The final targeted checks below cover the integrated local implementation. These tests establish
agreement with the protected operator suite, not universal correctness or an
improvement over the negative #9584 studies.

## Final targeted checks

The [final task tests](2026-09-26-task-owner/focused-final.log) pass 38 cases,
including the additional cancellation-versus-result persistence regression.
The [final process tests](2026-09-26-task-owner/cli-final.log) pass seven inbox
CLI cases and two real detached-owner/crash cases.
A cancellation acknowledged while the host seals its result cannot become
finished execution; the original observed process result remains available.
[Strict all-target Clippy](2026-09-26-task-owner/clippy.log) passes for coder,
knowledge, and Microcoder.

The local Microcoder synthetic fixture retains its exact
[grant](2026-09-26-task-owner/microcoder-repository/grant.json),
[task](2026-09-26-task-owner/microcoder-repository/task.json),
[full native and host trace](2026-09-26-task-owner/microcoder-repository/trace.atif.jsonl),
and [reconstructed view](2026-09-26-task-owner/microcoder-repository/view.json).
This demonstrates the common host boundary, not a paid model's coding ability.
The adapter issue #9674 remains open for its remaining acceptance; #9676 also
remains open for frozen knowledge delivery and source-lineage enforcement.

No full workspace gate was run. The development policy now requires relevant
targeted checks and reserves the full matrix for full releases. Earlier logs
retain their original coverage and are not relabeled as final evidence.

## Limits

The evidence is from macOS with an enforcing filesystem boundary. Unsupported
boundaries refuse execution; no Linux runtime result is inferred from a Mac
pass. Stable local filesystem locks fence cooperating owners in one store.
Separate stores, network filesystems, remote ownership, hostile same-user file
rewrites, and independent external workspace writers have different trust
requirements.

An uncertain process is never automatically restarted or declared quiescent.
A successful executor is not a passed independent check. A passed check is not
integration or buyer acceptance. This slice does not complete mobile control,
Nostr sessions, CoderOS packaging, or the overall suite migration.

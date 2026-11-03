# Coder qualification, 2026-11-03

This closes the pinned protocol gate left open by the
[first follow-up](../coder-followup-2026-11-03/README.md), consumes its reserved
continuation case, evaluates a genuine repository issue, and adds Microcoder
request-attempt observations. The historical repair is **not release-qualified**;
the production changes here concern observability and generated protocol data.

## Protocol and production changes

The existing local cache supplied protoc **29.3** and Python protobuf **5.29.3**.
The first pinned check exposed `proto/descriptors/owned.pb` drift from the new
v1 schemas. Regeneration repaired that omission and includes the new v2 schema.
After rebasing onto `9854c952af`, regeneration also retains the concurrent
Verse OS desk-control schema in the shared descriptor.
The final full gate passes: all seven modules compile, 21 Nostr source packages
pass pins/hashes, all consumer profiles and compatibility/migration checks pass,
and **30 conformance tests** pass. See [protocol.txt](protocol.txt).

Microcoder now records configured gateway/model, requested effort, outcomes,
available token counts, provider-reported cost and a separately named static
rate-card estimate. Optional-field presence survives gateway normalization,
worker transport and the session writer. Unknown is null, not zero. The model
is explicitly a configuration observation, not a resolved upstream identity.
The session writer rebuilds a metadata whitelist rather than copying arbitrary
payloads. [Observability v2](../../protocol/coder-observability-v2.md) defines
scope and limitations; v1 Codex snapshots are unchanged.

Validation: **421 Coder checks**, **55 gateway/presence checks**, **60 gateway-plan
checks** and **83 Microcoder checks** pass (these suites overlap; do not sum them).
The worker also builds and starts natively. Book structure and local links pass.
The unchanged availability policy's 89/89 mutation result remains qualified at
`e1e427275f`; it was not rerun. The repository-wide tools gate still has the same
eight unrelated missing-reason files and one stale port-list entry; details are
in [tools-check.txt](tools-check.txt).

## Reserved continuation case

Case 3 of repository-v2 was run once with Microcoder, with a fresh `CODER_HOME`.
The first turn diagnoses `unique_count` without editing; a fork then receives
the current objective, counting distinct byte values including zero and UTF-8
bytes. The obsolete sorting proposal must not control the repair.

The diagnostic turn finished in **13,264 ms**. Its logged commands were reviewed:
listing, finding, reading and Git-history inspection only. The repair turn
finished in **65,208 ms**, and external acceptance passes. The final source is a
256-entry byte-membership table, not a sort. Original AGENTS.md remains intact.
The two transcripts contain **15 v2 attempts** (4 diagnosis, 11 repair), with
configured `google/gemini-3.8-flash` and requested effort `low`. Missing cache
details and provider charges remain null. See [continuation/](continuation/).

This case is now consumed, public development evidence. Future held-out
measurements need a fresh case. This is one short continuation, not a long
conversation stress test. The harness did not take a byte snapshot between
turns; the no-edit diagnosis assessment rests on its reviewed tool transcript.

## Actual issue #12720

[Issue #12720](https://github.com/OpenAgentsInc/openagents/issues/12720) was already
fixed on main by `b499b83f43`. The evaluation therefore uses its actual pre-fix
repository commit **a331a9cfde**, without manufacturing a new bug on main.
The driver is a frozen executable built from **e1e427275f**. Its binary digest,
full snapshot commit and protected-check digest are in
[issue-12720/receipt.json](issue-12720/receipt.json).

Before inference, an independent external checker was frozen and hashed. It
rejects quoted numbers in all ten numeric fields and verifies refusal preserves
the last valid document. It fails on the original snapshot. The model received
the issue description and scope, not the checker or the known fix. Existing
checks had to remain verbatim; their exact original suffix remains present.
Only `aethyr/forge/doc.coax` changed in the evaluation worktree.

One Codex task, with a **180-second budget**, finished in **100,717 ms** and 15
tool starts. It used resolved `gpt-6.1-sol`, effort `xhigh`. The independent
checker and imported checks pass **98/98**. The model also added checks for 70
mistyped-field combinations and numeric compatibility. No human repaired its
output. However, the independent root-only mutation check reports **469 mutants,
395 killed, 74 survivors**. None were marked equivalent. Overall outcome:
**functional acceptance passed; release quality gate failed**.

Review of [reviewed.diff](issue-12720/reviewed.diff):

- The JSON-number guard reaches version, revision, fork revision, moved x/z,
  shaped yaw/scale and all three placed coordinates/orientation fields. The
  reader still publishes only its completed replacement, preserving rollback.
- The patch keeps existing numeric parsing and bounds, and preserves existing
  checks verbatim. Its added positive tests retain exponent/fraction spellings
  where placed values are stored as text.
- The remaining mutation holes prevent release qualification. They are not
  74 proven newly introduced defects: this historical reader already had weak
  checks, and no separate baseline mutation count was measured in this run.
- The model's read-only helper audit named other numeric readers as candidates.
  Those were not independently qualified here, and the known main-branch fix
  already audited related readers. They are not new confirmed findings.

The diff is retained as evaluation evidence and is **not applied over today's
existing fix**. No issue was closed or external comment posted. This public
historical task can be contaminated by prior knowledge or repository history;
it is not a private holdout or a broad coding-success comparison.

To reproduce the independent check, create an isolated worktree at the recorded
snapshot, apply `reviewed.diff` with `git apply --unidiff-zero`, and point the first `#load` in the recorded
`oracle.coax` at that worktree's `aethyr/forge/doc.coax`. The recorded original
oracle retains the absolute execution path and pre-inference hash. Run the
qualified compiler on it, then mutate the module with `--library --root-only`.
The bounded Coax runner is `coder/tests/issue.coax`; its options are documented
in the [pilot guide](../../coder/repository-pilot.md#real-repository-issue-replay).

The compiler remains SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
The current production worktree began at `e1e427275f`; live continuation used
the delivered observation changes before commit. `sources.sha256` records the
final relevant sources. Costs were not inferred from token totals.

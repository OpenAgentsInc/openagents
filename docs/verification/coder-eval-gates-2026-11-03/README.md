# Coder evaluation gates, 2026-11-03

The independent collector qualified two fresh, deliberately different cases.
These single runs establish bounded task success, not an engine comparison or
completion of the current issue. No evaluated product patch was merged.

| Case | Engine / observed configuration | Supervisor time | Independent result |
| --- | --- | ---: | --- |
| [Current #12719 policy prototype](current-progress/) | Codex, thread/start reports `gpt-6.1-sol`, requested effort `xhigh` | 64,511 ms / 180,000 ms | Acceptance passes; 13/13 mutants killed, zero survivors |
| [Eight-turn continuation](conversation/) | Microcoder, Vertex attempt configuration `google/gemini-3.8-flash`, requested effort `low` | 120,840 ms / 600,000 ms | Seven read-only pairs unchanged; final acceptance passes; 15/15 mutants killed, zero survivors |

Both [receipts](current-progress/qualification/receipt.json) and the
[conversation receipt](conversation/qualification/receipt.json) require engine
exit zero, budgets, protected bytes, checkpoints, independent acceptance,
zero-survivor mutation, saved artifacts and no equivalence annotations.

The current issue task intentionally requests only a new Coax admission-policy
prototype: matching modern revisions ignore client clocks; the explicit
benchmark legacy rule permits base -1 only at empty revision 0. Server wiring,
SQL migration, browser revision handling and end-to-end races were outside this
slice and remain unresolved. The task, external oracle, protected seals, raw
transcript, model source and reviewed patch are retained. Git status after the
run showed only the newly created `coax/web/api/progress-admission.coax`;
existing server/migration/instruction files were unchanged. The prototype is
stored here as evidence only. This does not close #12719.

The continuation has seven read-only discussions followed by one implementation
turn. It rejects obsolete modulo wrapping and a stale zero error sentinel,
retains inclusive bounds and -1 for invalid intervals, and preserves the
original regression. Independent acceptance enumerates endpoint pairs -2…65
and counts valid IDs by a separate loop. All eight transcript hashes match
`transcripts.sha256`; all seven before/after seals match. The final fixture
contains only AGENTS.md and src.coax. Final implementation removes a redundant
condition after an intermediate mutant survives; it does not exempt the mutant.

`observability.json` files extract only existing raw model/usage/attempt events.
The conversation records 23 request attempts. Provider cost and unavailable
usage details remain null; card estimates are not provider invoices. Codex
thread totals are cumulative and must not be summed across snapshots. Requested
effort is not a claim about provider-executed reasoning. These differently
scoped cases support no relative cost/speed/quality ranking.

Source baseline: `2d02e38d98` (full identity in source manifest). Qualified
compiler SHA-256:
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
The live runs used the initial collector/process implementation. Subsequent
changes add separate stderr capture with saturation refusal, preserve aggregate
conversation timeout/cut flags, accept the compiler's singular `1 check` footer,
and add invalid-chain regressions. The live runs were not repeated: both had
successful exits and no observed timeout/cut; their frozen evidence remains
unchanged. The final-source manifest identifies the shipped implementation,
not an assertion that the live runs exercised every later collector branch.

Validation completed in this worktree:

- Qualification authority policy: 26 mutants, 26 killed, zero survivors, no
  equivalent annotations. This earlier passing run was not repeated unchanged.
- Final collector: 149 checks pass, including footer and invalid-chain cases.
- Native collector smoke: positive receipt plus failed engine, changed protected
  bytes, unequal snapshots and failed acceptance all produce the expected status.
  Its retained fixtures show their final state after all refusal cases, not the
  earlier passing source bytes. Passing/refusal receipts and command logs are
  in [collector-smoke](collector-smoke/).
- New pin-policy unit: 27 checks pass; independent Python reference fixtures:
  six pass. Native successful stdout matches reference byte for byte after
  refreshing a pre-existing stale generated chapter index. All 48 vector generators regenerate matching bytes within the ten-minute
  aggregate cap; output is recorded in `pins-regeneration.txt`.
- Coder and Bitcoin book structure, exact listings and local links pass.
- Full pinned protocol check passes, including published compatibility and
  consumer profiles, after adding qualification v1 and regenerating owned.pb.
- Synthetic pixel oracle: accepts matching albedo and refuses wrong colour
  and absent layer; no private assets read.
- Tools-check still refuses five non-exempt asset generators. The
  [audit](../../coder/tools-check-audit.md) and root NEEDS_OWNER.md record the
  dependencies and pending owner choice. No fake reasons or waiver applied.

The collector assumes a trusted supervisor/evidence directory, protects an
explicit file list and observes bytes at boundaries. Manual scope/test audits
supplement the receipt; it is not continuous filesystem isolation. Absolute
paths in frozen logs/seals name the original worktrees and are not portable
replay paths. Use fresh worktrees and regenerate seals with the
[operating guide](../../coder/evaluation-qualification.md), rather than editing
this historical evidence. No credentials or private asset inputs are included.

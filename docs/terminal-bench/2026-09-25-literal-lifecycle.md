# Literal artifact lifecycle correction

The first literal artifact checker has a synthetic false alarm: it interprets
`Write scratch.txt as an intermediate file, then delete it before finishing.`
as a requirement that `scratch.txt` still exist at completion. Correct cleanup
therefore causes a failure call. This is an extraction error, even though the
metadata observation is accurate.

[#9648](https://github.com/OpenAgentsInc/openagents/issues/9648) corrects this
before the reserved 90-candidate confirmation starts. Its waiting launcher was
stopped, and the job inventory confirmed zero generated candidates. The original
protocol, preparation, plans, and stopped-launcher record remain retained. No
official outcome from the reserved groups informed the correction.

## What changes

The extractor abstains on explicitly temporary, intermediate, or transient
artifacts. It tracks earlier output declarations and removes their obligations
when a later instruction deletes, removes, or relocates the path or its parent.
It also reads cleanup commands in fenced blocks. Ambiguous pronouns conservatively
retire earlier obligations instead of guessing which output must remain.

Cleanup of an unrelated path, including an explicit path outside the working
directory, does not erase a separate final-output obligation. An output declared
after cleanup remains eligible. A moved path does not become an inferred new
output requirement: unsupported wording abstains.

The same correction distinguishes an output requested now from one that the
requested program would create later. For example, `Write a program to cache
later.csv` and `Write a program generating later.csv` do not require `later.csv`
to exist before that program is run. `Write a script to result.txt` still requires
the script itself. Function/method descriptions, purpose clauses, and the
supported gerund descriptions abstain.

This is a conservative grammar, not a natural-language proof system. It can miss
valid obligations and can still misunderstand unsupported wording. These checks
remain opt-in, fail-or-unknown evidence with no authority to certify completion,
stop a running agent, or reverse an edit. The original extractor and all runtime
policies remain unchanged.

## Validation and evidence

The correction uses synthetic language controls and the already-opened
[72-candidate development population](2026-09-25-literal-artifact-checks.md).
That replay is not another held-out accuracy estimate. The original official
labels, candidate identities, and reproduced-review predictions remain fixed.

The corrected binary is built from `24e7864537`, SHA-256
`99f21894b2ed16343545d1eb4a0e76f8ddbcd9af6ff8caab2d8ab0654a4eb9aa`.
Validation records establish:

- Eleven live CLI lifecycle controls reproduce nine false alarms from the old
  binary and none from the corrected binary. Both versions inspect the same
  correctly finished synthetic workspace; its contents remain unchanged.
- Five existing live controls still pass. All 12 ordinary contract plans remain
  byte-for-byte identical, preserving the original extractor.
- All 72 restored development candidates yield their previous literal calls:
  three failures, all officially failed, and 69 abstentions. No check is
  unavailable. Literal recall remains 3/11; OR with the unchanged earlier
  detector remains 7/10 precision and 7/11 recall. No additional accuracy claim
  follows from replaying opened labels.
- Replay uses zero model calls, 50.917 seconds of summed candidate process time,
  and a 0.642-second median including restore and container setup. This timing
  difference is not a controlled speed comparison with the earlier replay.
- The ten focused Rust tests pass. Formatting, strict Clippy, feature Clippy,
  default tests, and feature tests pass across the retained scoped gate runs.
  The first final feature run hits an unrelated Claude streaming test failure
  after that test passed in the default run. The complete feature phase passes
  on retry with `RUST_TEST_THREADS=4`, without a source change. Both runs remain
  retained. This is scoped Coder One verification, not a full-workspace gate.

The [manifest](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/truth9648-files.json)
binds all 529 files in the [evidence bundle](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/truth9648-records.tar.gz),
SHA-256 `85bc230e0cdda1b229a69cb738a540e0b9e2eec059a01d0df25578abf45108e8`.
It includes every old/new CLI invocation, expected control, plan, report,
compatibility check, replay record, paired development measurement, gate log,
original launcher stop, and zero-job inventory. The bundle was scanned against
exact local credential values, restored separately, and verified file by file.
The original checker and previous replay bundles remain intact.

## Protocol amendment

The [reserved confirmation protocol](../../bench/terminal-bench/experiments/2026-09-25-literal-confirmation/protocol.md)
must pin the corrected checker and a new job suffix before generation. Its tasks,
executor artifacts, policies, reviewer, thresholds, budgets, primary comparator,
and completion criterion stay fixed. The original unstarted protocol remains
available as an explicitly superseded version.

This follows the [September 25 assessment](../coder/design/2026-09-25-assessment.md):
independently supported observations still need trustworthy interpretation.
Finding this in a cheap control is useful progress; it is not evidence of
improved task completion. [#9584](https://github.com/OpenAgentsInc/openagents/issues/9584)
remains open until its declared measurement bar is met.

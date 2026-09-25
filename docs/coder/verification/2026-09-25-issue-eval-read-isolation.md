# Confine issue-evaluation reads

Issue [#9661](https://github.com/OpenAgentsInc/openagents/issues/9661), a
prerequisite to the default-policy comparison in
[#9624](https://github.com/OpenAgentsInc/openagents/issues/9624).

The initial comparison is invalid. Its third attempt searched the
operator's home directory and received private conversation history. The
network and write boundary did not prevent an outside read. No default
policy is selected from these attempts.

## Retained first attempt at the comparison

The [original protocol and pins](../../../bench/terminal-bench/experiments/2026-09-25-issue-flow-policies/protocol.md)
were pushed in `617bdc5723` before inference. The executable was built at
`cd5c1d32fb`; neither policy changed during the run.

| Slot | Entry | Policy | Graded checks | Flow outcome | Total seconds | Estimated Luna and Jev cost |
| --- | --- | --- | --- | --- | ---: | ---: |
| 1 | 9450, stale documentation | Lean | 2/4, failed | Finished | 121.0 | $0.009464096 |
| 2 | 9450, stale documentation | Requirements | 4/4, passed | Stuck | 175.6 | $0.014539566 |
| 3 | 9446, launcher | Requirements | 2/3, failed; outside-information exposure | Stuck | 1,222.3 | $0.023665042 |

All three attempts cost **$0.047668704** in recorded list-price estimates.
No total cost is missing. Slot 4 never launched: the operator reserved its
directory to make the driver stop before starting another attempt. The
third attempt completed its grading and receipt before that stop. The
driver's resulting `FileExistsError` is the deliberate stop, not another
model attempt.

The [evidence index](../../../bench/terminal-bench/experiments/2026-09-25-issue-flow-policies/records/evidence.json)
links full archives for slots 1 and 2. Each archive has a digest and a
member-by-member content inventory. Slot 3 is **quarantined locally** under
`~/.openagents/coder-one/issue-flow-policies-20260925/03-9446-requirements/`.
Its publication contains accounting and content digests, not raw output,
briefings, or a candidate diff. The private conversation is not copied into
this repository. The original records remain intact on the operator's host.

The documentation pair also exposes two distinct problems. Lean removed
the stale bullet, but its subsequent review restored it as an unrelated
deletion. The requirements loop left a correct artifact while declaring
some requirements stuck. An artifact's grade and the flow's completion
status are separate measures. These observations motivate follow-up work;
this incomplete, compromised study cannot rank the policies.

## Repair

`microluna::Seal` carries an optional read scope that applies to every
session command and to the issue-flow gate. Evaluation setup grants:

- The candidate and each command's owned scratch directory.
- The boundary's system program directories and the run's GitHub stub.
- Installed Rust toolchains, Rustup settings, and Cargo executables.
- Prefetched registry sources, read-only, without Cargo credentials or
  the operator's Cargo configuration. Cached Git dependencies are not
  granted; a task that needs an unavailable dependency fails offline.
- A private Cargo metadata directory, writable only for this run.
- On macOS, the selected Xcode bundle and its license receipts. The host
  supplies the developer directory and SDK explicitly and disables
  `xcrun`'s shared lookup cache.

Commands receive a scratch `HOME`. The gate builds inside the candidate's
own target directory, so another attempt's compiled outputs cannot serve
as an answer source. A remote or task-container command cannot substitute
for this host scope. Failure to construct the scope refuses the evaluation.

The manifest records `sealed.read_isolation` as
`candidate-and-toolchain-v1`, with the actual readable paths and writable
tool state. The gate records `reads_confined` and its paths. A normal
interactive turn without this evaluation seal keeps its existing behavior.
The macOS boundary still permits outside file metadata, but denies file
contents and directory listings. The trusted grader runs afterward on the
host and remains outside the model's scope.

## Verification

The model-style gate test compiles and runs a Rust fixture inside the
scope. Its assertions prove that it cannot read a planted private file,
write outside the candidate, receive the planted credential variables, or
connect to the external address. A marker proves that the test actually
ran. The Microluna command test uses the seal's read scope to read its
granted task and reject an adjacent private file.

The first compiler check exposed missing Xcode prerequisites. Those failed
checks led to the explicit installed-bundle, SDK, and license-receipt
grants; the home directory was not made readable. The corrected compiler
check passes on macOS 26.4 arm64.

The applicable manual gate is:

```sh
PATH=/opt/homebrew/opt/python@3.13/libexec/bin:$PATH \
CARGO_TARGET_DIR=/path/to/worktree-target ./scripts/verify-rust.sh \
  --crates coder-one,microluna,coder-boundary \
  --phases fmt,clippy,clippy-features,tests,tests-features
```

All five requested phases passed on macOS: formatting, Clippy in both
configurations, and tests in both configurations. The
[final receipt](../../../bench/terminal-bench/experiments/2026-09-25-issue-flow-policies/records/read-isolation-verification/20260925T200415Z-a3f615/run.json)
and [source digests](../../../bench/terminal-bench/experiments/2026-09-25-issue-flow-policies/records/read-isolation-verification/source.json)
identify the checked code. The receipt calls the overall run `partial`
because this is a scoped gate; it does not claim PostgreSQL,
external-model, Linux, or whole-workspace release acceptance.

The [first gate](../../../bench/terminal-bench/experiments/2026-09-25-issue-flow-policies/records/read-isolation-verification/20260925T195922Z-2e8137/run.json)
retains six failures. Issue
[#9662](https://github.com/OpenAgentsInc/openagents/issues/9662) repairs an
inventory test that tried to create a filename APFS rejects: an unsupported
Unix socket now exercises refusal on both operating systems, and Linux
retains the non-UTF-8 filename case. Five cancellation-dependent tests
failed under Apple's Python 3.9. Selecting the installed Python 3.13 makes
them pass unchanged. The [verification guide](../../verification.md)
records that prerequisite.

After integrating the concurrent main commits, the same five-phase gate
passed again. Its [receipt](../../../bench/terminal-bench/experiments/2026-09-25-issue-flow-policies-read-confined/records/preflight/rust-gate/run.json)
records the committed implementation used for the new comparison.

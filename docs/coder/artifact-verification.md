# Verify a retained artifact

`coder-project verify` runs operator-prepared checks against a retained Coder
scratch worktree. It records mechanical evidence separately from the delegate's
answer and from the operator's decision to integrate. This is the first host
verification layer for #9509. The `run-suite` program's host path now runs
through it under the bounds described below; `review-changes` remains
unimplemented.

## Prepare the check

First run `coder-project inspect` as described in
[project supervision](project-supervision.md). Review the committed patch and
copy its `tip` and `digest`. The inspector requires a clean, nonempty patch over
the recorded base and refuses changes outside the assigned paths.

Prepare a capability manifest for the checker, approve it with
`capability-trust`, and pin its manifest digest. Store the verification plan
outside the checkout and every delegate's writable grants. The checker command
and its arguments come from this host plan, never from a delegate's answer.

The plan has this shape; replace each placeholder with the inspected or approved
value. Arguments are passed directly to the adapter.

```json
{
  "base": "RECORDED_BASE_COMMIT",
  "tip": "INSPECTED_ARTIFACT_COMMIT",
  "owned_paths": ["crates/example"],
  "plan": {
    "schema": "openagents.verification.v1",
    "input_digest": "INSPECTED_ARTIFACT_DIGEST",
    "seconds": 300,
    "allow_unrestricted_reads": true,
    "allow_network": true,
    "checks": [{
      "id": "reviewed-check",
      "manifest": "/absolute/protected/checker.json",
      "manifest_digest": "APPROVED_MANIFEST_DIGEST",
      "arguments": ["--check"],
      "seconds": 120,
      "output_bytes": 65536,
      "acceptance": {"kind": "exit-success"}
    }]
  }
}
```

`exit-success` is appropriate only for a reviewed command whose exit status
expresses the intended check. A typed suite instead uses
`{"kind":"suite","suite_digest":"PINNED_SUITE","input_digest":"INSPECTED_ARTIFACT_DIGEST"}`.
Its complete stdout must be a JSON object containing `schema` equal to
`openagents.verification.v1`, matching `suite_digest` and `input_digest`, and
`verdict` equal to `passed`, `failed`, or `unverifiable`. Missing or mismatched
evidence cannot pass, even when the process exits successfully. A Gym adapter
must explicitly produce this contract; arbitrary Gym output is not accepted.

## Gate a program on typed suite evidence

A `check` step gated on `gate_not_met` runs the installed plan. Two further
bounds say what the plan must be before any check runs:

- `acceptance` — the evidence every check in the plan must produce:
  `suite` for a typed suite verdict or `exit-success` for a reviewed
  command. A plan whose checks answer with the other kind refuses at
  admission, so exit status cannot satisfy a requested typed suite. A
  `suite` requirement also narrows the plan to one suite identity.
- `max_tests` — the most checks the plan may run under the step, held
  against the plan's check count at admission.

The repository's `run-suite` program is one such step:

```json
{"name": "score", "kind": "check",
 "bounds": {"refuse_on": "gate_not_met", "acceptance": "suite", "max_tests": 16}}
```

The suite digests and the adapter arguments stay in the host-prepared plan;
the program carries only what the check requires. A host with no installed
plan refuses `run-suite` at admission, so it stays out of program selection
on a machine that was not given one. The earlier declared shape used a
`delegate` step followed by a check,
but the runtime refused its unsupported gate at admission. The new host path
executes independent checks directly; it never accepts a delegate's final text
as suite evidence.

## Run and review

```sh
export CODER_PROGRAM_EFFECTS=reads,network,subprocesses,spend
"$BIN/coder-project" verify "$REPO" "$RETAINED_WORKTREE" \
  "$PROTECTED_PLAN" "$NEW_EVIDENCE_DIRECTORY"
```

Use `coder-project run-suite` with the same arguments to invoke the repository's
suite program. That path requires every check to use typed `suite` acceptance,
pins all checks to one suite identity, and admits at most 16 checks. A plan using
`exit-success` refuses before execution. Both commands inspect the candidate
before and after checks and record independent evidence outside the checkout.

The explicit command authorizes its named program (`verify-artifact` or
`run-suite`) within the effect ceiling. It verifies capability approval again before each check,
runs checks serially with process-group cleanup, and records `plan.json`,
`result.json`, and `trace.atif.jsonl` in a new protected directory. Exit code `0`
means the verification program passed, `3` means a refusal or unmet gate, and
`2` means invalid configuration or an inspection error. The result always reports
`integration_accepted: false`.

The verifier snapshots the candidate before and after execution. Missing
snapshots or truncated output are unverifiable; failed commands and changed
candidate contents cannot pass. The artifact is inspected again after checks.
Evidence records output digests, elapsed command time, and typed verdicts. Review
coverage and run the repository's required manual gate before integrating.
Publish changes and accept the scheduler result separately.

## Enforcement and limits

The filesystem boundary denies writes except to private temporary scratch.
Unsupported boundaries refuse execution. Reads and network access are not
restricted by this boundary; both permissions must be explicitly enabled in the
plan. The capability's executable and manifest are pinned, but transitive tools
and dependencies still require operator review.

Each subprocess receives the lesser of its timeout and the remaining plan
budget. Setup consumes that budget. Snapshot collection and process cleanup are
outside the subprocess timeout, so `seconds` is not an end-to-end wall-clock
guarantee. Checks share no writable Cargo cache: `HOME` and `CARGO_TARGET_DIR`
point into private scratch, build concurrency is one, and only explicit
`RUSTUP_HOME` and `CARGO_HOME` are inherited. A checker that requires writable
shared caches needs a separate supported adapter; do not broaden the boundary
to make a test pass.

Current tests cover immutable workspace execution, failed and hung commands,
typed evidence identity, missing evidence, truncated output, plan validation,
evidence requirements at admission, the test budget, and the CLI's artifact
pinning. They use bounded fixture commands. They do not establish that a full
Cargo build or a Gym acceptance run works in this boundary.

`run-suite` is supported only as far as this path reaches. Still missing for
the full #9509 contract: a shipped suite adapter that emits the evidence
contract and the `metrics` output the program contract requires. The suite/doors inputs and
metrics/gate outputs remain the target contract; the current report records
only typed verification verdicts.

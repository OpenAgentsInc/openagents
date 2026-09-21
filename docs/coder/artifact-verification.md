# Verify a retained artifact

`coder-project verify` runs operator-prepared checks against a retained Coder
scratch worktree. It records mechanical evidence separately from the delegate's
answer and from the operator's decision to integrate. This is the first host
verification layer for #9509; it does not complete the `run-suite` or
`review-changes` program contracts.

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

## Run and review

```sh
export CODER_PROGRAM_EFFECTS=reads,network,subprocesses,spend
"$BIN/coder-project" verify "$REPO" "$RETAINED_WORKTREE" \
  "$PROTECTED_PLAN" "$NEW_EVIDENCE_DIRECTORY"
```

The explicit command authorizes the bundled `verify-artifact` program within
the effect ceiling. It verifies capability approval again before each check,
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
and the CLI's artifact pinning. They use bounded fixture commands. They do not
establish that a full Cargo build or a Gym acceptance run works in this boundary.

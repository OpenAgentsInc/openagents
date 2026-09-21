# Verify a retained artifact

`coder-project verify` runs operator-prepared checks against a retained Coder
scratch worktree. It records mechanical evidence separately from the delegate's
answer and from the operator's decision to integrate. This is the first host
verification layer for #9509. Both `run-suite` and `review-changes` use this
host path. The [suite adapter](#measure-a-gym-suite-through-the-adapter) retains
measurement evidence; [diff review](#review-a-pinned-diff) adds separately
recorded findings after mechanical checks pass. Neither workflow accepts an
artifact for integration.

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
pinning. The Gym adapter integration test also runs the installed checker
through this boundary against controlled local HTTP fixture doors. It covers
passed and failed gates, stale pins, mismatched model identities, truncated
output, retained measurement details, and unchanged candidate contents. These
fixtures do not establish model quality or prove that a full Cargo build fits
the same checker environment.

`run-suite` emits typed verification verdicts and retains the shipped Gym
adapter's measurement details. Still missing for the full #9509 contract are
dedicated program-level `metrics` and `gate` output bindings,
and the broader terminal and headless presentation of completion evidence.
The suite/doors inputs and metrics/gate outputs remain the target contract;
measurement details currently live under each check's `suite_evidence`.

## Measure a Gym suite through the adapter

`coder-project gym-suite PLAN.json` is a checker for the typed `suite`
acceptance mode. Approve the installed `coder-project` binary as a subprocess
capability and pass `gym-suite` and the absolute plan path as its arguments in
`run-suite`'s host verification plan. The adapter writes one
`openagents.verification.v1` object to stdout. Its optional `details` object
contains the Gym gate outcome, question and gate identities, measured rows,
missing-row counts, and elapsed time. The host retains those details under
`checks[].suite_evidence`; it still decides integration separately.

The protected adapter plan has this shape:

```json
{
  "suite": "/protected/suite.json",
  "suite_digest": "PINNED_GYM_SUITE_DIGEST",
  "questions": "/protected/questions.json",
  "question_digest": "PINNED_QUESTION_DIGEST",
  "gate": "/protected/gate.json",
  "gate_digest": "PINNED_GATE_DIGEST",
  "input_digest": "INSPECTED_ARTIFACT_DIGEST",
  "baseline": {"url": "http://127.0.0.1:8009", "model": "baseline-model"},
  "candidate": {"url": "http://127.0.0.1:8010", "model": "candidate-model"},
  "max_items": 1000,
  "seconds": 120
}
```

Use Gym's typed content digests, not a hash of the JSON file's formatting.
Keep the plan outside the candidate's write grants. The suite, question set,
and gate must match their pins before any inference starts. Both endpoints
must pass the native SDK's local-only configuration checks; the adapter sends
no provider credentials and follows no hosted fallback. It does not start or
build model servers. The host must prepare the intended candidate endpoint.
Model names are checked on responses, but are not cryptographic artifact
attestations; recorded door identities remain unverified.

The adapter asks both doors on the same development items, interleaved by
item, and applies the selected Gym score-comparison gate. It never reads the
locked partition or fits calibration on the development partition. A deployment
cost/latency gate cannot judge these score comparisons and remains
`unverifiable`. Missing answers, refusals, stale identities, and exceeded
bounds cannot pass. Documents are limited to 16 MiB each, development items to
1,000, and the whole measurement to the host's declared duration (at most one
hour). The outer verification plan also bounds subprocess time and output.
If the retained measurement object exceeds that output cap, the host refuses
truncated evidence; it does not accept the verdict alone.

The existing host boundary enforces a read-only candidate and owned scratch;
it still requires the plan's explicit unrestricted-read and network grants.
A local endpoint is a transport restriction, not a filesystem read sandbox or
proof that a model process cannot make its own network calls. Hosted suite
execution and verified model-artifact attestation remain separate work.

## Review a pinned diff

Run `coder-project review-changes REPOSITORY WORKTREE PLAN.json NEW_OUTPUT_DIRECTORY`
with a protected host plan outside the repository and its granted write roots:

```json
{
  "schema": "openagents.review-plan.v1",
  "verification": {
    "base": "FULL_RECORDED_BASE_COMMIT",
    "tip": "FULL_INSPECTED_SCRATCH_COMMIT",
    "owned_paths": ["crates/example"],
    "plan": "USE_THE_VERIFICATION_PLAN_OBJECT_DESCRIBED_ABOVE"
  },
  "reviewer": {
    "manifest": "/protected/reviewer-capability.json",
    "manifest_digest": "PINNED_MANIFEST_DIGEST",
    "arguments": [],
    "seconds": 120,
    "output_bytes": 65536
  },
  "policy": {"confirm_at": 0.9, "dismiss_below": 0.1},
  "excluded": []
}
```

Replace the `plan` placeholder with the actual versioned verification object.
The example thresholds illustrate the configuration shape; they are not measured
review policy. Choose and evaluate thresholds for the intended model and workload.
The command requires a configured [decision profile](decision-profiles.md), the
operator's program effect grants, and approval for both the mechanical check and
reviewer capabilities. The runtime resolves `openagents.review-finding.v1` from
the host question registry. Its semantic accuracy has not been measured.

The host independently inspects the scratch artifact, captures its exact diff,
and binds the verification plan to the artifact digest before execution. Mechanical
checks run first. A failed or incomplete check stops the program before the reviewer
runs. The reviewer runs in a read-only candidate boundary with private scratch,
bounded time and output, and independent before/after snapshots. Like the existing
verification boundary, it does not enforce network or filesystem read isolation;
the verification plan must explicitly permit those effects.

The reviewer receives `CODER_REVIEW_BASE`, `CODER_REVIEW_TIP`,
`CODER_REVIEW_INPUT_DIGEST`, `CODER_REVIEW_DIFF_DIGEST`, and
`CODER_REVIEW_DIFF` (the captured diff file path). Its complete stdout must be:

```json
{
  "schema": "openagents.review-findings.v1",
  "base": "ECHO_CODER_REVIEW_BASE",
  "tip": "ECHO_CODER_REVIEW_TIP",
  "input_digest": "ECHO_CODER_REVIEW_INPUT_DIGEST",
  "findings": [
    {
      "path": "crates/example/src/lib.rs",
      "span": {"start": 10, "end": 10},
      "severity": "error",
      "summary": "Describe the introduced problem.",
      "evidence": "Optional supporting evidence."
    }
  ]
}
```

`findings` is mandatory, including when empty. The host accepts at most 256
findings. It anchors paths to the inspected changes and line spans to added
new-side lines, excludes operator-declared paths, and preserves unanchored
findings without asking the model about them. Quoted Git paths do not support
line anchoring yet; file-level findings can omit `span`. Per-file decision
context is capped at 32 KiB and records truncation. A truncated context is not
proof of complete review coverage.

The result retains original findings, raw probabilities, and confirmed,
dismissed, unresolved, or unanswered dispositions. A failed decision call retains
collected evidence and unanswered findings. A missing document, wrong identity,
truncated output, crash, timeout, or changed snapshot cannot become an empty
successful review. Exit 0 means the reporting workflow finished, not that the
patch passed review: findings and unresolved results still require examination.
Exit 3 reports a program refusal; exit 2 reports host preparation or observation
failure. Every result sets `integration_accepted` to false. Merge, push, and issue
closure remain separate effects.

The subprocess/HTTP fixture exercises the real command with explicit empty,
malformed, failed, truncated, and three-way judged findings. It verifies retained
probabilities and ambiguous dispositions. It establishes protocol behavior, not
reviewer or decision-model quality. Full terminal presentation and measured
semantic review remain work under #9509 and #9503.

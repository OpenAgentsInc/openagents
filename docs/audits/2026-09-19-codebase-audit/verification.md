# Verification record

The final source snapshot is `1843fa6c18a05537bf2b022f69361a9ba3ef12a1`.
Checks ran on macOS with Rust 1.95.0 unless stated otherwise. The machine's
default stable toolchain was 1.94.1; it fails the workspace's declared Rust 1.95
requirement. Checks used existing local model artifacts where available and a
disposable PostgreSQL cluster for database acceptance tests.

The source baseline advanced during the audit. The large non-Kev test run covers
`f2a4bc79cba1b084425c22ce29ab31f85fb9e31a`; the subsequent source changes were
in `coder` and `coderbench`, whose complete package tests were then rerun at
their respective updated revisions. The final formatter, Clippy, and reproduction
runs cover the final snapshot. Counts below are per command and overlap; do not sum them as unique
test coverage.

## Commands and outcomes

| Check | Command | Observed result |
| --- | --- | --- |
| Formatting | `cargo +1.95.0 fmt --all -- --check` | Exit 1; differences in 51 distinct files. Existing issue #9402. |
| Strict linting with runtime features | `cargo +1.95.0 clippy --locked --workspace --all-targets --features 'kev/serve,lev/serve,gym/tui,jev/blocking' -- -D warnings` | Exit 101; three lint findings in Kev and Lev, listed under A23. This is a failing gate, not a successful lint run. |
| Workspace tests excluding expensive Kev inference | `cargo +1.95.0 test --locked --workspace --exclude kev --features 'lev/serve,gym/tui,jev/blocking'` | Exit 0; 655 passed and 5 ignored at `f2a4bc79cb`. Some environment-dependent tests can return early when a prerequisite is absent. |
| Coder tests after registry/probe changes | `cargo +1.95.0 test --locked -p coder` | Exit 0; 78 passed and 3 ignored at `06febed2d12`. Includes 25 additional unit tests relative to the preceding run. |
| CoderBench tests after the driver change | `cargo +1.95.0 test --locked -p coderbench` | Exit 0; 23 passed at `1843fa6c18`. |
| Kev API and encoder fixtures | `cargo +1.95.0 test --locked -p kev --features serve --lib --test api --test encode` | Exit 0; 9 passed. |
| Kev 0.5b numerical conformance and HTTP | `KEV_VARIANT=kev-0.5b cargo +1.95.0 test --locked --release -p kev --features serve --test conformance --test serve -- --nocapture` | Exit 0; 6 conformance tests and 4 HTTP tests passed with local 0.5b artifacts. |
| PostgreSQL and executable acceptance | `RUSTUP_TOOLCHAIN=1.95.0 ./scripts/test-postgres.sh` | Exit 0; store, gateway, multiprocess, bulk-import, and release load tests passed. The script also completed its executable health/authentication and two-process shadow checks. Relay source did not change between this run and the final snapshot. |
| Dependency advisories | `cargo deny check advisories` | Exit 1; default policy reports the `paste 1.0.15` unmaintained advisory, RUSTSEC-2024-0436. No checked-in deny policy was found. |
| Focused behavior probes | External temporary Cargo package running [reproduce.rs](reproduce.rs) | Exit 0; reproduced the behaviors listed below against the final snapshot. |
| Artifact inventory | `git ls-files swift/lev-bridge/.build training/lev-adapter/__pycache__` | 80 tracked generated paths. |
| Issue review | `gh issue list --state open --limit 500 --json number,title,body,labels,url,updatedAt,comments` | 27 open issues in the final snapshot; mapped in [issues.md](issues.md). |

The PostgreSQL load test processed 10,000 events across five runs and satisfied
its assertions. Its observed median throughput was about 3,138 events/second.
Other audit work shared the machine, so this number is an acceptance-test result,
not a production sizing claim or a clean latency baseline for #9382/#9393.

Kev's 0.5b conformance output included a maximum support-probability delta of
`4.128e-6` and packed-versus-separate delta of `1.192e-6`. This establishes the
selected fixture comparisons only. It does not establish the same result for
all checkpoints, accelerators, or concurrent serving workloads.

An earlier broad workspace run found a missing CoderBench golden. Upstream
restored it before the final snapshot, and the relevant tests then passed.
That transient failure is not an outstanding audit finding.

## Focused reproductions

The retained harness uses public workspace APIs, mock HTTP listeners on loopback,
and harmless files in a temporary directory. It invokes `/bin/sh` only for the
execution probes. It uses a separate temporary locked-read ledger, invokes no
decision model, and does not consume the repository's measurement read budget.
The credential-shaped argument supplied to the loopback clients is a fixed dummy
string. No real credential is needed or read by these probes.

The final run printed:

```text
embedded_example_is_plan=true
clarify_executed_command=true
unverified_delegations_required_correct=6 faults=[]
invalid_probability_accepted=-2
calibration_flipped_distribution={"yes": 0.25, "no": 0.7500000000000001} recorded_correct=false
split_utf8=Ok(("caf��", None))
eof_without_completion=Ok(("partial", None))
truncated_utf8_read=Some(Error { kind: InvalidData, message: "stream did not contain valid UTF-8" })
locked_first_reads_accepted=Some(16)
kev_refusal_classification=Harness("HTTP 422: audit")
readonly_delegate_status=answered wrote=true
delegate_status=timed out descendant_wrote_after_timeout=true
undeclared_bound_ignored=[]
jev_budget_ms=50 elapsed_ms=201 accepted=true
shell_status=timed out wrote_after_timeout=true
```

An earlier run admitted 15 of 16 first readers; the last run admitted all 16.
The race count and elapsed milliseconds depend on scheduling. The defect is
accepting more than one first reader, not a particular count. The harness prints
observations rather than treating these buggy outcomes as requirements to retain.
The output above is the final snapshot's. Remediation changes it where a
finding is fixed, and the harness is updated to keep compiling against the API
it probes. A04's line now reads
`unverified_delegations_required_correct=6 verdict=failed faults=[...]`, naming
the six unchecked delegations, the shortfall against `delegations_correct`, the
two decisions that answered null, the unread workspace, and the absent end
record. The [remediation register](remediation.md) records which findings have
landed.

To reproduce from the repository root on a Unix host with Rust 1.95.0 and the
dependencies available, create an external temporary package:

```sh
audit_repo="$(git rev-parse --show-toplevel)"
audit_tmp="$(mktemp -d "${TMPDIR:-/tmp}/openagents-audit.XXXXXX")"
python3 - "$audit_repo" "$audit_tmp" <<'PY'
import json
import pathlib
import shutil
import sys

root, work = map(pathlib.Path, sys.argv[1:])
(work / "src").mkdir()
shutil.copyfile(
    root / "docs/audits/2026-09-19-codebase-audit/reproduce.rs",
    work / "src/main.rs",
)
shutil.copyfile(root / "Cargo.lock", work / "Cargo.lock")
manifest = [
    '[package]',
    'name = "openagents-audit-probe"',
    'version = "0.0.0"',
    'edition = "2024"',
    'publish = false',
    '[dependencies]',
]
for name in ['coder', 'coderbench', 'atif', 'gym', 'jev']:
    path = json.dumps(str(root / 'crates' / name))
    manifest.append(f'{name} = {{ path = {path} }}')
manifest.extend([
    'serde_json = "1"',
    'tokio = { version = "1", features = ["full"] }',
    'tempfile = "3"',
])
(work / 'Cargo.toml').write_text('\n'.join(manifest) + '\n')
print(f'Probe package: {work}')
PY
CARGO_TARGET_DIR="$audit_repo/target" cargo +1.95.0 run \
  --manifest-path "$audit_tmp/Cargo.toml" -- "$audit_repo"
```

The command seeds the external package's lockfile from the workspace. Cargo may
update that copy to describe the probe package; the workspace lockfile remains
unchanged. Compilation uses the ignored workspace target directory. The probe
takes about 22 seconds after compilation because it deliberately waits past the
15-second shell deadline. Its own temporary fixtures are removed when it exits;
the external package remains available for inspection.

## Limits and follow-up verification

- A broad debug-mode Kev run selected all available local checkpoints and was
  stopped after several minutes of sustained CPU and memory use. It was replaced
  by the explicit release-mode 0.5b check above. The full checkpoint matrix did
  **not** complete during this audit.
- The one-hour relay soak was not run. The release load test was run separately
  through the PostgreSQL script, despite being ignored by ordinary `cargo test`.
- The two explicitly ignored real Devin delegation tests were not enabled.
  Coder's registry tests can probe the installed CLI for presence, but no real
  delegated agent task or paid generation call was requested by this audit.
- Local Apple helper tests that found their prerequisites ran as part of the
  test suite. This is not a comprehensive Apple OS/model/adapter compatibility
  matrix or a clean measurement window.
- No production relay, production database, or real backup/restore destination
  was modified. Replay, search equivalence, upload admission, and backup findings
  rely on the cited source paths and still need the proposed integration tests.
- No full interactive terminal session, accessibility review, fuzz campaign,
  Miri run, sanitizer run, or cross-platform build matrix was completed.
- A narrow tracked-text scan for common key/private-key patterns found no matches
  in the selected source and deployment paths, excluding retained transcripts.
  This is not a comprehensive secret-history or artifact-content audit. No
  credential values are reproduced in the report.
- Dependency advisory checks describe the fetched advisory data at audit time.
  They do not establish supply-chain provenance, license compliance, or the
  absence of undisclosed vulnerabilities.

For fixes, run the negative regression for the affected contract, its consumer's
integration test, and the agreed Rust gates. Run expensive live-model, soak, and
performance checks in a declared environment where skips and resource contention
are visible. Preserve the repository's ban on GitHub-billed automation.

## Follow-up evidence review

The implementation author's review identified consequences for closed issues
#9376 and #9384. The following read-only checks ran against the same source and
measurement files before creating remediation issues. They qualify the scope of
the findings; they do not replace the required post-fix rechecks.

### A05 and the calibration floors

The three quoted values in `Rule::v2` are raw block standard deviations:
ECE `0.0266`, Brier `0.0119`, and NLL `0.6428`. The corresponding one-block-per-side
two-sigma comparisons are approximately `0.075`, `0.034`, and `1.818`.

The raw path is
[`report_blocks`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/bin/gym.rs#L1822)
→ `Draws::observations` →
[`Draw::observation`](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/spread.rs#L123)
→ `calibrate::score`. It uses recorded `top` and `correct` values, without calling
`eval::mapped_observations`. The original feedback's claim that those three raw
values passed through that function is therefore not supported by the code.

The A05 fix still needs to regenerate the measurement record and review each
mapped claim against the chosen serving contract. Raw values may reproduce
unchanged. Mapped confidence for a fixed original choice and confidence for a
newly selected choice are different quantities and must be identified. Changes
to semantics or adopted values require updated gate provenance and digests;
unchanged values need an explicit derivation check, not an automatic withdrawal.

### A05: the fixed selection does not survive every wire shape

A follow-up at `4010baadbc` on 2026-09-20 found that the fixed-selection
implementation covers Choice but leaves Noul and Score inconsistent across
calibration and serving. The retained [wire probe](calibration-wire.rs) calls
`Map::apply_distribution`, `lev::estimator::answer`, JSON serialization,
`jev::SystemOneResponse::decode`, and `gym::eval::read_answer`. It compares the
served answer with a known target and the corresponding mapped observation.
It invokes no model and needs no credentials.

A map fitted on one incorrect observation at `0.8` gives the selected option
`0.25`. The probe reports:

```text
kind=choice raw_selected=yes mapped_metric_correct=false wire_selected=yes wire_correct=false
kind=noul raw_selected=yes mapped_metric_correct=false wire_selected=no wire_correct=true
kind=score raw_selected=0 mapped_metric_correct=false wire_selected=1 wire_correct=true
```

Choice carries an explicit selected option. Noul carries only the probability
of yes, which the evaluator thresholds again. Score carries a distribution,
which the evaluator takes an argmax over again. Passing the original selection
to `lev::estimator::answer` therefore does not preserve it for those two shapes.
The existing fixed-selection serving regression exercises Choice only.

These synthetic cases establish a remaining contract defect, not a change to
historical measurement rows. #9419 must resolve it across serving and metrics;
reproducing historical numbers alone is insufficient to close the issue. The
[issue evidence](https://github.com/OpenAgentsInc/openagents/issues/9419#issuecomment-5748060751)
also records the broader passing suites that did not detect it.

To run this narrower probe, use the temporary-package procedure above with
`calibration-wire.rs` as `src/main.rs` and add `lev` to the path dependency list.
Run it with Rust `1.97.1` and a Cargo target directory dedicated to the source
checkout. The probe prints what happened rather than asserting that these
incorrect outcomes must remain.

### A07 and the Kev comparison rows

A direct comparison of `crates/gym/results/support-v2-three-way.jsonl` with the
non-locked `(id, partition)` pairs in the pinned suite produced:

| Door | Expected pairs | Rows | Unique pairs | Missing | Unexpected | Matching suite digest | Answered, scored, and no refusal |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `kev-0.5b` | 157 | 157 | 157 | 0 | 0 | All | 157 |
| `kev-0.6b` | 157 | 157 | 157 | 0 | 0 | All | 157 |
| `kev-4b` | 157 | 157 | 157 | 0 | 0 | All | 157 |
| `kev-8b` | 157 | 157 | 157 | 0 | 0 | All | 157 |

This establishes coverage in the committed artifact. It does not verify the
receipt chain, every upstream attempt, or independently reproduce the inference.
The [runner](https://github.com/OpenAgentsInc/openagents/blob/1843fa6c18a05537bf2b022f69361a9ba3ef12a1/crates/gym/src/bin/gym.rs#L552)
increments `lost` when classification produces no row. Thus A07 mislabels a
door refusal and excludes it from scored/refused rows; it does not silently turn
that failure into a scored row or prove that this historical panel omitted items.

The remediation issue must re-check these records after fixing the contract,
verify receipt and run provenance, and correct #9384's assertion that the old
classifier established correct refusal accounting. Report any evidence that was
not retained rather than reconstructing a historical error response. The
published numerical comparisons should change only if that reconciliation or a
properly scoped rerun supplies a reason.

### A01 after the fix

Commit `25f0b54e4a` makes a generated answer and an executable plan separate
validated outcomes and moves execution intent into a host-owned `Permit`. The
harness's A01 probes were updated to that interface and gained two more
observations, an unpermitted turn and a permitted one, so the record separates
a boundary that closed from one that refuses everything.

Before the fix, at `81eb7fd31a`, the original A01 probes still reproduce the
audit's run. The parser's source is unchanged from the reviewed snapshot: a
diff of `crates/coder/src/shell.rs` between `1843fa6c18` and `81eb7fd31a` is
empty.

```text
embedded_example_is_plan=true
clarify_executed_command=true
```

After it, at `25f0b54e4a`, the harness prints:

```text
embedded_example_is_plan=false
clarify_executed_command=false
unpermitted_turn_executed_command=false
permitted_turn_executed_command=true
```

The clarification probe now hands the turn a complete plan under the supported
schema together with an executing permit, so what it measures is the permit
rather than the schema. The remaining probes are unchanged by this commit, and
the `shell_status=timed out wrote_after_timeout=true` line still reports A02.

The regressions run with the workspace toolchain: 23 tests, no failures and
none ignored.

```sh
cargo +1.97.1 test --locked -p coder --lib shell::tests
cargo +1.97.1 test --locked -p coder --lib permit::tests
cargo +1.97.1 test --locked -p coder --lib agent::tests
```

They cover a plan quoted as an example in prose, a fenced plan with prose
before or after it, a missing version, a later version, a version given as a
string, eight malformed command lists, an eleventh command, a clarifying turn
holding a valid plan, an unpermitted turn holding one, a proposal that reaches
the runner without a permit, and a permitted plan that still runs. The whole
crate passes: 85 library tests and 39 integration tests.

This is the parsing and execution-intent half of the #9413 release blocker. It
does not establish the process-tree termination of A02, the capture bounds of
A03, or the trusted executable probes of A17, and it does not verify that a
delegated executor honors a bound.

### A02 and A03 after the fix

Commit `a96845c40d` puts both execution paths behind one subprocess
supervisor, `crates/supervise`. A job's direct child leads a process group of
its own, a deadline or a cancelled caller terminates that group and reaps the
direct child before the job reports, and stdout and stderr are held to their
caps as they are read rather than after a whole `output()` is in memory.

The harness gained two observations for this, so the record can separate a
shell that stopped from a shell whose descendants stopped with it, and can
show where the output cap applies. The A02 shell probe now starts a
background child that would write at sixteen seconds, and the new A03 probe
offers four mebibytes to a command whose retained output is sixteen
kibibytes.

Before the fix, the harness reported the two A02 lines as the audit found
them, at `81eb7fd31a` and again at `6b13bdba5d`:

```text
delegate_status=timed out descendant_wrote_after_timeout=true
shell_status=timed out wrote_after_timeout=true
```

After it, at `a96845c40d`, with `cargo +1.97.1`:

```text
delegate_status=timed out descendant_wrote_after_timeout=false
shell_status=timed out wrote_after_timeout=false descendant_wrote_after_timeout=false
shell_printed_bytes=4194304 shell_kept_bytes=16419
```

The last line is an observation of the retained result: the command printed
four mebibytes and the outcome kept sixteen kibibytes and the byte count of
everything. It does not by itself establish that the cap applied during
capture rather than after it. The tests do that, by asking a program for more
output than its cap allows and reading back what was kept while the job was
terminated for running too long.

The A02 and A03 tests pass at `a96845c40d`: 24 in `supervise`, 16 in the
shell runner, 12 in delegation, and 6 in CoderBench's driver.

```sh
cargo +1.97.1 test --locked -p supervise
cargo +1.97.1 test --locked -p coder --lib shell::tests
cargo +1.97.1 test --locked -p coder --lib delegate::tests
cargo +1.97.1 test --locked -p coderbench --lib drive::tests
```

They cover a grandchild on a deadline, a cancelled future, a caller that
stopped waiting, a child that exits while a descendant remains, two
simultaneous jobs where only one is terminated, a child that ignores
`SIGTERM`, unreaped children, a job that will not spawn, oversized output on
both streams at once, a producer that never stops, a cap inside a multi-byte
character, partial output kept on a timeout, and a probe that removes its
temporary files however it ends. The whole workspace suite passes.

Platform support is stated rather than assumed: process-tree ownership here
is `process_group(0)` and `killpg`, and `supervise` does not compile on a
platform where no equivalent is implemented.

What this does not reach. It bounds time and captured output; it is not a
sandbox, and it does not bound what a program reads, writes, or sends. A
descendant that calls `setsid` leaves the group and is beyond it. The
supervisor signals the group immediately after reaping its leader, which
leaves a window of microseconds in which a host could recycle the group
identifier; closing it needs `waitid(WNOWAIT)`, which is not on the
asynchronous runtime's wait path. And process-tree termination does not
establish the trusted probes of A17 or the verified enforcement of A18,
which #9413 still waits on.

### Runtime admission and worktrees

Commit `1eb60eccea31873af59fe2df65e16ce46b99b928` landed during this follow-up.
At `1f78b260e2`, the following targeted checks passed: four tests total, no
failures or ignored tests.

```sh
cargo +1.95.0 test --locked -p coder --test program_run a_bound_nobody_claims_is_refused
cargo +1.95.0 test --locked -p coder --test program_run the_check_records_who_holds_each_bound
cargo +1.95.0 test --locked -p coder --lib worktree
```

They exercise unknown-bound refusal, the recorded host/executor distinction,
one separate checkout, and six concurrent distinct checkouts with cleanup.
They use test executors and temporary repositories, without live delegated work.

The [runtime admission code](https://github.com/OpenAgentsInc/openagents/blob/1eb60eccea31873af59fe2df65e16ce46b99b928/crates/coder/src/runtime.rs#L918)
still derives `Enforcement::Executor` from `manifest.enforces`. The passing
recording test proves that this claim is recorded; it does not independently
verify executor enforcement. The worktree tests establish separate checkouts,
not a read-only boundary. These checks credit partial progress on A18 and do not
replace the negative acceptance tests in #9427 or a review of the entire runtime.

### A07 after the fix

The refusal regression runs the real Kev router over a tiny generated model
through the Jev HTTP client and Gym classifier. It needs no downloaded weights.
Eight tests cover a successful answer, invalid input, unknown models, token and
HTTP body capacity, a tensor inference failure on valid input, connection loss,
incomplete response bodies, and legacy untyped error bodies. Typed refusals
produce refused rows; transport losses do not. The body-limit regression failed
with the old plain-text 413 response before the extractor rejection was mapped.

The historical reconciliation selects #9384's exact four run timestamps. Each
has all 157 expected item/split pairs once, every row scored. It verifies the
complete receipt chain, suite and gate digests, and full recorded base signatures.
The original 0.5B run has no verified base signature; the test preserves that
limitation. No retained result rows were edited, and the numerical comparisons
remain unchanged. Final-row coverage does not prove that no failed attempts or
retries occurred; the committed run record contains no raw failed-response log.

Run the checks with separate target directories for separate worktrees:

```sh
cargo +1.97.1 test -p kev --features serve --test refusals
cargo +1.97.1 test -p gym --test kev_variant_rows
```

The audit harness's typed-envelope line checks Gym's interpretation of an
authored body. The HTTP tests above establish that Kev sends the envelope.

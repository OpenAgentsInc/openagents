# CND-053: Terminal-Bench Signature Routing Fixtures

Status: retained regression fixture scaffold
Last updated: 2026-06-01
Owner: OpenAgents

This document records the first Benchmark Cloud fixture set for routing
previously failed Terminal-Bench tasks into Probe seed signatures. It supports
the retained-run plan tracked in:

- `docs/bootstrap/CND-051-shc-codex-terminal-bench-8task.md`
- `docs/bootstrap/CND-052-shc-codex-terminal-bench-16task-preserved.md`
- `https://github.com/OpenAgentsInc/cloud/issues/69`
- `https://github.com/OpenAgentsInc/vortex/issues/94`

These fixtures are internal regression guards. They are not a Terminal-Bench
score, not a public leaderboard claim, and not hidden-test disclosure.

## Fixture Location

```text
runners/py-bench-runner/fixtures/signature-routing/
```

Each JSON file is a normalized `BenchmarkTask` with public-safe metadata:

- Terminal-Bench task id;
- retained failure family;
- source document and preserved-run reference;
- task checksum from the measured SHC run;
- raw Codex reward/verifier summary from the committed report;
- expected Probe signature ids;
- candidate and forbidden signature ids;
- required evidence and closeout artifacts.

## Covered Families

| Failure family | Task fixture | Expected signature |
| --- | --- | --- |
| service readiness | `configure-git-webserver` | `coding.service_readiness` |
| local PyPI/simple index | `pypi-server` | `coding.python_package_index` |
| query optimizer workflow | `query-optimize` | `coding.query_optimizer_workflow` |
| SQLite/WAL recovery | `db-wal-recovery` | `coding.sqlite_wal_recovery` |
| G-code parser contract | `gcode-to-text` | `coding.gcode_parser_guard` |
| XSS sanitizer policy | `filter-js-from-html` | `coding.xss_sanitizer_policy` |
| benchmark runner stall | `query-optimize` operational stall | `benchmark.runner_supervisor` |

## Runner Support

The Python benchmark runner now writes `signature_selector_trace.json` when a
task carries `metadata.signatureRouting`.

Raw Codex baselines use:

```text
selectionEnabled = false
selectorMode = baseline_disabled
selectedSignatureIds = []
```

Probe+Codex signature runs use:

```text
selectionEnabled = true
selectorMode = fixture_expected
selectedSignatureIds = metadata.signatureRouting.expectedSignatureIds
```

For `probe-codex`, the Codex adapter also adds a public-safe Probe signature
addendum to the prompt with selected signatures, failure fingerprints, required
evidence, closeout artifacts, versioned playbook steps, and the retained
raw-reward to expected-signature-reward target. The addendum does not include
hidden verifier details, benchmark-local secrets, or provider auth.

`coding.sqlite_wal_recovery` now carries the learned v2 rule from the
account-backed rerun: copy the DB/WAL/SHM matched set before opening SQLite,
then run recovery/checkpoint/integrity checks only on the copy. This specific
rule is what changed the retained `db-wal-recovery` rerun from `0.0` reward to
`1.0` reward in the preserved SHC evidence below.

## Validation

Run from the runner directory:

```bash
cd runners/py-bench-runner
pytest
```

Targeted checks:

```bash
pytest tests/test_signature_routing.py
pytest tests/test_codex_adapter.py
```

Retained improvement check:

```bash
python3 -m openagents_bench.evaluate_signatures --fixture-dir fixtures/signature-routing --json
```

The evaluator reports `rawCodexMeanReward`,
`expectedProbeSignatureMeanReward`, and `expectedMeanRewardDelta` across all
retained fixtures. This is the local benchmark-regression improvement signal
for the prompt/playbook layer.

The tests cover:

- all retained failure families have at least one fixture;
- the expected signature is selected for `probe-codex`;
- forbidden signatures are not selected;
- raw Codex and Probe+Codex dry-run artifacts are comparable for the same
  service-readiness fixture;
- the Probe prompt addendum carries selected signature evidence without auth
  material.
- the retained fixture evaluator shows Probe+Codex selected-signature expected
  reward above the retained raw Codex baseline.

## Next Step

Use these fixtures in `Vortex` issue `SIG-TRAIN-001` to run the first retained
Terminal-Bench rerun comparison:

```text
raw Codex
Probe+Codex without selected signatures
Probe+Codex with fixed top-K signatures
Probe+Codex with capped selector-selected signatures
```

The first live rerun should target `configure-git-webserver` because it is a
small service-readiness miss and has an unambiguous expected signature. Preserve
raw traces, selector decisions, transcripts, commands, verifier output,
`result.json`, cost, and closeout artifacts before expanding to the remaining
families.

## 2026-06-01 Account-Backed Rerun Evidence

The `db-wal-recovery` fixture now has a complete account-backed learning loop on
`oa-shc-katy-01`:

| Run | Auth | Signature package | Reward | Notes |
| --- | --- | --- | ---: | --- |
| `/home/ubuntu/oa-bench-runs/shc-probe-codex-tb2-validpackage-chatgpt-20260601/` | ChatGPT `auth.json` through `CODEX_AUTH_JSON_PATH` | `coding.sqlite_wal_recovery` loaded | `0.0` | Codex opened SQLite before copying the WAL, and SQLite removed the unreadable sidecar. |
| `/home/ubuntu/oa-bench-runs/shc-probe-codex-tb2-validpackage-v2-chatgpt-20260601/` | ChatGPT `auth.json` through `CODEX_AUTH_JSON_PATH` | revised `coding.sqlite_wal_recovery` loaded | `1.0` | The package required DB/WAL/SHM copy-before-open; verifier passed `7/7`. |

Preserved tarballs:

- `/home/ubuntu/oa-bench-artifacts/shc-probe-codex-tb2-validpackage-chatgpt-20260601.tar.gz`
  - SHA256: `4314620de58c0fd7a08ec6fa53a3e946e62dcb7d82dd1a3409b34ef72516e14a`
- `/home/ubuntu/oa-bench-artifacts/shc-probe-codex-tb2-validpackage-v2-chatgpt-20260601.tar.gz`
  - SHA256: `72e6f23f242dcd6d000956594d11b39041ede6cbe6e098fb3099b5868607453b`

This is internal retained evidence only. It proves the no-API-key Codex account
path, package loading, failure capture, signature revision, and rerun pass for
one retained task. It is not a public Terminal-Bench score.

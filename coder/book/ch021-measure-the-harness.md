# 21. Measure the harness

> **Status: partly implemented.** OpenAgents implements canonical ATIF-v1.8
> trajectory logging and SHA-256 `steps_digest` verification
> (`coax/lib/nostr/atif/log.coax`), `#authority` gBench scoring, exact integer
> Wilson 95% confidence intervals, scaffold-lock comparability, and dataset
> distillation license gating (`coax/lib/gbench/policy.coax`), plus live
> repository pilot (`coder/tests/repository.coax`), gBench, Terminal-Bench 4.1,
> session-history friction scorecards (`evals/session-history/README.md`), and
> Gym A/B subagent benches. Controlled multi-engine ablations isolating each
> harness addition on held-out repository tasks remain open.

An agent change without measurement is guesswork. OpenAgents requires every
harness evaluation—whether a local capability eval inside Desktop, a gBench
local-model run, a Terminal-Bench 4.1 suite, or a repository pilot—to rest on
three verifiable pillars:

1. **Canonical ATIF-v1.8 trajectory logs (`coax/lib/nostr/atif/log.coax`,
   `#13830`):**
   - Every run writes append-only `.atif.jsonl` lines (`session` -> 1-based
     `step` records -> `end`) where unreported metric counts stay absent rather
     than fabricated as zero.
   - `atif_verify_log` checks lifecycle structure (rejecting missing headers,
     duplicate sessions, bad schema versions, or steps arriving after `end`)
     and computes a canonical SHA-256 `steps_digest`.
   - `atif_write_manifest` emits a closed `openagents.atif-manifest.v1` object
     binding the artifact digest, byte size, `steps_digest`, step count, state,
     and coverage.
2. **`#authority` benchmark scoring & comparability (`coax/lib/gbench/policy.coax`):**
   - `gb_classify_outcome` separates genuine capability failures (`FAIL`) from
     infrastructure/harness errors (`INFRA_ERROR`) so broken environments never
     masquerade as model regressions.
   - `gb_wilson_95` computes exact integer 95% Wilson confidence intervals in
     basis points (`0..10000`) with zero floating-point drift.
   - `gb_scaffolds_match` enforces Scaffold Contract v2: two arms are
     comparable only when their `scaffold_id` tokens match byte-for-byte.
   - `gb_admit_distill_suite` enforces a fail-closed dataset license/canary
     allowlist (`gsm8k` admitted; `gpqa` canary and `aime` CC-BY-NC denied)
     before trajectories can be distilled into Psionic training data.
3. **Live evaluation suites in this repository:**
   - **Repository pilot:** [`coder/tests/repository.coax`](../../coder/tests/repository.coax)
     and [`coder/tests/repository-summary.coax`](../../coder/tests/repository-summary.coax)
     ([journey guide](../../docs/coder/repository-pilot.md)).
   - **gBench & Terminal-Bench 4.1:** [`coax/lib/gbench/suites.coax`](../../coax/lib/gbench/suites.coax)
     and [`coax/lib/tbench/suites.coax`](../../coax/lib/tbench/suites.coax).
   - **ATIF-v1.8 session-history friction scorecard:**
     [`evals/session-history/README.md`](../../evals/session-history/README.md).
   - **Desktop Gym A/B subagent bench:** [`coax/lib/agent/desktop/gym.coax`](../../coax/lib/agent/desktop/gym.coax).

## Profile

This chapter's profile is `openagents-atif-gbench-v1, coder-repository-pilot-v1, gBench, Terminal-Bench 4.1, ATIF-v1.8 session-history friction scorecard, and Gym A/B subagent bench`:

- Emits and verifies an ATIF-v1.8 trajectory log, computes its canonical
  SHA-256 `steps_digest`, emits `openagents.atif-manifest.v1`, and catches a
  step arriving after `end`.
- Classifies pass vs. infrastructure error, computes exact integer Wilson 95%
  CI (`24/30 -> 8000 bps, [6269, 9049]`), enforces `scaffold_id` matching, and
  enforces fail-closed distillation license/canary policy.

## Worked example

The runnable example [measure-atif.coax](../examples/measure-atif.coax) drives
`coax/lib/nostr/atif/log.coax` and `coax/lib/gbench/policy.coax` directly.

<!-- source: coder/examples/measure-atif.coax example -->
```jai
main :: () {
  // 1. Emit a valid ATIF-v1.8 session -> 2 steps -> end trajectory log
  buf: [4096] u8;
  n := 0;
  k := atif_write_session_line(AtifSessionHeader.{
    trajectory_id = "traj-coder-ch21",
    session_id = "sess-coder-ch21",
    schema_version = ATIF_SCHEMA_VERSION,
    agent_name = "oa-worker",
    model_name = "inkling-2-mini",
    cwd = "/repo",
    git_branch = "main",
    source_harness = "openagents-core",
    created_at = "2026-11-26T00:00:00Z",
  }, buf[n..<buf.count]);
  n += k;

  sb: [1024] u8;
  sn := atif_write_step_json(AtifStep.{
    step_id = 1,
    timestamp = "2026-11-26T00:00:01Z",
    source = ATIF_SOURCE_USER,
    message = "Run openagents/core/query_loop.coax checks",
  }, sb[..]);
  n += atif_write_step_line(cast(string) sb[0..<sn], buf[n..<buf.count]);

  sn = atif_write_step_json(AtifStep.{
    step_id = 2,
    timestamp = "2026-11-26T00:00:02Z",
    source = ATIF_SOURCE_AGENT,
    message = "All checks passed.",
    model_name = "inkling-2-mini",
    tool_calls_json = "[{\"tool_call_id\":\"c1\",\"function_name\":\"Bash\",\"arguments\":{\"cmd\":\"coax test\"}}]",
    observation_json = "{\"results\":[{\"source_call_id\":\"c1\",\"content\":\"ok\"}]}",
    metrics = AtifMetrics.{has_prompt = true, prompt_tokens = 420, has_completion = true, completion_tokens = 38, has_cost = true, cost_usd_micros = 0},
  }, sb[..]);
  n += atif_write_step_line(cast(string) sb[0..<sn], buf[n..<buf.count]);
  n += atif_write_end_line("2026-11-26T00:00:03Z", ATIF_STATE_ENDED, buf[n..<buf.count]);

  // 2. Verify log lifecycle + canonical SHA-256 steps_digest and emit manifest
  sum := atif_verify_log(cast(string) buf[0..<n], "");
  art: [71] u8;
  _ = atif_artifact_digest(cast(string) buf[0..<n], art[..]);
  mbuf: [2048] u8;
  mn := atif_write_manifest(AtifManifestInput.{
    trajectory_id = "traj-coder-ch21",
    session_id = "sess-coder-ch21",
    schema_version = ATIF_SCHEMA_VERSION,
    form = "log",
    artifact_digest = cast(string) art[..],
    artifact_size = n,
    steps_digest = cast(string) sum.steps_digest[..],
    step_count = sum.step_count,
    state = ATIF_STATE_ENDED,
    coverage = ATIF_COV_COMPLETE,
  }, mbuf[..]);
  print("atif log: ok=% steps=% faults=% manifest_bytes=% digest=%\n",
    sum.ok, sum.step_count, sum.faults, mn, cast(string) sum.steps_digest[0..<16]);

  // 3. Verifier catches lifecycle faults (e.g. a step arriving after end)
  corrupt: [4096] u8;
  cn := atif_copy_str(corrupt[..], cast(string) buf[0..<n]);
  cn += atif_copy_str(corrupt[cn..<corrupt.count], "{\"record\":\"step\",\"step\":{\"step_id\":3,\"source\":\"user\",\"message\":\"late\"}}\n");
  bad := atif_verify_log(cast(string) corrupt[0..<cn], "");
  print("atif fault check: ok=% faults=%\n", bad.ok, bad.faults);

  // 4. #authority gBench outcome classification, integer Wilson 95% CI, scaffold lock & license gate
  out_pass := gb_classify_outcome(GbSuiteOutcomeInput.{applicable = 10, passed = 10, exit_code = GB_EXIT_OK});
  out_infra := gb_classify_outcome(GbSuiteOutcomeInput.{applicable = 10, passed = 10, harness_errors = 1, exit_code = GB_EXIT_OK});
  w := gb_wilson_95(24, 30);
  print("gbench score: pass=% infra=% rate_bps=% wilson_95=[%,%] scaffolds_same=% scaffolds_diff=%\n",
    gb_status_label(out_pass), gb_status_label(out_infra), gb_rate_bps(24, 30),
    w.low_bps, w.high_bps,
    gb_scaffolds_match("sc_965cd8ee", "sc_965cd8ee"),
    gb_scaffolds_match("sc_965cd8ee", "sc_deadbeef"));
  print("distill gate: gsm8k=% gpqa=% aime=%\n",
    gb_distill_reason(gb_admit_distill_suite("gsm8k")),
    gb_distill_reason(gb_admit_distill_suite("gpqa")),
    gb_distill_reason(gb_admit_distill_suite("aime")));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/measure-atif.coax
```

```text
atif log: ok=true steps=2 faults=0 manifest_bytes=566 digest=a488ea07e62f6884
atif fault check: ok=false faults=1
gbench score: pass=PASS infra=INFRA_ERROR rate_bps=8000 wilson_95=[6269,9049] scaffolds_same=true scaffolds_diff=false
distill gate: gsm8k=admitted_permissive gpqa=denied_canary_benchmark aime=denied_non_commercial_license
```

The checked fixture is [measure-atif.out](../examples/measure-atif.out).

## What remains

- Running periodic comparative ablations isolating each harness addition on a
  held-out multi-repo benchmark with identical prompts and budgets.

## Done means

- [x] ATIF-v1.8 logging/verification and gBench `#authority` scoring, Wilson CI,
  scaffold lock, and distillation license allowlists run in production tooling
  and `oa-worker`.
- [x] `coder/examples/measure-atif.coax` matches `coder/examples/measure-atif.out`
  in debug and release.
- [ ] Periodic ablation reports compare harness additions on held-out coding
  tasks.

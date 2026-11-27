# 45. A two-arm run

> **Status: partly implemented.** Two-arm baseline-vs-candidate execution with
> real `ATIF-v1.8` SHA-256 `steps_digest` recording and verification
> ([`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax),
> [`capability-eval.coax`](../../coax/lib/agent/desktop/capability-eval.coax),
> [`coax/lib/nostr/atif/log.coax`](../../coax/lib/nostr/atif/log.coax), `#13896`,
> `#14059`), `#authority` promotion gating
> ([`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax)),
> and NIP-EVAL arm coverage verification
> ([`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax))
> run on `cpu`, grounded by [`openagents/examples/two-arm-atif-eval.coax`](../examples/two-arm-atif-eval.coax)
> and [`openagents/examples/two-arm-atif-eval.out`](../examples/two-arm-atif-eval.out),
> and are adopted by [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax)
> and [`apps/openagents-desktop/CapabilityStudio.swift`](../../apps/openagents-desktop/CapabilityStudio.swift).
> Remaining: hosted sandboxed multi-case statistical uncertainty reporting and
> gate-digest pinning in `openagents/eval/run.coax`.

## Premise

With a test set in hand, this chapter runs it twice: once with the member
admitted and once without, on the same baseline agent. The difference is the
first piece of evidence.

## Draft

The question is never "is this plugin good?". The question is what admitting
it changed. To answer, you need two runs that differ in exactly one thing.

The baseline arm runs the agent as it is, on every case. The subject arm runs
the same agent, on the same cases, with the member admitted. Everything else
is locked: the model and its version, the instructions, the defaults already
in place, the environment and the grant. Both arms write full records (chapter
8), so anyone can read what happened in each case.

The report gives, for each arm, how many cases passed, the score, the cost and
the time. It gives the change between arms with its uncertainty. Six cases
can't support a strong statement, and the report says so and does not hide it.

A written gate reads the report and gives a verdict. The gate names one
primary outcome ahead of time, so a result can't be rescued after the fact by
picking a better-looking number. A member with no good effect on its primary
outcome is rejected, however safe it is.

Cost is part of the result. In the old tree's hosted runs, the repository map
passed more cases and also finished faster: 10.7 seconds against 24.9. A
member can stand in for compute.

The run happens in the evaluation sandbox from chapter 40, on your computer or
on a hosted runner.

## What we build

A plan for what this chapter ships once the draft is approved:

- A two-arm runner: the same cases with and without the subject, everything
  else locked.
- A report with both arms, the change, its uncertainty, cost and time, and the
  gate's verdict.
- A gate file with a primary outcome named ahead of time, pinned by digest in
  the report.

It builds on what exists today:

- [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax) and [`coax/lib/agent/desktop/capability-eval.coax`](../../coax/lib/agent/desktop/capability-eval.coax) (`ce_run_two_arm`, real `/bin/sh` two-arm execution, `proc_now_ms()` latency measurement, and `selftest`, `#13896`, `#14059`)
- [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax) (`#authority` two-arm promotion gate)
- [`coax/lib/nostr/atif/log.coax`](../../coax/lib/nostr/atif/log.coax) (canonical `ATIF-v1.8` log writer and SHA-256 `steps_digest` verifier)
- [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax) (`#authority` NIP-EVAL two-arm report coverage verifier)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/run`](../../coax/lib/nostr/run)
- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`coax/remote`](../../coax/remote)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Report | `openagents.eval.v1` `Report`, `Measurement`, `RunCoverage` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)) | Nostr kind 3189, public | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax),
  [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax),
  [`coax/lib/nostr/atif/log.coax`](../../coax/lib/nostr/atif/log.coax),
  [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax),
  [`openagents/examples/two-arm-atif-eval.coax`](../examples/two-arm-atif-eval.coax),
  and [`openagents/examples/two-arm-atif-eval.out`](../examples/two-arm-atif-eval.out)
  (remaining: statistical uncertainty runner in `openagents/eval/run.coax`).
- **Trace:** records a canonical 4-step two-arm `ATIF-v1.8` log
  (baseline/candidate positive + held-out), computes and verifies its SHA-256
  `steps_digest` (`ok=true`, `steps=4`, `digest=788e7160d63e2821`), promotes the
  passing candidate (`gate_pass=true`), and verifies a two-arm NIP-EVAL report
  (`verified=true`, `pass=true`).
- **Refusal:** a 1-byte mutation to the `ATIF-v1.8` log fails `atif_verify_log`
  (`tamper_atif_ok=false`) and refuses promotion (`CAP_FAIL_ATIF_UNVERIFIED`,
  `8`); overall regression refuses (`3`); held-out regression refuses (`4`); and
  a mismatched arm coverage sum in `openagents.eval-report.v1` refuses with
  `EV_COVERAGE` (`bad_coverage_refused=true`).
- **Independent check:** `coax/bin/coax run openagents/examples/two-arm-atif-eval.coax`
  and `--release` match [`openagents/examples/two-arm-atif-eval.out`](../examples/two-arm-atif-eval.out),
  independently cross-checked by [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax),
  `capability-eval.coax selftest`, and `oa-worker --selftest-capability`.

Planned paths: `openagents/eval/run.coax`.

## Sources

- **Episodes:**
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): comparing configurations on a benchmark
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:30, "Run it"
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): with-and-without thinking
- **Old code at `coder-v1.0.0`, crates:** `eval-runner`, `ext-eval`, `gym`, `workflow-run`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-ATIF](../../nostr/nips/openagents/NIP-ATIF.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "Capability delta"
  - [Coder book, chapter 21: Measure the harness](../../coder/book/ch021-measure-the-harness.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P15

## Open questions for the owner

1. How many cases does a test set need before its result may be published? The
   first adoption used six.

## Source listing

Run the Chapter 45 example in [`openagents/examples/two-arm-atif-eval.coax`](../examples/two-arm-atif-eval.coax) (committed transcript [`openagents/examples/two-arm-atif-eval.out`](../examples/two-arm-atif-eval.out)):

<!-- source: openagents/examples/two-arm-atif-eval.coax two-arm-atif-eval -->
```jai
tae_run :: () -> bool {
  // 1. Build canonical 4-step two-arm ATIF-v1.8 log and compute SHA-256 steps_digest.
  log_buf: [CE_BUF] u8;
  used := 0;
  line_b: [CE_LINE] u8;
  step_b: [CE_LINE] u8;
  scratch: [CE_LINE] u8;

  hdr := AtifSessionHeader.{
    trajectory_id = "fast-lint",
    session_id = "fast-lint",
    schema_version = ATIF_SCHEMA_VERSION,
    agent_name = "capability-eval",
    model_name = "two-arm-eval",
    cwd = ".",
    git_branch = "",
    source_harness = "openagents-desktop",
    created_at = "2026-11-26T00:00:00Z"
  };
  sn := atif_write_session_line(hdr, line_b[..]);
  used = ce_append(log_buf[..], used, cast(string) line_b[0..<sn]);

  hasher: [1] AtifStepsHasher;
  atif_steps_hash_init(hasher[..]);
  msgs := [4] string.[
    "arm=baseline case=positive exit=1",
    "arm=candidate case=positive exit=0",
    "arm=baseline case=heldout exit=0",
    "arm=candidate case=heldout exit=0"
  ];
  for k: 0..<4 {
    st := AtifStep.{
      step_id = k + 1,
      source = ATIF_SOURCE_AGENT,
      message = msgs[k],
      model_name = "two-arm-eval"
    };
    jn := atif_write_step_json(st, step_b[..]);
    _ = atif_steps_hash_step(hasher[..], cast(string) step_b[0..<jn], scratch[..]);
    ln := atif_write_step_line(cast(string) step_b[0..<jn], line_b[..]);
    used = ce_append(log_buf[..], used, cast(string) line_b[0..<ln]);
  }
  en := atif_write_end_line("2026-11-26T00:00:01Z", ATIF_STATE_ENDED, line_b[..]);
  used = ce_append(log_buf[..], used, cast(string) line_b[0..<en]);

  digest: [64] u8;
  _ = atif_steps_hash_final(hasher[..], digest[..]);
  digest_str := cast(string) digest[0..<64];
  vsum := atif_verify_log(cast(string) log_buf[0..<used], digest_str);

  // 2. Evaluate passing two-arm summary through #authority gate & format verdict card.
  pass_sum := CapEvalSummary.{
    total_cases = 2, heldout_cases = 1,
    baseline_passed = 1, candidate_passed = 2,
    baseline_heldout_passed = 1, candidate_heldout_passed = 1,
    candidate_latency_ms = 25, max_latency_ms = 500,
    atif_verified = vsum.ok && vsum.step_count == 4
  };
  v_pass := cap_admit_promotion(pass_sum);
  print("atif ok=% steps=% digest=% gate_pass=%\n",
    vsum.ok, vsum.step_count, digest_str[0..<16], v_pass == CAP_PASS);

  // 3. Fail-closed refusals: 1-byte tampered ATIF log, overall regression, held-out regression.
  tampered: [CE_BUF] u8;
  for i: 0..<used { tampered[i] = log_buf[i]; }
  for i: 0..<used {
    if tampered[i] == '1' && i > sn { tampered[i] = '0'; break; }
  }
  vbad := atif_verify_log(cast(string) tampered[0..<used], digest_str);
  tamper_sum := pass_sum;
  tamper_sum.atif_verified = vbad.ok;
  v_tamper := cap_admit_promotion(tamper_sum);

  reg_sum := pass_sum;
  reg_sum.baseline_passed = 2; reg_sum.candidate_passed = 1;
  v_reg := cap_admit_promotion(reg_sum);

  ho_sum := pass_sum;
  ho_sum.candidate_heldout_passed = 0;
  v_ho := cap_admit_promotion(ho_sum);
  print("refusals tamper_atif_ok=% tamper_code=% regressed_code=% heldout_code=%\n",
    vbad.ok, v_tamper, v_reg, v_ho);

  // 4. Independent NIP-EVAL check: two-arm coverage sums verified vs mismatched.
  held := [2] string.[EV_T_SUITE, EV_T_PART];
  trusted := [1] string.[EV_T_E1];
  rep_ok := eval_report(EV_T_REPORT, held[..], trusted[..]);
  bad_cov_buf: [4096] u8;
  bad_cov_json := ts_swap(EV_T_REPORT, "\"completed\":9", "\"completed\":8", bad_cov_buf[..]);
  rep_bad := eval_report(bad_cov_json, held[..], trusted[..]);
  print("nip_eval verified=% pass=% bad_coverage_refused=%\n",
    rep_ok.code == EV_VERIFIED, rep_ok.verdict == EV_PASS, rep_bad.code == EV_COVERAGE);

  return vsum.ok && vsum.step_count == 4 && v_pass == CAP_PASS &&
         !vbad.ok && v_tamper == CAP_FAIL_ATIF_UNVERIFIED &&
         v_reg == CAP_FAIL_REGRESSED && v_ho == CAP_FAIL_HELDOUT_REGRESSED &&
         rep_ok.code == EV_VERIFIED && rep_ok.verdict == EV_PASS && rep_bad.code == EV_COVERAGE;
}
```

Output (`coax/bin/coax run openagents/examples/two-arm-atif-eval.coax`, identical in `--release`):

```text
atif ok=true steps=4 digest=788e7160d63e2821 gate_pass=true
refusals tamper_atif_ok=false tamper_code=8 regressed_code=3 heldout_code=4
nip_eval verified=true pass=true bad_coverage_refused=true
ok=true
```

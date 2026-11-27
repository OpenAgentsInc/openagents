# 43. Test-time capabilities

> **Status: partly implemented.** The `#authority` NIP-EVAL claim and admission
> reader ([`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax))
> and local `#authority` capability promotion gate
> ([`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax),
> `#13896`, `#14059`) run on `cpu`, grounded by
> [`openagents/examples/test-time-capability-claims.coax`](../examples/test-time-capability-claims.coax)
> and [`openagents/examples/test-time-capability-claims.out`](../examples/test-time-capability-claims.out),
> and are adopted by [`openagents/registry/registry.coax`](../../openagents/registry/registry.coax),
> [`coax/lib/agent/desktop/capability-author.coax`](../../coax/lib/agent/desktop/capability-author.coax),
> and [`coder/tui/oa-worker.coax`](../../coder/tui/oa-worker.coax). Remaining:
> unified `openagents.eval.v1` protobuf schemas and cross-claim policy records in
> `openagents/eval/`.

## Premise

Chapters 12 to 42 fill the agent with members. This chapter asks whether each
one helps, and gives the vocabulary for the answer: a candidate, a claim,
adoption and credit.

## Draft

Test-time compute means spending more computation when a model answers: longer
reasoning, more samples, a search. It helps, and the published work says so.

Test-time capabilities go one step further. An agent can gain or lose
abilities while it runs, with no change to its weights, when a tool, a plugin,
a skill, a knowledge entry or another agent is admitted into the run. Episode
289 names this as the second of two ideas that make a general agent possible.
Decision models are the first.

"Lose" is the important word. A member can make an agent worse as easily as
better. Skills that an agent wrote for itself have been measured doing worse
than no skills at all. So a plugin that is installed, described or shown in a
demo proves nothing.

The essay sets out four statements, and this part builds each one:

- A component is a candidate capability.
- Evidence makes a claim. A claim says what admitting one identified component
  to one baseline agent changed, on one kind of task, under one grant, by one
  measurement.
- Adoption is policy. Turning evidence into "everyone gets this" is a separate
  decision with written rules.
- Credit goes to verification. The people who check a claim earn credit, and
  so do authors whose work is adopted.

Most of the mechanisms are prior art. What the essay proposes is the chain:
candidate, admission, a controlled comparison, reproduction, outside
validation, adoption, credit, and checking again when anything changes. The
essay is the specification, and the next nine chapters make it run.

## What we build

A plan for what this chapter ships once the draft is approved:

- The record types for a candidate, a claim, an evidence record and a policy.
- A glossary page for this part's terms, linked from every chapter.
- No runner yet. This chapter fixes the words so the later ones can be checked
  against them.

It builds on what exists today:

- [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax) (`#authority` `openagents.eval-report.v1` claim verifier and `openagents.eval-admission.v1` adoption policy)
- [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax) (`#authority` `CapEvalSummary` local promotion gate, `#13896`, `#14059`)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`docs/books/glossary.md`](../../docs/books/glossary.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A claim and its evidence | `openagents.eval.v1` `Publication` ([spec 21.1](../../docs/protocol/openagents-protocol-spec.md#211-openagentsevalv1)); evidence types are in `openagents.common.v1` ([spec 9.4](../../docs/protocol/openagents-protocol-spec.md#94-evidence-observation-and-context-manifests)) | Nostr kind 3189, public (NIP-EVAL) | Spec sketch; no `.proto` file. `coax/lib/nostr/eval` handles the NIP's JSON |
| Policy | `openagents.policy.v1` ([spec 19.2](../../docs/protocol/openagents-protocol-spec.md#192-openagentspolicyv1)) | Nostr (NIP-POL) | Spec sketch; no `.proto` file |

### The chapter's check

- **Source:** [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax),
  [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax),
  [`openagents/examples/test-time-capability-claims.coax`](../examples/test-time-capability-claims.coax),
  and [`openagents/examples/test-time-capability-claims.out`](../examples/test-time-capability-claims.out)
  (remaining: unified `openagents/eval/` protobuf claim records).
- **Trace:** reads an `openagents.eval-report.v1` claim for `x:pkg/agent`
  against held suite and partition digests and a trusted evaluator
  (`code=verified`, `verdict_pass=true`), admits it under an unexpired operator
  policy (`admit=ok`), and confirms local `#authority` promotion (`pass_code=0`).
- **Refusal:** an unheld suite digest downgrades the claim to `unverifiable`;
  an untrusted evaluator downgrades it to `untrusted`; a subject-only report
  (`baseline: null`) carrying a `comparison` measurement is refused as a claim
  (`measurement`); expired, untrusted-issuer, unverified-report, wrong-subject,
  and failing-verdict admissions are refused (`expired`, `issuer`, `unverified`,
  `subject`, `verdict`); and an unverified ATIF digest fails the local gate
  (`unverified_code=8`).
- **Independent check:** `coax/bin/coax run openagents/examples/test-time-capability-claims.coax`
  and `--release` match [`openagents/examples/test-time-capability-claims.out`](../examples/test-time-capability-claims.out),
  cross-checking the NIP-EVAL report/admission reader in
  [`coax/lib/nostr/eval/report.coax`](../../coax/lib/nostr/eval/report.coax)
  against the local `#authority` promotion gate in
  [`coax/lib/agent/desktop/capability-gate.coax`](../../coax/lib/agent/desktop/capability-gate.coax).

Planned paths: `openagents/eval/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 4:00, test-time capabilities
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): reusable components need evidence
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): measured components
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): benchmark discipline
- **Old code at `coder-v1.0.0`, crates:** `ext-eval`, `eval-runner`, `gym`
- **NIPs:** [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-KB](../../nostr/nips/openagents/NIP-KB.md)
- **Also:**
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), Part I: the concept and the lifecycle figure
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P15

## Source listing

Run the Chapter 43 example in [`openagents/examples/test-time-capability-claims.coax`](../examples/test-time-capability-claims.coax) (committed transcript [`openagents/examples/test-time-capability-claims.out`](../examples/test-time-capability-claims.out)):

<!-- source: openagents/examples/test-time-capability-claims.coax test-time-capability-claims -->
```jai
ttc_run :: () -> bool {
  held := [2] string.[EV_T_SUITE, EV_T_PART];
  trusted := [1] string.[EV_T_E1];
  ops := [1] string.["op"];

  // 1. Read verified claim and admit it under operator policy.
  view := eval_report(EV_T_REPORT, held[..], trusted[..]);
  seen := [1] EvalSeen.[EvalSeen.{digest = EV_T_R1, view = view}];
  admit_ok := eval_admit(EV_T_ADMIT, seen[..], ops[..], 1790000000);
  print("claim code=% subject=% verdict_pass=% admit=%\n",
    ttc_ev_name(view.code), view.subject, view.verdict == EV_PASS, ttc_ea_name(admit_ok));

  // 2. Report refusals: unheld suite, untrusted evaluator, comparison without baseline.
  v_unverif := eval_report(EV_T_REPORT, held[0..<1], trusted[..]);
  v_untrust := eval_report(EV_T_REPORT, held[..], trusted[0..<0]);

  b1: [4096] u8;
  b2: [4096] u8;
  no_base := ts_swap(EV_T_REPORT, "\"baseline\":{\"definition\":{\"id\":\"x:pkg/base\"}}", "\"baseline\":null", b1[..]);
  no_base_cov := ts_swap(no_base, "\"baseline\":{\"planned\":10,\"attempted\":10,\"completed\":7,\"refused\":0,\"failed\":3,\"cancelled\":0,\"unknown\":0,\"excluded\":0}", "\"baseline\":null", b2[..]);
  v_nobase := eval_report(no_base_cov, held[..], trusted[..]);
  print("report_refusals unheld=% untrusted=% no_baseline=%\n",
    ttc_ev_name(v_unverif.code), ttc_ev_name(v_untrust.code), ttc_ev_name(v_nobase.code));

  // 3. Admission refusals: expired, untrusted issuer, unverified report, wrong subject, failing verdict.
  ea_exp := eval_admit(EV_T_ADMIT, seen[..], ops[..], 1790090000);
  no_ops := [1] string.["other"];
  ea_iss := eval_admit(EV_T_ADMIT, seen[..], no_ops[..], 1790000000);
  bad_seen := [1] EvalSeen.[EvalSeen.{digest = EV_T_R1, view = EvalView.{code = EV_UNVERIFIABLE, verdict = EV_PASS, subject = "x:pkg/agent"}}];
  ea_unv := eval_admit(EV_T_ADMIT, bad_seen[..], ops[..], 1790000000);
  wrong_subj := [1] EvalSeen.[EvalSeen.{digest = EV_T_R1, view = EvalView.{code = EV_VERIFIED, verdict = EV_PASS, subject = "x:pkg/other"}}];
  ea_sub := eval_admit(EV_T_ADMIT, wrong_subj[..], ops[..], 1790000000);
  fail_seen := [1] EvalSeen.[EvalSeen.{digest = EV_T_R1, view = EvalView.{code = EV_VERIFIED, verdict = EV_FAIL, subject = "x:pkg/agent"}}];
  ea_ver := eval_admit(EV_T_ADMIT, fail_seen[..], ops[..], 1790000000);
  print("admit_refusals expired=% issuer=% unverified=% subject=% verdict=%\n",
    ttc_ea_name(ea_exp), ttc_ea_name(ea_iss), ttc_ea_name(ea_unv), ttc_ea_name(ea_sub), ttc_ea_name(ea_ver));

  // 4. Local #authority promotion gate mirrors the evidence requirement.
  s_ok := CapEvalSummary.{
    total_cases = 10, heldout_cases = 3, baseline_passed = 7, candidate_passed = 9,
    baseline_heldout_passed = 2, candidate_heldout_passed = 3, atif_verified = true
  };
  s_no_atif := s_ok;
  s_no_atif.atif_verified = false;
  g_ok := cap_admit_promotion(s_ok);
  g_bad := cap_admit_promotion(s_no_atif);
  print("local_gate pass_code=% unverified_code=%\n", g_ok, g_bad);

  return view.code == EV_VERIFIED && view.verdict == EV_PASS && view.subject == "x:pkg/agent" &&
         admit_ok == EA_OK && v_unverif.code == EV_UNVERIFIABLE && v_untrust.code == EV_UNTRUSTED &&
         v_nobase.code == EV_MEASUREMENT && ea_exp == EA_EXPIRED && ea_iss == EA_ISSUER &&
         ea_unv == EA_UNVERIFIED && ea_sub == EA_SUBJECT && ea_ver == EA_VERDICT &&
         g_ok == CAP_PASS && g_bad == CAP_FAIL_ATIF_UNVERIFIED;
}
```

Output (`coax/bin/coax run openagents/examples/test-time-capability-claims.coax`, identical in `--release`):

```text
claim code=verified subject=x:pkg/agent verdict_pass=true admit=ok
report_refusals unheld=unverifiable untrusted=untrusted no_baseline=measurement
admit_refusals expired=expired issuer=issuer unverified=unverified subject=subject verdict=verdict
local_gate pass_code=0 unverified_code=8
ok=true
```

# 69. Models we serve

> **Status: partly implemented.** The `#authority` hybrid executor runtime
> ([`psionic/lib/hybrid-exec.coax`](../../psionic/lib/hybrid-exec.coax)) covering
> all 8 Class-E compiled weight modules + 2 linked composites (`178/178` tests,
> `0` surviving mutants) runs on `cpu` with its runnable book example
> [`openagents/examples/psionic-hybrid-serve.coax`](../examples/psionic-hybrid-serve.coax)
> and committed debug/release transcript
> [`openagents/examples/psionic-hybrid-serve.out`](../examples/psionic-hybrid-serve.out),
> beside the Metal 4 `Inkling-2-Mini` GGUF + rank-8 LoRA routing verification
> ([W3.1 record](../../docs/verification/executor-w31-hybrid-exec.md)).
> Remaining: side-by-side chapter 16 front benchmark over `POST /v1/systemone`
> against a hosted decision route.

## Premise

The gateway reaches other people's models. This chapter adds our own: Psionic,
the engine that runs, serves and trains models in Coax, starting with the
decision models the front depends on.

## Draft

An agent whose every thought is rented can be switched off by its landlord.
The front makes a decision on every turn, so the decision model is the first
one we want on hardware we control.

Psionic is our machine-learning engine. [Episode
216](../../docs/transcripts/216.md) introduces it as an inference and training
stack. It was first written in Rust and is now being rebuilt in Coax, with its
own book that goes from tensors to serving and training.

Decision models suit owned hardware. They are small. [Episode
290](../../docs/transcripts/290.md) names the open decision models a Pylon
runs today and says one answers in a quarter of a second on an ordinary gaming
computer. A small general model handles simple tool use beside them.

Serving is the first half. The second is that a decision can be checked. If
the same model, the same state and the same question give the same
probabilities, a second machine can recompute an answer and compare. That is
what lets a stranger's machine serve decisions for pay in the next chapter.

Training is the long road. [Episodes 224](../../docs/transcripts/224.md) and
[238](../../docs/transcripts/238.md) are about training across many machines,
and the records from chapters 12 to 18 are the kind of data a decision model learns from,
where people consent. Training a better front is a plan and has no date.

This chapter builds little that is new. It connects the front to a model that
Psionic serves and measures it with chapter 16's test set against the hosted
route.

## What we build

A plan for what this chapter ships once the draft is approved:

- The front's decisions served by Psionic on our own hardware, behind the
  gateway.
- A side-by-side run of chapter 16's test set: our served model against the
  hosted route, with accuracy, time and cost.
- A recompute check: a second machine reproduces a served decision.

It builds on what exists today:

- [`psionic/lib/decision-models.coax`](../../psionic/lib/decision-models.coax)
- [`psionic/lib/decision-session.coax`](../../psionic/lib/decision-session.coax)
- [`psionic/lib/decision-calibration.coax`](../../psionic/lib/decision-calibration.coax)
- [`psionic/lib/hybrid-exec.coax`](../../psionic/lib/hybrid-exec.coax) and [`psionic/lib/executor-desktop.coax`](../../psionic/lib/executor-desktop.coax) (real 16.38 GB `Inkling-2-Mini` GGUF on Metal 4 + rank-8 LoRA routing adapter + frozen W2.2 compiled weight cores: `11/48 → 48/48` on held-out tasks, `0` silent hallucinations; [W3.1 verification record](../../docs/verification/executor-w31-hybrid-exec.md), [Psionic ch236](../../psionic/book/ch236-a-frozen-exact-core-with-a-learned-interface.md), [ch237](../../psionic/book/ch237-composable-weight-modules.md), [ch269](../../psionic/book/ch269-inkling-2-mini-on-metal.md))
- `psionic-inkling-serve` across Apple Silicon Metal 4 (`110–150+` tok/s decode, `Q8S`/`Q8_0` KV cache, `MTLBinaryArchive`) and Linux/Windows NVIDIA CUDA (`75.7+` tok/s RTX 4080 decode, 8-variant GGUF catalog `#14028`, cloud-first onboarding with `$0.00` local tier `#14087`; [Psionic ch269–271](../../psionic/book/ch269-inkling-2-mini-on-metal.md))
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coax/lib/gateway/psionic.coax`](../../coax/lib/gateway/psionic.coax)
- [`psionic/book/README.md`](../../psionic/book/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A served decision | `openagents.decision.v1alpha1` `SystemOneRequest`, `SystemOneResponse` ([`systemone.proto`](../../proto/openagents/decision/v1alpha1/systemone.proto)) | HTTP `POST /v1/systemone` on Psionic | In use on the server: the generated codec `psionic/lib/serve-proto.coax` |

### The chapter's check

- **Source:** [`psionic/lib/hybrid-exec.coax`](../../psionic/lib/hybrid-exec.coax),
  [`psionic/lib/module-v2-wave2.coax`](../../psionic/lib/module-v2-wave2.coax),
  [`openagents/examples/psionic-hybrid-serve.coax`](../examples/psionic-hybrid-serve.coax),
  and the committed stdout transcript [`openagents/examples/psionic-hybrid-serve.out`](../examples/psionic-hybrid-serve.out)
  (remaining: side-by-side chapter 16 test set over `POST /v1/systemone`).
- **Trace:** compiles the Ch 53 constrained DFA call grammar (`ch53_dfa_arity7=true`),
  invokes frozen Class-E cores `oa-exec-patch-rebase` (`1,28,102`, `steps=19`,
  `replay=true`) and `oa-exec-semver-solve` (`1,2,1`, `steps=11`, `replay=true`),
  and verifies every Ch 234 exact-replay receipt.
- **Refusal:** a dependency DAG cycle returns typed `<exec_refuse reason="cycle_detected">`
  (`inject_len=98`); flipping 1 bit in the replay trace fails receipt verification
  (`tamper_refused=true`).
- **Independent check:** `coax/bin/coax run openagents/examples/psionic-hybrid-serve.coax`
  and `--release` produce identical transcripts matching
  [`openagents/examples/psionic-hybrid-serve.out`](../examples/psionic-hybrid-serve.out),
  beside `coax/bin/coax test psionic/lib/hybrid-exec.coax` (`178/178`) and
  `psionic/examples/hybrid-exec.coax` (`10/10` exact replay across all 8 modules
  and 2 composites, #13962).

Planned paths: none. Serving is Psionic's ([`psionic/`](../../psionic)).

## Sources

- **Episodes:**
  - [Episode 216, *Psionic*](../../docs/transcripts/216.md)
  - [Episode 217, *Psionic: Fast Qwen 3.5*](../../docs/transcripts/217.md): fast inference on an open model
  - [Episode 224, *Distributed Training 101*](../../docs/transcripts/224.md): distributed training
  - [Episode 236, *Tassadar*](../../docs/transcripts/236.md): a model that executes
  - [Episode 238, *The Training Run Begins*](../../docs/transcripts/238.md)
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 25:00, decision models on gaming computers
  - [Episode 145, *Going Local*](../../docs/transcripts/145.md)
- **Old code at `coder-v1.0.0`, crates:** `psionic`, `inference`, `jev-hosted`, `model-access`
- **NIPs:** [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md)
- **Also:**
  - [Psionic: ML From Scratch](../../psionic/book/README.md), the decision chapters
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7

## Open questions for the owner

1. Which models does 1.0 serve on our own hardware, and which stay hosted?

## Source listing

Run the Chapter 69 example in [`openagents/examples/psionic-hybrid-serve.coax`](../examples/psionic-hybrid-serve.coax) (committed transcript [`openagents/examples/psionic-hybrid-serve.out`](../examples/psionic-hybrid-serve.out)):

<!-- source: openagents/examples/psionic-hybrid-serve.coax psionic-hybrid-serve -->
```jai
phs_run :: () -> bool {
  // 1. Ch 53 constrained byte-level DFA call grammar enforces exact argument arity.
  edges: [HX_GRAMMAR_EDGES] NxEdge;
  next: [HX_GRAMMAR_STATES * 256] int;
  acc: [HX_GRAMMAR_STATES] u8;
  live: [HX_GRAMMAR_STATES] u8;
  g7 := hx_build_arg_grammar(7, edges[..], next[..], acc[..], live[..]);
  g7_ok := g7.status == NxStatus.ok &&
           nx_accepts(g7.grammar, "10,4,7,25,3,2,100</exec_arg></exec_call>") &&
           !nx_accepts(g7.grammar, "10,4,7,25,3,2</exec_arg></exec_call>");
  print("ch53_dfa_arity7=%\n", g7_ok);

  // 2. Compile frozen Class-E cores and invoke through the hybrid ABI with Ch 234 exact replay.
  l1: [MV_MAX_LAYERS] EvLayerDesc; h1: [MV_MAX_HEADS] EvHeadDesc; w1: [65536] f64;
  l2: [MV_MAX_LAYERS] EvLayerDesc; h2: [MV_MAX_HEADS] EvHeadDesc; w2: [65536] f64;
  l3: [MV_MAX_LAYERS] EvLayerDesc; h3: [MV_MAX_HEADS] EvHeadDesc; w3: [65536] f64;
  m_pr := mv_compile_patch_rebase(l1[..], h1[..], w1[..]);
  m_sv := mv_compile_semver_solve(l2[..], h2[..], w2[..]);
  m_dep := mv_compile_dep_sched(l3[..], h3[..], w3[..]);

  sb: [65536] u8;
  s_pr := ev_serialize(m_pr, sb[..]);
  s_sv := ev_serialize(m_sv, sb[..]);
  s_dep := ev_serialize(m_dep, sb[..]);

  pc1 := hx_parse_call("<exec_call><exec_module>oa-exec-patch-rebase</exec_module><exec_arg>10,4,7,25,3,2,100</exec_arg></exec_call>");
  r1 := hx_invoke_core(pc1.module_id, m_pr, s_pr.digest, true, pc1.args[0..<pc1.n_args]);
  v1 := hx_verify_receipt(m_pr, s_pr.digest, r1);
  print("patch_rebase=%,%,% steps=% replay=%\n", r1.outputs[0], r1.outputs[1], r1.outputs[2], r1.rx_claim.steps, v1);

  pc2 := hx_parse_call("<exec_call><exec_module>oa-exec-semver-solve</exec_module><exec_arg>1,2,0,1,2,0,4</exec_arg></exec_call>");
  r2 := hx_invoke_core(pc2.module_id, m_sv, s_sv.digest, true, pc2.args[0..<pc2.n_args]);
  v2 := hx_verify_receipt(m_sv, s_sv.digest, r2);
  print("semver_solve=%,%,% steps=% replay=%\n", r2.outputs[0], r2.outputs[1], r2.outputs[2], r2.rx_claim.steps, v2);

  // 3. Refusals: DAG cycle triggers typed <exec_refuse reason="cycle_detected">; 1-bit receipt tamper fails verification.
  pc3 := hx_parse_call("<exec_call><exec_module>oa-exec-dep-sched</exec_module><exec_arg>0,1,14,64,520,0</exec_arg></exec_call>");
  r3 := hx_invoke_core(pc3.module_id, m_dep, s_dep.digest, true, pc3.args[0..<pc3.n_args]);
  ibuf: [HX_INJECT_MAX] u8;
  ilen := hx_format_injection(r3, ibuf[..]);
  tampered := r1;
  tampered.rx_claim.trace[0] ^= 1;
  tamper_ok := !hx_verify_receipt(m_pr, s_pr.digest, tampered);
  print("cycle_reason=% inject_len=% tamper_refused=%\n", hx_reason_name(r3.reason), ilen, tamper_ok);

  return g7_ok && v1 && v2 && r3.reason == HxExecReason.cycle_detected && tamper_ok;
}
```

Output (`coax/bin/coax run openagents/examples/psionic-hybrid-serve.coax`, identical in `--release`):

```text
ch53_dfa_arity7=true
patch_rebase=1,28,102 steps=19 replay=true
semver_solve=1,2,1 steps=11 replay=true
cycle_reason=cycle_detected inject_len=98 tamper_refused=true
ok=true
```

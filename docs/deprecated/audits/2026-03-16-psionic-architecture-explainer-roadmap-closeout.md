# 2026-03-16 Psionic Architecture Explainer Roadmap Closeout

## Scope

This note closes the umbrella roadmap issue `#3650`:

> ship the first real Apple adapter run as `Psionic architecture explainer`

It does not claim that the first run was accepted.
It claims that the roadmap produced the real target contract, the real
toolkit-backed Apple-valid export path, the real validation gate, the real
single-host reference run, and the real benchmarking addendum.

## What Closed Under This Roadmap

The critical Apple-run issues under this roadmap are now complete:

- `#3663` replaced the fake placeholder `adapter_weights.bin` path with the
  toolkit-backed Apple-valid runtime-asset export lane
- `#3664` added the canonical parity plus live bridge acceptance gate
- `#3658` executed the first real single-host `Psionic architecture explainer`
  run and wrote a durable rejected run report
- `#3659` wrote the timing, footprint, bottleneck, and minimum-profile
  addendum for that real run

The durable artifacts are:

- `crates/psionic/fixtures/apple_adapter/runs/psionic_architecture_explainer_first_real_run_report.json`
- `docs/audits/2026-03-15-psionic-architecture-explainer-first-real-run.md`
- `crates/psionic/fixtures/apple_adapter/runs/psionic_architecture_explainer_first_real_run_benchmark_addendum.json`
- `docs/audits/2026-03-16-psionic-architecture-explainer-first-real-run-benchmark-addendum.md`

## Final Umbrella Outcome

The roadmap succeeded in the sense that it delivered the first real Apple
adapter reference cycle for the frozen `Psionic architecture explainer`
contract.

The first real run outcome itself was:

- run id: `psionic-architecture-explainer-first-real-run-1773636619824`
- disposition: `RejectedBenchmark`
- operator lane: toolkit-backed training plus export, repo-owned orchestration
- benchmark result: base and adapted paths both scored `0`
- runtime-smoke failure: tokenizer-lineage drift (`expected ...2485db`, observed empty)

So the umbrella is complete, but the first run was rejected.

That is the right outcome to preserve.

## What This Roadmap Did Not Do

Closing `#3650` does not mean:

- the Apple lane is now accepted for production-quality `Psionic architecture`
  behavior
- the tokenizer-lineage/runtime-smoke drift is solved
- the repeated Apple Intelligence compatibility popup is solved
- the current Apple lane is Rust-native end to end
- clustered or mixed-backend Apple training exists

Those are follow-on issues, not reasons to keep the roadmap umbrella open after
the real run and addendum already landed.

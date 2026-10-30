# Native public-pool policy — 2026-10-30

The pool CLI uses the checked receipt, totals, pool-wire and public-bundle
units. Its effectful shell owns original signed-event storage for the
entire reduction; decoded DTO strings never point into reused relay scratch.
Native build and end-to-end evidence are recorded separately from these
pure policy checks.

Two new authored admission roots qualified with the scoped mutation compiler:

| Root | Baseline checks | Authored mutants | Killed | Survivors |
| --- | ---: | ---: | ---: | ---: |
| `mesh/pool-policy` | 273 | 40 | 40 | 0 |
| `mesh/pool-input-policy` | 277 | 25 | 25 | 0 |

The first root checks closed minute windows, bounded IO budgets, safe CLI
mode combinations, capacities and explicit author roles. The second checks
public-only event kinds, observed-response saturation/failure, provider
allowlists before receipt/label storage, and refusal of unverified ownership
or invalid beacon slugs. The original 40-mutant gate was retained unchanged
when the separate input-policy root was added. No equivalence waivers.

`--root-only` selects the physical entry source's mutation sites; all imported
baseline checks still run. It does not claim the new shell's complete native
program has been mutation tested. Native signed/encrypted protocol, transport,
wire and lifetime cases supplement these pure policy gates.

[Exact source closures, compiler hashes and transcripts](data/oct30-mesh-pool-policy/)
identify the qualified inputs. The scoped compiler adds mutation inventory
selection; native code generation uses the previously qualified compiler
`df4d78ef4ddc8f788cb4a87da31d538a2ecf174af14085a5f34ab1d868a80fb0`.

The protocol remains free-only with throwaway keys. Verification means that
supplied or observed signed inputs reproduce the advertised arithmetic and
commitments; relay EOSE is not proof of global completeness. The transport
conservatively refuses a 225th response even if it is duplicate/irrelevant.
Role-specific queries use at most 64 authors each and provider/namespace
filters; no `since=window.from` shortcut loses earlier canonical receipts.
Historical addressable beacons can be unavailable after replacement. Keep
the public bundle for stable independent recomputation; missing history is
not evidence that a provider was offline.

# OpenAgents first MVP

This directory is the canonical product-definition package for the first
deployable OpenAgents shape: a ProductSpec-native, local-first Codex workroom.
It keeps the committed intent and its supporting audit separate and easy to
read.

## Read in this order

1. [`openagents-codex-workroom-mvp.product-spec.md`](./openagents-codex-workroom-mvp.product-spec.md)
   — the exact ProductSpec v0.1 intent artifact. It owns the MVP problem,
   hypothesis, in/out/cut scope, user experience, solution, stable acceptance
   criteria, success metrics, risks, owner gates, and required receipts.
2. [`2026-07-13-openagents-codex-workroom-mvp-audit.md`](./2026-07-13-openagents-codex-workroom-mvp-audit.md)
   — the dated OpenChamber/OpenCode/Codex/OpenAgents evidence and option analysis
   behind that spec. It does not dispatch work or manufacture current proof.
3. [`../sol/MASTER_ROADMAP.md`](../sol/MASTER_ROADMAP.md) — the sequencing,
   priority, live-gate, and issue-triage authority.
4. [MVP-01 #8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
   — the closed-completed implementation/evidence ledger for this MVP.
5. [`../assurance/MVP_FIRST_ASSURANCESPEC.md`](../assurance/MVP_FIRST_ASSURANCESPEC.md)
   — the original first-dogfood plan. The live admitted companion and its
   reviewed receipts now execute that plan without changing MVP intent,
   release state, or promise authority.
6. [`2026-07-13-openagents-codex-workroom-rc6-candidate-receipt.md`](./2026-07-13-openagents-codex-workroom-rc6-candidate-receipt.md)
   — the narrow signed/notarized RC6 build, installed-smoke, restore, cleanup,
   and remaining-gates receipt.
7. [`2026-07-13-openagents-codex-workroom-rc7-candidate-receipt.md`](./2026-07-13-openagents-codex-workroom-rc7-candidate-receipt.md)
   — the signed/notarized RC7 artifact, source-launched real-Codex journey, and
   installed-driver falsifier for the ordinary logged-in-session MVP boundary.
8. [`2026-07-13-openagents-codex-workroom-rc8-candidate-receipt.md`](./2026-07-13-openagents-codex-workroom-rc8-candidate-receipt.md)
   — the signed/notarized RC8 artifact, exact installed 12-step real-Codex
   journey, renderer/app restart, and exact RC7-to-RC8 signed-update,
   rollback, diagnostics, reinstall, and cleanup receipt.
9. [`2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md`](./2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md)
   — the superseding signed/notarized candidate containing the distinct quota,
   rate-limit, authentication-revocation, and policy-denial states, plus exact
   installed real-Codex and RC8-to-RC9 lifecycle proof.
10. [`2026-07-13-openagents-codex-workroom-rc9-completion-audit.md`](./2026-07-13-openagents-codex-workroom-rc9-completion-audit.md)
    — the criterion-by-criterion implementation/evidence matrix and the exact
    boundary between accepted implementation and conditional rollout gates.
11. [`2026-07-13-openagents-codex-workroom-mvp-closure-receipt.md`](./2026-07-13-openagents-codex-workroom-mvp-closure-receipt.md)
    — the exact owner acceptance, close-rule disposition, and boundaries that
    permit #8756 to close without implying publication or broader rollout.

The Product Spec declares intent. Current upstream ProductSpec can also index
external evidence with Related Artifacts, but the local package has not yet
implemented that `0.19.0` feature. Runtime policy, behavior contracts, Eval
Suites, tests, reviewed artifacts, and receipts provide evidence; owner and
release policies decide what it permits; the promise registry alone authorizes
public claims.

## ProductSpec location and validation

The MVP Product Spec is intentionally co-located with its audit here by owner
direction. It remains a normal `.product-spec.md` file and is validated by the
repository ProductSpec test sweep alongside `specs/**/*.product-spec.md`.

```sh
bun packages/product-spec/src/cli.ts validate \
  docs/mvp/openagents-codex-workroom-mvp.product-spec.md
bun test packages/product-spec/test/product-spec.test.ts
```

Do not create a second copy under `specs/`; links, issues, dispatch prompts, and
future decision traces should cite this path plus `spec_revision`.

## Current ProductSpec workroom loop

OpenAgents Desktop already implements a ProductSpec workroom loop in
`apps/openagents-desktop/src/product-spec-workroom*`. It persists accepted
plans, criterion-mapped packets, dependencies, mutation leases, evidence
envelopes, verification receipts whose `verifierRef` differs from the host
evidence-producer ref, and owner packet disposition. The current host check is
reference inequality, not authenticated identity proof.

That loop is useful without AssuranceSpec. It answers who accepted the plan,
which packet ran, which evidence ref was registered, who verified it, and
whether the owner accepted or waived the packet. It does not decide before the
build which oracle is adequate, which falsifier it must reject, which
environment/proof rung counts, whether a full criterion is assured, whether a
release may proceed, or whether a public promise is green.

The current host verification receipt has only `passed` and treats
`evidenceRef` as opaque. A future Assurance integration therefore needs a typed
resolver that validates a current `CONFIRMED` Assurance Receipt and issues an
immutable RefSchema-safe handle before `evidenceKind: receipt` registration.
It must separately enforce Assurance producer/reviewer policy and must not
convert `REFUTED`, `INCONCLUSIVE`, stale, flaky, or infrastructure-failed
observations into workroom `verified`.

## Admitted AssuranceSpec dogfood

The live companion lives here as
`openagents-codex-workroom-mvp.assurance-spec.md`, beside the ProductSpec it
binds. Its revision-1 proposal remains frozen in the conformance corpus; the
live revision 2 is reviewed, admitted, and fully designed. The exact
Environment Profile, adapter lock, review, admission, deterministic Manifest,
18 candidate receipts, 18 falsifier receipts, 18 sensitivity receipts, and
full Desktop gate are committed under [`../../assurance`](../../assurance).
The reviewed Evidence Index projects every obligation across eight independent
axes without a blended score. Native JUnit and full-gate output remain private
under ignored `var/assurance` paths.

The target subject is currently ProductSpec format `0.1`, `spec_revision: 6`,
SHA-256
`fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1`, with
`CW-AC-01` through `CW-AC-18`. A changed revision or digest requires the pilot
binding and evidence to be reconciled; this README is not an authority for
silently pinning stale identity.

Revision 6 is valid under the current OpenAgents ProductSpec profile, but not
under upstream ProductSpec `0.19.0`: its criteria use Markdown `CW-AC-*` IDs
and its metrics use the existing OpenAgents shape rather than structured
`AC-*`/`SM-*` items. The first dogfood keeps this exact r6 baseline honest. A
separate reviewed revision must map those IDs and add portable Related
Artifacts before we claim item-level upstream Evidence Loop interoperability.
See
[`../assurance/PRODUCTSPEC_EVIDENCE_LOOP.md`](../assurance/PRODUCTSPEC_EVIDENCE_LOOP.md).

## Proposed PSEL-2 migration revision (revision 7)

The reviewed intent-identity migration exists as a proposal beside this
package (#8758):

- [`openagents-codex-workroom-mvp.rev7-proposed.product-spec.md`](./openagents-codex-workroom-mvp.rev7-proposed.product-spec.md)
  — revision 7, converting `CW-AC-01…18` to structured `AC-1…18` items and the
  seven metrics to `SM-1…7` (single-line whitespace-collapse normalization
  only; every other intent section is verbatim revision-6 prose). It validates
  under both the `openagents` and `upstream` ProductSpec profiles; each
  metric's `segment`/`source` provenance lives in its keyed Success Metric
  Context section, and its Decision Trace section records the migration and
  its approval state.
- [`openagents-codex-workroom-mvp.id-map.json`](./openagents-codex-workroom-mvp.id-map.json)
  — the machine-readable old→new ID map, digest-pinned to both revisions.
  `bun test packages/product-spec` enforces that the map, revision 6, and
  revision 7 agree exactly.

**Adoption is owner-gated and has not happened.** The live executable subject
of this package, the checked-in AssuranceSpec proposal, and the MVP-01
(#8756) dogfood remain bound to revision 6 (`CW-AC-*`, digest
`sha256:fba79633…`). PSEL-3 freezes the migrated document/intent identities,
rebinds the AssuranceSpec, and creates the new accepted plan/run for the
`AC-*` identity; no run, packet, or evidence is relabeled by the proposal.
Reconcile any future `CW-AC-*` reference through the ID map, never by
find-and-replace.

The pilot composes with the existing workroom rather than creating a second
packet/status ledger: after a qualifying normalized Assurance Receipt exists,
it is linked to an exact packet by reference. Portable ProductSpec Related
Artifact publication is a later step gated on the local parser catch-up and ID
reconciliation.

## Current boundary

This package does not yet update behavior/Eval registries, the public promise
registry, or launch claims. Those integrations wait for their explicit roadmap
slice and exact implementation receipts.

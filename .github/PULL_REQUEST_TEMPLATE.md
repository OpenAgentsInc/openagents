## Summary

<!-- What changed and why -->

## OA-RUST Mapping

- Issue(s): <!-- OA-RUST-### -->
- Owner lane: <!-- owner:* -->
- Phase: <!-- phase:* -->

## Invariant Gates (Required)

- [ ] `INV-01` Proto-first contract authority preserved (`proto/` updated first if cross-boundary changes).
- [ ] `INV-02` Mutations stay on HTTP APIs (no WS command RPC).
- [ ] `INV-03` No new SSE/poll live-sync lane introduced.
- [ ] `INV-04` Control/runtime authority boundary preserved (no cross-plane write/join behavior).
- [ ] `INV-05` No implicit in-memory coupling introduced across control/runtime/Khala.
- [ ] `INV-06` Khala remains projection/replay delivery only.
- [ ] `INV-07` Ordering/idempotency requirements addressed for sync changes.
- [ ] `INV-08` WorkOS auth provider + control-plane authz/session authority split preserved.
- [ ] `INV-09` Runtime deploy+migrate implications documented if runtime touched.
- [ ] `INV-10` Legacy deletion/cutover work mapped to parity gate issues.

Reference: `docs/plans/active/rust-migration-invariant-gates.md`

## Verification

<!-- Include exact commands and outcomes -->

```bash
# example
./scripts/local-ci.sh changed
```

## Release Impact

- [ ] No release impact.
- [ ] Runtime deploy required.
- [ ] Runtime migrate job required.
- [ ] Client min-version/replay compatibility impact.

## Checklist

- [ ] Docs updated for behavior/contract changes.
- [ ] Tests added/updated for changed behavior.
- [ ] Follow-up issues created for deferred work.


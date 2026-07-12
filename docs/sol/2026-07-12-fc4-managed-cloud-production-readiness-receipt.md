# FC-4 managed-cloud production readiness receipt

- Date: 2026-07-12
- Issue: [#8636](https://github.com/OpenAgentsInc/openagents/issues/8636)
- Proof rung: deployed schema + production authority/registry readiness
- Final disposition: partial; #8636 remains open
- Source commits: `336541a678`, `ae785489fd`, `af2b4af8e9`

## Outcome

The managed-cloud FleetRun capacity/claim schema is deployed to staging and
production, and the production authority tables can represent one owner-local
Codex run and one managed-cloud Codex run as distinct claims in the same intake
registry. The probe left no rows behind.

This receipt does **not** prove a managed executor, Agent Computer grant,
physical-mobile start, Desktop resume, or the concurrent hybrid journey in the
#8636 exit. Those remain gated by #8547 and owner/device acceptance.

## Migration receipt

The sanctioned direct-Postgres runner applied through the Cloud SQL Auth Proxy
with the workspace automation service account. No database URL, password,
token, raw topology, or customer data was printed or retained.

| Environment | Result |
| --- | --- |
| Staging | `0065_sarah_fleet_run_managed_cloud_capacity.sql` applied; rerun reports 0 pending / 66 applied |
| Production | `0065_sarah_fleet_run_managed_cloud_capacity.sql` applied; rerun reports 0 pending / 66 applied |

Both ledgers retain the exact `0065` SHA-256
`4b873ec20c7393545b6e15c2bd9dfb46da12f4d9839cd25e05acc62007c936bb`.
The live constraint is exactly the intended bounded widening:

```text
capacity_class = owner_local
OR (capacity_class = managed_cloud AND worker_kind = codex)
```

Production held 42 existing `owner_local` attempts and zero
`managed_cloud` attempts at verification time. That zero is honest: schema
readiness does not manufacture a live managed turn.

## Ordered-runner repair

Staging already contained AUDIO-3 retention tables created by its earlier
app-specific live path, but the shared migration ledger did not contain
`0064_audio_retention.sql`. The ordered runner therefore stopped before 0065
with duplicate-relation SQLSTATE `42P07`.

Commit `af2b4af8e9` made only the 0064 table/index creation statements
idempotent. Before accepting the ledger row, the live staging column,
primary/unique/foreign-key, disposition, sequence, expiry, and deletion-state
constraints were inspected and matched the checked-in contract. The repair
passed 14 migration/privacy tests. Staging then recorded 0064 and continued to
0065; production created 0064 normally before applying 0065.

## Production registry probe

Inside one explicit production transaction, the real authority tables accepted:

```text
managed_cloud|codex|claimed|claim.sarah_fleet_run.222222222222222222222222|pylon.public.hybrid_readiness_8636
owner_local|codex|claimed|claim.sarah_fleet_run.111111111111111111111111|pylon.public.hybrid_readiness_8636
```

The two target classes had distinct run, unit, idempotency, and claim refs but
shared the same owner and Pylon registry. The transaction was rolled back. The
post-rollback residue check was:

```text
requests=0 | claims=0 | pylons=0
```

This proves deployed representation and claim-registry readiness. The
repository's focused authority suite separately proves authenticated claim
selection, managed-Codex-only capacity derivation, duplicate-claim exclusion,
and public-safe projection against ephemeral Postgres. Neither proof replaces
the live executor/client exit.

## Remaining exit

1. #8547 must expose and accept the broker-authorized real Agent Computer path.
2. One real run must concurrently complete an owner-local unit and a managed
   Agent Computer unit under the same owner/run/claim truth.
3. Mobile and Desktop must show and resume the same target, fallback, claim,
   outcome, usage, compute, writeback, stop, and reclaim refs.
4. The journey must prove zero duplicate claims and zero silent target/provider
   substitution.

No phone, mirroring, emulator, renderer, mobile UI, Desktop UI, Worker/control
deployment, or grant-redemption action was performed for this receipt.

# Hydra MVP-3 Vignette Harness

Status: active  
Last updated: 2026-02-25

Single-command harness:

```bash
./scripts/vignette-hydra-mvp3.sh
```

This command runs the MVP-3 FX regression checks that gate Hydra RFQ/selection/settlement determinism:

1. RFQ authority path accepts valid bounded policy input and preserves idempotent replay/readback semantics.
2. Deterministic quote selection is stable across fixed candidate sets.
3. Settlement path enforces reservation constraints and emits deterministic provenance receipt payloads.
4. Replay/idempotency on settlement does not double-settle.
5. FX observability metrics are published and updated (`/internal/v1/hydra/observability` `fx` block).
6. Runtime OpenAPI surface includes Hydra FX authority contracts.

Induced failure assertions included in harness:

- Quote expiry path settles to withheld.
- Idempotency drift path conflicts deterministically.

The harness fails on first regression so it can be used in local CI and release validation.

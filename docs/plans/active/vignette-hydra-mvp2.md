# Hydra MVP-2 Vignette Harness

Status: active  
Last updated: 2026-02-25

Single-command harness:

```bash
./scripts/vignette-hydra-mvp2.sh
```

This command runs the MVP-2 regression checks that gate Hydra routing/risk observability:

1. Routing score path selects the expected provider and emits deterministic linkage.
2. CEP breaker activation filters CEP candidates and forces fallback/degraded routing behavior.
3. LLP withdrawal throttle posture is surfaced and counted in Hydra observability metrics.
4. Runtime internal OpenAPI contract publishes Hydra + credit endpoints and schemas.

The harness fails on first regression so it can be used in local CI and release validation.

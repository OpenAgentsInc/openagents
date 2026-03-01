# Kernel Strategy Spike (2026-03-01)

Related issue: [#2453](https://github.com/OpenAgentsInc/openagents/issues/2453)  
Decision record: `crates/cad/docs/decisions/0001-kernel-strategy.md`

## Purpose

Collect quick evidence for Wave 1 kernel strategy choice and fallback readiness.

## Scope

- Compare options A-D using weighted criteria.
- Run practical viability checks available in current environment.
- Capture required artifact categories:
  - boolean failure log
  - STEP outputs
  - checker/import results
  - peak memory signals

## Corpus Definition (Wave 1 Rack-Oriented)

- `RACK-01`: base two-bay solid with wall thickness parameter
- `RACK-02`: wall mount hole pattern (linear)
- `RACK-03`: vent hole pattern (dense)
- `RACK-04`: rib/thickness tradeoff edit
- `RACK-05`: variant objective generation sanity pass

This short spike captured environment viability artifacts first; full corpus execution is deferred until adapter work and engine wiring are in place.

## Artifact Index

- `artifacts/vcad_kernel_check.log`
- `artifacts/vcad_cli_help.log`
- `artifacts/opencascade_check.log`
- `boolean_failure_log.md`
- `step_outputs.md`
- `checker_results.md`
- `memory_results.md`

## Reproduction Commands

From `vcad` repo:

```bash
/usr/bin/time -p cargo check -p vcad-kernel --quiet
cargo run -q -p vcad-cli -- --help
```

From temp OpenCascade spike project:

```bash
/usr/bin/time -p cargo check --quiet --manifest-path /tmp/opencascade_spike/Cargo.toml
```

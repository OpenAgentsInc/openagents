# Parity Check Orchestration

Issue coverage: `VCAD-PARITY-007`

## Purpose

Provide one deterministic orchestration entrypoint for the parity baseline lanes:

- baseline manifest freeze check
- inventory/matrix/scorecard/fixture-corpus pipeline checks
- parity CI artifact manifest fixture check
- parity risk register + blocker workflow check
- baseline dashboard publication check
- parity fixture test lane
- formatting check

## Command

Run full parity orchestration:

```bash
scripts/cad/parity_check.sh
```

List lane IDs only:

```bash
scripts/cad/parity_check.sh --list
```

Run pipeline checks but skip cargo parity test lane:

```bash
scripts/cad/parity_check.sh --skip-tests
```

## Determinism Contract

`parity_check.sh` runs all lanes from repo root and fails fast on first lane error.
Each lane captures command output to a temp log and prints the log on failure for
local reproducibility.

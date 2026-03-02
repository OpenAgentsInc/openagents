# Parity Check Orchestration

Issue coverage: `VCAD-PARITY-007`

## Purpose

Provide one deterministic orchestration entrypoint for the parity baseline lanes:

- baseline manifest freeze check
- inventory/matrix/scorecard/fixture-corpus pipeline checks
- kernel adapter v2 parity fixture check
- kernel math parity fixture check
- kernel topology parity fixture check
- kernel geom parity fixture check
- kernel primitives parity fixture check
- kernel tessellate parity fixture check
- kernel booleans parity fixture check
- kernel boolean diagnostics parity fixture check
- kernel boolean BRep parity fixture check
- kernel NURBS parity fixture check
- kernel text parity fixture check
- kernel fillet parity fixture check
- kernel shell parity fixture check
- kernel step parity fixture check
- kernel precision parity fixture check
- primitive contracts parity fixture check
- transform parity fixture check
- pattern parity fixture check
- shell feature-graph parity fixture check
- fillet feature-graph parity fixture check
- chamfer feature-graph parity fixture check
- expanded finishing parity fixture check
- sweep parity fixture check
- loft parity fixture check
- topology repair parity fixture check
- material assignment parity fixture check
- vcad-eval receipts parity fixture check
- feature-op hash parity fixture check
- modeling edge-case parity fixture check
- core modeling checkpoint parity fixture check
- sketch entity set parity fixture check
- sketch plane parity fixture check
- sketch constraint enum parity fixture check
- sketch iterative LM parity fixture check
- sketch Jacobian/residual parity fixture check
- sketch constraint-status parity fixture check
- sketch extrude parity fixture check
- sketch revolve parity fixture check
- sketch sweep parity fixture check
- sketch loft parity fixture check
- assembly checkpoint parity fixture check
- drafting kernel scaffolding parity fixture check
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

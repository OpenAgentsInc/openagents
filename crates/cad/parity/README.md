# CAD Parity Artifacts

This folder stores machine-readable artifacts used by the VCAD parity program.

Current artifacts:

- `vcad_reference_manifest.json` (`VCAD-PARITY-001`)
- `openagents_start_manifest.json` (`VCAD-PARITY-001`)
- `vcad_capabilities_inventory.json` (`VCAD-PARITY-002`)
- `openagents_capabilities_inventory.json` (`VCAD-PARITY-003`)
- `vcad_openagents_gap_matrix.json` (`VCAD-PARITY-004`)
- `parity_scorecard.json` (`VCAD-PARITY-005`)

Regeneration/check command:

```bash
scripts/cad/freeze-parity-baseline.sh
scripts/cad/freeze-parity-baseline.sh --check
scripts/cad/vcad-capability-crawler-ci.sh
scripts/cad/openagents-capability-crawler-ci.sh
scripts/cad/parity-gap-matrix-ci.sh
scripts/cad/parity-scorecard-ci.sh
```

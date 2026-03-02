# Parity CI Lane And Artifact Upload

Issue coverage: `VCAD-PARITY-008`

## Purpose

Provide a CI-focused parity lane that:

- runs `scripts/cad/parity_check.sh` as the single parity gate
- verifies deterministic CI artifact manifest fixture integrity
- emits upload-ready parity evidence artifacts (bundle + sha256 + env metadata)

## Commands

Check-only mode (used by CI gate jobs before upload):

```bash
scripts/cad/parity-ci-lane.sh --check
```

Generate upload artifacts:

```bash
scripts/cad/parity-ci-lane.sh
```

Override output directory:

```bash
scripts/cad/parity-ci-lane.sh --artifacts-dir target/custom-parity-ci
```

List lane step IDs:

```bash
scripts/cad/parity-ci-lane.sh --list
```

## Output Contract

Default output directory: `target/parity-ci`

Generated outputs:

- `payload/` copied parity source artifacts and generated `parity_ci_artifact_manifest.json`
- payload includes baseline manifests, inventories, gap matrix, scorecard, fixture corpus, risk register, dashboard snapshot, kernel adapter v2 manifest, kernel math parity manifest, kernel topology parity manifest, kernel geom parity manifest, kernel primitives parity manifest, kernel tessellate parity manifest, kernel precision parity manifest, kernel booleans parity manifest, kernel boolean diagnostics parity manifest, kernel boolean brep parity manifest, kernel nurbs parity manifest, kernel text parity manifest, kernel fillet parity manifest, kernel shell parity manifest, kernel step parity manifest, primitive contracts parity manifest, transform parity manifest, pattern parity manifest, shell feature-graph parity manifest, fillet feature-graph parity manifest, chamfer feature-graph parity manifest, expanded finishing parity manifest, sweep parity manifest, loft parity manifest, topology repair parity manifest, material assignment parity manifest, vcad-eval receipts parity manifest, feature-op hash parity manifest, modeling edge-case parity manifest, core modeling checkpoint parity manifest, sketch entity set parity manifest, sketch plane parity manifest, sketch constraint enum parity manifest, sketch iterative LM parity manifest, sketch Jacobian/residual parity manifest, sketch constraint-status parity manifest, sketch extrude parity manifest, sketch revolve parity manifest, sketch sweep parity manifest, sketch loft parity manifest, and the feature-op hash reference corpus fixture
- payload includes baseline manifests, inventories, gap matrix, scorecard, fixture corpus, risk register, dashboard snapshot, kernel adapter v2 manifest, kernel math parity manifest, kernel topology parity manifest, kernel geom parity manifest, kernel primitives parity manifest, kernel tessellate parity manifest, kernel precision parity manifest, kernel booleans parity manifest, kernel boolean diagnostics parity manifest, kernel boolean brep parity manifest, kernel nurbs parity manifest, kernel text parity manifest, kernel fillet parity manifest, kernel shell parity manifest, kernel step parity manifest, primitive contracts parity manifest, transform parity manifest, pattern parity manifest, shell feature-graph parity manifest, fillet feature-graph parity manifest, chamfer feature-graph parity manifest, expanded finishing parity manifest, sweep parity manifest, loft parity manifest, topology repair parity manifest, material assignment parity manifest, vcad-eval receipts parity manifest, feature-op hash parity manifest, modeling edge-case parity manifest, core modeling checkpoint parity manifest, sketch entity set parity manifest, sketch plane parity manifest, sketch constraint enum parity manifest, sketch iterative LM parity manifest, sketch Jacobian/residual parity manifest, sketch constraint-status parity manifest, sketch extrude parity manifest, sketch revolve parity manifest, sketch sweep parity manifest, sketch loft parity manifest, assembly checkpoint parity manifest, drafting kernel scaffolding parity manifest, drafting projection parity manifest, drafting hidden-line parity manifest, drafting kernel/projection/hidden-line vcad reference fixtures, and the feature-op hash reference corpus fixture
- `parity_ci_artifacts.tar.gz` bundle for CI upload
- `parity_ci_artifacts.sha256` deterministic bundle checksum
- `parity_ci_upload.env` env-style metadata (paths + checksum) for CI upload step wiring

## Determinism Contract

- `scripts/cad/parity-ci-artifacts-ci.sh` enforces fixture lock with:
  `cargo run -p openagents-cad --bin parity-ci-artifacts -- --check`
- `parity_ci_artifact_manifest.json` tracks sha256 + byte lengths for all parity baseline artifacts.
- `crates/cad/tests/parity_ci_artifacts.rs` guards manifest schema and regeneration equivalence.

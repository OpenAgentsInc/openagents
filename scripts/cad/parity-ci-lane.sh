#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

LANE_LABELS=(
    "parity-check"
    "ci-artifact-manifest-check"
    "artifact-copy"
    "artifact-bundle"
    "artifact-checksum"
)

ARTIFACT_SOURCE_PATHS=(
    "crates/cad/parity/vcad_reference_manifest.json"
    "crates/cad/parity/openagents_start_manifest.json"
    "crates/cad/parity/vcad_capabilities_inventory.json"
    "crates/cad/parity/openagents_capabilities_inventory.json"
    "crates/cad/parity/vcad_openagents_gap_matrix.json"
    "crates/cad/parity/parity_scorecard.json"
    "crates/cad/parity/parity_risk_register.json"
    "crates/cad/parity/parity_dashboard.json"
    "crates/cad/parity/kernel_adapter_v2_manifest.json"
    "crates/cad/parity/kernel_math_parity_manifest.json"
    "crates/cad/parity/kernel_topology_parity_manifest.json"
    "crates/cad/parity/kernel_geom_parity_manifest.json"
    "crates/cad/parity/kernel_primitives_parity_manifest.json"
    "crates/cad/parity/kernel_tessellate_parity_manifest.json"
    "crates/cad/parity/kernel_booleans_parity_manifest.json"
    "crates/cad/parity/kernel_boolean_diagnostics_parity_manifest.json"
    "crates/cad/parity/kernel_boolean_brep_parity_manifest.json"
    "crates/cad/parity/kernel_nurbs_parity_manifest.json"
    "crates/cad/parity/kernel_text_parity_manifest.json"
    "crates/cad/parity/kernel_fillet_parity_manifest.json"
    "crates/cad/parity/kernel_shell_parity_manifest.json"
    "crates/cad/parity/kernel_step_parity_manifest.json"
    "crates/cad/parity/kernel_precision_parity_manifest.json"
    "crates/cad/parity/primitive_contracts_parity_manifest.json"
    "crates/cad/parity/transform_parity_manifest.json"
    "crates/cad/parity/pattern_parity_manifest.json"
    "crates/cad/parity/shell_feature_graph_parity_manifest.json"
    "crates/cad/parity/fillet_feature_graph_parity_manifest.json"
    "crates/cad/parity/chamfer_feature_graph_parity_manifest.json"
    "crates/cad/parity/expanded_finishing_parity_manifest.json"
    "crates/cad/parity/sweep_parity_manifest.json"
    "crates/cad/parity/loft_parity_manifest.json"
    "crates/cad/parity/topology_repair_parity_manifest.json"
    "crates/cad/parity/material_assignment_parity_manifest.json"
    "crates/cad/parity/vcad_eval_receipts_parity_manifest.json"
    "crates/cad/parity/feature_op_hash_parity_manifest.json"
    "crates/cad/parity/modeling_edge_case_parity_manifest.json"
    "crates/cad/parity/core_modeling_checkpoint_parity_manifest.json"
    "crates/cad/parity/fixtures/feature_op_hash_vcad_reference_corpus.json"
    "crates/cad/parity/fixtures/parity_fixture_corpus.json"
)

usage() {
    cat <<USAGE
Usage:
  scripts/cad/parity-ci-lane.sh
  scripts/cad/parity-ci-lane.sh --check
  scripts/cad/parity-ci-lane.sh --list
  scripts/cad/parity-ci-lane.sh --artifacts-dir <path>
  scripts/cad/parity-ci-lane.sh --skip-tests

Options:
  --check                Run parity checks + fixture checks only (no bundle output)
  --list                 Print CI lane step IDs and exit
  --artifacts-dir <path> Output directory for CI payload and bundle
  --skip-tests           Pass --skip-tests to scripts/cad/parity_check.sh
USAGE
}

ARTIFACTS_DIR="$ROOT_DIR/target/parity-ci"
CHECK_ONLY=0
LIST_ONLY=0
SKIP_TESTS=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_ONLY=1
            shift
            ;;
        --list)
            LIST_ONLY=1
            shift
            ;;
        --artifacts-dir)
            if [[ $# -lt 2 ]]; then
                printf 'missing value for --artifacts-dir\n\n' >&2
                usage >&2
                exit 2
            fi
            ARTIFACTS_DIR="$2"
            shift 2
            ;;
        --skip-tests)
            SKIP_TESTS=1
            shift
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if (( LIST_ONLY == 1 )); then
    for lane in "${LANE_LABELS[@]}"; do
        printf '%s\n' "$lane"
    done
    exit 0
fi

run_parity_check() {
    if (( SKIP_TESTS == 1 )); then
        "$ROOT_DIR/scripts/cad/parity_check.sh" --skip-tests
        return
    fi
    "$ROOT_DIR/scripts/cad/parity_check.sh"
}

run_parity_check
"$ROOT_DIR/scripts/cad/parity-ci-artifacts-ci.sh"

if (( CHECK_ONLY == 1 )); then
    printf 'CAD parity CI lane checks passed.\n'
    exit 0
fi

mkdir -p "$ARTIFACTS_DIR"
PAYLOAD_DIR="$ARTIFACTS_DIR/payload"
rm -rf "$PAYLOAD_DIR"
mkdir -p "$PAYLOAD_DIR"

for rel in "${ARTIFACT_SOURCE_PATHS[@]}"; do
    src="$ROOT_DIR/$rel"
    dst="$PAYLOAD_DIR/$(echo "$rel" | sed 's#^crates/cad/parity/##' | tr '/\\' '__')"
    cp "$src" "$dst"
done

cargo run -p openagents-cad --bin parity-ci-artifacts -- \
    --output "$PAYLOAD_DIR/parity_ci_artifact_manifest.json"

BUNDLE_PATH="$ARTIFACTS_DIR/parity_ci_artifacts.tar.gz"
rm -f "$BUNDLE_PATH"
tar -czf "$BUNDLE_PATH" -C "$PAYLOAD_DIR" .

BUNDLE_SHA256="$(sha256sum "$BUNDLE_PATH" | awk '{print $1}')"
SHA_FILE="$ARTIFACTS_DIR/parity_ci_artifacts.sha256"
printf '%s  %s\n' "$BUNDLE_SHA256" "$(basename "$BUNDLE_PATH")" >"$SHA_FILE"

UPLOAD_ENV="$ARTIFACTS_DIR/parity_ci_upload.env"
cat >"$UPLOAD_ENV" <<ENV
PARITY_CI_ARTIFACT_DIR=$ARTIFACTS_DIR
PARITY_CI_PAYLOAD_DIR=$PAYLOAD_DIR
PARITY_CI_MANIFEST_PATH=$PAYLOAD_DIR/parity_ci_artifact_manifest.json
PARITY_CI_BUNDLE_PATH=$BUNDLE_PATH
PARITY_CI_BUNDLE_SHA256=$BUNDLE_SHA256
PARITY_CI_BUNDLE_SHA_FILE=$SHA_FILE
ENV

printf 'CAD parity CI artifacts generated.\n'
printf 'bundle: %s\n' "$BUNDLE_PATH"
printf 'sha256: %s\n' "$BUNDLE_SHA256"
printf 'upload metadata: %s\n' "$UPLOAD_ENV"

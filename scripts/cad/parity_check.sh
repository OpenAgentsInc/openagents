#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

LANE_LABELS=(
    "baseline-manifests"
    "fixture-corpus-pipeline"
    "kernel-adapter-v2"
    "kernel-math"
    "kernel-topology"
    "kernel-geom"
    "kernel-primitives"
    "kernel-tessellate"
    "kernel-booleans"
    "kernel-boolean-diagnostics"
    "kernel-boolean-brep"
    "kernel-nurbs"
    "kernel-text"
    "kernel-fillet"
    "kernel-shell"
    "kernel-step"
    "kernel-precision"
    "primitive-contracts"
    "transform"
    "pattern"
    "shell-feature-graph"
    "fillet-feature-graph"
    "chamfer-feature-graph"
    "expanded-finishing"
    "sweep"
    "loft"
    "topology-repair"
    "material-assignment"
    "vcad-eval-receipts"
    "feature-op-hash"
    "modeling-edge-cases"
    "core-modeling-checkpoint"
    "sketch-entity-set"
    "sketch-plane"
    "sketch-constraint-enum"
    "sketch-iterative-lm"
    "sketch-jacobian-residual"
    "sketch-constraint-status"
    "sketch-extrude"
    "sketch-revolve"
    "sketch-sweep"
    "sketch-loft"
    "sketch-profile-validity"
    "sketch-interaction"
    "sketch-undo-redo"
    "sketch-fixture-equivalence"
    "sketch-constraints-checkpoint"
    "assembly-schema"
    "assembly-part-instance"
    "assembly-joint-frs"
    "assembly-joint-cb"
    "assembly-joint-limits-state"
    "assembly-fk"
    "assembly-ground-delete"
    "assembly-ui-selection-edit"
    "assembly-serialization-replay"
    "assembly-acceptance-scenes"
    "assembly-checkpoint"
    "drafting-kernel-scaffolding"
    "drafting-projection"
    "drafting-hidden-line"
    "drafting-dimension"
    "drafting-gdt"
    "drafting-section"
    "drafting-detail"
    "drafting-drawing-mode-ui"
    "drafting-persistence"
    "drafting-dxf-export"
    "drafting-pdf-export"
    "drafting-checkpoint"
    "step-import-entity"
    "step-export-post-boolean"
    "stl-import-export"
    "glb-export"
    "cad-cli-scaffold"
    "ci-artifact-manifest"
    "risk-register-workflow"
    "baseline-dashboard"
    "parity-fixture-tests"
    "rustfmt-check"
)

usage() {
    cat <<USAGE
Usage:
  scripts/cad/parity_check.sh
  scripts/cad/parity_check.sh --list
  scripts/cad/parity_check.sh --skip-tests

Options:
  --list        Print orchestration lane IDs and exit
  --skip-tests  Run artifact/check scripts but skip cargo parity test lane
USAGE
}

LIST_ONLY=0
SKIP_TESTS=0
for arg in "$@"; do
    case "$arg" in
        --list)
            LIST_ONLY=1
            ;;
        --skip-tests)
            SKIP_TESTS=1
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n\n' "$arg" >&2
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

run_lane() {
    local lane="$1"
    shift

    local tmp
    tmp="$(mktemp)"
    printf 'Parity lane: %s\n' "$lane" >&2
    if ! (cd "$ROOT_DIR" && "$@" >"$tmp" 2>&1); then
        cat "$tmp" >&2
        rm -f "$tmp"
        printf 'Parity lane failed: %s\n' "$lane" >&2
        exit 1
    fi
    rm -f "$tmp"
    printf 'Parity lane pass: %s\n' "$lane" >&2
}

run_lane "baseline-manifests" \
    "$ROOT_DIR/scripts/cad/freeze-parity-baseline.sh" --check

run_lane "fixture-corpus-pipeline" \
    "$ROOT_DIR/scripts/cad/parity-fixture-corpus-ci.sh"

run_lane "kernel-adapter-v2" \
    "$ROOT_DIR/scripts/cad/parity-kernel-adapter-v2-ci.sh"

run_lane "kernel-math" \
    "$ROOT_DIR/scripts/cad/parity-kernel-math-ci.sh"

run_lane "kernel-topology" \
    "$ROOT_DIR/scripts/cad/parity-kernel-topology-ci.sh"

run_lane "kernel-geom" \
    "$ROOT_DIR/scripts/cad/parity-kernel-geom-ci.sh"

run_lane "kernel-primitives" \
    "$ROOT_DIR/scripts/cad/parity-kernel-primitives-ci.sh"

run_lane "kernel-tessellate" \
    "$ROOT_DIR/scripts/cad/parity-kernel-tessellate-ci.sh"

run_lane "kernel-booleans" \
    "$ROOT_DIR/scripts/cad/parity-kernel-booleans-ci.sh"

run_lane "kernel-boolean-diagnostics" \
    "$ROOT_DIR/scripts/cad/parity-kernel-boolean-diagnostics-ci.sh"

run_lane "kernel-boolean-brep" \
    "$ROOT_DIR/scripts/cad/parity-kernel-boolean-brep-ci.sh"

run_lane "kernel-nurbs" \
    "$ROOT_DIR/scripts/cad/parity-kernel-nurbs-ci.sh"

run_lane "kernel-text" \
    "$ROOT_DIR/scripts/cad/parity-kernel-text-ci.sh"

run_lane "kernel-fillet" \
    "$ROOT_DIR/scripts/cad/parity-kernel-fillet-ci.sh"

run_lane "kernel-shell" \
    "$ROOT_DIR/scripts/cad/parity-kernel-shell-ci.sh"

run_lane "kernel-step" \
    "$ROOT_DIR/scripts/cad/parity-kernel-step-ci.sh"

run_lane "kernel-precision" \
    "$ROOT_DIR/scripts/cad/parity-kernel-precision-ci.sh"

run_lane "primitive-contracts" \
    "$ROOT_DIR/scripts/cad/parity-primitive-contracts-ci.sh"

run_lane "transform" \
    "$ROOT_DIR/scripts/cad/parity-transform-ci.sh"

run_lane "pattern" \
    "$ROOT_DIR/scripts/cad/parity-pattern-ci.sh"

run_lane "shell-feature-graph" \
    "$ROOT_DIR/scripts/cad/parity-shell-feature-graph-ci.sh"

run_lane "fillet-feature-graph" \
    "$ROOT_DIR/scripts/cad/parity-fillet-feature-graph-ci.sh"

run_lane "chamfer-feature-graph" \
    "$ROOT_DIR/scripts/cad/parity-chamfer-feature-graph-ci.sh"

run_lane "expanded-finishing" \
    "$ROOT_DIR/scripts/cad/parity-expanded-finishing-ci.sh"

run_lane "sweep" \
    "$ROOT_DIR/scripts/cad/parity-sweep-ci.sh"

run_lane "loft" \
    "$ROOT_DIR/scripts/cad/parity-loft-ci.sh"

run_lane "topology-repair" \
    "$ROOT_DIR/scripts/cad/parity-topology-repair-ci.sh"

run_lane "material-assignment" \
    "$ROOT_DIR/scripts/cad/parity-material-assignment-ci.sh"

run_lane "vcad-eval-receipts" \
    "$ROOT_DIR/scripts/cad/parity-vcad-eval-receipts-ci.sh"

run_lane "feature-op-hash" \
    "$ROOT_DIR/scripts/cad/parity-feature-op-hash-ci.sh"

run_lane "modeling-edge-cases" \
    "$ROOT_DIR/scripts/cad/parity-modeling-edge-cases-ci.sh"

run_lane "core-modeling-checkpoint" \
    "$ROOT_DIR/scripts/cad/parity-core-modeling-checkpoint-ci.sh"

run_lane "sketch-entity-set" \
    "$ROOT_DIR/scripts/cad/parity-sketch-entity-set-ci.sh"

run_lane "sketch-plane" \
    "$ROOT_DIR/scripts/cad/parity-sketch-plane-ci.sh"

run_lane "sketch-constraint-enum" \
    "$ROOT_DIR/scripts/cad/parity-sketch-constraint-enum-ci.sh"

run_lane "sketch-iterative-lm" \
    "$ROOT_DIR/scripts/cad/parity-sketch-iterative-lm-ci.sh"

run_lane "sketch-jacobian-residual" \
    "$ROOT_DIR/scripts/cad/parity-sketch-jacobian-residual-ci.sh"

run_lane "sketch-constraint-status" \
    "$ROOT_DIR/scripts/cad/parity-sketch-constraint-status-ci.sh"

run_lane "sketch-extrude" \
    "$ROOT_DIR/scripts/cad/parity-sketch-extrude-ci.sh"

run_lane "sketch-revolve" \
    "$ROOT_DIR/scripts/cad/parity-sketch-revolve-ci.sh"

run_lane "sketch-sweep" \
    "$ROOT_DIR/scripts/cad/parity-sketch-sweep-ci.sh"

run_lane "sketch-loft" \
    "$ROOT_DIR/scripts/cad/parity-sketch-loft-ci.sh"

run_lane "sketch-profile-validity" \
    "$ROOT_DIR/scripts/cad/parity-sketch-profile-validity-ci.sh"

run_lane "sketch-interaction" \
    "$ROOT_DIR/scripts/cad/parity-sketch-interaction-ci.sh"

run_lane "sketch-undo-redo" \
    "$ROOT_DIR/scripts/cad/parity-sketch-undo-redo-ci.sh"

run_lane "sketch-fixture-equivalence" \
    "$ROOT_DIR/scripts/cad/parity-sketch-fixture-equivalence-ci.sh"

run_lane "sketch-constraints-checkpoint" \
    "$ROOT_DIR/scripts/cad/parity-sketch-constraints-checkpoint-ci.sh"

run_lane "assembly-schema" \
    "$ROOT_DIR/scripts/cad/parity-assembly-schema-ci.sh"

run_lane "assembly-part-instance" \
    "$ROOT_DIR/scripts/cad/parity-assembly-part-instance-ci.sh"

run_lane "assembly-joint-frs" \
    "$ROOT_DIR/scripts/cad/parity-assembly-joint-frs-ci.sh"

run_lane "assembly-joint-cb" \
    "$ROOT_DIR/scripts/cad/parity-assembly-joint-cb-ci.sh"

run_lane "assembly-joint-limits-state" \
    "$ROOT_DIR/scripts/cad/parity-assembly-joint-limits-state-ci.sh"

run_lane "assembly-fk" \
    "$ROOT_DIR/scripts/cad/parity-assembly-fk-ci.sh"

run_lane "assembly-ground-delete" \
    "$ROOT_DIR/scripts/cad/parity-assembly-ground-delete-ci.sh"

run_lane "assembly-ui-selection-edit" \
    "$ROOT_DIR/scripts/cad/parity-assembly-ui-selection-edit-ci.sh"

run_lane "assembly-serialization-replay" \
    "$ROOT_DIR/scripts/cad/parity-assembly-serialization-replay-ci.sh"

run_lane "assembly-acceptance-scenes" \
    "$ROOT_DIR/scripts/cad/parity-assembly-acceptance-scenes-ci.sh"

run_lane "assembly-checkpoint" \
    "$ROOT_DIR/scripts/cad/parity-assembly-checkpoint-ci.sh"

run_lane "drafting-kernel-scaffolding" \
    "$ROOT_DIR/scripts/cad/parity-drafting-kernel-scaffolding-ci.sh"

run_lane "drafting-projection" \
    "$ROOT_DIR/scripts/cad/parity-drafting-projection-ci.sh"

run_lane "drafting-hidden-line" \
    "$ROOT_DIR/scripts/cad/parity-drafting-hidden-line-ci.sh"

run_lane "drafting-dimension" \
    "$ROOT_DIR/scripts/cad/parity-drafting-dimension-ci.sh"

run_lane "drafting-gdt" \
    "$ROOT_DIR/scripts/cad/parity-drafting-gdt-ci.sh"

run_lane "drafting-section" \
    "$ROOT_DIR/scripts/cad/parity-drafting-section-ci.sh"

run_lane "drafting-detail" \
    "$ROOT_DIR/scripts/cad/parity-drafting-detail-ci.sh"

run_lane "drafting-drawing-mode-ui" \
    "$ROOT_DIR/scripts/cad/parity-drafting-drawing-mode-ui-ci.sh"

run_lane "drafting-persistence" \
    "$ROOT_DIR/scripts/cad/parity-drafting-persistence-ci.sh"

run_lane "drafting-dxf-export" \
    "$ROOT_DIR/scripts/cad/parity-drafting-dxf-export-ci.sh"

run_lane "drafting-pdf-export" \
    "$ROOT_DIR/scripts/cad/parity-drafting-pdf-export-ci.sh"

run_lane "drafting-checkpoint" \
    "$ROOT_DIR/scripts/cad/parity-drafting-checkpoint-ci.sh"

run_lane "step-import-entity" \
    "$ROOT_DIR/scripts/cad/parity-step-import-entity-ci.sh"

run_lane "step-export-post-boolean" \
    "$ROOT_DIR/scripts/cad/parity-step-export-post-boolean-ci.sh"

run_lane "stl-import-export" \
    "$ROOT_DIR/scripts/cad/parity-stl-import-export-ci.sh"

run_lane "glb-export" \
    "$ROOT_DIR/scripts/cad/parity-glb-export-ci.sh"

run_lane "cad-cli-scaffold" \
    "$ROOT_DIR/scripts/cad/parity-cad-cli-scaffold-ci.sh"

run_lane "ci-artifact-manifest" \
    "$ROOT_DIR/scripts/cad/parity-ci-artifacts-ci.sh"

run_lane "risk-register-workflow" \
    "$ROOT_DIR/scripts/cad/parity-risk-register-ci.sh"

run_lane "baseline-dashboard" \
    "$ROOT_DIR/scripts/cad/parity-dashboard-ci.sh"

if (( SKIP_TESTS == 0 )); then
    run_lane "parity-fixture-tests" \
        cargo test -p openagents-cad parity_ --quiet
fi

run_lane "rustfmt-check" \
    cargo fmt --all -- --check

printf 'CAD parity orchestration checks passed.\n'

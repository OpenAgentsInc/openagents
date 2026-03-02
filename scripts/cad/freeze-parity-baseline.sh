#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PARITY_DIR="$ROOT_DIR/crates/cad/parity"
VCAD_REPO="${VCAD_REPO:-$HOME/code/vcad}"

VCAD_COMMIT="1b59e7948efcdb848d8dba6848785d57aa310e81"
OPENAGENTS_COMMIT="04faa5227f077c419f1c5c52ddebbb7552838fd4"
FROZEN_ON="2026-03-02"

usage() {
    cat <<USAGE
Usage:
  scripts/cad/freeze-parity-baseline.sh
  scripts/cad/freeze-parity-baseline.sh --check

Environment:
  VCAD_REPO   Path to local vcad repo (default: \$HOME/code/vcad)
USAGE
}

MODE="write"
if [[ "${1:-}" == "--check" ]]; then
    MODE="check"
elif [[ $# -gt 0 ]]; then
    usage >&2
    exit 2
fi

vcad_sources=(
    "README.md"
    "docs/features/index.md"
    "docs/features/ROADMAP.md"
    "docs/features/sketch-mode.md"
    "docs/features/sketch-operations.md"
    "docs/features/boolean-operations.md"
    "docs/features/import-export.md"
    "docs/features/assembly-joints.md"
    "docs/features/drafting-2d.md"
    "docs/features/headless-api.md"
    "docs/features/ray-tracing.md"
    "docs/features/physics-simulation.md"
)

openagents_sources=(
    "crates/cad/docs/PLAN.md"
    "crates/cad/docs/decisions/0001-kernel-strategy.md"
    "crates/cad/docs/CAD_FEATURE_OPS.md"
    "crates/cad/docs/CAD_SKETCH_CONSTRAINTS.md"
    "crates/cad/docs/CAD_SKETCH_FEATURE_OPS.md"
    "crates/cad/docs/CAD_STEP_IMPORT.md"
    "crates/cad/docs/CAD_STEP_EXPORT.md"
)

hash_stream() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 | awk '{print $1}'
    else
        printf 'Missing sha256 tool (need sha256sum or shasum)\n' >&2
        exit 1
    fi
}

blob_bytes() {
    local repo="$1"
    local commit="$2"
    local path="$3"
    git -C "$repo" show "${commit}:${path}" | wc -c | tr -d '[:space:]'
}

blob_sha() {
    local repo="$1"
    local commit="$2"
    local path="$3"
    git -C "$repo" show "${commit}:${path}" | hash_stream
}

ensure_blob() {
    local repo="$1"
    local commit="$2"
    local path="$3"
    if ! git -C "$repo" cat-file -e "${commit}:${path}" 2>/dev/null; then
        printf 'Missing blob in %s at %s:%s\n' "$repo" "$commit" "$path" >&2
        exit 1
    fi
}

write_manifest() {
    local output="$1"
    local baseline_kind="$2"
    local repository="$3"
    local reference_hint="$4"
    local repo_path="$5"
    local commit="$6"
    local source_var="$7"

    local -n source_paths="$source_var"

    {
        printf '{\n'
        printf '  "manifest_version": 1,\n'
        printf '  "issue_id": "VCAD-PARITY-001",\n'
        printf '  "parity_plan_path": "crates/cad/docs/VCAD_PARITY_PLAN.md",\n'
        printf '  "generator": "scripts/cad/freeze-parity-baseline.sh",\n'
        printf '  "frozen_on": "%s",\n' "$FROZEN_ON"
        printf '  "baseline_kind": "%s",\n' "$baseline_kind"
        printf '  "repository": "%s",\n' "$repository"
        printf '  "repository_commit": "%s",\n' "$commit"
        printf '  "reference_repo_hint": "%s",\n' "$reference_hint"
        printf '  "source_documents": [\n'
    } >"$output"

    local i path sha bytes comma
    for i in "${!source_paths[@]}"; do
        path="${source_paths[$i]}"
        ensure_blob "$repo_path" "$commit" "$path"
        sha="$(blob_sha "$repo_path" "$commit" "$path")"
        bytes="$(blob_bytes "$repo_path" "$commit" "$path")"
        comma=","
        if (( i == ${#source_paths[@]} - 1 )); then
            comma=""
        fi
        printf '    {"path":"%s","sha256":"%s","bytes":%s}%s\n' \
            "$path" "$sha" "$bytes" "$comma" >>"$output"
    done

    {
        printf '  ]\n'
        printf '}\n'
    } >>"$output"
}

if ! git -C "$VCAD_REPO" cat-file -e "$VCAD_COMMIT^{commit}" 2>/dev/null; then
    printf 'Missing vcad commit %s in repo %s\n' "$VCAD_COMMIT" "$VCAD_REPO" >&2
    exit 1
fi

if ! git -C "$ROOT_DIR" cat-file -e "$OPENAGENTS_COMMIT^{commit}" 2>/dev/null; then
    printf 'Missing openagents commit %s in repo %s\n' "$OPENAGENTS_COMMIT" "$ROOT_DIR" >&2
    exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

tmp_vcad="$tmp_dir/vcad_reference_manifest.json"
tmp_openagents="$tmp_dir/openagents_start_manifest.json"

write_manifest \
    "$tmp_vcad" \
    "vcad_reference" \
    "vcad" \
    "~/code/vcad" \
    "$VCAD_REPO" \
    "$VCAD_COMMIT" \
    vcad_sources

write_manifest \
    "$tmp_openagents" \
    "openagents_starting_point" \
    "openagents" \
    "." \
    "$ROOT_DIR" \
    "$OPENAGENTS_COMMIT" \
    openagents_sources

mkdir -p "$PARITY_DIR"

if [[ "$MODE" == "check" ]]; then
    diff -u "$PARITY_DIR/vcad_reference_manifest.json" "$tmp_vcad"
    diff -u "$PARITY_DIR/openagents_start_manifest.json" "$tmp_openagents"
    printf 'VCAD parity baseline manifests are up to date.\n'
    exit 0
fi

cp "$tmp_vcad" "$PARITY_DIR/vcad_reference_manifest.json"
cp "$tmp_openagents" "$PARITY_DIR/openagents_start_manifest.json"
printf 'Wrote parity baseline manifests to %s\n' "$PARITY_DIR"

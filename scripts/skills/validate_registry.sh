#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SKILLS_ROOT="${1:-$ROOT_DIR/skills}"
AGENTSKILLS_DIR="${AGENTSKILLS_DIR:-$ROOT_DIR/../agentskills}"

if [[ ! -d "$SKILLS_ROOT" ]]; then
    printf 'Skills root not found: %s\n' "$SKILLS_ROOT" >&2
    exit 1
fi

skill_dirs=()
while IFS= read -r skill_md_path; do
    skill_dirs+=("$(dirname "$skill_md_path")")
done < <(
    find "$SKILLS_ROOT" -mindepth 3 -maxdepth 3 -type f \
        \( -name 'SKILL.md' -o -name 'skill.md' \) | sort
)

if [[ ${#skill_dirs[@]} -eq 0 ]]; then
    printf 'No skills discovered under %s (nothing to validate).\n' "$SKILLS_ROOT"
    exit 0
fi

run_validate() {
    local skill_dir="$1"

    if command -v skills-ref >/dev/null 2>&1; then
        skills-ref validate "$skill_dir"
        return
    fi

    local local_ref="$AGENTSKILLS_DIR/skills-ref"
    if [[ ! -d "$local_ref" ]]; then
        printf 'skills-ref not found on PATH and local reference implementation missing at %s\n' "$local_ref" >&2
        printf 'Install skills-ref or set AGENTSKILLS_DIR to a checkout of agentskills.\n' >&2
        exit 1
    fi

    uv run --project "$local_ref" skills-ref validate "$skill_dir"
}

printf 'Validating %s skill(s) under %s\n' "${#skill_dirs[@]}" "$SKILLS_ROOT"
for skill_dir in "${skill_dirs[@]}"; do
    printf ' - %s\n' "$skill_dir"
    run_validate "$skill_dir"
done

printf 'Skills registry validation passed.\n'

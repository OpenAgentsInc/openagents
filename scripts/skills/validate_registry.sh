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
while IFS= read -r project_dir; do
    root_skill_path=""
    if [[ -f "$project_dir/SKILL.md" ]]; then
        root_skill_path="$project_dir/SKILL.md"
    elif [[ -f "$project_dir/skill.md" ]]; then
        root_skill_path="$project_dir/skill.md"
    fi

    nested_skill_dirs=()
    while IFS= read -r nested_dir; do
        if [[ -f "$nested_dir/SKILL.md" || -f "$nested_dir/skill.md" ]]; then
            nested_skill_dirs+=("$nested_dir")
        fi
    done < <(find "$project_dir" -mindepth 1 -maxdepth 1 -type d | sort)

    if [[ -n "$root_skill_path" && ${#nested_skill_dirs[@]} -gt 0 ]]; then
        printf "Project mixes root SKILL.md with nested skill directories: %s\n" "$project_dir" >&2
        printf 'Choose one layout:\n' >&2
        printf ' - single skill at %s/SKILL.md\n' "$project_dir" >&2
        printf ' - or multiple nested skills at %s/<skill-name>/SKILL.md\n' "$project_dir" >&2
        exit 1
    fi

    if [[ -n "$root_skill_path" ]]; then
        skill_dirs+=("$project_dir")
        continue
    fi

    if [[ ${#nested_skill_dirs[@]} -gt 0 ]]; then
        skill_dirs+=("${nested_skill_dirs[@]}")
    fi
done < <(find "$SKILLS_ROOT" -mindepth 1 -maxdepth 1 -type d | sort)

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

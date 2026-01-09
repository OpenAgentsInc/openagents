#!/usr/bin/env bash
set -euo pipefail

pattern='(todo!\s*\(|unimplemented!\s*\(|panic!\s*\(\s*"[^"]*not[^"]*implement)'

mode="all"
files=()

if [[ "${1:-}" == "--staged" ]]; then
    mode="staged"
    shift
elif [[ "${1:-}" == "--files" ]]; then
    mode="files"
    shift
    files=("$@")
fi

if [[ "$mode" == "staged" ]]; then
    while IFS= read -r file; do
        files+=("$file")
    done < <(git diff --cached --name-only --diff-filter=ACM | rg '\.rs$' | rg -v '^(docs/|target/|vendor/|\.git/|tests/fixtures/|crates/dsrs/|crates/dsrs-macros/)' || true)
elif [[ "$mode" == "all" ]]; then
    while IFS= read -r file; do
        files+=("$file")
    done < <(rg --files -g '*.rs' --glob '!docs/**' --glob '!target/**' --glob '!vendor/**' --glob '!.git/**' --glob '!tests/fixtures/**' --glob '!crates/dsrs/**' --glob '!crates/dsrs-macros/**' . || true)
fi

if [[ ${#files[@]} -eq 0 ]]; then
    exit 0
fi

matches=""
for file in "${files[@]}"; do
    if [[ ! -f "$file" ]]; then
        continue
    fi
    result=$(rg -n "$pattern" "$file" || true)
    if [[ -n "$result" ]]; then
        matches+="$result"$'\n'
    fi
done

if [[ -n "$matches" ]]; then
    echo "❌ Stub patterns detected (d-012). Remove todo!(), unimplemented!(), or panic!(\"not implemented\") usage." >&2
    echo "" >&2
    echo "$matches" >&2
    echo "" >&2
    echo "See docs/development/stub-exceptions.md for allowed exceptions." >&2
    exit 1
fi

exit 0

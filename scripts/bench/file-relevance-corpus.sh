#!/usr/bin/env bash
# Rebuild file-relevance-v1 (#11215) from git and the issue tracker, and check it.
#
#   scripts/bench/file-relevance-corpus.sh OUT_DIR            # from the committed manifest
#   scripts/bench/file-relevance-corpus.sh OUT_DIR --select   # re-run the selection from git too
#
# The committed manifest (crates/gym/suites/file-relevance-v1/) pins every item: issue, fix
# and parent commits, path, blob, label, and a digest of the item's text. This script reads
# the file heads from git at those commits and the issue text from GitHub, writes
# OUT_DIR/file-relevance-v1.corpus.json (it holds file contents and issue text: keep it out of
# git), and checks every digest. With --select it first rebuilds the issue -> fix dataset from
# main's history at the pinned rev and re-selects the items, which must reproduce the manifest.
# When tenant-train is on PATH (cargo build -p tenancy --bin tenant-train), the corpus is also
# registered with `tenant-train check` in OUT_DIR/registry.
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
out="${1:?usage: file-relevance-corpus.sh OUT_DIR [--select]}"
suite="$root/crates/gym/suites/file-relevance-v1"
mkdir -p "$out"
gh issue list -R OpenAgentsInc/openagents --state closed --limit 20000 \
  --json number,title,body,closedAt,createdAt > "$out/closed-issues.json"
if [ "${2:-}" = "--select" ]; then
  rev="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["rev"])' "$suite/manifest.json")"
  python3 -I "$root/scripts/bench/file-finding-dataset.py" --repo "$root" --rev "$rev" \
    --issues "$out/closed-issues.json" --multi --max-hand 40 --out "$out/dataset.json"
  python3 "$root/scripts/bench/file-relevance-corpus.py" build --repo "$root" \
    --dataset "$out/dataset.json" --out-dir "$out/select"
  for f in items.tsv.gz issues.tsv; do
    cmp -s "$out/select/$f" "$suite/$f" || { echo "selection differs from the committed $f" >&2; exit 1; }
  done
  echo "selection reproduces the committed manifest"
fi
python3 "$root/scripts/bench/file-relevance-corpus.py" materialize --repo "$root" \
  --manifest "$suite" --issues "$out/closed-issues.json" --out "$out/file-relevance-v1.corpus.json"
if command -v tenant-train >/dev/null; then
  tenant-train check --registry "$out/registry" --corpus "$out/file-relevance-v1.corpus.json"
fi

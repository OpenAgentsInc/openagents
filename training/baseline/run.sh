#!/usr/bin/env bash
# Produces every row in docs/decision-models/2026-09-19-frozen-embedding-baseline.md.
#
# Two suites, four featurizers, one command. The encoder revisions are pinned
# here so a rerun on a later day is the same run and not a different one.
#
#     ./run.sh
#
set -euo pipefail
cd "$(dirname "$0")"

PY=${PY:-.venv/bin/python}

MINILM=1110a243fdf4706b3f48f1d95db1a4f5529b4d41   # sentence-transformers/all-MiniLM-L6-v2
MPNET=e8c3b32edf5434bc2275fc9bab85f82640a19130    # sentence-transformers/all-mpnet-base-v2
BGE=a5beb1e3e68b9ab74eb54cfd186867f64f240e1a      # BAAI/bge-base-en-v1.5

"$PY" check_panel.py

for suite in ../../crates/lev/suites/support-v2.json \
             ../../crates/gym/suites/support-v2-three-way.json; do
    name=$(basename "$suite" .json)
    "$PY" baseline.py --suite "$suite" --encoder sentence-transformers/all-MiniLM-L6-v2 \
        --revision "$MINILM" --out "runs/$name-minilm.json"
    "$PY" baseline.py --suite "$suite" --encoder sentence-transformers/all-mpnet-base-v2 \
        --revision "$MPNET" --out "runs/$name-mpnet.json"
    "$PY" baseline.py --suite "$suite" --encoder BAAI/bge-base-en-v1.5 \
        --revision "$BGE" --out "runs/$name-bge.json"
    "$PY" baseline.py --suite "$suite" --encoder tfidf --out "runs/$name-tfidf.json"
done

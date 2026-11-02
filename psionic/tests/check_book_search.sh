#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs coax test, coax run and coax mutate on chapters 54 to 57's and 59's files)
# Chapters 54 to 57 and 59, book search, its HNSW index, its reranker, its
# cited answers and its browser bundle: Coax checks in both build modes, the examples against their
# recorded output, zero-survivor mutation of the ranking and status rules
# (rank.coax, #authority), of the index (hnsw.coax), of the reranker
# (rerank.coax), of the answer and citation rules (answer.coax,
# #authority) and of the bundle admission rule (bundle.coax, #authority),
# each on its own procedures, and the book. browser.coax (the bundle reader
# and search) is not mutation-gated yet: 108 of 968 mutants survive. The browser run itself is psionic/web/book-search/check.sh. CPU only; it does not
# read the books or train (the tool does: coax/web/tool.sh psionic/tools/book-search.coax).
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-book-search.XXXXXX");trap 'rm -rf "$out"' EXIT
for unit in rank corpus encoder hnsw rerank answer bundle browser;do
 "$coax" test "psionic/lib/book-search/$unit.coax" --library
 "$coax" test "psionic/lib/book-search/$unit.coax" --library --release
done
for example in book-search/book-search book-search/book-search-stale hnsw/hnsw hnsw/hnsw-refuse rerank/rerank rerank/rerank-refuse answer/answer answer/answer-refuse browser-search/browser-search browser-search/browser-search-refuse;do
 name=${example#*/}
 "$coax" run "psionic/examples/$name.coax" > "$out/$name.out";cmp "psionic/fixtures/$example.out" "$out/$name.out"
 "$coax" run "psionic/examples/$name.coax" --release > "$out/$name-release.out";cmp "psionic/fixtures/$example.out" "$out/$name-release.out"
done
if [[ $mutate == 1 ]];then
 "$coax" mutate psionic/lib/book-search/rank.coax --library --jobs "$jobs"
 "$coax" mutate psionic/lib/book-search/hnsw.coax --library --root-only --jobs "$jobs"
 "$coax" mutate psionic/lib/book-search/rerank.coax --library --root-only --jobs "$jobs"
 "$coax" mutate psionic/lib/book-search/answer.coax --library --root-only --jobs "$jobs"
 "$coax" mutate psionic/lib/book-search/bundle.coax --library --jobs "$jobs"
fi
python3 -I psionic/tests/book.py

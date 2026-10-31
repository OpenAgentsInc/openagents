#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: runs coax test, coax run and coax mutate on chapter 54's files)
# Chapter 54, book search: Coax checks in both build modes, the examples
# against their recorded output, zero-survivor mutation of the ranking and
# status rules (rank.coax, #authority), and the book. CPU only; it does not
# read the books or train (the tool does: coax/web/tool.sh psionic/tools/book-search.coax).
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-book-search.XXXXXX");trap 'rm -rf "$out"' EXIT
for unit in rank corpus encoder;do
 "$coax" test "psionic/lib/book-search/$unit.coax" --library
 "$coax" test "psionic/lib/book-search/$unit.coax" --library --release
done
for example in book-search book-search-stale;do
 "$coax" run "psionic/examples/$example.coax" > "$out/$example.out";cmp "psionic/fixtures/book-search/$example.out" "$out/$example.out"
 "$coax" run "psionic/examples/$example.coax" --release > "$out/$example-release.out";cmp "psionic/fixtures/book-search/$example.out" "$out/$example-release.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/book-search/rank.coax --library --jobs "$jobs";fi
python3 -I psionic/tests/book.py

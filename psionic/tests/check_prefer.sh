#!/usr/bin/env bash
# Chapter 138: the recorded preference run reproduces its transcript in
# release. The unit's checks and mutation run through psionic/check.sh
# --only nanochat/prefer; the short example runs with the book.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
# The run keeps about 16 MiB of buffers on the stack.
ulimit -s 65536
"$coax" run psionic/tests/prefer-long.coax --release | cmp - psionic/fixtures/prefer/long.out
echo 'prefer: the recorded preference run reproduces its transcript in release'

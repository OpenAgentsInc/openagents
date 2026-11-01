#!/usr/bin/env bash
# Chapter 136: the recorded supervised fine-tuning run reproduces its
# transcript in release. The unit's checks and mutation run through
# psionic/check.sh --only nanochat/sft; the short example runs with the book.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
# The run keeps about 10 MiB of buffers on the stack.
ulimit -s 65536
"$coax" run psionic/tests/sft-long.coax --release | cmp - psionic/fixtures/sft/long.out
echo 'sft: the recorded tuning run reproduces its transcript in release'

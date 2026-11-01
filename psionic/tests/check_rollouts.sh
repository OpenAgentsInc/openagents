#!/usr/bin/env bash
# Chapter 139: the recorded rollout run reproduces its transcript in
# release. The unit's checks and mutation run through psionic/check.sh
# --only nanochat/rollouts; the short example runs with the book.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
# The run keeps about 16 MiB of buffers on the stack.
ulimit -s 65536
"$coax" run psionic/tests/rollouts-long.coax --release | cmp - psionic/fixtures/rollouts/long.out
echo 'rollouts: the recorded rollout run reproduces its transcript in release'

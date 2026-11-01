#!/usr/bin/env bash
# Chapter 137: the domain corpus regenerates exactly, and the recorded runs
# (chapter 128's 1000-step base, then three 300-step continued runs)
# reproduce their transcript in release. The unit's checks and mutation run
# through psionic/check.sh --only nanochat/continue.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
python3 -I psionic/tools/continue_corpus.py | cmp - psionic/fixtures/continue/domain.coax
# The runs keep about 16 MiB of buffers on the stack.
ulimit -s 65536
"$coax" run psionic/tests/continue-long.coax --release | cmp - psionic/fixtures/continue/long.out
echo 'continue: the domain corpus regenerates; the base and three continued runs reproduce their transcript in release'

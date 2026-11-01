#!/usr/bin/env bash
# Chapter 128: the corpus and the independent reference values regenerate
# exactly, and the recorded 1000-step run reproduces its transcript in
# release (about a minute on one core). The unit's checks and mutation run
# through psionic/check.sh --only nanochat/pretrain.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
python3 -I psionic/tools/pretrain_corpus.py | cmp - psionic/fixtures/pretrain/corpus.coax
python3 -I psionic/tests/pretrain_oracle.py 2>/dev/null | cmp - psionic/fixtures/nanochat/pretrain-oracle.coax
# The run keeps about 8 MiB of buffers on the stack.
ulimit -s 65536
"$coax" run psionic/tests/pretrain-long.coax --release | cmp - psionic/fixtures/pretrain/long.out
echo 'pretrain: corpus and oracle regenerate; the 1000-step run reproduces its transcript in release'

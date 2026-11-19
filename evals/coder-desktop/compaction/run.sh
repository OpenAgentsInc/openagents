#!/bin/sh
# Firm reason: 5 (thin shell wrapper invoking coax test for the compaction eval suite)
set -eu

# Deterministic & long-horizon compaction evaluation suite (GitHub #13706).
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"

cd "$ROOT"
exec coax/bin/coax test evals/coder-desktop/compaction/compaction_test.coax --library

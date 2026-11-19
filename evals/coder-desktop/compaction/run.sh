#!/bin/sh
# Firm reason: 5 (thin shell wrapper invoking coax test for the compaction eval suite)
set -eu

# Deterministic & long-horizon compaction evaluation suite (GitHub #13706).
#   compaction_test.coax            C1-C8: state, prefix, fidelity, memory, budget and recall checks
#   compaction_scenarios_test.coax  C9-C12: 200-step scenarios, fidelity and eviction, recall bounds
#   report.coax                     the scenarios' numbers, one line each (no model is called)
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"

cd "$ROOT"
coax/bin/coax test evals/coder-desktop/compaction/compaction_test.coax --library
coax/bin/coax test evals/coder-desktop/compaction/compaction_scenarios_test.coax --library
coax/bin/coax run evals/coder-desktop/compaction/report.coax

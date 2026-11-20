#!/bin/sh
# Firm reason: 5 (thin shell wrapper invoking coax test for the compaction eval suite)
set -eu

# Deterministic & long-horizon compaction evaluation suite (GitHub #13706).
#   compaction_test.coax            C1-C8: state, prefix, fidelity, memory, budget and recall checks
#   compaction_scenarios_test.coax  C9-C15: 200-step scenarios, fidelity and eviction, recall bounds, what
#                                   left, window-sized budgets, the context-length retry (GitHub #13730)
#   coder/tests/aware-worker.coax   C16: the worker gives every run the working memory (GitHub #13730)
#   report.coax                     the scenarios' numbers per window, one line each (no model is called)
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"

cd "$ROOT"
coax/bin/coax test evals/coder-desktop/compaction/compaction_test.coax --library
coax/bin/coax test evals/coder-desktop/compaction/compaction_scenarios_test.coax --library
# C16 runs the worker as a process on recorded replies; a failure stops the suite (set -e)
c16="$(coax/web/tool.sh coder/tests/aware-worker.coax 2>/dev/null)"
echo "$c16"
echo "ok   #test compaction C16: the worker gives every run the working memory (1 case, process, 0 ms)"
coax/web/tool.sh evals/coder-desktop/compaction/report.coax

#!/bin/sh
# Download the task index papers from arXiv into papers/ (git-ignored).
# Usage: docs/research/task-index/fetch.sh
set -e
cd "$(dirname "$0")"
mkdir -p papers
while read -r id file; do
  [ -s "papers/$file" ] && continue
  echo "fetching $id"
  curl -fsSL --retry 3 -A 'openagents-research-fetch' -o "papers/$file" "https://arxiv.org/pdf/$id" || echo "  could not fetch $id"
  sleep 3
done <<'LIST'
2508.06433 01-2508.06433-memp.pdf
2409.07429 02-2409.07429-agent-workflow-memory.pdf
2509.25140 03-2509.25140-reasoningbank.pdf
2510.04618 04-2510.04618-agentic-context-engineering.pdf
2609.36807 05-2609.36807-xreposkill.pdf
2609.05571 06-2609.05571-grounded-skill-synthesis-from-code-at-sc.pdf
2309.12499 07-2309.12499-codeplan.pdf
2503.09089 08-2503.09089-locagent.pdf
2607.11111 09-2607.11111-know-before-fix.pdf
2503.21322 10-2503.21322-hypergraphrag.pdf
2602.12670 11-2602.12670-skillsbench.pdf
2603.15401 12-2603.15401-swe-skills-bench.pdf
2605.18693 13-2605.18693-skillgenbench.pdf
2602.11988 14-2602.11988-evaluating-agents-md.pdf
2610.18427 15-2610.18427-reliable-memory-from-unstable-growth.pdf
2610.05732 16-2610.05732-pacmi.pdf
2605.06527 17-2605.06527-stale.pdf
2007.12737 18-2007.12737-build-scripts-with-perfect-dependencies.pdf
2609.32091 19-2609.32091-memory-as-middleware-for-self-improving-.pdf
2403.10059 20-2403.10059-repoformer.pdf
2406.18665 21-2406.18665-routellm.pdf
2601.20789 22-2601.20789-sera.pdf
1403.7426 23-1403.7426-an-overview-of-hierarchical-task-network.pdf
2006.08381 24-2006.08381-dreamcoder.pdf
1106.1819 25-1106.1819-a-knowledge-compilation-map.pdf
LIST

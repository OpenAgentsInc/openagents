#!/bin/bash
# Count all tests across the OpenAgents codebase
# Usage: ./scripts/count_tests.sh

set -e

echo "# Test Count Report"
echo "Generated: $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
echo ""

total=0

# Function to count tests in a directory
count_tests() {
    local crate=$1
    local test_dir=$2
    local pattern=$3

    if [ ! -d "$test_dir" ]; then
        return
    fi

    local count=$(find "$test_dir" -name "*.rs" -type f -exec grep -c "$pattern" {} + 2>/dev/null | awk '{s+=$1} END {print s}')

    if [ -z "$count" ]; then
        count=0
    fi

    if [ "$count" -gt 0 ]; then
        echo "$crate: $count tests"
        total=$((total + count))
    fi
}

echo "## Integration Tests"
count_tests "autopilot" "crates/autopilot/tests" "^#\[test\]"
count_tests "autopilot" "crates/autopilot/tests" "^#\[tokio::test\]"
count_tests "compute" "crates/compute/tests" "^async fn test_"
count_tests "issues" "crates/issues/tests" "^#\[test\]"
count_tests "issues-mcp" "crates/issues-mcp/tests" "^#\[test\]"
count_tests "marketplace" "crates/marketplace/tests" "^#\[test\]"
count_tests "nostr" "crates/nostr/core/tests" "^#\[test\]"
count_tests "recorder" "crates/recorder/tests" "^#\[test\]"
count_tests "ui" "crates/ui/tests" "^#\[test\]"

echo ""
echo "## Inline Tests"
count_tests "autopilot" "crates/autopilot/src" "^#\[test\]"
count_tests "compute" "crates/compute/src" "^#\[test\]"
count_tests "issues" "crates/issues/src" "^#\[test\]"
count_tests "marketplace" "crates/marketplace/src" "^#\[test\]"
count_tests "nostr" "crates/nostr/core/src" "^#\[test\]"
count_tests "recorder" "crates/recorder/src" "^#\[test\]"
count_tests "ui" "crates/ui/src" "^#\[test\]"

echo ""
echo "## Total: $total tests"

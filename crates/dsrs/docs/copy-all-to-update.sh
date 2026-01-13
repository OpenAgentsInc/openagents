#!/bin/bash
# Copy OTHER docs (not in copy-all.sh) to clipboard for review/update
# These docs may contain outdated terminology that conflicts with the canonical docs

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$SCRIPT_DIR/../../.."

# Priority 1: Core execution & decision pipeline docs (most likely to have terminology drift)
PRIORITY_1=(
    # Autopilot-core execution flow
    "$ROOT_DIR/crates/autopilot-core/docs/EXECUTION_FLOW.md"
    "$ROOT_DIR/crates/autopilot-core/docs/REPLAY.md"

    # Adjutant DSPy pipelines
    "$ROOT_DIR/crates/adjutant/docs/README.md"
    "$ROOT_DIR/crates/adjutant/docs/ARCHITECTURE.md"

    # Protocol surface (NIP-90, job types)
    "$ROOT_DIR/docs/PROTOCOL_SURFACE.md"

    # RLM tool definitions
    "$ROOT_DIR/crates/rlm/docs/README.md"
    "$ROOT_DIR/crates/rlm/docs/ARCHITECTURE.md"
)

# Priority 2: Runtime & infrastructure docs
PRIORITY_2=(
    # Runtime (large surface area)
    "$ROOT_DIR/crates/runtime/docs/README.md"
    "$ROOT_DIR/crates/runtime/docs/ARCHITECTURE.md"
    "$ROOT_DIR/crates/runtime/docs/EXECUTION.md"
    "$ROOT_DIR/crates/runtime/docs/METRICS.md"
    "$ROOT_DIR/crates/runtime/docs/TOOLS.md"
    "$ROOT_DIR/crates/runtime/docs/REPLAY.md"

    # Protocol definitions
    "$ROOT_DIR/crates/protocol/docs/README.md"
    "$ROOT_DIR/crates/protocol/docs/JOBS.md"
    "$ROOT_DIR/crates/protocol/docs/RECEIPTS.md"

    # Pylon (provider/DVM)
    "$ROOT_DIR/crates/pylon/docs/README.md"
    "$ROOT_DIR/crates/pylon/docs/ARCHITECTURE.md"
    "$ROOT_DIR/crates/pylon/docs/PROVIDER.md"
)

# Priority 3: Autopilot product surface
PRIORITY_3=(
    "$ROOT_DIR/crates/autopilot/docs/README.md"
    "$ROOT_DIR/crates/autopilot/docs/CLI.md"
    "$ROOT_DIR/crates/autopilot/docs/SESSIONS.md"
    "$ROOT_DIR/crates/autopilot/docs/UI.md"
)

# Priority 4: Nostr/network layer
PRIORITY_4=(
    "$ROOT_DIR/crates/nostr/docs/README.md"
    "$ROOT_DIR/crates/nexus/docs/README.md"
    "$ROOT_DIR/crates/frlm/docs/README.md"
)

# Priority 5: Feature crates & .openagents directives
PRIORITY_5=(
    # .openagents user-facing docs
    "$ROOT_DIR/.openagents/TODO.md"
    "$ROOT_DIR/.openagents/USERSTORIES.md"
    "$ROOT_DIR/.openagents/DIRECTIVES.md"

    # Feature crate READMEs
    "$ROOT_DIR/crates/issues/docs/README.md"
    "$ROOT_DIR/crates/gateway/docs/README.md"
    "$ROOT_DIR/crates/spark/docs/README.md"
    "$ROOT_DIR/crates/onyx/docs/README.md"
    "$ROOT_DIR/crates/agent/docs/README.md"
)

# Combine all priorities
ALL_DOCS=(
    "${PRIORITY_1[@]}"
    "${PRIORITY_2[@]}"
    "${PRIORITY_3[@]}"
    "${PRIORITY_4[@]}"
    "${PRIORITY_5[@]}"
)

# Count existing files
file_count=0
for doc in "${ALL_DOCS[@]}"; do
    [ -f "$doc" ] && file_count=$((file_count + 1))
done

# Copy to clipboard with priority labels
{
    echo "=== DOCS NEEDING UPDATE (not in canonical set) ==="
    echo ""
    echo "These docs may contain outdated terminology. Check for:"
    echo "  - 'policy_version' → should be 'policy_bundle_id'"
    echo "  - 'step_utility' in 0-1 range → should be 'step_utility_norm'"
    echo "  - 'Verified PR Bundle' → should be 'Verified Patch Bundle'"
    echo "  - 'PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl' paths"
    echo "  - Hardcoded kind numbers → should reference schema IDs"
    echo "  - 'Datacenter' lane → should be 'Cloud'"
    echo "  - 'proof' for Cashu → should be 'Cashu Proof'"
    echo ""
    echo "Reference: GLOSSARY.md wins terminology conflicts"
    echo ""

    current_priority=""

    for i in "${!ALL_DOCS[@]}"; do
        file="${ALL_DOCS[$i]}"

        # Determine priority tier
        if [ $i -lt ${#PRIORITY_1[@]} ]; then
            tier="PRIORITY 1 (Core Execution)"
        elif [ $i -lt $((${#PRIORITY_1[@]} + ${#PRIORITY_2[@]})) ]; then
            tier="PRIORITY 2 (Runtime/Infrastructure)"
        elif [ $i -lt $((${#PRIORITY_1[@]} + ${#PRIORITY_2[@]} + ${#PRIORITY_3[@]})) ]; then
            tier="PRIORITY 3 (Autopilot Product)"
        elif [ $i -lt $((${#PRIORITY_1[@]} + ${#PRIORITY_2[@]} + ${#PRIORITY_3[@]} + ${#PRIORITY_4[@]})) ]; then
            tier="PRIORITY 4 (Network Layer)"
        else
            tier="PRIORITY 5 (Features/Directives)"
        fi

        # Print tier header on change
        if [ "$tier" != "$current_priority" ]; then
            if [ -n "$current_priority" ]; then
                echo ""
                echo ""
            fi
            echo "========================================"
            echo "=== $tier ==="
            echo "========================================"
            current_priority="$tier"
        fi

        if [ -f "$file" ]; then
            # Get relative path for readability
            relpath="${file#$ROOT_DIR/}"
            echo ""
            echo "---- $relpath ----"
            echo ""
            cat "$file"
        fi
    done
} | pbcopy

echo "Copied $file_count docs to clipboard (prioritized for update review)"
echo ""
echo "Priority breakdown:"
echo "  P1 (Core Execution):      ${#PRIORITY_1[@]} files"
echo "  P2 (Runtime/Infra):       ${#PRIORITY_2[@]} files"
echo "  P3 (Autopilot Product):   ${#PRIORITY_3[@]} files"
echo "  P4 (Network Layer):       ${#PRIORITY_4[@]} files"
echo "  P5 (Features/Directives): ${#PRIORITY_5[@]} files"

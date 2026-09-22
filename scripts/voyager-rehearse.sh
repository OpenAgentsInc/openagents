#!/usr/bin/env bash
# voyager-rehearse.sh — the D5 rehearsal harness: run the arena demo
# end-to-end, retain the evidence, repeat. Two rehearsals is the demo
# doc's bar; each run lands in its own directory under the runs root
# and gets its coverage.json, metrics.json, and evidence.md.
#
# Usage:
#   ./scripts/voyager-rehearse.sh [--scenario quest|war] [--count N]
#
# Prerequisites (the script checks rather than assumes):
#   - ./scripts/build-mc-bridge.sh has produced the helper
#   - the server jar for the arena's minecraft version is fetched
#   - java resolves (VOYAGER_JAVA, PATH, or Homebrew openjdk)
#   - postgres is reachable for the relay (VOYAGER_RELAY_DATABASE_URL)
#
# Environment:
#   VOYAGER_RUNS            — where runs land (default ~/.openagents/voyager/runs)
#   VOYAGER_RELAY_BIN       — nostr-relay binary override
#   VOYAGER_RELAY_DATABASE_URL — relay store (default postgres://127.0.0.1:5432/voyager_relay)
set -euo pipefail

cd "$(dirname "$0")/.."

SCENARIO=""
COUNT=2
while [ $# -gt 0 ]; do
    case "$1" in
        --scenario) SCENARIO="$2"; shift 2 ;;
        --count) COUNT="$2"; shift 2 ;;
        *) echo "unknown argument $1" >&2; exit 2 ;;
    esac
done

RUNS="${VOYAGER_RUNS:-$HOME/.openagents/voyager/runs}"
mkdir -p "$RUNS"

echo "rehearsal: building the voyager binary"
cargo build -p voyager --bin voyager 2>&1 | tail -1

VOYAGER="$(pwd)/target/debug/voyager"
if [ ! -x "$VOYAGER" ]; then
    echo "rehearsal: $VOYAGER missing" >&2
    exit 1
fi

# The helper's path follows build-mc-bridge.sh's own rule:
# $CARGO_TARGET_DIR or the worktree's target-mc.
BRIDGE="${CARGO_TARGET_DIR:-$(pwd)/target-mc}/release/mc-bridge"
if [ ! -x "$BRIDGE" ]; then
    echo "rehearsal: mc-bridge not built at $BRIDGE" >&2
    echo "rehearsal: run ./scripts/build-mc-bridge.sh first" >&2
    exit 1
fi

for n in $(seq 1 "$COUNT"); do
    echo
    echo "rehearsal $n of $COUNT (scenario: ${SCENARIO:-manifest})"
    before="$(ls -td "$RUNS"/*/ 2>/dev/null | head -1 || true)"
    args=(run --world worlds/arena.json --bridge "$BRIDGE" --runs "$RUNS")
    if [ -n "$SCENARIO" ]; then
        args+=(--scenario "$SCENARIO")
    fi
    if "$VOYAGER" "${args[@]}"; then
        echo "rehearsal $n: episode completed"
    else
        echo "rehearsal $n: episode reported failures (kept for evidence)"
    fi
    # The newest run directory is this rehearsal's.
    after="$(ls -td "$RUNS"/*/ 2>/dev/null | head -1 || true)"
    if [ -n "$after" ] && [ "$after" != "$before" ]; then
        echo "rehearsal $n: rendering evidence in $after"
        "$VOYAGER" evidence "$after"
    fi
done

echo
echo "rehearsals retained under $RUNS"

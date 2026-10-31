#!/usr/bin/env bash
# Run chapter 170's load generator: BUILD_DIR then bench-drive's arguments.
# When PSIONIC_API_KEY is unset and the target port belongs to a server this
# user runs on this machine, its key is read from that process's environment
# (Linux: ss and /proc). The key is passed on and never printed.
set -euo pipefail
dir=${1:?usage: psionic/platform/bench-run.sh BUILD_DIR responses|systemone PORT ...}; shift
if [[ -z ${PSIONIC_API_KEY:-} && $# -ge 2 && $1 != compare ]]; then
    for pid in $(ss -ltnpH "sport = :$2" 2>/dev/null | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); do
        [[ -O /proc/$pid ]] || continue
        key=$(tr '\0' '\n' < "/proc/$pid/environ" 2>/dev/null | sed -n 's/^PSIONIC_API_KEY=//p' | head -n 1)
        if [[ -n $key ]]; then export PSIONIC_API_KEY=$key; break; fi
    done
fi
exec "$dir/bench-drive" "$@"

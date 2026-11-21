#!/usr/bin/env bash
# Firm reason: 5 (process wiring only: no logic). The container's start: the hosted server on
# 127.0.0.1:8811 and socat passing PORT's connections to it. Either one ending ends the container.
set -euo pipefail
mkdir -p "$GB_ACCOUNTS"
gamebuilder serve --accounts "$GB_ACCOUNTS" --origin "$GB_ORIGIN" --port 8811 &
# socat listens only once the server does: Cloud Run sends the waiting request the moment PORT
# accepts, and one passed on before 8811 listens is refused and the instance is thrown away.
for _ in $(seq 1 200); do
  (exec 3<>/dev/tcp/127.0.0.1/8811) 2>/dev/null && break
  sleep 0.05
done
socat "TCP-LISTEN:${PORT},fork,reuseaddr" TCP:127.0.0.1:8811 &
wait -n
exit 1

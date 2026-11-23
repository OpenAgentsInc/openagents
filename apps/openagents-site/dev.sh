#!/usr/bin/env bash
# Firm reason: 5 (thin local static dev server wrapper for apps/openagents-site).
set -euo pipefail
dir="$(cd "$(dirname "$0")" && pwd)"
port="${1:-8800}"
host="${OA_SITE_HOST:-127.0.0.1}"
echo "OpenAgents site dev server: http://${host}:${port}/"
exec python3 -m http.server "$port" --bind "$host" --directory "$dir"

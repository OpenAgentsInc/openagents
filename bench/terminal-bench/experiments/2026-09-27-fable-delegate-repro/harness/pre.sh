#!/usr/bin/env bash
# Preflight before each #9776 attempt: other Claude-login runs and disk.
echo "== tbench/harbor runs:"; pgrep -af 'tbench run|harbor run' | grep -v pgrep || echo none
echo "== claude print-mode sessions (delegates):"; pgrep -af 'claude .*( -p | -p$|--print|stream-json)' | grep -v pgrep || echo none
df -h / | tail -1

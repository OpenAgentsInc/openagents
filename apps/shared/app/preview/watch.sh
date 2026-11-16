#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: runs the Coax tool loop.coax; holds no logic).
# Keeps a picture of the phone app's screen up to date while you edit (README.md).
#   apps/shared/app/preview/watch.sh [--out DIR] [--all] [--window] [--size ...] [--case FILE]
exec "$(dirname "$0")/../../../../coax/web/tool.sh" "$(dirname "$0")/loop.coax" watch "$@"

#!/usr/bin/env bash
# Run verse-kms on the screen: switch to a free VT so the compositor lets go
# of the display, run the realm, and always switch back.
#   sudo verse/kms/run.sh [BUILD_DIR] [verse-kms args...]   e.g. --seconds 60
# Escape leaves the realm; the --seconds limit (default 60) ends it anyway.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
dir="$here/build"
if [[ $# -gt 0 && -d "$1" ]]; then dir="$1"; shift; fi
[[ $EUID -eq 0 ]] || { echo "run.sh needs root (DRM master and the VT)" >&2; exit 1; }
from="$(fgconsole)"
to="$(fgconsole --next-available)"
back() { chvt "$from"; }
trap back EXIT
chvt "$to"
sleep 1   # the session pauses and drops DRM master
"$dir/verse-kms" "$@"

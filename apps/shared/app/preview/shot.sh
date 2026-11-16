#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: runs the Coax tool loop.coax; holds no logic).
# A picture of the phone app's screen on the Mac, with no Xcode and no simulator (README.md).
#   apps/shared/app/preview/shot.sh [--size WxH|small|phone|large] [--large-text] [--keyboard H] [--text TEXT] [--case FILE] OUT.png
#   apps/shared/app/preview/shot.sh --all DIR
exec "$(dirname "$0")/../../../../coax/web/tool.sh" "$(dirname "$0")/loop.coax" shot "$@"

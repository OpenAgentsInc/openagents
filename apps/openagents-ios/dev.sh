#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: runs the Coax tool apps/shared/app/preview/loop.coax; holds no logic).
# The shortest simulator loop: generate only if a Coax input changed, xcodegen only if the project
# changed, an incremental build of the app, install, relaunch, one screenshot, and each step's time.
# No test suites. It uses the booted simulator unless another session contests it, then a spare,
# and waits for another xcodebuild to end (apps/shared/app/preview/README.md).
#   apps/openagents-ios/dev.sh [--sim NAME] [--shot OUT.png] [--settle MS]
exec "$(dirname "$0")/../../coax/web/tool.sh" "$(dirname "$0")/../shared/app/preview/loop.coax" ios "$@"

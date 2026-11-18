#!/usr/bin/env bash
# Firm reason: 4 (the bootstrap: no Coax tool runs before this tree's compiler is installed); then
# a thin wrapper (5): every check and rule lives in coax/web/contributor-check.coax.
# The one check an outside contributor runs before opening a pull request (CONTRIBUTING.md,
# GitHub #13663). It needs no secrets, no logins and no private hosts: it installs this tree's Coax
# compiler (coax/build.sh: the published build, else a bootstrap from source), then runs the local
# checks against the point where your branch left main. The compiler's own mutation gates are off
# (COAX_MUTATION=0, AGENTS.md "Getting the compiler"): they judge compiler changes, take over an
# hour on a bootstrap, and run on the team's release hosts after a merge.
#   scripts/check.sh               every check
#   scripts/check.sh --list        the commands, none run
#   scripts/check.sh --base REF    compare with REF (default: upstream/main, else origin/main)
set -euo pipefail
cd "$(dirname "$0")/.."
COAX_MUTATION=0 coax/build.sh
COAX="$PWD/coax/bin/coax" exec coax/web/tool.sh contributor-check "$@"

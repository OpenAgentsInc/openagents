#!/usr/bin/env bash
# Focused authority/native gate; the real restart runner takes prebuilt artifacts
# so it can reuse a qualified relay, toy engine and unchanged native fleet.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
"$coax" test psionic/lib/pylon/generation.coax --library
"$coax" test psionic/lib/pylon/generation.coax --library --release
"$coax" mutate psionic/lib/pylon/generation.coax --library --root-only --jobs "${PSIONIC_JOBS:-4}"
psionic/tests/pylon/generation_host.sh

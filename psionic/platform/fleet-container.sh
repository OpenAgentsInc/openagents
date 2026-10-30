#!/usr/bin/env bash
# Produce a private Cloud Run fleet build context. Compilation policy stays
# in Coax; the container supplies native networking, TLS and process effects.
set -euo pipefail
cd "$(dirname "$0")/../.."
[[ $# == 1 && ! -e "$1" ]] || { echo 'usage: fleet-container.sh NEW_CONTEXT_DIRECTORY' >&2; exit 2; }
out=$1
coax=${COAX:-$PWD/coax/bin/coax}
mkdir -p "$out/platform"
"$coax" c psionic/lib/fleet/run.coax --run-steps 250000000 > "$out/fleet.c"
"$coax" c coax/lib/crypto/entropy.coax --library --header "$out/entropy-policy.h" > /dev/null
cp "${FLEET_HOST:-psionic/platform/fleet-host.c}" "$out/fleet-host.c"
for name in socket-native tls-native entropy-native entropy-foreign process-native; do
  cp "coax/platform/$name.c" "coax/platform/$name.h" "$out/platform/"
done
cp psionic/platform/fleet-container/Dockerfile psionic/platform/fleet-container/start.sh "$out/"
git rev-parse HEAD > "$out/SOURCE_COMMIT"
shasum -a 256 "$coax" | cut -d ' ' -f1 > "$out/COAX_SHA256"
(cd "$out"; find . -type f ! -name SOURCES.sha256 -exec shasum -a 256 {} \; | LC_ALL=C sort > SOURCES.sha256)
echo "fleet: generated $out (fresh throwaway buyer; private IAM service required)"

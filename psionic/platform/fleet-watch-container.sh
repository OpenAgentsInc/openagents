#!/usr/bin/env bash
# Assemble immutable native source + unchanged optional attestation oracle.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
[[ $# == 3 && ! -e $1 && -d $2 && $3 =~ ^us-central1-docker.pkg.dev/openagentsgemini/[a-z0-9_./-]+@sha256:[0-9a-f]{64}$ ]] || { echo 'usage: fleet-watch-container.sh NEW_CONTEXT TEXT_CLIENT_CONTEXT CONFIDENTIAL_IMAGE@sha256:DIGEST' >&2;exit 2; }
out=$1;client=$2;confidential=$3
mkdir -p "$out"
"$root/psionic/platform/fleet-watch-build.sh" "$out"
# Binaries above qualify local adapters; Linux images compile the same C sources.
rm -f "$out/fleet-watch" "$out/fleet-watch-probe"
cp "$root/psionic/platform/fleet-watch-face.c" "$root/psionic/platform/fleet-watch-probe-face.c" "$out/"
cp "$root/psionic/platform/fleet-watch-run.sh" "$root/psionic/platform/probe-gptoss.sh" "$out/"
mkdir "$out/text";cp -R "$client/." "$out/text/"
for file in text-probe.c text-probe.h entropy-policy.h pylon-host.c text-probe-entry.c platform/socket-native.c platform/tls-native.c platform/entropy-native.c platform/entropy-foreign.c;do [[ -f "$out/text/$file" ]] || { echo "missing native client input: $file" >&2;exit 1; };done
{ printf 'ARG CONFIDENTIAL_IMAGE=%s\n' "$confidential";tail -n +2 "$root/psionic/platform/fleet-watch.Dockerfile"; } > "$out/Dockerfile"
git -C "$root" rev-parse HEAD > "$out/SOURCE_COMMIT"
shasum -a 256 "${COAX:-$root/coax/bin/coax}" | cut -d ' ' -f1 > "$out/COAX_SHA256"
printf '%s\n' "$confidential" > "$out/CONFIDENTIAL_IMAGE"
(cd "$out";find . -type f ! -name SOURCES.sha256 -exec shasum -a 256 {} \; | LC_ALL=C sort > SOURCES.sha256)
echo "watch context: $out; confidential component pinned to $confidential"

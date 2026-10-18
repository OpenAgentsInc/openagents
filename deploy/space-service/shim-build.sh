#!/usr/bin/env bash
# Build the external containerd shim from the same pinned native source.
set -euo pipefail
space_source=${1:?Provide the pinned SPACE checkout.}
space_output=${2:?Provide the output binary under scratch.}
space_go=${SPACE_GO:?Provide the verified Go 1.25.0 binary.}
space_go_cache=${SPACE_GO_CACHE:?Provide the persistent agent Go cache.}
[[ $(git -C "$space_source" rev-parse HEAD) == de87b436c35dac056564bdd5b2a113c9046e189b ]]
[[ $("$space_go" version) == 'go version go1.25.0 linux/amd64' ]]
git -C "$space_source" diff --quiet HEAD -- shim
space_output=$(realpath -m "$space_output")
# Ask for the base snapshot on creates; the node falls back to cold (#12145).
git -C "$space_source" apply "$(dirname "$(realpath "$0")")/shim-boot-mode.patch"
trap 'git -C "$space_source" apply -R "$(dirname "$(realpath "$0")")/shim-boot-mode.patch"' EXIT
cd "$space_source/shim"
CGO_ENABLED=0 GOTOOLCHAIN=local GOCACHE="$space_go_cache" \
  "$space_go" build -mod=readonly -trimpath -o "$space_output" .
"$space_go" version -m "$space_output" | grep -q 'vcs.revision=de87b436c35dac056564bdd5b2a113c9046e189b'
"$space_go" version -m "$space_output" | grep -q 'vcs.modified=true'
sha256sum "$space_output"

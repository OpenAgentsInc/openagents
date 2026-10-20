#!/usr/bin/env bash
# Build the private Rust LNURL backend from the SDK revision our wallet uses.
set -euo pipefail
revision=2c140a5d721f04d21c5ac334fecc3efe7c069943
repo="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)"
scratch="${OPENAGENTS_SCRATCH:-$(openagents scratch)}"
checkout="$scratch/breez-lnurl-$revision"
if [[ ! -d "$checkout/.git" ]]; then
  git clone --no-checkout https://github.com/breez/spark-sdk.git "$checkout"
fi
git -C "$checkout" fetch origin "$revision"
git -C "$checkout" checkout --detach "$revision"
[[ "$(git -C "$checkout" rev-parse HEAD)" == "$revision" ]]
[[ -z "$(git -C "$checkout" status --porcelain)" ]]
patch="$repo/deploy/lightning-address/nip57-raw-request.patch"
git -C "$checkout" apply --unidiff-zero --check "$patch"
git -C "$checkout" apply --unidiff-zero "$patch"
trap 'git -C "$checkout" apply --unidiff-zero -R "$patch"' EXIT
docker build --label "org.opencontainers.image.revision=$revision" \
  -t "openagents-lnurl-core:$revision" -f "$checkout/crates/breez-sdk/lnurl/Dockerfile" "$checkout"
docker build --label "org.opencontainers.image.revision=$revision" \
  --build-arg "BASE_IMAGE=openagents-lnurl-core:$revision" \
  -t "openagents-lnurl:$revision" -f "$repo/deploy/lightning-address/Dockerfile" "$checkout"

#!/bin/bash
# One deploy image build on Cloud Build with a registry layer cache (#12177):
#
#   scripts/deploy/cloud-buildx.sh DOCKERFILE IMAGE CACHE [docker build flags...]
#
# Run from the build context's root inside a gcr.io/cloud-builders/docker
# step. Cloud Build starts every build on a fresh machine, so BuildKit's own
# cache is empty each time; this keeps the Dockerfile's dependency layers
# (its `deps` target, see scripts/deploy/cargo-skeleton.sh) in
# us-central1-docker.pkg.dev/openagentsgemini/build-cache/CACHE:deps, a
# repository with mutable tags whose old untagged layers expire after a week.
#
# The `deps` target builds against that cache first. Its layers are then
# exported back (mode=max, so the intermediate stages are kept, not only the
# last one) while the full image builds beside the export on the same
# builder, reusing them, and is pushed to IMAGE. Only the dependency layers are exported, never the
# source or the compiled workspace crates, which change on every commit. A
# cache that can't be read or written makes the build slower, never fail.
set -euo pipefail

dockerfile=${1:?DOCKERFILE}
image=${2:?IMAGE}
cache=us-central1-docker.pkg.dev/openagentsgemini/build-cache/${3:?CACHE}:deps
shift 3

# The builder runs on Cloud Build's network, and its RUN steps that ask for
# the host network reach the metadata server: scripts/deploy/oa-kache.sh
# reads the build account's credentials there for the shared compile cache.
docker buildx create --name oa-deploy --driver docker-container \
    --driver-opt network=cloudbuild \
    --buildkitd-flags '--allow-insecure-entitlement network.host' --use > /dev/null
started=$(date +%s)
# First the dependency layers, from the registry cache or compiled. The
# image's stages start from them, so nothing waits that wouldn't anyway.
first=$(mktemp)
docker buildx build --progress=plain --allow network.host -f "$dockerfile" --target deps \
    --cache-from "type=registry,ref=$cache" . 2>&1 | tee "$first"
echo "cloud-buildx: dependency layers took $(( $(date +%s) - started )) s"
# Every dependency layer came from the registry: nothing to export. (An
# export beside the image build then also kept the image build from using
# the registry's layers, so it compiled them again, #12427.) A compile in
# the dependency stages prints oa-kache's upload line.
export_cache=1
if ! grep -q 'oa-kache: uploads finished' "$first"; then export_cache=; fi
rm -f "$first"
# Then the export, in the background while the image builds: every layer it
# names is now local, so it only compresses and uploads. Compressing the
# multi-gigabyte target directories took 400 to 800 s with gzip before the
# image could start (#12417); zstd is several times faster, and the image
# no longer waits for it. (Started together with the dependency build, the
# export raced it and sometimes left the compiled layers out.)
log=$(mktemp)
deps=
if [ -n "$export_cache" ]; then
    docker buildx build --progress=plain --allow network.host -f "$dockerfile" --target deps \
        --cache-from "type=registry,ref=$cache" \
        --cache-to "type=registry,ref=$cache,mode=max,image-manifest=true,oci-mediatypes=true,compression=zstd,ignore-error=true" \
        . > "$log" 2>&1 &
    deps=$!
fi
docker buildx build --progress=plain --allow network.host -f "$dockerfile" \
    --cache-from "type=registry,ref=$cache" \
    --provenance=false --sbom=false "$@" -t "$image" --push .
echo "cloud-buildx: the image took $(( $(date +%s) - started )) s"
# The cache export never fails the build: a stale cache only makes the next
# build slower.
if [ -z "$deps" ]; then
    echo "cloud-buildx: the dependency layers came from the registry cache; nothing to export"
elif wait "$deps"; then
    grep 'preparing build cache for export .*done' "$log" || true
    echo "cloud-buildx: the dependency cache export finished $(( $(date +%s) - started )) s after the start"
else
    sed 's/^/deps: /' "$log" | tail -n 40
    echo "cloud-buildx: the dependency cache was not exported (above); the image is pushed"
fi
rm -f "$log"

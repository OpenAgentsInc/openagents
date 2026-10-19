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
# First the `deps` target builds against that cache and exports its layers
# back (mode=max, so the intermediate stages are kept, not only the last
# one). The full image then builds on the same builder, reusing them, and is
# pushed to IMAGE. Only the dependency layers are exported, never the
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
docker buildx build --progress=plain --allow network.host -f "$dockerfile" --target deps \
    --cache-from "type=registry,ref=$cache" \
    --cache-to "type=registry,ref=$cache,mode=max,image-manifest=true,oci-mediatypes=true,ignore-error=true" \
    .
echo "cloud-buildx: dependency layers took $(( $(date +%s) - started )) s"
started=$(date +%s)
docker buildx build --progress=plain --allow network.host -f "$dockerfile" \
    --cache-from "type=registry,ref=$cache" \
    --provenance=false --sbom=false "$@" -t "$image" --push .
echo "cloud-buildx: the image took $(( $(date +%s) - started )) s"

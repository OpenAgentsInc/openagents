#!/usr/bin/env bash
# Build a digest-pinned research image from one committed source tree.
set -euo pipefail
ref=${1:?use REF OUTPUT_DIRECTORY}
output=${2:?use REF OUTPUT_DIRECTORY}
root=$(git rev-parse --show-toplevel)
commit=$(git -C "$root" rev-parse "$ref^{commit}")
epoch=$(git -C "$root" show -s --format=%ct "$commit")
example=${LAYERTREE_EXAMPLE:-attestation_probe}
recipe=deploy/layertree/reproducible-probe.Dockerfile
case "$example" in
  attestation_probe) features=events ;;
  key_lifecycle_probe) features=gcp-sealing ;;
  signet_service) features=events; recipe=deploy/layertree/signet-service.Dockerfile ;;
  *) echo 'Use a supported research example.' >&2; exit 2 ;;
esac
mkdir -p "$output"
output=$(cd "$output" && pwd)
[[ ! -e $output/build.json && ! -e $output/image.oci.tar ]] || { echo 'Use an empty output directory.' >&2; exit 2; }
context=$output/context
mkdir "$context"
git -C "$root" archive "$commit" crates/layertree-tee-lab "$recipe" | tar -x -C "$context"
# Buildx's Docker client also works with the builder's older Docker daemon.
export DOCKER_API_VERSION=${DOCKER_API_VERSION:-$(docker version --format '{{.Server.APIVersion}}')}
builder=${LAYERTREE_BUILDER:-oa-layertree-repro-agent2}
buildkit=moby/buildkit@sha256:98cc6a3fc46220d00f8224ae483f3274fc874e9be8d7dd1e2e2c5481209228b5
if docker buildx inspect "$builder" > "$output/builder-inspect.txt" 2>/dev/null; then
    grep -F "image=\"$buildkit\"" "$output/builder-inspect.txt" >/dev/null || { echo 'The builder must use the pinned BuildKit image.' >&2; exit 2; }
else
    docker buildx create --name "$builder" --driver docker-container --driver-opt "image=$buildkit" > "$output/builder-name.txt"
fi
args=()
if [[ -n ${LAYERTREE_IMAGE_TAG:-} ]]; then
    [[ $LAYERTREE_IMAGE_TAG == us-central1-docker.pkg.dev/openagentsgemini/layertree-research/attestation-probe:* ]] || { echo 'Use the research registry.' >&2; exit 2; }
    args+=(--tag "$LAYERTREE_IMAGE_TAG" --output type=image,push=true,rewrite-timestamp=true,oci-mediatypes=true)
fi
docker buildx build --builder "$builder" --platform linux/amd64 --provenance=false --sbom=false \
    --build-arg "SOURCE_DATE_EPOCH=$epoch" --build-arg "SOURCE_COMMIT=$commit" \
    --build-arg "PROBE_EXAMPLE=$example" --build-arg "PROBE_FEATURES=$features" \
    --file "$context/$recipe" \
    --output "type=oci,dest=$output/image.oci.tar,rewrite-timestamp=true,oci-mediatypes=true" \
    --metadata-file "$output/buildkit-metadata.json" "${args[@]}" "$context" > "$output/build.log" 2>&1
python3 - "$output" "$commit" "$epoch" "$example" "$recipe" <<'PY'
import hashlib,json,sys,tarfile
from pathlib import Path
out=Path(sys.argv[1])
with tarfile.open(out/'image.oci.tar') as tar:
    index=json.load(tar.extractfile('index.json'))
    images=[x for x in index['manifests'] if x.get('platform',{}).get('os')!='unknown']
    assert len(images)==1
    digest=images[0]['digest']
    body=tar.extractfile('blobs/sha256/'+digest.removeprefix('sha256:')).read()
    assert 'sha256:'+hashlib.sha256(body).hexdigest()==digest
result={'schema':'openagents.layertree.image-build.v1','source_commit':sys.argv[2],
        'source_date_epoch':int(sys.argv[3]),'image_digest':digest,'probe_example':sys.argv[4],
        'recipe':sys.argv[5],
        'signing_service':sys.argv[4]=='signet_service'}
(out/'build.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
PY

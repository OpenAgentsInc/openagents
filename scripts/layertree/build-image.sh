#!/usr/bin/env bash
# Build a digest-pinned research image from one committed source tree.
set -euo pipefail
ref=${1:?use REF OUTPUT_DIRECTORY}
output=${2:?use REF OUTPUT_DIRECTORY}
root=$(git rev-parse --show-toplevel)
commit=$(git -C "$root" rev-parse "$ref^{commit}")
epoch=$(git -C "$root" show -s --format=%ct "$commit")
mkdir -p "$output"
output=$(cd "$output" && pwd)
[[ ! -e $output/build.json && ! -e $output/image.oci.tar ]] || { echo 'Use an empty output directory.' >&2; exit 2; }
context=$output/context
mkdir "$context"
git -C "$root" archive "$commit" crates/layertree-tee-lab deploy/layertree/reproducible-probe.Dockerfile | tar -x -C "$context"
builder=${LAYERTREE_BUILDER:-oa-layertree-repro-agent2}
buildkit=moby/buildkit@sha256:98cc6a3fc46220d00f8224ae483f3274fc874e9be8d7dd1e2e2c5481209228b5
if docker buildx inspect "$builder" > "$output/builder-inspect.txt" 2>/dev/null; then
    docker buildx inspect "$builder" --format '{{json .DriverOptions}}' | grep -F "$buildkit" >/dev/null || { echo 'The builder must use the pinned BuildKit image.' >&2; exit 2; }
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
    --file "$context/deploy/layertree/reproducible-probe.Dockerfile" \
    --output "type=oci,dest=$output/image.oci.tar,rewrite-timestamp=true,oci-mediatypes=true" \
    --metadata-file "$output/buildkit-metadata.json" "${args[@]}" "$context" > "$output/build.log" 2>&1
python3 - "$output" "$commit" "$epoch" <<'PY'
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
        'source_date_epoch':int(sys.argv[3]),'image_digest':digest,
        'recipe':'deploy/layertree/reproducible-probe.Dockerfile',
        'signing_service':False}
(out/'build.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result))
PY

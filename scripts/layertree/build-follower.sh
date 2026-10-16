#!/usr/bin/env bash
# Build the standalone reader from committed sources, outside the signer image.
set -euo pipefail
ref=${1:?use REF OUTPUT_DIRECTORY}
out=${2:?use REF OUTPUT_DIRECTORY}
: "${CARGO_TARGET_DIR:?Set the long-lived target directory outside the checkout.}"
root=$(git rev-parse --show-toplevel)
commit=$(git -C "$root" rev-parse "$ref^{commit}")
image=rust@sha256:39f68a3e8e3ff425f8945ffa91128e60ff930d53e17fbb5214e95824bdd46f1b
mkdir -p "$out" "$CARGO_TARGET_DIR"
out=$(cd "$out" && pwd)
target=$(cd "$CARGO_TARGET_DIR" && pwd)
[[ ! -e $out/source && ! -e $out/build.json ]] || { echo 'Choose a new output directory.' >&2; exit 2; }
mkdir "$out/source"
git -C "$root" archive "$commit" Cargo.toml rust-toolchain.toml crates/layertree-tee-lab crates/nostr crates/nostr-transport nips/openagents/schemas scripts/layertree/followers.py | tar -x -C "$out/source"
# Path dependencies inherit only these package/lint fields. Keep an isolated
# parent workspace so unrelated packages and vendor patches are not required.
python3 - "$out/source/Cargo.toml" <<'PY'
from pathlib import Path
import sys
p=Path(sys.argv[1]); source=p.read_text(); keep=[]; active=False
for line in source.splitlines():
    if line.startswith('['):
        active=line.startswith(('[workspace.package]', '[workspace.lints.'))
    if active:
        keep.append(line)
p.write_text('[workspace]\nmembers=["crates/nostr","crates/nostr-transport"]\nexclude=["crates/layertree-tee-lab"]\nresolver="2"\n\n'+'\n'.join(keep)+'\n')
PY
cache=${LAYERTREE_FOLLOW_CARGO_CACHE:-$(dirname "$target")/layertree-follow-cargo-agent3}
mkdir -p "$cache"
cache=$(cd "$cache" && pwd)
features=follow-live
bins=(--bin layertree-follow)
if [[ ${LAYERTREE_STAGE_ATTACKS:-0} == 1 ]]; then
  features=stage-attacks
  bins+=(--bin layertree-stage)
fi
docker run --rm --user "$(id -u):$(id -g)" \
  -v "$out/source:/source:ro" -v "$target:/target" -v "$cache:/cargo" \
  -e CARGO_HOME=/cargo -e CARGO_TARGET_DIR=/target -e CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}" \
  -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_RELEASE_STRIP=symbols \
  -w /source "$image" cargo build --locked --release \
  --manifest-path crates/layertree-tee-lab/Cargo.toml --features "$features" "${bins[@]}" \
  > "$out/build.log" 2>&1
cp "$target/release/layertree-follow" "$out/layertree-follow"
if [[ $features == stage-attacks ]]; then cp "$target/release/layertree-stage" "$out/layertree-stage"; fi
python3 - "$out" "$commit" "$image" <<'PY'
import hashlib,json,sys
from pathlib import Path
out=Path(sys.argv[1]); sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
files={str(p.relative_to(out/'source')):sha(p) for p in sorted((out/'source').rglob('*')) if p.is_file()}
(out/'build.json').write_text(json.dumps({'schema':'layertree.follower.build.v1','source_commit':sys.argv[2],
 'compiler_image':sys.argv[3],'binary_sha256':sha(out/'layertree-follow'),'stage_binary_sha256':sha(out/'layertree-stage') if (out/'layertree-stage').exists() else None,'source_sha256':files},indent=2)+'\n')
print(json.dumps({'source_commit':sys.argv[2],'binary_sha256':sha(out/'layertree-follow')}))
PY

#!/usr/bin/env bash
# Build the DeepSeek-V4.1 stage worker (`psionic-v41-stage --features cuda`,
# #12398) for linux/x86_64 on a GPU build host, and upload it for
# `pylon cloud up --worker`.
#
#   v41-stage-build.sh [build] [--host HOST] [--arch sm_NN] [--commit REV] [--dry-run|--no-upload]
#   v41-stage-build.sh test [--host HOST] [--arch sm_NN] [--commit REV]
#   v41-stage-build.sh pylon [--host HOST] [--commit REV]
#
# `build` (the default) builds the release binary for --arch (default
# sm_120, the RTX PRO 6000 of a G4 machine) and uploads it with the group
# manifests:
#
#   gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/sm_NN/psionic-v41-stage
#   gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/sm_NN/manifests/SHA256.json
#   gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/sm_NN/lib/libcudart.so.13 ...
#   gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/sm_NN/BUILD.json
#   gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/sm_NN/GENERATIONS.json
#
# BUILD.json (schema v2, #12407) is what a stage machine installs against
# (scripts/pylon/v41-stage-startup.sh): the worker's and every library's
# SHA-256, the architecture the fatbinary actually holds (read back with
# cuobjdump, and refused unless it is exactly --arch), the worker's own
# build-info and numerical profile, the group manifests by digest (taken
# from COMMIT, not the checkout), the commit and its tree, the base image's
# digest and the builder image's ID, rustc, and nvcc. The script prints the
# SHA-256 of BUILD.json, the bundle hash that covers the worker and every
# library (#12410); pass it to `pylon cloud up --worker-build` and to
# `psionic-v41-stage qualify --worker-build` to pin a group or a
# qualification run to this build, and to scripts/pylon/v41-build-check.py
# to check a download of it. GENERATIONS.json records the uploaded objects'
# generations, which name these exact bytes even if the path is rewritten.
#
# `build --no-upload` builds and checks the same way and prints BUILD.json
# instead of uploading, such as to check that changing --arch on one cache
# rebuilds the kernels.
#
# `test` runs the crate's tests with CUDA on the host's own GPU and uploads
# nothing. --arch defaults to the host GPU's, read before the build and
# passed explicitly, so a test build never depends on what the build script
# guesses.
#
# `pylon` builds the `pylon` binary from the root workspace in the same
# image, for a group head in the zone (`pylon cloud up --head stage0|vm
# --pylon URL`), and uploads it as
#
#   gs://openagents-psionic-models/bin/pylon/COMMIT/pylon
#
# The build runs on HOST (default coderos-4080) over ssh, inside the
# nvidia/cuda Ubuntu 24.04 image with CUDA 13.0, so the binary links the
# stage machines' glibc (common-cu129-ubuntu-2404). Psionic's kernels need
# CUDA 13's headers, and its backend opens libcudart.so.13 and
# libcublas.so.13, which that image lacks: the build uploads them in lib/,
# the stage machine installs them beside the worker, and its NVIDIA 580
# driver runs CUDA 13.0. Only the committed source of COMMIT
# (default HEAD) goes to the host: crates/psionic, crates/stage-link, and the
# root manifest it inherits from. Caches stay on the host under
# ~/work/agent-targets/openagents-crates-psionic-docker-cuda, with the source
# in its src/ folder: each build syncs the commit into that folder by
# checksum, so files that didn't change keep their times and Cargo rebuilds
# only the crates a commit touched. The container loads
# scripts/dev/cargo-build-env.sh, so its intermediate files are keyed by the
# compiler as on every other agent build, and the CUDA backend's build script
# recompiles its kernels when the architecture changes. A lock on the host
# runs one build at a time. The upload uses CLOUDSDK_CONFIG (default
# ~/work/.secrets/gcloud-sa-config).
set -euo pipefail

mode=build
host=coderos-4080
arch=
commit=HEAD
dry=0
upload=1
if [[ ${1:-} == build || ${1:-} == test || ${1:-} == pylon ]]; then
  mode=$1
  shift
fi
while [[ $# -gt 0 ]]; do
  case $1 in
    --host) host=$2; shift 2 ;;
    --arch) arch=$2; shift 2 ;;
    --commit) commit=$2; shift 2 ;;
    --dry-run) dry=1; shift ;;
    --no-upload) upload=0; shift ;;
    *) echo "unknown argument $1" >&2; exit 2 ;;
  esac
done

repo=$(git rev-parse --show-toplevel)
sha=$(git -C "$repo" rev-parse --verify "$commit^{commit}")
short=${sha:0:10}
image=nvidia/cuda:13.0.1-devel-ubuntu24.04
builder=openagents/psionic-cuda-build:13.0.1
bucket=gs://openagents-psionic-models/bin/psionic-v41-stage
remote_cache='~/work/agent-targets/openagents-crates-psionic-docker-cuda'
remote_src="~/work/v41-stage-build-$short-$$"
if [[ $mode == build && -z $arch ]]; then
  arch=sm_120
fi
case ${arch:-sm_89} in
  sm_[0-9]*) ;;
  *) echo "--arch takes sm_NN, such as sm_120" >&2; exit 2 ;;
esac
dest="$bucket/$short/$arch"
# The root workspace's source and target stay apart from Psionic's.
src_dir=src
paths=(crates/psionic crates/stage-link crates/pylon/placements Cargo.toml rust-toolchain.toml scripts/dev/cargo-build-env.sh)
if [[ $mode == pylon ]]; then
  arch=
  dest=gs://openagents-psionic-models/bin/pylon/$short
  src_dir=src-pylon
  # Crates read registry files from across the checkout at compile time,
  # so this sends everything but Psionic, the benchmark evidence, and
  # binary assets.
  paths=(. ':!crates/psionic' ':!bench' ':!assets' ':!bins')
fi
# The group manifests come from COMMIT, like the source, so a build of
# another commit never carries this checkout's placements.
placements=crates/pylon/placements/deepseek-v41
tree=$(git -C "$repo" rev-parse --verify "$sha^{tree}")

if [[ $dry == 1 ]]; then
  echo "commit  $sha"
  echo "host    $host ($image)"
  [[ $mode == pylon ]] && echo "mode    pylon" || echo "mode    $mode for ${arch:-the host GPU}"
  [[ $mode == build ]] && echo "upload  $dest/psionic-v41-stage"
  [[ $mode == pylon ]] && echo "upload  $dest/pylon"
  exit 0
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/v41-stage-build.XXXXXX")
cleanup() {
  rm -rf "$scratch"
  ssh "$host" "rm -rf $remote_src" 2>/dev/null || true
}
trap cleanup EXIT

echo "v41-stage-build: $mode of $short on $host"
# Absolute paths, so the quoted commands below need no tilde expansion.
rhome=$(ssh "$host" 'printf %s "$HOME"')
if [[ $mode == test && -z $arch ]]; then
  # The host GPU's architecture, explicit, so the build records what it
  # compiled for.
  cap=$(ssh "$host" "timeout 30 nvidia-smi --query-gpu=compute_cap --format=csv,noheader" |
    head -n 1 | tr -d '. \r')
  [[ $cap =~ ^[0-9]+$ ]] || { echo "can't read $host's GPU architecture" >&2; exit 1; }
  arch=sm_$cap
fi
remote_cache=${remote_cache/#\~/$rhome}
remote_src=${remote_src/#\~/$rhome}
git -C "$repo" archive --format=tar.gz -o "$scratch/src.tar.gz" "$sha" "${paths[@]}"
ssh "$host" "mkdir -p $remote_src/work $remote_cache/$src_dir"
scp -q "$scratch/src.tar.gz" "$host:$remote_src/src.tar.gz"
# `git archive` stamps every file with the commit's time, which would make
# Cargo rebuild every crate. Copy only files whose bytes changed into the
# persistent source folder (without -t, a copied file gets the current
# time), under the host's build lock.
lock="$remote_cache/build.lock"

# Runs inside the container: /src is the source, /cache the caches.
inner=$(cat <<'INNER'
set -euo pipefail
export RUSTUP_HOME=/cache/rustup CARGO_HOME=/cache/cargo PATH=/cache/cargo/bin:$PATH
if ! [ -x /cache/cargo/bin/rustup ]; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain none
fi
if [ "$MODE" = pylon ]; then
  cd /cache/src-pylon
  export CARGO_TARGET_DIR=/cache/target-pylon CARGO_BUILD_JOBS=4
  . /cache/src-pylon/scripts/dev/cargo-build-env.sh
  cargo build --locked --release -p pylon
  cp /cache/target-pylon/release/pylon /src/pylon
  rustc -V >/src/rustc.txt
  exit 0
fi
cd /cache/src/crates/psionic
export CARGO_TARGET_DIR=/cache/target CARGO_BUILD_JOBS=4
# Compiler-keyed intermediate files, as for every agent build (#12071).
. /cache/src/scripts/dev/cargo-build-env.sh
export NVCC=/usr/local/cuda/bin/nvcc
# Always explicit: psionic-backend-cuda rebuilds its kernels when these
# change, and records the one it used.
export CUDAARCHS=$ARCH PSI_CUDA_ARCH=$ARCH
# The CUDA libraries ride beside the worker, in lib/.
export RUSTFLAGS='-C link-arg=-Wl,-rpath,$ORIGIN/lib'

if [ "$MODE" = test ]; then
  PSIONIC_REQUIRE_CUDA=1 cargo test --locked -p psionic-backend-cuda --lib intermediate_prompt_chunks -- --test-threads=1
  PSIONIC_REQUIRE_CUDA=1 cargo test --locked -p psionic-v41-stage --features cuda -- --test-threads=1
  exit 0
fi
cargo build --locked --release -p psionic-v41-stage --features cuda
bin=/cache/target/release/psionic-v41-stage
"$bin" manifest --split 0-20,20-40 >/dev/null
# What the fatbinary holds, not what was asked for: exactly $ARCH.
cuobjdump --list-elf "$bin" | sed -n 's/.*\.\(sm_[0-9]*a\{0,1\}\)\..*/\1/p' | sort -u >/src/elf.txt
if [ "$(cat /src/elf.txt)" != "$ARCH" ]; then
  echo "the worker's kernels are for $(tr '\n' ' ' </src/elf.txt), not $ARCH" >&2
  exit 1
fi
"$bin" build-info >/src/build-info.json
cp "$bin" /src/psionic-v41-stage
mkdir -p /src/lib
for lib in libcudart.so.13 libcublas.so.13 libcublasLt.so.13; do
  cp -L "/usr/local/cuda/lib64/$lib" /src/lib/
done
rustc -V >/src/rustc.txt
nvcc --version | tail -n 1 >/src/nvcc.txt
INNER
)
# The build image adds curl for rustup to the CUDA image; the build runs as
# the host user, so every cached file stays the user's.
dockerfile="FROM $image
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates pkg-config && rm -rf /var/lib/apt/lists/*"
# shellcheck disable=SC2029
ssh "$host" "printf '%s\n' $(printf %q "$dockerfile") | docker build -q -t $builder - >/dev/null"
gpu=()
if [[ $mode == test ]]; then
  gpu=(--device nvidia.com/gpu=all)
fi
# The host's part and the container's part travel as files, so neither
# needs another layer of shell quoting.
printf '%s\n' "$inner" >"$scratch/inner.sh"
cat >"$scratch/host.sh" <<HOST
set -eu
tar -xzf $remote_src/src.tar.gz -C $remote_src/work
rsync -rlc --delete $remote_src/work/ $remote_cache/$src_dir/
docker run --rm ${gpu[*]} --user \$(id -u):\$(id -g) -e HOME=/cache/home \\
  -e MODE=$mode -e ARCH=$arch -v $remote_src:/src -v $remote_cache:/cache $builder \\
  bash /src/inner.sh
HOST
scp -q "$scratch/inner.sh" "$scratch/host.sh" "$host:$remote_src/"
# shellcheck disable=SC2029
ssh "$host" "flock $lock sh $remote_src/host.sh"
if [[ $mode == test ]]; then
  echo "v41-stage-build: tests passed on $host"
  exit 0
fi
export CLOUDSDK_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
# The images the build ran in, by content.
# shellcheck disable=SC2029
image_digest=$(ssh "$host" "docker image inspect --format '{{index .RepoDigests 0}}' $image")
# shellcheck disable=SC2029
builder_id=$(ssh "$host" "docker image inspect --format '{{.Id}}' $builder")
# Writes BUILD.json from the named files and values in the environment.
write_build() {
  python3 - "$@" <<'PY'
import hashlib, json, os, sys
out, kind = sys.argv[1], sys.argv[2]
env = os.environ
def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()
def text(path):
    return open(path).read().strip()
scratch = env["SCRATCH"]
build = {
    "commit": env["SHA"],
    "tree": env["TREE"],
    "source_paths": env["PATHS"].split(),
    "image": env["IMAGE"],
    "image_digest": env["IMAGE_DIGEST"],
    "builder_image_id": env["BUILDER_ID"],
    "rustc": text(f"{scratch}/rustc.txt"),
}
if kind == "pylon":
    build["schema"] = "openagents.pylon.build.v1"
    build["sha256"] = sha(f"{scratch}/pylon")
else:
    info = json.load(open(f"{scratch}/build-info.json"))
    if info.get("sha256") != sha(f"{scratch}/psionic-v41-stage"):
        sys.exit("the worker's build-info names another executable")
    if info.get("cuda_arch") != env["ARCH"]:
        sys.exit(f"the worker was compiled for {info.get('cuda_arch')}, not {env['ARCH']}")
    lib = f"{scratch}/lib"
    manifests = f"{scratch}/manifests"
    build.update({
        "schema": "openagents.psionic_v41_stage.build.v2",
        "arch": env["ARCH"],
        "embedded_arch": text(f"{scratch}/elf.txt").split(),
        "features": ["cuda"],
        "profile": info["profiles"]["cuda"],
        "build_info": info,
        "nvcc": text(f"{scratch}/nvcc.txt"),
        "sha256": info["sha256"],
        "libraries": {name: sha(f"{lib}/{name}") for name in sorted(os.listdir(lib))},
        "manifests": {
            name[:-5]: f"{env['PLACEMENTS']}/{env['PLACEMENT_' + name[:-5]]}"
            for name in sorted(os.listdir(manifests))
        },
    })
json.dump(build, open(out, "w"), indent=2, sort_keys=True)
open(out, "a").write("\n")
PY
}
export SCRATCH=$scratch SHA=$sha TREE=$tree PATHS="${paths[*]}" IMAGE=$image \
  IMAGE_DIGEST=$image_digest BUILDER_ID=$builder_id ARCH=$arch PLACEMENTS=$placements
# Records each uploaded object's generation, which names its exact bytes.
generations() {
  local url first=1
  printf '{\n'
  for url in "$@"; do
    [[ $first == 1 ]] || printf ',\n'
    first=0
    printf '  "%s": "%s"' "$url" \
      "$(gcloud storage objects describe "$url" --format='value(generation)')"
  done
  printf '\n}\n'
}
if [[ $mode == pylon ]]; then
  scp -q "$host:$remote_src/pylon" "$host:$remote_src/rustc.txt" "$scratch/"
  write_build "$scratch/BUILD.json" pylon
  gcloud storage cp -q "$scratch/pylon" "$scratch/BUILD.json" "$dest/"
  generations "$dest/pylon" "$dest/BUILD.json" >"$scratch/GENERATIONS.json"
  gcloud storage cp -q "$scratch/GENERATIONS.json" "$dest/"
  cat "$scratch/GENERATIONS.json"
  echo "v41-stage-build: uploaded $dest/pylon (sha256 $(shasum -a 256 "$scratch/pylon" | cut -d ' ' -f 1))"
  echo "pylon cloud up ... --head stage0 --pylon $dest/pylon --pylon-build $(shasum -a 256 "$scratch/BUILD.json" | cut -d ' ' -f 1)"
  exit 0
fi

scp -q "$host:$remote_src/psionic-v41-stage" "$host:$remote_src/rustc.txt" \
  "$host:$remote_src/nvcc.txt" "$host:$remote_src/elf.txt" \
  "$host:$remote_src/build-info.json" "$scratch/"
scp -q -r "$host:$remote_src/lib" "$scratch/"
mkdir -p "$scratch/manifests"
while IFS= read -r file; do
  [[ $file == *.json ]] || continue
  git -C "$repo" show "$sha:$placements/$file" >"$scratch/placement.json"
  digest=$(shasum -a 256 "$scratch/placement.json" | cut -d ' ' -f 1)
  mv "$scratch/placement.json" "$scratch/manifests/$digest.json"
  export "PLACEMENT_$digest=$file"
done < <(git -C "$repo" ls-tree --name-only "$sha:$placements")
write_build "$scratch/BUILD.json" worker
build_sha=$(shasum -a 256 "$scratch/BUILD.json" | cut -d ' ' -f 1)
# The bundle checks out as a stage machine will check it.
python3 "$repo/scripts/pylon/v41-build-check.py" build "$scratch" --pin "$build_sha" >/dev/null
if [[ $upload == 0 ]]; then
  cat "$scratch/BUILD.json"
  echo "v41-stage-build: built $arch (BUILD.json sha256 $build_sha); uploaded nothing"
  exit 0
fi
gcloud storage cp -q "$scratch/psionic-v41-stage" "$scratch/BUILD.json" "$dest/"
gcloud storage cp -q "$scratch"/manifests/*.json "$dest/manifests/"
gcloud storage cp -q "$scratch"/lib/* "$dest/lib/"
objects=("$dest/psionic-v41-stage" "$dest/BUILD.json")
for lib in "$scratch"/lib/*; do objects+=("$dest/lib/${lib##*/}"); done
generations "${objects[@]}" >"$scratch/GENERATIONS.json"
gcloud storage cp -q "$scratch/GENERATIONS.json" "$dest/"
cat "$scratch/GENERATIONS.json"
echo "v41-stage-build: uploaded $dest/psionic-v41-stage (BUILD.json sha256 $build_sha)"
echo "pylon cloud up ... --worker $dest/psionic-v41-stage --worker-build $build_sha"
echo "psionic-v41-stage qualify ... --worker-build $build_sha"

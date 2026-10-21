#!/usr/bin/env bash
# Build the DeepSeek-V4.1 stage worker (`psionic-v41-stage --features cuda`,
# #12398) for linux/x86_64 on a GPU build host, and upload it for
# `pylon cloud up --worker`.
#
#   v41-stage-build.sh [build] [--host HOST] [--arch sm_NN] [--commit REV] [--dry-run]
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
#
# `test` runs the crate's tests with CUDA on the host's own GPU (--arch
# defaults to the host's) and uploads nothing.
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
# only the crates a commit touched. A lock on the host runs one build at a
# time. The upload uses CLOUDSDK_CONFIG (default
# ~/work/.secrets/gcloud-sa-config).
set -euo pipefail

mode=build
host=coderos-4080
arch=
commit=HEAD
dry=0
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
paths=(crates/psionic crates/stage-link Cargo.toml rust-toolchain.toml)
if [[ $mode == pylon ]]; then
  arch=
  dest=gs://openagents-psionic-models/bin/pylon/$short
  src_dir=src-pylon
  # Crates read registry files from across the checkout at compile time,
  # so this sends everything but Psionic, the benchmark evidence, and
  # binary assets.
  paths=(. ':!crates/psionic' ':!bench' ':!assets' ':!bins')
fi

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
  CARGO_TARGET_DIR=/cache/target-pylon CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 \
    cargo build --locked --release -p pylon
  cp /cache/target-pylon/release/pylon /src/pylon
  rustc -V >/src/rustc.txt
  exit 0
fi
cd /cache/src/crates/psionic
export CARGO_TARGET_DIR=/cache/target CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4
export NVCC=/usr/local/cuda/bin/nvcc
if [ -n "$ARCH" ]; then
  export CUDAARCHS=$ARCH PSI_CUDA_ARCH=$ARCH
fi
# The CUDA libraries ride beside the worker, in lib/.
export RUSTFLAGS='-C link-arg=-Wl,-rpath,$ORIGIN/lib'

if [ "$MODE" = test ]; then
  PSIONIC_REQUIRE_CUDA=1 cargo test --locked -p psionic-v41-stage --features cuda -- --test-threads=1
  exit 0
fi
cargo build --locked --release -p psionic-v41-stage --features cuda
bin=/cache/target/release/psionic-v41-stage
"$bin" manifest --split 0-20,20-40 >/dev/null
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
if [[ $mode == pylon ]]; then
  scp -q "$host:$remote_src/pylon" "$scratch/"
  gcloud storage cp -q "$scratch/pylon" "$dest/pylon"
  echo "v41-stage-build: uploaded $dest/pylon (sha256 $(shasum -a 256 "$scratch/pylon" | cut -d ' ' -f 1))"
  echo "pylon cloud up ... --head stage0 --pylon $dest/pylon"
  exit 0
fi

scp -q "$host:$remote_src/psionic-v41-stage" "$host:$remote_src/rustc.txt" \
  "$host:$remote_src/nvcc.txt" "$scratch/"
scp -q -r "$host:$remote_src/lib" "$scratch/"
mkdir -p "$scratch/manifests"
for file in "$repo"/crates/pylon/placements/deepseek-v41/*.json; do
  digest=$(shasum -a 256 "$file" | cut -d ' ' -f 1)
  cp "$file" "$scratch/manifests/$digest.json"
done
binary_sha=$(shasum -a 256 "$scratch/psionic-v41-stage" | cut -d ' ' -f 1)
cat >"$scratch/BUILD.json" <<JSON
{
  "schema": "openagents.psionic_v41_stage.build.v1",
  "commit": "$sha",
  "arch": "$arch",
  "image": "$image",
  "features": ["cuda"],
  "rustc": "$(cat "$scratch/rustc.txt")",
  "nvcc": "$(cat "$scratch/nvcc.txt")",
  "sha256": "$binary_sha"
}
JSON
gcloud storage cp -q "$scratch/psionic-v41-stage" "$scratch/BUILD.json" "$dest/"
gcloud storage cp -q "$scratch"/manifests/*.json "$dest/manifests/"
gcloud storage cp -q "$scratch"/lib/* "$dest/lib/"
echo "v41-stage-build: uploaded $dest/psionic-v41-stage (sha256 $binary_sha)"
echo "pylon cloud up ... --worker $dest/psionic-v41-stage"

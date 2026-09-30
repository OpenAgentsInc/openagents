#!/usr/bin/env bash
# Build the Linux release reproducibly: the window, `coder`, and
# `microcoder` compiled and packaged (AppImage, .deb, .tar.gz, SHA256SUMS)
# by scripts/desktop/package-linux.sh inside a pinned Debian 11 Rust image,
# so any Linux computer with Docker gets the same bytes from the same
# commit.
#
# usage: scripts/desktop/build-linux-release.sh --out DIR [--work DIR]
#          [--jobs N]
#
#   --out DIR    where the packages go (created; must be empty or absent)
#   --work DIR   the Cargo home and target directory kept between runs
#                (default: $TMPDIR/openagents-linux-release); a fresh one
#                builds from nothing
#   --jobs N     Cargo's parallel jobs (default: Cargo's)
#
# What is pinned, and why the output repeats:
#
# - The image `rust:1.97.1-bullseye` by digest: the toolchain in
#   rust-toolchain.toml, and glibc 2.31, so the binaries run on Debian 11,
#   Ubuntu 20.04, and anything newer.
# - The AppImage type-2 runtime, release 20251108, by SHA-256. It is static
#   and needs only `fusermount` (FUSE 2 or 3) on the computer running it.
# - The source is this checkout's committed tree (`git archive HEAD`),
#   copied to /src; uncommitted changes are refused. Paths are the same in
#   every run (/src, /cargo, /target) and are remapped out of the binaries.
# - SOURCE_DATE_EPOCH is the commit's time, so every packaged file carries
#   it (see package-linux.sh).
#
# The container runs as the calling user; only the image layer that adds
# the packaging tools runs apt as root.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
image="rust:1.97.1-bullseye@sha256:02d78ca3f928195c2a907543de778adfd728ad7e2a24fdc6aef582b7c77842e0"
runtime_url="https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64"
runtime_sha256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"

out=""
work="${TMPDIR:-/tmp}/openagents-linux-release"
jobs=""
while (($#)); do
  case "$1" in
    --out) out="$2"; shift 2 ;;
    --work) work="$2"; shift 2 ;;
    --jobs) jobs="$2"; shift 2 ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) echo "build-linux-release: unknown argument $1" >&2; exit 2 ;;
  esac
done
[[ -n "$out" ]] || { echo "build-linux-release: --out is required" >&2; exit 2; }
[[ "$(uname -s)" == Linux && "$(uname -m)" == x86_64 ]] \
  || { echo "build-linux-release: run this on x86_64 Linux with Docker" >&2; exit 1; }
[[ -z "$jobs" || "$jobs" =~ ^[0-9]+$ ]] || { echo "build-linux-release: bad --jobs" >&2; exit 2; }
if [[ -n "$(git -C "$root" status --porcelain --untracked-files=no)" ]]; then
  echo "build-linux-release: commit or set aside local changes first; the build uses HEAD" >&2
  exit 1
fi
if [[ -d "$out" && -n "$(ls -A "$out")" ]]; then
  echo "build-linux-release: $out is not empty" >&2
  exit 1
fi

commit="$(git -C "$root" rev-parse HEAD)"
epoch="$(git -C "$root" log -1 --format=%ct HEAD)"
mkdir -p "$out" "$work/cargo" "$work/target" "$work/src"
out="$(cd "$out" && pwd)"
work="$(cd "$work" && pwd)"

# The committed tree only, in a clean folder.
rm -rf "$work/src"
mkdir -p "$work/src"
git -C "$root" archive --format=tar HEAD | tar -x -C "$work/src"

runtime="$work/runtime-x86_64"
if [[ ! -f "$runtime" ]] || [[ "$(sha256sum "$runtime" | cut -d' ' -f1)" != "$runtime_sha256" ]]; then
  curl -fsSL "$runtime_url" -o "$runtime.part"
  mv "$runtime.part" "$runtime"
fi
[[ "$(sha256sum "$runtime" | cut -d' ' -f1)" == "$runtime_sha256" ]] \
  || { echo "build-linux-release: the AppImage runtime's SHA-256 is wrong" >&2; exit 1; }

# The pinned image plus the packaging tools.
tag="openagents-linux-release:${image##*:}"
tag="${tag:0:60}"
docker build -q -t "$tag" - >/dev/null <<EOF
FROM $image
RUN apt-get update \
 && apt-get install -y --no-install-recommends squashfs-tools dpkg-dev binutils file \
 && rm -rf /var/lib/apt/lists/*
EOF

remap="--remap-path-prefix=/src=/openagents --remap-path-prefix=/cargo=/cargo"
# Low priority: the container is not a child of this shell, so `nice`
# here would not reach it.
docker run --rm \
  --cpu-shares 64 \
  --user "$(id -u):$(id -g)" \
  -e HOME=/tmp \
  -e CARGO_HOME=/cargo \
  -e CARGO_TARGET_DIR=/target \
  -e CARGO_INCREMENTAL=0 \
  -e CARGO_BUILD_JOBS="${jobs:-}" \
  -e SOURCE_DATE_EPOCH="$epoch" \
  -e RUSTFLAGS="$remap" \
  -e DEB_MAINTAINER="${DEB_MAINTAINER:-}" \
  -v "$work/src:/src:ro" \
  -v "$work/cargo:/cargo" \
  -v "$work/target:/target" \
  -v "$runtime:/runtime:ro" \
  -v "$out:/out" \
  -w /src \
  "$tag" \
  bash -c '
    set -euo pipefail
    umask 022
    [[ -n "${CARGO_BUILD_JOBS:-}" ]] || unset CARGO_BUILD_JOBS
    [[ -n "${DEB_MAINTAINER:-}" ]] || unset DEB_MAINTAINER
    rustc --version
    nice -n 19 scripts/desktop/package-linux.sh --out /out --appimage-runtime /runtime
  '
printf '%s\n' "commit $commit" "source_date_epoch $epoch" "image $image" \
  "appimage_runtime $runtime_url $runtime_sha256" > "$out/BUILDINFO"
echo "build-linux-release: built $commit into $out"

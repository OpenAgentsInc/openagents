#!/usr/bin/env bash
# Build the DeepSeek-V4.1 stage worker for a Mac's GPU
# (`psionic-v41-stage --features metal`, #12420) and write its bundle.
#
#   v41-stage-build-metal.sh [--out DIR] [--upload]
#
# Builds the release worker from this checkout's committed state (the
# working tree must be clean), copies it to DIR (default
# ~/work/v41-metal-bundles/COMMIT), and writes DIR/BUILD.json, schema v2 with
# "backend": "metal": the worker's SHA-256, no libraries, and the SHA-256 of
# the Metal kernel source the worker compiles when a stage starts, with the
# commit, its tree, rustc, macOS, the GPU family, and the worker's
# build-info. The worker verifies the bundle at start and announces the
# SHA-256 of BUILD.json as its bundle hash, which a group entry can pin.
# --upload copies DIR to
#
#   gs://openagents-psionic-models/bin/psionic-v41-stage/COMMIT/metal/
#
# Run it on a Mac with Xcode's Metal toolchain; it uses the shared Cargo
# target and build settings (scripts/dev/cargo-build-env.sh).
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd -P)
out= upload=0
while [ $# -gt 0 ]; do
  case $1 in
    --out) out=$2; shift 2 ;;
    --upload) upload=1; shift ;;
    *) echo "usage: $0 [--out DIR] [--upload]" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] || { echo "a Metal worker builds on a Mac" >&2; exit 1; }
cd "$here"
if [ -n "$(git status --porcelain -- crates/psionic crates/stage-link)" ]; then
  echo "commit crates/psionic and crates/stage-link first: the bundle names a commit" >&2
  exit 1
fi
commit=$(git rev-parse HEAD)
short=$(git rev-parse --short=10 HEAD)
tree=$(git rev-parse HEAD^{tree})
out=${out:-$HOME/work/v41-metal-bundles/$short}
: "${CARGO_TARGET_DIR:=$HOME/work/agent-targets/openagents-crates-psionic}"
export CARGO_TARGET_DIR
cd crates/psionic
. "$here/scripts/dev/cargo-build-env.sh"
cargo build --locked --release -p psionic-v41-stage --features metal
mkdir -p "$out"
rm -f "$out/BUILD.json"
cp "$CARGO_TARGET_DIR/release/psionic-v41-stage" "$out/psionic-v41-stage"
info=$("$out/psionic-v41-stage" build-info)
python3 - "$out" "$commit" "$tree" "$info" <<'PY'
import hashlib, json, platform, subprocess, sys
out, commit, tree, info = sys.argv[1:5]
info = json.loads(info)
worker = hashlib.sha256(open(f"{out}/psionic-v41-stage", "rb").read()).hexdigest()
assert info["sha256"] == worker, "build-info names another executable"
assert info["metal_kernels_sha256"], "the worker has no Metal kernels"
record = {
    "schema": "openagents.psionic_v41_stage.build.v2",
    "backend": "metal",
    "sha256": worker,
    "libraries": {},
    "metal_kernels_sha256": info["metal_kernels_sha256"],
    "commit": commit,
    "tree": tree,
    "rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip().splitlines(),
    "macos": platform.mac_ver()[0],
    "machine": platform.machine(),
    "profiles": info.get("profiles"),
    "build_info": info,
}
open(f"{out}/BUILD.json", "w").write(json.dumps(record, indent=1, sort_keys=True) + "\n")
PY
bundle=$(shasum -a 256 "$out/BUILD.json" | cut -d' ' -f1)
"$out/psionic-v41-stage" build-info | python3 -c 'import json,sys; d=json.load(sys.stdin); assert d["bundle"] == "verified", d["bundle"]'
echo "v41-stage-build-metal: $out (BUILD.json sha256 $bundle)"
if [ "$upload" = 1 ]; then
  url="gs://openagents-psionic-models/bin/psionic-v41-stage/$short/metal"
  gcloud storage cp "$out/psionic-v41-stage" "$out/BUILD.json" "$url/"
  echo "uploaded $url (BUILD.json sha256 $bundle)"
fi

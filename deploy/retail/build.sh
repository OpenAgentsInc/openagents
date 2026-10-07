#!/usr/bin/env bash
# Build a clean current-main package under the retained target/build lease.
set -euo pipefail
root=$(git rev-parse --show-toplevel)
cd "$root"
if [ -n "$(git status --porcelain)" ]; then
  echo 'retail package requires a clean committed checkout' >&2
  exit 1
fi
git fetch origin main
if [ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]; then
  echo 'retail package requires current origin/main' >&2
  exit 1
fi
: "${CARGO_TARGET_DIR:?select the retained Cargo target directory}"
case "$CARGO_TARGET_DIR" in "$root"|"$root"/*) echo 'Cargo target must stay outside the checkout' >&2; exit 1;; esac
out=${1:?use an absolute new release directory}
case "$out" in /*) ;; *) echo 'release directory must be absolute' >&2; exit 1;; esac
if [ -e "$out" ]; then echo 'release directory already exists' >&2; exit 1; fi
export RETAIL_BUILD_COMMIT=$(git rev-parse HEAD)
export RETAIL_BUILD_TREE=clean
openagents lease build --keep-target-dir -- cargo build --locked --release \
  -p retail-service -p retail-qualify -p compute-workbench -p openagents-cli \
  --features compute-workbench/client \
  --bin retail-service --bin retail-qualify --bin retail-client --bin openagents
if [ "$(git rev-parse HEAD)" != "$RETAIL_BUILD_COMMIT" ] || [ -n "$(git status --porcelain)" ]; then
  echo 'retail source changed during the build; no package was published' >&2
  exit 1
fi
umask 077
mkdir -m 700 "$out"
for binary in retail-service retail-qualify retail-client openagents; do
  cp "$CARGO_TARGET_DIR/release/$binary" "$out/$binary"
done
python3 - "$out" "$RETAIL_BUILD_COMMIT" <<'PY'
import hashlib,json,sys
from pathlib import Path
root=Path(sys.argv[1])
files={p.name:'sha256:'+hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.iterdir())}
(root/'package.json').write_text(json.dumps({'schema':'openagents.cloud.retail-package.v1','commit':sys.argv[2],'tree':'clean','files':files,'activation':'closed until the exact runtime identity and native funded evidence are approved'},indent=2)+'\n')
PY
printf 'Retail package: %s\n' "$out"

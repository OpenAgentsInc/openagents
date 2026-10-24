#!/usr/bin/env bash
set -euo pipefail

# The web package is apps/aethyr; its Rust workspace is crates/aethyr.
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
monorepo="$(cd "$repo/../.." && pwd -P)"
crates="$monorepo/crates/aethyr"
cd "$crates"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(dirname "$monorepo")/aethyr-target}"
python3 - "$monorepo" "$CARGO_TARGET_DIR" <<'PY'
from pathlib import Path
import sys
root,target=map(lambda p:Path(p).resolve(),sys.argv[1:])
if target==root or root in target.parents:
    raise SystemExit('CARGO_TARGET_DIR must be outside the source checkout.')
PY
export CARGO_INCREMENTAL=0
export RUSTC="$(rustup which rustc)"
state_key="$(python3 - "$crates" "$RUSTC" <<'PY'
import hashlib,subprocess,sys
identity=sys.argv[1]+'\0'+sys.argv[2]+'\0'+subprocess.check_output([sys.argv[2],'-vV'],text=True)
print(hashlib.sha256(identity.encode()).hexdigest())
PY
)"
export CARGO_BUILD_BUILD_DIR="$CARGO_TARGET_DIR/worktrees/$state_key"
kernel_optimizer="${AETHYRFORGE_WASM_OPT:-wasm-opt}"
# Homebrew prints "wasm-opt version 132"; the GitHub release binary adds " (version_132)".
if ! optimizer_version="$("$kernel_optimizer" --version 2>/dev/null)" || [[ "$optimizer_version" != "wasm-opt version 132" && "$optimizer_version" != "wasm-opt version 132 (version_132)" ]]; then
  echo 'Build the browser kernel with Binaryen 132 (wasm-opt on PATH, or AETHYRFORGE_WASM_OPT).' >&2
  exit 1
fi
if [[ "${1:-}" == "--test" ]]; then
  cargo test --locked -p af-kernel
fi
kernel_artifact="$(cargo build --locked -p af-kernel --target wasm32-unknown-unknown --release --message-format=json-render-diagnostics | python3 -c '
import json,sys
artifact=None
for line in sys.stdin:
    try: row=json.loads(line)
    except json.JSONDecodeError: continue
    if row.get("reason")=="compiler-artifact" and row.get("target",{}).get("name")=="af_kernel":
        artifact=next((name for name in row.get("filenames",[]) if name.endswith(".wasm")),artifact)
if not artifact: raise SystemExit("Cargo did not report the kernel Wasm artifact.")
print(artifact)
')"
cd "$repo"
mkdir -p public
kernel_input="$CARGO_BUILD_BUILD_DIR/kernel-input-$$.wasm"
kernel_output="$CARGO_BUILD_BUILD_DIR/kernel-optimized-$$.wasm"
trap 'rm -f "$kernel_input" "$kernel_output"' EXIT
cp "$kernel_artifact" "$kernel_input"
"$kernel_optimizer" -O3 --strip-debug --strip-producers "$kernel_input" -o "$kernel_output"
cp "$kernel_output" public/kernel.wasm
python3 - <<'PY'
from pathlib import Path
path=Path('public/kernel.wasm')
print(f'Built {path}: {path.stat().st_size:,} bytes')
PY

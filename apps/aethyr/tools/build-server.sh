#!/usr/bin/env bash
set -euo pipefail
# The web package is apps/aethyr; its Rust workspace is crates/aethyr.
monorepo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd -P)"
repo="$monorepo/crates/aethyr"
cd "$repo"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(dirname "$monorepo")/aethyr-target}"
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RUSTC="$(rustup which rustc)"
state_key="$(python3 - "$repo" "$RUSTC" <<'PY'
import hashlib,subprocess,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();target=Path(__import__('os').environ['CARGO_TARGET_DIR']).resolve()
if target==root or root in target.parents:raise SystemExit('CARGO_TARGET_DIR must be outside the source checkout.')
identity=str(root)+'\0'+sys.argv[2]+'\0'+subprocess.check_output([sys.argv[2],'-vV'],text=True)
print(hashlib.sha256(identity.encode()).hexdigest())
PY
)"
export CARGO_BUILD_BUILD_DIR="$CARGO_TARGET_DIR/worktrees/$state_key"
cargo build --locked -p af-world-server --message-format=json-render-diagnostics | python3 -c '
import json,sys
artifact=None
for line in sys.stdin:
    try: row=json.loads(line)
    except json.JSONDecodeError: continue
    if row.get("reason")=="compiler-artifact" and row.get("target",{}).get("name")=="af-world-server":
        artifact=row.get("executable") or artifact
if not artifact: raise SystemExit("Cargo did not report the world server binary.")
print(artifact)
'

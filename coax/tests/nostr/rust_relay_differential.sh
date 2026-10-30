#!/usr/bin/env bash
# The R1 differential, end to end: builds the Coax relay (ASan+UBSan) outside
# the sandbox, then runs relay_rust_differential.py inside
# rust_relay_sandbox.sh's network-less sandbox next to the old Rust relay and
# its throwaway Postgres. Run `rust_relay_sandbox.sh all` once first.
#   coax/tests/nostr/rust_relay_differential.sh     # SCRATCH, NOBLE, BUN as in the two scripts
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
scratch="${SCRATCH:-${TMPDIR:-/tmp}/coax-rust-relay}"
mkdir -p "$scratch/coax"
scratch="$(cd "$scratch" && pwd -P)"
noble="${NOBLE:-/home/christopherdavid/oa-board/coax/web/project/node_modules}"
bun="$(readlink -f "$(command -v "${BUN:-bun}")")"
python="$(readlink -f "$(command -v python3)")"
[[ -x "$scratch/target/release/nostr-relay" ]] || { echo "build the Rust relay first: $here/rust_relay_sandbox.sh all" >&2; exit 1; }
"$root/coax/relay/build.sh" "$scratch/coax/relay" --sanitize >/dev/null
noble="$(cd "$noble" && pwd -P)"
RUN_BINDS="$noble" RUN_ENV="COAX_RELAY=$scratch/coax/relay BUN=$bun NOBLE=$noble" SCRATCH="$scratch" \
    exec "$here/rust_relay_sandbox.sh" run "$python" -I "$here/relay_rust_differential.py"

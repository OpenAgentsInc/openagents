#!/usr/bin/env bash
# Run coder-sync tests against an isolated disposable Postgres cluster.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
scratch=${OPENAGENTS_SCRATCH:-$(openagents scratch)}
pg_bin=${CODER_SYNC_POSTGRES_BIN:-$(pg_config --bindir)}
init_args=()
if [[ -n "${CODER_SYNC_POSTGRES_SHARE:-}" ]]; then init_args=(-L "$CODER_SYNC_POSTGRES_SHARE"); fi
state=$(mktemp -d "$scratch/coder-sync-test.XXXXXX")
port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')
cleanup() {
  "$pg_bin/pg_ctl" -D "$state/data" -m immediate stop >/dev/null 2>&1 || true
  rm -rf "$state"
}
trap cleanup EXIT
"$pg_bin/initdb" "${init_args[@]}" -D "$state/data" -U coder_sync_test -A trust --no-locale > "$state/init.log"
if ! "$pg_bin/pg_ctl" -D "$state/data" -l "$state/postgres.log" -o "-h 127.0.0.1 -p $port -k ''" -w start >/dev/null; then
  cat "$state/postgres.log" >&2
  exit 1
fi
export CODER_SYNC_TEST_DATABASE_URL="host=127.0.0.1 port=$port user=coder_sync_test dbname=postgres"
export RUSTUP_TOOLCHAIN=${RUSTUP_TOOLCHAIN:-1.98.1}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$HOME/work/openagents-target-agent1}
cd "$root"
cargo test --manifest-path deploy/coder-sync/Cargo.toml --features server "$@"

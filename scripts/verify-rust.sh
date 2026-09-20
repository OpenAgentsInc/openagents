#!/usr/bin/env bash
# Run the local Rust gate. Optional exclusions are printed as partial coverage.
set -euo pipefail
cd "$(dirname "$0")/.."
postgres=1
metal=0
soak=0
for argument in "$@"; do
  case "$argument" in
    --skip-postgres) postgres=0 ;;
    --with-metal) metal=1 ;;
    --with-soak) soak=1 ;;
    *) echo "Usage: $0 [--skip-postgres] [--with-metal] [--with-soak]" >&2; exit 64 ;;
  esac
done

features='kev/serve,lev/serve,gym/tui,jev/blocking'
echo 'Gate: Rust 1.97.1, rustfmt style edition 2024.'
python3 scripts/test_fetch_kev_artifacts.py
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked --workspace --all-targets --features "$features" -- -D warnings
cargo test --locked --workspace
cargo test --locked --workspace --features "$features"
cargo +1.95.0 check --locked --workspace --all-targets --features "$features"
cargo +1.94.0 check --locked -p kev --lib
if cargo +1.97.1 deny --version >/dev/null 2>&1; then
  ./scripts/check-dependencies.sh
else
  echo 'SKIPPED: dependency policy; cargo-deny is not installed, so this is a partial gate.'
fi

if (( postgres )); then
  ./scripts/test-postgres.sh
else
  echo 'SKIPPED: PostgreSQL acceptance; this is a partial gate.'
fi
if (( metal )); then
  cargo clippy --locked -p kev --all-targets --features serve,metal -- -D warnings
  cargo test --locked -p kev --features serve,metal
else
  echo 'SKIPPED: Metal checks; use --with-metal on a supported Apple host.'
fi
if (( soak )); then
  ./scripts/test-soak.sh
else
  echo 'SKIPPED: long-running relay soak; use --with-soak.'
fi
echo 'External model and paid live-door measurements require their documented separate runs.'
echo 'Only completed commands above constitute verification evidence.'

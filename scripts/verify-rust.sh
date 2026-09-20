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

phase() {
  local label=$1
  shift
  python3 scripts/run-verification-phase.py "$label" -- "$@"
}

features='kev/serve,lev/serve,gym/tui,jev/blocking'
echo 'Gate: Rust 1.97.1, rustfmt style edition 2024.'
phase "Artifact acquisition tests" python3 scripts/test_fetch_kev_artifacts.py
phase "Workspace formatting" cargo fmt --all --check
phase "Default workspace Clippy" cargo clippy --locked --workspace --all-targets -- -D warnings
phase "Feature workspace Clippy" cargo clippy --locked --workspace --all-targets --features "$features" -- -D warnings
phase "Default workspace tests" cargo test --locked --workspace -- --nocapture
phase "Feature workspace tests" cargo test --locked --workspace --features "$features" -- --nocapture
phase "Rust 1.95 workspace check" cargo +1.95.0 check --locked --workspace --all-targets --features "$features"
phase "Rust 1.94 Kev check" cargo +1.94.0 check --locked -p kev --lib
if cargo +1.97.1 deny --version >/dev/null 2>&1; then
  phase "Dependency policy" ./scripts/check-dependencies.sh
else
  echo 'SKIPPED: dependency policy; cargo-deny is not installed, so this is a partial gate.'
fi

if (( postgres )); then
  phase "PostgreSQL acceptance" ./scripts/test-postgres.sh
else
  echo 'SKIPPED: PostgreSQL acceptance; this is a partial gate.'
fi
if (( metal )); then
  phase "Metal Clippy" cargo clippy --locked -p kev --all-targets --features serve,metal -- -D warnings
  phase "Metal feature tests" cargo test --locked -p kev --features serve,metal -- --nocapture
else
  echo 'SKIPPED: Metal checks; use --with-metal on a supported Apple host.'
fi
if (( soak )); then
  phase "Relay soak" ./scripts/test-soak.sh
else
  echo 'SKIPPED: long-running relay soak; use --with-soak.'
fi
echo 'External model and paid live-door measurements require their documented separate runs.'
echo 'Only completed commands above constitute verification evidence.'

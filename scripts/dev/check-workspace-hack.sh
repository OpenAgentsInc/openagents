#!/usr/bin/env bash
# Check generated dependency features and workspace-hack dependency lines.
set -euo pipefail
cd "$(dirname "$0")/../.."

# Metadata checks compile nothing. Let each workspace's rust-toolchain.toml
# select Cargo and rustc instead of inheriting a compiler pinned for a build.
unset RUSTC CARGO_BUILD_RUSTC RUSTUP_TOOLCHAIN CARGO

hakari_python=""
for candidate in python3 python3.13 python3.12 python3.11; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import tomllib' >/dev/null 2>&1; then
    hakari_python=$candidate
    break
  fi
done
if [[ -z $hakari_python ]]; then
  echo 'Install Python 3.11 or newer to check workspace dependency targets.' >&2
  exit 1
fi

if [[ $(cargo hakari --version 2>/dev/null) != 'cargo-hakari 0.9.39' ]]; then
  echo 'Install cargo-hakari 0.9.39: cargo install cargo-hakari --version 0.9.39 --locked' >&2
  exit 1
fi

for workspace in . apps/openagents; do
  (
    cd "$workspace"
    echo "Checking workspace dependencies: $workspace"
    # Hakari has no --locked flag. Check the same all-features metadata
    # first so an outdated lockfile fails before Hakari can update it.
    cargo metadata --locked --all-features --format-version 1 >/dev/null
    cargo hakari generate --diff
    cargo hakari manage-deps --dry-run
    cargo hakari verify
  )
done

"$hakari_python" scripts/dev/workspace-hack-targets.py --check

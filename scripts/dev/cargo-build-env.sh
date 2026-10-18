#!/bin/sh
# Source after setting CARGO_TARGET_DIR, then run Cargo directly (#12071).
# The gate supplies oa_build_manifest; direct builds discover their workspace.
[ -n "${CARGO_TARGET_DIR:-}" ] || { echo 'Set CARGO_TARGET_DIR before loading the build settings.' >&2; return 1; }
oa_build_manifest=${oa_build_manifest:-$(cargo locate-project --workspace --message-format plain)} || return 1
oa_build_parent=$(CDPATH= cd -- "$(dirname -- "$oa_build_manifest")" && pwd -P) || return 1
oa_build_manifest=$oa_build_parent/$(basename -- "$oa_build_manifest")
if command -v shasum >/dev/null 2>&1; then
  oa_build_hash=$(printf '%s' "$oa_build_manifest" | shasum -a 256 | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  oa_build_hash=$(printf '%s' "$oa_build_manifest" | sha256sum | awk '{print $1}')
else
  echo 'Install shasum or sha256sum, then try the build again.' >&2
  return 1
fi
[ "${#oa_build_hash}" = 64 ] || return 1
CARGO_BUILD_BUILD_DIR=$CARGO_TARGET_DIR/worktrees/${oa_build_hash%"${oa_build_hash#??}"}/${oa_build_hash#??}
CARGO_INCREMENTAL=0
export CARGO_BUILD_BUILD_DIR CARGO_INCREMENTAL
mkdir -p "$CARGO_BUILD_BUILD_DIR" || return 1
printf '%s\n' "$oa_build_manifest" > "$CARGO_BUILD_BUILD_DIR/.workspace.$$.new" || return 1
mv -f "$CARGO_BUILD_BUILD_DIR/.workspace.$$.new" "$CARGO_BUILD_BUILD_DIR/.openagents-workspace" || return 1
unset oa_build_manifest oa_build_parent oa_build_hash

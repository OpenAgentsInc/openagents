#!/bin/sh
# Source after setting CARGO_TARGET_DIR, then run Cargo directly (#12071).
# The gate supplies oa_build_manifest; direct builds discover their workspace.
[ -n "${CARGO_TARGET_DIR:-}" ] || { echo 'Set CARGO_TARGET_DIR before loading the build settings.' >&2; return 1; }
oa_build_manifest=${oa_build_manifest:-$(cargo locate-project --workspace --message-format plain)} || return 1
oa_build_parent=$(CDPATH= cd -- "$(dirname -- "$oa_build_manifest")" && pwd -P) || return 1
oa_build_manifest=$oa_build_parent/$(basename -- "$oa_build_manifest")
# OPENAGENTS_NIGHTLY=1 opts a local dev build or test into the pinned nightly
# in scripts/dev/nightly-toolchain with the parallel front end (#12186).
# Releases, the land queue, the pool and Docker never set it. Its compiler
# version gives it build state of its own below, apart from stable's.
if [ "${OPENAGENTS_NIGHTLY:-}" = 1 ]; then
  oa_build_nightly=$(sed -n '1p' "$(dirname -- "$oa_build_manifest")/scripts/dev/nightly-toolchain" 2>/dev/null)
  [ -n "$oa_build_nightly" ] || oa_build_nightly=$(sed -n '1p' "$(dirname -- "$oa_build_manifest")/../../scripts/dev/nightly-toolchain" 2>/dev/null)
  oa_build_threads=${OPENAGENTS_NIGHTLY_THREADS:-8}
  # rustup which installs a missing toolchain, so ask the list first.
  if [ -n "$oa_build_nightly" ] && command -v rustup >/dev/null 2>&1 \
    && rustup toolchain list 2>/dev/null | grep -q "^$oa_build_nightly" \
    && oa_build_rustc=$(rustup which --toolchain "$oa_build_nightly" rustc 2>/dev/null); then
    RUSTC=$oa_build_rustc
    # RUSTFLAGS replaces the config's build.rustflags, so add to whichever
    # one Cargo reads.
    if [ -n "${RUSTFLAGS:-}" ]; then
      RUSTFLAGS="$RUSTFLAGS -Zthreads=$oa_build_threads"
      export RUSTFLAGS
    else
      CARGO_BUILD_RUSTFLAGS="${CARGO_BUILD_RUSTFLAGS:+$CARGO_BUILD_RUSTFLAGS }-Zthreads=$oa_build_threads"
      export CARGO_BUILD_RUSTFLAGS
    fi
  else
    echo "OPENAGENTS_NIGHTLY is set, but ${oa_build_nightly:-the pinned nightly} is not installed, so this build uses stable. Install it with: rustup toolchain install ${oa_build_nightly:-NIGHTLY} --profile minimal" >&2
  fi
fi
# A rustup proxy selects again in each dependency's working directory. Pin
# the compiler here, before Cargo changes directories (#12113).
oa_build_rustc=$(command -v "${RUSTC:-${CARGO_BUILD_RUSTC:-rustc}}") || { echo 'Cannot find the Rust compiler. Set RUSTC, then try again.' >&2; return 1; }
oa_build_parent=$(CDPATH= cd -- "$(dirname -- "$oa_build_rustc")" && pwd -P) || return 1
oa_build_rustc=$oa_build_parent/$(basename -- "$oa_build_rustc")
oa_build_rustup=$oa_build_parent/rustup
if [ -x "$oa_build_rustup" ] && [ "$oa_build_rustc" -ef "$oa_build_rustup" ]; then
  oa_build_rustc=$("$oa_build_rustup" which rustc) || return 1
fi
oa_build_version=$("$oa_build_rustc" -vV) || return 1
case $oa_build_version in
  'rustc '*) ;;
  *) echo 'Cannot read the Rust compiler version. Set RUSTC, then try again.' >&2; return 1 ;;
esac
RUSTC=$oa_build_rustc
export RUSTC
# Versioned keys also leave any old, incorrectly stamped fingerprints behind.
if command -v shasum >/dev/null 2>&1; then
  oa_build_hash=$(printf '%s\000%s\000%s\000%s' compiler-v1 "$oa_build_manifest" "$RUSTC" "$oa_build_version" | shasum -a 256 | awk '{print $1}')
elif command -v sha256sum >/dev/null 2>&1; then
  oa_build_hash=$(printf '%s\000%s\000%s\000%s' compiler-v1 "$oa_build_manifest" "$RUSTC" "$oa_build_version" | sha256sum | awk '{print $1}')
else
  echo 'Install shasum or sha256sum, then try the build again.' >&2
  return 1
fi
[ "${#oa_build_hash}" = 64 ] || return 1
CARGO_BUILD_BUILD_DIR=$CARGO_TARGET_DIR/worktrees/${oa_build_hash%"${oa_build_hash#??}"}/${oa_build_hash#??}
export CARGO_BUILD_BUILD_DIR
# The workspace profile decides which crates compile incrementally (#12178).
# An inherited CARGO_INCREMENTAL would override it for every crate: 0 turns
# the edit loop's incremental crates off, 1 takes every crate out of kache.
unset CARGO_INCREMENTAL
# The build directory's per-checkout hash would otherwise enter every key
# that names a build-script output (OUT_DIR, -L), so map it to one fixed root.
KACHE_BASE_DIR=$CARGO_BUILD_BUILD_DIR
export KACHE_BASE_DIR
# The shared remote compile cache (#12176) is set in kache's own config file
# (scripts/dev/remote-build-cache.sh). Builds through this helper stay local
# unless OA_BUILD_CACHE=remote: measured, the remote made debug builds slower
# and release builds faster (docs/coder/runtime/remote-build-cache.md). A
# missing key file also keeps a build local, and kache compiles locally on
# any remote error.
oa_build_kache=${KACHE_CONFIG:-${XDG_CONFIG_HOME:-$HOME/.config}/kache/config.toml}
if [ "${OA_BUILD_CACHE:-local}" != remote ]; then
  KACHE_LOCAL_ONLY=1
  export KACHE_LOCAL_ONLY
elif [ -r "$oa_build_kache" ]; then
  oa_build_key=$(sed -n 's/^# credentials: key //p' "$oa_build_kache" | tail -n 1)
  if [ -n "$oa_build_key" ] && [ ! -r "$oa_build_key" ]; then
    KACHE_LOCAL_ONLY=1
    export KACHE_LOCAL_ONLY
  fi
fi
mkdir -p "$CARGO_BUILD_BUILD_DIR" || return 1
case $(uname -s) in
  Darwin) touch "$CARGO_TARGET_DIR/.metadata_never_index" || return 1 ;;
esac
printf '%s\n%s\n' "$RUSTC" "$oa_build_version" > "$CARGO_BUILD_BUILD_DIR/.compiler.$$.new" || return 1
mv -f "$CARGO_BUILD_BUILD_DIR/.compiler.$$.new" "$CARGO_BUILD_BUILD_DIR/.openagents-compiler" || return 1
printf '%s\n' "$oa_build_manifest" > "$CARGO_BUILD_BUILD_DIR/.workspace.$$.new" || return 1
mv -f "$CARGO_BUILD_BUILD_DIR/.workspace.$$.new" "$CARGO_BUILD_BUILD_DIR/.openagents-workspace" || return 1
# Register custom targets so disk cleanup need not guess their folder names.
oa_build_target_path=$(CDPATH= cd -- "$CARGO_TARGET_DIR" && pwd -P) || return 1
if command -v shasum >/dev/null 2>&1; then
  oa_build_target_id=$(printf '%s' "$oa_build_target_path" | shasum -a 256 | awk '{print $1}')
else
  oa_build_target_id=$(printf '%s' "$oa_build_target_path" | sha256sum | awk '{print $1}')
fi
mkdir -p "$HOME/.openagents/build-targets" || return 1
printf '%s\n' "$oa_build_target_path" > "$HOME/.openagents/build-targets/.$oa_build_target_id.$$.new" || return 1
mv -f "$HOME/.openagents/build-targets/.$oa_build_target_id.$$.new" "$HOME/.openagents/build-targets/$oa_build_target_id" || return 1
unset oa_build_target_path oa_build_target_id
unset oa_build_nightly oa_build_threads oa_build_manifest oa_build_parent oa_build_hash oa_build_rustc oa_build_rustup oa_build_version oa_build_kache oa_build_key

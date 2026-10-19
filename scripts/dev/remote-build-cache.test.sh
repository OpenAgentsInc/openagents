#!/usr/bin/env bash
# Tests scripts/dev/remote-build-cache.sh and the remote-cache part of
# scripts/dev/cargo-build-env.sh in a fake HOME. Nothing reaches the network.
# Run: scripts/dev/remote-build-cache.test.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(cd "$(mktemp -d)" && pwd -P)
trap 'rm -rf "$tmp"' EXIT
script=$here/remote-build-cache.sh
export RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}
export HOME=$tmp/home XDG_CONFIG_HOME=$tmp/home/.config CARGO_HOME=$tmp/home/.cargo
unset KACHE_CONFIG CLOUDSDK_CONFIG RUSTC_WRAPPER KACHE_LOCAL_ONLY OA_BUILD_CACHE
cargo_bin=$(command -v cargo || true)
mkdir -p "$HOME/bin"
# A stand-in kache; systemctl is kept off PATH so no unit is touched.
printf '#!/bin/sh\necho "kache 0.28.1"\n' > "$HOME/bin/kache"
chmod +x "$HOME/bin/kache"
export KACHE=$HOME/bin/kache PATH=/usr/bin:/bin OA_BUILD_CACHE_SERVICE=0
cfg=$XDG_CONFIG_HOME/kache/config.toml

fail=0
check() { if eval "$2"; then :; else echo "FAIL: $1"; fail=1; fi; }

# No credentials: refused, nothing written.
check "refuses without credentials" "! OA_BUILD_CACHE_GCE=0 sh '$script' enable >/dev/null 2>&1"
check "nothing written without credentials" "[ ! -e '$cfg' ]"

# GCE identity: a new config gets the cap and the block.
OA_BUILD_CACHE_GCE=1 sh "$script" enable --local-max-size 8GiB >/dev/null
check "cap written" "grep -qx 'local_max_size = \"8GiB\"' '$cfg'"
check "remote written" "grep -qx 'bucket = \"openagentsgemini-rust-build-cache\"' '$cfg'"
check "gcs type" "grep -qx 'type = \"gcs\"' '$cfg'"
check "gce source" "grep -qx '# credentials: gce' '$cfg'"

# Idempotent, and existing settings stay.
printf 'auto_gc = true\n' > "$tmp/extra"
{ head -n 2 "$cfg"; cat "$tmp/extra"; tail -n +3 "$cfg"; } > "$cfg.x" && mv "$cfg.x" "$cfg"
OA_BUILD_CACHE_GCE=1 sh "$script" enable >/dev/null
check "one block after two enables" "[ \$(grep -c '^\[cache.remote\]' '$cfg') = 1 ]"
check "other settings kept" "grep -qx 'auto_gc = true' '$cfg'"
check "new config evicts shared" "grep -qx 'gc_evict_shared = true' '$cfg'"
check "cap kept" "grep -qx 'local_max_size = \"8GiB\"' '$cfg'"

# A key file is recorded; the helper goes local when it disappears.
printf '{}' > "$tmp/key.json"
OA_BUILD_CACHE_GCE=0 sh "$script" enable --key "$tmp/key.json" >/dev/null
check "key source" "grep -qx '# credentials: key $tmp/key.json' '$cfg'"
sh "$script" status > "$tmp/status"
check "status names the key" "grep -q 'credentials: key file $tmp/key.json' '$tmp/status'"

helper() {
  # Source the helper in a throwaway workspace and print KACHE_LOCAL_ONLY.
  ( cd "$tmp/ws" && CARGO_TARGET_DIR=$tmp/target PATH="$(dirname "$cargo_bin"):$PATH" \
      sh -c ". '$here/cargo-build-env.sh' && printf '%s' \"\${KACHE_LOCAL_ONLY:-remote}\"" )
}
if [ -n "$cargo_bin" ]; then
  mkdir -p "$tmp/ws/src"
  printf '[package]\nname = "ws"\nversion = "0.0.0"\nedition = "2021"\n' > "$tmp/ws/Cargo.toml"
  : > "$tmp/ws/src/lib.rs"
  check "helper stays local by default" "[ \"\$(helper)\" = 1 ]"
  check "OA_BUILD_CACHE=remote uses the remote" "[ \"\$(OA_BUILD_CACHE=remote helper)\" = remote ]"
  rm "$tmp/key.json"
  check "OA_BUILD_CACHE=remote goes local without the key" "[ \"\$(OA_BUILD_CACHE=remote helper)\" = 1 ]"
  printf '{}' > "$tmp/key.json"
else
  echo "skip: no cargo for the helper checks"
fi

# A foreign remote is never overwritten.
sh "$script" disable >/dev/null
check "disable removes the block" "! grep -q 'cache.remote' '$cfg'"
check "disable keeps settings" "grep -qx 'auto_gc = true' '$cfg'"
printf '[cache.remote]\ntype = "s3"\nbucket = "theirs"\n' >> "$cfg"
check "refuses a foreign remote" "! OA_BUILD_CACHE_GCE=1 sh '$script' enable >/dev/null 2>&1"
check "foreign remote kept" "grep -qx 'bucket = \"theirs\"' '$cfg'"

# --set-wrapper writes a Cargo config only when there is none.
OA_BUILD_CACHE_GCE=1 KACHE_CONFIG=$tmp/other.toml sh "$script" enable --set-wrapper >/dev/null
check "wrapper written" "grep -qx 'rustc-wrapper = \"$KACHE\"' '$CARGO_HOME/config.toml'"
printf '[build]\njobs = 2\n' > "$CARGO_HOME/config.toml"
check "wrapper refused over an existing config" "! OA_BUILD_CACHE_GCE=1 KACHE_CONFIG=$tmp/other.toml sh '$script' enable --set-wrapper >/dev/null 2>&1"

# install-kache refuses a download with the wrong checksum.
if [ "$(uname -s)" = Linux ]; then
  mkdir -p "$tmp/rel"
  printf 'not kache' > "$tmp/rel/kache"
  arch=$(uname -m); [ "$arch" = arm64 ] && arch=aarch64; [ "$arch" = amd64 ] && arch=x86_64
  tar -czf "$tmp/rel/kache-$arch-unknown-linux-musl.tar.gz" -C "$tmp/rel" kache
  check "bad checksum refused" "! OA_KACHE_URL=file://$tmp/rel sh '$script' install-kache --dir '$tmp/inst' >/dev/null 2>&1"
  check "nothing installed" "[ ! -e '$tmp/inst/kache' ]"
fi

[ "$fail" = 0 ] && echo "remote-build-cache: all checks passed"
exit "$fail"

#!/usr/bin/env bash
# Build the evidence guests to Wasm and pin them.
#
# For each guest (repo-map, code-search, and test-report) this script:
#
# 1. Builds `crates/plugin-<guest>` for wasm32-unknown-unknown with the
#    pinned toolchain and the workspace's `guest` profile.
# 2. Copies the module to `crates/plugin/fixtures/<guest>.wasm`.
# 3. Writes the build receipt `crates/plugin/fixtures/<guest>.receipt.json`:
#    the PDK source digest, the guest source digest, and the module digest.
# 4. Inlines the module into `programs/evidence-guests.json`, as the step's
#    `bytes_base64`, and pins its digest and size in the step's target.
#
# Paths are remapped so the bytes don't depend on where the checkout or the
# Cargo home is. Run it twice and the digests match.
#
# Prerequisites: rustup with the 1.97.1 toolchain and its
# wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown
# --toolchain 1.97.1`), jq, and sha256sum.
#
# Usage: ./scripts/build-plugin-guests.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
toolchain="1.97.1"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
fixtures="$root/crates/plugin/fixtures"
program="$root/programs/evidence-guests.json"
guests=(repo-map code-search test-report)

digest() {
  printf 'sha256:%s' "$(cat "$@" | sha256sum | cut -d' ' -f1)"
}

export RUSTFLAGS="--remap-path-prefix=$root=/openagents --remap-path-prefix=$cargo_home=/cargo"

packages=()
for guest in "${guests[@]}"; do
  packages+=(-p "plugin-$guest")
done
cargo "+$toolchain" build --locked --profile guest --target "$target" \
  --manifest-path "$root/Cargo.toml" "${packages[@]}"

pdk_digest="$(digest "$root/crates/plugin-pdk/src/lib.rs" "$root/crates/plugin-pdk/src/guest.rs")"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

for guest in "${guests[@]}"; do
  crate="plugin-$guest"
  built="$target_dir/$target/guest/${crate//-/_}.wasm"
  wasm="$fixtures/$guest.wasm"
  cp "$built" "$wasm"
  guest_digest="$(digest "$wasm")"
  size="$(wc -c < "$wasm" | tr -d ' ')"
  jq -n \
    --arg guest "$guest" \
    --arg crate "$crate" \
    --arg pdk "$pdk_digest" \
    --arg source "$(digest "$root/crates/$crate/Cargo.toml" "$root/crates/$crate/src/lib.rs")" \
    --arg module "$guest_digest" \
    --argjson size "$size" \
    --arg toolchain "$toolchain" \
    --arg target "$target" \
    '{
      schema: "openagents.plugin-build-receipt.v1",
      guest: $guest,
      crate: $crate,
      profile: "snapshot-read",
      pdk_digest: $pdk,
      source_digest: $source,
      guest_digest: $module,
      size: $size,
      toolchain: $toolchain,
      target: $target,
      cargo_profile: "guest",
      command: "./scripts/build-plugin-guests.sh"
    }' > "$fixtures/$guest.receipt.json"

  step="${guest//-/_}"
  base64 -w0 "$wasm" > "$scratch/$guest.b64"
  jq --indent 2 \
    --arg step "$step" \
    --arg module "$guest_digest" \
    --argjson size "$size" \
    --rawfile bytes "$scratch/$guest.b64" \
    '(.definition.steps[] | select(.name == $step) | .target.artifact) |= (.digest = $module | .size = $size)
     | .binding.steps[$step].module.bytes_base64 = $bytes' \
    "$program" > "$scratch/program.json"
  mv "$scratch/program.json" "$program"
  printf '%s %s %s bytes\n' "$guest" "$guest_digest" "$size"
done

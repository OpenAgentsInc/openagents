#!/usr/bin/env bash
# Build Wasm guests and pin them.
#
# The guests are the three evidence guests (repo-map, code-search, and
# test-report) and the example plugins' tools (explain-error,
# release-notes, dependency-check, and action-items). For each guest named on the
# command line, or every guest without names, this script:
#
# 1. Builds `crates/plugin-<guest>` for wasm32-unknown-unknown with the
#    pinned toolchain and the workspace's `guest` profile.
# 2. Copies the module to `crates/plugin/fixtures/<guest>.wasm`.
# 3. Writes the build receipt `crates/plugin/fixtures/<guest>.receipt.json`:
#    the PDK source digest, the guest source digest, and the module digest.
# 4. For an evidence guest, inlines the module into
#    `programs/evidence-guests.json`, as the step's `bytes_base64`, and pins
#    its digest and size in the step's target.
# 5. Does the same for the guest's catalog extension, the program under
#    `crates/plugin-<guest>/programs/` that the hosted eval runner and
#    `openagents ext eval` test, and restates that program's digest in the
#    extension's `package.json`. A guest that changes changes its test
#    set's subject, so release new results after rebuilding.
#
# Paths are remapped so the bytes don't depend on where the checkout or the
# Cargo home is: two builds on the same kind of machine give the same
# digests. They do depend on the build host's platform: the evidence
# guests' checked-in bytes came from a Linux x86_64 build, and a macOS arm64
# build of the same source gives other bytes. So rebuild only the guests
# you changed, by name, and their receipts and digests move while the
# others' stay as released.
#
# Prerequisites: rustup with the 1.97.1 toolchain and its
# wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown
# --toolchain 1.97.1`), jq, and sha256sum or shasum.
#
# Usage: ./scripts/build-plugin-guests.sh [GUEST...]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
toolchain="1.97.1"
target="wasm32-unknown-unknown"
target_dir="${CARGO_TARGET_DIR:-$root/target}"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
fixtures="$root/crates/plugin/fixtures"
program="$root/programs/evidence-guests.json"
evidence=(repo-map code-search test-report)
all=(repo-map code-search test-report explain-error release-notes dependency-check action-items)

if [ $# -gt 0 ]; then
  guests=("$@")
  for guest in "${guests[@]}"; do
    case " ${all[*]} " in
      *" $guest "*) ;;
      *) echo "unknown guest $guest; the guests are: ${all[*]}" >&2; exit 64 ;;
    esac
  done
else
  guests=("${all[@]}")
fi

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | cut -d' ' -f1
  else
    shasum -a 256 | cut -d' ' -f1
  fi
}

digest() {
  printf 'sha256:%s' "$(cat "$@" | sha256)"
}

# One line of base64, on GNU and BSD base64 alike.
encode() {
  base64 < "$1" | tr -d '\n'
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
  encode "$wasm" > "$scratch/$guest.b64"

  case " ${evidence[*]} " in
    *" $guest "*)
      jq --indent 2 \
        --arg step "$step" \
        --arg module "$guest_digest" \
        --argjson size "$size" \
        --rawfile bytes "$scratch/$guest.b64" \
        '(.definition.steps[] | select(.name == $step) | .target.artifact) |= (.digest = $module | .size = $size)
         | .binding.steps[$step].module.bytes_base64 = $bytes' \
        "$program" > "$scratch/program.json"
      mv "$scratch/program.json" "$program"
      ;;
  esac
  printf '%s %s %s bytes\n' "$guest" "$guest_digest" "$size"

  # The catalog extension: the program its package pins.
  crate_dir="$root/crates/$crate"
  name="$(jq -r .program.name "$crate_dir/package.json")"
  extension="$crate_dir/programs/$name.json"
  jq --indent 2 \
    --arg step "$step" \
    --arg module "$guest_digest" \
    --argjson size "$size" \
    --rawfile bytes "$scratch/$guest.b64" \
    '(.definition.steps[] | select(.name == $step) | .target.artifact) |= (.digest = $module | .size = $size)
     | .binding.steps[$step].module.bytes_base64 = $bytes' \
    "$extension" > "$scratch/extension.json"
  mv "$scratch/extension.json" "$extension"
  # A package states a file's digest as the SHA-256 of the file's text as
  # a JSON string (`coder::package::digest`).
  stated="$(jq -Rs . "$extension" | tr -d '\n' | sha256)"
  jq --indent 2 --arg digest "$stated" '.program.digest = $digest' "$crate_dir/package.json" > "$scratch/package.json"
  mv "$scratch/package.json" "$crate_dir/package.json"
  printf '%s extension %s\n' "$guest" "$stated"
done

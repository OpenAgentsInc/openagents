#!/usr/bin/env bash
# Assemble openagents.com's content tier for livehost (#12435): the files
# `openagents-web` reads through `live_content`, at the paths it reads them
# by, plus the design-language bundle. Deploy the result with
#   livehost deploy OUTDIR --tenant openagents --channel staging ...
#
# Usage: scripts/deploy/web-content.sh OUTDIR

set -euo pipefail

out="${1:?usage: web-content.sh OUTDIR}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
web="$root/crates/openagents-web"

rm -rf "$out"
mkdir -p "$out/static"
cp -R "$web/content" "$out/content"
cp "$web/static/fonts.css" "$web/static/legacy-demo.css" "$out/static/"
cargo run --quiet --locked --manifest-path "$root/Cargo.toml" -p openagents-ui \
  --bin openagents-ui-bundle -- "$out" "$root/crates/openagents-ui"

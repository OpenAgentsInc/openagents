#!/usr/bin/env bash
# Assemble openagents.com's content tier for livehost (#12435): the files
# `openagents-web` reads through `live_content`, at the paths it reads them
# by, the design-language bundle, and a site.json naming the route handler
# components in crates/openagents-web/components, which `livehost deploy`
# builds when their sources change. Deploy the result with
#   livehost deploy OUTDIR --tenant openagents --channel staging ...
#
# Usage: scripts/deploy/web-content.sh OUTDIR
#
# WEB_CONTENT_SOURCE names another tree to take the content from, such as
# a commit `scripts/deploy/web.sh stage` exported; the bundling tool still
# builds from this checkout, so its build stays warm (#12427).

set -euo pipefail

out="${1:?usage: web-content.sh OUTDIR}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_root="${WEB_CONTENT_SOURCE:-$root}"
web="$source_root/crates/openagents-web"

rm -rf "$out"
mkdir -p "$out/static"
cp -R "$web/content" "$out/content"
cp "$web/static/fonts.css" "$web/static/legacy-demo.css" "$out/static/"
cargo run --quiet --locked --manifest-path "$root/Cargo.toml" -p openagents-ui \
  --bin openagents-ui-bundle -- "$out" "$source_root/crates/openagents-ui"
cat > "$out/site.json" <<JSON
{
  "tenant": "openagents",
  "guests": "$web/components",
  "guest_inputs": ["$source_root/crates/livehost/wit", "$source_root/crates/pylons-view"],
  "handlers": {"pylons-list": "pylons-list"}
}
JSON

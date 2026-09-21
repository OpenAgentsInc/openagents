#!/usr/bin/env bash
# Fetches a vanilla Minecraft server jar into the version cache.
#
#   ./scripts/fetch-mc-server.sh 1.21.11
#
# Mojang publishes versions through a two-step manifest: one index names
# every release's metadata URL, and that metadata names the server jar's
# URL and SHA-1. The script resolves both, downloads the jar to
# `~/.openagents/voyager/minecraft/versions/<version>/server.jar`, and
# verifies the digest. Nothing here is trusted from the network alone —
# a jar that fails the digest is deleted, not kept.
set -euo pipefail

version="${1:-}"
if [ -z "$version" ]; then
  echo "usage: $0 <minecraft-version>" >&2
  exit 2
fi

cache="${VOYAGER_MC_DIR:-$HOME/.openagents/voyager/minecraft}"
dir="$cache/versions/$version"
jar="$dir/server.jar"

if [ -f "$jar" ]; then
  echo "already have $jar"
  exit 0
fi

for tool in curl python3 shasum; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "$tool is not on PATH" >&2
    exit 1
  fi
done

mkdir -p "$dir"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

index="$tmp/versions.json"
curl -fsSL -o "$index" \
  "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"

meta_url="$(python3 - "$index" "$version" <<'PY'
import json, sys
index = json.load(open(sys.argv[1]))
for entry in index["versions"]:
    if entry["id"] == sys.argv[2]:
        print(entry["url"])
        break
PY
)"
if [ -z "$meta_url" ]; then
  echo "version $version is not in Mojang's manifest" >&2
  exit 1
fi

meta="$tmp/meta.json"
curl -fsSL -o "$meta" "$meta_url"

read -r jar_url jar_sha1 < <(python3 - "$meta" <<'PY'
import json, sys
meta = json.load(open(sys.argv[1]))
server = meta["downloads"]["server"]
print(server["url"], server["sha1"])
PY
)

echo "fetching minecraft $version server"
curl -fsSL -o "$jar" "$jar_url"

actual="$(shasum -a 1 "$jar" | awk '{print $1}')"
if [ "$actual" != "$jar_sha1" ]; then
  echo "sha1 mismatch: manifest said $jar_sha1, got $actual" >&2
  rm -f "$jar"
  exit 1
fi

echo "fetched $jar (sha1 $actual)"

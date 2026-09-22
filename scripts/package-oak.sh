#!/bin/sh
# Package the caller binaries — `oak`, `oak-mcp`, `oak-mcp-http` — as a
# checksummed tarball with a provenance manifest.
#
# Usage: ./scripts/package-oak.sh [target-triple]
#
# With no argument the script packages the host target. Artifacts land in
# `dist/clients/`:
#
#   oak-<version>-<target>.tar.gz   the three binaries
#   oak-<version>-<target>.manifest.json   commit, toolchain, target
#   SHA256SUMS                      checksums for both artifacts
#
# Checksums are what an installer verifies. Signed provenance — a
# signature over SHA256SUMS by the release operator's key — is a release
# step layered on this output where the process supports it; this script
# does not sign.
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

target=${1:-$(rustc -vV | awk '/^host:/ {print $2}')}
version=$(awk -F'"' '/^version = / {print $2; exit}' Cargo.toml)
commit=$(git rev-parse --short=12 HEAD)
rustc_version=$(rustc --version)

out="dist/clients"
stage="$out/stage"
rm -rf "$stage"
mkdir -p "$stage"

echo "packaging oak $version for $target (commit $commit)"

if [ "$target" = "$(rustc -vV | awk '/^host:/ {print $2}')" ]; then
    cargo build --release --locked -p oak --features mcp-http \
        --bin oak --bin oak-mcp --bin oak-mcp-http
    bin_dir="${CARGO_TARGET_DIR:-target}/release"
else
    cargo build --release --locked -p oak --features mcp-http \
        --bin oak --bin oak-mcp --bin oak-mcp-http \
        --target "$target"
    bin_dir="${CARGO_TARGET_DIR:-target}/$target/release"
fi

for bin in oak oak-mcp oak-mcp-http; do
    cp "$bin_dir/$bin" "$stage/$bin"
done

name="oak-$version-$target"
tar -czf "$out/$name.tar.gz" -C "$stage" oak oak-mcp oak-mcp-http

cat >"$out/$name.manifest.json" <<EOF
{
 "package": "oak",
 "version": "$version",
 "target": "$target",
 "commit": "$commit",
 "rustc": "$rustc_version",
 "built_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
 "binaries": ["oak", "oak-mcp", "oak-mcp-http"]
}
EOF

(cd "$out" && shasum -a 256 "$name.tar.gz" "$name.manifest.json" >SHA256SUMS)

rm -rf "$stage"
echo "wrote $out/$name.tar.gz"
echo "wrote $out/$name.manifest.json"
echo "wrote $out/SHA256SUMS"

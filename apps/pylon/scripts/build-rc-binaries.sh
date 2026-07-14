#!/usr/bin/env bash
# Build signed, portable Node Pylon launchers. The same Vite Plus/tsdown-built
# ESM artifact runs on every supported Node 24 host; Bun native executables are
# intentionally no longer produced.
set -euo pipefail
cd "$(dirname "$0")/../../.."

VERSION="${1:-1.0.0-rc.2}"
CHANNEL="${OA_PYLON_CHANNEL:-rc}"
OUT="apps/pylon/dist/rc/$VERSION"
SIGNER="apps/oa-updates/scripts/sign-release.ts"
VERIFIER="apps/oa-updates/scripts/verify-release.ts"

SRC_VERSION="$(node --import tsx -e 'import { PYLON_VERSION } from "./apps/pylon/src/version.ts"; console.log(PYLON_VERSION)')"
PKG_VERSION="$(node -e 'console.log(require("./apps/pylon/package.json").version)')"
if [ "$SRC_VERSION" != "$VERSION" ] || [ "$PKG_VERSION" != "$VERSION" ]; then
  echo "FAIL: cutting $VERSION but source=$SRC_VERSION package=$PKG_VERSION" >&2
  exit 1
fi

node scripts/build-public-cli-artifacts.mjs --filter @openagentsinc/pylon
mkdir -p "$OUT"
platforms=(darwin-arm64 darwin-x64 linux-x64 linux-arm64)
for platform in "${platforms[@]}"; do
  artifact="$OUT/pylon-$platform"
  cp apps/pylon/dist/index.mjs "$artifact"
  chmod 0755 "$artifact"
  OPENAGENTS_RELEASE_SIGNED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    node --import tsx "$SIGNER" "$artifact" > "$artifact.sig.json"
  node --import tsx "$VERIFIER" "$artifact" "$artifact.sig.json"
done

node --input-type=module - "$OUT" "$VERSION" "$CHANNEL" "${platforms[@]}" <<'NODE'
import { readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"
const [out, version, channel, ...platforms] = process.argv.slice(2)
const records = Object.fromEntries(platforms.map((platform) => {
  const envelope = JSON.parse(readFileSync(join(out, `pylon-${platform}.sig.json`), "utf8"))
  return [platform, {
    file: `pylon-${platform}`,
    sha256: envelope.sha256,
    signature: envelope.signature,
    kid: envelope.kid,
    runtime: "node>=24.13.1",
  }]
}))
writeFileSync(join(out, "manifest.json"), JSON.stringify({
  schema: "openagents.pylon.release_manifest.v1",
  product: "pylon",
  version,
  channel,
  artifactKind: "portable-node-esm",
  platforms: records,
}, null, 2) + "\n")
NODE

echo "portable Node RC artifacts written to $OUT"

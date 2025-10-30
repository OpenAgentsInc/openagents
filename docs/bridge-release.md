# oa-bridge Release and Deployment

This document captures how we build and ship prebuilt `oa-bridge` binaries and align the `tricoder` NPX launcher to consume them.

Overview
- CI workflow: `.github/workflows/release-oa-bridge.yml` runs on annotated tags `v*`.
- Produced artifacts (per tag):
  - macOS: `oa-bridge-aarch64-apple-darwin.zip`, `oa-bridge-x86_64-apple-darwin.zip`
  - Linux: `oa-bridge-x86_64-unknown-linux-gnu.zip`, `oa-bridge-aarch64-unknown-linux-gnu.zip`
  - Windows: `oa-bridge-x86_64-pc-windows-msvc.zip`
- Linux arm64 builds use `cross` (Docker) to avoid apt multiarch mirror issues and OpenSSL headaches.
- Bridge crate avoids OpenSSL: `jsonschema` default features are disabled; `rusqlite` is bundled in `tinyvex`. No TLS system deps required for the bridge.

Release Steps
1) Ensure `main` is green.
2) Tag the repo:
   - `git tag vX.Y.Z && git push origin vX.Y.Z`
3) Watch GitHub Actions → “Release oa-bridge binaries”. All matrix jobs should turn green.
4) Verify assets on the release page match the expected names above.

Update tricoder (NPX launcher)
- The launcher looks for prebuilt release assets and falls back to `cargo run -p oa-bridge` if none are found.
- Minimum bridge version is set in `packages/tricoder/src/index.ts` (`TRICODER_MIN_BRIDGE`, defaults to `v0.2.2`). Update if needed.
- To publish tricoder as `latest` after a bridge release:
  1) `cd packages/tricoder`
  2) Bump `package.json` version (e.g., `0.2.2`).
  3) `npm publish` (requires NPM auth). This sets the default `npx tricoder@latest` to the new version.
  4) Optional: use `--tag next` for pre-releases (e.g., `-rc.N`).

Quick sanity test
- Prebuilt path: `TRICODER_BRIDGE_VERSION=vX.Y.Z npx tricoder --no-run --verbose`
  - Should print the resolved asset and not fall back to cargo.
- Cargo fallback: `TRICODER_PREFER_BIN=0 npx tricoder --run-bridge --verbose`

Troubleshooting
- Linux arm64 apt failures (404s for arm64 package lists): avoided by using `cross`. Do not add `dpkg --add-architecture arm64` to CI.
- OpenSSL not found: avoided by removing transitive `reqwest` via `jsonschema` and bundling SQLite; no OpenSSL needed.
- Windows sanity step: We skip bash-only sanity checks on Windows to avoid PowerShell parsing errors; Linux/macOS checks suffice.


#!/usr/bin/env bash
# build-agent-computer-rootfs.sh — source-controlled Agent Computer rootfs bake
# (CX-3 #8547 item 1; supersedes the hand-run debootstrap recipe recorded in
# docs/cloud/bootstrap/CND-056-cloud-vm-firecracker-provisioner.md).
#
# Produces the ext4 guest rootfs for the Firecracker Agent Computer with the
# full pinned runtime baked in:
#   - Ubuntu 22.04 (jammy) debootstrap base: git, python3, ca-certificates,
#     openssh-client
#   - Node ${NODE_VERSION} at /usr/local/bin/node
#   - seven pinned coding harnesses: codex, claude-code, cursor, goose,
#     opencode, pi, and grok
#   - the vsock guest agent (guest-agent.py, agent-guest.service enabled,
#     AF_VSOCK :1024)
#   - the Vite Plus packed turn-runner bundle at /opt/agent/turn-runner.bundle
#     with an executable link at /opt/agent/turn-runner
#     of apps/pylon/deploy/agent-computer/turn-runner.ts)
#   - the fixed PORT-03 retained-session controller bundle at
#     /opt/agent/portable-session-control.bundle with an executable link at
#     /opt/agent/portable-session-control (no arbitrary command surface)
#   - the signed TypeScript language server artifact at a fixed /opt/agent path
#     for the managed LSP profile; DAP stays unadmitted
#   - oa-workroomd at /usr/local/bin/oa-workroomd (staged by
#     build-workroomd-for-image.sh)
#   - the proven egress fix: systemd-networkd + systemd-resolved disabled and
#     masked (kernel ip= boot-arg networking), static /etc/resolv.conf
#
# MUST run as root on a Linux x86_64 host (normally the nested-virt bake host
# agent-computer-gce-1). It never touches an existing image in place: it bakes
# to --output (default a new timestamped file), fsck-verifies, seals the
# sha256, and writes a refs-and-digests-only bake receipt JSON next to the
# image. Re-pin agent-computer-image.manifest.json `rootfsDigest` from that
# receipt only after the boot smoke passes.
#
# SECURITY: no credentials, tokens, wallet material, or private topology go
# into the image or the receipt. The image carries runtime + agents only;
# provider auth is broker-redeemed per turn at runtime (broker_only custody).
set -euo pipefail

# --- pins ------------------------------------------------------------------
NODE_VERSION="24.13.1"
NODE_TARBALL_SHA256="30215f90ea3cd04dfbc06e762c021393fa173a1d392974298bbc871a8e461089"
CODEX_VERSION="0.144.0"
# npm tarball @openai/codex-0.144.0-linux-x64.tgz
CODEX_TARBALL_SHA256="391a3793d21feff08da2d9132f01107dd56fa5a48a158e23d15c6d56e34f7cb2"
# package/vendor/x86_64-unknown-linux-musl/bin/codex inside that tarball
CODEX_BINARY_SHA256="901923c1808a151f6926d41d703c17ad48815662cefb1c8d832a052c44271429"
CLAUDE_CODE_VERSION="2.1.218"
OPENCODE_VERSION="1.18.4"
PI_VERSION="0.81.1"
GOOSE_VERSION="1.43.0"
GOOSE_TARBALL_SHA256="a9a96f559a8b5f20b11597b78e4aa5bb0b9b29796ec4f808ca466a3f59a5ec20"
CURSOR_VERSION="2026.07.20-8cc9c0b"
CURSOR_TARBALL_SHA256="6e9f17247ffeb5f8f7e2246b4bcd6bb26cb2d5a9f9a4b0012c9a80d868ed25b4"
GROK_VERSION="0.2.106"
GROK_BINARY_SHA256="7180d0e03cc2a496033ff3aae2223ce239446a9827a59faa76091c7edd5e1c38"
TYPESCRIPT_LANGUAGE_SERVER_VERSION="5.3.0"
TYPESCRIPT_LANGUAGE_SERVER_TARBALL_SHA256="398cacc17fff2108652e7b4050e3182008d17063246b3fea7dcf5fae2ce1560e"
TYPESCRIPT_VERSION="5.9.3"
TYPESCRIPT_TARBALL_SHA256="10e108c9cf7d5f2879053dff18515fb405abf2ccef63eaaf017d9c571687a1d3"
SUITE="jammy"
MIRROR="http://archive.ubuntu.com/ubuntu"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<'EOF'
Usage: sudo build-agent-computer-rootfs.sh [options]
  --output PATH        output ext4 image (default ./agent-computer-rootfs-<date>.ext4)
  --size-mib N         image size in MiB (default 4096)
  --turn-runner PATH   prebuilt Node/Vite Plus turn-runner executable (required
                       unless --repo-root lets this script compile it)
  --portable-session-control PATH
                       prebuilt Node/Vite Plus portable session controller
                       (required unless --repo-root lets this script compile it)
  --repo-root PATH     openagents checkout to compile the turn-runner from
                       (requires host Vite Plus; used when --turn-runner absent)
  --workroomd PATH     oa-workroomd release binary (from
                       build-workroomd-for-image.sh); required unless
                       --skip-workroomd
  --skip-workroomd     bake without oa-workroomd (smoke images only)
  --help
EOF
}

OUTPUT=""
SIZE_MIB=4096
TURN_RUNNER=""
PORTABLE_SESSION_CONTROL=""
REPO_ROOT=""
WORKROOMD=""
SKIP_WORKROOMD=0
while [ $# -gt 0 ]; do
  case "$1" in
    --output) OUTPUT="$2"; shift 2 ;;
    --size-mib) SIZE_MIB="$2"; shift 2 ;;
    --turn-runner) TURN_RUNNER="$2"; shift 2 ;;
    --portable-session-control) PORTABLE_SESSION_CONTROL="$2"; shift 2 ;;
    --repo-root) REPO_ROOT="$2"; shift 2 ;;
    --workroomd) WORKROOMD="$2"; shift 2 ;;
    --skip-workroomd) SKIP_WORKROOMD=1; shift ;;
    --help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

fail() { echo "FAIL: $*" >&2; exit 1; }

install_packed_node_bundle() {
  local entry="$1"
  local entry_name="$2"
  local bundle_name="$3"
  local source_dir
  local target_dir

  source_dir="$(cd "$(dirname "$entry")" && pwd)"
  target_dir="$MNT/opt/agent/$bundle_name"
  install -d "$target_dir"

  # Vite Plus can emit imports beside the entry file. Keep the complete
  # executable bundle so that the entry does not lose a runtime or dynamic
  # chunk after it enters the guest image.
  find "$source_dir" -maxdepth 1 -type f -name '*.mjs' -exec \
    install -m 0644 '{}' "$target_dir/" \;
  [ -f "$target_dir/$(basename "$entry")" ] \
    || fail "packed Node entry was not copied: $entry"
  chmod 0755 "$target_dir/$(basename "$entry")"
  ln -s "$bundle_name/$(basename "$entry")" "$MNT/opt/agent/$entry_name"
}

# --- preflight (fail-closed; never a partial silent bake) -------------------
[ "$(uname -s)" = "Linux" ] || fail "bake host must be Linux (this is a guest rootfs bake)"
[ "$(uname -m)" = "x86_64" ] || fail "bake host must be x86_64"
[ "$(id -u)" = "0" ] || fail "must run as root (debootstrap + loop mount)"
for tool in debootstrap mkfs.ext4 curl tar sha256sum e2fsck unzip chroot jq; do
  command -v "$tool" >/dev/null 2>&1 || fail "missing tool: $tool"
done

if [ -z "$TURN_RUNNER" ]; then
  [ -n "$REPO_ROOT" ] || fail "--turn-runner or --repo-root is required"
  command -v vp >/dev/null 2>&1 || fail "--repo-root pack path needs Vite Plus"
fi
if [ -z "$PORTABLE_SESSION_CONTROL" ]; then
  [ -n "$REPO_ROOT" ] || fail "--portable-session-control or --repo-root is required"
  command -v vp >/dev/null 2>&1 || fail "--repo-root pack path needs Vite Plus"
fi
if [ "$SKIP_WORKROOMD" = "0" ]; then
  [ -n "$WORKROOMD" ] || fail "--workroomd is required (or pass --skip-workroomd for a smoke image)"
  [ -f "$WORKROOMD" ] || fail "workroomd binary not found: $WORKROOMD"
fi

[ -n "$OUTPUT" ] || OUTPUT="./agent-computer-rootfs-$(date -u +%Y%m%d%H%M%S).ext4"
[ ! -e "$OUTPUT" ] || fail "refusing to overwrite existing image: $OUTPUT (bake to a new path, re-pin after smoke)"

WORK="$(mktemp -d /tmp/agent-computer-bake.XXXXXX)"
MNT="$WORK/mnt"
mkdir -p "$MNT"
cleanup() {
  set +e
  mountpoint -q "$MNT/proc" && umount "$MNT/proc"
  mountpoint -q "$MNT" && umount "$MNT"
  rm -rf "$WORK"
}
trap cleanup EXIT

# --- 1. image + base system -------------------------------------------------
echo "==> creating ${SIZE_MIB}MiB ext4 image at $OUTPUT"
dd if=/dev/zero of="$OUTPUT" bs=1M count=0 seek="$SIZE_MIB" status=none
mkfs.ext4 -q -F "$OUTPUT"
mount -o loop "$OUTPUT" "$MNT"

echo "==> debootstrap $SUITE (git, python3, ca-certificates, openssh-client)"
debootstrap --include=git,python3,ca-certificates,openssh-client,zstd \
  "$SUITE" "$MNT" "$MIRROR"
# Native harness probes need the normal Linux process view. The mount is a
# bake-time input only and is removed before the image is sealed.
mount -t proc proc "$MNT/proc"

echo "$SUITE" >/dev/null # suite recorded in the receipt below
echo "agent-computer" > "$MNT/etc/hostname"

# --- 2. proven egress fix (kernel ip= networking; no networkd/resolved) -----
echo "==> applying egress fix (mask systemd-networkd/resolved, static resolv.conf)"
chroot "$MNT" systemctl disable systemd-networkd.service systemd-networkd.socket \
  systemd-networkd-wait-online.service >/dev/null 2>&1 || true
for unit in systemd-networkd.service systemd-networkd.socket \
  systemd-networkd-wait-online.service systemd-resolved.service; do
  ln -sf /dev/null "$MNT/etc/systemd/system/$unit"
done
rm -f "$MNT/etc/resolv.conf"
printf 'nameserver 8.8.8.8\nnameserver 1.1.1.1\n' > "$MNT/etc/resolv.conf"

# --- 3. Node (pinned and digest-verified) -----------------------------------
echo "==> installing Node $NODE_VERSION"
curl -fsSL -o "$WORK/node.tar.xz" \
  "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz"
echo "$NODE_TARBALL_SHA256  $WORK/node.tar.xz" | sha256sum -c - >/dev/null \
  || fail "Node tarball digest mismatch"
tar -xJf "$WORK/node.tar.xz" -C "$WORK"
cp -a "$WORK/node-v${NODE_VERSION}-linux-x64/." "$MNT/usr/local/"
chroot "$MNT" /usr/local/bin/node --version | grep -qx "v$NODE_VERSION" \
  || fail "baked Node does not report $NODE_VERSION"

# --- 4. codex (pinned, digest-verified) — the CX-3 lane ----------------------
echo "==> installing codex $CODEX_VERSION (npm linux-x64 vendor musl build)"
curl -fsSL -o "$WORK/codex.tgz" \
  "https://registry.npmjs.org/@openai/codex/-/codex-${CODEX_VERSION}-linux-x64.tgz"
echo "$CODEX_TARBALL_SHA256  $WORK/codex.tgz" | sha256sum -c - >/dev/null \
  || fail "codex npm tarball digest mismatch"
tar -xzf "$WORK/codex.tgz" -C "$WORK" \
  package/vendor/x86_64-unknown-linux-musl/bin/codex \
  package/vendor/x86_64-unknown-linux-musl/codex-path/rg
echo "$CODEX_BINARY_SHA256  $WORK/package/vendor/x86_64-unknown-linux-musl/bin/codex" \
  | sha256sum -c - >/dev/null || fail "codex binary digest mismatch"
install -m 0755 "$WORK/package/vendor/x86_64-unknown-linux-musl/bin/codex" \
  "$MNT/usr/local/bin/codex"
install -m 0755 "$WORK/package/vendor/x86_64-unknown-linux-musl/codex-path/rg" \
  "$MNT/usr/local/bin/rg"
chroot "$MNT" /usr/local/bin/codex --version >/dev/null \
  || fail "baked codex binary does not execute in the guest chroot"
chroot "$MNT" /usr/local/bin/codex login --help >/dev/null \
  || fail "baked codex login command is unavailable"
if chroot "$MNT" /usr/local/bin/codex login status >"$WORK/codex-login-status.txt" 2>&1; then
  fail "fresh image unexpectedly reports a logged-in Codex account"
fi
grep -Eiq 'not logged in|not authenticated|login required' "$WORK/codex-login-status.txt" \
  || fail "fresh image Codex login status was not the expected signed-out state"

# --- 5. remaining pinned harnesses -----------------------------------------
# The npm lock is image-local. It does not inherit the monorepo lock or float
# transitive versions. No provider key or subscription credential is present.
echo "==> installing pinned JavaScript harnesses"
install -d "$MNT/opt/agent/harnesses"
install -m 0644 "$SCRIPT_DIR/harnesses/package.json" \
  "$MNT/opt/agent/harnesses/package.json"
install -m 0644 "$SCRIPT_DIR/harnesses/package-lock.json" \
  "$MNT/opt/agent/harnesses/package-lock.json"
chroot "$MNT" /usr/local/bin/npm ci --prefix /opt/agent/harnesses \
  --ignore-scripts --no-audit --no-fund >/dev/null \
  || fail "pinned Agent Computer harness npm install failed"
if ! chroot "$MNT" /bin/sh -c \
  'cd /opt/agent/harnesses && /usr/local/bin/npm audit signatures --json' \
  > "$WORK/harness-signatures.json"; then
  fail "Agent Computer harness npm signature verification failed"
fi
jq -e '.invalid == [] and .missing == []' "$WORK/harness-signatures.json" >/dev/null \
  || fail "Agent Computer harness signature verification reported invalid or missing signatures"
# Lifecycle scripts stay disabled for the full dependency tree. Run only the
# signed, exact-version Claude package installer after the signature audit so
# that its platform-native executable is materialized.
chroot "$MNT" /usr/local/bin/node \
  /opt/agent/harnesses/node_modules/@anthropic-ai/claude-code/install.cjs \
  || fail "pinned Claude native executable install failed"
chroot "$MNT" /usr/local/bin/node \
  /opt/agent/harnesses/node_modules/opencode-ai/postinstall.mjs \
  || fail "pinned OpenCode native executable install failed"
for harness in claude opencode pi; do
  [ -x "$MNT/opt/agent/harnesses/node_modules/.bin/$harness" ] \
    || fail "pinned $harness executable is absent"
  ln -s "/opt/agent/harnesses/node_modules/.bin/$harness" \
    "$MNT/usr/local/bin/$harness"
done
chroot "$MNT" /usr/local/bin/claude --version | grep -Fq "$CLAUDE_CODE_VERSION" \
  || fail "baked claude does not report $CLAUDE_CODE_VERSION"
chroot "$MNT" /usr/local/bin/opencode --version | grep -Fq "$OPENCODE_VERSION" \
  || fail "baked opencode does not report $OPENCODE_VERSION"
chroot "$MNT" /usr/local/bin/pi --version | grep -Fq "$PI_VERSION" \
  || fail "baked pi does not report $PI_VERSION"

echo "==> installing goose $GOOSE_VERSION"
curl -fsSL -o "$WORK/goose.tgz" \
  "https://github.com/aaif-goose/goose/releases/download/v${GOOSE_VERSION}/goose-x86_64-unknown-linux-gnu.tar.gz"
echo "$GOOSE_TARBALL_SHA256  $WORK/goose.tgz" | sha256sum -c - >/dev/null \
  || fail "goose tarball digest mismatch"
tar -xzf "$WORK/goose.tgz" -C "$WORK"
GOOSE_BINARY="$(find "$WORK" -type f -name goose -perm -u+x -print -quit)"
[ -n "$GOOSE_BINARY" ] || fail "goose release did not contain its executable"
install -m 0755 "$GOOSE_BINARY" "$MNT/usr/local/bin/goose"
chroot "$MNT" /usr/local/bin/goose --version | grep -Fq "$GOOSE_VERSION" \
  || fail "baked goose does not report $GOOSE_VERSION"

echo "==> installing Cursor Agent $CURSOR_VERSION"
curl -fsSL -o "$WORK/cursor-agent.tgz" \
  "https://downloads.cursor.com/lab/${CURSOR_VERSION}/linux/x64/agent-cli-package.tar.gz"
echo "$CURSOR_TARBALL_SHA256  $WORK/cursor-agent.tgz" | sha256sum -c - >/dev/null \
  || fail "Cursor Agent tarball digest mismatch"
install -d "$MNT/opt/agent/cursor-agent/$CURSOR_VERSION"
tar --strip-components=1 -xzf "$WORK/cursor-agent.tgz" \
  -C "$MNT/opt/agent/cursor-agent/$CURSOR_VERSION"
[ -x "$MNT/opt/agent/cursor-agent/$CURSOR_VERSION/cursor-agent" ] \
  || fail "Cursor Agent release did not contain its executable"
ln -s "/opt/agent/cursor-agent/$CURSOR_VERSION/cursor-agent" \
  "$MNT/usr/local/bin/cursor-agent"
chroot "$MNT" /usr/local/bin/cursor-agent --version | grep -Fq "$CURSOR_VERSION" \
  || fail "baked Cursor Agent does not report $CURSOR_VERSION"

echo "==> installing Grok CLI $GROK_VERSION"
curl -fsSL -o "$WORK/grok" \
  "https://storage.googleapis.com/grok-build-public-artifacts/cli/grok-${GROK_VERSION}-linux-x86_64"
echo "$GROK_BINARY_SHA256  $WORK/grok" | sha256sum -c - >/dev/null \
  || fail "Grok CLI binary digest mismatch"
install -m 0755 "$WORK/grok" "$MNT/usr/local/bin/grok"
chroot "$MNT" /usr/local/bin/grok version | grep -Fq "$GROK_VERSION" \
  || fail "baked Grok CLI does not report $GROK_VERSION"

# --- 6. managed IDE protocol helpers ---------------------------------------
echo "==> installing signed TypeScript LSP $TYPESCRIPT_LANGUAGE_SERVER_VERSION with TypeScript $TYPESCRIPT_VERSION"
install -d "$MNT/opt/agent/typescript-lsp"
cat > "$MNT/opt/agent/typescript-lsp/package.json" <<'EOF'
{"name":"openagents-agent-computer-typescript-lsp","private":true,"dependencies":{"typescript":"5.9.3","typescript-language-server":"5.3.0"}}
EOF
cat > "$MNT/opt/agent/typescript-lsp/package-lock.json" <<'EOF'
{"name":"openagents-agent-computer-typescript-lsp","lockfileVersion":3,"requires":true,"packages":{"":{"name":"openagents-agent-computer-typescript-lsp","dependencies":{"typescript":"5.9.3","typescript-language-server":"5.3.0"}},"node_modules/typescript":{"version":"5.9.3","resolved":"https://registry.npmjs.org/typescript/-/typescript-5.9.3.tgz","integrity":"sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw==","license":"Apache-2.0","bin":{"tsc":"bin/tsc","tsserver":"bin/tsserver"},"engines":{"node":">=14.17"}},"node_modules/typescript-language-server":{"version":"5.3.0","resolved":"https://registry.npmjs.org/typescript-language-server/-/typescript-language-server-5.3.0.tgz","integrity":"sha512-5puofxZHgFdAYtfNpmwCAvgtaYgg8wrUnH30m7Ze3QuguId5RNRadKASpOpyDxTyUdAF51FjhTdjntLw/EuWcQ==","license":"Apache-2.0","bin":{"typescript-language-server":"lib/cli.mjs"},"engines":{"node":">=20"}}}}
EOF
chroot "$MNT" /usr/local/bin/npm ci --prefix /opt/agent/typescript-lsp \
  --ignore-scripts --no-audit --no-fund >/dev/null \
  || fail "pinned TypeScript LSP npm install failed"
if ! chroot "$MNT" /bin/sh -c \
  'cd /opt/agent/typescript-lsp && /usr/local/bin/npm audit signatures --json' \
  > "$WORK/typescript-lsp-signatures.json"; then
  fail "TypeScript LSP npm signature verification failed"
fi
jq -e '.invalid == [] and .missing == []' "$WORK/typescript-lsp-signatures.json" >/dev/null \
  || fail "TypeScript LSP npm signature verification reported invalid or missing signatures"
chroot "$MNT" /usr/local/bin/node \
  /opt/agent/typescript-lsp/node_modules/typescript-language-server/lib/cli.mjs --version \
  | grep -qx "$TYPESCRIPT_LANGUAGE_SERVER_VERSION" \
  || fail "baked TypeScript language server does not report the pinned version"

# --- 7. vsock guest agent (source-controlled, proven artifact) ---------------
echo "==> installing vsock guest agent"
install -d "$MNT/opt/agent"
install -m 0755 "$SCRIPT_DIR/guest-agent.py" "$MNT/opt/agent/guest-agent.py"
install -m 0644 "$SCRIPT_DIR/agent-guest.service" \
  "$MNT/etc/systemd/system/agent-guest.service"
install -d "$MNT/etc/systemd/system/multi-user.target.wants"
ln -sf /etc/systemd/system/agent-guest.service \
  "$MNT/etc/systemd/system/multi-user.target.wants/agent-guest.service"

# --- 8. turn-runner -----------------------------------------------------------
if [ -z "$TURN_RUNNER" ]; then
  echo "==> compiling turn-runner from $REPO_ROOT"
  TURN_RUNNER="$WORK/turn-runner"
  (cd "$REPO_ROOT" && vp pack \
    apps/pylon/deploy/agent-computer/turn-runner.ts \
    --out-dir "$WORK/turn-runner-build" --platform node --target node24 \
    --deps.always-bundle '.*')
  TURN_RUNNER="$WORK/turn-runner-build/turn-runner.mjs"
fi
[ -f "$TURN_RUNNER" ] || fail "turn-runner binary not found: $TURN_RUNNER"
install_packed_node_bundle "$TURN_RUNNER" "turn-runner" "turn-runner.bundle"

# Prove that the packed entry and every runtime dependency resolve inside the
# sealed guest. An intentionally incomplete work context must reach the
# turn-runner's typed failure closeout and write its public-safe result artifact.
# A missing external workspace package, broken entry point, or incomplete chunk
# set fails the bake here instead of producing an empty live microVM artifact.
install -d "$MNT/qa/artifacts"
printf '{}\n' > "$MNT/tmp/turn-runner-bake-probe.json"
if chroot "$MNT" /usr/bin/env \
  OA_ARTIFACT_DIR=/qa/artifacts \
  /opt/agent/turn-runner /tmp/turn-runner-bake-probe.json \
  >"$WORK/turn-runner-bake-probe.stdout" 2>"$WORK/turn-runner-bake-probe.stderr"; then
  fail "turn-runner bake probe unexpectedly accepted an incomplete work context"
fi
jq -e \
  '.schemaVersion == "openagents.agent_computer.turn_result.v1"
   and .failureReasonRef == "agent_computer.turn_failed"' \
  "$MNT/qa/artifacts/result.json" >/dev/null \
  || fail "turn-runner bake probe did not emit the typed failure artifact"
rm -rf "$MNT/qa/artifacts" "$MNT/tmp/turn-runner-bake-probe.json"

# --- 9. retained portable-session controller ---------------------------------
if [ -z "$PORTABLE_SESSION_CONTROL" ]; then
  echo "==> compiling portable-session-control from $REPO_ROOT"
  PORTABLE_SESSION_CONTROL="$WORK/portable-session-control"
  (cd "$REPO_ROOT" && vp pack \
    apps/pylon/deploy/agent-computer/portable-session-control.ts \
    --out-dir "$WORK/portable-session-control-build" \
    --platform node --target node24)
  PORTABLE_SESSION_CONTROL="$WORK/portable-session-control-build/portable-session-control.mjs"
fi
[ -f "$PORTABLE_SESSION_CONTROL" ] \
  || fail "portable-session-control binary not found: $PORTABLE_SESSION_CONTROL"
install_packed_node_bundle \
  "$PORTABLE_SESSION_CONTROL" \
  "portable-session-control" \
  "portable-session-control.bundle"

# --- 10. oa-workroomd ----------------------------------------------------------
WORKROOMD_SHA256="skipped"
if [ "$SKIP_WORKROOMD" = "0" ]; then
  echo "==> installing oa-workroomd"
  install -m 0755 "$WORKROOMD" "$MNT/usr/local/bin/oa-workroomd"
  WORKROOMD_SHA256="$(sha256sum "$WORKROOMD" | cut -d' ' -f1)"
fi

# --- 11. seal -----------------------------------------------------------------
HARNESS_PACKAGE_LOCK_SHA256="$(sha256sum "$SCRIPT_DIR/harnesses/package-lock.json" | cut -d' ' -f1)"
TURN_RUNNER_SHA256="$(sha256sum "$TURN_RUNNER" | cut -d' ' -f1)"
PORTABLE_SESSION_CONTROL_SHA256="$(sha256sum "$PORTABLE_SESSION_CONTROL" | cut -d' ' -f1)"
umount "$MNT/proc"
umount "$MNT"
e2fsck -fy "$OUTPUT" >/dev/null || [ $? -le 1 ] || fail "e2fsck reported unrecovered errors"
ROOTFS_SHA256="$(sha256sum "$OUTPUT" | cut -d' ' -f1)"

RECEIPT="${OUTPUT%.ext4}.bake-receipt.json"
cat > "$RECEIPT" <<EOF
{
  "schemaVersion": "openagents.agent_computer_rootfs_bake_receipt.v1",
  "bakedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "suite": "$SUITE",
  "sizeMib": $SIZE_MIB,
  "nodeVersion": "$NODE_VERSION",
  "nodeAbi": "node-v${NODE_VERSION}-linux-x64",
  "codexVersion": "$CODEX_VERSION",
  "codexBinarySha256": "$CODEX_BINARY_SHA256",
  "harnesses": {
    "claudeCode": { "version": "$CLAUDE_CODE_VERSION" },
    "cursor": { "version": "$CURSOR_VERSION", "tarballSha256": "$CURSOR_TARBALL_SHA256" },
    "goose": { "version": "$GOOSE_VERSION", "tarballSha256": "$GOOSE_TARBALL_SHA256" },
    "grok": { "version": "$GROK_VERSION", "binarySha256": "$GROK_BINARY_SHA256" },
    "opencode": { "version": "$OPENCODE_VERSION" },
    "pi": { "version": "$PI_VERSION" },
    "npmPackageLockSha256": "$HARNESS_PACKAGE_LOCK_SHA256",
    "credentialMaterial": "runtime_only"
  },
  "typescriptLanguageServer": {
    "artifactRef": "artifact.npm.typescript-language-server.${TYPESCRIPT_LANGUAGE_SERVER_VERSION}.sha256-${TYPESCRIPT_LANGUAGE_SERVER_TARBALL_SHA256}",
    "profileRef": "profile.agent-computer.typescript-lsp.v1",
    "version": "$TYPESCRIPT_LANGUAGE_SERVER_VERSION",
    "registryIntegrity": "sha512-5puofxZHgFdAYtfNpmwCAvgtaYgg8wrUnH30m7Ze3QuguId5RNRadKASpOpyDxTyUdAF51FjhTdjntLw/EuWcQ==",
    "tarballSha256": "$TYPESCRIPT_LANGUAGE_SERVER_TARBALL_SHA256",
    "tarballSha256Role": "registry provenance only; installed bytes use package-lock sha512 integrity and npm signature verification",
    "typescriptVersion": "$TYPESCRIPT_VERSION",
    "typescriptRegistryIntegrity": "sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw==",
    "typescriptTarballSha256": "$TYPESCRIPT_TARBALL_SHA256",
    "typescriptTarballSha256Role": "registry provenance only; installed bytes use package-lock sha512 integrity and npm signature verification",
    "nodeAbi": "node-v${NODE_VERSION}-linux-x64",
    "command": "/usr/local/bin/node",
    "argv": ["/opt/agent/typescript-lsp/node_modules/typescript-language-server/lib/cli.mjs", "--stdio"],
    "license": "Apache-2.0",
    "licenseNoticePaths": ["/opt/agent/typescript-lsp/node_modules/typescript-language-server/LICENSE", "/opt/agent/typescript-lsp/node_modules/typescript/LICENSE.txt"],
    "signatureVerification": "npm audit signatures; invalid=[] and missing=[] required",
    "protocolHandshake": "LSP initialize response plus $/typescriptVersion version=${TYPESCRIPT_VERSION}",
    "liveness": "generation-bound wrapper PID, child-exit coupling, and ready-file handshake",
    "cleanup": "LSP shutdown, enforced termination, and ready/state file removal",
    "target": "linux-x64"
  },
  "turnRunnerSha256": "$TURN_RUNNER_SHA256",
  "portableSessionControlSha256": "$PORTABLE_SESSION_CONTROL_SHA256",
  "workroomdSha256": "$WORKROOMD_SHA256",
  "rootfsSha256": "$ROOTFS_SHA256",
  "buildScript": "apps/pylon/deploy/agent-computer/build-agent-computer-rootfs.sh"
}
EOF

echo "==> BAKE OK"
echo "    image:   $OUTPUT"
echo "    sha256:  $ROOTFS_SHA256"
echo "    receipt: $RECEIPT"
echo "Next: boot-smoke the image in a microVM (guest agent ready + all seven harness version probes"
echo "over vsock exec), then re-pin rootfsDigest in agent-computer-image.manifest.json."

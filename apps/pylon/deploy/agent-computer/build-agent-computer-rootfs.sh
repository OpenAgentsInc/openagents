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
#   - the PINNED codex binary (npm @openai/codex linux-x64 vendor musl build)
#     at /usr/local/bin/codex (+ its vendored rg at /usr/local/bin/rg) — the
#     CX-3 in-VM `codex exec --json` lane depends on this
#   - the vsock guest agent (guest-agent.py, agent-guest.service enabled,
#     AF_VSOCK :1024)
#   - the Vite Plus packed turn-runner at /opt/agent/turn-runner
#     of apps/pylon/deploy/agent-computer/turn-runner.ts)
#   - the fixed PORT-03 retained-session controller at
#     /opt/agent/portable-session-control (no arbitrary command surface)
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

# --- preflight (fail-closed; never a partial silent bake) -------------------
[ "$(uname -s)" = "Linux" ] || fail "bake host must be Linux (this is a guest rootfs bake)"
[ "$(uname -m)" = "x86_64" ] || fail "bake host must be x86_64"
[ "$(id -u)" = "0" ] || fail "must run as root (debootstrap + loop mount)"
for tool in debootstrap mkfs.ext4 curl tar sha256sum e2fsck unzip chroot; do
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

# --- 5. vsock guest agent (source-controlled, proven artifact) ---------------
echo "==> installing vsock guest agent"
install -d "$MNT/opt/agent"
install -m 0755 "$SCRIPT_DIR/guest-agent.py" "$MNT/opt/agent/guest-agent.py"
install -m 0644 "$SCRIPT_DIR/agent-guest.service" \
  "$MNT/etc/systemd/system/agent-guest.service"
install -d "$MNT/etc/systemd/system/multi-user.target.wants"
ln -sf /etc/systemd/system/agent-guest.service \
  "$MNT/etc/systemd/system/multi-user.target.wants/agent-guest.service"

# --- 6. turn-runner -----------------------------------------------------------
if [ -z "$TURN_RUNNER" ]; then
  echo "==> compiling turn-runner from $REPO_ROOT"
  TURN_RUNNER="$WORK/turn-runner"
  (cd "$REPO_ROOT" && vp pack \
    apps/pylon/deploy/agent-computer/turn-runner.ts \
    --out-dir "$WORK/turn-runner-build" --platform node --target node24 --exe)
  TURN_RUNNER="$WORK/turn-runner-build/turn-runner.mjs"
fi
[ -f "$TURN_RUNNER" ] || fail "turn-runner binary not found: $TURN_RUNNER"
install -m 0755 "$TURN_RUNNER" "$MNT/opt/agent/turn-runner"

# --- 7. retained portable-session controller ---------------------------------
if [ -z "$PORTABLE_SESSION_CONTROL" ]; then
  echo "==> compiling portable-session-control from $REPO_ROOT"
  PORTABLE_SESSION_CONTROL="$WORK/portable-session-control"
  (cd "$REPO_ROOT" && vp pack \
    apps/pylon/deploy/agent-computer/portable-session-control.ts \
    --out-dir "$WORK/portable-session-control-build" \
    --platform node --target node24 --exe)
  PORTABLE_SESSION_CONTROL="$WORK/portable-session-control-build/portable-session-control.mjs"
fi
[ -f "$PORTABLE_SESSION_CONTROL" ] \
  || fail "portable-session-control binary not found: $PORTABLE_SESSION_CONTROL"
install -m 0755 "$PORTABLE_SESSION_CONTROL" \
  "$MNT/opt/agent/portable-session-control"

# --- 8. oa-workroomd -----------------------------------------------------------
WORKROOMD_SHA256="skipped"
if [ "$SKIP_WORKROOMD" = "0" ]; then
  echo "==> installing oa-workroomd"
  install -m 0755 "$WORKROOMD" "$MNT/usr/local/bin/oa-workroomd"
  WORKROOMD_SHA256="$(sha256sum "$WORKROOMD" | cut -d' ' -f1)"
fi

# --- 9. seal ------------------------------------------------------------------
TURN_RUNNER_SHA256="$(sha256sum "$TURN_RUNNER" | cut -d' ' -f1)"
PORTABLE_SESSION_CONTROL_SHA256="$(sha256sum "$PORTABLE_SESSION_CONTROL" | cut -d' ' -f1)"
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
  "codexVersion": "$CODEX_VERSION",
  "codexBinarySha256": "$CODEX_BINARY_SHA256",
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
echo "Next: boot-smoke the image in a microVM (guest agent ready + 'codex --version'"
echo "over vsock exec), then re-pin rootfsDigest in agent-computer-image.manifest.json."

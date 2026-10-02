#!/usr/bin/env bash
#
# Set up a Coder host: the machine a cloud Coder run builds and works on.
#
# One script for every backend that starts warm from a snapshot:
#   - the daily GCE image `oa-coder-host` (scripts/cloud/build-coder-host-image.sh,
#     docs/deployment/coder-host-image.md), and
#   - the daily Boat template `oa-coder-main-<date>` (Boat SDK plan B5,
#     docs/cloud/2026-10-02-boat-sdk-plan.md).
#
# What it installs, idempotently:
#   - build tools (build-essential, pkg-config, clang, cmake, protobuf),
#     git, gh, ripgrep, jq
#   - rustup and the toolchain pinned in rust-toolchain.toml (1.97.1)
#   - sccache, wired in through a constant rustc wrapper
#   - Node.js (LTS) and the engine CLIs: Codex (`codex`), Claude Code
#     (`claude`), Grok Build (`grok`). None is logged in. No credential of
#     any kind is written by this script; logins come later, per host.
#   - a clone of OpenAgentsInc/openagents at the requested revision
#   - with --warm: a warm Cargo target for openagents-cli, microcoder and
#     coder (their libraries, binaries and test targets) in the Coder target
#     slot the task runner leases first, and the release `openagents`
#     binary in /usr/local/bin. The workspace's own test executables,
#     binaries and incremental caches are then pruned (--no-prune keeps
#     them): a Coder worktree never reuses them.
#
# Usage (as root, or as a user with passwordless sudo):
#   scripts/cloud/coder-host-setup.sh [--user NAME] [--repo-dir DIR]
#       [--repo-url URL] [--rev REV] [--warm] [--no-release-binary]
#       [--sccache-bucket BUCKET] [--no-engines] [--no-prune]
#
# Run as root it sets up the user `coder` (created if missing); run as any
# other user it sets up that user. It never prints a secret and never reads
# one.
set -euo pipefail

user=""
repo_dir=""
repo_url="https://github.com/OpenAgentsInc/openagents.git"
rev="origin/main"
warm="false"
release_binary="true"
sccache_bucket=""
engines="true"
prune="true"
# Pinned tool versions. The Rust toolchain itself comes from the repository's
# rust-toolchain.toml; this is only the fallback before the clone exists.
RUST_TOOLCHAIN_DEFAULT="1.97.1"
SCCACHE_VERSION="v0.18.0"
NODE_MAJOR="24"
# The packages the warm target covers. Every Coder run builds and tests some
# of these; the rest of the workspace builds on top of their dependencies.
WARM_PACKAGES=(openagents-cli microcoder coder)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user) user="${2:?}"; shift 2 ;;
    --repo-dir) repo_dir="${2:?}"; shift 2 ;;
    --repo-url) repo_url="${2:?}"; shift 2 ;;
    --rev) rev="${2:?}"; shift 2 ;;
    --warm) warm="true"; shift ;;
    --no-release-binary) release_binary="false"; shift ;;
    --sccache-bucket) sccache_bucket="${2:?}"; shift 2 ;;
    --no-engines) engines="false"; shift ;;
    --no-prune) prune="false"; shift ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "coder-host-setup: unknown argument: $1" >&2; exit 2 ;;
  esac
done

log() { local phase="$1"; shift; printf 'OA_CODER_HOST_SETUP %s t=%s %s\n' "$phase" "$(date -u +%s)" "$*"; }

if [[ "$(id -u)" == 0 ]]; then
  SUDO=""
  user="${user:-coder}"
else
  SUDO="sudo -n"
  user="${user:-$(id -un)}"
fi
as_root() { $SUDO "$@"; }

if ! id "$user" >/dev/null 2>&1; then
  as_root useradd --create-home --shell /bin/bash "$user"
fi
home="$(getent passwd "$user" | cut -d: -f6)"
repo_dir="${repo_dir:-$home/openagents}"

# Run a command as the host user with a login-like environment.
as_user() {
  if [[ "$(id -un)" == "$user" ]]; then
    env HOME="$home" PATH="$home/.cargo/bin:$home/.grok/bin:$home/.local/bin:/usr/local/bin:/usr/bin:/bin" "$@"
  else
    as_root runuser -u "$user" -- env HOME="$home" \
      PATH="$home/.cargo/bin:$home/.grok/bin:$home/.local/bin:/usr/local/bin:/usr/bin:/bin" "$@"
  fi
}

# ---------------------------------------------------------------- packages
log packages begin
export DEBIAN_FRONTEND=noninteractive
as_root install -d -m 0755 /etc/apt/keyrings
if [[ ! -s /etc/apt/keyrings/githubcli-archive-keyring.gpg ]]; then
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    | as_root tee /etc/apt/keyrings/githubcli-archive-keyring.gpg >/dev/null
  as_root chmod 0644 /etc/apt/keyrings/githubcli-archive-keyring.gpg
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    | as_root tee /etc/apt/sources.list.d/github-cli.list >/dev/null
fi
as_root apt-get update -q
as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends \
  build-essential pkg-config clang cmake protobuf-compiler libprotobuf-dev libssl-dev \
  ca-certificates curl git gh ripgrep jq xz-utils unzip zstd procps time \
  >/dev/null
log packages end

# ---------------------------------------------------------------- node
if ! command -v node >/dev/null 2>&1 || [[ "$(node -v | cut -c2- | cut -d. -f1)" != "$NODE_MAJOR" ]]; then
  log node begin
  node_version="$(curl -fsSL https://nodejs.org/dist/index.json \
    | jq -r --arg m "v$NODE_MAJOR." '[.[] | select(.version | startswith($m))][0].version')"
  tmp="$(mktemp -d)"
  curl -fsSL -o "$tmp/node.tar.xz" \
    "https://nodejs.org/dist/${node_version}/node-${node_version}-linux-x64.tar.xz"
  as_root rm -rf /opt/node
  as_root install -d -m 0755 /opt/node
  as_root tar -xJf "$tmp/node.tar.xz" -C /opt/node --strip-components=1
  rm -rf "$tmp"
  for bin in node npm npx; do as_root ln -sfn "/opt/node/bin/$bin" "/usr/local/bin/$bin"; done
  log node end "version=$node_version"
fi

# ---------------------------------------------------------------- repository
log repo begin
if [[ ! -d "$repo_dir/.git" ]]; then
  as_user git clone --quiet "$repo_url" "$repo_dir"
fi
as_user git -C "$repo_dir" fetch --quiet origin main
sha="$(as_user git -C "$repo_dir" rev-parse --verify "$rev^{commit}")"
as_user git -C "$repo_dir" checkout --quiet --detach "$sha"
log repo end "rev=$sha"

# ---------------------------------------------------------------- rust
log rust begin
if [[ ! -x "$home/.cargo/bin/rustup" ]]; then
  as_user sh -c 'curl -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none' >/dev/null
fi
toolchain="$RUST_TOOLCHAIN_DEFAULT"
if [[ -f "$repo_dir/rust-toolchain.toml" ]]; then
  toolchain="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo_dir/rust-toolchain.toml")"
fi
as_user rustup toolchain install "$toolchain" --profile minimal -c clippy -c rustfmt >/dev/null
as_user rustup default "$toolchain" >/dev/null
log rust end "toolchain=$toolchain"

# ---------------------------------------------------------------- sccache
if [[ ! -x /usr/local/bin/sccache ]] || ! /usr/local/bin/sccache --version | grep -q "${SCCACHE_VERSION#v}"; then
  log sccache begin
  tmp="$(mktemp -d)"
  name="sccache-${SCCACHE_VERSION}-x86_64-unknown-linux-musl"
  curl -fsSL -o "$tmp/s.tgz" \
    "https://github.com/mozilla/sccache/releases/download/${SCCACHE_VERSION}/${name}.tar.gz"
  tar -xzf "$tmp/s.tgz" -C "$tmp"
  as_root install -m 0755 "$tmp/$name/sccache" /usr/local/bin/sccache
  rm -rf "$tmp"
  log sccache end
fi
# The wrapper Cargo always runs. It is one constant path, so turning sccache
# on or off never changes a fingerprint. It bypasses sccache when the boot
# check (or an operator) left /run/oa-coder-host/no-sccache, or when
# OA_SCCACHE=0.
as_root tee /usr/local/bin/oa-rustc-wrapper >/dev/null <<'WRAPPER'
#!/bin/sh
if [ "${OA_SCCACHE:-1}" != 0 ] && [ ! -e /run/oa-coder-host/no-sccache ] && [ -x /usr/local/bin/sccache ]; then
  exec /usr/local/bin/sccache "$@"
fi
exec "$@"
WRAPPER
as_root chmod 0755 /usr/local/bin/oa-rustc-wrapper
as_user install -d -m 0755 "$home/.cargo" "$home/.config/sccache"
as_user tee "$home/.cargo/config.toml" >/dev/null <<'CARGO'
# Written by scripts/cloud/coder-host-setup.sh.
[build]
rustc-wrapper = "/usr/local/bin/oa-rustc-wrapper"
CARGO
if [[ -n "$sccache_bucket" ]]; then
  # The bucket is read through the machine's own identity (the GCE service
  # account); no key file is involved.
  as_user tee "$home/.config/sccache/config" >/dev/null <<SCCACHE
[cache.gcs]
bucket = "$sccache_bucket"
key_prefix = "oa-coder-host"
rw_mode = "READ_WRITE"
SCCACHE
else
  as_user tee "$home/.config/sccache/config" >/dev/null <<SCCACHE
[cache.disk]
dir = "$home/.cache/sccache"
size = 21474836480
SCCACHE
fi

# ---------------------------------------------------------------- engines
if [[ "$engines" == "true" ]]; then
  log engines begin
  as_root env PATH="/opt/node/bin:$PATH" npm install -g --prefix /usr/local --no-fund --no-audit --loglevel=error \
    @openai/codex @anthropic-ai/claude-code >/dev/null
  # Grok Build installs per user into ~/.grok/bin.
  if [[ ! -x "$home/.grok/bin/grok" ]]; then
    as_user bash -c 'curl -fsSL https://x.ai/cli/install.sh | bash' >/dev/null
  fi
  # No engine may be logged in on an image or template.
  for auth in "$home/.codex/auth.json" "$home/.claude/.credentials.json" "$home/.grok/auth.json"; do
    if [[ -e "$auth" ]]; then
      echo "coder-host-setup: refusing: an engine login exists at $auth" >&2
      exit 3
    fi
  done
  # ~/.claude.json is Claude Code's settings and state file; Boat's image
  # ships one with only onboarding and theme. It is a login only when it
  # carries an account or a key.
  if [[ -e "$home/.claude.json" ]] && jq -e 'has("oauthAccount") or has("primaryApiKey") or has("customApiKeyResponses")' "$home/.claude.json" >/dev/null 2>&1; then
    echo "coder-host-setup: refusing: an engine login exists at $home/.claude.json" >&2
    exit 3
  fi
  log engines end
fi

# The Coder task runner leases target slots under ~/.openagents/targets,
# named after the repository's common Git directory
# (crates/coder/src/task/targets.rs): <name>-<sha256(path)[..12]>-slot-<n>.
common="$(cd "$repo_dir/.git" && pwd -P)"
tag="$(printf '%s' "$common" | sha256sum | cut -c1-12)"
slot="$home/.openagents/targets/$(basename "$(dirname "$common")")-$tag-slot-0"
as_user install -d -m 0755 "$home/.openagents/targets" "$slot"

# ---------------------------------------------------------------- warm build
if [[ "$warm" == "true" ]]; then
  log warm-fetch begin
  for attempt in 1 2 3; do
    as_user sh -c "cd '$repo_dir' && cargo fetch --locked" && break
    [[ $attempt == 3 ]] && exit 1
    sleep $(( attempt * 15 ))
  done
  log warm-fetch end
  # One package per invocation, as a Coder run builds and tests them:
  # Cargo unifies features across the packages of one invocation, so a
  # combined build would leave the per-package feature sets cold.
  # `--tests` builds each package's test targets (its dev-dependencies
  # included); --keep-going keeps one test target that does not compile on
  # main from leaving the rest cold.
  partial=""
  for p in "${WARM_PACKAGES[@]}"; do
    log warm-build begin "package=$p"
    as_user sh -c "cd '$repo_dir' && CARGO_TARGET_DIR='$slot' cargo build --locked -p $p"
    log warm-build end "package=$p"
    log warm-tests begin "package=$p"
    if as_user sh -c "cd '$repo_dir' && CARGO_TARGET_DIR='$slot' cargo build --locked --keep-going --tests -p $p"; then
      log warm-tests end "package=$p"
    else
      partial="$partial $p"
      log warm-tests end "package=$p partial=true"
    fi
  done
  if [[ "$prune" == "true" ]]; then
    # A Coder run builds in its own worktree, at a path unlike this clone's,
    # so the workspace's own outputs (test executables, binaries,
    # incremental caches) are never reused there: measured, they were 57 of
    # 77 GiB. Dependency rlibs, rmeta and build-script outputs are what a
    # worktree build reuses; they stay. A build in the clone itself relinks.
    log warm-prune begin
    as_user find "$slot/debug/deps" -maxdepth 1 -type f -executable ! -name '*.so' -delete
    as_user find "$slot/debug" -maxdepth 1 -type f -executable -delete
    as_user rm -rf "$slot/debug/incremental"
    log warm-prune end "slot_bytes=$(du -sb "$slot" | cut -f1)"
  fi
  if [[ "$release_binary" == "true" ]]; then
    log release begin
    rel="$home/.cache/oa-release-target"
    as_user sh -c "cd '$repo_dir' && CARGO_TARGET_DIR='$rel' cargo build --locked --release -p openagents-cli --bin openagents"
    as_root install -m 0755 "$rel/release/openagents" /usr/local/bin/openagents
    as_user rm -rf "$rel"
    log release end
  fi
  as_user sccache --show-stats >/dev/null 2>&1 || true
  as_user sccache --stop-server >/dev/null 2>&1 || true
fi

# ---------------------------------------------------------------- manifest
version_of() { as_user "$@" 2>/dev/null | head -1 | tr -d '\r' || true; }
manifest="$home/.openagents/coder-host.json"
jq -n \
  --arg built_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg rev "$sha" \
  --arg repo_dir "$repo_dir" \
  --arg slot "$slot" \
  --arg warm "$warm" \
  --arg partial "${partial:-}" \
  --arg packages "${WARM_PACKAGES[*]}" \
  --arg rustc "$(version_of rustc --version)" \
  --arg sccache "$(version_of sccache --version)" \
  --arg sccache_bucket "$sccache_bucket" \
  --arg node "$(version_of node --version)" \
  --arg gh "$(version_of gh --version)" \
  --arg codex "$(version_of codex --version)" \
  --arg claude "$(version_of claude --version)" \
  --arg grok "$(version_of grok --version)" \
  --arg openagents "$(version_of openagents --version)" \
  '{schema:"openagents.coder_host.v1", built_at:$built_at, rev:$rev, repo_dir:$repo_dir,
    warm_target:{slot:$slot, warm:($warm=="true"), packages:($packages|split(" ")),
                 tests_not_compiling:($partial|split(" ")|map(select(length>0)))},
    tools:{rustc:$rustc, sccache:$sccache, sccache_bucket:$sccache_bucket, node:$node, gh:$gh,
           codex:$codex, claude:$claude, grok:$grok, openagents:$openagents},
    logins:"none"}' | as_user tee "$manifest" >/dev/null
log finished "manifest=$manifest"

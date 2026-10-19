#!/bin/sh
# One shared remote Rust compile cache for every builder (#12176).
#
# kache (the rustc wrapper every builder runs) keeps its remote in its own
# config file, so this script edits that file and sets up the credentials
# kache's daemon reads. docs/coder/runtime/remote-build-cache.md explains the
# design; this is the machine setup.
#
# Usage: scripts/dev/remote-build-cache.sh COMMAND [OPTIONS]
#   install-kache [--dir DIR]
#       Install the pinned kache release (Linux) into DIR (~/.local/bin),
#       after checking its SHA-256. A kache of that version already there is
#       kept.
#   enable [--key FILE] [--local-max-size SIZE] [--set-wrapper] [--force]
#       Point kache at the shared bucket. Credentials, in order: --key (a
#       service-account key, which kache's daemon gets through its systemd
#       user unit on Linux or its launchd service on macOS), this VM's GCE
#       service account, or gcloud's application default credentials.
#       --local-max-size sets the local cap when the config file is new (a
#       new file also gets gc_evict_shared = true). --set-wrapper makes kache
#       Cargo's rustc wrapper when ~/.cargo/config.toml has none. Without
#       credentials it refuses unless --force.
#   disable
#       Remove the remote; kache stays a local cache.
#   status
#       The config file, bucket, credential source, and wrapper.
#
# Environment: OA_BUILD_CACHE_BUCKET (openagentsgemini-rust-build-cache),
# KACHE_CONFIG (kache's config file), KACHE (the kache program),
# OA_BUILD_CACHE_SERVICE=0 (leave the daemon's service alone).
# It never prints a key's contents.
set -eu

KACHE_VERSION=0.28.1
KACHE_SHA256_X86_64=68635a9f92ce5015a2c48c98b968d1b3707b6dbee0ca725e51d1671c9346f38f
KACHE_SHA256_AARCH64=b2110805eb24afe9b408e5b9a1e3d0584b7fdd0b79fe8cbe548ab29fbaf50176
KACHE_URL=${OA_KACHE_URL:-https://github.com/kunobi-ninja/kache/releases/download/v$KACHE_VERSION}

bucket=${OA_BUILD_CACHE_BUCKET:-openagentsgemini-rust-build-cache}
begin='# >>> openagents remote build cache (#12176, scripts/dev/remote-build-cache.sh)'
end='# <<< openagents remote build cache'

die() { echo "remote-build-cache: $*" >&2; exit 1; }

config_file() {
  if [ -n "${KACHE_CONFIG:-}" ]; then
    printf '%s\n' "$KACHE_CONFIG"
  else
    printf '%s\n' "${XDG_CONFIG_HOME:-$HOME/.config}/kache/config.toml"
  fi
}

# A GCE VM answers with its own service account; OA_BUILD_CACHE_GCE=0/1
# overrides the check (tests, or a VM whose account lacks the bucket).
on_gce() {
  case ${OA_BUILD_CACHE_GCE:-} in
    1) return 0 ;;
    0) return 1 ;;
  esac
  grep -qs 'Google' /sys/class/dmi/id/product_name
}

kache_bin() {
  if [ -n "${KACHE:-}" ]; then printf '%s\n' "$KACHE"; return; fi
  command -v kache 2>/dev/null || {
    for k in "$HOME/.local/bin/kache" "$HOME/.cargo/bin/kache"; do
      [ -x "$k" ] && { printf '%s\n' "$k"; return; }
    done
    return 1
  }
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}

# Print the file without our block.
strip_block() {
  awk -v b="$begin" -v e="$end" '$0==b{skip=1;next} skip&&$0==e{skip=0;next} !skip' "$1"
}

systemd_user() {
  [ "$(uname -s)" = Linux ] && command -v systemctl >/dev/null 2>&1 &&
    systemctl --user show-environment >/dev/null 2>&1
}

dropin() {
  printf '%s\n' "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/kache.service.d/openagents-remote.conf"
}

cmd_install_kache() {
  dir=$HOME/.local/bin
  while [ $# -gt 0 ]; do
    case $1 in
      --dir) dir=${2:?}; shift 2 ;;
      *) die "unknown option: $1" ;;
    esac
  done
  [ "$(uname -s)" = Linux ] || die "install kache with Cargo or Homebrew on this system (cargo install kache --version $KACHE_VERSION --locked)"
  case $(uname -m) in
    x86_64|amd64) arch=x86_64; want=$KACHE_SHA256_X86_64 ;;
    aarch64|arm64) arch=aarch64; want=$KACHE_SHA256_AARCH64 ;;
    *) die "no kache release for $(uname -m)" ;;
  esac
  if [ -x "$dir/kache" ] && "$dir/kache" --version 2>/dev/null | grep -qx "kache $KACHE_VERSION"; then
    echo "kache $KACHE_VERSION is already at $dir/kache"
    return
  fi
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  name=kache-$arch-unknown-linux-musl.tar.gz
  curl -fsSL -o "$tmp/$name" "$KACHE_URL/$name" || die "could not download $name"
  got=$(sha256 "$tmp/$name")
  [ "$got" = "$want" ] || die "$name has the wrong SHA-256 ($got); nothing installed"
  tar -xzf "$tmp/$name" -C "$tmp" kache
  mkdir -p "$dir"
  install -m 0755 "$tmp/kache" "$dir/kache.new"
  mv -f "$dir/kache.new" "$dir/kache"
  echo "installed kache $KACHE_VERSION at $dir/kache"
}

cmd_enable() {
  key=
  max=
  wrapper=
  force=
  while [ $# -gt 0 ]; do
    case $1 in
      --key) key=${2:?}; shift 2 ;;
      --local-max-size) max=${2:?}; shift 2 ;;
      --set-wrapper) wrapper=1; shift ;;
      --force) force=1; shift ;;
      *) die "unknown option: $1" ;;
    esac
  done
  if [ -n "$key" ]; then
    [ -r "$key" ] || die "cannot read the key file $key"
    case $key in /*) ;; *) key=$(CDPATH= cd -- "$(dirname -- "$key")" && pwd -P)/$(basename -- "$key") ;; esac
    source="key $key"
  elif on_gce; then
    source=gce
  elif [ -r "${CLOUDSDK_CONFIG:-$HOME/.config/gcloud}/application_default_credentials.json" ]; then
    source=adc
  elif [ -n "$force" ]; then
    source=none
  else
    die "no credentials for gs://$bucket: pass --key FILE, run on a GCE VM, or run gcloud auth application-default login"
  fi
  file=$(config_file)
  mkdir -p "$(dirname -- "$file")"
  if [ ! -f "$file" ]; then
    # gc_evict_shared keeps the cap real beside long-lived target
    # directories (docs/background/kache.md).
    { echo '[cache]'; [ -z "$max" ] || echo "local_max_size = \"$max\""; echo 'gc_evict_shared = true'; } > "$file"
  fi
  rest=$(strip_block "$file")
  if printf '%s\n' "$rest" | grep -Eq '^[[:space:]]*\[cache\.remote\]'; then
    die "$file already names another [cache.remote]; remove it first"
  fi
  {
    printf '%s\n' "$rest"
    echo "$begin"
    echo "# credentials: $source"
    echo '[cache.remote]'
    echo 'type = "gcs"'
    echo "bucket = \"$bucket\""
    echo 'prefix = "artifacts"'
    echo "$end"
  } > "$file.new.$$"
  mv -f "$file.new.$$" "$file"
  echo "kache remote: gs://$bucket/artifacts in $file (credentials: ${source%% *})"
  case $source in
    key\ *)
      if [ "${OA_BUILD_CACHE_SERVICE:-1}" = 0 ]; then
        :
      elif [ "$(uname -s)" = Darwin ]; then
        # kache's login service, with the key in its own environment only.
        k=$(kache_bin) || die "kache is not installed"
        plist=$HOME/Library/LaunchAgents/ninja.kunobi.kache.plist
        [ -f "$plist" ] || "$k" daemon install >/dev/null
        plutil -replace EnvironmentVariables.GOOGLE_APPLICATION_CREDENTIALS -string "$key" "$plist"
        launchctl bootout "gui/$(id -u)" "$plist" 2>/dev/null || true
        launchctl bootstrap "gui/$(id -u)" "$plist"
        echo "kache daemon: launchd service, key in $plist"
      elif systemd_user; then
        unit=$(dropin)
        mkdir -p "$(dirname -- "$unit")"
        printf '[Service]\nEnvironment=GOOGLE_APPLICATION_CREDENTIALS=%s\n' "$key" > "$unit"
        k=$(kache_bin) || die "kache is not installed; run install-kache first"
        [ -f "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/kache.service" ] || "$k" daemon install >/dev/null
        systemctl --user daemon-reload
        systemctl --user restart kache.service
        echo "kache daemon: systemd user unit, key from $unit"
      else
        echo "No login service manager: the kache daemon reads the key only when it starts with GOOGLE_APPLICATION_CREDENTIALS set."
      fi
      ;;
  esac
  if [ -n "$wrapper" ]; then
    k=$(kache_bin) || die "kache is not installed; run install-kache first"
    cargo_config=${CARGO_HOME:-$HOME/.cargo}/config.toml
    if [ ! -f "$cargo_config" ]; then
      mkdir -p "$(dirname -- "$cargo_config")"
      printf '# Written by scripts/dev/remote-build-cache.sh (#12176).\n[build]\nrustc-wrapper = "%s"\n' "$k" > "$cargo_config"
      echo "Cargo wrapper: $k in $cargo_config"
    elif grep -q 'rustc-wrapper' "$cargo_config"; then
      echo "Cargo wrapper: left as $cargo_config has it"
    else
      die "$cargo_config exists without a rustc-wrapper; add rustc-wrapper = \"$k\" under [build]"
    fi
  fi
}

cmd_disable() {
  file=$(config_file)
  if [ -f "$file" ]; then
    strip_block "$file" > "$file.new.$$"
    mv -f "$file.new.$$" "$file"
  fi
  unit=$(dropin)
  if [ -f "$unit" ]; then
    rm -f "$unit"
    if systemd_user; then
      systemctl --user daemon-reload
      systemctl --user try-restart kache.service || true
    fi
  fi
  echo "kache remote: off ($file)"
}

cmd_status() {
  file=$(config_file)
  if [ -f "$file" ] && grep -qxF "$begin" "$file"; then
    remote=$(sed -n 's/^bucket = "\(.*\)"$/\1/p' "$file" | tail -1)
    source=$(sed -n 's/^# credentials: //p' "$file" | tail -1)
    echo "remote: gs://$remote/artifacts"
    case $source in
      key\ *)
        k=${source#key }
        if [ -r "$k" ]; then echo "credentials: key file $k"; else echo "credentials: key file $k is missing; builds stay local"; fi
        ;;
      *) echo "credentials: $source" ;;
    esac
  else
    echo "remote: off"
  fi
  echo "config: $file"
  k=$(kache_bin) && echo "kache: $k ($("$k" --version 2>/dev/null))" || echo "kache: not installed"
  cargo_config=${CARGO_HOME:-$HOME/.cargo}/config.toml
  w=$(sed -n 's/^rustc-wrapper *= *"\(.*\)"/\1/p' "$cargo_config" 2>/dev/null | head -1)
  echo "rustc wrapper: ${RUSTC_WRAPPER:-${w:-none}}"
}

command=${1:-}
[ $# -eq 0 ] || shift
case $command in
  install-kache) cmd_install_kache "$@" ;;
  enable) cmd_enable "$@" ;;
  disable) cmd_disable "$@" ;;
  status) cmd_status "$@" ;;
  -h|--help|help|'') sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//' ;;
  *) die "unknown command: $command (try --help)" ;;
esac

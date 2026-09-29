#!/usr/bin/env bash
# Install or upgrade iroh-relay on a Debian 12 host. Run as root from a
# checkout of this directory:
#   sudo ./install.sh
# Override the version only together with its checksum:
#   sudo IROH_RELAY_VERSION=1.4.0 IROH_RELAY_SHA256=<sha256 of the tarball> ./install.sh
set -euo pipefail

VERSION="${IROH_RELAY_VERSION:-1.3.0}"
# SHA-256 of iroh-relay-v1.3.0-x86_64-unknown-linux-musl.tar.gz from the
# n0-computer/iroh GitHub release v1.3.0.
SHA256="${IROH_RELAY_SHA256:-677f4c62342a6ba8044459b5fd4302f2b1dcb8402542072e3a4ade5039bc0b9e}"
ARCH="x86_64-unknown-linux-musl"
TARBALL="iroh-relay-v${VERSION}-${ARCH}.tar.gz"
URL="https://github.com/n0-computer/iroh/releases/download/v${VERSION}/${TARBALL}"
HERE="$(cd "$(dirname "$0")" && pwd)"

[ "$(id -u)" -eq 0 ] || { echo "run as root" >&2; exit 1; }

if ! id iroh-relay >/dev/null 2>&1; then
  useradd --system --home-dir /var/lib/iroh-relay --no-create-home \
    --shell /usr/sbin/nologin iroh-relay
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fsSL -o "$tmp/$TARBALL" "$URL"
echo "$SHA256  $tmp/$TARBALL" | sha256sum -c -
tar -xzf "$tmp/$TARBALL" -C "$tmp"

release="/opt/iroh-relay/releases/v${VERSION}"
install -d -m 0755 "$release"
install -m 0755 "$tmp/iroh-relay" "$release/iroh-relay"
ln -sfn "$release" /opt/iroh-relay/current.new
mv -T /opt/iroh-relay/current.new /opt/iroh-relay/current

install -d -m 0755 /etc/iroh-relay
install -m 0644 "$HERE/iroh-relay.toml" /etc/iroh-relay/iroh-relay.toml
install -m 0644 "$HERE/iroh-relay.service" /etc/systemd/system/iroh-relay.service

systemctl daemon-reload
systemctl enable iroh-relay.service
systemctl restart iroh-relay.service
sleep 2
systemctl --no-pager --lines=0 status iroh-relay.service
/opt/iroh-relay/current/iroh-relay --version || true

#!/usr/bin/env bash
# Firm reason: 4. The startup script of the Linux release VM (coax-publish-linux,
# coax/RELEASE.md, Published builds). The VM is off until the Mac's release
# host starts it on a compiler change (publish-host.sh, COAX_PUBLISH_LINUX_VM).
# At each boot it publishes main's linux-x86_64 compiler build if that build is
# missing, then powers off, so it costs only while it builds.
#
# It runs as root from the instance metadata (startup-script). The VM runs as
# the automation service account, so gcloud uploads with the metadata server's
# credentials: no key is ever copied to it. Nix gives the static musl toolchain.
set -uo pipefail
exec >> /var/log/coax-publish.log 2>&1
echo "== $(date -u +%FT%TZ) boot"
user=coax
if ! id "$user" > /dev/null 2>&1; then
  apt-get update -q && apt-get install -yq git curl xz-utils ca-certificates
  useradd -m -s /bin/bash "$user"
  mkdir -p /nix && chown "$user" /nix
  su - "$user" -c 'curl -fsSL https://nixos.org/nix/install | sh -s -- --no-daemon'
  su - "$user" -c 'mkdir -p ~/.config/nix && echo "experimental-features = nix-command flakes" > ~/.config/nix/nix.conf'
fi
clone=/home/$user/.cache/coax-publish/openagents
su - "$user" -c "[ -d $clone/.git ] || { mkdir -p \$(dirname $clone) && git clone -q https://github.com/OpenAgentsInc/openagents $clone; }; git -C $clone fetch -q origin main && git -C $clone checkout -q --detach origin/main"
su - "$user" -c ". ~/.nix-profile/etc/profile.d/nix.sh; export PATH=\$PATH:/usr/bin:/usr/lib/google-cloud-sdk/bin; $clone/coax/publish-host.sh"
echo "== $(date -u +%FT%TZ) done (exit $?), powering off"
[[ -f /etc/coax-publish-stay-on ]] || poweroff

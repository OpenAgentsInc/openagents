#!/usr/bin/env bash
# Firm reason: 5. The startup script of a remote build host (oa-build-N,
# docs/verse/remote-builds.md): apt, useradd, the Nix and Bun installers,
# git and poweroff. It runs as root from the instance metadata at every boot.
#
# First boot: the user coax, Nix (the Wasm toolchain comes from nixpkgs,
# coax/web/toolchain.sh), Bun, and a blob-less clone of the repository in
# ~coax/oa that every job's worktree shares. Every boot: the idle watchdog,
# which powers the VM off once no job slot is held and nothing has run for
# oa-remote-idle-minutes (metadata, default 15). A stopped VM bills only its
# disk, and the disk keeps the caches (~/.cache/coax-wasm, coax-check,
# coax-release) for the next start.
set -uo pipefail
exec >> /var/log/oa-remote.log 2>&1
echo "== $(date -u +%FT%TZ) boot"
user=coax
home=/home/$user
meta() { curl -fsS -H 'Metadata-Flavor: Google' "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1" 2> /dev/null; }
if [[ ! -f /var/lib/oa-remote-ready ]]; then
  apt-get update -q
  DEBIAN_FRONTEND=noninteractive apt-get install -yq git curl xz-utils ca-certificates build-essential python3 rsync unzip jq util-linux
  id "$user" > /dev/null 2>&1 || useradd -m -s /bin/bash "$user"
  mkdir -p /nix && chown "$user" /nix
  su - "$user" -c '[ -e ~/.nix-profile ] || curl -fsSL https://nixos.org/nix/install | sh -s -- --no-daemon'
  su - "$user" -c 'mkdir -p ~/.config/nix && echo "experimental-features = nix-command flakes" > ~/.config/nix/nix.conf'
  su - "$user" -c '[ -x ~/.bun/bin/bun ] || curl -fsSL https://bun.sh/install | bash'
  su - "$user" -c 'grep -q nix-profile ~/.bashrc 2> /dev/null || printf "%s\n" ". ~/.nix-profile/etc/profile.d/nix.sh" "export PATH=\$HOME/.bun/bin:\$PATH" >> ~/.bashrc'
  # Blob-less: the history's trees and commits (the repository's packs are
  # about 10 GB), with file contents fetched as a checkout needs them.
  su - "$user" -c '[ -d ~/oa/.git ] || git clone -q --filter=blob:none https://github.com/OpenAgentsInc/openagents ~/oa'
  su - "$user" -c 'mkdir -p ~/slots ~/results ~/inbox'
  touch /var/lib/oa-remote-ready
  echo "== $(date -u +%FT%TZ) set up"
fi
touch "$home/busy"; chown "$user" "$home/busy"
idle="$(meta oa-remote-idle-minutes)"; [[ "$idle" =~ ^[0-9]+$ ]] || idle=15
# The watchdog: a slot is held while its lock is (host-job.sh holds
# ~/slots/N/lock with flock for the whole job).
cat > /usr/local/bin/oa-remote-idle <<EOF
#!/usr/bin/env bash
while sleep 60; do
  held=0
  for l in $home/slots/*/lock; do
    [[ -e "\$l" ]] || continue
    flock -n "\$l" true || { held=1; break; }
  done
  if [[ \$held == 1 ]]; then touch $home/busy; continue; fi
  if [[ -n "\$(find $home/busy -mmin +$idle 2> /dev/null)" ]]; then
    echo "\$(date -u +%FT%TZ) idle $idle minutes: powering off" >> /var/log/oa-remote.log
    poweroff
  fi
done
EOF
chmod 755 /usr/local/bin/oa-remote-idle
systemctl stop oa-remote-idle 2> /dev/null || true
systemd-run --unit oa-remote-idle --collect /usr/local/bin/oa-remote-idle
echo "== $(date -u +%FT%TZ) watchdog: $idle idle minutes"

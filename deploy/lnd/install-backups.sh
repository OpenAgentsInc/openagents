#!/usr/bin/env bash
set -euo pipefail
umask 077
commit=${1:?commit required}
binary=${2:?backup binary required}
[[ $commit =~ ^[0-9a-f]{40}$ && $(id -u) == 0 ]] || exit 2
apt-get install -y -qq age tor socat
id oa-lnd-backup >/dev/null 2>&1 || useradd --system --shell /usr/sbin/nologin oa-lnd-backup
runtime=/run/openagents-lnd-backup-install
install -d -m 0700 "$runtime"
export CLOUDSDK_CONFIG="$runtime/gcloud"
mkdir -p "$CLOUDSDK_CONFIG"
if [[ ! -s /etc/openagents-lnd/backup-recipient ]]; then
  # Only the public recipient survives on this host. Recovery uses an operator
  # account with Secret Manager access; the node cannot retrieve this identity.
  age-keygen -o "$runtime/identity" 2>/dev/null
  age-keygen -y "$runtime/identity" > "$runtime/recipient"
  gcloud --quiet secrets versions add lnd-oa-lnd-1-backup-age-key --project=openagentsgemini --data-file="$runtime/identity" >/dev/null
  install -m 0644 "$runtime/recipient" /etc/openagents-lnd/backup-recipient
  shred -u "$runtime/identity"
fi
base="https://raw.githubusercontent.com/OpenAgentsInc/openagents/$commit/deploy/lnd"
curl -fsSL "$base/openagents-lnd-backup-upload" -o /usr/local/sbin/openagents-lnd-backup-upload
chmod 0755 /usr/local/sbin/openagents-lnd-backup-upload
for unit in openagents-lnd-backups.service openagents-lnd-backup-upload.service openagents-lnd-third-party-tower.service; do
  curl -fsSL "$base/$unit" -o "/etc/systemd/system/$unit"
done
install -m 0755 "$binary" /opt/openagents-lnd/lnd-backups
printf '%s\n' "$commit" > /opt/openagents-lnd/BACKUPS_COMMIT
sha256sum /opt/openagents-lnd/lnd-backups > /opt/openagents-lnd/BACKUPS_SHA256
/opt/openagents-lnd/lncli --lnddir=/var/lib/openagents-lnd/lnd --network=mainnet bakemacaroon --root_key_id=203 --ip_address=127.0.0.1 --save_to=/var/lib/openagents-lnd/backup.macaroon info:read offchain:read >/dev/null
chmod 0600 /var/lib/openagents-lnd/backup.macaroon
jq '.macaroon="/run/openagents-lnd-backup/readonly.macaroon" | .output="/run/openagents-lnd-backup/channel.backup"' /etc/openagents-lnd/health.json > /etc/openagents-lnd/backups.json
chmod 0644 /etc/openagents-lnd/backups.json
systemctl daemon-reload
systemctl enable --now openagents-lnd-backups openagents-lnd-backup-upload

# Enable the watchtower client while keeping LND's clearnet peer settings.
curl -fsSL "$base/openagents-lnd-secrets" -o /usr/local/sbin/openagents-lnd-secrets
chmod 0755 /usr/local/sbin/openagents-lnd-secrets
systemctl enable --now tor@default openagents-lnd-third-party-tower
systemctl restart openagents-lnd

#!/usr/bin/env bash
set -euo pipefail
umask 077
commit=${1:?commit required}
binary=${2:?backup binary required}
[[ $commit =~ ^[0-9a-f]{40}$ && $(id -u) == 0 ]] || exit 2
apt-get install -y -qq age
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
for unit in openagents-lnd-backups.service openagents-lnd-backup-upload.service; do
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

# Enable the watchtower client.
curl -fsSL "$base/openagents-lnd-secrets" -o /usr/local/sbin/openagents-lnd-secrets
chmod 0755 /usr/local/sbin/openagents-lnd-secrets
systemctl restart openagents-lnd
for _ in $(seq 1 90); do
  if /opt/openagents-lnd/lncli --lnddir=/var/lib/openagents-lnd/lnd getinfo >/dev/null 2>&1; then
    /opt/openagents-lnd/lncli --lnddir=/var/lib/openagents-lnd/lnd wtclient add 0281f4affcd16ad56a6a5be055d19e24a17f9932d057f61c5403fc0809ebc696aa@tower.mineracks.com:9911 >/dev/null
    exit 0
  fi
  sleep 1
done
exit 1

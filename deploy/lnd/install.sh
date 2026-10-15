#!/usr/bin/env bash
# Run as root on the newly provisioned host, using artifacts from one commit.
set -euo pipefail
umask 077
commit=${1:?commit is required}
health_binary=${2:?health binary is required}
[[ $commit =~ ^[0-9a-f]{40}$ ]] || exit 2
[[ $(id -u) == 0 ]] || { echo 'Run the installer as root.' >&2; exit 2; }
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg jq expect age openssl
if ! command -v gcloud >/dev/null; then
  curl -fsSL https://packages.cloud.google.com/apt/doc/apt-key.gpg | gpg --dearmor --yes -o /usr/share/keyrings/google-cloud.gpg
  echo 'deb [signed-by=/usr/share/keyrings/google-cloud.gpg] https://packages.cloud.google.com/apt cloud-sdk main' > /etc/apt/sources.list.d/google-cloud-sdk.list
  apt-get update -qq
  apt-get install -y -qq google-cloud-cli
fi
for user in oa-lnd oa-lnd-health; do
  if ! id "$user" >/dev/null 2>&1; then useradd --system --home "/var/lib/$user" --shell /usr/sbin/nologin "$user"; fi
done
install -d -m 0755 /var/lib/openagents-lnd /opt/openagents-lnd /etc/openagents-lnd
# Only format a disk with no filesystem. Existing live state is always kept.
if ! blkid /dev/disk/by-id/google-lnd-data >/dev/null 2>&1; then
  mkfs.ext4 -L oa-lnd-data /dev/disk/by-id/google-lnd-data
fi
if ! grep -q '^LABEL=oa-lnd-data ' /etc/fstab; then
  echo 'LABEL=oa-lnd-data /var/lib/openagents-lnd ext4 defaults,nofail 0 2' >> /etc/fstab
fi
mountpoint -q /var/lib/openagents-lnd || mount /var/lib/openagents-lnd
chown oa-lnd:oa-lnd /var/lib/openagents-lnd
chmod 0700 /var/lib/openagents-lnd
install -d -o oa-lnd-health -g oa-lnd-health -m 0750 /var/lib/openagents-lnd-health
cache=/var/lib/openagents-lnd/install
install -d -o root -g root -m 0700 "$cache" "$cache/gnupg"
export GNUPGHOME="$cache/gnupg"
version=v0.21.1-beta
archive="lnd-linux-amd64-$version.tar.gz"
release="https://github.com/lightningnetwork/lnd/releases/download/$version"
for file in "$archive" "manifest-$version.txt" "manifest-roasbeef-$version.sig"; do
  curl -fL --retry 3 "$release/$file" -o "$cache/$file"
done
curl -fsSL "https://raw.githubusercontent.com/lightningnetwork/lnd/$version/scripts/keys/roasbeef.asc" -o "$cache/release-key.asc"
fingerprint=$(gpg --batch --with-colons --show-keys "$cache/release-key.asc" | awk -F: '$1=="fpr"{print $10;exit}')
[[ $fingerprint == A5B61896952D9FDA83BC054CDC42612E89237182 ]] || { echo 'Release signing key does not match.' >&2; exit 1; }
gpg --batch --import "$cache/release-key.asc" >/dev/null 2>&1
gpg --batch --verify "$cache/manifest-roasbeef-$version.sig" "$cache/manifest-$version.txt"
(cd "$cache"; awk -v file="$archive" '$2==file{print}' "manifest-$version.txt" | sha256sum --check --strict -)
tar -xzf "$cache/$archive" -C "$cache"
# Reinstallation must stop the writer before replacing its binary.
systemctl stop openagents-lnd.service 2>/dev/null || true
install -m 0755 "$cache/lnd-linux-amd64-$version/lnd" "$cache/lnd-linux-amd64-$version/lncli" /opt/openagents-lnd/
install -m 0755 "$health_binary" /opt/openagents-lnd/lnd-health
printf '%s\n' "$commit" > /opt/openagents-lnd/COMMIT
base="https://raw.githubusercontent.com/OpenAgentsInc/openagents/$commit/deploy/lnd"
for script in openagents-lnd-secrets; do curl -fsSL "$base/$script" -o "/usr/local/sbin/$script"; chmod 0755 "/usr/local/sbin/$script"; done
for unit in openagents-lnd.service openagents-lnd-health.service openagents-lnd-health.timer openagents-lnd-alert.service; do
  curl -fsSL "$base/$unit" -o "/etc/systemd/system/$unit"
done
# Create a password only for a node that has never been initialized. The
# helper refuses if the existing Secret Manager password would be reused.
if [[ ! -f /var/lib/openagents-lnd/lnd/data/chain/bitcoin/mainnet/wallet.db ]]; then
  /usr/local/sbin/openagents-lnd-secrets --initialize-password
fi
systemctl daemon-reload
systemctl enable --now openagents-lnd
if [[ ! -f /var/lib/openagents-lnd/lnd/data/chain/bitcoin/mainnet/wallet.db ]]; then
  # lncli's terminal transcript contains the fresh recovery words. It goes
  # straight to the seed secret; neither stdout nor the service log gets it.
  export LND_CEREMONY=/run/openagents-lnd/seed-ceremony
  expect <<'EXPECT'
set timeout 120
log_user 0
log_file -noappend -a $env(LND_CEREMONY)
set f [open "/run/openagents-lnd/secrets/password" r]
set password [read $f]
close $f
spawn -noecho runuser -u oa-lnd -- /opt/openagents-lnd/lncli --lnddir=/var/lib/openagents-lnd/lnd --network=mainnet create
expect -re "Input wallet password:"
send -- "$password\r"
expect -re "Confirm.*password:"
send -- "$password\r"
expect -re "Enter y/n"
send -- "n\r"
expect -re "passphrase.*:"
send -- "\r"
expect eof
set result [wait]
exit [lindex $result 3]
EXPECT
  gcloud --quiet secrets versions add lnd-oa-lnd-1-seed --project=openagentsgemini --data-file="$LND_CEREMONY" >/dev/null
  shred -u "$LND_CEREMONY"
fi
lcli() { runuser -u oa-lnd -- /opt/openagents-lnd/lncli --lnddir=/var/lib/openagents-lnd/lnd --network=mainnet "$@"; }
ready=0
for _ in $(seq 1 180); do
  if lcli getinfo > "$cache/info.json" 2>/dev/null; then ready=1; break; fi
  sleep 1
done
[[ $ready == 1 ]] || { echo 'LND did not become ready.' >&2; exit 1; }
pay_ip=10.128.0.46
for role in invoice readonly payout ops monitor; do
  case $role in
    invoice) id=101; ip="$pay_ip"; permissions=(info:read invoices:read invoices:write);;
    readonly) id=102; ip="$pay_ip"; permissions=(info:read invoices:read offchain:read onchain:read peers:read address:read);;
    payout) id=103; ip="$pay_ip"; permissions=(info:read offchain:read offchain:write);;
    ops) id=104; ip=127.0.0.1; permissions=(info:read offchain:read offchain:write onchain:read onchain:write peers:read peers:write address:read address:write);;
    monitor) id=202; ip=127.0.0.1; permissions=(info:read offchain:read onchain:read peers:read);;
  esac
  file="/var/lib/openagents-lnd/$role.macaroon"
  lcli bakemacaroon --root_key_id="$id" --ip_address="$ip" --save_to="$file" "${permissions[@]}" >/dev/null
  chmod 0600 "$file"
  if [[ $role != monitor ]]; then
    gcloud --quiet secrets versions add "lnd-oa-lnd-1-macaroon-$role" --project=openagentsgemini --data-file="$file" >/dev/null
  fi
done
gcloud --quiet secrets versions add lnd-oa-lnd-1-tls-cert --project=openagentsgemini --data-file=/var/lib/openagents-lnd/lnd/tls.cert >/dev/null
# Only this public fingerprint and local paths enter the monitor's settings.
pin=$(openssl x509 -in /var/lib/openagents-lnd/lnd/tls.cert -outform DER | sha256sum | cut -d' ' -f1)
LND_CERT_PIN="$pin" python3 - <<'PY'
import json,os,pathlib
settings={'endpoint':'https://127.0.0.1:10009','certificate_sha256':os.environ['LND_CERT_PIN'],'macaroon':'/run/openagents-lnd-health/readonly.macaroon','output':'/var/lib/openagents-lnd-health/health.json','minimum_inbound_msat':100000000,'hot_cap_sats':0}
pathlib.Path('/etc/openagents-lnd/health.json').write_text(json.dumps(settings,indent=2)+'\n')
PY
chmod 0644 /etc/openagents-lnd/health.json
systemctl enable --now openagents-lnd-health.timer
systemctl start openagents-lnd-health.service || true
/opt/openagents-lnd/lnd --version
cat /opt/openagents-lnd/COMMIT
jq '{identity_pubkey,synced_to_chain,synced_to_graph,num_active_channels,num_peers,block_height}' "$cache/info.json"

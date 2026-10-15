#!/usr/bin/env bash
# Unfunded standalone tower; never import a payment node's seed.
set -euo pipefail
umask 077
[[ $(id -u) == 0 ]] || exit 2
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq ca-certificates curl gnupg jq
id oa-lnd-tower >/dev/null 2>&1 || useradd --system --shell /usr/sbin/nologin oa-lnd-tower
cache=/var/lib/openagents-lnd-tower/install
install -d -m 0700 "$cache" "$cache/gnupg"
install -d -o oa-lnd-tower -g oa-lnd-tower -m 0700 /var/lib/openagents-lnd-tower/node
install -d -m 0755 /opt/openagents-lnd-tower /etc/openagents-lnd-tower
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
systemctl stop openagents-lnd-tower 2>/dev/null || true
install -m 0755 "$cache/lnd-linux-amd64-$version/lnd" "$cache/lnd-linux-amd64-$version/lncli" /opt/openagents-lnd-tower/
# Tokens and RPC credentials stay out of process arguments and output.
python3 - <<'CONFIG'
import json,urllib.request,base64,pathlib
req=urllib.request.Request('http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token',headers={'Metadata-Flavor':'Google'})
token=json.load(urllib.request.urlopen(req))['access_token']
req=urllib.request.Request('https://secretmanager.googleapis.com/v1/projects/openagentsgemini/secrets/oa-bitcoind-rpc-creds/versions/latest:access',headers={'Authorization':'Bearer '+token})
raw=base64.b64decode(json.load(urllib.request.urlopen(req))['payload']['data']).decode()
fields=dict(line.split('=',1) for line in raw.splitlines() if '=' in line and not line.startswith('#'))
private=urllib.request.urlopen(urllib.request.Request('http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/ip',headers={'Metadata-Flavor':'Google'})).read().decode()
text=f'''[Application Options]
lnddir=/var/lib/openagents-lnd-tower/node
listen=127.0.0.1:9735
rpclisten=127.0.0.1:10009
restlisten=127.0.0.1:8080
noseedbackup=true
[Bitcoin]
bitcoin.mainnet=true
bitcoin.node=bitcoind
[Bitcoind]
bitcoind.rpchost=10.42.0.2:8332
bitcoind.rpcuser={fields['rpcuser']}
bitcoind.rpcpass={fields['rpcpassword']}
bitcoind.zmqpubrawblock=tcp://10.42.0.2:28332
bitcoind.zmqpubrawtx=tcp://10.42.0.2:28333
[watchtower]
watchtower.active=true
watchtower.listen={private}:9911
watchtower.externalip={private}:9911
'''
pathlib.Path('/etc/openagents-lnd-tower/lnd.conf').write_text(text)
CONFIG
chown root:oa-lnd-tower /etc/openagents-lnd-tower/lnd.conf
chmod 0640 /etc/openagents-lnd-tower/lnd.conf
cat > /etc/systemd/system/openagents-lnd-tower.service <<'UNIT'
[Unit]
Description=OpenAgents private LND watchtower
After=network-online.target
Wants=network-online.target
[Service]
User=oa-lnd-tower
Group=oa-lnd-tower
ExecStart=/opt/openagents-lnd-tower/lnd --configfile=/etc/openagents-lnd-tower/lnd.conf
Restart=on-failure
RestartSec=5
TimeoutStopSec=120
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=/var/lib/openagents-lnd-tower/node
[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable --now openagents-lnd-tower
for _ in $(seq 1 120); do
  if /opt/openagents-lnd-tower/lncli --lnddir=/var/lib/openagents-lnd-tower/node tower info; then exit 0; fi
  sleep 1
done
exit 1

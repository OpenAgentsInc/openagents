#!/usr/bin/env bash
# Install the read-only schedule without replacing reviewed policy.
set -euo pipefail
[[ $EUID == 0 && $# == 3 ]] || { echo 'Usage: sudo install-autoloop.sh COMMIT BINARY SHA256' >&2; exit 2; }
revision=$1
artifact=$2
expected=$3
[[ $revision =~ ^[a-f0-9]{40}$ && $expected =~ ^[a-f0-9]{64}$ ]] || exit 2
[[ $(sha256sum "$artifact" | cut -d' ' -f1) == "$expected" ]] || exit 1
source_dir=$(cd "$(dirname "$0")" && pwd)
# Installing this service never enables a Loop connection or spending.
install -m 0755 "$artifact" /opt/openagents-lnd/lnd-autoloop
if [[ ! -e /etc/openagents-lnd/channel-policy.toml ]]; then
  install -o root -g oa-lnd-health -m 0640 "$source_dir/channel-policy.toml" /etc/openagents-lnd/channel-policy.toml
fi
if [[ ! -e /etc/openagents-lnd/autoloop.json ]]; then
  python3 - "$source_dir/autoloop.example.json" <<'PY'
import json,sys,hashlib,subprocess,os,grp
s=json.load(open(sys.argv[1]))
s['lnd']['certificate_sha256']=hashlib.sha256(subprocess.check_output(['openssl','x509','-in','/var/lib/openagents-lnd/tls.cert','-outform','DER'])).hexdigest()
p='/etc/openagents-lnd/autoloop.json'
with open(p,'x') as f:json.dump(s,f,indent=2);f.write('\n')
os.chown(p,0,grp.getgrnam('oa-lnd-health').gr_gid);os.chmod(p,0o640)
PY
fi
install -m 0644 "$source_dir/openagents-lnd-autoloop.service" "$source_dir/openagents-lnd-autoloop.timer" /etc/systemd/system/
printf '%s\n' "$revision" > /opt/openagents-lnd/AUTOLOOP_COMMIT
systemctl daemon-reload
systemctl enable --now openagents-lnd-autoloop.timer
echo 'Liquidity review scheduled; existing policy was preserved.'

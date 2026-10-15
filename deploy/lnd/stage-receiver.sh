#!/usr/bin/env bash
# Stage a checked binary and receive-only credentials. Never selects LND traffic.
set -euo pipefail
umask 077
commit=${1:?commit required}
binary=${2:?checked openagents binary required}
[[ $(id -u) == 0 && $commit =~ ^[0-9a-f]{40}$ ]] || exit 2
id openagents-pay >/dev/null
install -d -m 0755 "/opt/openagents-pay/lnd-candidates/$commit"
install -m 0755 "$binary" "/opt/openagents-pay/lnd-candidates/$commit/openagents"
base="https://raw.githubusercontent.com/OpenAgentsInc/openagents/$commit/deploy/lnd"
curl -fsSL "$base/openagents-lnd-receive-secrets" -o /usr/local/sbin/openagents-lnd-receive-secrets
chmod 0755 /usr/local/sbin/openagents-lnd-receive-secrets
curl -fsSL "$base/openagents-lnd-receive-secrets.service" -o /etc/systemd/system/openagents-lnd-receive-secrets.service
for unit in openagents-pay-front openagents-pay-reconcile; do
 install -d -m 0755 "/etc/systemd/system/$unit.service.d"
 cat > "/etc/systemd/system/$unit.service.d/lnd-receive.conf" <<UNIT
[Unit]
Wants=openagents-lnd-receive-secrets.service
After=openagents-lnd-receive-secrets.service
UNIT
done
systemctl daemon-reload
systemctl enable --now openagents-lnd-receive-secrets
printf 'Staged LND receiver from %s. Active binary and routes were not changed.\n' "$commit"

#!/bin/sh
# Firm reason: 5 (a thin wrapper: openssl and systemctl around Caddy's certificate)
# The staging unit's WebTransport (#13176) on the room VM, as root. UDP 443 has one owner: the
# staging unit, oa-room@staging, answering for room-staging.openagents.com (the production shards
# keep WebSocket only until the transport is the default, docs/verse/webtransport-server.md).
# It copies Caddy's certificate for that name into /etc/oa-room/wt/ (the key as PKCS#8, as the
# room's TLS reads it), gives the unit the two files and UDP 443 (a drop-in), and restarts the unit
# when either changed. A daily timer runs it again, so Caddy's renewals reach the room.
#   sh wt-cert.sh --install   install this script and its timer, then run it (vm-deploy.sh wt)
#   sh wt-cert.sh             copy and restart when changed (the timer)
set -eu
name=room-staging.openagents.com unit=oa-room@staging
if [ "${1:-}" = --install ]; then
  install -m 755 "$0" /opt/oa-room/wt-cert.sh
  printf '[Unit]\nDescription=The room staging unit'"'"'s WebTransport certificate\n\n[Service]\nType=oneshot\nExecStart=/bin/sh /opt/oa-room/wt-cert.sh\n' > /etc/systemd/system/oa-room-wt-cert.service
  printf '[Unit]\nDescription=The room staging unit'"'"'s WebTransport certificate, daily\n\n[Timer]\nOnCalendar=daily\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n' > /etc/systemd/system/oa-room-wt-cert.timer
  systemctl daemon-reload
  systemctl enable --now oa-room-wt-cert.timer >/dev/null 2>&1
  exec sh /opt/oa-room/wt-cert.sh
fi
src=""
for d in /var/lib/caddy/.local/share/caddy/certificates/*/"$name"; do [ -s "$d/$name.crt" ] && src="$d"; done
[ -n "$src" ] || { echo "wt-cert: Caddy has no certificate for $name yet"; exit 1; }
t="$(mktemp -d)"; trap 'rm -rf "${t:?}"' EXIT
cp "$src/$name.crt" "$t/cert.pem"
openssl pkcs8 -topk8 -nocrypt -in "$src/$name.key" -out "$t/key.pem"
openssl pkey -in "$t/key.pem" -noout -text 2>/dev/null | grep -q 'prime256v1' || { echo "wt-cert: the key isn't ECDSA P-256"; exit 1; }
install -d -m 700 /etc/oa-room/wt
changed=0
for f in cert.pem key.pem; do cmp -s "$t/$f" "/etc/oa-room/wt/$f" || { install -m 600 "$t/$f" "/etc/oa-room/wt/$f"; changed=1; }; done
drop=/etc/systemd/system/$unit.service.d/wt.conf
want="$(printf '[Service]\nAmbientCapabilities=CAP_NET_BIND_SERVICE\nLoadCredential=wt-cert:/etc/oa-room/wt/cert.pem\nLoadCredential=wt-key:/etc/oa-room/wt/key.pem\nEnvironment=OA_ROOM_WT_CERT=%%d/wt-cert OA_ROOM_WT_KEY=%%d/wt-key OA_ROOM_WT_PORT=443\n')"
if [ "$(cat "$drop" 2>/dev/null)" != "$want" ]; then
  install -d -m 755 "$(dirname "$drop")"; printf '%s\n' "$want" > "$drop"; systemctl daemon-reload; changed=1
fi
if [ "$changed" = 1 ] && systemctl is-active -q "$unit"; then systemctl restart "$unit"; echo "wt-cert: $unit restarted with the certificate"; else echo "wt-cert: unchanged"; fi

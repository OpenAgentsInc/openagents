#!/bin/sh
# Firm reason: 5 (a thin wrapper: openssl and systemctl around Caddy's certificates)
# The room units' WebTransport (#13176, #13187) on the room VM, as root. Each unit owns one UDP port:
# the staging unit, oa-room@staging, UDP 443 for room-staging.openagents.com; production shard K,
# oa-room@sK, UDP 443K for room.openagents.com (its endpoint's port, setup.sh; Caddy has that TCP
# port for its WebSocket). It copies Caddy's certificate for each name into /etc/oa-room/wt/NAME/
# (the key as PKCS#8, as the room's TLS reads it), gives each unit its two files and its port (a
# drop-in). The staging unit restarts when either changed. A production shard isn't cut off its
# players by a renewal: it takes the new files at its next restart (a deploy drains it first,
# vm-deploy.sh), unless the certificate it runs expires within a week, when it restarts here. Caddy
# renews 30 days ahead. A daily timer runs it again.
#   sh wt-cert.sh --install   install this script and its timer, then run it (vm-deploy.sh wt)
#   sh wt-cert.sh             copy and restart when changed (the timer)
set -eu
if [ "${1:-}" = --install ]; then
  install -m 755 "$0" /opt/oa-room/wt-cert.sh
  printf '[Unit]\nDescription=The room units'"'"' WebTransport certificates\n\n[Service]\nType=oneshot\nExecStart=/bin/sh /opt/oa-room/wt-cert.sh\n' > /etc/systemd/system/oa-room-wt-cert.service
  printf '[Unit]\nDescription=The room units'"'"' WebTransport certificates, daily\n\n[Timer]\nOnCalendar=daily\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n' > /etc/systemd/system/oa-room-wt-cert.timer
  systemctl daemon-reload
  systemctl enable --now oa-room-wt-cert.timer >/dev/null 2>&1
  exec sh /opt/oa-room/wt-cert.sh
fi
t="$(mktemp -d)"; trap 'rm -rf "${t:?}"' EXIT
# NAME: Caddy's certificate for it into /etc/oa-room/wt/NAME; prints 0 unchanged, 1 changed, 2 changed
# and the one it replaced expires within a week
copy() {
  name="$1" src=""
  for d in /var/lib/caddy/.local/share/caddy/certificates/*/"$name"; do [ -s "$d/$name.crt" ] && src="$d"; done
  [ -n "$src" ] || { echo "wt-cert: Caddy has no certificate for $name yet" >&2; exit 1; }
  mkdir -p "$t/$name"
  cp "$src/$name.crt" "$t/$name/cert.pem"
  openssl pkcs8 -topk8 -nocrypt -in "$src/$name.key" -out "$t/$name/key.pem"
  openssl pkey -in "$t/$name/key.pem" -noout -text 2>/dev/null | grep -q 'prime256v1' || { echo "wt-cert: $name's key isn't ECDSA P-256" >&2; exit 1; }
  install -d -m 700 /etc/oa-room/wt "/etc/oa-room/wt/$name"
  c=0
  if ! cmp -s "$t/$name/cert.pem" "/etc/oa-room/wt/$name/cert.pem"; then
    c=1; [ -s "/etc/oa-room/wt/$name/cert.pem" ] && ! openssl x509 -checkend 604800 -noout -in "/etc/oa-room/wt/$name/cert.pem" >/dev/null 2>&1 && c=2
  fi
  for f in cert.pem key.pem; do cmp -s "$t/$name/$f" "/etc/oa-room/wt/$name/$f" || { install -m 600 "$t/$name/$f" "/etc/oa-room/wt/$name/$f"; [ "$c" = 0 ] && c=1; }; done
  echo "$c"
}
# UNIT NAME PORT CHANGED: the unit's drop-in; the staging unit restarts when it or the certificate
# changed, a production shard only when its certificate was about to expire (CHANGED 2)
unit() {
  u="oa-room@$1" drop="/etc/systemd/system/oa-room@$1.service.d/wt.conf" changed="$4"
  want="$(printf '[Service]\nAmbientCapabilities=CAP_NET_BIND_SERVICE\nLoadCredential=wt-cert:/etc/oa-room/wt/%s/cert.pem\nLoadCredential=wt-key:/etc/oa-room/wt/%s/key.pem\nEnvironment=OA_ROOM_WT_CERT=%%d/wt-cert OA_ROOM_WT_KEY=%%d/wt-key OA_ROOM_WT_PORT=%s\n' "$2" "$2" "$3")"
  if [ "$(cat "$drop" 2>/dev/null)" != "$want" ]; then
    install -d -m 755 "$(dirname "$drop")"; printf '%s\n' "$want" > "$drop"; systemctl daemon-reload; [ "$changed" = 0 ] && changed=1
  fi
  if [ "$1" != staging ] && [ "$changed" = 1 ]; then echo "wt-cert: $u takes UDP $3 and its certificate at its next restart"; return; fi
  if [ "$changed" != 0 ] && systemctl is-active -q "$u"; then systemctl restart "$u"; echo "wt-cert: $u restarted (UDP $3)"; else echo "wt-cert: $u unchanged (UDP $3)"; fi
}
# the certificates moved from /etc/oa-room/wt/ to a directory per name
rm -f /etc/oa-room/wt/cert.pem /etc/oa-room/wt/key.pem
s="$(copy room-staging.openagents.com)"; p="$(copy room.openagents.com)"
unit staging room-staging.openagents.com 443 "$s"
for k in 1 2 3 4 5 6; do unit "s$k" room.openagents.com "$((4430 + k))" "$p"; done

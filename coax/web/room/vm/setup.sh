#!/bin/sh
# Firm reason: 5 (a thin wrapper: installs the room VM's packages, unit and TLS front, as root)
# Run on the room VM by vm-deploy.sh provision, from a directory holding this file, oa-room@.service
# and Caddyfile. It can run again: every step leaves what is already there.
set -eu
export DEBIAN_FRONTEND=noninteractive
need=""
for p in caddy gcc libc6-dev binutils curl ca-certificates; do dpkg -s "$p" >/dev/null 2>&1 || need="$need $p"; done
if [ -n "$need" ]; then apt-get update -q >/dev/null && apt-get install -y -q --no-install-recommends $need >/dev/null; fi
install -d -m 755 /opt/oa-room /opt/oa-room/releases /opt/oa-room/bin /etc/oa-room
[ -f /etc/oa-room/secret.env ] || install -m 600 /dev/null /etc/oa-room/secret.env
for k in 1 2 3 4 5 6; do
  # OA_ROOM_ENDPOINT: the directory hands pages this shard's own address (#13179)
  printf 'PORT=%s\nOA_ROOM_SHARD=%s\nOA_ROOM_DIR_GEN=1\nOA_ROOM_TICKET_REQUIRED=1\nOA_ROOM_ENDPOINT=wss://room.openagents.com/api/room/s%s/\n' "$((8080 + k))" "$k" "$k" > "/etc/oa-room/s$k.env"
  ln -sfn /opt/oa-room/current/room "/opt/oa-room/bin/s$k"
done
# the staging unit reports to the staging API (its directory, control, vitals and checkpoints)
printf 'PORT=8090\nOA_ROOM_SHARD=1\nOA_ROOM_DIR_GEN=1\nOA_ROOM_TICKET_REQUIRED=1\nOA_ROOM_API=https://staging.openagents.com\nOA_ROOM_CONTROL_URL=https://staging.openagents.com/api/playtest/room-control\nOA_ROOM_ENDPOINT=wss://room-staging.openagents.com/api/room/s1/\n' > /etc/oa-room/staging.env
ln -sfn /opt/oa-room/staging/room /opt/oa-room/bin/staging
install -m 644 oa-room@.service /etc/systemd/system/oa-room@.service
install -m 644 Caddyfile /etc/caddy/Caddyfile
systemctl daemon-reload
systemctl enable --now caddy >/dev/null 2>&1
systemctl reload caddy
echo "setup: done"

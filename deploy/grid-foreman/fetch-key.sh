#!/bin/sh
# Writes the Grid Foreman's Nostr key from Secret Manager to $1 (mode 0600,
# owned by grid-foreman). Run as root by the unit's ExecStartPre. Never
# prints the key.
set -eu
out=$1
secret=${GRID_FOREMAN_SECRET:-grid-foreman-nostr-key}
umask 077
tmp=$(mktemp "$out.XXXXXX")
trap 'rm -f "$tmp"' EXIT
gcloud secrets versions access latest --secret="$secret" --quiet > "$tmp"
test "$(tr -d '\n' < "$tmp" | wc -c)" -eq 64
chown grid-foreman:grid-foreman "$tmp"
chmod 600 "$tmp"
mv -f "$tmp" "$out"
trap - EXIT

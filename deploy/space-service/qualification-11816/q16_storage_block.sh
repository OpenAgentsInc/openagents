#!/bin/sh
# #11816: block all non-private HTTPS from this worker (host and pods) for at most $1 seconds, then restore.
# Object storage is the worker's only external HTTPS dependency during the check.
set -eu
secs=${1:?seconds}
[ "$(hostname -s)" = oa-space-service-worker ] || { echo "wrong host" >&2; exit 2; }
rule4() { iptables "$1" "$2" ! -d 10.0.0.0/8 -p tcp --dport 443 -m comment --comment q16-storage -j REJECT; }
rule6() { ip6tables "$1" "$2" -p tcp --dport 443 -m comment --comment q16-storage -j REJECT; }
add() { for ch in OUTPUT FORWARD; do rule4 -I $ch; rule6 -I $ch; done; }
del() { for ch in OUTPUT FORWARD; do while rule4 -D $ch 2>/dev/null; do :; done; while rule6 -D $ch 2>/dev/null; do :; done; done; }
trap del EXIT INT TERM
add
echo "{\"blocked_at\": $(date +%s), \"scope\": \"all non-10/8 IPv4 and all IPv6 TCP 443, host OUTPUT and pod FORWARD\"}"
if curl -s -m 5 -o /dev/null https://storage.googleapis.com/ ; then echo '{"probe_while_blocked":"reachable"}'; else echo '{"probe_while_blocked":"refused"}'; fi
sleep "$secs"
del; trap - EXIT
if curl -s -m 5 -o /dev/null https://storage.googleapis.com/ ; then echo "{\"restored_at\": $(date +%s), \"probe_after\":\"reachable\"}"; else echo "{\"restored_at\": $(date +%s), \"probe_after\":\"refused\"}"; fi

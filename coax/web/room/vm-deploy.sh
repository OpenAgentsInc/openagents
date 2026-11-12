#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: gcloud, ssh and curl around the room VM)
# The room server on a GCE VM (docs/deployment/room-vm.md, #13174): oa-room-1, e2-standard-2 in
# us-central1-a on the static address oa-room-1-ip. It runs production's six shards as systemd units
# oa-room@s1 to oa-room@s6 (ports 8081 to 8086) and a staging unit oa-room@staging (8090, reporting
# to the staging API). The HTTPS load balancer reaches each unit through the unmanaged instance group
# oa-room-vm-ig (named ports s1..s6, staging) and the backend services oa-room-vm-s1..s6 and
# oa-room-vm-staging. Caddy on the VM holds Let's Encrypt certificates for room.openagents.com and
# room-staging.openagents.com on TCP 443 and routes /api/room/sK/ to shard K, for direct use.
#
#   coax/web/room/vm-deploy.sh provision          the VM, its firewall, DNS, TLS and backends (idempotent)
#   coax/web/room/vm-deploy.sh secret             the room keys (room-keys.md), from Secret Manager to the VM only
#   coax/web/room/vm-deploy.sh deploy --staging   stage this checkout on a build host, compile it on the
#                                                 VM, run it as oa-room@staging and check its health
#   coax/web/room/vm-deploy.sh deploy             promote the release staging runs to the production
#                                                 shards, one at a time: drain, restart, healthy
#   coax/web/room/vm-deploy.sh deploy --staging --seed-tick-us N
#                                                 a seeded bad build for a canary's own check: every tick
#                                                 busy N microseconds more. Its release is named "-seeded",
#                                                 and no production shard ever takes one
#   coax/web/room/vm-deploy.sh rollback           the previous production release, the same way
#   coax/web/room/vm-deploy.sh rollback --staging the staging unit back on the release before its last
#   coax/web/room/vm-deploy.sh canary K           production shard K alone onto staging's release (drained,
#                                                 restarted, healthy): the canary of a release
#                                                 (coax/web/release-set.coax, #13299); the rest follow with
#                                                 `deploy`, which leaves the canary running
#   coax/web/room/vm-deploy.sh canary-back        the canary shard back on production's release
#   coax/web/room/vm-deploy.sh releases           each link's release, a line each: current, previous,
#                                                 staging, staging-previous, canary
#   coax/web/room/vm-deploy.sh switch vm|run      production's /api/room/ rules (openagents.com and
#                                                 everglade.world) to the VM or back to Cloud Run
#   coax/web/room/vm-deploy.sh switch staging-vm|staging-run    the same for staging.openagents.com
#   coax/web/room/vm-deploy.sh drain K [SECONDS]  drain production shard K on the VM
#   coax/web/room/vm-deploy.sh kill K|staging     SIGKILL one unit (the failover check: systemd starts it
#                                                 again, its pages come back through the directory)
#   coax/web/room/vm-deploy.sh crash K|staging    the crash test (#13298): SIGUSR2 to one unit's room process,
#                                                 which traps in Coax on purpose; it dies of SIGABRT, systemd
#                                                 starts it again, and the new process posts the crash report
#   coax/web/room/vm-deploy.sh symbols [NAME]     keep a release's debug file in the symbols bucket, named by
#                                                 its build ID (deploy --staging does this; NAME: staging's)
#   coax/web/room/vm-deploy.sh resolve BUILD OFFSET...   a crash report's "f ? +0xOFFSET" frames, or any
#                                                 build's: the function and FILE:LINE of each, from the kept symbols
#   coax/web/room/vm-deploy.sh setup              the VM's packages, units, env files and Caddy again
#   coax/web/room/vm-deploy.sh status
#   coax/web/room/vm-deploy.sh wt                 the units' WebTransport: staging on UDP 443, shard K on
#                                                 UDP 443K, Caddy's certificates renewed daily (vm/wt-cert.sh);
#                                                 then each running shard drained and restarted onto it
#
# Player state lives in the API, not on the VM: each room posts its seats' HP, XP, WANTED and
# checkpoints to openagents.com (room-host.c vitals_post, checkpoint_post), as Cloud Run's do, so a
# restart or a move between hosts loses nothing.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
cd "$root"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 zone=us-central1-a vm=oa-room-1 address=oa-room-1-ip shards=6
host=room.openagents.com staging_host=room-staging.openagents.com
group=oa-room-vm-ig hc=oa-room-vm-hc map=coaxial-openagents-web-staging
scratch="$(mktemp -d)"; trap 'rm -rf "${scratch:?}"' EXIT
g() { gcloud "$@" --project "$project" --quiet; }
# A command on the VM as its OS Login user (root through sudo), over IAP: gcloud's notices go to a log
# that is shown only when the command fails.
on_vm() {
  if ! g compute ssh "$vm" --zone "$zone" --tunnel-through-iap --command "$1" 2>"$scratch/ssh.err"; then
    grep -v -i -E "numpy|increasing_the_tcp|^WARNING: *$|^$" "$scratch/ssh.err" >&2 || true; return 1
  fi
}
exists() { g "$@" --format='value(name)' >/dev/null 2>&1; }
ip() { g compute addresses describe "$address" --region "$region" --format='value(address)'; }
backend() { echo "oa-room-vm-$1"; }

health() {   # the body of shard K's /health on the VM through Caddy (K = staging for the staging unit)
  if [[ "$1" == staging ]]; then curl -s -m 10 "https://$staging_host/health" || true
  else curl -s -m 10 "https://$host/api/room/s$1/health" || true; fi
}
wait_healthy() {   # K [SECONDS]
  local k="$1" until=$((SECONDS + ${2:-120})) want="shard $1," h
  [[ "$k" == staging ]] && want="shard 1,"
  while (( SECONDS < until )); do
    h="$(health "$k")"
    if [[ "$h" == ok*"$want"* ]]; then echo "room-vm: $k healthy"; return 0; fi
    sleep 1
  done
  echo "room-vm: $k is not healthy after ${2:-120} s: $(head -c 200 <<<"$h")" >&2; return 1
}

dns() {   # NAME: an A record to the VM's address (Cloudflare, not proxied)
  [[ -n "${CLOUDFLARE_API_TOKEN:-}" ]] || { set -a; . "$HOME/work/.secrets/cloudflare-openagents.env"; set +a; }
  python3 -I - "$1" "$(ip)" <<'PY'
import json, os, sys, urllib.request
name, address = sys.argv[1:]
token = os.environ["CLOUDFLARE_API_TOKEN"]
def cf(method, path, body=None):
    r = urllib.request.Request("https://api.cloudflare.com/client/v4" + path, method=method,
        data=json.dumps(body).encode() if body else None,
        headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
    with urllib.request.urlopen(r, timeout=30) as x: return json.load(x)["result"]
zone = cf("GET", "/zones?name=openagents.com")[0]["id"]
found = cf("GET", f"/zones/{zone}/dns_records?name={name}")
body = {"type": "A", "name": name, "content": address, "ttl": 300, "proxied": False}
if found and found[0]["content"] == address and found[0]["type"] == "A": print(f"dns: {name} A {address}")
elif found: cf("PUT", f"/zones/{zone}/dns_records/{found[0]['id']}", body); print(f"dns: {name} A {address} (updated)")
else: cf("POST", f"/zones/{zone}/dns_records", body); print(f"dns: {name} A {address} (created)")
PY
}

has() { g secrets describe "$1" >/dev/null 2>&1; }
# The room keys, piped from Secret Manager into the VM's root-only files; never printed. Each is used when
# its secret exists (#13260, docs/deployment/room-keys.md): the shared seat-ticket key (oa-room-ticket-key,
# until the owner deletes it), the API's v5 public keys (oa-room-ticket-pubkeys) into secret.env, and
# shard K's own write key (oa-room-write-key-sK) into sK-write.env. A running shard reads them at its restart.
secret() {
  { if has oa-room-ticket-key; then printf OA_ROOM_TICKET_KEY=; g secrets versions access latest --secret oa-room-ticket-key; echo; fi
    if has oa-room-ticket-pubkeys; then printf OA_ROOM_TICKET_PUBKEYS=; g secrets versions access latest --secret oa-room-ticket-pubkeys; echo; fi; } |
    on_vm 'sudo sh -c "umask 077; cat > /etc/oa-room/secret.env.new && mv /etc/oa-room/secret.env.new /etc/oa-room/secret.env"'
  for k in 1 2 3 4 5 6; do
    if has "oa-room-write-key-s$k"; then
      { printf OA_ROOM_WRITE_KEY=; g secrets versions access latest --secret "oa-room-write-key-s$k"; echo; } |
        on_vm "sudo sh -c 'umask 077; cat > /etc/oa-room/s$k-write.env.new && mv /etc/oa-room/s$k-write.env.new /etc/oa-room/s$k-write.env'"
    else on_vm "sudo rm -f /etc/oa-room/s$k-write.env"
    fi
  done
  # The staging unit is shard 1 (staging.env OA_ROOM_SHARD=1) reporting to the staging API, which holds
  # the same write master, so its key is shard 1's. Without it, once oa-room-ticket-key is gone the
  # staging API refuses every write from staging (401).
  if has oa-room-write-key-s1; then
    { printf OA_ROOM_WRITE_KEY=; g secrets versions access latest --secret oa-room-write-key-s1; echo; } |
      on_vm "sudo sh -c 'umask 077; cat > /etc/oa-room/staging-write.env.new && mv /etc/oa-room/staging-write.env.new /etc/oa-room/staging-write.env'"
  else on_vm "sudo rm -f /etc/oa-room/staging-write.env"
  fi
  echo "room-vm: the room keys are on $vm (/etc/oa-room/secret.env, sK-write.env and staging-write.env, root only)"
}

setup() {   # the VM's packages, unit, env files and Caddy (vm/setup.sh); a running unit keeps its env until restarted
  COPYFILE_DISABLE=1 tar -C "$here/vm" -czf - oa-room@.service Caddyfile setup.sh |
    on_vm 'd=$(mktemp -d) && tar -xzf - -C "$d" 2>/dev/null && cd "$d" && sudo sh setup.sh'
}

provision() {
  exists compute addresses describe "$address" --region "$region" || g compute addresses create "$address" --region "$region" --network-tier PREMIUM
  # the room's ports only: TCP and UDP 443 to anyone (staging's), and 4431-4436 (shard K's own
  # endpoint, 443K: its WebSocket through Caddy on TCP, its WebTransport on UDP, #13187), the shards' ports to the load balancer and its
  # health checks, SSH through IAP; everything else is refused (the default network allows SSH from anywhere)
  exists compute firewall-rules describe oa-room-deny-other-ingress ||
    g compute firewall-rules create oa-room-deny-other-ingress --network default --direction INGRESS --action DENY --rules all --source-ranges 0.0.0.0/0 --target-tags oa-room --priority 1000
  exists compute firewall-rules describe oa-room-iap-ssh ||
    g compute firewall-rules create oa-room-iap-ssh --network default --direction INGRESS --action ALLOW --rules tcp:22 --source-ranges 35.235.240.0/20 --target-tags oa-room --priority 900
  if exists compute firewall-rules describe oa-room-public; then
    g compute firewall-rules update oa-room-public --rules tcp:443,udp:443,tcp:4431-4436,udp:4431-4436 >/dev/null
  else
    g compute firewall-rules create oa-room-public --network default --direction INGRESS --action ALLOW --rules tcp:443,udp:443,tcp:4431-4436,udp:4431-4436 --source-ranges 0.0.0.0/0 --target-tags oa-room --priority 900
  fi
  exists compute firewall-rules describe oa-room-lb ||
    g compute firewall-rules create oa-room-lb --network default --direction INGRESS --action ALLOW --rules tcp:8081-8090 --source-ranges 130.211.0.0/22,35.191.0.0/16 --target-tags oa-room --priority 900
  # no service account: the VM calls no Google API (its one secret arrives over SSH)
  exists compute instances describe "$vm" --zone "$zone" ||
    g compute instances create "$vm" --zone "$zone" --machine-type e2-standard-2 --image-family debian-12 --image-project debian-cloud \
      --boot-disk-size 20GB --boot-disk-type pd-balanced --address "$(ip)" --network-tier PREMIUM --tags oa-room \
      --labels app=oa-room,owner=openagents --no-service-account --no-scopes --maintenance-policy MIGRATE --restart-on-failure \
      --shielded-secure-boot --metadata enable-oslogin=TRUE
  dns "$host"; dns "$staging_host"
  local until=$((SECONDS + 300))
  until on_vm true 2>/dev/null; do (( SECONDS < until )) || { echo "room-vm: no SSH to $vm" >&2; exit 1; }; sleep 5; done
  setup
  on_vm 'sudo test -s /etc/oa-room/secret.env' || secret
  exists compute instance-groups unmanaged describe "$group" --zone "$zone" || {
    g compute instance-groups unmanaged create "$group" --zone "$zone"
    g compute instance-groups unmanaged add-instances "$group" --zone "$zone" --instances "$vm"; }
  g compute instance-groups unmanaged set-named-ports "$group" --zone "$zone" \
    --named-ports "s1:8081,s2:8082,s3:8083,s4:8084,s5:8085,s6:8086,staging:8090" >/dev/null
  exists compute health-checks describe "$hc" ||
    g compute health-checks create http "$hc" --use-serving-port --request-path /health --check-interval 5s --timeout 3s --healthy-threshold 1 --unhealthy-threshold 3
  # a WebSocket lives as long as the backend's timeout: a day (Cloud Run's rooms had an hour)
  for k in $(seq 1 "$shards") staging; do
    local b; b="$(backend "s$k")"; [[ "$k" == staging ]] && b="$(backend staging)"
    exists compute backend-services describe "$b" --global || {
      g compute backend-services create "$b" --global --load-balancing-scheme EXTERNAL_MANAGED --protocol HTTP \
        --port-name "$( [[ "$k" == staging ]] && echo staging || echo "s$k")" --health-checks "$hc" --timeout 86400s --connection-draining-timeout 30s
      g compute backend-services add-backend "$b" --global --instance-group "$group" --instance-group-zone "$zone" \
        --balancing-mode UTILIZATION --max-utilization 1.0 --capacity-scaler 1.0; }
  done
  echo "room-vm: provisioned $vm at $(ip) ($host, $staging_host)"
}

stage() {   # the room's C sources, staged on a build host (Coax to C runs the units' checks)
  local out="$here/build/stage"
  coax/remote/remote.sh run --no-fallback -- coax/web/room/stage.sh coax/web/room/build/stage >"$scratch/stage.log" 2>&1 ||
    { tail -30 "$scratch/stage.log" >&2; echo "room-vm: staging the sources failed" >&2; exit 1; }
  [[ -s "$out/src/room.c" && -s "$out/src/room-host.c" ]] || { echo "room-vm: the staged sources are missing" >&2; exit 1; }
  echo "$out"
}
# The release's name: the commit, "+" when the checkout had changes, and the time.
release_name() {   # [seeded]
  local c; c="$(git rev-parse --short=10 HEAD)"
  [[ -z "$(git status --porcelain -- coax/web/room apps/openagents-web verse aethyr 2>/dev/null)" ]] || c="$c+"
  echo "$c-$(date -u +%Y%m%d%H%M%S)${1:+-seeded}"
}
# Upload OUT/src and compile it on the VM (Debian bookworm's gcc, the steps Cloud Run's image runs),
# into /opt/oa-room/releases/NAME/room; the start check must hold.
build_on_vm() {   # out name [seed-tick-us]
  local out="$1" name="$2" seed="${3:-}"
  [[ -z "$seed" || "$seed" =~ ^[1-9][0-9]{0,6}$ ]] || { echo "room-vm: --seed-tick-us takes microseconds (1 to 9999999)" >&2; exit 2; }
  # libcurl's headers: the room's posters hold one libcurl handle each (room-host.c post_do, #13300);
  # vm/setup.sh installs them on a new VM, and a VM from before gets them here
  COPYFILE_DISABLE=1 tar -C "$out" -czf - src |
    on_vm "set -e; r=/opt/oa-room/releases/$name; sudo rm -rf \"\${r:?}\"; sudo mkdir -p \"\$r\"; sudo tar -xzf - -C \"\$r\" 2>/dev/null
      dpkg -s libcurl4-openssl-dev >/dev/null 2>&1 || { sudo apt-get update -q >/dev/null 2>&1; sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends libcurl4-openssl-dev >/dev/null; }
      cd \"\$r/src\" && sudo env OA_ROOM_SEED_TICK_US=$seed nice -n 10 sh compile.sh \"\$r/room\" >\"/tmp/compile-$name.log\" 2>&1 || { tail -30 \"/tmp/compile-$name.log\"; exit 1; }
      sudo rm -rf \"\${r:?}/src\"; \"\$r/room\" --start-check" ||
    { echo "room-vm: the build on $vm failed" >&2; exit 1; }
  # keep the five newest releases and every one a unit points at
  on_vm "cd /opt/oa-room/releases && keep=\"\$(readlink -f /opt/oa-room/current /opt/oa-room/staging /opt/oa-room/previous /opt/oa-room/staging-previous 2>/dev/null | xargs -n1 basename 2>/dev/null; cut -d' ' -f2 /opt/oa-room/canary 2>/dev/null)\";
    ls -1t | tail -n +6 | while read -r d; do echo \"\$keep\" | grep -qx \"\$d\" || sudo rm -rf \"/opt/oa-room/releases/\${d:?}\"; done" || true
}

# A release's symbols (#13298, docs/deployment/crash-reports.md): the binary's debug file (objcopy
# --only-keep-debug, gzipped), from the VM to the symbols bucket under the binary's build ID. The
# binary itself keeps its line table, so a crash resolves on the VM while the release is there (the
# five newest); the bucket is for after that. Never stops a deploy.
symbols() {
  local name="${1:-}" id
  [[ -n "$name" ]] || name="$(on_vm 'basename "$(readlink -f /opt/oa-room/staging)"')"
  [[ "$name" =~ ^[A-Za-z0-9.+_-]+$ ]] || { echo "room-vm: no such release: $name" >&2; return 1; }
  id="$(on_vm "readelf -n /opt/oa-room/releases/$name/room 2>/dev/null | sed -n 's/.*Build ID: *//p' | head -1")"
  [[ "$id" =~ ^[0-9a-f]{8,64}$ ]] || { echo "room-vm: release $name has no build ID: its symbols are not kept" >&2; return 1; }
  on_vm "d=\$(mktemp -d) && objcopy --only-keep-debug /opt/oa-room/releases/$name/room \"\$d/room.debug\" && gzip -9 -c \"\$d/room.debug\"; rm -rf \"\${d:?}\"" > "$scratch/room.debug.gz"
  [[ -s "$scratch/room.debug.gz" ]] || { echo "room-vm: no debug file came from $vm" >&2; return 1; }
  "$here/../symbols.sh" room-put "$id" "$scratch/room.debug.gz" "$name"
}
# The function and FILE:LINE of each offset in the binary of build BUILD: its debug file from the
# bucket, to the VM (which has addr2line), and gone again. For each offset: its own "0x..." line, then
# a function and its place for the code there and for each function it was inlined into.
resolve() {
  local id="${1:?usage: vm-deploy.sh resolve BUILD OFFSET...}"; shift
  local offs="" o
  for o in "$@"; do o="${o#+}"; o="${o#0x}"; [[ "$o" =~ ^[0-9a-f]{1,16}$ ]] || { echo "room-vm: an offset in hex: $o" >&2; return 2; }; offs+=" 0x$o"; done
  [[ -n "$offs" ]] || { echo "usage: vm-deploy.sh resolve BUILD OFFSET..." >&2; return 2; }
  "$here/../symbols.sh" room-get "$id" "$scratch/resolve.debug.gz"
  on_vm "d=\$(mktemp -d) && gunzip -c > \"\$d/room.debug\" && addr2line -a -f -i -e \"\$d/room.debug\"$offs; rm -rf \"\${d:?}\"" < "$scratch/resolve.debug.gz"
}

drain_wait="${OA_ROOM_DRAIN_WAIT:-300}"
# Drains production shard K on the VM (deploy.sh's drain, #12797): a signed POST, after which it seats
# nobody new and the directory places nobody on it; it waits up to WAIT seconds for its world events,
# tells every page "restarting" and saves every seat; then its health says "drain done".
drain() {
  local k="$1" wait="${2:-$drain_wait}" at mac code h
  [[ -n "${OA_ROOM_TICKET_KEY:-}" ]] || ! has oa-room-ticket-key || { OA_ROOM_TICKET_KEY="$(g secrets versions access latest --secret oa-room-ticket-key)"; export OA_ROOM_TICKET_KEY; }
  at="$(python3 -I -c 'import time; print(int(time.time() * 1000))')"
  code=000
  # a shard on its own write key takes a drain signed with it (#13260, room-write-tool.coax); one on the shared key, that key's
  if has "oa-room-write-key-s$k"; then
    mac="$(OA_ROOM_WRITE_KEY="$(g secrets versions access latest --secret "oa-room-write-key-s$k")" "$here/../tool.sh" "$here/room-write-tool.coax" mac drain "$k" "drain $k $at $wait")" || return 1
    code="$(curl -s -o /dev/null -w '%{http_code}' -m 30 -X POST -H "X-Room-At: $at" -H "X-Room-Wait: $wait" -H "X-Room-Mac: $mac" \
      "https://$host/api/room/s$k/drain" || true)"
  fi
  if [[ "$code" != 200 && -n "${OA_ROOM_TICKET_KEY:-}" ]]; then
    mac="$(python3 -I -c 'import hashlib, hmac, os, sys; print(hmac.new(os.environ["OA_ROOM_TICKET_KEY"].encode(), sys.argv[1].encode(), hashlib.sha256).hexdigest())' "drain $k $at $wait")"
    code="$(curl -s -o /dev/null -w '%{http_code}' -m 30 -X POST -H "X-Room-At: $at" -H "X-Room-Wait: $wait" -H "X-Room-Mac: $mac" \
      "https://$host/api/room/s$k/drain" || true)"
  fi
  if [[ "$code" == 403 ]]; then echo "room-vm: shard $k refused the drain (its key or clock)" >&2; return 1; fi
  if [[ "$code" != 200 ]]; then echo "room-vm: shard $k: nothing to drain (answered $code)"; return 0; fi
  local until=$((SECONDS + wait + 60))
  while (( SECONDS < until )); do
    h="$(health "$k")"
    if [[ "$h" == *"drain done"* || "$h" != ok* ]]; then echo "room-vm: shard $k drained"; return 0; fi
    sleep 2
  done
  echo "room-vm: shard $k didn't finish its drain in $((wait + 60)) s" >&2; return 1
}
active() { on_vm "systemctl is-active oa-room@s$1" 2>/dev/null | grep -qx active; }
# Every running production shard onto /opt/oa-room/current, one at a time: drained, restarted, healthy.
restart_shards() {   # [SKIP]: a shard that already runs the release (the canary)
  for k in $(seq 1 "$shards"); do
    if [[ "$k" == "${1:-}" ]]; then echo "room-vm: shard $k is the canary: it runs the release already"; continue; fi
    if ! active "$k"; then echo "room-vm: shard $k is not running (production is on Cloud Run): left stopped"; continue; fi
    drain "$k" || exit 1
    on_vm "sudo systemctl restart oa-room@s$k"
    wait_healthy "$k" || { echo "room-vm: stopping before the next shard; vm-deploy.sh rollback goes back" >&2; exit 1; }
  done
}

deploy() {
  local out name
  if [[ "${1:-}" == --staging ]]; then
    local seed=""
    if [[ "${2:-}" == --seed-tick-us ]]; then seed="${3:?usage: vm-deploy.sh deploy --staging --seed-tick-us MICROSECONDS}"; fi
    out="$(stage)"; name="$(release_name ${seed:+seeded})"
    echo "room-vm: building $name on $vm${seed:+ (a seeded bad build: every tick $seed us longer; staging only)}"
    build_on_vm "$out" "$name" "$seed"
    # the release staging ran before stays reachable: vm-deploy.sh rollback --staging
    on_vm "was=\$(readlink -f /opt/oa-room/staging || true); [ -z \"\$was\" ] || [ \"\$was\" = /opt/oa-room/releases/$name ] || sudo ln -sfn \"\$was\" /opt/oa-room/staging-previous
      sudo ln -sfn /opt/oa-room/releases/$name /opt/oa-room/staging && sudo systemctl enable oa-room@staging >/dev/null 2>&1; sudo systemctl restart oa-room@staging"
    wait_healthy staging
    symbols "$name" || echo "room-vm: the release's symbols were not kept (vm-deploy.sh symbols $name tries again)" >&2
    echo "room-vm: staging runs $name (https://$staging_host); vm-deploy.sh deploy promotes it"
    return
  fi
  name="$(promotable)"
  local canary; canary="$(on_vm "cat /opt/oa-room/canary 2>/dev/null || true")"
  # every shard's binary is production's release again (a canary's link pointed at its own)
  on_vm "cur=\$(readlink -f /opt/oa-room/current || true); [ -z \"\$cur\" ] || [ \"\$cur\" = /opt/oa-room/releases/$name ] || sudo ln -sfn \"\$cur\" /opt/oa-room/previous; sudo ln -sfn /opt/oa-room/releases/$name /opt/oa-room/current
    for k in \$(seq 1 $shards); do sudo ln -sfn /opt/oa-room/current/room /opt/oa-room/bin/s\$k; done; sudo rm -f /opt/oa-room/canary"
  echo "room-vm: production's release is $name"
  if [[ "$canary" == *" $name" ]]; then restart_shards "${canary%% *}"; else restart_shards; fi
}
# The release staging runs, when production may take it: named, healthy, and not a seeded bad build.
promotable() {
  local name
  name="$(on_vm 'basename "$(readlink -f /opt/oa-room/staging)"')"
  [[ -n "$name" ]] || { echo "room-vm: staging runs nothing: vm-deploy.sh deploy --staging first" >&2; exit 1; }
  [[ "$name" != *-seeded ]] || { echo "room-vm: staging runs $name, a seeded bad build: production never takes one" >&2; exit 1; }
  wait_healthy staging 10 >&2 || { echo "room-vm: staging isn't healthy: not promoting $name" >&2; exit 1; }
  echo "$name"
}
# The canary (#13299, coax/web/release-set.coax): production shard K alone onto staging's release.
# Its unit's binary link points at that release; /opt/oa-room/canary says "K NAME" until `deploy`
# brings the rest, or `canary-back` puts the shard back on production's.
canary() {
  local k="${1:?usage: vm-deploy.sh canary K}" name
  [[ "$k" =~ ^[1-9][0-9]?$ ]] && (( k <= shards )) || { echo "room-vm: no shard $k" >&2; exit 2; }
  name="$(promotable)"
  active "$k" || { echo "room-vm: shard $k is not running: no canary there" >&2; exit 1; }
  drain "$k" || exit 1
  on_vm "sudo ln -sfn /opt/oa-room/releases/$name/room /opt/oa-room/bin/s$k && echo '$k $name' | sudo tee /opt/oa-room/canary >/dev/null && sudo systemctl restart oa-room@s$k"
  wait_healthy "$k" || { echo "room-vm: the canary is not healthy; vm-deploy.sh canary-back puts shard $k back" >&2; exit 1; }
  echo "room-vm: shard $k is the canary on $name"
}
canary_back() {
  local was k
  was="$(on_vm "cat /opt/oa-room/canary 2>/dev/null || true")"; k="${was%% *}"
  [[ "$k" =~ ^[1-9][0-9]?$ ]] || { echo "room-vm: no canary is out"; return 0; }
  drain "$k" "${OA_ROOM_DRAIN_WAIT:-60}" || true
  on_vm "sudo ln -sfn /opt/oa-room/current/room /opt/oa-room/bin/s$k && sudo rm -f /opt/oa-room/canary && sudo systemctl restart oa-room@s$k"
  wait_healthy "$k"
  echo "room-vm: shard $k is back on production's release"
}
releases() {
  on_vm 'for l in current previous staging staging-previous; do r=""; [ -L /opt/oa-room/$l ] && r="$(basename "$(readlink -f /opt/oa-room/$l)")"; printf "%s: %s\n" "$l" "$r"; done; printf "canary: %s\n" "$(cat /opt/oa-room/canary 2>/dev/null)"'
}

rollback() {
  local prev
  if [[ "${1:-}" == --staging ]]; then
    prev="$(on_vm 'readlink -f /opt/oa-room/staging-previous || true')"
    [[ -n "$prev" ]] || { echo "room-vm: no earlier staging release is recorded" >&2; exit 1; }
    on_vm "cur=\$(readlink -f /opt/oa-room/staging); sudo ln -sfn $prev /opt/oa-room/staging; sudo ln -sfn \"\$cur\" /opt/oa-room/staging-previous; sudo systemctl restart oa-room@staging"
    wait_healthy staging
    echo "room-vm: staging runs $(basename "$prev") again"
    return
  fi
  prev="$(on_vm 'readlink -f /opt/oa-room/previous || true')"
  [[ -n "$prev" ]] || { echo "room-vm: no previous release is recorded" >&2; exit 1; }
  on_vm "cur=\$(readlink -f /opt/oa-room/current); sudo ln -sfn $prev /opt/oa-room/current; sudo ln -sfn \"\$cur\" /opt/oa-room/previous
    for k in \$(seq 1 $shards); do sudo ln -sfn /opt/oa-room/current/room /opt/oa-room/bin/s\$k; done; sudo rm -f /opt/oa-room/canary"
  echo "room-vm: production's release is $(basename "$prev")"
  restart_shards
}

# The url map's room rules: in the production and everglade path matchers (or staging's), each rule
# for /api/room/sK/ (and /api/room/, shard 1's) goes to the VM's backend or back to Cloud Run's service.
url_map() {   # vm | run | staging-vm | staging-run
  g compute url-maps describe "$map" --global --format json > "$scratch/map.json"
  python3 -I - "$scratch/map.json" "$project" "$1" <<'PY'
import json, re, sys
path, project, mode = sys.argv[1:]
m = json.load(open(path))
base = f"https://www.googleapis.com/compute/v1/projects/{project}/global/backendServices/"
matchers = ("staging",) if mode.startswith("staging") else ("production", "everglade")
to_vm = mode.endswith("vm")
changed = 0
for pm in m["pathMatchers"]:
    if pm["name"] not in matchers: continue
    for r in pm.get("routeRules", []):
        name = r.get("service", "").rsplit("/", 1)[-1]
        if not re.fullmatch(r"oa-room(-\d+)?|oa-room-vm-(s\d+|staging)", name): continue
        prefixes = [x.get("prefixMatch", "") for x in r["matchRules"]]
        k = next((int(p[len("/api/room/s"):-1]) for p in prefixes if re.fullmatch(r"/api/room/s\d+/", p)), 1 if "/api/room/" in prefixes else None)
        if k is None: continue
        if to_vm: want = "oa-room-vm-staging" if mode.startswith("staging") else f"oa-room-vm-s{k}"
        else: want = "oa-room" if k == 1 else f"oa-room-{k}"
        if name != want: r["service"] = base + want; changed += 1
for key in ("kind", "creationTimestamp", "id", "selfLink"): m.pop(key, None)
json.dump(m, open(path, "w"))
print(f"url map: {changed} room rules to {'the VM' if to_vm else 'Cloud Run'} ({', '.join(matchers)})")
PY
  g compute url-maps validate --global --source "$scratch/map.json" --format 'value(result.loadSucceeded)' 2>&1 | grep -qi true ||
    { echo "room-vm: the new url map doesn't validate; nothing changed" >&2; exit 1; }
  g compute url-maps import "$map" --global --source "$scratch/map.json" >/dev/null
}
lb_healthy() {   # BACKEND: the load balancer's health check sees the unit
  local until=$((SECONDS + 180))
  while (( SECONDS < until )); do
    [[ "$(g compute backend-services get-health "$1" --global --format='value(status.healthStatus[0].healthState)' 2>/dev/null)" == HEALTHY ]] && return 0
    sleep 5
  done
  echo "room-vm: the load balancer doesn't see $1 healthy" >&2; return 1
}
switch() {
  case "${1:-}" in
    vm)
      on_vm '[ -x /opt/oa-room/current/room ]' || { echo "room-vm: no production release: vm-deploy.sh deploy first" >&2; exit 1; }
      setup >/dev/null   # the units' env files as setup.sh writes them, endpoints included
      for k in $(seq 1 "$shards"); do on_vm "sudo systemctl enable oa-room@s$k >/dev/null 2>&1; sudo systemctl start oa-room@s$k"; done
      for k in $(seq 1 "$shards"); do wait_healthy "$k"; lb_healthy "$(backend "s$k")"; done
      url_map vm
      echo "room-vm: production's rooms are on $vm; vm-deploy.sh switch run goes back to Cloud Run" ;;
    run)
      url_map run
      # Each running shard first restarts without its endpoint, so its next report clears it from the
      # directory and new tickets name the load balancer's path again (#13179); then it drains (its
      # pages hear "restarting" and sit again, now through Cloud Run) and stops, so its reports end.
      for k in $(seq 1 "$shards"); do
        if active "$k"; then
          on_vm "sudo sed -i '/^OA_ROOM_ENDPOINT=/d' /etc/oa-room/s$k.env && sudo systemctl restart oa-room@s$k" || true
          wait_healthy "$k" 30 || true; sleep 6
          drain "$k" "${OA_ROOM_DRAIN_WAIT:-30}" || true; on_vm "sudo systemctl disable --now oa-room@s$k >/dev/null 2>&1" || true
        fi
      done
      echo "room-vm: vm-deploy.sh setup puts the shards' endpoints back before the next switch vm"
      echo "room-vm: production's rooms are on Cloud Run (oa-room, oa-room-2..$shards)" ;;
    staging-vm)
      wait_healthy staging 10; lb_healthy "$(backend staging)"; url_map staging-vm
      echo "room-vm: staging.openagents.com's rooms are on $vm's staging unit" ;;
    staging-run)
      url_map staging-run; echo "room-vm: staging.openagents.com's rooms are on Cloud Run" ;;
    *) echo "usage: vm-deploy.sh switch vm|run|staging-vm|staging-run" >&2; exit 2 ;;
  esac
}

status() {
  echo "vm: $vm $(g compute instances describe "$vm" --zone "$zone" --format='value(status)') at $(ip)"
  on_vm 'printf "current: %s\nprevious: %s\nstaging: %s\n" "$(basename "$(readlink -f /opt/oa-room/current)" 2>/dev/null)" "$(basename "$(readlink -f /opt/oa-room/previous)" 2>/dev/null)" "$(basename "$(readlink -f /opt/oa-room/staging)" 2>/dev/null)"; for u in s1 s2 s3 s4 s5 s6 staging; do printf "oa-room@%s %s\n" "$u" "$(systemctl is-active oa-room@$u)"; done' || true
  g compute url-maps describe "$map" --global --format json | python3 -I -c '
import json, sys
m = json.load(sys.stdin)
for pm in m["pathMatchers"]:
    rooms = sorted({r["service"].rsplit("/", 1)[-1] for r in pm.get("routeRules", []) if "/oa-room" in r.get("service", "")})
    if rooms: print("url map " + pm["name"] + ": " + " ".join(rooms))'
  for k in $(seq 1 "$shards") staging; do echo "$k: $(health "$k" | head -1 | cut -c1-160)"; done
}

case "${1:-}" in
  provision) provision ;;
  wt) COPYFILE_DISABLE=1 tar -C "$here/vm" -czf - wt-cert.sh |
      on_vm 'd=$(mktemp -d) && tar -xzf - -C "$d" 2>/dev/null && cd "$d" && sudo sh wt-cert.sh --install'
      restart_shards ;;   # each shard drained, then restarted onto its UDP port and certificate
  secret) secret ;;
  deploy) shift; deploy "$@" ;;
  rollback) shift; rollback "$@" ;;
  canary) shift; canary "$@" ;;
  canary-back) canary_back ;;
  releases) releases ;;
  switch) shift; switch "$@" ;;
  drain) shift; drain "${1:?usage: vm-deploy.sh drain K [SECONDS]}" "${2:-$drain_wait}" ;;
  kill) shift; u="${1:?usage: vm-deploy.sh kill K|staging}"; [[ "$u" == staging ]] || u="s$u"
    on_vm "sudo systemctl kill -s KILL oa-room@$u"; echo "room-vm: oa-room@$u killed" ;;
  crash) shift; u="${1:?usage: vm-deploy.sh crash K|staging}"; [[ "$u" == staging ]] || u="s$u"
    # the room process only (never its helpers), and only its SIGUSR2: room-host.c runs the Coax crash test
    on_vm "sudo systemctl kill --kill-who=main -s USR2 oa-room@$u"; echo "room-vm: oa-room@$u asked to crash (SIGUSR2); its next start posts the report" ;;
  symbols) shift; symbols "${1:-}" ;;
  resolve) shift; resolve "$@" ;;
  setup) setup ;;
  status) status ;;
  *) sed -n '2,34p' "$0" >&2; exit 2 ;;
esac

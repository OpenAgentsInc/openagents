#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: gcloud, ssh and curl around the repo VM)
# The indexed repo service on a GCE VM (docs/deployment/repo-vm.md, #13326): oa-repo-1, e2-standard-2
# in us-central1-a on the default network, with no external address (outbound HTTPS to github.com goes
# through the network's Cloud NAT) and the static internal address oa-repo-1-internal. Its data disk
# oa-repo-1-data (100 GB pd-balanced, snapshotted daily by oa-repo-1-daily) is mounted at
# /var/lib/oa-repo. Two systemd units share the VM: oa-repo@production on 8080 and oa-repo@staging
# on 8081, each its own user, data directory and grant key. Only the Cloud Run subnets
# oa-run-production (10.42.29.0/26: oa-api, oa-agent) and oa-run-staging (10.42.30.0/26: the staging
# pair) reach them, production's port from production's range and staging's from staging's.
#
#   coax/web/repo/vm-deploy.sh create             the VM, its disk, snapshots, subnets, firewall, keys (idempotent)
#   coax/web/repo/vm-deploy.sh setup              the VM's packages, disk mount, users, units and env files again
#   coax/web/repo/vm-deploy.sh keys               the grant keys, from Secret Manager to the VM's root-only files
#   coax/web/repo/vm-deploy.sh deploy --staging   build oa-repo (coax/web/repo/build.sh on a build host), ship it,
#                                                 run it as oa-repo@staging and wait for its /healthz
#   coax/web/repo/vm-deploy.sh deploy             promote the binary staging runs to oa-repo@production
#   coax/web/repo/vm-deploy.sh rollback [--staging]   the unit's previous binary
#   coax/web/repo/vm-deploy.sh status
#   coax/web/repo/vm-deploy.sh connect [--apply]  Direct VPC egress and OA_REPO_URL on oa-api, oa-api-staging,
#                                                 oa-agent and oa-agent-staging: printed, run only with --apply
#
# Every subcommand takes --dry-run: it prints the gcloud and SSH commands that change something and runs
# none of them (read-only describes still run). OA_REPO_REPOS (setup, create) overrides the repo list.
#
# Cost of what create makes, per month (list prices read 2026-11-11 from the Cloud Billing Catalog API,
# us-central1, on demand; NAT is an estimate, its SKU was not read):
#   e2-standard-2: (2 vCPU x $0.02181159 + 8 GB x $0.00292353) = $0.06701/h x 730 h   $48.92
#   data disk, 100 GB pd-balanced x $0.10                                             $10.00
#   boot disk, 10 GB pd-balanced x $0.10                                               $1.00
#   snapshots, about 10 GB stored (a mirror plus 7 daily increments) x $0.05           ~$0.50
#   Cloud NAT, one VM ($0.0014/h x 730) plus a few GB of fetches at $0.045/GB          ~$1.20 (estimate)
#   internal address, subnets, firewall rules, Direct VPC egress                        $0
#   total                                                                             ~$61.60
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
cd "$root"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 zone=us-central1-a network=default
vm=oa-repo-1 disk=oa-repo-1-data address=oa-repo-1-internal policy=oa-repo-1-daily tag=oa-repo
# shellcheck disable=SC2034  # read through ${!name} (sub_$u, range_$u, key_$u)
sub_production=oa-run-production range_production=10.42.29.0/26
# shellcheck disable=SC2034
sub_staging=oa-run-staging range_staging=10.42.30.0/26
port_production=8080 port_staging=8081
key_production=oa-repo-grant-key key_staging=oa-repo-grant-key-staging
repos="${OA_REPO_REPOS:-openagents=https://github.com/OpenAgentsInc/openagents.git}"
scratch="$(mktemp -d)"; trap 'rm -rf "${scratch:?}"' EXIT

dry=false apply=false staging=false args=()
for a in "$@"; do
  case "$a" in
    --dry-run) dry=true ;;
    --apply) apply=true ;;
    --staging) staging=true ;;
    *) args+=("$a") ;;
  esac
done
set -- "${args[@]+"${args[@]}"}"

say() { echo "repo-vm: $*"; }
g() { gcloud "$@" --project "$project" --quiet; }
# A command that changes something: printed under --dry-run, run otherwise.
run() {
  if $dry; then printf '  [dry-run] gcloud'; printf ' %q' "$@" --project "$project" --quiet; echo; else g "$@"; fi
}
exists() { g "$@" --format='value(name)' >/dev/null 2>&1; }
# A command on the VM as its OS Login user (root through sudo), over IAP. Under --dry-run it is printed
# and its standard input (when it has one) is left unread, so nothing piped to it is fetched or shown.
on_vm() {
  if $dry; then printf '  [dry-run] gcloud compute ssh %s --zone %s --tunnel-through-iap --command %q\n' "$vm" "$zone" "$1"; return 0; fi
  if ! g compute ssh "$vm" --zone "$zone" --tunnel-through-iap --command "$1" 2>"$scratch/ssh.err"; then
    grep -v -i -E "numpy|increasing_the_tcp|^WARNING: *$|^$" "$scratch/ssh.err" >&2 || true; return 1
  fi
}
internal_ip() {
  local a; a="$(g compute addresses describe "$address" --region "$region" --format='value(address)' 2>/dev/null || true)"
  if [[ -n "$a" ]]; then echo "$a"; elif $dry; then echo INTERNAL_IP; else return 1; fi
}
port() { if [[ "$1" == staging ]]; then echo "$port_staging"; else echo "$port_production"; fi; }

# ---- create ------------------------------------------------------------------------------------

new_key() {   # SECRET: made once, 32 random bytes as hex, never printed
  exists secrets describe "$1" && { say "secret $1 exists"; return 0; }
  say "creating secret $1 (32 random bytes, hex; not printed)"
  run secrets create "$1" --replication-policy automatic --labels app=oa-repo,owner=openagents
  if $dry; then printf '  [dry-run] openssl rand -hex 32 | tr -d "\\n" | gcloud secrets versions add %s --data-file - --project %s --quiet\n' "$1" "$project"
  else openssl rand -hex 32 | tr -d '\n' | g secrets versions add "$1" --data-file - >/dev/null; fi
}

# The grant keys, piped from Secret Manager into the VM's root-only env files; never printed. The VM has
# no service account, so it reads nothing from Google itself. A unit reads its key when it restarts.
keys() {
  say "writing the grant keys to $vm (/etc/oa-repo/{production,staging}.secret.env, root only)"
  local u k
  for u in production staging; do
    k=key_$u; k="${!k}"
    if $dry; then echo "  [dry-run] gcloud secrets versions access latest --secret $k | (ssh) > /etc/oa-repo/$u.secret.env"; continue; fi
    { printf OA_REPO_GRANT_KEY=; g secrets versions access latest --secret "$k"; echo; } |
      on_vm "sudo sh -c 'umask 077; cat > /etc/oa-repo/$u.secret.env.new && mv /etc/oa-repo/$u.secret.env.new /etc/oa-repo/$u.secret.env'"
  done
}

setup() {
  say "installing git, the data disk's mount, the users, units and env files on $vm (vm/setup.sh)"
  if $dry; then on_vm "sudo OA_REPO_REPOS='$repos' sh setup.sh   (from a copy of coax/web/repo/vm/)"; return; fi
  COPYFILE_DISABLE=1 tar -C "$here/vm" -czf - oa-repo@.service setup.sh |
    on_vm "d=\$(mktemp -d) && tar -xzf - -C \"\$d\" 2>/dev/null && cd \"\$d\" && sudo OA_REPO_REPOS='$repos' sh setup.sh"
}

create() {
  local u sub range p ip
  for u in production staging; do
    sub=sub_$u; sub="${!sub}"; range=range_$u; range="${!range}"
    if exists compute networks subnets describe "$sub" --region "$region"; then say "subnet $sub exists"
    else
      say "creating subnet $sub ($range on $network) for the $u Cloud Run services' Direct VPC egress"
      run compute networks subnets create "$sub" --network "$network" --region "$region" --range "$range"
    fi
  done
  if exists compute addresses describe "$address" --region "$region"; then say "address $address exists"
  else
    say "reserving the static internal address $address in subnet $network"
    run compute addresses create "$address" --region "$region" --subnet "$network"
  fi
  # SSH through IAP, each unit's port from its own Cloud Run range, everything else refused (the
  # default network allows SSH from anywhere and every port from 10.128.0.0/9 at priority 65534)
  if exists compute firewall-rules describe oa-repo-deny-other-ingress; then say "firewall oa-repo-deny-other-ingress exists"
  else
    say "creating firewall oa-repo-deny-other-ingress (deny all ingress to tag $tag, priority 1000)"
    run compute firewall-rules create oa-repo-deny-other-ingress --network "$network" --direction INGRESS --action DENY \
      --rules all --source-ranges 0.0.0.0/0 --target-tags "$tag" --priority 1000
  fi
  if exists compute firewall-rules describe oa-repo-iap-ssh; then say "firewall oa-repo-iap-ssh exists"
  else
    say "creating firewall oa-repo-iap-ssh (tcp:22 from IAP's 35.235.240.0/20)"
    run compute firewall-rules create oa-repo-iap-ssh --network "$network" --direction INGRESS --action ALLOW \
      --rules tcp:22 --source-ranges 35.235.240.0/20 --target-tags "$tag" --priority 900
  fi
  for u in production staging; do
    range=range_$u; range="${!range}"; p="$(port "$u")"
    if exists compute firewall-rules describe "oa-repo-$u-callers"; then say "firewall oa-repo-$u-callers exists"
    else
      say "creating firewall oa-repo-$u-callers (tcp:$p from $range only)"
      run compute firewall-rules create "oa-repo-$u-callers" --network "$network" --direction INGRESS --action ALLOW \
        --rules "tcp:$p" --source-ranges "$range" --target-tags "$tag" --priority 900
    fi
  done
  if exists compute resource-policies describe "$policy" --region "$region"; then say "snapshot schedule $policy exists"
  else
    say "creating snapshot schedule $policy (daily at 09:00 UTC, kept 7 days, in $region)"
    run compute resource-policies create snapshot-schedule "$policy" --region "$region" --daily-schedule --start-time 09:00 \
      --max-retention-days 7 --on-source-disk-delete keep-auto-snapshots --storage-location "$region"
  fi
  if exists compute disks describe "$disk" --zone "$zone"; then say "disk $disk exists"
  else
    say "creating disk $disk (100 GB pd-balanced, snapshotted by $policy)"
    run compute disks create "$disk" --zone "$zone" --size 100GB --type pd-balanced --resource-policies "$policy" \
      --labels app=oa-repo,owner=openagents,issue=13326
  fi
  new_key "$key_production"; new_key "$key_staging"
  if exists compute instances describe "$vm" --zone "$zone"; then say "VM $vm exists"
  else
    ip="$(internal_ip)"
    say "creating VM $vm (e2-standard-2, Debian 12, 10 GB boot, no external address, no service account, at $ip)"
    # no service account: the VM calls no Google API (its keys arrive over SSH); no external address:
    # nothing on the internet reaches it, and its fetches leave through the network's Cloud NAT
    run compute instances create "$vm" --zone "$zone" --machine-type e2-standard-2 --image-family debian-12 --image-project debian-cloud \
      --boot-disk-size 10GB --boot-disk-type pd-balanced --network "$network" --subnet "$network" --private-network-ip "$ip" --no-address \
      --disk "name=$disk,device-name=oa-repo-data,mode=rw,boot=no,auto-delete=no" --tags "$tag" \
      --labels app=oa-repo,owner=openagents,issue=13326 --no-service-account --no-scopes \
      --maintenance-policy MIGRATE --restart-on-failure --shielded-secure-boot --metadata enable-oslogin=TRUE
  fi
  if ! $dry; then
    local until=$((SECONDS + 300))
    until on_vm true 2>/dev/null; do (( SECONDS < until )) || { echo "repo-vm: no SSH to $vm" >&2; exit 1; }; sleep 5; done
  fi
  setup
  keys
  say "created $vm at $(internal_ip); next: vm-deploy.sh deploy --staging, then connect"
}

# ---- deploy ------------------------------------------------------------------------------------

# The release's name: the commit, "+" when the checkout had changes, and the time.
release_name() {
  local c; c="$(git rev-parse --short=10 HEAD)"
  [[ -z "$(git status --porcelain -- coax/web/repo coax/lib/repo 2>/dev/null)" ]] || c="$c+"
  echo "$c-$(date -u +%Y%m%d%H%M%S)"
}
build() {   # the binary, on a build host (Debian 12 x86-64, as the VM is); prints its path
  local out="coax/web/repo/build/oa-repo"
  if [[ ! -x coax/web/repo/build.sh ]]; then
    echo "repo-vm: coax/web/repo/build.sh is missing (or not executable): nothing to build" >&2
    $dry || exit 1
  fi
  if $dry; then echo "  [dry-run] coax/remote/remote.sh run --no-fallback -- coax/web/repo/build.sh $out" >&2; echo "$out"; return; fi
  coax/remote/remote.sh run --no-fallback -- coax/web/repo/build.sh "$out" >"$scratch/build.log" 2>&1 ||
    { tail -30 "$scratch/build.log" >&2; echo "repo-vm: the build failed" >&2; exit 1; }
  [[ -s "$out" ]] || { echo "repo-vm: the build wrote no $out" >&2; exit 1; }
  file -b "$out" | grep -q 'ELF 64-bit.*x86-64' || { echo "repo-vm: $out is not a Linux x86-64 executable: $(file -b "$out")" >&2; exit 1; }
  echo "$out"
}
# Upload the binary into /opt/oa-repo/releases/NAME/oa-repo; every library it needs must be on the VM.
ship() {
  local bin="$1" name="$2" r="/opt/oa-repo/releases/$2"
  say "shipping $name to $vm ($r)"
  local cmd="set -e; sudo rm -rf '$r'; sudo mkdir -p '$r'; gunzip -c | sudo tee '$r/oa-repo' >/dev/null; sudo chmod 755 '$r/oa-repo'
    if ldd '$r/oa-repo' | grep -q 'not found'; then ldd '$r/oa-repo' | grep 'not found'; exit 1; fi"
  if $dry; then on_vm "$cmd"; else gzip -c "$bin" | on_vm "$cmd" || { echo "repo-vm: shipping $name failed" >&2; exit 1; }; fi
  # keep the five newest releases and every one a unit points at
  on_vm "cd /opt/oa-repo/releases && keep=\"\$(readlink -f /opt/oa-repo/production /opt/oa-repo/production.previous /opt/oa-repo/staging /opt/oa-repo/staging.previous 2>/dev/null | xargs -n1 basename 2>/dev/null)\";
    ls -1t | tail -n +6 | while read -r d; do echo \"\$keep\" | grep -qx \"\$d\" || sudo rm -rf \"/opt/oa-repo/releases/\${d:?}\"; done" || true
}
healthz() {   # UNIT: the unit's /healthz body, from the VM itself
  on_vm "curl -fsS -m 5 http://127.0.0.1:$(port "$1")/healthz" 2>/dev/null || true
}
wait_healthy() {   # UNIT [SECONDS]
  $dry && { echo "  [dry-run] wait for http://127.0.0.1:$(port "$1")/healthz on $vm to answer ok"; return 0; }
  local until=$((SECONDS + ${2:-60})) h
  while (( SECONDS < until )); do
    h="$(healthz "$1")"
    [[ "$h" == ok* ]] && { say "oa-repo@$1 healthy"; return 0; }
    sleep 2
  done
  echo "repo-vm: oa-repo@$1 is not healthy after ${2:-60} s: $(head -c 200 <<<"$h")" >&2; return 1
}
# Point UNIT at release directory DIR (the old one becomes UNIT.previous), restart it, wait for health.
point() {
  local u="$1" dir="$2"
  say "oa-repo@$u -> $(basename "$dir"); restarting it"
  on_vm "cur=\$(readlink -f /opt/oa-repo/$u || true); [ -z \"\$cur\" ] || [ \"\$cur\" = '$dir' ] || sudo ln -sfn \"\$cur\" /opt/oa-repo/$u.previous
    sudo ln -sfn '$dir' /opt/oa-repo/$u && sudo systemctl enable oa-repo@$u >/dev/null 2>&1; sudo systemctl restart oa-repo@$u"
  wait_healthy "$u" || { echo "repo-vm: vm-deploy.sh rollback$([[ $u == staging ]] && echo ' --staging') goes back" >&2; exit 1; }
}

deploy() {
  local bin name dir
  if $staging; then
    bin="$(build)"; name="$(release_name)"
    ship "$bin" "$name"
    point staging "/opt/oa-repo/releases/$name"
    say "staging runs $name; vm-deploy.sh deploy promotes it"
    return
  fi
  if $dry; then dir=/opt/oa-repo/releases/STAGING_RELEASE; on_vm 'readlink -f /opt/oa-repo/staging'
  else
    dir="$(on_vm 'readlink -f /opt/oa-repo/staging || true')"
    [[ -n "$dir" ]] || { echo "repo-vm: staging runs nothing: vm-deploy.sh deploy --staging first" >&2; exit 1; }
    wait_healthy staging 10 || { echo "repo-vm: staging isn't healthy: not promoting $(basename "$dir")" >&2; exit 1; }
  fi
  say "promoting staging's binary $(basename "$dir") to production"
  point production "$dir"
}

rollback() {
  local u=production prev
  $staging && u=staging
  if $dry; then prev=/opt/oa-repo/releases/PREVIOUS_RELEASE; on_vm "readlink -f /opt/oa-repo/$u.previous"
  else
    prev="$(on_vm "readlink -f /opt/oa-repo/$u.previous || true")"
    [[ -n "$prev" ]] || { echo "repo-vm: no previous release is recorded for $u" >&2; exit 1; }
  fi
  say "rolling oa-repo@$u back to $(basename "$prev")"
  point "$u" "$prev"
}

# ---- status ------------------------------------------------------------------------------------

status() {
  echo "vm: $vm $(g compute instances describe "$vm" --zone "$zone" --format='value(status)' 2>/dev/null || echo 'not created') at $(internal_ip 2>/dev/null || echo 'no address')"
  echo "disk: $disk $(g compute disks describe "$disk" --zone "$zone" --format='value(sizeGb,type.basename(),resourcePolicies.basename())' 2>/dev/null || echo 'not created')"
  echo "last snapshot: $(g compute snapshots list --filter="sourceDisk~/$disk\$" --sort-by=~creationTimestamp --limit 1 --format='value(name,creationTimestamp)' 2>/dev/null || true)"
  # shellcheck disable=SC2016  # expanded on the VM
  on_vm 'df -h /var/lib/oa-repo | tail -1; for u in production staging; do printf "oa-repo@%s %s, release %s, previous %s, healthz %s\n" "$u" "$(systemctl is-active oa-repo@$u)" "$(basename "$(readlink -f /opt/oa-repo/$u)" 2>/dev/null)" "$(basename "$(readlink -f /opt/oa-repo/$u.previous)" 2>/dev/null)" "$(curl -fsS -m 5 http://127.0.0.1:$([ $u = production ] && echo 8080 || echo 8081)/healthz 2>/dev/null || echo down)"; done' || true
  local s
  for s in oa-api oa-api-staging oa-agent oa-agent-staging; do
    g run services describe "$s" --region "$region" --format=json 2>/dev/null | python3 -I -c '
import json, sys
d = json.load(sys.stdin); t = d["spec"]["template"]
ni = t["metadata"].get("annotations", {}).get("run.googleapis.com/network-interfaces", "none")
eg = t["metadata"].get("annotations", {}).get("run.googleapis.com/vpc-access-egress", "")
url = next((e.get("value", "") for e in t["spec"]["containers"][0].get("env", []) if e["name"] == "OA_REPO_URL"), "unset")
name = d["metadata"]["name"]
print(f"{name}: vpc {ni} {eg}, OA_REPO_URL {url}")' || echo "$s: not found"
  done
}

# ---- connect -----------------------------------------------------------------------------------

# Direct VPC egress (private ranges only: the internet, Cloud SQL's connector and Google APIs keep
# their current path) on each caller's stage subnet, and OA_REPO_URL to its unit. Note that
# coax/web/api/deploy.sh and coax/web/agent/deploy.sh carry OA_REPO_URL over from the live service,
# since their --set-env-vars would otherwise drop it; the network settings carry over by themselves.
connect() {
  local ip s u sub
  ip="$(internal_ip)" || { echo "repo-vm: no address $address: vm-deploy.sh create first" >&2; exit 1; }
  local doit=false; $apply && ! $dry && doit=true
  $doit || say "the change connect --apply makes (nothing is run without --apply):"
  for s in oa-api oa-agent oa-api-staging oa-agent-staging; do
    u=production; [[ "$s" == *-staging ]] && u=staging
    sub=sub_$u; sub="${!sub}"
    local cmd=(run services update "$s" --region "$region" --network "$network" --subnet "$sub" --vpc-egress private-ranges-only
      --update-env-vars "OA_REPO_URL=http://$ip:$(port "$u")")
    if $doit; then say "updating $s: Direct VPC egress on $sub, OA_REPO_URL=http://$ip:$(port "$u")"; g "${cmd[@]}" >/dev/null
    else printf '  gcloud'; printf ' %q' "${cmd[@]}" --project "$project" --quiet; echo; fi
  done
  $doit && say "connected; vm-deploy.sh status shows each service's network and OA_REPO_URL"
  return 0
}

case "${1:-}" in
  create) create ;;
  setup) setup ;;
  keys) keys ;;
  deploy) deploy ;;
  rollback) rollback ;;
  status) status ;;
  connect) connect ;;
  *) sed -n '2,24p' "$0" >&2; exit 2 ;;
esac

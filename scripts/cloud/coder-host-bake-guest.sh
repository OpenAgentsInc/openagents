#!/usr/bin/env bash
#
# The startup script of the temporary `oa-coder-host` builder VM.
# scripts/cloud/build-coder-host-image.sh passes it as `startup-script`
# metadata; it runs once, as root, on a stock Debian 12 boot disk.
#
# It clones the repository at the revision in metadata `oa-rev`, runs the
# shared scripts/cloud/coder-host-setup.sh with a warm build, installs the
# boot-time ready unit, seals the disk for imaging, and prints
#   OA_CODER_HOST_BAKE_OK {json}      or
#   OA_CODER_HOST_BAKE_FAILED ...
# on the serial console, which the orchestrator reads. It writes no secret:
# the VM's service account reaches the sccache bucket through the metadata
# server, and nothing about that identity is stored on the disk.
set -euo pipefail

serial() { printf '%s\n' "$*" >/dev/ttyS0 2>/dev/null || true; printf '%s\n' "$*"; }
md() {
  curl -fsS -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1" 2>/dev/null || true
}

if [[ -e /var/lib/oa-coder-host/baked ]]; then
  # A booted copy of the image never re-runs the bake.
  exit 0
fi

started="$(date -u +%s)"
trap 'serial "OA_CODER_HOST_BAKE_FAILED line=$LINENO status=$?"' ERR

rev="$(md oa-rev)"
repo_url="$(md oa-repo-url)"
bucket="$(md oa-sccache-bucket)"
repo_url="${repo_url:-https://github.com/OpenAgentsInc/openagents.git}"
[[ -n "$rev" ]] || { serial "OA_CODER_HOST_BAKE_FAILED no oa-rev metadata"; exit 1; }

export DEBIAN_FRONTEND=noninteractive
# The builder has no external address; egress goes through Cloud NAT, whose
# mapping for a new VM can lag its boot by a little. Wait for it.
for _ in $(seq 1 60); do
  curl -fsS -o /dev/null --max-time 5 https://deb.debian.org/ && break
  sleep 3
done
for attempt in 1 2 3; do
  apt-get update -q -o Acquire::Retries=3 >/dev/null 2>&1 && \
    ! apt-get update -q 2>&1 | grep -q '^W: Failed' && break
  sleep $(( attempt * 10 ))
done
apt-get install -y -q --no-install-recommends git ca-certificates curl >/dev/null

id coder >/dev/null 2>&1 || useradd --create-home --shell /bin/bash coder
repo=/home/coder/openagents
if [[ ! -d "$repo/.git" ]]; then
  runuser -u coder -- git clone --quiet "$repo_url" "$repo"
fi
runuser -u coder -- git -C "$repo" fetch --quiet origin main
runuser -u coder -- git -C "$repo" checkout --quiet --detach "$rev"
serial "OA_CODER_HOST_BAKE phase=clone t=$(( $(date -u +%s) - started ))"

setup_args=(--user coder --repo-dir "$repo" --rev "$rev" --warm)
[[ -n "$bucket" ]] && setup_args+=(--sccache-bucket "$bucket")
# Progress lines go to the serial console as they happen.
# The setup script comes from the clone at that revision, unless the
# orchestrator passed a local copy (--local-setup, for testing a change
# before it lands).
setup="$repo/scripts/cloud/coder-host-setup.sh"
if md oa-setup-script >/var/tmp/oa-coder-host-setup.sh && [[ -s /var/tmp/oa-coder-host-setup.sh ]]; then
  setup=/var/tmp/oa-coder-host-setup.sh
  serial "OA_CODER_HOST_BAKE phase=setup source=metadata"
fi
bash "$setup" "${setup_args[@]}" 2>&1 \
  | while IFS= read -r line; do
      case "$line" in OA_CODER_HOST_SETUP*) serial "$line" ;; *) printf '%s\n' "$line" ;; esac
    done
[[ "${PIPESTATUS[0]}" == 0 ]]

# ---------------------------------------------------------------- ready unit
# On every boot of a host made from the image: fetch origin/main into the
# baked clone (bounded), check that sccache can reach its bucket (else turn
# it off for this boot), and print OA_CODER_HOST_READY on the serial console.
install -d -m 0755 /usr/local/libexec
cat >/usr/local/libexec/oa-coder-host-ready <<'READY'
#!/bin/bash
set -uo pipefail
install -d -m 0755 /run/oa-coder-host
rm -f /run/oa-coder-host/no-sccache /run/oa-coder-host/ready
# Egress is through Cloud NAT, which can lag a fresh VM's boot; retry.
fetch="failed"
for attempt in 1 2 3 4 5 6; do
  if timeout 30 runuser -u coder -- git -C /home/coder/openagents fetch --quiet origin main; then
    fetch="ok"; [[ $attempt == 1 ]] || fetch="ok-after-$attempt"; break
  fi
  sleep 5
done
sccache="ok"
runuser -u coder -- env HOME=/home/coder /usr/local/bin/sccache --stop-server >/dev/null 2>&1 || true
if ! timeout 30 runuser -u coder -- env HOME=/home/coder /usr/local/bin/sccache --start-server >/dev/null 2>&1; then
  sccache="off"
  touch /run/oa-coder-host/no-sccache
fi
head="$(runuser -u coder -- git -C /home/coder/openagents rev-parse --short=12 origin/main 2>/dev/null || echo unknown)"
uptime_s="$(cut -d' ' -f1 /proc/uptime)"
line="OA_CODER_HOST_READY uptime=${uptime_s} fetch=${fetch} sccache=${sccache} origin_main=${head}"
echo "$line" >/run/oa-coder-host/ready
echo "$line" >/dev/ttyS0 2>/dev/null || true
echo "$line"
READY
chmod 0755 /usr/local/libexec/oa-coder-host-ready
cat >/etc/systemd/system/oa-coder-host-ready.service <<'UNIT'
[Unit]
Description=Coder host: fetch origin/main and check sccache
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/local/libexec/oa-coder-host-ready
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
UNIT
systemctl enable oa-coder-host-ready.service >/dev/null 2>&1

# ---------------------------------------------------------------- seal
du_target="$(du -sb /home/coder/.openagents/targets 2>/dev/null | cut -f1 || echo 0)"
manifest="$(cat /home/coder/.openagents/coder-host.json)"
install -d -m 0755 /var/lib/oa-coder-host
cp /home/coder/.openagents/coder-host.json /var/lib/oa-coder-host/manifest.json
touch /var/lib/oa-coder-host/baked
apt-get clean
rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/* /root/.cache /home/coder/.npm /home/coder/.cache/sccache
rm -f /etc/ssh/ssh_host_*
truncate -s 0 /etc/machine-id
rm -f /var/lib/dbus/machine-id /var/lib/systemd/random-seed
ln -sf /etc/machine-id /var/lib/dbus/machine-id
journalctl --rotate >/dev/null 2>&1 || true
journalctl --vacuum-time=1s >/dev/null 2>&1 || true
fstrim -av >/dev/null 2>&1 || true
sync
used="$(df -B1 --output=used / | tail -1 | tr -d ' ')"
elapsed=$(( $(date -u +%s) - started ))
serial "OA_CODER_HOST_BAKE_OK $(jq -c --argjson elapsed "$elapsed" --argjson used "$used" --argjson target "$du_target" \
  '. + {bake_seconds:$elapsed, disk_used_bytes:$used, warm_target_bytes:$target}' <<<"$manifest")"

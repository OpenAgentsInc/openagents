#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/cloud/build-managed-sandbox-guest-image.sh \
    --project PROJECT --zone ZONE --image-name NAME [--apply]

Builds the immutable SBX-09 GCE guest image with Python guest drivers,
including the forensic worker preflight. Default mode is dry-run. The
retired Node SDKs are not installed.

With --with-coldcard-toolchain the image additionally carries a pinned
arm-none-eabi cross toolchain and the two pinned Coldcard firmware trees the
OFR-014 artifact witness builds. That toolchain is what lets a build actually
run inside an admitted guest instead of being described by a fixture.
USAGE
}

project=""
zone="us-central1-a"
image_name=""
apply="false"
coldcard="false"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --project) project="${2:-}"; shift 2 ;;
    --zone) zone="${2:-}"; shift 2 ;;
    --image-name) image_name="${2:-}"; shift 2 ;;
    --with-coldcard-toolchain) coldcard="true"; shift ;;
    --apply) apply="true"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done
if [[ -z "$project" || -z "$zone" || -z "$image_name" ]]; then
  usage >&2
  exit 2
fi
if [[ ! "$image_name" =~ ^[a-z]([-a-z0-9]{0,61}[a-z0-9])?$ ]]; then
  echo "image name is not a valid immutable GCE image name" >&2
  exit 2
fi

# Pinned Coldcard artifact-witness inputs (openagents #9296, OFR-014).
# The toolchain is pinned by exact package version, not by "whatever apt has":
# the artifact-witness suite refuses a matrix whose three builds do not share
# one toolchain digest, so a floating compiler would silently break the gate.
COLDCARD_VULNERABLE_COMMIT="bcc2c382a324690a2fcf972c0bac3b79bf923f7b"
COLDCARD_FIXED_COMMIT="ca72463709f4e3f8964952039d5caf955f566a87"
COLDCARD_GCC_VERSION="12.2.1"
COLDCARD_TOOLCHAIN_PACKAGES="gcc-arm-none-eabi=15:12.2.rel1-1 binutils-arm-none-eabi=2.40-2+18+b1 libnewlib-arm-none-eabi=3.3.0-1.3+deb12u1 libstdc++-arm-none-eabi-newlib=15:12.2.rel1-1+23"

revision="$(git rev-parse HEAD)"
stamp="$(date -u +%Y%m%d%H%M%S)-$$"
builder="oa-msb-image-builder-${stamp}"
smoke="${builder}-smoke"
setup_file="$(mktemp)"
smoke_file="$(mktemp)"
image_created="false"
image_admitted="false"
cleanup() {
  rm -f "$setup_file" "$smoke_file"
  if [[ "$apply" == "true" ]]; then
    gcloud compute instances delete "$smoke" \
      --project "$project" --zone "$zone" --quiet >/dev/null 2>&1 || true
    gcloud compute instances delete "$builder" \
      --project "$project" --zone "$zone" --quiet >/dev/null 2>&1 || true
    if [[ "$image_created" == "true" && "$image_admitted" != "true" ]]; then
      gcloud compute images delete "$image_name" \
        --project "$project" --quiet >/dev/null 2>&1 || true
    fi
  fi
}
trap cleanup EXIT

cat >"$setup_file" <<'SETUP'
#!/usr/bin/env bash
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends bubblewrap ca-certificates curl git iptables openssh-server python3 xz-utils
id openagents >/dev/null 2>&1 || useradd --create-home --shell /bin/bash openagents
install -d -o openagents -g openagents -m 0700 \
  /workspace \
  /var/lib/openagents/managed-sandbox-checkpoints \
  /var/lib/openagents/managed-sandbox-turns \
  /var/lib/openagents/managed-sandbox-io
install -d -o root -g root -m 0755 /opt/openagents-managed-sandbox
cat >/etc/tmpfiles.d/openagents-managed-sandbox.conf <<'TMPFILES'
d /run/openagents-managed-sandbox 0750 openagents openagents -
d /run/openagents-managed-sandbox/io 0700 openagents openagents -
TMPFILES
systemd-tmpfiles --create /etc/tmpfiles.d/openagents-managed-sandbox.conf
install -o root -g root -m 0755 /tmp/managed-sandbox-guest-turn.py \
  /opt/openagents-managed-sandbox/managed-sandbox-guest-turn.py
install -o root -g root -m 0755 /tmp/managed-sandbox-guest-interrupt.py \
  /opt/openagents-managed-sandbox/managed-sandbox-guest-interrupt.py
install -o root -g root -m 0755 /tmp/managed-sandbox-guest-io.py \
  /opt/openagents-managed-sandbox/managed-sandbox-guest-io.py
install -o root -g root -m 0755 /tmp/managed-sandbox-guest-checkpoint.py \
  /opt/openagents-managed-sandbox/managed-sandbox-guest-checkpoint.py
install -o root -g root -m 0755 /tmp/forensic-worker-driver.py \
  /opt/openagents-managed-sandbox/forensic-worker-driver.py
rm -f \
  /tmp/managed-sandbox-guest-turn.py \
  /tmp/managed-sandbox-guest-interrupt.py \
  /tmp/managed-sandbox-guest-io.py \
  /tmp/managed-sandbox-guest-checkpoint.py \
  /tmp/forensic-worker-driver.py
# The forensic residue and usage proofs must observe every process, including
# processes owned by other users. An unprivileged scan gets EACCES on another
# user's /proc/<pid>/fd, which makes the observation incomplete and is refused.
# Grant exactly the two read/settle subcommands of exactly that one absolute
# executable, and nothing else. No shell, no wildcard, no other binary.
install -o root -g root -m 0440 /dev/stdin /etc/sudoers.d/openagents-forensic-worker <<'SUDOERS'
openagents ALL=(root) NOPASSWD: /opt/openagents-managed-sandbox/forensic-worker-driver.py prepare-stop
openagents ALL=(root) NOPASSWD: /opt/openagents-managed-sandbox/forensic-worker-driver.py usage
SUDOERS
visudo -c -f /etc/sudoers.d/openagents-forensic-worker
cat >/etc/systemd/system/openagents-managed-sandbox-hostkeys.service <<'UNIT'
[Unit]
Description=Generate per-guest OpenSSH host keys
After=local-fs.target
Before=ssh.service

[Service]
Type=oneshot
ExecStart=/usr/bin/ssh-keygen -A
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
UNIT
systemctl enable openagents-managed-sandbox-hostkeys.service
cat >/etc/systemd/system/openagents-managed-sandbox-metadata-guard.service <<'UNIT'
[Unit]
Description=Block managed-sandbox workload access to GCE metadata
After=network-online.target
Before=google-startup-scripts.service ssh.service

[Service]
Type=oneshot
ExecStart=/bin/sh -c '/usr/sbin/iptables -C OUTPUT -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT 2>/dev/null || /usr/sbin/iptables -I OUTPUT 1 -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT'
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
UNIT
systemctl enable openagents-managed-sandbox-metadata-guard.service
printf '%s\n' \
  'PasswordAuthentication no' \
  'PermitRootLogin no' \
  'AllowUsers openagents' \
  >/etc/ssh/sshd_config.d/90-openagents-managed-sandbox.conf
SETUP

# The Coldcard artifact-witness toolchain is a separate, opt-in layer so the
# default managed-sandbox guest stays small. It is appended before the image
# cleanup below, not after, or apt state and caches would survive into the seal.
if [[ "$coldcard" == "true" ]]; then
  cat >>"$setup_file" <<SETUP_COLDCARD_PINS
COLDCARD_VULNERABLE_COMMIT='${COLDCARD_VULNERABLE_COMMIT}'
COLDCARD_FIXED_COMMIT='${COLDCARD_FIXED_COMMIT}'
COLDCARD_TOOLCHAIN_PACKAGES='${COLDCARD_TOOLCHAIN_PACKAGES}'
SETUP_COLDCARD_PINS
  cat >>"$setup_file" <<'SETUP_COLDCARD'
# shellcheck disable=SC2086
apt-get install -y --no-install-recommends $COLDCARD_TOOLCHAIN_PACKAGES \
  autoconf automake build-essential libffi-dev libtool make pkg-config python3-dev
# The Coldcard Node driver was deleted with the TypeScript lane. The
# opt-in toolchain still bakes the pinned firmware trees.
install -d -o root -g root -m 0755 /opt/coldcard
# Both pinned trees are materialized and 'make setup' is completed here, at
# image build time. A guest runs with the network unshared, so anything not
# baked in now can never be fetched later.
for pinned in "vulnerable:$COLDCARD_VULNERABLE_COMMIT" "fixed:$COLDCARD_FIXED_COMMIT"; do
  name="${pinned%%:*}"
  commit="${pinned##*:}"
  rm -rf "/tmp/coldcard-${name}"
  git clone -q --no-checkout https://github.com/Coldcard/firmware.git "/tmp/coldcard-${name}"
  git -C "/tmp/coldcard-${name}" checkout -q "$commit"
  git -C "/tmp/coldcard-${name}" submodule update -q --init --depth 1 \
    external/micropython external/libngu external/ckcc-protocol external/mpy-qr
  ( cd "/tmp/coldcard-${name}/stm32" && make -f MK-Makefile setup >/dev/null )
  mv "/tmp/coldcard-${name}" "/opt/coldcard/${name}"
done
printf '{"vulnerable":{"commitSha":"%s","repository":"%s"},"fixed":{"commitSha":"%s","repository":"%s"}}\n' \
  "$COLDCARD_VULNERABLE_COMMIT" https://github.com/Coldcard/firmware.git \
  "$COLDCARD_FIXED_COMMIT" https://github.com/Coldcard/firmware.git \
  >/opt/coldcard/pins.json
chown -R root:root /opt/coldcard
# The build copies these trees as the unprivileged workload user, and git's
# pack directories are owner-read-only by default.
chmod -R a+rX /opt/coldcard
SETUP_COLDCARD
fi

cat >>"$setup_file" <<'SETUP'
apt-get clean
rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*
rm -f /etc/ssh/ssh_host_*
# systemd requires the machine-id path to exist while it generates a fresh ID
# for the cloned boot. Removing the path makes systemd-networkd unable to
# derive its DHCP identity, which strands an otherwise RUNNING GCE guest.
truncate -s 0 /etc/machine-id
rm -f /var/lib/dbus/machine-id /var/lib/systemd/random-seed
install -d -m 0755 /var/lib/dbus
ln -s /etc/machine-id /var/lib/dbus/machine-id
truncate -s 0 /var/log/wtmp /var/log/btmp /var/log/lastlog 2>/dev/null || true
sync
SETUP

cat >"$smoke_file" <<'SMOKE'
#!/bin/sh
set -eu
test -s /etc/machine-id
ip -4 -o address show scope global | grep -q 'inet '
test -x /usr/bin/python3
test -x /opt/openagents-managed-sandbox/managed-sandbox-guest-turn.py
test -x /opt/openagents-managed-sandbox/managed-sandbox-guest-interrupt.py
test -x /usr/bin/setsid
test -x /opt/openagents-managed-sandbox/managed-sandbox-guest-io.py
test -x /opt/openagents-managed-sandbox/managed-sandbox-guest-checkpoint.py
test -x /opt/openagents-managed-sandbox/forensic-worker-driver.py
test -d /var/lib/openagents/managed-sandbox-checkpoints
test -d /var/lib/openagents/managed-sandbox-turns
test -d /var/lib/openagents/managed-sandbox-io
test -d /run/openagents-managed-sandbox/io
test "$(stat -c '%U:%G:%a' /var/lib/openagents/managed-sandbox-io)" = 'openagents:openagents:700'
test "$(stat -c '%U:%G:%a' /run/openagents-managed-sandbox/io)" = 'openagents:openagents:700'
# The runtime invokes prepare-stop as `openagents` over SSH. Prove here, in the
# image, that the narrow sudoers grant makes a COMPLETE process observation
# reachable that way. Without this the guest only fails at live stop time.
test "$(runuser -u openagents -- sudo -n \
  /opt/openagents-managed-sandbox/forensic-worker-driver.py prepare-stop \
  | sed -n 's/.*"processObservation":"\([a-z]*\)".*/\1/p')" = 'proc'
# The grant must not extend to any other subcommand of the same executable.
! runuser -u openagents -- sudo -n \
  /opt/openagents-managed-sandbox/forensic-worker-driver.py preflight >/dev/null 2>&1
test "$(runuser -u openagents -- /usr/bin/bwrap \
  --die-with-parent --unshare-net --unshare-pid --unshare-uts --unshare-ipc \
  --ro-bind / / --bind /workspace /workspace --tmpfs /run --proc /proc \
  --dev /dev --chdir /workspace /bin/pwd)" = '/workspace'
runuser -u openagents -- \
  /opt/openagents-managed-sandbox/forensic-worker-driver.py preflight >/dev/null
systemctl is-active --quiet openagents-managed-sandbox-hostkeys.service
systemctl is-active --quiet openagents-managed-sandbox-metadata-guard.service
systemctl is-active --quiet ssh.service
find /etc/ssh -maxdepth 1 -type f -name 'ssh_host_*_key' -size +0c | grep -q .
/usr/sbin/iptables -C OUTPUT -d 169.254.169.254/32 -m owner --uid-owner openagents -j REJECT
curl -fsS -H 'Metadata-Flavor: Google' \
  http://metadata.google.internal/computeMetadata/v1/instance/name >/dev/null
SMOKE

# The Coldcard checks run before the ready marker is printed, so an image that
# lacks a working cross toolchain or a pinned tree can never be admitted.
if [[ "$coldcard" == "true" ]]; then
  cat >>"$smoke_file" <<SMOKE_COLDCARD
test -x /usr/bin/arm-none-eabi-gcc
test -x /usr/bin/arm-none-eabi-nm
test -x /usr/bin/arm-none-eabi-objdump
test -x /usr/bin/arm-none-eabi-objcopy
test -x /usr/bin/arm-none-eabi-ar
test -x /usr/bin/make
test "\$(/usr/bin/arm-none-eabi-gcc -dumpversion)" = '${COLDCARD_GCC_VERSION}'
test "\$(git -C /opt/coldcard/vulnerable rev-parse HEAD)" = '${COLDCARD_VULNERABLE_COMMIT}'
test "\$(git -C /opt/coldcard/fixed rev-parse HEAD)" = '${COLDCARD_FIXED_COMMIT}'
test -s /opt/coldcard/pins.json
test -x /opt/coldcard/vulnerable/external/micropython/mpy-cross/mpy-cross
test -x /opt/coldcard/fixed/external/micropython/mpy-cross/mpy-cross
test -f /opt/coldcard/vulnerable/external/libngu/ngu/random.c
test -f /opt/coldcard/fixed/stm32/COLDCARD_MK4/rng.c
# The workload user must be able to read the trees it will copy, and it must
# still be unable to write into the pinned originals.
runuser -u openagents -- test -r /opt/coldcard/vulnerable/.git/HEAD
! runuser -u openagents -- touch /opt/coldcard/vulnerable/.oa-write-probe 2>/dev/null
# Prove the cross compiler actually emits Cortex-M4 code in this image.
runuser -u openagents -- sh -c 'cd /tmp && printf "int oa_probe(void){return 1;}" >oa-probe.c && /usr/bin/arm-none-eabi-gcc -mthumb -mcpu=cortex-m4 -c oa-probe.c -o oa-probe.o && /usr/bin/arm-none-eabi-nm --defined-only oa-probe.o | grep -q " T oa_probe" && rm -f oa-probe.c oa-probe.o'
SMOKE_COLDCARD
fi

# The ready marker is always appended last, so every check above it must pass
# before the builder is allowed to admit the image.
cat >>"$smoke_file" <<'SMOKE_READY'
printf 'OA_MSB_IMAGE_SMOKE_READY\n' >/dev/ttyS0
SMOKE_READY

if [[ "$apply" != "true" ]]; then
  cat <<SUMMARY
Managed-sandbox guest image dry run
  project:  $project
  zone:     $zone
  image:    $image_name
  revision: $revision
  builder:  $builder
  guest:    python3 turn/interrupt/io/checkpoint/forensic drivers
  coldcard: $coldcard${coldcard:+ (gcc $COLDCARD_GCC_VERSION; $COLDCARD_VULNERABLE_COMMIT / $COLDCARD_FIXED_COMMIT)}
SUMMARY
  exit 0
fi

if gcloud compute images describe "$image_name" --project "$project" >/dev/null 2>&1; then
  read -r existing_revision existing_status existing_id existing_boot_smoke < <(
    gcloud compute images describe "$image_name" \
      --project "$project" \
      --format='value(labels.openagents-source-revision,status,id,labels.openagents-boot-smoke)'
  )
  if [[ "$existing_revision" != "$revision" || "$existing_status" != "READY" || \
        -z "$existing_id" || "$existing_boot_smoke" != "passed" ]]; then
    echo "immutable image exists but does not match this source revision in READY state: $image_name" >&2
    exit 2
  fi
  existing_digest="$(printf '%s' "${project}|${image_name}|${existing_id}" | \
    shasum -a 256 | awk '{print $1}')"
  cat <<SUMMARY
Managed-sandbox guest image already admitted
  project:       $project
  imageName:     $image_name
  imageId:       $existing_id
  imageDigest:   sha256:$existing_digest
  sourceRevision:$revision
  bootSmoke:     passed
  guest:         python3 turn/interrupt/io/checkpoint/forensic drivers
SUMMARY
  exit 0
fi

gcloud compute instances create "$builder" \
  --project "$project" \
  --zone "$zone" \
  --machine-type e2-standard-2 \
  --image-family debian-12 \
  --image-project debian-cloud \
  --boot-disk-size 20GB \
  --no-service-account \
  --no-scopes \
  --labels "openagents-managed=image-builder,openagents-component=managed-sandbox-guest"

for _ in $(seq 1 30); do
  if gcloud compute ssh "openagents@${builder}" \
    --project "$project" --zone "$zone" --quiet \
    --command 'true' >/dev/null 2>&1; then
    break
  fi
  sleep 5
done

gcloud compute scp \
  scripts/cloud/managed-sandbox-guest-turn.py \
  "openagents@${builder}:/tmp/managed-sandbox-guest-turn.py" \
  --project "$project" --zone "$zone" --quiet
gcloud compute scp \
  scripts/cloud/managed-sandbox-guest-interrupt.py \
  "openagents@${builder}:/tmp/managed-sandbox-guest-interrupt.py" \
  --project "$project" --zone "$zone" --quiet
gcloud compute scp \
  scripts/cloud/managed-sandbox-guest-io.py \
  "openagents@${builder}:/tmp/managed-sandbox-guest-io.py" \
  --project "$project" --zone "$zone" --quiet
gcloud compute scp \
  scripts/cloud/managed-sandbox-guest-checkpoint.py \
  "openagents@${builder}:/tmp/managed-sandbox-guest-checkpoint.py" \
  --project "$project" --zone "$zone" --quiet
gcloud compute scp \
  scripts/cloud/forensic-worker-driver.py \
  "openagents@${builder}:/tmp/forensic-worker-driver.py" \
  --project "$project" --zone "$zone" --quiet
if [[ "$coldcard" == "true" ]]; then
  true
fi
gcloud compute scp "$setup_file" "openagents@${builder}:/tmp/setup.sh" \
  --project "$project" --zone "$zone" --quiet
gcloud compute ssh "openagents@${builder}" \
  --project "$project" --zone "$zone" --quiet \
  --command 'sudo bash /tmp/setup.sh'
gcloud compute instances stop "$builder" --project "$project" --zone "$zone" --quiet
gcloud compute images create "$image_name" \
  --project "$project" \
  --source-disk "$builder" \
  --source-disk-zone "$zone" \
  --family oa-managed-sandbox-guest-v1 \
  --labels "openagents-managed=managed-sandbox-image,openagents-contract=managed-sandbox-v1,openagents-source-revision=${revision},openagents-boot-smoke=pending"
image_created="true"

# Boot the sealed image once before admission. The marker proves that DHCP,
# metadata startup, per-guest SSH host keys, and the workload metadata guard
# all survive cloning. This is intentionally a private, no-identity VM.
gcloud compute instances create "$smoke" \
  --project "$project" \
  --zone "$zone" \
  --machine-type e2-small \
  --image "$image_name" \
  --image-project "$project" \
  --no-address \
  --no-service-account \
  --no-scopes \
  --metadata "block-project-ssh-keys=TRUE,enable-oslogin=FALSE,disable-legacy-endpoints=TRUE,serial-port-enable=TRUE" \
  --metadata-from-file "startup-script=${smoke_file}" \
  --labels "openagents-managed=image-smoke,openagents-component=managed-sandbox-guest"

smoke_ready="false"
for _ in $(seq 1 60); do
  if gcloud compute instances get-serial-port-output "$smoke" \
       --project "$project" --zone "$zone" --port 1 2>/dev/null | \
       grep -Fq 'OA_MSB_IMAGE_SMOKE_READY'; then
    smoke_ready="true"
    break
  fi
  sleep 5
done
if [[ "$smoke_ready" != "true" ]]; then
  echo "sealed managed-sandbox image failed its private boot smoke: $image_name" >&2
  gcloud compute instances get-serial-port-output "$smoke" \
    --project "$project" --zone "$zone" --port 1 2>/dev/null | tail -120 >&2 || true
  exit 1
fi
gcloud compute images add-labels "$image_name" \
  --project "$project" --labels openagents-boot-smoke=passed >/dev/null
image_admitted="true"

image_id="$(gcloud compute images describe "$image_name" \
  --project "$project" --format='value(id)')"
image_digest="$(printf '%s' "${project}|${image_name}|${image_id}" | shasum -a 256 | awk '{print $1}')"
cat <<SUMMARY
Managed-sandbox guest image built
  project:       $project
  imageName:     $image_name
  imageId:       $image_id
  imageDigest:   sha256:$image_digest
  sourceRevision:$revision
  bootSmoke:     private DHCP + startup + hostkeys + metadata guard passed
  guest:         python3 turn/interrupt/io/checkpoint/forensic drivers
  coldcard:      $coldcard${coldcard:+ (gcc $COLDCARD_GCC_VERSION; $COLDCARD_VULNERABLE_COMMIT / $COLDCARD_FIXED_COMMIT)}
SUMMARY

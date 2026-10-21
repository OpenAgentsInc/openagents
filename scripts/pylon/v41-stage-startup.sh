#!/bin/sh
# Startup script for a DeepSeek-V4.1 stage machine (#12402). `pylon cloud up`
# passes it as the machine's startup-script with these metadata values:
#   oa-weights     the checkpoint's gs:// prefix (never Hugging Face)
#   oa-layers      this machine's stages' layer ranges, such as 0-10+10-20
#   oa-embeddings  1 when a stage here holds the embeddings
#   oa-head        1 when a stage here holds the final norm and head
#   oa-worker      the stage worker binary's gs:// URL (W11), as
#                  scripts/pylon/v41-stage-build.sh uploads it:
#                  .../psionic-v41-stage/COMMIT/sm_NN/psionic-v41-stage,
#                  with the group manifests beside it in manifests/ and
#                  the CUDA 13 libraries it opens in lib/
#   oa-manifest    the group manifest's SHA-256
#   oa-build       optional: the SHA-256 of the build's BUILD.json, which
#                  pins the worker, its libraries, its target, and its
#                  profile to the qualified execution configuration
# and, when the group head runs in the zone (#12399, `--head stage0|vm`):
#   oa-role        `head` on a machine that only heads the group
#   oa-group-head  1 on the machine the head runs on
#   oa-pylon       the pylon binary's gs:// URL, for that machine, with its
#                  BUILD.json beside it
#   oa-pylon-build optional: the SHA-256 of that BUILD.json
#   oa-stage-ssh   scripts/pylon/v41-stage-ssh.sh, for that machine
#
# It first removes the ready mark, so nothing trusts a machine whose
# installation is changing. It then fetches the worker's BUILD.json (schema
# v2, written by scripts/pylon/v41-stage-build.sh) and installs the build
# only if the worker and every CUDA library hash to what BUILD.json lists,
# BUILD.json names this machine's GPU architecture, and the group manifest
# is one the build was made with (#12407). The build goes to a staging
# folder and is renamed into /opt/openagents/builds/BUILD_SHA256 whole. It
# copies only the shards the stages read (W9's stage selection), the
# tokenizer files, and the group manifest into the weights folder, and
# installs /opt/openagents/stage-run, which the group head starts over ssh
# for each stage, so the head owns every worker's lifetime. On the head's
# machine it also installs /opt/openagents/head-run and stage-ssh, after the
# same check of the pylon binary against its BUILD.json.
#
# The ready mark is written last, atomically, and names this boot, the
# manifest, and the build:
#
#   boot BOOT_ID
#   manifest SHA256
#   build SHA256
#
# stage-run starts a worker only when the mark names the current boot and
# its own manifest and build, and rechecks the build's hashes first; the
# head's ssh probe (v41-head-ssh.sh) waits for a mark of the current boot.
# A failed, interrupted, or older boot's installation is never ready.
#
# Local mode, for tests: OA_STAGE_METADATA=DIR reads each metadata value from
# the file DIR/KEY instead of the metadata server; OA_STAGE_ROOT and
# OA_STAGE_OPT replace /mnt/stage and /opt/openagents; OA_GCLOUD and
# OA_NVIDIA_SMI replace gcloud and nvidia-smi; OA_BOOT_ID replaces
# /proc/sys/kernel/random/boot_id.
set -eu
root=${OA_STAGE_ROOT:-/mnt/stage}
opt=${OA_STAGE_OPT:-/opt/openagents}
gcloud=${OA_GCLOUD:-gcloud}
# A G4 machine's Titanium SSDs read far faster than its boot disk: stripe
# them into one volume at $root before copying any weights. A restart
# remounts the existing array.
if [ -z "${OA_STAGE_METADATA:-}" ] && ! mountpoint -q "$root"; then
  set -- /dev/disk/by-id/google-local-nvme-ssd-*
  if [ -e "$1" ]; then
    mkdir -p "$root"
    if [ "$#" -eq 1 ]; then
      dev=$1
    else
      dev=/dev/md/oa-stage
      mdadm --assemble --scan >/dev/null 2>&1 || true
      [ -e "$dev" ] || mdadm --create "$dev" --name=oa-stage --level=0 --force \
        --raid-devices="$#" "$@" --run >/dev/null 2>&1
    fi
    blkid "$dev" >/dev/null 2>&1 ||
      mkfs.ext4 -q -F -E lazy_itable_init=1,lazy_journal_init=1 "$dev"
    mount -o noatime "$dev" "$root"
    chmod 755 "$root"
  fi
fi
mkdir -p "$root/weights" "$opt"
# Nothing may trust this machine until every check below passes.
rm -f "$root/ready" "$root/ready.new"
exec >>"$root/startup.log" 2>&1
boot=$(cat "${OA_BOOT_ID:-/proc/sys/kernel/random/boot_id}")
md() {
  if [ -n "${OA_STAGE_METADATA:-}" ]; then
    cat "$OA_STAGE_METADATA/$1"
    return
  fi
  curl -fsS -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/$1"
}
optional() { md "$1" 2>/dev/null || true; }
fail() {
  echo "oa-stage: $*"
  exit 1
}
# The checker stage-run and head-run use too. `build DIR` checks DIR's
# worker and libraries against DIR/BUILD.json and prints its architecture;
# `file PATH SHA256` checks one file; `field JSON KEY` prints a field.
cat >"$opt/oa-verify.new" <<'PY'
import hashlib, json, os, sys

def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()

def refuse(message):
    print(message, file=sys.stderr)
    sys.exit(1)

verb = sys.argv[1]
if verb == "file":
    got = sha256(sys.argv[2])
    if got != sys.argv[3]:
        refuse(f"{sys.argv[2]} hashes to {got}, not {sys.argv[3]}")
elif verb == "field":
    value = json.load(open(sys.argv[2])).get(sys.argv[3])
    if not isinstance(value, str) or not value:
        refuse(f"{sys.argv[2]} has no {sys.argv[3]}")
    print(value)
elif verb == "build":
    root = sys.argv[2]
    build = json.load(open(os.path.join(root, "BUILD.json")))
    if build.get("schema") != "openagents.psionic_v41_stage.build.v2":
        refuse(f"BUILD.json has schema {build.get('schema')}; rebuild with scripts/pylon/v41-stage-build.sh")
    libraries = build.get("libraries") or {}
    if not libraries:
        refuse("BUILD.json lists no CUDA libraries")
    got = sha256(os.path.join(root, "psionic-v41-stage"))
    if got != build.get("sha256"):
        refuse(f"the worker hashes to {got}, but BUILD.json says {build.get('sha256')}")
    present = sorted(os.listdir(os.path.join(root, "lib")))
    if present != sorted(libraries):
        refuse(f"lib/ holds {present}, but BUILD.json lists {sorted(libraries)}")
    for name, want in sorted(libraries.items()):
        got = sha256(os.path.join(root, "lib", name))
        if got != want:
            refuse(f"lib/{name} hashes to {got}, but BUILD.json says {want}")
    if len(sys.argv) > 3 and sys.argv[3] not in (build.get("manifests") or {}):
        refuse(f"the build was not made with group manifest {sys.argv[3]}")
    print(build.get("arch", ""))
else:
    refuse(f"unknown check {verb}")
PY
mv "$opt/oa-verify.new" "$opt/oa-verify"
verify() { python3 "$opt/oa-verify" "$@"; }
# Writes the ready mark atomically, last.
publish_ready() {
  printf 'boot %s\n%s' "$boot" "$1" >"$root/ready.new"
  mv "$root/ready.new" "$root/ready"
}
# The budget host starts the head as soon as head-run exists, and the head
# checks every stage program at once, so head-run comes after stage-run and
# before the long weight copy (stage-run waits for the weights itself).
install_head() {
  [ "$(optional oa-group-head)" = 1 ] || return 0
  pylon_url=$(md oa-pylon)
  rm -rf "$opt/pylon.staging"
  mkdir -p "$opt/pylon.staging"
  "$gcloud" storage cp "$pylon_url" "${pylon_url%/*}/BUILD.json" "$opt/pylon.staging/"
  pylon_build=$(sha256sum "$opt/pylon.staging/BUILD.json" | cut -d ' ' -f 1)
  pin=$(optional oa-pylon-build)
  [ -z "$pin" ] || [ "$pin" = "$pylon_build" ] ||
    fail "the pylon build is $pylon_build, but this group runs $pin"
  pylon_sha=$(verify field "$opt/pylon.staging/BUILD.json" sha256) ||
    fail "the pylon build has no sha256"
  verify file "$opt/pylon.staging/${pylon_url##*/}" "$pylon_sha" ||
    fail "the pylon binary does not match its BUILD.json"
  chmod 755 "$opt/pylon.staging/${pylon_url##*/}"
  mv "$opt/pylon.staging/${pylon_url##*/}" "$opt/pylon-$pylon_sha"
  rm -rf "$opt/pylon.staging"
  md oa-stage-ssh >"$opt/stage-ssh.new"
  chmod 755 "$opt/stage-ssh.new"
  mv "$opt/stage-ssh.new" "$opt/stage-ssh"
  cat >"$opt/head-run.new" <<RUN
#!/bin/sh
# Head the group in the foreground of the budget host's ssh session; the
# launch arrives on standard input, and the session ending stops the head.
python3 $opt/oa-verify file $opt/pylon-$pylon_sha $pylon_sha || exit 1
exec $opt/pylon-$pylon_sha cloud head --home "\$HOME/.openagents/v41-head"
RUN
  chmod 755 "$opt/head-run.new"
  mv "$opt/head-run.new" "$opt/head-run"
  echo "oa-stage: the group head can start here (pylon $pylon_sha)"
}
if [ "$(optional oa-role)" = head ]; then
  install_head
  publish_ready "head"
  exit 0
fi
weights=$(md oa-weights)
layers=$(md oa-layers)
embeddings=$(md oa-embeddings)
head=$(md oa-head)
worker=$(md oa-worker)
manifest=$(md oa-manifest)
echo "oa-stage: layers $layers from $weights"
# Stage the whole build, check it against BUILD.json, and refuse a build for
# another GPU architecture, all before copying any weights.
staging="$opt/build.staging"
rm -rf "$staging"
mkdir -p "$staging/lib"
"$gcloud" storage cp "${worker%/*}/BUILD.json" "$staging/"
build=$(sha256sum "$staging/BUILD.json" | cut -d ' ' -f 1)
pin=$(optional oa-build)
[ -z "$pin" ] || [ "$pin" = "$build" ] ||
  fail "the worker build is $build, but this group runs $pin"
"$gcloud" storage cp "$worker" "$staging/psionic-v41-stage"
"$gcloud" storage cp "${worker%/*}/lib/*" "$staging/lib/"
arch=$(verify build "$staging" "$manifest") || fail "the worker build $build does not check out"
have=$("${OA_NVIDIA_SMI:-nvidia-smi}" --query-gpu=compute_cap --format=csv,noheader |
  head -n 1 | tr -d '. ')
if [ "${arch#sm_}" != "$have" ]; then
  fail "the worker is built for $arch, but this GPU is sm_$have"
fi
case $worker in
  */sm_*/*)
    path_arch=${worker%/*}
    path_arch=${path_arch##*/}
    [ "$path_arch" = "$arch" ] || fail "the worker's path says $path_arch, but BUILD.json says $arch"
    ;;
esac
chmod 755 "$staging/psionic-v41-stage"
installed="$opt/builds/$build"
mkdir -p "$opt/builds"
rm -rf "$installed"
mv "$staging" "$installed"
cat >"$opt/stage-run.new" <<RUN
#!/bin/sh
# Wait for this boot's installation of manifest $manifest and build $build,
# recheck the build, then run one stage worker in the foreground of the
# head's ssh session; the session ending ends it.
want="boot \$(cat ${OA_BOOT_ID:-/proc/sys/kernel/random/boot_id})
manifest $manifest
build $build"
i=0
while [ "\$(cat $root/ready 2>/dev/null)" != "\$want" ]; do
  i=\$((i + 1))
  [ "\$i" -le "\${OA_STAGE_WAIT:-3600}" ] || { echo "this machine's stage installation is not ready" >&2; exit 1; }
  sleep 1
done
python3 $opt/oa-verify build $installed $manifest >/dev/null || exit 1
export LD_LIBRARY_PATH=$installed/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}
exec $installed/psionic-v41-stage --weights $root/weights "\$@"
RUN
chmod 755 "$opt/stage-run.new"
mv "$opt/stage-run.new" "$opt/stage-run"
echo "oa-stage: installed build $build for $arch"
install_head
"$gcloud" storage cp "${weights}model.safetensors.index.json" "${weights}config.json" \
  "${weights}tokenizer.json" "${weights}tokenizer_config.json" "$root/weights/"
python3 - "$root/weights/model.safetensors.index.json" "$layers" "$embeddings" "$head" \
  >"$root/shards" <<'PY'
import json, sys
index, layers, embeddings, head = sys.argv[1], sys.argv[2], sys.argv[3] == "1", sys.argv[4] == "1"
spans = [tuple(int(n) for n in span.split("-")) for span in layers.split("+")]
want = set()
for name, shard in json.load(open(index))["weight_map"].items():
    part = name.split(".")
    if part[0] == "layers" and any(a <= int(part[1]) < b for a, b in spans):
        want.add(shard)
    elif (name == "embed.weight" and embeddings) or (name in ("norm.weight", "head.weight") and head):
        want.add(shard)
print("\n".join(sorted(want)))
PY
sed "s|^|$weights|" "$root/shards" | "$gcloud" storage cp -I "$root/weights/"
# The group manifest, which each stage names by its SHA-256.
mkdir -p "$root/weights/manifests"
"$gcloud" storage cp "${worker%/*}/manifests/$manifest.json" "$root/weights/manifests/"
got=$(sha256sum "$root/weights/manifests/$manifest.json" | cut -d ' ' -f 1)
if [ "$got" != "$manifest" ]; then
  fail "the group manifest hashes to $got, not $manifest"
fi
publish_ready "manifest $manifest
build $build"
echo "oa-stage: ready with $(wc -l <"$root/shards") shards, manifest $manifest, build $build"

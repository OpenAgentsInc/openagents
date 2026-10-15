#!/usr/bin/env bash
# Add our public runtime binaries to SPACE's bootable seed, then publish it.
# Run as space-spike after install.sh, with binaries extracted from our image
# under /var/lib/space-spike/runtime/bin. No account files are copied.
set -euo pipefail
export PATH="$HOME/.local/bin:$PATH"
upstream="$HOME/SPACE"
seed="$HOME/.space-devbox/kind/seed/build"
out="$HOME/oa-coder-runtime"
mount_dir="$HOME/runtime-mount"
test -f "$seed/rootfs.ext4"
test -x /var/lib/space-spike/runtime/bin/coder
test -x /var/lib/space-spike/runtime/bin/claude
mkdir -p "$out" "$mount_dir" "$HOME/evidence"
test ! -e "$out/manifest.json" || { echo 'Keep the previous template; use a fresh output directory.' >&2; exit 1; }
cp --reflink=auto "$seed/rootfs.ext4" "$out/rootfs.ext4"
truncate -s 4G "$out/rootfs.ext4"
sudo e2fsck -pf "$out/rootfs.ext4" || [[ $? == 1 ]]
sudo resize2fs "$out/rootfs.ext4"
sudo mount -o loop "$out/rootfs.ext4" "$mount_dir"
cleanup() {
  for path in dev proc ''; do
    if mountpoint -q "$mount_dir${path:+/$path}"; then sudo umount "$mount_dir${path:+/$path}"; fi
  done
}
trap cleanup EXIT
sudo install -m 0755 /var/lib/space-spike/runtime/bin/coder /var/lib/space-spike/runtime/bin/claude "$mount_dir/usr/local/bin/"
sudo mount --bind /dev "$mount_dir/dev"
sudo mount -t proc proc "$mount_dir/proc"
# These packages support the memory and network acceptance checks.
sudo cp -a "$mount_dir/etc/resolv.conf" "$out/resolv.conf.original"
sudo rm -f "$mount_dir/etc/resolv.conf"
sudo cp -L /etc/resolv.conf "$mount_dir/etc/resolv.conf"
sudo chroot "$mount_dir" apt-get update
sudo chroot "$mount_dir" env DEBIAN_FRONTEND=noninteractive apt-get install -y python3 curl ca-certificates git
sudo rm -f "$mount_dir/etc/resolv.conf"
sudo cp -a "$out/resolv.conf.original" "$mount_dir/etc/resolv.conf"
sudo chroot "$mount_dir" coder --version
sudo chroot "$mount_dir" claude --version
cleanup
trap - EXIT
python3 - "$seed" "$out" <<'PY'
import datetime, hashlib, json, pathlib, sys
seed,out=map(pathlib.Path,sys.argv[1:])
metadata=json.loads((seed/'metadata.json').read_text())
metadata.update(name='oa-coder-runtime',id='oa-coder-runtime',description='OpenAgents Coder runtime for the SPACE spike',vcpu=2,memory_mib=2048,disk_size_mb=4096,built_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
(out/'metadata.json').write_text(json.dumps(metadata,indent=2)+'\n')
h=hashlib.sha256()
with (out/'rootfs.ext4').open('rb') as f:
 for chunk in iter(lambda:f.read(1<<20),b''): h.update(chunk)
digest='sha256:'+h.hexdigest()
manifest=json.loads((seed/'manifest.json').read_text())
manifest.update(name=metadata['name'],content_hash=digest,build_date=metadata['built_at'])
manifest['files']['rootfs.ext4']={'digest':digest,'size':(out/'rootfs.ext4').stat().st_size}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('Runtime template digest:',digest)
PY
cp "$out/manifest.json" "$HOME/evidence/runtime-manifest.json"
sha256sum /var/lib/space-spike/runtime/bin/* >"$HOME/evidence/runtime-binaries.sha256"
cd "$upstream"
# Reuse the upstream publisher and its existing, local cluster identity.
docker buildx build --load -f docker/Dockerfile.seed-publish --build-context "seed=$out" -t oa-space-runtime-publish:local .
kind load docker-image oa-space-runtime-publish:local --name space-local
ready=0
for attempt in {1..120}; do
  if kubectl --context kind-space-local -n space get job space-seed-publish >/dev/null 2>&1; then ready=1; break; fi
  sleep 5
done
[[ "$ready" == 1 ]] || { echo 'The seed publication job did not appear.' >&2; exit 1; }
kubectl --context kind-space-local -n space wait --for=condition=complete job/space-seed-publish --timeout=600s
kubectl --context kind-space-local -n space get job space-seed-publish -o json |
  python3 -c 'import json,sys
j=json.load(sys.stdin)
j.pop("status",None)
j["metadata"]={"name":"oa-space-runtime-publish","namespace":"space"}
j["spec"].pop("selector",None)
j["spec"]["template"]["metadata"]={}
j["spec"]["template"]["spec"]["containers"][0]["image"]="oa-space-runtime-publish:local"
print(json.dumps(j))' | kubectl --context kind-space-local apply -f -
kubectl --context kind-space-local -n space wait --for=condition=complete job/oa-space-runtime-publish --timeout=600s
kubectl --context kind-space-local -n space logs job/oa-space-runtime-publish >"$HOME/evidence/runtime-publication.log"
echo 'Runtime template published as oa-coder-runtime:latest.'

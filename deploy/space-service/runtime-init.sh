#!/usr/bin/env bash
# Install the pinned execution pair once; never overwrite a referenced runtime.
set -euo pipefail
[[ $(id -u) == 0 ]] && mountpoint -q /space
space_runtime_marker=/space/config/runtime-install.json
if [[ -f $space_runtime_marker ]]; then
  python3 - "$space_runtime_marker" <<'PY'
import hashlib,json,sys
from pathlib import Path
manifest=json.loads(Path(sys.argv[1]).read_text())
assert manifest['source']=='de87b436c35dac056564bdd5b2a113c9046e189b'
for filename,want in manifest['files'].items():
    assert hashlib.file_digest(open(filename,'rb'),'sha256').hexdigest()==want
print('The installed runtime matches its saved hashes.')
PY
  exit 0
fi
[[ ! -e /space/vendor/firecracker && ! -L /space/vendor/firecracker ]]
[[ ! -e /space/vendor/kernels && ! -L /space/vendor/kernels ]]
install -d -m 0700 /space/config /space/scratch/runtime-prepare
space_runtime_installer=/space/config/firecracker-install-de87b436.sh
curl --fail --silent --show-error --location --retry 3 \
  https://raw.githubusercontent.com/perplexityai/SPACE/de87b436c35dac056564bdd5b2a113c9046e189b/infra/space/packer/scripts/base/04-firecracker.sh \
  -o "$space_runtime_installer"
printf '%s  %s\n' 401c6467e60e01110ab4ad0309ba9b30e47dcbc4d66a8d44caa83e7ea003ee64 "$space_runtime_installer" | sha256sum --check
TMPDIR=/space/scratch/runtime-prepare FIRECRACKER_DEFAULT_VERSION=v1.15.1 \
  FIRECRACKER_VERSIONS=v1.15.1 FIRECRACKER_KERNEL_CI_VERSIONS=v1.15 \
  bash "$space_runtime_installer"
python3 - "$space_runtime_marker" <<'PY'
import hashlib,json,os,sys
from pathlib import Path
files=['/space/vendor/firecracker/v1.15.1/bin/firecracker','/space/vendor/firecracker/v1.15.1/bin/jailer','/space/vendor/kernels/6.1.155/vmlinux.bin']
hashes={name:hashlib.file_digest(open(name,'rb'),'sha256').hexdigest() for name in files}
assert hashes[files[-1]]=='e20e46d0c36c55c0d1014eb20576171b3f3d922260d9f792017aeff53af3d4f2'
marker=Path(sys.argv[1]); staging=marker.with_suffix('.new')
staging.write_text(json.dumps({'source':'de87b436c35dac056564bdd5b2a113c9046e189b','firecracker':'v1.15.1','guest_kernel':'6.1.155','files':hashes},indent=2)+'\n')
os.chmod(staging,0o600); staging.replace(marker)
print('Pinned Firecracker, Jailer, and guest kernel installed; hashes saved.')
PY

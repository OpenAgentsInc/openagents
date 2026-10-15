#!/usr/bin/env bash
# Install a checked candidate and create disabled settings only when absent.
set -euo pipefail
[[ $EUID == 0 && $# == 3 ]] || { echo 'Usage: sudo stage-payouts.sh COMMIT BINARY SHA256' >&2; exit 2; }
revision=$1
artifact=$2
expected=$3
[[ $revision =~ ^[a-f0-9]{40}$ && $expected =~ ^[a-f0-9]{64}$ ]] || exit 2
[[ $(sha256sum "$artifact" | cut -d' ' -f1) == "$expected" ]] || exit 1
destination=/opt/openagents-pay/lnd-candidates/$revision
install -d -m 0755 "$destination"
install -m 0755 "$artifact" "$destination/openagents"
(cd "$destination" && sha256sum openagents > openagents.sha256)
# These files outlive releases. Never replace operator policy or settings.
python3 - "$(dirname "$0")" <<'PY'
import json,pathlib,sys,hashlib,subprocess,grp,os
source=pathlib.Path(sys.argv[1]);target=pathlib.Path('/etc/openagents-pay')
pin=hashlib.sha256(subprocess.check_output(['openssl','x509','-in','/run/openagents-lnd-receive/tls.cert','-outform','DER'])).hexdigest()
for name,template in [('payout-policy.json','payout-policy.example.json'),('lnd-payout.json','payout.example.json')]:
    path=target/name
    if path.exists(): continue
    data=json.loads((source/template).read_text())
    if 'certificate_sha256' in data:data['certificate_sha256']=pin
    with path.open('x') as out:out.write(json.dumps(data,indent=2)+'\n')
    os.chown(path,0,grp.getgrnam('openagents-pay').gr_gid);path.chmod(0o640)
PY
echo 'Payout candidate staged; the running worker and spending policy were preserved.'

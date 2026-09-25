#!/usr/bin/env python3
"""Exercise review isolation with synthetic files, without inference or grades."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

from reproduce import container_command, execution, write


def fingerprint(app):
    return {str(p.relative_to(app)): {
        'sha256': hashlib.sha256(p.read_bytes()).hexdigest(),
        'mode': oct(p.stat().st_mode & 0o777),
        'uid': p.stat().st_uid, 'gid': p.stat().st_gid,
    } for p in sorted(app.rglob('*')) if p.is_file()}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--image', required=True, help='Pinned public image with sh and cc')
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    a.out.mkdir(parents=True, exist_ok=False)
    image = json.loads(subprocess.check_output(['docker', 'image', 'inspect', a.image], text=True))[0]
    write(a.out / 'image.json', {k: image.get(k) for k in ('Id', 'RepoDigests', 'Architecture', 'Os')})
    if image['Config'].get('User') not in (None, '', 'root', '0', '0:0'):
        raise ValueError('The negative control requires a root-default public image')
    with tempfile.TemporaryDirectory(prefix='truth9644-fixture-') as temp:
        app = Path(temp) / 'app'
        app.mkdir()
        if app.stat().st_uid == 0:
            raise ValueError('Run this fixture as the non-root snapshot restore user')
        (app / 'owner-only.txt').write_text('owner-readable synthetic fixture\n')
        (app / 'owner-only.txt').chmod(0o600)
        before = fingerprint(app)
        identity = hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest()
        write(a.out / 'candidate-before.json', before)
        records = []
        commands = {
            'owner_read': 'cat /app/owner-only.txt',
            'scratch_execute': "printf 'int main(void) { return 0; }\\n' > /tmp/control.c && cc /tmp/control.c -o /tmp/control && /tmp/control",
            'candidate_write': 'printf changed > /app/owner-only.txt',
            'root_write': 'touch /etc/truth9644-write-test',
        }
        for profile in ('original', 'owner-exec'):
            settings = execution(profile, app)
            command = container_command(image['Id'], app, identity, settings)
            container = None
            record = {'profile': profile, 'settings': settings, 'command': command, 'probes': {}}
            try:
                container = subprocess.check_output(command, text=True, timeout=30).strip()
                inspection = json.loads(subprocess.check_output(['docker', 'inspect', container], text=True))[0]
                host = inspection['HostConfig']
                mounts = inspection['Mounts']
                record['isolation'] = {k: host[k] for k in (
                    'ReadonlyRootfs', 'NetworkMode', 'CapDrop', 'SecurityOpt',
                    'Memory', 'MemorySwap', 'PidsLimit', 'NanoCpus', 'Tmpfs', 'Privileged')}
                record['user'] = inspection['Config']['User']
                record['mounts'] = mounts
                assert host['ReadonlyRootfs'] and host['NetworkMode'] == 'none'
                assert host['CapDrop'] == ['ALL'] and not host['Privileged']
                assert 'no-new-privileges' in host['SecurityOpt']
                assert host['Memory'] == host['MemorySwap'] == 2 * 1024**3
                assert host['PidsLimit'] == 128 and host['NanoCpus'] == 2_000_000_000
                assert len(mounts) == 1 and mounts[0]['Destination'] == '/app' and not mounts[0]['RW']
                for name, script in commands.items():
                    started = time.monotonic()
                    result = subprocess.run(['docker', 'exec', container, 'sh', '-c', script],
                                            capture_output=True, text=True, timeout=30)
                    record['probes'][name] = {'command': script, 'exit': result.returncode,
                                             'stdout': result.stdout, 'stderr': result.stderr,
                                             'seconds': time.monotonic() - started}
                probes = record['probes']
                if profile == 'owner-exec':
                    assert probes['owner_read']['exit'] == 0
                    assert probes['owner_read']['stdout'] == 'owner-readable synthetic fixture\n'
                    assert probes['scratch_execute']['exit'] == 0
                else:
                    assert probes['owner_read']['exit'] != 0
                    assert probes['scratch_execute']['exit'] == 126
                assert probes['candidate_write']['exit'] != 0 and probes['root_write']['exit'] != 0
                record['candidate_after'] = fingerprint(app)
                assert record['candidate_after'] == before
                record['passed'] = True
            finally:
                if container:
                    cleanup = subprocess.run(['docker', 'rm', '-f', container], capture_output=True, text=True, timeout=30)
                    record['cleanup_exit'] = cleanup.returncode
                write(a.out / (profile + '.json'), record)
            records.append(record)
        write(a.out / 'result.json', {'schema': 'openagents.review-profile-fixture.v1',
                                     'passed': all(r['passed'] for r in records),
                                     'inference_calls': 0, 'official_grades_read': 0,
                                     'image': image['Id'], 'candidate_identity': identity})
        print('Both profiles verified; candidate bytes and modes unchanged; no inference or grades')


if __name__ == '__main__':
    main()

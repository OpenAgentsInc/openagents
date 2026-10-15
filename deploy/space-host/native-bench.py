#!/usr/bin/env python3
"""Measure explicit node cold and base-snapshot creates on the spike host.

The pinned Kind shim cold-boots public HTTP creates even when a base snapshot
exists. This separate measurement uses the unchanged node CLI's explicit boot
selection. Its timing ends at guest readiness, without an HTTP command.
"""

import json
import pathlib
import re
import subprocess
import sys
import time

arm, output = sys.argv[1:]
assert arm in ('cold', 'snapshot')
path = pathlib.Path(output)
assert not path.exists(), 'Keep previous results; use a fresh output path.'
home = pathlib.Path.home()
manifest = json.loads((home / 'evidence/runtime-manifest.json').read_text())
node = 'space-local-worker2'
report = {'schema': 'openagents.space_native_spike.v1', 'requested_arm': arm,
          'upstream': 'de87b436c35dac056564bdd5b2a113c9046e189b',
          'template_digest': manifest['content_hash'], 'node': node,
          'measurement': 'docker CLI invocation to blocking guest readiness; no command',
          'creates': [], 'computers': [], 'leftovers': []}

def save():
    pending = path.with_suffix('.pending')
    pending.write_text(json.dumps(report, indent=2) + '\n')
    pending.replace(path)

def cli(*args):
    result = subprocess.run(['docker', 'exec', node, '/space/bin/space-sbxmgr', *args],
                            capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise RuntimeError(f'Node CLI failed ({result.returncode}): {result.stderr[:2000]}')
    return result.stdout

def prepare_network():
    # Kind disables precreated network slots because sandbox pods bring CNI
    # namespaces. Explicit node creates need a small pool of their own.
    present = subprocess.run(['docker', 'exec', node, 'test', '-x', '/space/bin/grpcurl'])
    if present.returncode:
        container = 'oa-space-grpcurl-helper'
        subprocess.run(['docker', 'create', '--name', container,
                        'oa-space-runtime-publish:local'], check=True, capture_output=True)
        try:
            subprocess.run(['docker', 'cp', container + ':/usr/local/bin/grpcurl',
                            str(home / 'grpcurl')], check=True)
        finally:
            subprocess.run(['docker', 'rm', container], check=True, capture_output=True)
        subprocess.run(['docker', 'cp', str(home / 'grpcurl'),
                        node + ':/space/bin/grpcurl'], check=True)
        subprocess.run(['docker', 'cp', str(home / 'SPACE/proto'),
                        node + ':/space/spike-proto'], check=True)

    def rpc(method, body):
        return json.loads(subprocess.check_output([
            'docker', 'exec', node, '/space/bin/grpcurl', '-plaintext',
            '-import-path', '/space/spike-proto', '-proto', 'node/v1/node.proto',
            '-d', json.dumps(body), 'unix:///space/run/node-manager/node-manager.sock',
            'space.node.v1.NodeManager/' + method], text=True, timeout=30))

    report['network_pool_before'] = rpc('GetNetworkPoolStatus', {})
    report['network_pool_configured'] = rpc('SetNetworkPoolConfig', {
        'target_ready_slots': 2, 'retained_ready_slots': 2, 'min_ready_slots': 1})
    until = time.monotonic() + 120
    while True:
        status = rpc('GetNetworkPoolStatus', {})
        if status.get('networkSlotsReady', 0) >= 2:
            report['network_pool_ready'] = status
            save()
            return
        if time.monotonic() >= until:
            raise RuntimeError('Native network slots did not become ready.')
        time.sleep(1)

try:
    prepare_network()
    for index in range(20):
        save()
        start = time.monotonic()
        text = cli('create', '--runtime', 'fc', '--template', 'oa-coder-runtime',
                   '--content-hash', manifest['content_hash'], '--autostart', '--blocking',
                   '--boot-mode', 'cold' if arm == 'cold' else 'base-snapshot')
        elapsed = (time.monotonic() - start) * 1000
        label = re.search(r'^Sandbox:\s+(\S+)', text, re.M).group(1)
        report['computers'].append(label)
        save()
        mode = re.search(r'^Boot mode:\s+(\S+)', text, re.M).group(1)
        expected = 'cold' if arm == 'cold' else 'base-snapshot'
        report['creates'].append({'computer_label': label, 'ready_ms': elapsed,
                                  'actual_boot_mode': mode, 'create_stdout': text})
        save()
        if mode != expected:
            raise RuntimeError('Node create fell back to a different boot mode.')
        sandbox = re.search(r'/sandboxes/([0-9a-f-]{36})/fc.log', text).group(1)
        backend = json.loads(subprocess.check_output(
            [str(home / 'boot-mode.py'), sandbox], text=True, timeout=10))
        report['creates'][-1]['backend'] = backend
        if backend['boot_mode'] != arm:
            raise RuntimeError('Firecracker launch differs from the node response.')
        if index == 0:
            config = json.loads(subprocess.check_output([
                'sudo', '-n', 'curl', '--noproxy', '*', '-fsS', '--max-time', '5',
                '--unix-socket', f"/proc/{backend['host_pid']}/root/firecracker.sock",
                'http://localhost/machine-config'], text=True, timeout=10))
            report['machine_config'] = config
            if config['vcpu_count'] != 2 or config['mem_size_mib'] != 2048:
                raise RuntimeError('Both native arms must use 2 vCPU and 2048 MiB.')
        cli('delete', label, '--blocking')
        report['computers'].remove(label)
        save()
        print(f'{arm} {index + 1}/20: {elapsed:.1f} ms', flush=True)
    values = sorted(row['ready_ms'] for row in report['creates'])
    report.update(p50_ms=values[9], p90_ms=values[17], p50_under_2s=values[9] < 2000,
                  passed=True)
except Exception as error:
    report.update(passed=False, error=str(error))
finally:
    for label in report['computers'][:]:
        try:
            cli('delete', label, '--blocking')
            report['computers'].remove(label)
        except Exception:
            report['leftovers'].append(label)
    save()
if not report['passed'] or report['leftovers']:
    sys.exit(1)

#!/usr/bin/env python3
"""Check the retained zero-alpha failure and its exact inputs."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = '834aed6cb0b79c80fbd009c3caa51c78dba06628'


def read(path):
    return json.loads(path.read_text())


def identity(path):
    return {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'bytes': path.stat().st_size}


def main():
    value = read(HERE / 'verification.json')
    assert value['source'] == SOURCE
    assert value['status'] == 'native_color_parity_failed_before_timing'
    assert value['timing_capture_runs'] == value['paired_samples'] == 0
    assert value['performance_acceptance'] is value['visual_acceptance'] is None
    assert value['current_evidence_promoted'] is False
    for name, item in value['original_files'].items():
        assert identity(HERE / 'proof' / name) == {key: item[key] for key in ['sha256', 'bytes']}
    binding = read(HERE / 'proof/10936-final-resolve-build-binding.json')
    assert binding['source'] == SOURCE
    for profile, build in binding['builds'].items():
        assert build['source'] == SOURCE and build['features'] == ['capture'] and build['exit'] == 0
        receipt = read(HERE / 'proof' / Path(build['build_receipt']).name)
        assert receipt['resource'] == 'build' and receipt['exit'] == 0 and receipt['held_whole_run']
        assert receipt['acquired_at_ms'] <= build['start_unix'] * 1000 <= build['end_unix'] * 1000 <= receipt['released_at_ms']
    pbr = (HERE / 'proof/10936-final-resolve-native-build.log').read_text()
    assert '208 passed; 0 failed; 18 ignored' in pbr
    receipt = read(HERE / 'proof/10936-final-resolve-native-build-lease.json')
    assert receipt['resource'] == 'build' and receipt['exit'] == 0 and receipt['held_whole_run']
    records = read(HERE / value['native']['manifest'])
    assert len(records) == 1 and records[0]['source'] == SOURCE and records[0]['exit'] == 101
    receipt = read(HERE / value['native']['gpu_receipt'])
    assert receipt['resource'] == 'gpu' and receipt['exit'] == 101 and receipt['held_whole_run'] and not receipt['nested']
    assert receipt['acquired_at_ms'] <= records[0]['start_unix'] * 1000 <= records[0]['end_unix'] * 1000 <= receipt['released_at_ms']
    log = (HERE / value['native']['log']).read_text()
    for text in ['4x zero alpha pixel 0', 'left: [0.06262207, 0.06262207, 0.06262207, 1.0]',
                 'right: [0.0, 0.0, 0.0, 1.0]', '0 passed; 1 failed; 0 ignored']:
        assert text in log
    fixture = (HERE / value['fixture_excerpt']['file']).read_text()
    assert fixture.index('("volume"') < fixture.index('("zero emission"') < fixture.index('("zero alpha"')
    assert 'MRT must preserve the original scene color on a fresh history' in fixture
    protocol = read(HERE / 'proof/10936-final-resolve-replication-plan.json')
    assert protocol['runtime_source'] == SOURCE and protocol['total_trials'] == 3 and protocol['known_first_results'] is False
    artifacts = read(HERE / 'artifacts.json')
    expected = {str(p.relative_to(HERE)): identity(p) for p in sorted(HERE.rglob('*'))
                if p.is_file() and p not in [HERE / 'artifacts.json', HERE / 'SHA256SUMS']}
    assert artifacts == expected
    hashes = {}
    for line in (HERE / 'SHA256SUMS').read_text().splitlines():
        digest, name = line.split('  ', 1)
        assert name not in hashes
        hashes[name] = digest
    assert hashes == {str(p.relative_to(HERE)): identity(p)['sha256'] for p in sorted(HERE.rglob('*'))
                      if p.is_file() and p != HERE / 'SHA256SUMS'}
    print('PASS: failed 834 native color check, successful builds, exact receipts, no timing captures, complete hashes')


if __name__ == '__main__':
    main()

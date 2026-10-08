"""Verify the retained debris and relighting evidence without running a renderer."""

from pathlib import Path
import hashlib
import json
import math

HERE = Path(__file__).resolve().parent
SOURCE = '834aed6cb0b79c80fbd009c3caa51c78dba06628'
OLD_SOURCE = '46cf41188b8067f10a08a2eb2a49466446b1760d'
FIXTURE_SOURCE = 'df355b0cd1b45843b54aea40a6e6f3d665dfa237'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text())


def spread(values):
    values = sorted(values)
    return {key: values[math.floor((len(values) - 1) * fraction + .5)]
            for key, fraction in [('p50', .5), ('p99', .99), ('max', 1)]}


def inventory(folder):
    rows = (folder / 'SHA256SUMS').read_text().splitlines()
    recorded = {}
    for row in rows:
        assert len(row) >= 67 and row[64:66] == '  '
        digest, name = row[:64], row[66:]
        assert name not in recorded and len(bytes.fromhex(digest)) == 32
        assert not Path(name).is_absolute() and '..' not in Path(name).parts
        recorded[name] = digest
    actual = {str(path.relative_to(folder)): sha(path) for path in folder.rglob('*')
              if path.is_file() and path != folder / 'SHA256SUMS'}
    assert actual == recorded, folder
    evidence = read(folder / 'verification.json')
    assert set(evidence['artifacts']) == set(actual) - {'verification.json'}
    for name, item in evidence['artifacts'].items():
        assert item['sha256'] == actual[name]
        assert item['bytes'] == (folder / name).stat().st_size
    return evidence, len(actual)


def ledger(folder, run):
    report = folder / run['report']
    assert sha(report) == run['report_sha256']
    data = read(report)
    timing = data['submission_timing']
    rows = timing['frames']
    assert [row['index'] for row in rows] == list(range(960))
    assert timing['submitted_frames'] == timing['completed_frames'] == 960
    assert timing['maximum_pending_frames'] == timing['pending_frame_limit'] == 2
    assert timing['final_drain_charged_to_frame'] == 959
    assert all(row['started_ms'] <= row['submitted_ms'] <= row['observed_completed_ms'] for row in rows)
    for i, row in enumerate(rows):
        end = rows[i + 1]['started_ms'] if i + 1 < len(rows) else timing['elapsed_through_final_drain_ms']
        assert abs(row['continuous_iteration_ms'] - (end - row['started_ms'])) < .001
        assert abs(row['start_to_observed_completion_ms'] - (row['observed_completed_ms'] - row['started_ms'])) < .0001
        assert abs(row['submit_to_observed_completion_ms'] - (row['observed_completed_ms'] - row['submitted_ms'])) < .0001
    assert abs(sum(row['continuous_iteration_ms'] for row in rows) - timing['elapsed_through_final_drain_ms']) < .001
    for key in ['continuous_iteration_ms', 'start_to_observed_completion_ms', 'submit_to_observed_completion_ms']:
        for label, value in spread([row[key] for row in rows]).items():
            assert abs(value - timing[key][label]) < .0001
    for phase, (start, end) in {'before': (0, 330), 'swarm': (330, 690), 'after': (690, 960)}.items():
        assert data['phases'][phase]['frames'] == end - start
        for label, value in spread([row['frame_ms'] for row in rows[start:end]]).items():
            assert abs(value - data['phases'][phase]['frame_ms'][label]) < .0001
    saved = folder / run['frame_ledger']
    assert sha(saved) == run['frame_ledger_sha256']
    assert [json.loads(line) for line in saved.read_text().splitlines()] == rows
    assert sorted(data['pixel_readback']['primary_readback_frame_indices'] + data['pixel_readback']['completion_only_frame_indices']) == list(range(960))
    assert not data['gpu_timestamps_requested'] and not data['gpu_timestamps_available']
    assert data['width'] == 1920 and data['height'] == 1080 and data['temporal_aa_enabled']
    assert data['phases']['swarm']['chunks_max'] == 509
    assert data['phases']['swarm']['posed_vertices_max'] == 0
    assert data['phases'] == run['phases']
    for phase, key in [('swarm', 'swarm_p99_ms'), ('after', 'after_p99_ms')]:
        assert data['phases'][phase]['frame_ms']['p99'] == run['acceptance'][key] < 16.7
    for image in run['images'].values():
        assert (folder / image).is_file()
    return len(rows)


def main():
    debris, debris_files = inventory(HERE)
    relight_folder = HERE.parent / 'destruction-relighting'
    relight, relight_files = inventory(relight_folder)
    assert debris['tested_source_commit'] == SOURCE
    manifest = read(HERE / 'current-command-manifest.json')
    count = 0
    for profile, short in [('development', 'dev'), ('release', 'release')]:
        run = debris['current_runs'][profile]
        assert run['tested_source_commit'] == SOURCE
        entry = next(row for row in manifest if row['name'] == run['capture_job_name'])
        quiet = read(HERE / run['quiet_lease_receipt'])
        gpu = read(HERE / run['gpu_lease_receipt'])
        assert entry['source'] == SOURCE and entry['exit'] == 0
        assert entry['binary_sha256'] == run['tested_binary_sha256']
        assert entry['command'] == run['capture_command']
        assert entry['environment'] == run['environment']
        for receipt in [quiet, gpu]:
            assert receipt['exit'] == 0 and receipt['held_whole_run']
            assert receipt['acquired_at_ms'] <= entry['start_unix'] * 1000 <= entry['end_unix'] * 1000 <= receipt['released_at_ms']
        count += ledger(HERE, run)
        old = debris['historical_production_runs'][profile]
        assert old['tested_source_commit'] == OLD_SOURCE
        count += ledger(HERE, old)
    for profile in ['development', 'release']:
        old = debris['historical_reactive_production_runs'][profile]
        assert old['tested_source_commit'] == '36df9ceb4bbb7fb0c7be30bd1c3538e0be78ab3e'
        count += ledger(HERE, old)
    quiet = read(HERE / 'legacy/reactive-production-36df9ceb4bbb/current-quiet-lease.json')
    gpu = read(HERE / 'legacy/reactive-production-36df9ceb4bbb/current-gpu-lease.json')
    native = debris['reactive_native_evidence']
    assert native['fixture_source'] == FIXTURE_SOURCE and native['tests_passed'] == 4
    for name, item in native['referenced_artifacts'].items():
        path = HERE / name
        assert sha(path) == item['sha256'] and path.stat().st_size == item['bytes']
    tests = read(HERE / native['manifest'])
    assert len(tests) == 4 and all(row['exit'] == 0 and row['source'] == FIXTURE_SOURCE for row in tests)
    for row in tests:
        for receipt in [quiet, gpu]:
            assert receipt['acquired_at_ms'] <= row['start_unix'] * 1000 <= row['end_unix'] * 1000 <= receipt['released_at_ms']
    assert relight['tested_source_commit'] == OLD_SOURCE
    assert relight['supplementary_reactive_evidence']['capture_source_unchanged'] == OLD_SOURCE
    historical_rows = 0
    assert len(debris['intermediate_runs']) == 7
    for run in debris['intermediate_runs'].values():
        assert run['status'] == 'not_met'
        path = HERE / run['report']
        assert sha(path) == run['report_sha256']
        data = read(path)
        rows = data['submission_timing']['frames']
        assert [row['index'] for row in rows] == list(range(960))
        for phase, (start, end) in {'before': (0, 330), 'swarm': (330, 690), 'after': (690, 960)}.items():
            for label, value in spread([row['frame_ms'] for row in rows[start:end]]).items():
                assert abs(value - data['phases'][phase]['frame_ms'][label]) < .001
        assert data['phases']['swarm']['frame_ms']['p99'] > 16.7
        historical_rows += len(rows)
    controls = {key: read(relight_folder / ('current-' + key + '-capture.json')) for key in ['off', 'on']}
    for key, data in controls.items():
        rows = data['submission_timing']['frames']
        assert [row['index'] for row in rows] == list(range(960))
        assert [json.loads(line) for line in (relight_folder / ('current-' + key + '-frame-ledger.jsonl')).read_text().splitlines()] == rows
        assert not data['temporal_aa_enabled']
        assert data['submission_timing']['pending_frame_limit'] == 1
    for phase in ['before', 'swarm', 'after']:
        for field in relight['validation']['matched_physical_counter_fields']:
            assert controls['off']['phases'][phase][field] == controls['on']['phases'][phase][field]
        for field in relight['validation']['physical_phase_end_fields']:
            assert controls['off']['phases'][phase]['debris_end'][field] == controls['on']['phases'][phase]['debris_end'][field]
    end = controls['on']['phases']['after']['debris_end']
    assert (end['merged_chunks'], end['static_parts'], end['static_vertices'], end['awake_bodies'], end['sleeping_bodies']) == (391, 674, 53550, 0, 118)
    assert end['static_groups'] == 148
    assert controls['off']['phases']['after']['debris_end']['static_groups'] == 626
    for run in debris['current_runs'].values():
        assert read(HERE / run['build_lease_receipt'])['exit'] == 0
    assert (HERE / 'current-scoped-fmt.log').stat().st_size == 0
    print(json.dumps({'status': 'artifact_consistency_pass', 'debris_files': debris_files,
                      'relighting_files': relight_files, 'production_ledger_rows': count,
                      'historical_failure_ledger_rows': historical_rows, 'relighting_ledger_rows': 1920,
                      'native_tests': 4, 'gpu_duration_claim': False}))


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Verify all retained rows, protocol boundaries, scoped results, and hashes."""
import hashlib
import json
import math
from pathlib import Path
import statistics

HERE = Path(__file__).resolve().parent
SOURCE = 'fb9a5fd280db673926cfd0d649f924da3091a24b'
STEM = '10936-final-copy'


def read(path):
    return json.loads(path.read_text())


def identity(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return {'sha256': digest.hexdigest(), 'bytes': path.stat().st_size}


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def main():
    verification = read(HERE / 'verification.json')
    assert verification['source'] == SOURCE
    if verification['status'] == 'review_pending':
        assert verification['acceptance'] == {'visual': None, 'timing': None, 'promotion_authorized': False}
    else:
        assert verification['status'] == 'high_cost_and_scoped_visual_verified'
        assert verification['acceptance']['visual'] is True and verification['acceptance']['timing'] is True and verification['acceptance']['promotion_authorized'] is True
        root = read(HERE / 'proof/10936-final-copy-root-visual-review.json')
        assert root['source'] == SOURCE and root['status'] == 'accepted_reviewed_regions'
        assert len(root['reviewed_images']) == 17 and len(root['prepared_not_reviewed']) == 3
        retained = read(HERE / 'root-review-images.json')
        for item in retained['images'].values():
            path = HERE / item['file'] if item.get('file') else Path(item['path'])
            assert identity(path) == {key: item[key] for key in ['sha256', 'bytes']}
    review = read(HERE / 'review.json')
    protocol_path = HERE / 'proof' / (STEM + '-replication-plan.json')
    protocol = read(protocol_path)
    assert protocol['runtime_source'] == SOURCE and protocol['total_trials'] == 3
    assert protocol['additional_trials'] == 2 and protocol['known_first_results'] is False
    pause = read(HERE / 'proof' / (STEM + '-background-build-pause.json'))
    assert pause['source'] == SOURCE and pause['batch_exit'] == 0
    reports, jobs = {}, []
    primary_rows = paired_rows = 0
    for label, item in review['runs'].items():
        report_path = HERE / item['report']
        report = read(report_path)
        assert identity(report_path)['sha256'] == item['report_sha256']
        manifest = read(HERE / item['manifest'])
        assert manifest['source'] == SOURCE and manifest['exit'] == 0 and manifest['features'] == ['capture']
        assert manifest['warning_diagnostics'] is None
        assert manifest['replication_plan']['sha256'] == identity(protocol_path)['sha256']
        assert protocol['registered_unix'] < manifest['start_unix']
        assert pause['pause_unix'] <= manifest['start_unix'] <= manifest['end_unix'] <= pause['resume_unix']
        assert '--temporal-diagnostics' not in manifest['command']
        for resource in ['gpu', 'quiet']:
            receipt = read(HERE / item[resource + '_receipt'])
            assert receipt['resource'] == resource and receipt['exit'] == 0
            assert receipt['held_whole_run'] and not receipt['nested']
            assert receipt['acquired_at_ms'] <= manifest['start_unix'] * 1000 <= manifest['end_unix'] * 1000 <= receipt['released_at_ms']
            assert pause['pause_unix'] <= receipt['acquired_at_ms'] / 1000 <= receipt['released_at_ms'] / 1000 <= pause['resume_unix']
        assert report['width'] == 1920 and report['height'] == 1080 and report['fps'] == 60
        assert report['temporal_texture_diagnostics']['enabled'] is False
        raw = report['submission_timing']['frames']
        count = 480 if label in ['pan-paired', 'orbit-paired'] else 960
        assert len(raw) == report['submission_timing']['submitted_frames'] == report['submission_timing']['completed_frames'] == count
        assert [row['index'] for row in raw] == list(range(count))
        assert rows(HERE / item['frame_ledger']) == raw
        primary_rows += count
        comparison = report.get('temporal_comparison')
        if comparison:
            pairs = [row for phase in ['before', 'swarm', 'after'] for row in comparison['phases'][phase]['frame_results']]
            assert rows(HERE / item['pair_ledger']) == pairs
            paired_rows += len(pairs)
            assert {phase: {key: value for key, value in stats.items() if key != 'frame_results'}
                    for phase, stats in comparison['phases'].items()} == item['original_phase_comparison']
            assert all(stats['mean_gpu_increment_ms'] is None for stats in comparison['phases'].values())
            expected_valid = 1 if label == 'orbit-paired' else 0
            assert sum(stats['valid_gpu_frames'] for stats in comparison['phases'].values()) == expected_valid
        if label in ['high-dev', 'high-release']:
            rebuilt = report['rebuild_capture']
            assert rebuilt['pristine_frame'] == 0 and rebuilt['restoration_frame'] == 960 and rebuilt['after_simulation_frame'] == 959
            assert rebuilt['pristine_view'] == rebuilt['restored_view']
            assert 960 in report['pixel_readback']['additional_artifact_readback_frame_indices']
        reports[label] = report
        jobs.append(manifest)
    assert primary_rows == 6720 and paired_rows == 4752 and len(reports) == 8
    assert all(a['end_unix'] <= b['start_unix'] for a, b in zip(sorted(jobs, key=lambda x: x['start_unix']), sorted(jobs, key=lambda x: x['start_unix'])[1:]))
    pooled = read(HERE / 'proof' / (STEM + '-high-pooled.json'))
    assert pooled['source'] == SOURCE and pooled['registered_protocol_sha256'] == identity(protocol_path)['sha256']
    labels = ['high-paired', 'high-replicate-2', 'high-replicate-3']
    assert [item['report_sha256'] for item in pooled['runs']] == [review['runs'][label]['report_sha256'] for label in labels]
    assert len({read(HERE / review['runs'][label]['manifest'])['binary_sha256'] for label in labels}) == 1
    high_pairs = 0
    for phase in ['before', 'swarm', 'after']:
        values = [[row['wall_render_completion_increment_ms'] for row in reports[label]['temporal_comparison']['phases'][phase]['frame_results']] for label in labels]
        assert len({len(value) for value in values}) == 1
        means = [statistics.mean(value) for value in values]
        mean = statistics.mean(means)
        half = 4.303 * statistics.stdev(means) / math.sqrt(3)
        expected = pooled['phases'][phase]
        assert expected['run_means_ms'] == means
        assert expected['all_retained_samples'] == sum(map(len, values))
        high_pairs += expected['all_retained_samples']
        for key, actual in [('mean', mean), ('lower', mean - half), ('upper', mean + half)]:
            assert abs(expected['primary_run_mean_95pct_ci_ms'][key] - actual) < 1e-12
        assert expected['primary_upper_under_1ms'] == (mean + half < 1)
        assert expected['per_run_bounds_ms'] == [reports[label]['temporal_comparison']['phases'][phase]['wall_render_completion_mean_95pct_ci_ms'] for label in labels]
        assert expected['no_outliers_removed'] and expected['valid_gpu_pairs'] == 0
    assert high_pairs == 2856 and pooled['strict_high_1ms_gate_pass']
    assert reports['high-paired']['temporal_comparison']['phases']['before']['wall_mean_upper_95pct_under_1ms'] is False
    for phase in ['swarm', 'after']:
        assert reports['medium-paired']['temporal_comparison']['phases'][phase]['wall_mean_upper_95pct_under_1ms'] is False
    for label in ['pan-paired', 'orbit-paired']:
        stats = reports[label]['temporal_comparison']['phases']
        assert stats['before']['wall_mean_upper_95pct_under_1ms'] is True
        assert not stats['swarm']['frame_results'] and not stats['after']['frame_results']
    native = read(HERE / 'proof' / (STEM + '-native-manifest.json'))
    assert len(native) == 11 and all(item['source'] == SOURCE and item['exit'] == 0 for item in native)
    assert '210 passed; 0 failed; 18 ignored' in (HERE / 'proof' / (STEM + '-native-build.log')).read_text()
    original_pbr = read(HERE / 'proof/10936-final-copy-pbr-command.json')
    assert original_pbr['source'] == SOURCE and original_pbr['exit'] == 0 and original_pbr['command'][-4:] == ['cargo', 'test', '-p', 'verse-pbr']
    assert original_pbr['result'] == {'passed': 210, 'ignored': 18}
    consumers = read(HERE / 'proof/10936-final-MRT-consumer-tests.json')
    assert all(item['source'] == 'a3acb16a8482e77cc151799e4e16ce2aa8c1b6c0' and item['exit'] == 0 for item in consumers)
    for item in read(HERE / 'source-images.json')['images'].values():
        image = HERE / item['file'] if item['storage'] == 'git' else Path(item['path'])
        assert identity(image) == {key: item[key] for key in ['sha256', 'bytes']}
        header = image.open('rb').read(24)
        assert header[:8] == b'\x89PNG\r\n\x1a\n' and int.from_bytes(header[16:20], 'big') == 1920 and int.from_bytes(header[20:24], 'big') == 1080
    artifacts = {str(path.relative_to(HERE)): identity(path) for path in sorted(HERE.rglob('*'))
                 if path.is_file() and path not in [HERE / 'artifacts.json', HERE / 'SHA256SUMS']}
    assert read(HERE / 'artifacts.json') == artifacts
    sums = {}
    for line in (HERE / 'SHA256SUMS').read_text().splitlines():
        digest, name = line.split('  ', 1)
        assert name not in sums
        sums[name] = digest
    assert sums == {str(path.relative_to(HERE)): identity(path)['sha256'] for path in sorted(HERE.rglob('*'))
                    if path.is_file() and path != HERE / 'SHA256SUMS'}
    print('PASS: 8 exact reports, 6720 primary rows, 4752 pairs, 2856 High pairs; preregistered primary bounds, raw failures, image inventory, leases, and hashes; scoped root visual verdict retained')


if __name__ == '__main__':
    main()

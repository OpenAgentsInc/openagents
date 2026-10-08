#!/usr/bin/env python3
"""Verify the complete fixed-protocol cost archive and its unchanged raw rows."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics


def read(path):
    return json.loads(path.read_text())


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def identity(path):
    return {'sha256': digest(path), 'bytes': path.stat().st_size}


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def check(root, raw):
    verification = read(root / 'verification.json')
    assert verification['status'] == 'performance_unresolved'
    assert verification['acceptance']['strict_high_1ms_gate_pass'] is False
    assert not verification['acceptance']['promotion_authorized'] and not verification['acceptance']['supersedes_top_level_acceptance']
    commands = read(root / 'commands.json')
    source = verification['source']
    assert len(commands['jobs']) == 6 and commands['source'] == source
    original_images = read(root / 'staged-source-images.json')
    current_images = read(root / 'source-images.json')
    for name, item in original_images['images'].items():
        assert current_images['images'][name] == item
    for name, expected in read(root / 'staged-artifacts.json').items():
        saved_name = {'verification.json': 'staged-verification.json', 'source-images.json': 'staged-source-images.json'}.get(name, name)
        assert identity(root / saved_name) == expected, 'Original six-case staged artifact changed: ' + name
    primary, pairs, reports = 0, 0, {}
    for label, record in verification['reports'].items():
        path = root / record['file']
        assert identity(path) == {key: record[key] for key in ['sha256', 'bytes']}
        report = read(path)
        job = verification['jobs'][label]
        assert job['source'] == source and job['exit'] == 0 and job['features'] == ['capture']
        assert report['temporal_texture_diagnostics']['enabled'] is False
        timing = report['submission_timing']
        frame_rows = timing['frames']
        frames = 480 if label in ['pan-paired', 'orbit-paired'] else 960
        assert [row['index'] for row in frame_rows] == list(range(frames))
        assert timing['submitted_frames'] == timing['completed_frames'] == frames
        assert rows(root / 'reports' / (label + '-frame-ledger.jsonl')) == frame_rows
        comparison = report.get('temporal_comparison')
        expected_pairs = [row for phase in comparison['phases'].values() for row in phase['frame_results']] if comparison else []
        assert rows(root / 'reports' / (label + '-temporal-pairs.jsonl')) == expected_pairs
        if comparison:
            assert [row['frame'] for row in expected_pairs] == list(range(comparison['warmup_frames'], frames))
            assert timing['temporal_baseline_submitted_frames'] == timing['temporal_baseline_completed_frames'] == frames
        primary += len(frame_rows)
        pairs += len(expected_pairs)
        reports[label] = report
        if raw:
            assert identity(Path(job['command'][1]) / 'capture.json') == identity(path)
        if label in commands['bindings']:
            binding = commands['bindings'][label]
            receipt_names = {resource: root / 'proof' / (label + '-' + resource + '_receipt.json') for resource in ['gpu', 'quiet']}
        else:
            receipt_names = {resource: root / 'proof' / (job['name'] + '-' + resource + '.json') for resource in ['gpu', 'quiet']}
        for resource, path in receipt_names.items():
            receipt = read(path)
            assert receipt['resource'] == resource and receipt['exit'] == 0 and receipt['held_whole_run'] and not receipt['nested']
            assert receipt['acquired_at_ms'] <= job['start_unix'] * 1000 <= job['end_unix'] * 1000 <= receipt['released_at_ms']
    assert primary == verification['primary_rows'] == 6720
    protocol = read(root / verification['registered_protocol'])
    pooled = read(root / verification['pooled_result'])
    assert protocol['runtime_source'] == pooled['source'] == source
    assert protocol['total_trials'] == 3 and protocol['additional_trials'] == 2
    assert pooled['registered_protocol_sha256'] == digest(root / verification['registered_protocol'])
    high_reports = [reports[label] for label in ['high-paired', 'high-replicate-2', 'high-replicate-3']]
    high_samples = 0
    for phase in ['before', 'swarm', 'after']:
        values = [[row['wall_render_completion_increment_ms'] for row in report['temporal_comparison']['phases'][phase]['frame_results']] for report in high_reports]
        means = [statistics.mean(value) for value in values]
        result = pooled['phases'][phase]
        mean, half = statistics.mean(means), 4.303 * statistics.stdev(means) / math.sqrt(3)
        assert result['run_means_ms'] == means
        assert math.isclose(result['primary_run_mean_95pct_ci_ms']['mean'], mean, abs_tol=1e-12)
        assert math.isclose(result['primary_run_mean_95pct_ci_ms']['upper'], mean + half, abs_tol=1e-12)
        assert result['primary_upper_under_1ms'] == (mean + half < 1)
        assert result['per_run_bounds_ms'] == [report['temporal_comparison']['phases'][phase]['wall_render_completion_mean_95pct_ci_ms'] for report in high_reports]
        assert result['all_retained_samples'] == sum(map(len, values))
        high_samples += result['all_retained_samples']
    assert high_samples == verification['warmed_high_pairs'] == 2856
    assert pooled['strict_high_1ms_gate_pass'] is False and pooled['phases']['before']['primary_run_mean_95pct_ci_ms']['upper'] > 1
    for name, item in current_images['images'].items():
        if raw:
            assert identity(Path(item['path'])) == {key: item[key] for key in ['sha256', 'bytes']}, name
        if item['storage'] == 'git':
            assert identity(root / item['file']) == {key: item[key] for key in ['sha256', 'bytes']}, name
    actual = {str(path.relative_to(root)): identity(path) for path in sorted(root.rglob('*')) if path.is_file() and path.name not in ['artifacts.json', 'SHA256SUMS']}
    assert read(root / 'artifacts.json') == actual
    sums = {line.split('  ', 1)[1]: line.split('  ', 1)[0] for line in (root / 'SHA256SUMS').read_text().splitlines()}
    assert sums == {str(path.relative_to(root)): digest(path) for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'SHA256SUMS'}
    print(f'ARCHIVE PASS: {source}; {primary} primary rows, {pairs} total warmed pairs, {high_samples} fixed-protocol High pairs, complete image identities and hashes. Strict cost remains unresolved.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--raw', action='store_true')
    args = parser.parse_args()
    check(args.root, args.raw)

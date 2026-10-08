#!/usr/bin/env python3
"""Check the retained temporal AA reports and artifact identities."""
from pathlib import Path
import hashlib
import json
import math

ROOT = Path(__file__).resolve().parent

def read(path):
    return json.loads((ROOT / path).read_text())

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def check():
    manifest = read('artifacts.json')
    for name, identity in manifest.items():
        path = ROOT / name
        assert path.stat().st_size == identity['bytes'], f'Size changed: {name}'
        assert digest(path) == identity['sha256'], f'Hash changed: {name}'
    v = read('verification.json')
    commands = read('commands.json')
    records = {r['name']: r for r in commands['captures']}
    total = 0
    for label, summary in v['results'].items():
        report = read(summary['report'])
        record = records[summary['capture_command']]
        assert record['source'] == v['tested_source_commit'] == summary['source']
        assert record['binary_sha256'] == v['tested_binary_sha256'] == summary['binary_sha256']
        assert record['profile'] == summary['profile'] == 'release'
        assert record['exit'] == 0
        assert record['quality'] == summary['quality'] == report['effective_quality']
        assert report['width'] == 1920 and report['height'] == 1080 and report['fps'] == 60
        assert report['temporal_aa_available'] and report['temporal_aa_enabled']
        frames = summary['simulation_frames']
        submission = report['submission_timing']
        assert submission['submitted_frames'] == submission['completed_frames'] == frames
        assert submission['temporal_baseline_submitted_frames'] == submission['temporal_baseline_completed_frames'] == frames
        assert submission['policy'] == 'serial_diagnostics'
        pixels = report['pixel_readback']
        assert pixels['policy'] == 'all_frames_temporal_comparison'
        assert pixels['primary_readback_count'] == pixels['temporal_baseline_readback_count'] == frames
        assert pixels['primary_readback_frame_indices'] == list(range(frames))
        assert pixels['temporal_baseline_readback_frame_indices'] == list(range(frames))
        temporal = report['temporal_comparison']
        assert temporal['warmup_frames'] == 8
        measured = []
        for phase, values in summary['phases'].items():
            raw = temporal['phases'][phase]
            assert values == {key: raw.get(key) for key in values}, f'Summary changed: {label}/{phase}'
            results = raw['frame_results']
            assert len(results) == raw['frames'] == raw['invalid_gpu_frames']
            assert raw['valid_gpu_frames'] == 0
            assert raw['gpu_increment_ms'] is None and raw['mean_gpu_increment_ms'] is None
            assert raw['off_first_frames'] + raw['on_first_frames'] == len(results)
            for row in results:
                assert all(row[key] is None for key in ['off_gpu_ms', 'on_gpu_ms', 'gpu_increment_ms'])
            mean = sum(row['wall_render_completion_increment_ms'] for row in results) / len(results)
            assert math.isclose(mean, raw['mean_wall_render_completion_increment_ms'], abs_tol=1e-6)
            interval = raw['wall_render_completion_mean_95pct_ci_ms']
            assert interval['samples'] == len(results)
            assert raw['wall_mean_upper_95pct_under_1ms'] == (interval['upper'] < 1)
            if label != 'medium-short-8s':
                assert interval['upper'] < 1
            measured += [row['frame'] for row in results]
        assert measured == list(range(8, frames)), f'Missing/repeated pair: {label}'
        total += len(measured)
    assert v['results']['medium-short-8s']['phases']['swarm']['wall_mean_upper_95pct_under_1ms'] is False
    legacy = read('legacy/caster-dae/verification.json')
    assert all(phase['wall_mean_upper_95pct_under_1ms'] is False for phase in legacy['results']['high'].values())
    assert legacy['source'] != v['tested_source_commit'] and legacy['unpinned_override']
    for test in v['native_tests']:
        log = (ROOT / test['proof']).read_text()
        assert f"test {test['name']} ... ok" in log
        assert '1 passed; 0 failed' in log
    log = (ROOT / v['checks']['proof']).read_text()
    for count in [190, 183, 290, 9]:
        assert f'test result: ok. {count} passed; 0 failed' in log
    assert 'Finished `release` profile' in log
    assert all(v['acceptance'][key] for key in ['high_wall_mean_upper_95pct_under_1ms', 'medium_full_wall_mean_upper_95pct_under_1ms', 'pan_and_orbit_wall_mean_upper_95pct_under_1ms'])
    print(f'PASS: {len(manifest)} artifact hashes, {len(v["results"])} reports, {total} paired samples, current wall mean gates, null GPU costs, retained failures, and native/check proofs')

if __name__ == '__main__':
    check()

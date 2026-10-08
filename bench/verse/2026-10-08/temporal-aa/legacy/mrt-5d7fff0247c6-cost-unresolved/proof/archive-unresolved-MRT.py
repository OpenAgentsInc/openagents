#!/usr/bin/env python3
"""Archive all six cases and the fixed three-run High protocol without promotion."""
import hashlib
import json
import math
from pathlib import Path
import shutil
import statistics

SCRATCH = Path(__file__).resolve().parent
ROOT = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents/bench/verse/2026-10-08/temporal-aa')
SOURCE = '5d7fff0247c6fef70447fe96103838a39e38b4f8'
STAGE = ROOT / 'staged' / SOURCE[:12]
ARCHIVE = ROOT / 'legacy' / ('mrt-' + SOURCE[:12] + '-cost-unresolved')


def read(path):
    return json.loads(path.read_text())


def identity(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return {'sha256': h.hexdigest(), 'bytes': path.stat().st_size}


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        assert identity(source) == identity(destination), f'Refuse to overwrite retained data: {destination}'
    else:
        shutil.copy2(source, destination)


protocol_path = SCRATCH / '10936-final-MRT-replication-plan.json'
pooled_path = SCRATCH / '10936-final-MRT-high-pooled.json'
protocol, pooled = read(protocol_path), read(pooled_path)
assert protocol['runtime_source'] == pooled['source'] == SOURCE
assert protocol['additional_trials'] == 2 and protocol['total_trials'] == 3
assert pooled['strict_high_1ms_gate_pass'] is False
assert identity(protocol_path)['sha256'] == pooled['registered_protocol_sha256']
if STAGE.exists():
    assert not ARCHIVE.exists()
    ARCHIVE.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(STAGE), str(ARCHIVE))
else:
    assert ARCHIVE.exists()
commands = read(ARCHIVE / 'commands.json')
assert commands['source'] == SOURCE and len(commands['jobs']) == 6
old_verification = read(ARCHIVE / 'verification.json')
if not (ARCHIVE / 'staged-verification.json').exists():
    copy(ARCHIVE / 'verification.json', ARCHIVE / 'staged-verification.json')
    copy(ARCHIVE / 'artifacts.json', ARCHIVE / 'staged-artifacts.json')
    copy(ARCHIVE / 'source-images.json', ARCHIVE / 'staged-source-images.json')
for filename in ['10936-final-MRT-curation-plan.json', '10936-final-MRT-replication-plan.json', '10936-final-MRT-high-pooled.json', '10936-final-MRT-pool.py']:
    copy(SCRATCH / filename, ARCHIVE / 'proof' / filename)
copy(Path(__file__).resolve(), ARCHIVE / 'proof' / 'archive-unresolved-MRT.py')
reports = {label: read(ARCHIVE / 'reports' / (label + '.json')) for label in commands['jobs']}
jobs = dict(commands['jobs'])
images = read(ARCHIVE / 'source-images.json')
for number, stem in enumerate(protocol['additional_prefixes'], 2):
    label = f'high-replicate-{number}'
    job = read(SCRATCH / (stem + '-manifest.json'))
    report = read(SCRATCH / stem / 'capture.json')
    assert job['source'] == SOURCE and job['exit'] == 0 and job['features'] == ['capture']
    assert job['start_unix'] > protocol['registered_unix'] and job['binary_sha256'] == jobs['high-paired']['binary_sha256']
    assert job['command'][2:] == jobs['high-paired']['command'][2:]
    assert report['submission_timing']['submitted_frames'] == report['submission_timing']['completed_frames'] == 960
    assert not report['temporal_texture_diagnostics']['enabled']
    copy(SCRATCH / stem / 'capture.json', ARCHIVE / 'reports' / (label + '.json'))
    for suffix in ['-manifest.json', '-gpu.json', '-quiet.json', '-check.json', '.log']:
        copy(SCRATCH / (stem + suffix), ARCHIVE / 'proof' / (stem + suffix))
    copy(SCRATCH / f'10936-final-MRT-replicate-{number}-runner.py', ARCHIVE / 'proof' / f'10936-final-MRT-replicate-{number}-runner.py')
    for resource in ['gpu', 'quiet']:
        receipt = read(SCRATCH / (stem + '-' + resource + '.json'))
        assert receipt['resource'] == resource and receipt['exit'] == 0 and receipt['held_whole_run'] and not receipt['nested']
        assert receipt['acquired_at_ms'] <= job['start_unix'] * 1000 <= job['end_unix'] * 1000 <= receipt['released_at_ms']
    reports[label], jobs[label] = report, job
    for original in sorted((SCRATCH / stem).rglob('*.png')):
        relative = str(original.relative_to(SCRATCH / stem))
        item = {'path': str(original), **identity(original), 'storage': 'durable_scratch'}
        if '/' not in relative:
            destination = ARCHIVE / 'selected' / (label + '-' + original.name)
            copy(original, destination)
            item.update(storage='git', file=str(destination.relative_to(ARCHIVE)))
        images['images'][label + '/' + relative] = item
write(ARCHIVE / 'source-images.json', images)
copy(SCRATCH / '10936-final-MRT-replicate-cases.py', ARCHIVE / 'proof' / '10936-final-MRT-replicate-cases.py')
for label, report in reports.items():
    (ARCHIVE / 'reports' / (label + '-frame-ledger.jsonl')).write_text(''.join(json.dumps(row, separators=(',', ':')) + '\n' for row in report['submission_timing']['frames']))
    comparison = report.get('temporal_comparison')
    pairs = [row for phase in comparison['phases'].values() for row in phase['frame_results']] if comparison else []
    (ARCHIVE / 'reports' / (label + '-temporal-pairs.jsonl')).write_text(''.join(json.dumps(row, separators=(',', ':')) + '\n' for row in pairs))
for phase in ['before', 'swarm', 'after']:
    high_reports = [reports[label] for label in ['high-paired', 'high-replicate-2', 'high-replicate-3']]
    values = [[row['wall_render_completion_increment_ms'] for row in report['temporal_comparison']['phases'][phase]['frame_results']] for report in high_reports]
    means = [statistics.mean(value) for value in values]
    expected_mean = statistics.mean(means)
    half = 4.303 * statistics.stdev(means) / math.sqrt(3)
    result = pooled['phases'][phase]
    assert result['run_means_ms'] == means
    assert abs(result['primary_run_mean_95pct_ci_ms']['mean'] - expected_mean) < 1e-12
    assert abs(result['primary_run_mean_95pct_ci_ms']['upper'] - (expected_mean + half)) < 1e-12
    assert result['primary_upper_under_1ms'] == (expected_mean + half < 1)
    assert result['all_retained_samples'] == sum(map(len, values))
    assert result['no_outliers_removed'] and result['valid_gpu_pairs'] == 0
verification = {'schema': 'openagents.verse.temporal-cost-attempt.v1', 'status': 'performance_unresolved', 'source': SOURCE, 'acceptance': {'strict_high_1ms_gate_pass': False, 'promotion_authorized': False, 'supersedes_top_level_acceptance': False}, 'original_six_cases': list(commands['jobs']), 'additional_cases': ['high-replicate-2', 'high-replicate-3'], 'registered_protocol': 'proof/10936-final-MRT-replication-plan.json', 'pooled_result': 'proof/10936-final-MRT-high-pooled.json', 'primary_interval': 'Student t df2 across the three independent per-phase run means, with registered t=4.303; every upper bound must be below 1 ms.', 'primary_upper_bounds_ms': {phase: result['primary_run_mean_95pct_ci_ms']['upper'] for phase, result in pooled['phases'].items()}, 'supplementary_interval_limitation': 'The balanced-block interval remains supplementary. Its before upper0.8520136987 ms does not replace the registered primary upper1.11584236599 ms.', 'primary_rows': sum(report['submission_timing']['completed_frames'] for report in reports.values()), 'warmed_high_pairs': sum(pooled['phases'][phase]['all_retained_samples'] for phase in pooled['phases']), 'root_visual_review': {'high_frame': 471, 'medium_frame': 472, 'detached_head_ghost_seen': False, 'review': 'Reviewed native on/off pairs show matching current head and debris silhouettes in the inspected views.', 'static_frame': 120, 'static_tradeoff': 'Expected TAA softening remains visible.', 'original_static_crop': [750, 420, 1150, 780], 'crop_limitation': 'The original static crop begins at y420 and excludes the roof ridge near y392. Its method remains unchanged; a future attempt uses a wider crop.'}, 'reports': {label: {'file': 'reports/' + label + '.json', **identity(ARCHIVE / 'reports' / (label + '.json'))} for label in reports}, 'jobs': jobs, 'source_images': 'source-images.json', 'limits': ['All primary rows, warmed paired rows, outliers, and original source PNG identities are retained; no best run is selected.', 'The two ordinary High runs pass the 16.7 ms debris p99 target, but this complete temporal attempt remains unpromoted because the registered strict cost gate is unmet.', 'The first Medium bounds remain unresolved and are not a High acceptance claim.', 'Completed offscreen wall intervals include encode, completion wait, mapping, and pixel readback; they do not measure GPU duration or presentation.', 'Raw reports, original crop methods, first-six stage metadata, and all three per-run confidence bounds remain unchanged.']}
write(ARCHIVE / 'verification.json', verification)
(ARCHIVE / 'README.md').write_text('This runtime `' + SOURCE + '` attempt remains `performance_unresolved` and does not replace top-level acceptance. All six original cases and the two precommitted extra High runs remain retained. The original stage is moved here without duplicating its figures. Every raw report, primary row, warmed pair, outlier, receipt, exact command, and original PNG identity remains available.\n\nThe fixed three-run protocol uses the Student t df2 interval across the three independent phase means as its primary result. The before mean is 0.707627 ms and its upper bound is 1.115842 ms, so the strict 1 ms gate is unmet. Swarm and aftermath upper bounds are 0.758646 ms and 0.673847 ms. The supplementary balanced-block before bound of 0.852014 ms does not replace the registered primary result, and no run or outlier is discarded. [Protocol](proof/10936-final-MRT-replication-plan.json), [pooled result](proof/10936-final-MRT-high-pooled.json), and [pool calculation](proof/10936-final-MRT-pool.py) preserve the method and all per-run bounds.\n\nThe root reviews High frame 471 and Medium frame 472 at native resolution and sees no detached head contour in those views. Static pan/orbit frame 120 shows expected TAA softening. The original static crop `[750, 420, 1150, 780]` excludes the roof ridge near y392; these archived crop methods remain unchanged. A future attempt uses `[700, 360, 1200, 740]`. Visual observations do not resolve the strict cost gate.\n\nThe two ordinary High runs pass the debris p99 target, but the current debris evidence remains on its accepted prior source. R in these two raw reports follows frame 959 as artifact frame 960 outside phase statistics; continuing clock and character state prevent a pristine/R pixel-identity claim. This archive retains 6,720 primary rows and 2,856 warmed High pairs. Additional image originals remain in durable scratch with hashes; selected phase originals and the first-six native crops remain here. GPU duration is unavailable.\n')
write(ARCHIVE / 'artifacts.json', {str(path.relative_to(ARCHIVE)): identity(path) for path in sorted(ARCHIVE.rglob('*')) if path.is_file() and path.name not in ['artifacts.json', 'SHA256SUMS']})
(ARCHIVE / 'SHA256SUMS').write_text(''.join(identity(path)['sha256'] + '  ' + str(path.relative_to(ARCHIVE)) + '\n' for path in sorted(ARCHIVE.rglob('*')) if path.is_file() and path.name != 'SHA256SUMS'))
print(ARCHIVE)
print('Strict cost attempt archived; existing top-level acceptance remains unchanged')

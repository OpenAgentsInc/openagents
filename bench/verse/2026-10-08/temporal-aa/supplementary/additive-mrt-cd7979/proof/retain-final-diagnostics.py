#!/usr/bin/env python3
"""Complete failed stage archives and retain the separate additive MRT diagnostic."""
import hashlib
import json
from pathlib import Path
import shutil
import struct

SCRATCH = Path(__file__).resolve().parent
ROOT = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents/bench/verse/2026-10-08/temporal-aa/supplementary')
ATTEMPTS = [
    ('10936-stage-diagnostics', 'stage-history-1d38', '1d38cf6812690967f814e9c24624e79fea03bc21', [464, 476], 'visual_failed'),
    ('10936-current-footprint', 'current-footprint-bfa', 'bfa66f3b07e8081d06bb084fa1e348d946684986', [464, 476], 'visual_failed'),
    ('10936-additive-mrt', 'additive-mrt-cd7979', 'cd7979bedbeab7a44d71cec857b942ab0f3ca8e1', [468, 474], 'scoped_visual_diagnostic_pass'),
]


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def identity(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return {'sha256': h.hexdigest(), 'bytes': path.stat().st_size}


def png_size(path):
    with path.open('rb') as stream:
        header = stream.read(24)
    assert header[:8] == b'\x89PNG\r\n\x1a\n' and header[12:16] == b'IHDR'
    return list(struct.unpack('>II', header[16:24]))


def copy(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        assert identity(source) == identity(target), f'Refuse to overwrite different retained data: {target}'
    else:
        shutil.copy2(source, target)


def json_lines(path, rows):
    path.write_text(''.join(json.dumps(row, separators=(',', ':')) + '\n' for row in rows))


for prefix, folder, source, sequence, status in ATTEMPTS:
    raw = SCRATCH / prefix
    target = ROOT / folder
    target.mkdir(parents=True, exist_ok=True)
    manifest = read(SCRATCH / (prefix + '-manifest.json'))
    receipt = read(SCRATCH / (prefix + '-gpu.json'))
    report = read(raw / 'capture.json')
    assert manifest['source'] == source and manifest['exit'] == receipt['exit'] == 0
    assert receipt['held_whole_run'] and receipt['acquired_at_ms'] <= manifest['start_unix'] * 1000 and receipt['released_at_ms'] >= manifest['end_unix'] * 1000
    assert report['temporal_texture_diagnostics']['enabled'] and not report['temporal_texture_diagnostics']['timing_acceptance_available']
    assert [row['frame'] for row in report['temporal_texture_diagnostics']['frames']] == list(range(sequence[0], sequence[1] + 1))
    copy(raw / 'capture.json', target / 'capture.json')
    for suffix in ['.py', '-manifest.json', '-gpu.json', '.log', '-build.log', '-build-lease.json']:
        copy(SCRATCH / (prefix + suffix), target / 'proof' / (prefix + suffix))
    copy(Path(__file__).resolve(), target / 'proof' / 'retain-final-diagnostics.py')
    shutil.copy2(SCRATCH / '10936-diagnostic-archive-check.py', target / 'check.py')
    json_lines(target / 'frame-ledger.jsonl', report['submission_timing']['frames'])
    json_lines(target / 'diagnostic-frames.jsonl', report['temporal_texture_diagnostics']['frames'])
    paired = report.get('temporal_comparison')
    json_lines(target / 'temporal-pairs.jsonl', [row for phase in paired['phases'].values() for row in phase['frame_results']] if paired else [])
    if prefix == '10936-additive-mrt':
        inventory = {}
        for path in sorted(raw.rglob('*.png')):
            relative = str(path.relative_to(raw))
            item = {'path': str(path), **identity(path), 'pixels': png_size(path), 'storage': 'durable_scratch'}
            destination = None
            if path.name.endswith(('-marker.png', '-history-reactive.png', '-additive-fx.png')):
                destination = target / 'masks' / path.name
            elif path.name[:4] in ['0471', '0472']:
                destination = target / 'selected' / path.name
            if destination:
                copy(path, destination)
                item.update(storage='git', file=str(destination.relative_to(target)))
            inventory[relative] = item
        write(target / 'source-images.json', {'schema': 'openagents.verse.image-inventory.v1', 'images': inventory})
        numeric_path = SCRATCH / (prefix + '-current-footprint-differences.json')
        numeric = read(numeric_path)
        assert numeric['source'] == source and [row['frame'] for row in numeric['rows']] == list(range(468, 475))
        assert all(row['current_nonretainable_rgb_max_8bit_difference'] == row['current_nonretainable_rgb_changed_pixels'] == 0 for row in numeric['rows'])
        copy(numeric_path, target / 'audit' / 'current-footprint-differences.json')
        boards = {}
        for frame in range(469, 473):
            original = SCRATCH / f'{prefix}-stages-{frame}.png'
            saved = target / 'audit' / f'{frame}-stages.png'
            copy(original, saved)
            boards[str(saved.relative_to(target))] = {'path': str(original), **identity(original), 'pixels': png_size(original), 'crop_bounds_from_root_review': [490, 260, 700, 455], 'method': 'Root-created magnified review board, copied byte-for-byte; full original diagnostic PNGs and their identities are retained separately.'}
        write(target / 'audit' / 'review-boards.json', {'source': source, 'boards': boards})
        verification = {'schema': 'openagents.verse.temporal-visual-attempt.v1', 'status': status, 'source': source, 'binary_sha256': manifest['binary_sha256'], 'profile': manifest['profile'], 'features': manifest['features'], 'timing_acceptance_available': False, 'supersedes_top_level_acceptance': False, 'simulation_frames': 540, 'sequence_frames': sequence, 'command_manifest': 'proof/' + prefix + '-manifest.json', 'gpu_receipt': 'proof/' + prefix + '-gpu.json', 'report': 'capture.json', 'source_images': 'source-images.json', 'observations': 'audit/current-footprint-differences.json', 'root_visual_review': {'frames': [471, 472], 'bounds': [490, 260, 700, 455], 'detached_head_ghost_seen': False, 'verdict': 'The previously detached head contour is absent in the reviewed region; fire pixels agree across incoming HDR, new history, and sharpened review stages.'}, 'limitations': ['GPU-only diagnostic; no quiet receipt or timing gate', 'Additional texture snapshots/readbacks and temporal-diagnostics feature differ from final capture-only builds', 'Mapped 8-bit equality does not establish equality of underlying HDR floats', 'A seven-frame diagnostic and scoped root review do not replace the final six-case visual and timing acceptance', 'Published kit selected locally; no cross-asset or cross-source speedup claim']}
        write(target / 'verification.json', verification)
        (target / 'README.md').write_text(
            f'The additive MRT diagnostic uses source `{source}` and binary SHA-256 `{manifest["binary_sha256"]}`. The GPU-only High 9-second run completes 540 primary frames and captures seven texture diagnostics at frames 468–474. The root reviews frames 471 and 472 in crop `[490, 260, 700, 455]` and sees no detached head contour in that region. This scoped diagnostic pass does not establish final six-case acceptance or a timing gate.\n\n'
            '[Stage 471](audit/471-stages.png) and [stage 472](audit/472-stages.png) retain the root-created magnified boards. [Numeric comparisons](audit/current-footprint-differences.json) report zero mapped 8-bit RGB differences between incoming HDR and new history at every current negative-alpha pixel across all seven frames. These quantized comparisons do not establish equality of the underlying HDR floats. The masks retain separate lit-body R and actual additive-FX G coverage.\n\n'
            '[capture.json](capture.json) preserves the exact raw report, including every timing sample, outlier, diagnostic record, and matrix. [frame-ledger.jsonl](frame-ledger.jsonl) and [diagnostic-frames.jsonl](diagnostic-frames.jsonl) export unchanged rows for inspection. [source-images.json](source-images.json) records SHA-256, byte size, dimensions, and original durable location for all 53 PNGs. All seven body, negative-history, and additive-FX masks, plus the full original stages for frames 471 and 472, remain in Git. Other originals remain in durable scratch. The exact runner, argv manifest, capture log, build log, and lease receipts remain under `proof/`.\n\n'
            'Run `python3 check.py --raw` to verify the report, exported rows, original images, retained originals, lease coverage, and complete file hashes. Omit `--raw` when durable scratch is unavailable.\n')
    else:
        verification = read(target / 'verification.json')
        assert verification['source'] == source and verification['status'] == 'visual_failed'
        inventory = read(target / 'source-images.json')['images']
        assert set(inventory) == {str(path.relative_to(raw)) for path in raw.rglob('*.png')}
        for relative, item in inventory.items():
            assert identity(raw / relative) == {key: item[key] for key in ['sha256', 'bytes']}
        with (target / 'README.md').open('a') as stream:
            text = '\nThe diagnostic exits 0 but its visual verdict remains `visual_failed`: detached contours persist at frames 471 and 472. [frame-ledger.jsonl](frame-ledger.jsonl), [temporal-pairs.jsonl](temporal-pairs.jsonl), and [diagnostic-frames.jsonl](diagnostic-frames.jsonl) retain all primary, paired, and texture diagnostic rows without filtering. Run `python3 check.py --raw` to verify the raw copy, exported rows, original image identities, retained originals, GPU lease coverage, and complete file hashes. This archive does not supersede top-level acceptance.\n'
            if text not in (target / 'README.md').read_text():
                stream.write(text)
    write(target / 'archive.json', {'schema': 'openagents.verse.diagnostic-archive.v1', 'source': source, 'binary_sha256': manifest['binary_sha256'], 'status': status, 'raw_report': {'path': str(raw / 'capture.json'), **identity(raw / 'capture.json')}, 'original_pngs': len(inventory), 'primary_rows': len(report['submission_timing']['frames']), 'diagnostic_rows': len(report['temporal_texture_diagnostics']['frames']), 'paired_rows': sum(len(phase['frame_results']) for phase in paired['phases'].values()) if paired else 0, 'timing_acceptance_available': False, 'supersedes_top_level_acceptance': False})
    # Finish the inventories after every retained artifact exists.
    write(target / 'artifacts.json', {str(path.relative_to(target)): identity(path) for path in sorted(target.rglob('*')) if path.is_file() and path.name not in ['artifacts.json', 'SHA256SUMS']})
    (target / 'SHA256SUMS').write_text(''.join(identity(path)['sha256'] + '  ' + str(path.relative_to(target)) + '\n' for path in sorted(target.rglob('*')) if path.is_file() and path.name != 'SHA256SUMS'))
    print(folder, status, 'retained with', len(inventory), 'original PNG identities')

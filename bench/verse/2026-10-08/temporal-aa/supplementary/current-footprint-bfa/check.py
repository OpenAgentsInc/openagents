#!/usr/bin/env python3
"""Check a retained diagnostic archive without inferring timing acceptance."""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def identity(path):
    return {'sha256': digest(path), 'bytes': path.stat().st_size}


def read(path):
    return json.loads(path.read_text())


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def png_size(path):
    with path.open('rb') as stream:
        header = stream.read(24)
    require(header[:8] == b'\x89PNG\r\n\x1a\n' and header[12:16] == b'IHDR', f'Invalid PNG header: {path}')
    return list(struct.unpack('>II', header[16:24]))


def check(root, raw):
    verification = read(root / 'verification.json')
    manifest = read(root / verification['command_manifest'])
    receipt = read(root / verification['gpu_receipt'])
    report = read(root / verification['report'])
    require(verification['source'] == manifest['source'] and verification['binary_sha256'] == manifest['binary_sha256'], 'Source/binary attribution changed')
    require(verification['timing_acceptance_available'] is False and verification['supersedes_top_level_acceptance'] is False, 'A diagnostic archive must not promote timing or top-level acceptance')
    require(manifest['exit'] == receipt['exit'] == 0 and receipt['resource'] == 'gpu' and receipt['held_whole_run'], 'Incomplete native GPU run')
    require(receipt['acquired_at_ms'] <= manifest['start_unix'] * 1000 and receipt['released_at_ms'] >= manifest['end_unix'] * 1000, 'GPU receipt misses a capture boundary')
    require('--temporal-diagnostics' in manifest['command'] and 'temporal-diagnostics' in manifest['features'], 'This archive must retain its actual diagnostic configuration')
    require(report['temporal_texture_diagnostics']['enabled'] and not report['temporal_texture_diagnostics']['timing_acceptance_available'], 'Raw report changed its diagnostic timing exclusion')
    frames = verification['simulation_frames']
    timing = report['submission_timing']
    require(timing['submitted_frames'] == timing['completed_frames'] == frames and sum(p['frames'] for p in report['phases'].values()) == frames, 'Incomplete frame counts')
    require([row['index'] for row in timing['frames']] == list(range(frames)), 'Primary frame ledger is incomplete or reordered')
    require(rows(root / 'frame-ledger.jsonl') == timing['frames'], 'Exported primary frame rows changed')
    diagnostics = report['temporal_texture_diagnostics']['frames']
    require(rows(root / 'diagnostic-frames.jsonl') == diagnostics, 'Exported diagnostic rows changed')
    first, last = verification['sequence_frames']
    require([row['frame'] for row in diagnostics] == list(range(first, last + 1)), 'Missing or reordered diagnostic sequence')
    paired = report.get('temporal_comparison')
    expected_pairs = [row for phase in paired['phases'].values() for row in phase['frame_results']] if paired else []
    require(rows(root / 'temporal-pairs.jsonl') == expected_pairs, 'Exported paired rows changed')
    inventory = read(root / 'source-images.json')['images']
    for relative, item in inventory.items():
        if raw:
            source = Path(item['path'])
            require(identity(source) == {key: item[key] for key in ['sha256', 'bytes']}, f'Raw source PNG changed: {relative}')
            require(png_size(source) == item['pixels'], f'Raw PNG dimensions changed: {relative}')
        if item['storage'] == 'git':
            retained = root / item['file']
            require(identity(retained) == {key: item[key] for key in ['sha256', 'bytes']}, f'Retained original PNG changed: {relative}')
            require(png_size(retained) == item['pixels'], f'Retained PNG dimensions changed: {relative}')
    for row in diagnostics:
        for name in row['files']:
            require(name in inventory, f'Uninventoried diagnostic source PNG: {name}')
    numeric_path = root / 'audit/current-footprint-differences.json'
    if numeric_path.exists():
        numeric = read(numeric_path)
        require(numeric['source'] == manifest['source'], 'Numeric comparison source changed')
        require([row['frame'] for row in numeric['rows']] == [row['frame'] for row in diagnostics], 'Numeric comparison misses a diagnostic frame')
        for numeric_row, diagnostic in zip(numeric['rows'], diagnostics):
            require(numeric_row['body_pixels'] == diagnostic['marker_covered_pixels'] and numeric_row['additive_fx_pixels'] == diagnostic['additive_fx_covered_pixels'] and numeric_row['nonretainable_pixels'] == diagnostic['nonretainable_history_pixels'], 'Numeric pixel counts differ from the raw diagnostic')
            require(numeric_row['current_nonretainable_rgb_max_8bit_difference'] == numeric_row['current_nonretainable_rgb_changed_pixels'] == 0, 'Mapped current/history equality claim changed')
        boards = read(root / 'audit/review-boards.json')
        require(boards['source'] == manifest['source'], 'Review board source changed')
        for name, item in boards['boards'].items():
            require(identity(root / name) == {key: item[key] for key in ['sha256', 'bytes']} and png_size(root / name) == item['pixels'], 'Review board changed')
            if raw:
                require(identity(Path(item['path'])) == {key: item[key] for key in ['sha256', 'bytes']}, 'Original root review board changed')
    if raw:
        original_root = Path(manifest['command'][1])
        require(set(inventory) == {str(path.relative_to(original_root)) for path in original_root.rglob('*.png')}, 'Image inventory misses an original PNG')
        require(identity(original_root / 'capture.json') == identity(root / 'capture.json'), 'Raw report copy changed')
    artifact_inventory = read(root / 'artifacts.json')
    actual = {str(path.relative_to(root)): identity(path) for path in sorted(root.rglob('*')) if path.is_file() and path.name not in ['artifacts.json', 'SHA256SUMS']}
    require(artifact_inventory == actual, 'Artifact hash inventory is incomplete or stale')
    sums = {line.split('  ', 1)[1]: line.split('  ', 1)[0] for line in (root / 'SHA256SUMS').read_text().splitlines()}
    covered = {str(path.relative_to(root)): digest(path) for path in sorted(root.rglob('*')) if path.is_file() and path.name != 'SHA256SUMS'}
    require(sums == covered, 'SHA256SUMS is incomplete or stale')
    print(f"DIAGNOSTIC ARCHIVE PASS: {verification['source']} / {verification['status']}; {frames} primary rows, {len(expected_pairs)} paired rows, {len(diagnostics)} diagnostic rows, {len(inventory)} original PNG identities. Timing remains unavailable.")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--raw', action='store_true', help='Also check every original PNG and raw report in durable scratch')
    arguments = parser.parse_args()
    check(arguments.root, arguments.raw)

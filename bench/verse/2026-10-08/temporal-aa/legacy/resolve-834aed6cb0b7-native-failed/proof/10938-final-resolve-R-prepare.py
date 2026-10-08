#!/usr/bin/env python3
"""Prepare a separate final saved-view R supplement without changing acceptance."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys

sys.dont_write_bytecode = True
SCRATCH = Path(__file__).resolve().parent
FINAL = '834aed6cb0b79c80fbd009c3caa51c78dba06628'


def identity(path):
    path = Path(path)
    h = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    return {'path': str(path.resolve()), 'sha256': h.hexdigest(), 'bytes': path.stat().st_size}


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True)
    parser.add_argument('--plan', type=Path, default=SCRATCH / '10936-final-resolve-curation-plan.json')
    parser.add_argument('--stage', action='store_true', help='Copy the proposed release R evidence into scratch only')
    args = parser.parse_args()
    if args.source != FINAL:
        raise ValueError('Use the explicitly frozen final runtime source')
    spec = importlib.util.spec_from_file_location('debris_refresh', SCRATCH / '10937-final-resolve-evidence-refresh.py')
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    plan, checked = helper.inspect(args.plan, args.source)
    binding = plan['cases']['high-release']
    prefix = Path(binding['prefix'])
    report = json.loads((prefix / 'capture.json').read_text())
    job = json.loads(Path(binding['manifest']).read_text())
    rebuilt = report['rebuild_capture']
    if not (rebuilt['pristine_frame'] == 0 and rebuilt['restoration_frame'] == 960 and rebuilt['after_simulation_frame'] == 959 and rebuilt['pristine_view'] == rebuilt['restored_view']):
        raise ValueError('The R receipt must restore the saved view after the final simulation frame')
    if 960 not in report['pixel_readback']['additional_artifact_readback_frame_indices']:
        raise ValueError('The additional R readback is missing')
    proof = {'schema': 'openagents.verse.saved-view-rebuild.v1', 'status': 'root_visual_review_pending', 'source': args.source, 'profile': job['profile'], 'features': job['features'], 'binary_sha256': job['binary_sha256'], 'manifest': identity(binding['manifest']), 'report': identity(prefix / 'capture.json'), 'images': {name: identity(prefix / rebuilt[key]) for name, key in [('pristine', 'pristine_image'), ('restored', 'restored_image')]}, 'rebuild_capture': rebuilt, 'submission_frames': report['submission_timing']['submitted_frames'], 'completed_frames': report['submission_timing']['completed_frames'], 'phase_statistic_frames': sum(phase['frames'] for phase in report['phases'].values()), 'phase_end': report['phases']['after']['debris_end'], 'leases': checked['high-release']['leases'], 'temporal_aa_enabled': report['temporal_aa_enabled'], 'limits': ['Saved pristine view and stage are reused after R without a simulation step.', 'Clock and character state continue, so pristine/R pixel equality is not claimed.', 'R is artifact frame 960 after simulation frame 959 and does not enter the 960 primary frame rows or phase statistics.', 'This supplement adds a final-source R view; the source46cf matched AA-off controls retain their original source and scope.', 'Visual review is pending; no off/on control, GPU-duration, or selective-worker convergence claim is derived from this R pair.'], 'acceptance': {'visual': None, 'promotion_authorized': False}}
    output = SCRATCH / '10938-final-resolve-R-inspection.json'
    write(output, proof)
    print(output)
    if args.stage:
        stage = SCRATCH / '10938-final-resolve-R-staged'
        if stage.exists():
            raise ValueError('Retain the existing proposed R supplement separately before another stage')
        stage.mkdir()
        files = {'capture.json': prefix / 'capture.json', 'capture.log': Path(job['log']), 'command-manifest.json': Path(binding['manifest']), 'quiet-lease.json': Path(binding['quiet_receipt']), 'gpu-lease.json': Path(binding['gpu_receipt']), 'pristine.png': prefix / rebuilt['pristine_image'], 'restored.png': prefix / rebuilt['restored_image'], 'aftermath.png': prefix / 'aftermath.png'}
        for name, original in files.items():
            shutil.copy2(original, stage / name)
        (stage / 'frame-ledger.jsonl').write_text(''.join(json.dumps(row, separators=(',', ':')) + '\n' for row in report['submission_timing']['frames']))
        write(stage / 'verification.proposed.json', proof)
        write(stage / 'artifacts.json', {path.name: {key: value for key, value in identity(path).items() if key != 'path'} for path in sorted(stage.iterdir()) if path.is_file() and path.name not in ['artifacts.json', 'SHA256SUMS']})
        (stage / 'SHA256SUMS').write_text(''.join(identity(path)['sha256'] + '  ' + path.name + '\n' for path in sorted(stage.iterdir()) if path.is_file() and path.name != 'SHA256SUMS'))
        print(stage)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, TypeError) as error:
        print('R supplement preparation failed: ' + str(error), file=sys.stderr)
        raise SystemExit(2)

#!/usr/bin/env python3
"""Preserve prior debris evidence and inspect an explicitly bound final attempt."""
import argparse
import copy
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
SCRATCH = Path(__file__).resolve().parent
ROOT = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
DEBRIS = ROOT / 'bench/verse/2026-10-08/instanced-debris'
RELIGHT = DEBRIS.parent / 'destruction-relighting'
PRIOR = '36df9ceb4bbb7fb0c7be30bd1c3538e0be78ab3e'
FINAL = '834aed6cb0b79c80fbd009c3caa51c78dba06628'
ARCHIVE = DEBRIS / ('legacy/reactive-production-' + PRIOR[:12])


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    return json.loads(Path(path).read_text())


def identity(path):
    path = Path(path)
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return {'sha256': h.hexdigest(), 'bytes': path.stat().st_size}


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def refresh_inventory(folder):
    verification = read(folder / 'verification.json')
    verification['artifacts'] = {str(path.relative_to(folder)): identity(path) for path in sorted(folder.rglob('*')) if path.is_file() and path not in [folder / 'verification.json', folder / 'SHA256SUMS']}
    write(folder / 'verification.json', verification)
    (folder / 'SHA256SUMS').write_text(''.join(identity(path)['sha256'] + '  ' + str(path.relative_to(folder)) + '\n' for path in sorted(folder.rglob('*')) if path.is_file() and path != folder / 'SHA256SUMS'))


def check_archive():
    archived = read(ARCHIVE / 'archive.json')
    require(archived['source'] == PRIOR, 'Prior archive source changed')
    for original, item in archived['copied_files'].items():
        require(identity(ARCHIVE / item['file']) == {key: item[key] for key in ['sha256', 'bytes']}, f'Prior retained file changed: {original}')
    original = read(ARCHIVE / 'original-verification.json')
    require(original['tested_source_commit'] == PRIOR and original['acceptance'] == archived['acceptance_at_archive'], 'Prior acceptance changed')
    for run in original['current_runs'].values():
        raw = read(ARCHIVE / run['report'])
        require(identity(ARCHIVE / run['report'])['sha256'] == run['report_sha256'], 'Prior raw report changed')
        require([json.loads(line) for line in (ARCHIVE / run['frame_ledger']).read_text().splitlines()] == raw['submission_timing']['frames'], 'Prior ledger changed')
    print('PRIOR ARCHIVE PASS: ' + PRIOR + '; original reports, ledgers, images, metadata, commands, logs, and leases remain byte-identical')


def prepare_archive():
    current = read(DEBRIS / 'verification.json')
    require(current['tested_source_commit'] == PRIOR, 'Prepare the prior archive before refreshing current evidence')
    if ARCHIVE.exists():
        check_archive()
        return
    ARCHIVE.mkdir(parents=True)
    files = {}
    rename = {'README.md': 'original-README.md', 'verification.json': 'original-verification.json', 'check.py': 'original-check.py', 'SHA256SUMS': 'original-SHA256SUMS'}
    for source in sorted(DEBRIS.iterdir()):
        if not source.is_file() or not (source.name.startswith('current-') or source.name in rename):
            continue
        destination = ARCHIVE / rename.get(source.name, source.name)
        shutil.copy2(source, destination)
        files[source.name] = {'file': destination.name, **identity(source)}
    write(ARCHIVE / 'archive.json', {'schema': 'openagents.verse.debris-prior-archive.v1', 'source': PRIOR, 'original_root': str(DEBRIS), 'copied_files': files, 'acceptance_at_archive': current['acceptance'], 'note': 'This snapshot preserves the accepted current 36df attempt byte-for-byte. Older 46cf production and DAE history remain at the evidence root. The original verification, README, check script, and SHA inventory keep their original root-relative context and are explicitly named original-*; use the copied-files map to verify this subset.'})
    (ARCHIVE / 'README.md').write_text('This archive preserves the accepted production debris attempt on source `' + PRIOR + '` before the final MRT refresh. All files in `archive.json` are byte-identical copies of their original paths. The two raw reports retain 960 rows each, all outliers, and the original acceptance values. The exact ledgers, eight selected original PNGs, command manifest, capture and build logs, shared quiet/GPU receipts, and historical native/check scripts remain copied.\n\nThe `original-*` files retain their original evidence-root context. Their references to older production, DAE, and temporal files still describe the original root; those historical files remain there. This subset has its own copied-file hash map in [archive.json](archive.json). The active final reports do not replace or relabel this source.\n')
    current['archived_reactive_current'] = {'source': PRIOR, 'directory': str(ARCHIVE.relative_to(DEBRIS)), 'archive': str((ARCHIVE / 'archive.json').relative_to(DEBRIS)), 'acceptance_unchanged': True}
    before_acceptance = copy.deepcopy(current['acceptance'])
    write(DEBRIS / 'verification.json', current)
    refresh_inventory(DEBRIS)
    require(read(DEBRIS / 'verification.json')['acceptance'] == before_acceptance, 'Archive preparation changed current acceptance')
    check_archive()


def inspect(plan_path, source):
    require(source == FINAL and re.fullmatch(r'[0-9a-f]{40}', source), 'Use the explicitly frozen final runtime source')
    plan = read(plan_path)
    require(plan['source'] == source, 'Final plan source changed')
    runner = load_module('final_mrt_runner', SCRATCH / '10936-final-resolve-runner.py')
    results = {}
    for index, label in [(0, 'high-dev'), (1, 'high-release')]:
        binding = plan['cases'][label]
        manifest = read(binding['manifest'])
        rows = manifest if isinstance(manifest, list) else [manifest]
        matching = [row for row in rows if row.get('name') == binding['job']]
        require(len(matching) == 1, 'Missing or repeated final case manifest')
        job = matching[0]
        require(job['exit'] == 0 and job['source'] == source and job['binary_sha256'] == binding['binary_sha256'] and job['profile'] == binding['profile'], 'Final case source/profile/binary changed')
        require(job['features'] == ['capture'] and '--temporal-diagnostics' not in job['command'], 'Use capture-only final cases')
        require(str(Path(job['command'][1]).resolve()) == str(Path(binding['prefix']).resolve()), 'Final report prefix differs from the command')
        require(job['command'] == runner.capture_argv(index, job['command'][0]), 'Final command differs from the six-case specification')
        for resource in ['gpu', 'quiet']:
            receipt = read(binding[resource + '_receipt'])
            require(receipt['resource'] == resource and receipt['exit'] == 0 and receipt['held_whole_run'] and receipt['nested'] is False, 'Final case lacks an independent completed ' + resource + ' receipt')
            require(receipt['acquired_at_ms'] <= job['start_unix'] * 1000 and receipt['released_at_ms'] >= job['end_unix'] * 1000, 'Final ' + resource + ' receipt misses a capture boundary')
        report_path = Path(binding['prefix']) / 'capture.json'
        report = read(report_path)
        case_check = runner.check_report(index, report)
        frame_rows = report['submission_timing']['frames']
        for phase, (first, last) in {'before': (0, 330), 'swarm': (330, 690), 'after': (690, 960)}.items():
            values = sorted(row['frame_ms'] for row in frame_rows[first:last])
            require(report['phases'][phase]['frames'] == len(values), 'Final phase boundaries changed')
            for statistic, fraction in [('p50', .5), ('p99', .99), ('max', 1)]:
                expected = values[math.floor((len(values) - 1) * fraction + .5)]
                require(abs(expected - report['phases'][phase]['frame_ms'][statistic]) < .0001, 'Raw phase statistic differs from its unfiltered frame rows')
        for position, row in enumerate(frame_rows):
            end = frame_rows[position + 1]['started_ms'] if position + 1 < len(frame_rows) else report['submission_timing']['elapsed_through_final_drain_ms']
            require(row['started_ms'] <= row['submitted_ms'] <= row['observed_completed_ms'], 'Final frame chronology changed')
            require(abs(row['continuous_iteration_ms'] - (end - row['started_ms'])) < .001, 'Final cadence interval differs from adjacent starts/final drain')
        require(abs(sum(row['continuous_iteration_ms'] for row in frame_rows) - report['submission_timing']['elapsed_through_final_drain_ms']) < .001, 'The primary cadence ledger does not cover the complete timed drain')
        require(report['gpu_timestamps_requested'] is False and report['gpu_timestamps_available'] is False, 'Ordinary High runs must not claim GPU-duration diagnostics')
        require(report['phases']['swarm']['chunks_max'] == 509 and report['phases']['swarm']['posed_vertices_max'] == 0, 'Final debris geometry/instancing counters changed')
        phases = report['phases']
        acceptance = {'frame_p99_limit_ms': 16.7, 'swarm_p99_ms': phases['swarm']['frame_ms']['p99'], 'after_p99_ms': phases['after']['frame_ms']['p99'], 'swarm_pass': phases['swarm']['frame_ms']['p99'] < 16.7, 'after_pass': phases['after']['frame_ms']['p99'] < 16.7}
        acceptance['timing_pass'] = acceptance['swarm_pass'] and acceptance['after_pass']
        results[label] = {'source': source, 'binary_sha256': job['binary_sha256'], 'profile': job['profile'], 'manifest': {'path': binding['manifest'], **identity(binding['manifest'])}, 'report': {'path': str(report_path), **identity(report_path)}, 'leases': {resource: {'path': binding[resource + '_receipt'], **identity(binding[resource + '_receipt'])} for resource in ['gpu', 'quiet']}, 'computed_acceptance': acceptance, 'swarm_max_ms': phases['swarm']['frame_ms']['max'], 'case_integrity': case_check, 'root_visual_review': None, 'promotion_authorized': False}
    output = SCRATCH / '10937-final-resolve-inspection.json'
    write(output, {'status': 'review_pending', 'source': source, 'cases': results, 'promotion_authorized': False, 'acceptance_changed': False, 'note': 'Per-case GPU/quiet receipts cover each exact command. All 960 primary rows and every outlier remain in the raw report. R follows primary frame 959 as artifact frame 960, outside phase statistics. A computed timing flag is not a final root visual verdict.'})
    print(output)
    return plan, results


def prepare_checker(source):
    require(source == FINAL, 'Use the explicitly frozen final runtime source')
    original = (DEBRIS / 'check.py').read_text()
    require("SOURCE = '" + PRIOR + "'" in original, 'Prepare the checker from the unchanged prior current evidence')
    code = original.replace("SOURCE = '" + PRIOR + "'", "SOURCE = '" + source + "'")
    code = code.replace("    quiet = read(HERE / 'current-quiet-lease.json')\n    gpu = read(HERE / 'current-gpu-lease.json')\n", '')
    code = code.replace("        entry = next(row for row in manifest if row['name'] == '10937-high-reactive-' + short)", "        entry = next(row for row in manifest if row['name'] == run['capture_job_name'])\n        quiet = read(HERE / run['quiet_lease_receipt'])\n        gpu = read(HERE / run['gpu_lease_receipt'])")
    code = code.replace("    native = debris['reactive_native_evidence']", "    for profile in ['development', 'release']:\n        old = debris['historical_reactive_production_runs'][profile]\n        assert old['tested_source_commit'] == '" + PRIOR + "'\n        count += ledger(HERE, old)\n    quiet = read(HERE / 'legacy/reactive-production-" + PRIOR[:12] + "/current-quiet-lease.json')\n    gpu = read(HERE / 'legacy/reactive-production-" + PRIOR[:12] + "/current-gpu-lease.json')\n    native = debris['reactive_native_evidence']")
    code = code.replace("    assert read(HERE / 'current-build-lease.json')['exit'] == 0", "    for run in debris['current_runs'].values():\n        assert read(HERE / run['build_lease_receipt'])['exit'] == 0")
    # Include nested inventories when a future supplement has its own SHA file.
    code = code.replace("if path.is_file() and path.name != 'SHA256SUMS'", "if path.is_file() and path != folder / 'SHA256SUMS'")
    output = SCRATCH / '10937-final-resolve-check.py'
    output.write_text(code)
    compile(code, str(output), 'exec')
    print(output)


def stage_refresh(plan_path, source):
    plan, inspected = inspect(plan_path, source)
    # All six finished jobs must bind before preparing the proposed current record.
    curator = load_module('six_case_curator', DEBRIS.parent / 'temporal-aa/curate.py')
    curator.validate(plan)
    check_archive()
    current = read(DEBRIS / 'verification.json')
    require(current['tested_source_commit'] == PRIOR, 'Do not overwrite a newer current attempt')
    stage = SCRATCH / '10937-final-resolve-staged'
    require(not stage.exists(), 'Retain an existing proposed refresh separately before staging another')
    stage.mkdir()
    prior_runs = copy.deepcopy(current['current_runs'])
    for run in prior_runs.values():
        for key in ['report', 'capture_log', 'quiet_lease_receipt', 'gpu_lease_receipt', 'frame_ledger']:
            run[key] = str(ARCHIVE.relative_to(DEBRIS) / run[key])
        run['images'] = {key: str(ARCHIVE.relative_to(DEBRIS) / value) for key, value in run['images'].items()}
    current['historical_reactive_production_runs'] = prior_runs
    current['historical_reactive_production_checks'] = copy.deepcopy(current['current_checks'])
    for key in ['log', 'command_script', 'build_lease']:
        value = current['historical_reactive_production_checks'][key]
        current['historical_reactive_production_checks'][key] = str(ARCHIVE.relative_to(DEBRIS) / value)
    current['historical_reactive_production_capture'] = copy.deepcopy(current['capture'])
    current['historical_reactive_production_visual_review'] = copy.deepcopy(current['visual_review'])
    current['current_runs'] = {}
    jobs, builds = [], {}
    for profile, label, short in [('development', 'high-dev', 'dev'), ('release', 'high-release', 'release')]:
        binding = plan['cases'][label]
        prefix = Path(binding['prefix'])
        job = read(binding['manifest'])
        report = read(prefix / 'capture.json')
        jobs.append(job)
        destinations = {'report': f'current-{short}-capture.json', 'capture_log': f'current-{short}-capture.log', 'quiet_lease_receipt': f'current-{short}-quiet-lease.json', 'gpu_lease_receipt': f'current-{short}-gpu-lease.json', 'build_lease_receipt': f'current-{short}-build-lease.json', 'build_log': f'current-{short}-build.log', 'raw_command_manifest': f'current-{short}-command-manifest.json', 'frame_ledger': f'current-{short}-frame-ledger.jsonl'}
        for key, original in [('report', prefix / 'capture.json'), ('capture_log', Path(job['log'])), ('quiet_lease_receipt', Path(binding['quiet_receipt'])), ('gpu_lease_receipt', Path(binding['gpu_receipt'])), ('build_lease_receipt', Path(job['build_receipt']['path']) if 'path' in job['build_receipt'] else None), ('build_log', Path(job['build_log']['path']) if 'path' in job['build_log'] else None), ('raw_command_manifest', Path(binding['manifest']))]:
            if original is None:
                build_binding = read(job['build_binding']['path'])
                original = Path(build_binding['builds'][job['profile']]['build_receipt' if key == 'build_lease_receipt' else 'build_log'])
            shutil.copy2(original, stage / destinations[key])
        (stage / destinations['frame_ledger']).write_text(''.join(json.dumps(row, separators=(',', ':')) + '\n' for row in report['submission_timing']['frames']))
        images = {}
        for name in ['establishing', 'impact', 'ground-smoke', 'aftermath', 'pristine', 'restored']:
            require((prefix / (name + '.png')).is_file(), 'Missing selected final original: ' + name)
            destination = f'current-{short}-{name}.png'
            shutil.copy2(prefix / (name + '.png'), stage / destination)
            images[name] = destination
        run = copy.deepcopy(read(ARCHIVE / 'original-verification.json')['current_runs'][profile])
        run.update(destinations)
        run.update(status='review_pending', tested_source_commit=source, tested_binary=job['command'][0], tested_binary_sha256=job['binary_sha256'], build_profile=job['profile'], build_arguments=job['build_command'], capture_job_name=job['name'], capture_command=job['command'], environment=job['environment'], report_sha256=identity(stage / destinations['report'])['sha256'], frame_ledger_sha256=identity(stage / destinations['frame_ledger'])['sha256'], phases=report['phases'], acceptance=inspected[label]['computed_acceptance'], images=images, rebuild_capture=report['rebuild_capture'], warning_diagnostics=job['warning_diagnostics'])
        for key in ['width', 'height', 'fps', 'steps_per_frame', 'effective_quality', 'temporal_aa_enabled', 'light_settled_before_capture', 'adapter', 'pixel_readback', 'submission_timing', 'gpu_timestamps_requested', 'gpu_timestamps_available']:
            run[key] = report[key]
        run['artifact_checks'] = {'status': 'case_integrity_pass', 'primary_rows': 960, 'all_outliers_retained': True, 'R_artifact_frame': 960, 'after_simulation_frame': 959, 'R_enters_primary_phase_statistics': False}
        current['current_runs'][profile] = run
        builds[profile] = {'source': source, 'profile': job['profile'], 'features': job['features'], 'binary_sha256': job['binary_sha256'], 'command': job['build_command'], 'log': destinations['build_log'], 'build_lease': destinations['build_lease_receipt']}
    write(stage / 'current-command-manifest.json', jobs)
    shutil.copy2(SCRATCH / '10936-final-resolve-runner.py', stage / 'current-capture-runner.py')
    shutil.copy2(SCRATCH / '10937-final-resolve-check.py', stage / 'check.py')
    current.update(status='review_pending', tested_source_commit=source, tested_binary_sha256={profile: run['tested_binary_sha256'] for profile, run in current['current_runs'].items()}, tested_source_scope='Capture-only binaries built at the explicitly frozen runtime source. Root visual/timing review remains pending in this scratch proposal.', visual_review={'status': 'pending_root_verdict', 'source': source, 'scope': 'Selected High development/release originals and saved-view R proof; no pristine/R pixel-identity claim.'}, current_checks={'source': source, 'scope': 'Exact capture-only builds; earlier product/native checks remain at their recorded sources.', 'capture_builds': builds})
    current['capture'].pop('quiet_lease_receipt', None)
    current['capture'].pop('gpu_lease_receipt', None)
    current['capture'].update(per_case_lease_receipts={profile: {resource: run[resource + '_lease_receipt'] for resource in ['quiet', 'gpu']} for profile, run in current['current_runs'].items()}, rebuild_capture_enabled=True, R_timing_scope='R follows simulation frame 959 as artifact frame 960 after the final primary drain; R is excluded from phase statistics.')
    current['acceptance'].update({profile: run['acceptance'] for profile, run in current['current_runs'].items()})
    current['acceptance'].update(timing_pass=all(run['acceptance']['timing_pass'] for run in current['current_runs'].values()), development_swarm_max_ms=inspected['high-dev']['swarm_max_ms'], root_visual_pass=None, promotion_authorized=False)
    current['artifacts'] = {}
    write(stage / 'verification.proposed.json', current)
    write(stage / 'refresh-plan.json', {'status': 'review_pending', 'source': source, 'stage': str(stage), 'prior_archive': str(ARCHIVE), 'files': {path.name: identity(path) for path in sorted(stage.iterdir()) if path.is_file()}, 'top_level_acceptance_changed': False, 'promotion_authorized': False, 'required_before_promotion': 'The root must review all six final cases and explicitly approve these High metrics and selected visuals. Copy no proposed record over the current evidence before that verdict; rebuild its inventory after final docs and supplemental R proof exist.'})
    print(stage)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True)
    parser.add_argument('--plan', type=Path, default=SCRATCH / '10936-final-resolve-curation-plan.json')
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare-archive', action='store_true')
    mode.add_argument('--check-archive', action='store_true')
    mode.add_argument('--prepare-checker', action='store_true')
    mode.add_argument('--inspect', action='store_true')
    mode.add_argument('--stage-refresh', action='store_true', help='Prepare a scratch-only proposed refresh after all six jobs finish; do not promote')
    args = parser.parse_args()
    require(args.source == FINAL, 'Use the explicitly frozen final runtime source')
    if args.prepare_archive:
        prepare_archive()
    elif args.check_archive:
        check_archive()
    elif args.prepare_checker:
        prepare_checker(args.source)
    elif args.stage_refresh:
        stage_refresh(args.plan, args.source)
    else:
        inspect(args.plan, args.source)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, TypeError) as error:
        print('Evidence preparation failed: ' + str(error), file=sys.stderr)
        raise SystemExit(2)

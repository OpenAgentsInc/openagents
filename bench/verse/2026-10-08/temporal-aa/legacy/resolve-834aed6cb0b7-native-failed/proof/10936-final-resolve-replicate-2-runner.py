#!/usr/bin/env python3
"""Prepare or run one explicitly bound final temporal AA capture case."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

SCRATCH = Path(__file__).resolve().parent
ROOT = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
STEM = '10936-final-resolve'
PINS = {
    'kit': ('c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e', 21467658),
    'base_pack': ('a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f', 10636202),
    'baked_layers': ('fc5414a1bfef9e730f3d7d779e4447f12cc86d4e571042eec42518abb30ef7c2', 51684139),
}
CASES = [
    ('high-dev', 'dev', 'high', 16, None, None, True),
    ('high-release', 'release', 'high', 16, None, None, True),
    ('high-paired', 'release', 'high', 16, '439:484', None, False),
    ('medium-paired', 'release', 'medium', 16, '439:484', None, False),
    ('pan-paired', 'release', 'high', 8, '120:135', 'pan', False),
    ('orbit-paired', 'release', 'high', 8, '120:135', 'orbit', False),
]
WARNING_PATTERN = r'(?i)\b(?:warning|warn|validation error|device lost|panic)\b'
ALLOWED_ENV = {'VERSE_HOME', 'VERSE_KIT_BAKE'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path):
    return json.loads(Path(path).read_text())


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def identity(path):
    path = Path(path)
    return {'path': str(path.resolve()), 'sha256': digest(path), 'bytes': path.stat().st_size}


def write(path, value):
    path = Path(path)
    path.write_text(json.dumps(value, indent=2) + '\n')


def case_paths(index):
    label = CASES[index][0]
    name = '10936-final-resolve-high-replicate-2'
    return {key: str(SCRATCH / (name + suffix)) for key, suffix in {
        'prefix': '', 'manifest': '-manifest.json', 'log': '.log',
        'quiet_receipt': '-quiet.json', 'gpu_receipt': '-gpu.json', 'check': '-check.json',
    }.items()} | {'job': name, 'case': label, 'case_index': index}


def capture_argv(index, binary):
    _, _, _, seconds, sequence, camera, rebuild = CASES[index]
    argv = [str(binary), case_paths(index)['prefix'], '--live', '--settle-light', '--no-video', '--seconds', str(seconds)]
    if camera is None:
        argv += ['--impact-frame', '469', '--smoke-frame', '600']
    if rebuild:
        argv += ['--capture-rebuild']
    if sequence:
        argv += ['--compare-temporal-aa', '--sequence', sequence]
    if camera:
        argv += ['--static-houses', '--camera', camera]
    return argv


def verify_build_argv(command, profile):
    require(command[:2] == ['cargo', 'build'], 'Record a Cargo build argv')
    values, switches = {}, []
    cursor = 2
    while cursor < len(command):
        flag = command[cursor]
        if flag == '--release':
            switches.append(flag)
            cursor += 1
            continue
        require(flag in ['-p', '--example', '--features'] and flag not in values, 'Unexpected or repeated capture build flag')
        require(cursor + 1 < len(command), 'Missing capture build flag value')
        values[flag] = command[cursor + 1]
        cursor += 2
    require(values == {'-p': 'verse', '--example': 'meteor_showcase_capture', '--features': 'capture'}, 'Build exactly the capture-only meteor_showcase_capture example')
    require(switches == (['--release'] if profile == 'release' else []), 'Build release mode only for the release binding')


def verify_binding(binding, source, profiles):
    require(re.fullmatch(r'[0-9a-f]{40}', source), 'Use an exact lowercase 40-digit source SHA')
    require(binding['source'] == source, 'The supplied source differs from the build binding')
    require(binding['cwd'] == str(ROOT), 'Bind the build and capture working directory explicitly')
    require(set(binding['assets']) in [{'kit', 'base_pack'}, {'kit', 'base_pack', 'baked_layers'}], 'Bind the kit and base pack, and any selected baked layers')
    for label, asset in binding['assets'].items():
        require(Path(asset['path']).is_absolute(), f'Use an absolute asset path: {label}')
        require((asset['sha256'], asset['bytes']) == PINS[label], f'Asset differs from the published pin: {label}')
        require(identity(asset['path']) == asset, f'Asset bytes changed: {label}')
    base = ROOT / 'assets/verse/everglade' / (PINS['base_pack'][0] + '.vtp')
    require(binding['assets']['base_pack']['path'] == str(base), 'The compiled capture reads the base pack from its build workspace')
    selected = binding.get('environment', {})
    require(set(selected) <= ALLOWED_ENV, 'Only VERSE_HOME and an explicitly pinned VERSE_KIT_BAKE may supplement the fixed capture environment')
    if 'VERSE_KIT_BAKE' in selected:
        require('baked_layers' in binding['assets'], 'Bind the selected baked layer file')
        require(selected['VERSE_KIT_BAKE'] == binding['assets']['baked_layers']['path'], 'Baked layer environment differs from its asset binding')
    else:
        require('baked_layers' not in binding['assets'], 'Do not label an unused baked layer file as a capture input')
    for profile in profiles:
        build = binding['builds'][profile]
        require(build['source'] == source and build['profile'] == profile, f'Build source/profile changed: {profile}')
        require(build['features'] == ['capture'], f'Use capture-only builds, without temporal-diagnostics: {profile}')
        require(build['start_unix'] < build['end_unix'], f'Invalid build chronology: {profile}')
        verify_build_argv(build['command'], profile)
        require(Path(build['binary']['path']).is_absolute(), f'Use an absolute binary path: {profile}')
        require(identity(build['binary']['path']) == build['binary'], f'Binary bytes changed: {profile}')
        require(os.access(build['binary']['path'], os.X_OK), f'Binary is not executable: {profile}')
        receipt = read(build['build_receipt'])
        require(receipt['resource'] == 'build' and receipt['exit'] == 0 and receipt['held_whole_run'], f'Incomplete build receipt: {profile}')
        require(receipt['acquired_at_ms'] <= build['start_unix'] * 1000 and receipt['released_at_ms'] >= build['end_unix'] * 1000, f'Build receipt does not cover the recorded build: {profile}')
        require(Path(build['build_log']).is_file(), f'Missing build log: {profile}')
    return binding


def environment(binding, source, quality):
    env = os.environ.copy()
    removed = sorted(key for key in env if key.startswith('VERSE_'))
    for key in removed:
        env.pop(key)
    env.update(binding.get('environment', {}))
    env.update(VERSE_QUALITY=quality, VERSE_KIT_PACK=binding['assets']['kit']['path'], OPENAGENTS_CAPTURE_SOURCE_COMMIT=source)
    recorded = {key: env.get(key) for key in ['VERSE_QUALITY', 'VERSE_KIT_PACK', 'VERSE_KIT_BAKE', 'VERSE_HOME', 'VERSE_KIT_UNPINNED', 'VERSE_TEMPORAL_AA', 'OPENAGENTS_CAPTURE_SOURCE_COMMIT', 'OPENAGENTS_LEASES', 'OPENAGENTS_LEASE_ID', 'OPENAGENTS_SCRATCH', 'WGPU_BACKEND', 'WGPU_POWER_PREF', 'WGPU_TRACE']}
    return env, recorded, removed


def plan(binding, source, binding_path):
    value = read(ROOT / 'bench/verse/2026-10-08/temporal-aa/curation-plan.template.json')
    value.update(source=source, assets=binding['assets'])
    proofs = {f'{STEM}-runner.py': str(Path(__file__).resolve()), f'{STEM}-build-binding.json': str(Path(binding_path).resolve())}
    for index, (label, profile, _, _, _, _, _) in enumerate(CASES):
        paths = case_paths(index)
        value['cases'][label] = {key: paths[key] for key in ['manifest', 'job', 'prefix', 'quiet_receipt', 'gpu_receipt']}
        value['cases'][label].update(profile=profile, binary_sha256=binding['builds'][profile]['binary']['sha256'])
        for key, suffix in [('log', '.log'), ('check', '-check.json')]:
            proofs[paths['job'] + suffix] = paths[key]
    for profile in ['dev', 'release']:
        build = binding['builds'][profile]
        proofs[f'{STEM}-{profile}-build.log'] = build['build_log']
        proofs[f'{STEM}-{profile}-build-lease.json'] = build['build_receipt']
    value.update(proofs=proofs, preparation_status='pending_captures_and_case_receipts', acceptance={'visual': None, 'timing': None, 'promotion_authorized': False})
    return value


def check_report(index, report):
    label, _, quality, seconds, sequence, camera, rebuild = CASES[index]
    frames = seconds * 60
    require((report['width'], report['height'], report['fps']) == (1920, 1080, 60), f'Unexpected dimensions: {label}')
    require(report['effective_quality'] == quality and report['temporal_aa_enabled'], f'Unexpected quality/AA: {label}')
    require(report['mode'] == 'live' and report['light_settled_before_capture'], f'Unexpected live/light mode: {label}')
    diagnostics = report['temporal_texture_diagnostics']
    require(diagnostics['enabled'] is False and diagnostics['timing_acceptance_available'] is True and diagnostics['frames'] == [], 'Diagnostic snapshots must remain disabled')
    timing = report['submission_timing']
    require(timing['submitted_frames'] == timing['completed_frames'] == frames, 'Missing primary submissions/completions')
    require([row['index'] for row in timing['frames']] == list(range(frames)), 'Primary frame ledger is incomplete or reordered')
    require(sum(phase['frames'] for phase in report['phases'].values()) == frames, 'Phase statistics do not cover all simulation frames')
    require(report['static_houses'] == (camera is not None), 'Static house scenario changed')
    require(report['camera_path'] == (camera or 'director'), 'Camera path changed')
    if camera is None:
        require(report['impact_frame'] == 469 and report['smoke_frame'] == 600, 'Selected impact/smoke frames changed')
    if sequence:
        require(report['sequence_frames'] == list(map(int, sequence.split(':'))), 'Selected paired sequence changed')
        require(timing['policy'] == 'serial_diagnostics', 'Paired frames must use serial completion')
        require(timing['temporal_baseline_submitted_frames'] == timing['temporal_baseline_completed_frames'] == frames, 'Missing baseline frames')
        require(report['rebuild_capture'] is None, 'Paired comparison cannot include R capture')
    else:
        require(timing['policy'] == 'bounded_two_frames', 'Unpaired captures must retain bounded continuous submission')
        restored = report['rebuild_capture']
        require(restored is not None and restored['pristine_frame'] == 0 and restored['restoration_frame'] == frames and restored['after_simulation_frame'] == frames - 1, 'R must follow the last simulation frame')
        require(restored['pristine_view'] == restored['restored_view'], 'R camera changed')
        for key in ['pristine_image', 'restored_image']:
            require((Path(case_paths(index)['prefix']) / restored[key]).is_file(), f'Missing {key}')
    return {'status': 'case_integrity_pass', 'case': label, 'simulation_frames': frames, 'last_simulation_frame': frames - 1, 'phase_statistic_frames': frames, 'submission_policy': timing['policy'], 'rebuild_capture': report['rebuild_capture'], 'timing_scope': 'The capture records the final primary drain before deferred artifact writes and R. R is an additional artifact render after the last simulation frame; it does not enter the primary frame ledger or phase statistics. The command wall interval includes setup, artifact writes, and R.', 'warning_diagnostics': None, 'acceptance': {'visual': None, 'timing': None, 'promotion_authorized': False}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, help='Exact source SHA recorded by the build, never inferred from HEAD')
    parser.add_argument('--case-index', required=True, type=int, choices=range(len(CASES)))
    parser.add_argument('--build-binding', required=True, type=Path)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--run', action='store_true', help='Run this one case inside an existing GPU outer / quiet inner lease')
    modes.add_argument('--write-plan', action='store_true', help='Write the six-case pending curation plan in scratch; do not run a capture')
    modes.add_argument('--finish', action='store_true', help='Verify the completed case and both finalized lease receipts')
    modes.add_argument('--dry-run', action='store_true', help='Print the exact case specification; the default mode')
    args = parser.parse_args()
    require(args.case_index == 2, "Only the preregistered High paired case may be replicated")
    require(args.source == "834aed6cb0b79c80fbd009c3caa51c78dba06628", "Replication source changed")
    _, profile, quality, _, _, _, _ = CASES[args.case_index]
    required_profiles = ['dev', 'release'] if args.write_plan else [profile]
    binding = verify_binding(read(args.build_binding), args.source, required_profiles)
    if args.write_plan:
        output = SCRATCH / (STEM + '-curation-plan.json')
        write(output, plan(binding, args.source, args.build_binding))
        print(output)
        return
    paths = case_paths(args.case_index)
    build = binding['builds'][profile]
    env, recorded_env, removed_env = environment(binding, args.source, quality)
    command = capture_argv(args.case_index, build['binary']['path'])
    if args.finish:
        row = read(paths['manifest'])
        require(row['source'] == args.source and row['command'] == command and row['exit'] == 0, 'Incomplete or differently bound capture')
        require(row['binary_sha256'] == build['binary']['sha256'], 'Captured binary differs from the build binding')
        require('kit: licensed' in Path(paths['log']).read_text(errors='replace').splitlines(), 'The capture did not report the licensed kit')
        for resource in ['gpu', 'quiet']:
            receipt = read(paths[resource + '_receipt'])
            require(receipt['resource'] == resource and receipt['exit'] == 0 and receipt['held_whole_run'] and receipt['nested'] is False, f'Incomplete independent {resource} receipt')
            require(receipt['acquired_at_ms'] <= row['start_unix'] * 1000 and receipt['released_at_ms'] >= row['end_unix'] * 1000, f'{resource} receipt does not cover capture boundaries')
        check = check_report(args.case_index, read(Path(paths['prefix']) / 'capture.json'))
        check.update(source=args.source, binary_sha256=row['binary_sha256'], warning_diagnostics=row['warning_diagnostics'], leases={resource: identity(paths[resource + '_receipt']) for resource in ['gpu', 'quiet']}, manifest=identity(paths['manifest']), report=identity(Path(paths['prefix']) / 'capture.json'))
        write(paths['check'], check)
        print(paths['check'])
        return
    row = {**paths, 'schema': 'openagents.verse.capture-job.v1', 'name': paths['job'], 'source': args.source, 'source_binding_method': 'Explicit CLI source and root-supplied build binding; no ambient HEAD lookup', 'profile': profile, 'quality': quality, 'features': ['capture'], 'binary': build['binary']['path'], 'binary_sha256': build['binary']['sha256'], 'binary_bytes': build['binary']['bytes'], 'build_binding': identity(args.build_binding), 'build_command': build['command'], 'build_receipt': identity(build['build_receipt']), 'build_log': identity(build['build_log']), 'command': command, 'cwd': str(ROOT), 'environment': recorded_env, 'removed_inherited_verse_environment_names': removed_env, 'assets': binding['assets'], 'warning_diagnostics': None, 'warning_scan_pattern': WARNING_PATTERN, 'start_unix': None, 'end_unix': None, 'exit': None, 'acceptance': {'visual': None, 'timing': None, 'promotion_authorized': False}}
    row["replication_plan"] = identity(SCRATCH / "10936-final-resolve-replication-plan.json")
    if not args.run:
        print(json.dumps(row, indent=2))
        return
    require(set(env.get('OPENAGENTS_LEASES', '').split(',')) >= {'gpu', 'quiet'}, 'Run one case under GPU outer / quiet inner leases')
    require(all(not Path(paths[key]).exists() for key in ['prefix', 'manifest', 'log', 'check', 'gpu_receipt', 'quiet_receipt']), 'Refuse to overwrite an existing case or receipt; retain previous attempts separately')
    row['start_unix'] = time.time()
    write(paths['manifest'], row)
    with Path(paths['log']).open('x') as log:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    row.update(exit=result.returncode, end_unix=time.time())
    matched = [line for line in Path(paths['log']).read_text(errors='replace').splitlines() if re.search(WARNING_PATTERN, line)]
    row['warning_diagnostics'] = matched or None
    write(paths['manifest'], row)
    if result.returncode == 0:
        require('kit: licensed' in Path(paths['log']).read_text(errors='replace').splitlines(), 'The capture did not report the licensed kit')
        check = check_report(args.case_index, read(Path(paths['prefix']) / 'capture.json'))
        check.update(status='capture_integrity_pass_receipts_pending', source=args.source, binary_sha256=row['binary_sha256'], warning_diagnostics=row['warning_diagnostics'])
        write(paths['check'], check)
    print(paths['job'] + ' exit ' + str(result.returncode), flush=True)
    raise SystemExit(result.returncode)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, KeyError, OSError, TypeError) as error:
        print('Binding/case check failed: ' + str(error), file=sys.stderr)
        raise SystemExit(2)

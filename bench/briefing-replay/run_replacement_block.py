#!/usr/bin/env python3
"""Register and run a separate replacement for factorial block 4."""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import time

import run_factorial as coordinator

SCHEMA = 'openagents.briefing.replacement-block.v1'
ORDINALS = [13, 14, 15, 16]
ARMS = ['D', 'A', 'C', 'B']
ENVIRONMENT = {'DISABLE_AUTOUPDATER': '1'}


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False) + '\n').encode()


def file_record(path, name=None):
    path = Path(path)
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError('A registered file must be regular, without a symlink')
    digest = hashlib.sha256()
    count = 0
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
            count += len(chunk)
    if count != metadata.st_size:
        raise ValueError('A registered file changed while it was being read')
    return {'path': name if name is not None else str(path), 'bytes': count, 'sha256': digest.hexdigest()}


def absolute(path):
    path = Path(os.path.abspath(path))
    if path != path.resolve():
        raise ValueError('Registered paths must not contain symlinks')
    return path


def source_roots():
    return ['plan.json', 'schedule.json', 'harness/replay.py', 'harness/instruction_guard.py',
            'warmups/opus', 'warmups/sonnet',
            *('runs/' + row['label'] for row in coordinator.schedule()[:12])]


def manifest(study):
    study = absolute(study)
    records = []
    def visit(path):
        path = absolute(path)
        metadata = path.lstat()
        if stat.S_ISDIR(metadata.st_mode):
            for child in sorted(path.iterdir()):
                visit(child)
        else:
            records.append(file_record(path, path.relative_to(study).as_posix()))
    for name in source_roots():
        visit(study / name)
    return sorted(records, key=lambda item: item['path'])


def executable_record(path):
    path = absolute(path)
    metadata = path.lstat()
    if path.name != 'claude' or not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
        raise ValueError('The pinned executable must be a separate regular file named claude')
    if metadata.st_mode & 0o222 or not metadata.st_mode & 0o111:
        raise ValueError('The pinned executable must be executable and have no write permission bits')
    return {**file_record(path), 'version': coordinator.CLAUDE_CLI_VERSION}


def check_source(plan_path, study):
    plan = json.loads(coordinator.read(plan_path))
    coordinator.validate_plan(plan)
    frozen = canonical(plan)
    digest = coordinator.sha(frozen)
    if coordinator.read(study / 'plan.json') != frozen:
        raise ValueError('The original study must retain the exact canonical registered plan')
    expected = {'plan_sha256': digest, 'runs': coordinator.schedule()}
    if json.loads(coordinator.read(study / 'schedule.json')) != expected:
        raise ValueError('The original schedule differs from the registered order')
    for key, name in [('runner', 'replay.py'), ('instruction_guard', 'instruction_guard.py')]:
        if file_record(study / 'harness' / name)['sha256'] != plan[key]['sha256']:
            raise ValueError('The original harness differs from the registered plan')
    for row in coordinator.schedule()[:12]:
        run = study / 'runs' / row['label']
        arm = json.loads(coordinator.read(run / 'arm-result.json'))
        expected = {**row, 'plan_sha256': digest,
                    'runner_result_sha256': file_record(run / 'result.json')['sha256']}
        if any(arm.get(key) != value for key, value in expected.items()):
            raise ValueError('An original retained arm differs from its plan or result')
    for family in coordinator.MODELS:
        file_record(study / 'warmups' / family / 'result.json')
    return plan


def registration(plan_path, study, output, executable, bound_files=()):
    plan_path, study, output = map(absolute, (plan_path, study, output))
    if output.exists() or output.is_symlink():
        raise ValueError('The replacement output must not already exist')
    if study == output or study in output.parents or output in study.parents:
        raise ValueError('The replacement output must be separate from the original study')
    plan = check_source(plan_path, study)
    coordinator_path = absolute(coordinator.__file__)
    helper_path = absolute(__file__)
    if len(bound_files) > 64 or len(set(map(str, map(absolute, bound_files)))) != len(bound_files):
        raise ValueError('Bind at most 64 distinct additional input files')
    additional = sorted((file_record(absolute(path)) for path in bound_files), key=lambda item: item['path'])
    return {'schema': SCHEMA, 'block': 4, 'ordinals': ORDINALS, 'arms': ARMS,
            'original_plan': {**file_record(plan_path), 'canonical_sha256': coordinator.sha(canonical(plan))},
            'original_study': str(study), 'output': str(output), 'copied_files': manifest(study),
            'executable': executable_record(executable), 'coordinator': file_record(coordinator_path),
            'helper_sha256': file_record(helper_path)['sha256'], 'environment': ENVIRONMENT,
            'additional_inputs': additional}


def validate(record, output_may_exist=False):
    keys = {'schema', 'block', 'ordinals', 'arms', 'original_plan', 'original_study', 'output',
            'copied_files', 'executable', 'coordinator', 'helper_sha256', 'environment', 'additional_inputs'}
    if set(record) != keys or record['schema'] != SCHEMA or record['block'] != 4:
        raise ValueError('Unknown or missing replacement registration fields')
    if record['environment'] != ENVIRONMENT:
        raise ValueError('The registered updater environment must match exactly')
    if record['ordinals'] != ORDINALS or record['arms'] != ARMS:
        raise ValueError('Only the complete registered block 4 order D,A,C,B may be replaced')
    if [(row['ordinal'], row['arm']) for row in coordinator.schedule()[12:]] != list(zip(ORDINALS, ARMS)):
        raise ValueError('The coordinator block order changed')
    study, output = absolute(record['original_study']), absolute(record['output'])
    if study == output or study in output.parents or output in study.parents:
        raise ValueError('The replacement output must be separate from the original study')
    if not output_may_exist and (output.exists() or output.is_symlink()):
        raise ValueError('The replacement output must not already exist')
    if record['helper_sha256'] != file_record(absolute(__file__))['sha256']:
        raise ValueError('The helper differs from its registration')
    if record['coordinator'] != file_record(absolute(coordinator.__file__)):
        raise ValueError('The coordinator differs from its registration')
    additional = record['additional_inputs']
    if len(additional) > 64 or additional != sorted(
            (file_record(absolute(item['path'])) for item in additional), key=lambda item: item['path']):
        raise ValueError('An additional registered input differs from its registration')
    if len({item['path'] for item in additional}) != len(additional):
        raise ValueError('Additional registered inputs must be distinct')
    bound = record['original_plan']
    plan_path = absolute(bound['path'])
    plan = check_source(plan_path, study)
    if bound != {**file_record(plan_path), 'canonical_sha256': coordinator.sha(canonical(plan))}:
        raise ValueError('The original plan differs from its registration')
    if record['copied_files'] != manifest(study):
        raise ValueError('The original copied-file manifest differs from its registration')
    if record['executable'] != executable_record(record['executable']['path']):
        raise ValueError('The pinned executable differs from its registration')
    return plan


def save_new(path, data):
    with Path(path).open('xb') as handle:
        handle.write(data)


def copy_registered(record, output):
    source = Path(record['original_study'])
    for item in record['copied_files']:
        relative = item['path']
        destination = output / relative
        destination.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
        with (source / relative).open('rb') as reader, destination.open('xb') as writer:
            shutil.copyfileobj(reader, writer, 1024 * 1024)
        if file_record(destination, relative) != item:
            raise ValueError('A copied artifact differs from its registration')
    check_copies(record, output)


def check_copies(record, output):
    if manifest(output) != record['copied_files']:
        raise ValueError('The copied artifact manifest differs from its registration')


@contextlib.contextmanager
def pinned_environment(executable):
    keys = ['PATH', *ENVIRONMENT]
    original = {key: os.environ.get(key) for key in keys}
    os.environ['PATH'] = str(Path(executable).parent) + (os.pathsep + original['PATH'] if original['PATH'] else '')
    os.environ.update(ENVIRONMENT)
    try:
        if shutil.which('claude') != executable:
            raise ValueError('PATH does not resolve claude to the pinned executable')
        yield
    finally:
        for key, value in original.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def check_binary(bound):
    if executable_record(bound['path']) != bound:
        raise ValueError('The pinned executable differs from its registration')
    result = subprocess.run([bound['path'], '--version'], stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, check=True, timeout=30)
    version = result.stdout.decode('utf-8').strip()
    if version not in (bound['version'], bound['version'] + ' (Claude Code)'):
        raise ValueError('The pinned executable reports a different CLI version')
    if executable_record(bound['path']) != bound:
        raise ValueError('The pinned executable changed during its version check')
    return {'sha256': bound['sha256'], 'version': bound['version']}


def execute(record, registration_bytes):
    started = time.monotonic()
    if json.loads(registration_bytes) != record:
        raise ValueError('The lineage bytes differ from the registration')
    plan = validate(record)
    output = Path(record['output'])
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    save_new(output / 'replacement-registration.json', registration_bytes)
    copy_registered(record, output)
    setup_elapsed = time.monotonic() - started
    completed, arm_timings = [], []
    events = output / 'replacement-events.jsonl'
    def event(value):
        with events.open('ab') as handle:
            handle.write(canonical(value))
    event({'phase': 'setup', 'elapsed_s': setup_elapsed})
    def verify_binary(ordinal, phase):
        check_started = time.monotonic()
        try:
            binary = check_binary(record['executable'])
        except Exception:
            event({'ordinal': ordinal, 'phase': phase, 'binary_binding_ok': False,
                   'verification_elapsed_s': time.monotonic() - check_started})
            raise
        elapsed = time.monotonic() - check_started
        event({'ordinal': ordinal, 'phase': phase, 'binary_binding_ok': True,
               'binary': binary, 'verification_elapsed_s': elapsed})
        return elapsed
    with pinned_environment(record['executable']['path']):
        for ordinal in ORDINALS:
            preflight_started = time.monotonic()
            validate(record, output_may_exist=True)
            check_copies(record, output)
            row = coordinator.schedule()[ordinal - 1]
            if (output / 'runs' / row['label']).exists():
                raise ValueError('A replacement run already exists; it will not be repeated')
            before_elapsed = verify_binary(ordinal, 'before')
            preflight_elapsed = time.monotonic() - preflight_started
            execution_started = time.monotonic()
            try:
                coordinator.execute(plan, output, first=ordinal, last=ordinal)
            finally:
                coordinator_elapsed = time.monotonic() - execution_started
                after_elapsed = verify_binary(ordinal, 'after')
            arm_timings.append({'ordinal': ordinal, 'preflight_elapsed_s': preflight_elapsed,
                                'before_binary_verification_elapsed_s': before_elapsed,
                                'after_binary_verification_elapsed_s': after_elapsed,
                                'coordinator_elapsed_s': coordinator_elapsed})
            completed.append(ordinal)
        final_started = time.monotonic()
        validate(record, output_may_exist=True)
        check_copies(record, output)
        final_elapsed = time.monotonic() - final_started
    elapsed = time.monotonic() - started
    receipt = {'schema': 'openagents.briefing.replacement-complete.v1',
               'registration_sha256': coordinator.sha(registration_bytes),
               'original_plan_sha256': record['original_plan']['canonical_sha256'],
               'completed_ordinals': completed, 'elapsed_s': elapsed,
               'initial_setup_elapsed_s': setup_elapsed, 'arm_timings': arm_timings,
               'final_validation_elapsed_s': final_elapsed,
               'wrapper_overhead_elapsed_s': elapsed - sum(item['coordinator_elapsed_s'] for item in arm_timings),
               'timing_note': 'Wrapper overhead is separate from the unchanged runner endpoint. Preflight includes the before-binary check. Elapsed time excludes completion-receipt serialization.'}
    save_new(output / 'replacement-complete.json', canonical(receipt))
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--registration', type=Path, required=True)
    parser.add_argument('--register', action='store_true')
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--plan', type=Path)
    parser.add_argument('--original-study', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--executable', type=Path)
    parser.add_argument('--bind-file', type=Path, action='append', default=[])
    args = parser.parse_args()
    if args.register:
        if args.execute or any(value is None for value in (args.plan, args.original_study, args.output, args.executable)):
            parser.error('--register needs --plan, --original-study, --output, and --executable, without --execute')
        value = registration(args.plan, args.original_study, args.output, args.executable, args.bind_file)
        target = absolute(args.registration)
        study, output = Path(value['original_study']), Path(value['output'])
        if study == target or study in target.parents or output == target or output in target.parents:
            parser.error('Store the prospective registration outside both study directories')
        save_new(target, canonical(value))
        print(json.dumps({'registered': True, 'registration_sha256': coordinator.sha(canonical(value))}))
        return
    if args.bind_file or any(value is not None for value in (args.plan, args.original_study, args.output, args.executable)):
        parser.error('Execution and validation use only the bound --registration')
    data = coordinator.read(args.registration)
    value = json.loads(data)
    if args.execute:
        print(json.dumps(execute(value, data)))
    else:
        validate(value)
        print(json.dumps({'valid': True, 'execution': False, 'ordinals': ORDINALS, 'arms': ARMS}))


if __name__ == '__main__':
    main()

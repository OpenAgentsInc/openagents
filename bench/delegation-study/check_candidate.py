#!/usr/bin/env python3
"""Check a retained candidate in a fresh, offline historical-source namespace."""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import time
import traceback
import uuid

import candidate
import run_remote
from seed_manifest import cargo_features, feature_arguments, digest, validate_seed

SCHEMA = 'openagents.delegation.final-checks.v1'
PHASES = ('scope', 'format', 'ordinary', 'independent')


class Invalid(ValueError):
    """A scrubbed validation failure suitable for a public receipt."""


@contextmanager
def interrupt_handlers():
    """Turn external cancellation into normal cleanup and receipt retention."""
    def interrupted(number, frame):
        raise InterruptedError('signal_' + str(number))
    previous = {number: signal.getsignal(number) for number in (signal.SIGTERM, signal.SIGINT)}
    try:
        for number in previous:
            signal.signal(number, interrupted)
        yield
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)


def durable_json(path, value):
    temporary = path.with_suffix('.tmp')
    with temporary.open('w') as handle:
        json.dump(value, handle, sort_keys=True, indent=2)
        handle.write('\n')
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def relative_path(value):
    if not isinstance(value, str) or not value or any(ord(c) < 32 for c in value):
        raise Invalid('invalid_relative_path')
    path = PurePosixPath(value)
    if path.is_absolute() or '..' in path.parts or '.git' in path.parts or path.as_posix() != value or value == '.':
        raise Invalid('invalid_relative_path')
    return path


def initial(config):
    row = {'schema': SCHEMA, 'run_id': config.get('run_id'),
            'source_commit': config.get('source_commit'),
            'source_archive_sha256': config.get('source_archive_sha256'),
            'candidate_manifest_sha256': config.get('candidate_manifest_sha256'),
            'checker_sha256': config.get('checker', {}).get('sha256'),
            'target_seed_manifest_sha256': config.get('target_seed_manifest_sha256'),
            'completed': False, 'accepted': False, 'status': 'incomplete', 'execution_closed': False,
            'identity_validated': False, 'scope_validated': False,
            'phase_budget_policy': 'remaining_total_deadline', 'candidate_symlinks': 'rejected',
            'phases': {}, **{p: {'passed': None, 'status': 'not_started'} for p in PHASES}}
    if config.get('cargo_features'):row['cargo_features'] = config['cargo_features']
    return row


def validate_config(config):
    if str(uuid.UUID(config['run_id'])) != config['run_id']:
        raise Invalid('invalid_run_id')
    for key, length in [('source_commit', 40), ('source_archive_sha256', 64),
                        ('candidate_manifest_sha256', 64), ('target_seed_manifest_sha256', 64)]:
        if not re.fullmatch('[0-9a-f]{%d}' % length, config[key]):
            raise Invalid('invalid_digest')
    packages = config['packages']
    if not packages or len(packages) > 16 or any(not re.fullmatch(r'[A-Za-z0-9_-]+', p) for p in packages):
        raise Invalid('invalid_packages')
    if not config['allowed_paths'] or len(config['allowed_paths']) > 64:
        raise Invalid('invalid_allowed_paths')
    for name in config['allowed_paths']:
        relative_path(name.rstrip('/'))
    checker = config['checker']
    injection = relative_path(checker['injection'])
    if injection.suffix != '.rs' or 'tests' not in injection.parts:
        raise Invalid('invalid_checker_injection')
    if checker['package'] not in packages or not re.fullmatch(r'[A-Za-z0-9_-]+', checker['target']):
        raise Invalid('invalid_checker_target')
    # The independent command selects the checker package only.
    cargo_features(config.get('cargo_features',[]),[checker['package']])
    if injection.name != checker['target'] + '.rs' or not re.fullmatch('[0-9a-f]{64}', checker['sha256']):
        raise Invalid('invalid_checker_identity')
    for seconds in (config['total_timeout_s'],):
        if type(seconds) not in (int, float) or not 0 < seconds <= 3600:
            raise Invalid('invalid_timeout')


def safe_path(root, name):
    path = root / relative_path(name)
    for part in (path, *path.parents):
        if part == root:
            break
        if part.is_symlink():
            raise Invalid('candidate_symlink_path')
    return path


def descriptor(path):
    try:
        info = path.lstat()
    except FileNotFoundError:
        return None
    if not stat.S_ISREG(info.st_mode):
        raise Invalid('candidate_nonregular_path')
    return {'kind': 'file', 'mode': stat.S_IMODE(info.st_mode),
            'bytes': info.st_size, 'sha256': digest(path)}


def scoped(changes, allowed, injection):
    for name, change in changes.items():
        relative_path(name)
        if name == injection or not any(name == p.rstrip('/') or name.startswith(p.rstrip('/') + '/') for p in allowed):
            return False
        if any(value is not None and value.get('kind') != 'file' for value in change.values()):
            return False
    return True


def apply_candidate(workspace, candidate_dir, changes):
    # Validate every preimage before applying any deletion or replacement.
    for name, change in changes.items():
        if set(change) != {'before', 'after'} or descriptor(safe_path(workspace, name)) != change['before']:
            raise Invalid('candidate_preimage_mismatch')
    for name in changes:
        path = safe_path(workspace, name)
        if path.exists():
            path.unlink()
    with tarfile.open(candidate_dir / 'candidate.tar.gz', 'r:gz') as archive:
        for member in archive:
            if not member.isfile():
                raise Invalid('candidate_nonregular_payload')
            path = safe_path(workspace, member.name)
            path.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(member) as source, path.open('xb') as destination:
                shutil.copyfileobj(source, destination, 1024 * 1024)
            path.chmod(member.mode)
    for name, change in changes.items():
        if descriptor(safe_path(workspace, name)) != change['after']:
            raise Invalid('candidate_postimage_mismatch')


def copy_seed(source, destination, files):
    """Copy a verified baseline without sharing writable inodes with it."""
    if sys.platform.startswith('linux'):
        subprocess.run(['cp', '-a', '--reflink=auto', str(source), str(destination)],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    else:
        shutil.copytree(source, destination)
    for name in files:
        before, after = (source / name).stat(), (destination / name).stat()
        if (before.st_dev, before.st_ino) == (after.st_dev, after.st_ino):
            raise Invalid('seed_copy_shares_writable_inode')


def namespace(workspace, home, toolchain, checker=None):
    args = run_remote.base_arguments(workspace, home, toolchain)
    # Replace the writable source mount with a read-only mount for all checks.
    index = args.index(str(workspace))
    args[index - 1] = '--ro-bind'
    if checker:
        args += ['--ro-bind', str(checker['path']), '/workspace/' + checker['injection']]
    args += ['--setenv', 'CARGO_TARGET_DIR', '/home/executor/target',
             '--setenv', 'CARGO_INCREMENTAL', '0', '--setenv', 'CARGO_NET_OFFLINE', 'true',
             '--chdir', '/workspace', '--']
    return args


def execute(argv, output, phase, timeout):
    start = time.monotonic()
    timed_out = False
    with (output / (phase + '.stdout')).open('wb') as stdout, (output / (phase + '.stderr')).open('wb') as stderr:
        process = subprocess.Popen(argv, stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            durable_json(output / 'active-process.json', {'pid': process.pid})
            try:
                code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                code = process.wait()
        finally:
            # Namespace exit kills its descendants; also close the host group.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            (output / 'active-process.json').unlink(missing_ok=True)
    return {'passed': code == 0 and not timed_out,
            'status': 'timeout' if timed_out else ('passed' if code == 0 else 'failed'),
            'exit_code': code, 'wall_s': time.monotonic() - start}


def budget_failure(row, reason):
    for phase in PHASES:
        if row[phase]['passed'] is None:
            row[phase].update(passed=False, status=reason)
    active = row.get('active_check', {})
    if active.get('part') and active['phase'] in PHASES:
        row[active['phase']].setdefault(active['part'], {'passed': False, 'status': reason})
    row.update(completed=bool(row['identity_validated'] and row['scope_validated']),
               accepted=False, status=reason)


def check(config, output, execute_command=execute):
    """Trusted worker. Tests inject command results; production uses namespaces."""
    start = time.monotonic()
    deadline = start + config['total_timeout_s']
    row = initial(config)
    save = lambda: durable_json(output / 'checks.json', row)
    try:
        validate_config(config)
        features=cargo_features(config.get('cargo_features',[]),config['packages'])
        checker = config['checker']
        archive = Path(config['source_archive'])
        if digest(archive) != config['source_archive_sha256']:
            raise Invalid('source_archive_changed')
        candidate_dir = Path(config['candidate_dir'])
        value = candidate.validate(candidate_dir, config['candidate_manifest_sha256'],
                                   config['source_commit'], config['source_archive_sha256'], deadline)
        changes = value['changes']
        row['identity_validated'] = True
        row['scope'] = {'passed': scoped(changes, config['allowed_paths'], checker['injection']),
                        'status': 'passed', 'wall_s': time.monotonic() - start}
        row['scope_validated'] = True
        if not row['scope']['passed']:
            row['scope']['status'] = 'prohibited_scope'
            budget_failure(row, 'not_run_prohibited_scope')
            return row
        save()
        checker_path = Path(checker['path'])
        if checker_path.is_symlink() or not checker_path.is_file() or digest(checker_path) != checker['sha256']:
            raise Invalid('checker_changed')
        seed = Path(config['target_seed'])
        operation = time.monotonic()
        seed_value = validate_seed(seed, config['target_seed_manifest_sha256'],
                                   config['source_commit'], config['source_archive_sha256'],
                                   build_environment=config['toolchain'].get('environment', {}),features=features)
        row['phases']['seed_validation_s'] = time.monotonic() - operation
        row['seed_policy'] = seed_value['seed_policy']
        if not set(config['packages']).issubset(set(seed_value.get('packages', []))):
            raise Invalid('seed_package_mismatch')
        workspace, home = output / 'workspace', output / 'home'
        workspace.mkdir(); home.mkdir(); (home / '.cargo').mkdir()
        operation = time.monotonic()
        run_remote.export(archive, workspace)
        row['phases']['export_s'] = time.monotonic() - operation
        operation = time.monotonic()
        row['snapshot_commit'] = run_remote.initialize_snapshot(workspace, home, config['source_commit'])
        row['phases']['git_snapshot_s'] = time.monotonic() - operation
        if config.get('expected_snapshot_commit') is not None and row['snapshot_commit'] != config['expected_snapshot_commit']:
            raise Invalid('snapshot_commit_changed')
        operation = time.monotonic()
        apply_candidate(workspace, candidate_dir, changes)
        row['phases']['apply_candidate_s'] = time.monotonic() - operation
        # Copy only the verified baseline cache. Never read the executor target.
        operation = time.monotonic()
        copy_seed(seed / 'target', home / 'target', seed_value['files'])
        row['phases']['seed_copy_s'] = time.monotonic() - operation
        # Unchanged units come from this exact verified baseline, so retain
        # their source mtimes. Refresh all surviving candidate changes, including
        # non-Rust include inputs, and the independently injected checker.
        refreshed = time.time_ns()
        for name, change in changes.items():
            if change['after'] is not None:
                os.utime(safe_path(workspace, name), ns=(refreshed, refreshed))
        row['candidate_mtimes_refreshed_unix_ns'] = refreshed
        row['source_mtime_policy'] = 'changed_and_injected_only_verified_base_seed'
        frozen_checker = output / 'checker.rs'
        shutil.copyfile(checker_path, frozen_checker)
        frozen_checker.chmod(0o400)
        os.utime(frozen_checker, ns=(refreshed, refreshed))
        checked = dict(checker, path=str(frozen_checker))
        injection = safe_path(workspace, checker['injection'])
        if injection.exists():
            raise Invalid('checker_injection_collision')
        injection.parent.mkdir(parents=True, exist_ok=True)
        # Mount point is present only for independent checking; ordinary Cargo
        # runs before it is created so it cannot discover an empty test target.
        base = namespace(workspace, home, config['toolchain'])
        version_commands = [('cargo', ['cargo', '--version']), ('rustc', ['rustc', '-Vv'])]
        if 'rustdoc' in seed_value.get('toolchain', {}):
            rustdoc = config['toolchain'].get('environment', {}).get('RUSTDOC')
            if not rustdoc or not Path(rustdoc).is_absolute():
                raise Invalid('rustdoc_path_missing')
            version_commands.append(('rustdoc', [rustdoc, '-Vv']))
        for name, argv in version_commands:
            result = execute_command(base + argv, output, 'version-' + name, max(.001, deadline - time.monotonic()))
            observed = (output / ('version-' + name + '.stdout')).read_text().strip()
            if not result['passed'] or observed != seed_value.get('toolchain', {}).get(name):
                raise Invalid('toolchain_changed')
        row['toolchain'] = seed_value['toolchain']
        row['phases']['setup_s'] = time.monotonic() - start
        package_args = [arg for name in config['packages'] for arg in ('-p', name)]
        if time.monotonic() >= deadline:
            budget_failure(row, 'not_run_after_deadline'); return row
        row['format'] = {'passed': None, 'status': 'running'}; save()
        row['format'] = execute_command(base + ['cargo', 'fmt'] + package_args + ['--', '--check'],
                                        output, 'format', deadline - time.monotonic()); save()
        if row['format']['status'] == 'timeout':
            budget_failure(row, 'not_run_after_deadline'); return row
        for phase in ('ordinary', 'independent'):
            if phase == 'independent':
                injection.touch(mode=0o400)
                command_base = namespace(workspace, home, config['toolchain'], checked)
                command = ['cargo', 'test', '--locked', '--offline', '-p', checker['package'], '--test', checker['target']]
                test_arguments = ['--', '--test-threads=1']
            else:
                command_base = base
                command = ['cargo', 'test', '--locked', '--offline'] + package_args
                test_arguments = []
            command += feature_arguments(features)
            row[phase] = {'passed': None, 'status': 'running'}; save()
            phase_start = time.monotonic()
            for part, arguments in [('compile', ['--no-run']), ('test', test_arguments)]:
                if time.monotonic() >= deadline:
                    budget_failure(row, 'not_run_after_deadline'); return row
                row['active_check'] = {'phase': phase, 'part': part}; save()
                result = execute_command(command_base + command + arguments, output,
                                         phase + '-' + part, deadline - time.monotonic())
                row[phase][part] = result
                row.pop('active_check', None)
                save()
                if not result['passed']:
                    row[phase].update(passed=False, status=result['status'], wall_s=time.monotonic() - phase_start)
                    if part == 'compile':
                        row[phase]['test'] = {'passed': False, 'status': 'not_run_after_compile_failure'}
                    break
            else:
                row[phase].update(passed=True, status='passed', wall_s=time.monotonic() - phase_start)
            save()
            if row[phase]['status'] == 'timeout':
                budget_failure(row, 'not_run_after_deadline'); return row
        row.update(completed=True, accepted=all(row[p]['passed'] is True for p in PHASES), status='complete')
    except Exception as error:
        with (output / 'infrastructure-error.log').open('w') as log:
            traceback.print_exc(file=log)
        row.update(completed=False, accepted=False, status='infrastructure_error',
                   error_code=str(error) if isinstance(error, Invalid) else type(error).__name__)
    finally:
        row['wall_s'] = time.monotonic() - start
        save()
    return row


def stop_worker(process, output):
    # Stop the namespace group first, then its worker. No result is finalized
    # while an observed checking process is still able to write its scratch home.
    path = output / 'active-process.json'
    if path.exists():
        try:
            os.killpg(json.loads(path.read_text())['pid'], signal.SIGKILL)
        except ProcessLookupError:
            pass
        path.unlink(missing_ok=True)
    if process.poll() is None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait()


def _run(config, output):
    """Run once under a total wall-time watchdog and retain a durable receipt."""
    began = time.monotonic()
    output = Path(output).resolve()
    os.umask(0o077)
    output.mkdir(parents=True, exist_ok=False)
    row = initial(config)
    process = None
    try:
        validate_config(config)
        config_path = output / 'private-config.json'
        durable_json(config_path, config)
        with (output / 'worker.stdout').open('wb') as stdout, (output / 'worker.stderr').open('wb') as stderr:
            process = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), '--worker', str(config_path), str(output)],
                                       stdout=stdout, stderr=stderr, start_new_session=True)
            try:
                process.wait(timeout=max(.001, config['total_timeout_s'] - (time.monotonic() - began)))
            except subprocess.TimeoutExpired:
                stop_worker(process, output)
                if (output / 'checks.json').exists():
                    row = json.loads((output / 'checks.json').read_text())
                budget_failure(row, 'total_deadline')
            else:
                if (output / 'checks.json').exists():
                    row = json.loads((output / 'checks.json').read_text())
                if process.returncode != 0 and row.get('status') != 'infrastructure_error':
                    row.update(completed=False, accepted=False, status='infrastructure_error', error_code='worker_exit')
    except Exception as error:
        # Stop the writer before reading its last durable progress checkpoint.
        if process is not None:
            try:
                stop_worker(process, output)
            except Exception:
                pass  # The final cleanup attempt records any remaining failure.
        if (output / 'checks.json').exists():
            try:
                saved = json.loads((output / 'checks.json').read_text())
                if (saved.get('run_id') == config['run_id'] and
                        saved.get('candidate_manifest_sha256') == config['candidate_manifest_sha256']):
                    row = saved
            except (OSError, ValueError, AttributeError):
                row['checkpoint_unreadable'] = True
        with (output / 'infrastructure-error.log').open('w') as log:
            traceback.print_exc(file=log)
        row.update(completed=False, accepted=False,
                   status='interrupted' if isinstance(error, InterruptedError) else 'infrastructure_error',
                   error_code=str(error) if isinstance(error, Invalid) else type(error).__name__)
    finally:
        try:
            if process is not None:
                stop_worker(process, output)
            row['execution_closed'] = True
        except Exception as error:
            row.update(completed=False, accepted=False, status='infrastructure_error',
                       execution_closed=False, error_code='cleanup_' + type(error).__name__)
        row['total_wall_s'] = time.monotonic() - began
        durable_json(output / 'checks.json', row)
    return row


def run(config, output):
    """Run one checked attempt with bounded execution and signal cleanup."""
    with interrupt_handlers():
        return _run(config, output)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--worker', action='store_true')
    parser.add_argument('config'); parser.add_argument('output')
    args = parser.parse_args()
    config = json.loads(Path(args.config).read_text())
    with interrupt_handlers():
        result = check(config, Path(args.output)) if args.worker else run(config, Path(args.output))
    print(json.dumps(result, sort_keys=True))
    return 0 if result['completed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())

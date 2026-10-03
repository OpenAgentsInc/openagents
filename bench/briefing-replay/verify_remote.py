#!/usr/bin/env python3
"""Verify one replay candidate inside the dedicated Linux benchmark sandbox."""
import base64
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import signal
import subprocess
import sys
import time


def relative_path(value):
    path = PurePosixPath(value)
    if not value or value == '.' or path.is_absolute() or '..' in path.parts or str(path) != value:
        raise ValueError('Paths must be normalized repository-relative paths')
    return value


def inside(repo, relative):
    path = repo
    for part in PurePosixPath(relative_path(relative)).parts:
        path = path / part
        if path.is_symlink():
            raise ValueError('Benchmark paths must not traverse symlinks')
    return path


def export(source, base, repo, roots=()):
    archive = subprocess.Popen(['git', '-C', str(source), 'archive', base, *roots], stdout=subprocess.PIPE)
    try:
        extracted = subprocess.run(['tar', '-xf', '-', '-C', str(repo)], stdin=archive.stdout)
    finally:
        archive.stdout.close()
    if archive.wait() or extracted.returncode:
        raise RuntimeError('Historical export failed')


@contextmanager
def pinned_repo(request):
    """Hold an exclusive lease over a source-pinned, benchmark-owned tree."""
    config = request['config']
    root = Path(request['remote_root']).resolve()
    root.mkdir(parents=True, exist_ok=True)
    fd = os.open(root / 'verify.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'r+') as lease:
        try:
            fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError('Another verification holds this benchmark root') from error
        source = Path(config['remote_source_repo']).resolve(strict=True)
        base = config['source_commit']
        if len(base) not in (40, 64) or any(char not in '0123456789abcdef' for char in base):
            raise ValueError('Use a full lowercase source commit')
        resolved = subprocess.check_output(['git', '-C', str(source), 'rev-parse', '--verify', base + '^{commit}'], text=True).strip()
        if resolved != base:
            raise ValueError('The declared source does not resolve to the pinned commit')
        roots = config['allowed_roots']
        if not roots or len(set(roots)) != len(roots):
            raise ValueError('Supply distinct allowed roots')
        for relative in roots:
            relative_path(relative)
        pin = {'schema': 'openagents.briefing.remote-source.v1', 'source_commit': base, 'source_repo': str(source), 'allowed_roots': sorted(roots)}
        repo, pin_path = root / 'repo', root / 'source-pin.json'
        if repo.is_symlink() or pin_path.is_symlink():
            raise ValueError('The benchmark repository and source pin must not be symlinks')
        if repo.exists() or pin_path.exists():
            if not repo.is_dir() or not pin_path.is_file() or json.loads(pin_path.read_text()) != pin:
                raise ValueError('Remote source pin is missing or changed; use a fresh benchmark root')
        else:
            repo.mkdir()
            export(source, base, repo)
            # An interrupted export leaves an unpinned tree, which the next run refuses.
            with pin_path.open('x') as handle:
                handle.write(json.dumps(pin, indent=2) + '\n')
        yield root, repo, source, base, lease.fileno()


def verify(request, root, repo, source, base, lease_fd):
    config = request['config']
    run_id = request['run_id']
    if not run_id or any(char not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for char in run_id):
        raise ValueError('Invalid run identity')
    if type(request['attempt']) is not int or request['attempt'] < 1:
        raise ValueError('Invalid attempt number')
    if hashlib.sha256(json.dumps(request['changes'], sort_keys=True).encode()).hexdigest() != request['candidate_digest']:
        raise ValueError('Candidate digest does not match its files')
    destination = root / 'results' / (run_id + '-' + str(request['attempt']) + '.json')
    if Path(request['result_path']).resolve() != destination:
        raise ValueError('Result destination does not match the request identity')
    attempt = root / 'checks' / run_id / str(request['attempt'])
    attempt.mkdir(parents=True, exist_ok=False)
    # Only benchmark-owned trees are reset. No owner checkout is touched.
    for relative in config['allowed_roots']:
        path = inside(repo, relative)
        if path.exists():
            shutil.rmtree(path) if path.is_dir() else path.unlink()
    export(source, base, repo, config['allowed_roots'])
    for change in request['changes']:
        relative = relative_path(change['path'])
        if not any(relative == allowed or relative.startswith(allowed.rstrip('/') + '/') for allowed in config['allowed_roots']):
            raise ValueError('Candidate file is outside the allowed roots')
        path = inside(repo, relative)
        if change['content'] is None:
            path.unlink(missing_ok=True)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(base64.b64decode(change['content'], validate=True))
    env = os.environ.copy()
    env['CARGO_HOME'] = env.get('CARGO_HOME', str(Path.home() / '.cargo'))
    env['RUSTUP_HOME'] = env.get('RUSTUP_HOME', str(Path.home() / '.rustup'))
    home = attempt / 'home'
    home.mkdir()
    env.update(HOME=str(home), CARGO_TARGET_DIR=config['target_dir'], CARGO_INCREMENTAL='0', GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_TERMINAL_PROMPT='0')
    for key in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_COMMON_DIR'):
        env.pop(key, None)
    outcomes = []

    def run(name, command, cwd=repo):
        start = time.monotonic()
        log_path = attempt / (name + '.log')
        with log_path.open('wb') as log:
            # A surviving check keeps the lease if the verifier is interrupted.
            process = subprocess.Popen(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True, pass_fds=(lease_fd,))
            timed_out = False
            try:
                code = process.wait(timeout=300)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                code = 124
        text = log_path.read_text(errors='replace')
        outcomes.append({'name': name, 'command': command, 'exit_code': code, 'timed_out': timed_out, 'wall_s': time.monotonic() - start, 'log': text})
        return code

    # Formatting is a symmetric deterministic service, not a model repair.
    run('initial-format', ['cargo', 'fmt', '-p', config['package'], '--', '--check'])
    run('autoformat', ['cargo', 'fmt', '-p', config['package']])
    run('format', ['cargo', 'fmt', '-p', config['package'], '--', '--check'])
    run('own-tests', ['cargo', 'test', '-p', config['package'], '--offline'])
    harness = attempt / 'harness'
    harness.mkdir()
    (harness / 'Cargo.toml').write_text(config['checker_manifest'].replace('{REPO}', str(repo)).replace('{CHECKER}', config['remote_checker']))
    shutil.copyfile(repo / 'Cargo.lock', harness / 'Cargo.lock')
    run('acceptance', ['cargo', '+1.97.1', 'test', '--manifest-path', str(harness / 'Cargo.toml'), '--offline', '--test', 'acceptance', '--', '--test-threads=1'], harness)
    # Return formatted files; the model is idle while its candidate is checked.
    files = []
    for relative in config['allowed_roots']:
        path = repo / relative
        candidates = path.rglob('*') if path.is_dir() else [path]
        for item in candidates:
            if item.is_file() and not item.is_symlink() and item.suffix == '.rs':
                files.append({'path': item.relative_to(repo).as_posix(), 'content': base64.b64encode(item.read_bytes()).decode()})
    required = [row for row in outcomes if row['name'] != 'initial-format']
    result = {'run_id': request['run_id'], 'attempt': request['attempt'], 'source_commit': base, 'candidate_digest': request['candidate_digest'], 'checks': outcomes, 'passed': all(row['exit_code'] == 0 for row in required), 'formatted_files': files}
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open('x') as handle:
        handle.write(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'result_path': str(destination), 'passed': result['passed'], 'checks': [{k: v for k, v in row.items() if k != 'log'} for row in outcomes]}), flush=True)


def main():
    request = json.loads(Path(sys.argv[1]).read_text())
    with pinned_repo(request) as paths:
        verify(request, *paths)


if __name__ == '__main__':
    main()

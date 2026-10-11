#!/usr/bin/env python3
"""Run matched historical sessions with isolated external checks and one repair."""
import argparse
import base64
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import selectors
import shlex
import shutil
import signal
import subprocess
import time
import urllib.parse
import urllib.request
import uuid

import instruction_guard


MODEL = 'claude-opus-5-5'
TOOLS = ['Read', 'Edit', 'Write', 'Glob', 'Grep']


class ScopeViolation(ValueError):
    """A candidate changed files outside its permitted scope."""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save(path, value):
    temporary = path.with_name(path.name + '.' + uuid.uuid4().hex + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def api(method, path, data=None):
    request = urllib.request.Request(
        os.environ.get('BOAT_API_BASE', 'https://oa-boat-157437760789.us-central1.run.app/api/v1').rstrip('/') + '/' + path,
        data=None if data is None else json.dumps(data).encode(), method=method,
        headers={'Authorization': 'Bearer ' + os.environ['BOAT_API_KEY'], 'Content-Type': 'application/json'},
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def put(sandbox, path, content):
    api('PUT', 'sandboxes/' + sandbox + '/files', {'path': path, 'content': base64.b64encode(content).decode(), 'encoding': 'base64'})


def get(sandbox, path):
    data = api('GET', 'sandboxes/' + sandbox + '/files?' + urllib.parse.urlencode({'path': path, 'encoding': 'base64'}))
    return base64.b64decode(data['content']) if data['encoding'] == 'base64' else data['content'].encode()


def allowed(path, roots):
    parsed = PurePosixPath(path)
    return bool(path and path != '.' and not parsed.is_absolute() and '..' not in parsed.parts and str(parsed) == path and any(path == root or path.startswith(root.rstrip('/') + '/') for root in roots))


def candidate_path(workspace, relative):
    path = workspace
    for part in PurePosixPath(relative).parts:
        path = path / part
        if path.is_symlink():
            raise ScopeViolation('Candidate path traverses a symlink: ' + relative)
    return path


def baseline_map(repository, revision):
    rows = subprocess.check_output(['git', 'ls-tree', '-rz', revision], cwd=repository).split(b'\0')
    result = {}
    for row in rows:
        if row:
            metadata, name = row.split(b'\t', 1)
            mode, kind, oid = metadata.split()
            result[name.decode()] = (mode.decode(), oid.decode())
    return result


def changes(workspace, original, roots):
    result = []
    for relative, (mode, oid) in original.items():
        path = workspace / relative
        content = os.readlink(path).encode() if path.is_symlink() else path.read_bytes() if path.is_file() else None
        actual = None if content is None else hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest()
        if actual != oid:
            if not allowed(relative, roots) or path.is_symlink():
                raise ScopeViolation('Out-of-scope or symlink change: ' + relative)
            candidate_path(workspace, relative)
            result.append({'path': relative, 'content': None if content is None else base64.b64encode(content).decode()})
    for path in workspace.rglob('*'):
        if not path.is_file() and not path.is_symlink():
            continue
        relative = path.relative_to(workspace).as_posix()
        if relative in original:
            continue
        if not allowed(relative, roots) or path.is_symlink():
            raise ScopeViolation('Out-of-scope or symlink addition: ' + relative)
        candidate_path(workspace, relative)
        result.append({'path': relative, 'content': base64.b64encode(path.read_bytes()).decode()})
    return sorted(result, key=lambda item: item['path'])


def verify(config, workspace, original, run_id, attempt, output):
    started = time.monotonic()
    changed = changes(workspace, original, config['allowed_roots'])
    candidate_digest = digest(json.dumps(changed, sort_keys=True).encode())
    save(output / ('candidate-' + str(attempt) + '.json'), changed)
    remote_root = config['remote_root']
    result_path = remote_root + '/results/' + run_id + '-' + str(attempt) + '.json'
    request = {'config': config, 'remote_root': remote_root, 'run_id': run_id, 'attempt': attempt, 'changes': changed, 'candidate_digest': candidate_digest, 'result_path': result_path}
    request_path = remote_root + '/requests/' + run_id + '-' + str(attempt) + '.json'
    sandbox = config['sandbox']
    put(sandbox, request_path, json.dumps(request).encode())
    command = 'python3 ' + shlex.quote(config['remote_verifier']) + ' ' + shlex.quote(request_path)
    launch = api('POST', 'sandboxes/' + sandbox + '/commands', {'command': command, 'detached': True})
    save(output / ('verifier-launch-' + str(attempt) + '.json'), launch)
    # Five checks each have a 300-second limit, plus termination and setup time.
    deadline = time.monotonic() + 1800
    while time.monotonic() < deadline:
        status = api('GET', 'sandboxes/' + sandbox + '/commands/' + str(launch['processId']))
        if status.get('status') == 'exited':
            if status.get('exitCode') != 0:
                # Retain errors privately; a broken verifier invalidates the pair.
                save(output / ('verifier-error-' + str(attempt) + '.json'), status)
                raise RuntimeError('Verifier infrastructure failed; inspect the private error record')
            break
        if status.get('status') == 'lost':
            save(output / ('verifier-error-' + str(attempt) + '.json'), status)
            raise RuntimeError('Verifier process was lost; inspect the private error record')
        time.sleep(1)
    else:
        raise TimeoutError('Verifier infrastructure timed out')
    result = json.loads(get(sandbox, result_path))
    expected = {'run_id': run_id, 'attempt': attempt, 'source_commit': config['source_commit'], 'candidate_digest': candidate_digest}
    if any(result.get(key) != value for key, value in expected.items()):
        raise RuntimeError('Verifier result identity does not match the candidate')
    for item in result.pop('formatted_files'):
        if not allowed(item['path'], config['allowed_roots']):
            raise RuntimeError('Formatter returned an out-of-scope file')
        path = candidate_path(workspace, item['path'])
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(base64.b64decode(item['content'], validate=True))
    result['external_wall_s'] = time.monotonic() - started
    save(output / ('checks-' + str(attempt) + '.json'), result)
    save(output / ('normalized-candidate-' + str(attempt) + '.json'), changes(workspace, original, config['allowed_roots']))
    return result


def feedback(result):
    parts = ['Independent verification of your candidate failed. The harness ran formatting and has applied its formatting-only changes to your files. You have one repair turn. Fix the failures below using your current files; the harness will run the same checks once more. No further repair is available.']
    for row in result['checks']:
        if row['name'] != 'initial-format' and row['exit_code'] != 0:
            parts.append('Check: ' + row['name'] + '\nExit code: ' + str(row['exit_code']) + '\n' + row['log'][-8000:])
    return '\n\n'.join(parts)[:16000]


def run(config, condition, output, warmup=False):
    output.mkdir(parents=True, mode=0o700, exist_ok=False)
    run_id = uuid.uuid4().hex
    workspace = Path(config['workspace_parent']) / run_id
    preparation_started = time.monotonic()
    metadata = {'run_id': run_id, 'condition': condition, 'source_commit': config['source_commit'], 'model': MODEL, 'effort': 'medium', 'warmup': warmup, 'attempts': [], 'input_turns_sent': 0, 'executor_started': False}
    original = None
    process = None
    selector = None
    start = None
    checks_wall = 0.0
    result_events = []
    buffer = b''
    log = err = None
    try:
        workspace.mkdir(parents=True, exist_ok=False)
        # Warmup uses the same historical instructions and available files.
        archive = subprocess.Popen(['git', 'archive', config['source_commit']], cwd=config['source_repo'], stdout=subprocess.PIPE)
        try:
            extracted = subprocess.run(['tar', '-xf', '-', '-C', str(workspace)], stdin=archive.stdout)
        finally:
            archive.stdout.close()
        if archive.wait() or extracted.returncode:
            raise RuntimeError('Source export failed')
        original = baseline_map(config['source_repo'], config['source_commit'])
        manifest = json.loads(instruction_guard.bounded(config['instruction_manifest'], instruction_guard.MAX_BYTES))
        block = instruction_guard.bounded(config['instruction_block'], instruction_guard.MAX_BYTES)
        instructions_bytes = instruction_guard.bounded(config['instructions_file'], instruction_guard.MAX_PROMPT)
        if manifest['declared_source_revision'] != config['source_commit']:
            raise ValueError('Instruction manifest belongs to a different source revision')

        def check_instructions():
            try:
                return instruction_guard.verify(manifest, block, workspace, instructions_bytes)
            except (ValueError, OSError) as error:
                raise ScopeViolation('Required instruction integrity failed: ' + str(error)) from error

        metadata['instruction_guard'] = check_instructions()
        instructions = instructions_bytes.decode('utf-8')
        common = b'Reply only READY. This is an instruction-prefix warmup; use no tools.' if warmup else instruction_guard.bounded(config['task_file'], instruction_guard.MAX_PROMPT)
        suffix = b''
        if condition == 'treatment' and not warmup:
            suffix = b'\n\nPrepared evidence follows. It is optional source evidence; mandatory instructions above remain in force. Use additional file reads when necessary.\n\n' + instruction_guard.bounded(config['brief_file'], instruction_guard.MAX_PROMPT)
        prompt_bytes = common + suffix
        instruction_guard.verify_pair(instructions_bytes + b'\0' + common, instructions_bytes + b'\0' + prompt_bytes, suffix)
        prompt = prompt_bytes.decode('utf-8')
        command = [
            'claude', '-p', '--safe-mode', '--restricted', '--tools', ','.join(TOOLS),
            '--permission-mode', 'acceptEdits', '--permission-prompts', 'none',
            '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}',
            '--disable-slash-commands', '--no-chrome', '--no-session-persistence',
            '--exclude-dynamic-system-prompt-sections', '--append-system-prompt', instructions,
            '--input-format', 'stream-json', '--output-format', 'stream-json', '--verbose',
            '--model', MODEL, '--effort', 'medium', '--max-budget-usd', '10',
        ]
        metadata.update(instructions_sha256=digest(instructions_bytes), prompt_sha256=digest(prompt_bytes), prompt_bytes=len(prompt_bytes), common_input_sha256=digest(instructions_bytes + b'\0' + common), suffix_sha256=digest(suffix), preparation_wall_s=time.monotonic() - preparation_started)
        save(output / 'request.json', metadata)
        start = time.monotonic()
        model_env = os.environ.copy()
        model_env.pop('BOAT_API_KEY', None)
        log = (output / 'events.jsonl').open('wb')
        err = (output / 'stderr.log').open('wb')
        process = subprocess.Popen(command, cwd=workspace, env=model_env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err, start_new_session=True)
        metadata['executor_started'] = True

        def send(text):
            check_instructions()
            message = {'type': 'user', 'message': {'role': 'user', 'content': [{'type': 'text', 'text': text}]}}
            # Count before writing: a failed write may have delivered a partial input.
            metadata['input_turns_sent'] += 1
            save(output / 'progress.json', metadata)
            process.stdin.write(json.dumps(message).encode() + b'\n')
            process.stdin.flush()

        send(prompt)
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        done = False
        while not done:
            if time.monotonic() - start - checks_wall > 600:
                metadata['timeout'] = True
                break
            ready = selector.select(1)
            if not ready:
                if process.poll() is not None:
                    break
                continue
            data = os.read(process.stdout.fileno(), 65536)
            if not data:
                break
            log.write(data)
            log.flush()
            buffer += data
            while b'\n' in buffer:
                line, buffer = buffer.split(b'\n', 1)
                if not line:
                    continue
                event = json.loads(line)
                if event.get('type') == 'system' and event.get('subtype') == 'init':
                    metadata['init'] = event
                    if set(event.get('tools', [])) != set(TOOLS) or event.get('model') != MODEL:
                        raise RuntimeError('Unexpected executor tool or model configuration')
                if event.get('type') != 'result':
                    continue
                result_events.append(event)
                attempt = len(result_events)
                attempt_record = {'attempt': attempt, 'result_received_wall_s': time.monotonic() - start, 'cumulative_cost_usd': event.get('total_cost_usd'), 'result': event}
                # Retain the charge before any external verifier can fail.
                metadata['attempts'].append(attempt_record)
                save(output / 'progress.json', metadata)
                if attempt != metadata['input_turns_sent'] or attempt > (1 if warmup else 2):
                    raise RuntimeError('Unexpected number of executor results')
                if 'init' not in metadata:
                    raise RuntimeError('Executor returned a result without initialization metadata')
                if warmup:
                    check_instructions()
                    done = True
                    break
                check_start = time.monotonic()
                try:
                    check_instructions()
                    verification = verify(config, workspace, original, run_id, attempt, output)
                    check_instructions()
                except ScopeViolation as error:
                    verification = {'passed': False, 'scope_failure': str(error), 'checks': []}
                except Exception as error:
                    attempt_record['verification_error'] = {'type': type(error).__name__, 'message': str(error)[:2000]}
                    raise
                finally:
                    elapsed = time.monotonic() - check_start
                    checks_wall += elapsed
                    attempt_record['external_wall_s'] = elapsed
                verification['external_wall_s'] = elapsed
                attempt_record['verification'] = verification
                print(json.dumps({'output': str(output), 'attempt': attempt, 'passed': verification['passed'], 'cumulative_cost_usd': event.get('total_cost_usd'), 'elapsed_s': time.monotonic() - start}), flush=True)
                save(output / 'progress.json', metadata)
                if verification['passed'] or attempt >= 2 or verification.get('scope_failure') or event.get('is_error') is not False or event.get('subtype') != 'success':
                    done = True
                    break
                repair = feedback(verification)
                (output / 'feedback.txt').write_text(repair)
                send(repair)
    except (Exception, KeyboardInterrupt) as error:
        metadata['error'] = {'type': type(error).__name__, 'message': str(error)[:2000]}
    finally:
        if selector is not None:
            selector.close()
        if process is not None:
            try:
                if process.stdin and not process.stdin.closed:
                    try:
                        process.stdin.close()
                    except BrokenPipeError:
                        pass
                if process.poll() is None:
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        metadata['executor_forced_stop'] = True
                        os.killpg(process.pid, signal.SIGTERM)
                        try:
                            process.wait(timeout=5)
                        except subprocess.TimeoutExpired:
                            os.killpg(process.pid, signal.SIGKILL)
                            process.wait()
            except Exception as error:
                metadata['cleanup_error'] = {'type': type(error).__name__, 'message': str(error)[:2000]}
            if process.stdout:
                process.stdout.close()
        for handle in (log, err):
            if handle is not None:
                handle.close()
        metadata['preparation_wall_s'] = metadata.get('preparation_wall_s', time.monotonic() - preparation_started)
        metadata['wall_s'] = 0.0 if start is None else time.monotonic() - start
        metadata['checks_wall_s'] = checks_wall
        metadata['agent_wall_s'] = metadata['wall_s'] - checks_wall
        metadata['exit_code'] = None if process is None else process.returncode
        metadata['session_results'] = len(result_events)
        costs = [event.get('total_cost_usd') for event in result_events]
        valid_costs = [cost for cost in costs if type(cost) in (int, float) and math.isfinite(cost) and cost >= 0]
        costs_valid = len(valid_costs) == len(costs) and all(a <= b for a, b in zip(valid_costs, valid_costs[1:]))
        metadata['cost_usd_list_estimate'] = valid_costs[-1] if valid_costs else (0.0 if not metadata['executor_started'] else None)
        metadata['cost_complete'] = bool(not metadata['executor_started'] or (costs_valid and len(result_events) == metadata['input_turns_sent'] and bool(result_events)))
        metadata['cost_status'] = 'not_started' if not metadata['executor_started'] else 'complete_cli_report' if metadata['cost_complete'] else 'incomplete_lower_bound' if valid_costs else 'unknown'
        metadata['repair_cost_usd_list_estimate'] = valid_costs[1] - valid_costs[0] if costs_valid and len(valid_costs) == 2 else None
        if not warmup and original is not None:
            try:
                save(output / 'final-candidate.json', changes(workspace, original, config['allowed_roots']))
            except Exception as error:
                metadata['scope_failure'] = str(error)[:2000]
        last_verification = metadata['attempts'][-1].get('verification', {}) if metadata['attempts'] else {}
        model_ok = bool(result_events and all(event.get('is_error') is False and event.get('subtype') == 'success' for event in result_events) and metadata['exit_code'] == 0 and metadata['cost_complete'] and 'init' in metadata)
        metadata['completed'] = bool(model_ok and not any(metadata.get(key) for key in ('error', 'timeout', 'scope_failure', 'cleanup_error', 'executor_forced_stop')))
        metadata['accepted'] = bool(not warmup and metadata['completed'] and last_verification.get('passed') and not last_verification.get('scope_failure'))
        if metadata['completed'] and not last_verification.get('scope_failure'):
            try:
                shutil.rmtree(workspace)
            except OSError as error:
                metadata['workspace_cleanup_error'] = str(error)[:2000]
        if workspace.exists():
            metadata['retained_workspace'] = str(workspace)
        save(output / 'result.json', metadata)
        print(json.dumps({k: metadata[k] for k in ['condition', 'wall_s', 'agent_wall_s', 'cost_usd_list_estimate', 'cost_status', 'accepted', 'session_results', 'exit_code']}), flush=True)
    return metadata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--condition', choices=['control', 'treatment'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--warmup', action='store_true')
    args = parser.parse_args()
    result = run(json.loads(args.config.read_text()), args.condition, args.output, args.warmup)
    if not result['completed']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()

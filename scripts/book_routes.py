#!/usr/bin/env python3
"""Qualify bounded book routes and reuse only precisely matching observations.

Maintenance tooling, never product runtime. --check reads metadata only.
--plan may ask Coax for dependency metadata; it never runs route examples.
Receipts bind authored/declared inputs and observed tool/host identities, not
execution attestation, all SDK internals or production deployment.
"""
import argparse
from contextvars import ContextVar
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = 'docs/books/routes.json'
SCHEMA = 1
DEADLINE = ContextVar('book_route_deadline', default=None)
ID = re.compile(r'[a-z][a-z0-9-]*\Z')
SHA = re.compile(r'[a-f0-9]{64}\Z')


class RouteError(ValueError):
    pass


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    value = hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda: source.read(1048576), b''):
            value.update(block)
    return value.hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def unique_pairs(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise RouteError(f'duplicate JSON key: {key}')
        value[key] = item
    return value


def read_json(path):
    return json.loads(Path(path).read_text(), object_pairs_hook=unique_pairs)


def checked_path(root, name):
    """Authored inputs must be files contained by the checkout, including links."""
    if not isinstance(name, str) or not name or '\\' in name:
        raise RouteError(f'invalid input path: {name!r}')
    p = PurePosixPath(name)
    if p.is_absolute() or '..' in p.parts or str(p) != name:
        raise RouteError(f'input path must be normalized and relative: {name!r}')
    base = Path(root).resolve()
    result = base / name
    try:
        resolved = result.resolve(strict=True)
        resolved.relative_to(base)
    except (OSError, ValueError) as exc:
        raise RouteError(f'missing or escaping input: {name}') from exc
    if not resolved.is_file():
        raise RouteError(f'input is not a file: {name}')
    return result


def step_paths(step):
    result = [step['stdout'], *step.get('inputs', [])]
    if step['kind'] == 'coax-run':
        result.append(step['source'])
    else:
        result.extend(step.get('sources', []))
    return result


def validate_step(root, step):
    if not isinstance(step, dict) or not ID.fullmatch(step.get('id', '')):
        raise RouteError('invalid step ID')
    if step.get('kind') not in ('coax-run', 'command'):
        raise RouteError('unknown route step kind')
    if type(step.get('exit_status')) is not int or not 0 <= step['exit_status'] <= 255:
        raise RouteError('exit_status must be an explicit byte')
    modes = step.get('modes')
    allowed = {'debug', 'release'} if step['kind'] == 'coax-run' else {'native'}
    if not isinstance(modes, list) or not modes or len(set(modes)) != len(modes) or set(modes) - allowed:
        raise RouteError('invalid or duplicate execution modes')
    if type(step.get('timeout', 120)) is not int or not 1 <= step.get('timeout', 120) <= 840:
        raise RouteError('step timeout must be 1..840 seconds')
    if 'run_steps' in step and (type(step['run_steps']) is not int or not 1 <= step['run_steps'] <= 250000000):
        raise RouteError('invalid Coax compile-time budget')
    if step['kind'] == 'command':
        argv = step.get('argv')
        if not isinstance(argv, list) or not argv or any(not isinstance(x, str) or not x for x in argv):
            raise RouteError('command requires a nonempty argv list')
        if not step.get('inputs') or not step.get('sources'):
            raise RouteError('command must declare host/script inputs and Coax source roots')
        if argv[0] == 'bash':
            if len(argv) < 2 or argv[1].startswith('-'):
                raise RouteError('bash command requires a declared script, not shell code')
            checked_path(root, argv[1])
            if argv[1] not in step['inputs']:
                raise RouteError('bash script must be a declared input')
        elif '/' in argv[0]:
            checked_path(root, argv[0])
        else:
            raise RouteError('command entry must be bash or a declared checkout executable')
    env = step.get('env', {})
    if not isinstance(env, dict) or any(not re.fullmatch('[A-Z][A-Z0-9_]*', k) or not isinstance(v, str) for k, v in env.items()):
        raise RouteError('invalid declared environment')
    if {'COAX', 'COAX_CC', 'PATH', 'HOME', 'TMPDIR'} & env.keys():
        raise RouteError('runner-owned environment cannot be replaced')
    for name in step_paths(step):
        checked_path(root, name)


def load_manifest(root, path):
    path = Path(path)
    if path.is_absolute():
        try:
            path = Path(root) / path.resolve(strict=True).relative_to(Path(root).resolve())
        except (OSError, ValueError) as exc:
            raise RouteError('catalog escapes checkout') from exc
    else:
        path = checked_path(root, str(path))
    data = read_json(path)
    if data.get('schema_version') != SCHEMA or not isinstance(data.get('routes'), list) or not data['routes']:
        raise RouteError('unsupported or empty route catalog')
    seen = set()
    for route in data['routes']:
        key = route.get('id', '')
        if not ID.fullmatch(key) or key in seen:
            raise RouteError('duplicate or invalid route ID')
        seen.add(key)
        if route.get('book') not in ('coax', 'aethyr', 'psionic', 'nostr'):
            raise RouteError('unknown book')
        if not route.get('title') or not route.get('scope') or not route.get('limitations'):
            raise RouteError('route needs a title, bounded scope and limitations')
        if not isinstance(route.get('prerequisites'), dict):
            raise RouteError('route needs explicit prerequisites')
        for name in route.get('chapters', []) + route.get('evidence', []):
            checked_path(root, name)
        if not isinstance(route.get('steps'), list) or not route['steps']:
            raise RouteError('route needs at least one step')
        step_ids = set()
        for step in route['steps']:
            validate_step(root, step)
            if step['id'] in step_ids:
                raise RouteError('duplicate step ID')
            step_ids.add(step['id'])
    return data


def execute(argv, root, env=None, timeout=120):
    """A bounded process group: compiler/tool children are reaped on interruption."""
    deadline = DEADLINE.get()
    if deadline is not None:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise subprocess.TimeoutExpired(argv, timeout, output=b'', stderr=b'')
        timeout = min(timeout, remaining)
    process = subprocess.Popen(argv, cwd=root, env=env, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
        return subprocess.CompletedProcess(argv, process.returncode, stdout, stderr)
    except BaseException:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = process.communicate(timeout=5)
        error = sys.exc_info()[1]
        error.output, error.stderr = stdout, stderr
        raise


def tool_identity(path, version_args):
    path = Path(shutil.which(str(path)) or path).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise RouteError(f'tool is not executable: {path}')
    result = execute([str(path), *version_args], ROOT, timeout=20)
    if result.returncode:
        raise RouteError(f'cannot identify tool: {path.name}')
    return {'sha256': file_sha(path), 'version': result.stdout.decode(errors='replace').strip()}


def tool_context(compiler):
    return {'coax': tool_identity(compiler, ['version']),
            'cc': tool_identity(os.environ.get('COAX_CC', 'cc'), ['--version']),
            'python': {'version': sys.version, 'executable_sha256': file_sha(Path(sys.executable).resolve())}}


def host_target():
    return {'system': platform.system(), 'release': platform.release(), 'machine': platform.machine(),
            'SDKROOT': os.environ.get('SDKROOT', ''),
            'MACOSX_DEPLOYMENT_TARGET': os.environ.get('MACOSX_DEPLOYMENT_TARGET', '')}


def environment(compiler, extra=None, work=None):
    # Unlisted ambient compiler flags, loader overrides and credentials do not
    # enter these local examples. PATH/HOME support the selected host toolchain.
    result = {key: os.environ[key] for key in ('PATH', 'HOME', 'SDKROOT', 'MACOSX_DEPLOYMENT_TARGET') if key in os.environ}
    result.update(LC_ALL='C', LANG='C', COAX=str(Path(compiler).resolve()),
                  COAX_CC=str(Path(shutil.which(os.environ.get('COAX_CC', 'cc')) or os.environ.get('COAX_CC', 'cc')).resolve()))
    if work is not None:
        result['TMPDIR'] = str(work)
    result.update(extra or {})
    return result


def dependency_inputs(root, step, compiler, mode):
    roots = [step['source']] if step['kind'] == 'coax-run' else step['sources']
    inputs = set(step_paths(step))
    for source in roots:
        cmd = [str(compiler), 'deps', source, '--library']
        if mode == 'release':
            cmd.append('--release')
        if 'run_steps' in step:
            cmd += ['--run-steps', str(step['run_steps'])]
        result = execute(cmd, root, environment(compiler, step.get('env')), min(step.get('timeout', 120), 240))
        if result.returncode:
            raise RouteError(f'dependency discovery failed for {source}: {result.stderr.decode(errors="replace")}')
        graph = json.loads(result.stdout)
        if graph.get('schema_version') != 1 or not isinstance(graph.get('sources'), list) or not graph['sources']:
            raise RouteError('invalid dependency metadata')
        for key in ('sources', 'descriptors'):
            if not isinstance(graph.get(key), list) or any(not isinstance(value, str) for value in graph[key]):
                raise RouteError('invalid dependency metadata list')
            for value in graph[key]:
                path = Path(value)
                if path.is_absolute():
                    try:
                        value = str(path.resolve(strict=True).relative_to(Path(root).resolve()))
                    except (OSError, ValueError) as exc:
                        raise RouteError('dependency escapes checkout') from exc
                checked_path(root, value)
                inputs.add(value)
    return sorted(inputs)


def input_state(root, paths):
    inputs, resolutions = {}, {}
    for name in sorted(paths):
        path = checked_path(root, name)
        inputs[name] = file_sha(path)
        resolutions[name] = str(path.resolve().relative_to(Path(root).resolve()))
    return inputs, resolutions


def tools_for_step(root, step, compiler):
    tools = dict(tool_context(compiler))
    if step['kind'] == 'command':
        first = step['argv'][0]
        if '/' in first:
            tools['command'] = {'sha256': file_sha(checked_path(root, first))}
        elif first == 'bash':
            tools['command'] = tool_identity('bash', ['--version'])
        else:
            raise RouteError('command entry must be bash or a declared checkout executable')
    return tools


def bound_environment(compiler, step):
    values = environment(compiler, step.get('env'))
    # Paths of the selected executables are represented by content identities;
    # inherited resolution/cache homes stay explicit rather than unbound.
    values['COAX'] = '<selected-coax>'
    values['COAX_CC'] = '<selected-cc>'
    return values


def closure_digest(inputs, resolutions):
    return digest(canonical({'inputs': inputs, 'resolutions': resolutions}).encode())


def fingerprint_step(root, step, compiler, target, *, mode=None, previous=None):
    validate_step(root, step)
    mode = mode or step['modes'][0]
    if mode not in step['modes']:
        raise RouteError('mode is outside the declared step')
    recipe = {k: v for k, v in step.items() if k != 'modes'}
    recipe.update(mode=mode, runner_sha256=file_sha(__file__), locale='C')
    tools = tools_for_step(root, step, compiler)
    identity = {'recipe_sha256': digest(canonical(recipe).encode()), 'runner_sha256': file_sha(__file__),
                'tools': tools, 'target': target, 'environment': bound_environment(compiler, step)}
    paths = None
    if (previous and all(previous.get(k) == value for k, value in identity.items())
            and set(step_paths(step)) <= set(previous.get('inputs', {}))
            and previous.get('closure_sha256') == closure_digest(previous.get('inputs'), previous.get('resolutions'))):
        try:
            old, resolutions = input_state(root, previous['inputs'])
            if old == previous['inputs'] and resolutions == previous['resolutions']:
                paths = list(old)
        except (OSError, KeyError, RouteError):
            pass
    if paths is None:
        paths = dependency_inputs(root, step, compiler, mode)
    inputs, resolutions = input_state(root, paths)
    return dict(identity, inputs=inputs, resolutions=resolutions, closure_sha256=closure_digest(inputs, resolutions),
                expected={'stdout': step['stdout'], 'sha256': inputs[step['stdout']], 'exit_status': step['exit_status']})


def artifact_path(base, descriptor):
    name = descriptor['path']
    path = checked_path(base, name)
    if not SHA.fullmatch(descriptor['sha256']) or type(descriptor['bytes']) is not int:
        raise RouteError('invalid artifact descriptor')
    data = path.read_bytes()
    if len(data) != descriptor['bytes'] or digest(data) != descriptor['sha256']:
        raise RouteError('artifact changed')
    return data


def reusable(prior, fingerprint, root, receipt_dir=None):
    if not prior or prior.get('status') != 'passed' or prior.get('fingerprint') != fingerprint:
        return False
    try:
        if type(prior['exit_status']) is not int or prior['exit_status'] != fingerprint['expected']['exit_status']:
            return False
        stdout = artifact_path(receipt_dir or root, prior['stdout'])
        stderr = artifact_path(receipt_dir or root, prior['stderr'])
        return not stderr and digest(stdout) == fingerprint['expected']['sha256']
    except (OSError, KeyError, TypeError, RouteError):
        return False


def git_revision(root):
    try:
        return subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True, stderr=subprocess.DEVNULL).strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def atomic_json(path, value):
    path = Path(path)
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name + '.', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as output:
            value['integrity_sha256'] = digest(canonical({k: v for k, v in value.items() if k != 'integrity_sha256'}).encode())
            json.dump(value, output, indent=2, allow_nan=False)
            output.write('\n')
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def row_command(step, mode, compiler):
    if step['kind'] == 'command':
        return step['argv']
    argv = [str(compiler), 'run', step['source']]
    if mode == 'release':
        argv.append('--release')
    if 'run_steps' in step:
        argv += ['--run-steps', str(step['run_steps'])]
    return argv


def run_route(root, route, compiler, receipt, previous=None, timeout=810, *, target=None, plan=False):
    root, compiler, receipt = Path(root).resolve(), Path(compiler).resolve(), Path(receipt).resolve()
    if not 1 <= timeout <= 840:
        raise RouteError('whole-route limit must be 1..840 seconds')
    if not route.get('steps'):
        raise RouteError('cannot qualify an empty route')
    old, prior_path = {}, None
    if previous:
        prior_path = Path(previous).resolve()
        proof = read_json(prior_path)
        if proof.get('integrity_sha256') != digest(canonical({k: v for k, v in proof.items() if k != 'integrity_sha256'}).encode()):
            raise RouteError('prior receipt integrity mismatch')
        if proof.get('schema_version') != SCHEMA or proof.get('route', {}).get('id') != route['id']:
            raise RouteError('prior receipt is for another route or schema')
        if proof.get('status') == 'passed':
            old = {r['key']: r for r in proof['steps']}
            if len(old) != len(proof['steps']):
                raise RouteError('duplicate prior step')
    target = target or host_target()
    proof = {'schema_version': SCHEMA, 'kind': 'book-route-observation',
             'route': {k: route[k] for k in ('id', 'book', 'title', 'scope', 'limitations')},
             'status': 'planned' if plan else 'running', 'observed_revision': git_revision(root),
             'started_at': datetime.now(timezone.utc).isoformat(), 'steps': [],
             'route_definition': route, 'route_definition_sha256': digest(canonical(route).encode()),
             'revision_scope': 'Git HEAD at observation; source hashes, not a claim that the worktree is clean or published.',
             'scope': 'Exact selected native examples and declared workflow, not all book capabilities, product adoption, live transport, browser rendering or production deployment.'}
    if prior_path:
        proof['previous_receipt'] = {'path': str(prior_path), 'sha256': file_sha(prior_path)}
    if not plan:
        receipt.parent.mkdir(parents=True, exist_ok=True)
        # Refuse overwrite of both failed and successful evidence.
        with receipt.open('x'):
            pass
        assets = receipt.parent / (receipt.stem + '.assets')
        try:
            assets.mkdir(mode=0o700)
        except BaseException:
            receipt.unlink()
            raise
        atomic_json(receipt, proof)
    deadline = time.monotonic() + timeout
    deadline_token = DEADLINE.set(deadline)
    try:
        for step in route['steps']:
            for mode in step['modes']:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise RouteError('whole-route time budget exhausted')
                key = step['id'] + ':' + mode
                prior = old.get(key)
                fp = fingerprint_step(root, step, compiler, target, mode=mode,
                                      previous=prior.get('fingerprint') if prior else None)
                can_retain = reusable(prior, fp, root, prior_path.parent if prior_path else None)
                row = {'key': key, 'fingerprint': fp, 'mode': mode, 'status': 'planned' if plan else 'running',
                       'action': 'retain' if can_retain else 'execute'}
                proof['steps'].append(row)
                if plan:
                    continue
                def store_output(which, data):
                    name = f'{receipt.stem}.assets/{step["id"]}-{mode}.{which}'
                    path = receipt.parent / name
                    with path.open('xb') as output:
                        output.write(data)
                    return {'path': name, 'sha256': digest(data), 'bytes': len(data)}
                if can_retain:
                    row.update(status='passed', exit_status=prior['exit_status'], retained=True,
                               origin=prior.get('origin', {'receipt_sha256': file_sha(prior_path),
                                                         'observed_revision': read_json(prior_path)['observed_revision']}))
                    row['stdout'] = store_output('stdout', artifact_path(prior_path.parent, prior['stdout']))
                    row['stderr'] = store_output('stderr', artifact_path(prior_path.parent, prior['stderr']))
                else:
                    work = assets / (step['id'] + '-' + mode + '.work')
                    work.mkdir(mode=0o700)
                    argv = row_command(step, mode, compiler)
                    row.update(command=argv, retained=False)
                    atomic_json(receipt, proof)
                    started = time.monotonic()
                    try:
                        result = execute(argv, root, environment(compiler, step.get('env'), work),
                                         min(step.get('timeout', 120), max(0.001, deadline - time.monotonic())))
                        row.update(exit_status=result.returncode, seconds=time.monotonic()-started)
                        stdout, stderr = result.stdout, result.stderr
                    except BaseException as exc:
                        row.update(exit_status=None, seconds=time.monotonic()-started)
                        stdout, stderr = getattr(exc, 'output', None) or b'', getattr(exc, 'stderr', None) or b''
                        row['stdout'] = store_output('stdout', stdout)
                        row['stderr'] = store_output('stderr', stderr)
                        if isinstance(exc, subprocess.TimeoutExpired):
                            row.update(timeout=True, status='failed')
                            raise RouteError(f'{key} timed out; inspect retained output') from exc
                        row['status'] = 'interrupted' if isinstance(exc, (KeyboardInterrupt, SystemExit)) else 'failed'
                        raise
                    row['stdout'] = store_output('stdout', stdout)
                    row['stderr'] = store_output('stderr', stderr)
                    passed = (row['exit_status'] == step['exit_status'] and not stderr and digest(stdout) == fp['expected']['sha256'])
                    row['status'] = 'passed' if passed else 'failed'
                    if not passed:
                        raise RouteError(f'{key} failed; inspect retained output')
                current, resolutions = input_state(root, fp['inputs'])
                if (current != fp['inputs'] or resolutions != fp['resolutions']
                        or tools_for_step(root, step, compiler) != fp['tools']
                        or bound_environment(compiler, step) != fp['environment']
                        or file_sha(__file__) != fp['runner_sha256']):
                    row['status'] = 'failed'
                    raise RouteError(f'qualification input changed during {key}')
                atomic_json(receipt, proof)
        if not plan:
            for row in proof['steps']:
                current, resolutions = input_state(root, row['fingerprint']['inputs'])
                if current != row['fingerprint']['inputs'] or resolutions != row['fingerprint']['resolutions']:
                    raise RouteError('an earlier input changed before final publication')
                step = next(x for x in route['steps'] if x['id'] + ':' + row['mode'] == row['key'])
                fp = row['fingerprint']
                if (tools_for_step(root, step, compiler) != fp['tools']
                        or bound_environment(compiler, step) != fp['environment']
                        or file_sha(__file__) != fp['runner_sha256']
                        or not reusable(row, fp, root, receipt.parent)):
                    raise RouteError('an earlier tool, runner, environment or output changed before final publication')
            proof.update(status='passed', executed_runs=sum(not r['retained'] for r in proof['steps']),
                         retained_runs=sum(r['retained'] for r in proof['steps']))
    except BaseException as exc:
        proof.update(status='interrupted' if isinstance(exc, (KeyboardInterrupt, SystemExit)) else 'failed', error=str(exc))
        if isinstance(exc, (KeyboardInterrupt, SystemExit)):
            raise
    finally:
        DEADLINE.reset(deadline_token)
        proof['finished_at'] = datetime.now(timezone.utc).isoformat()
        if not plan:
            atomic_json(receipt, proof)
    return proof


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--manifest', type=Path, default=Path(MANIFEST))
    parser.add_argument('--coax', type=Path, default=Path(os.environ.get('COAX', ROOT/'coax/bin/coax')))
    parser.add_argument('--route')
    parser.add_argument('--receipt', type=Path)
    parser.add_argument('--previous', type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--plan', action='store_true')
    parser.add_argument('--max-seconds', type=int, default=810)
    args = parser.parse_args()
    try:
        manifest = load_manifest(args.root, args.manifest)
        if args.check:
            print(f'book routes: {len(manifest["routes"])} profiles, {sum(len(r["steps"]) for r in manifest["routes"])} steps; metadata valid')
            return
        if not args.route or not (args.receipt or args.plan):
            parser.error('execution needs --route and --receipt; planning needs --route --plan')
        selected = [r for r in manifest['routes'] if r['id'] == args.route]
        if len(selected) != 1:
            parser.error('unknown route')
        proof = run_route(args.root, selected[0], args.coax, args.receipt or Path('unused-plan.json'),
                          args.previous, args.max_seconds, plan=args.plan)
        print(json.dumps(proof, indent=2))
        if proof['status'] not in ('passed', 'planned'):
            raise SystemExit(1)
    except (OSError, ValueError, KeyError) as exc:
        parser.error(str(exc))


if __name__ == '__main__':
    main()

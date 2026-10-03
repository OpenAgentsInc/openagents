"""Run only the retrospective depth check in an isolated source-pinned tree."""
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

TEST = 'a_catalog_visible_deep_import_notifies_without_child_writes'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main(request_path):
    request = json.loads(Path(request_path).read_text())
    checker = Path(request['checker']).resolve(strict=True)
    verifier_path = Path(request['verifier']).resolve(strict=True)
    if sha(checker) != request['checker_sha256'] or sha(verifier_path) != request['verifier_sha256']:
        raise ValueError('The checker or source-isolation helper changed')
    revisions = request['revisions']
    if len(revisions) != 10 or len({r['label'] for r in revisions}) != 10:
        raise ValueError('Exactly ten distinct retrospective revisions are required')
    spec = importlib.util.spec_from_file_location('source_isolation', verifier_path)
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    config = request['config']
    if config['source_commit'] != 'aeb7f9fb19e0d56c13400702894172ae472e3bcf':
        raise ValueError('Unexpected historical source pin')
    started = time.monotonic()
    outcomes = []
    with helper.pinned_repo(request) as (root, repo, source, base, lease_fd):
        evidence = root / 'depth-diagnostic'
        evidence.mkdir(exist_ok=False)
        shutil.copyfile(request_path, evidence / 'request.json')
        shutil.copyfile(checker, evidence / 'deep_import.rs')
        try:
            for revision in revisions:
                label = revision['label']
                if not label or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in label):
                    raise ValueError('Invalid revision label')
                changes_path = Path(revision['candidate']).resolve(strict=True)
                if sha(changes_path) != revision['candidate_sha256']:
                    raise ValueError('Candidate bytes changed')
                changes = json.loads(changes_path.read_text())
                attempt = evidence / label
                attempt.mkdir()
                for relative in config['allowed_roots']:
                    path = helper.inside(repo, relative)
                    if path.exists():
                        shutil.rmtree(path) if path.is_dir() else path.unlink()
                helper.export(source, base, repo, config['allowed_roots'])
                for change in changes:
                    relative = helper.relative_path(change['path'])
                    if not any(relative == allowed or relative.startswith(allowed.rstrip('/') + '/') for allowed in config['allowed_roots']):
                        raise ValueError('Candidate path is outside the declared scope')
                    path = helper.inside(repo, relative)
                    if change['content'] is None:
                        path.unlink(missing_ok=True)
                    else:
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes(base64.b64decode(change['content'], validate=True))
                harness = attempt / 'harness'
                harness.mkdir()
                (harness / 'Cargo.toml').write_text(config['checker_manifest'].replace('{REPO}', str(repo)).replace('{CHECKER}', str(evidence / 'deep_import.rs')))
                shutil.copyfile(repo / 'Cargo.lock', harness / 'Cargo.lock')
                home = attempt / 'home'
                home.mkdir()
                env = os.environ.copy()
                env['CARGO_HOME'] = env.get('CARGO_HOME', str(Path.home() / '.cargo'))
                env['RUSTUP_HOME'] = env.get('RUSTUP_HOME', str(Path.home() / '.rustup'))
                env.update(HOME=str(home), CARGO_TARGET_DIR=config['target_dir'], CARGO_INCREMENTAL='0', GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_TERMINAL_PROMPT='0')
                for key in ('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_COMMON_DIR'):
                    env.pop(key, None)
                command = ['cargo', '+1.97.1', 'test', '--manifest-path', str(harness / 'Cargo.toml'), '--offline', '--test', 'acceptance', TEST, '--', '--exact', '--test-threads=1']
                check_started = time.monotonic()
                log_path = attempt / 'depth.log'
                with log_path.open('xb') as log:
                    process = subprocess.Popen(command, cwd=harness, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True, pass_fds=(lease_fd,))
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
                outcome = {'label': label, 'source_commit': base, 'candidate_sha256': revision['candidate_sha256'], 'checker_sha256': request['checker_sha256'], 'exit_code': code, 'timed_out': timed_out, 'check_wall_s': time.monotonic() - check_started, 'log_sha256': sha(log_path), 'command': command}
                (attempt / 'result.json').write_text(json.dumps(outcome, indent=2) + '\n')
                outcomes.append(outcome)
                print(json.dumps({k: v for k, v in outcome.items() if k != 'command'}), flush=True)
        finally:
            summary = {'schema': 'openagents.briefing.depth-posthoc.v1', 'retrospective': True, 'changes_frozen_score': False, 'model_calls': 0, 'paid_model_cost_usd': 0, 'infrastructure_cost_usd': None, 'infrastructure_cost_note': 'Not measured by this runner; retain any provider accounting separately.', 'selected_revisions': len(revisions), 'completed_revisions': len(outcomes), 'wall_s': time.monotonic() - started, 'outcomes': outcomes}
            (evidence / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')


if __name__ == '__main__':
    main(sys.argv[1])

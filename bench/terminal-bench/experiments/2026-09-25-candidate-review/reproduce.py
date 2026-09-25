#!/usr/bin/env python3
"""Review attributable final snapshots without opening official outcomes."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def restore(archive, root):
    """Restore only bounded regular /app files; refuse links and special files."""
    with tarfile.open(archive) as tf:
        members = tf.getmembers()
        if len(members) > 50000 or sum(m.size for m in members) > 128 * 1024**2:
            raise ValueError('Snapshot exceeds restoration bounds')
        seen = set()
        for m in members:
            p = PurePosixPath(m.name)
            if (p.is_absolute() or '..' in p.parts or not p.parts or p.parts[0] != 'app'
                    or str(p) in seen or not (m.isfile() or m.isdir())):
                raise ValueError('Snapshot contains an unsafe or unsupported member')
            seen.add(str(p))
        for m in members:
            p = root / m.name
            if m.isdir():
                p.mkdir(parents=True, exist_ok=True)
            else:
                p.parent.mkdir(parents=True, exist_ok=True)
                with tf.extractfile(m) as source, p.open('xb') as dest:
                    shutil.copyfileobj(source, dest)
                p.chmod(m.mode & 0o777)
    if not (root / 'app').is_dir():
        raise ValueError('Snapshot has no app directory')


def tree(root):
    return {str(p.relative_to(root)): sha(p) for p in sorted(root.rglob('*')) if p.is_file()}


def snapshot(trial, dest):
    comp = json.loads((trial / 'agent/episode/artifacts/composition.json').read_text())
    # Snapshot is immediately after the only executor. Later writers invalidate it.
    branches = comp.get('branches') or []
    if (len(branches) != 1 or branches[0].get('role') != 'primary' or comp.get('escalated')
            or comp.get('repair') or comp.get('second') or comp.get('persist')):
        raise ValueError('Post-executor snapshot cannot be attributed after another writer')
    meta = json.loads((trial / 'agent/episode/snapshot/snapshot.json').read_text())
    if (meta.get('taken') is not True or meta.get('stage') != 'post-executor'
            or meta.get('workdir') != '/app' or meta.get('outside')
            or meta.get('archive', {}).get('paths') != ['app']):
        raise ValueError('No complete attributable app snapshot')
    archive = trial / 'agent/episode/snapshot/workspace.tar.gz'
    if sha(archive) != meta['archive']['sha256']:
        raise ValueError('Snapshot digest mismatch')
    restore(archive, dest)
    # Require every collected /app file to match the snapshot used for review.
    manifest = json.loads((trial / 'artifacts/manifest.json').read_text())
    outside = [e['source'] for e in manifest
               if not e['source'].startswith(('/app/', '/logs/')) and e['source'] not in ['/app', '/logs']]
    if outside:
        raise ValueError('The final candidate includes paths outside the retained app snapshot')
    compared = {}
    for entry in manifest:
        source = PurePosixPath(entry['source'])
        if entry['status'] != 'ok' or not (source == PurePosixPath('/app') or str(source).startswith('/app/')):
            continue
        retained = trial / entry['destination']
        if not retained.resolve().is_relative_to(trial.resolve()):
            raise ValueError('Artifact path escapes trial')
        found = sorted(retained.rglob('*')) if retained.is_dir() else [retained]
        for p in found:
            if not p.is_file() or p.is_symlink():
                continue
            relative = source / p.relative_to(retained) if retained.is_dir() else source
            restored = dest / str(relative).lstrip('/')
            if not restored.is_file() or sha(restored) != sha(p):
                raise ValueError('Collected final artifact differs from snapshot')
            compared[str(relative)] = sha(p)
    return {'snapshot': meta, 'composition_sha256': sha(trial / 'agent/episode/artifacts/composition.json'),
            'collected_files': compared, 'files': tree(dest)}


def environment(row, jobs, out):
    trial = jobs / row['job'] / row['trial']
    cfg = json.loads((trial / 'config.json').read_text())
    task = Path(cfg['task']['path'])
    if task.name != row['task'].split('/')[-1]:
        raise ValueError('Task identity mismatch')
    context = task / 'environment'
    tag = 'truth9584-review/' + task.name + ':public-environment'
    out.mkdir(parents=True, exist_ok=True)
    cached = out / (task.name + '.json')
    if cached.exists():
        old = json.loads(cached.read_text())
        if old.get('files') != tree(context):
            raise ValueError('Public environment changed since the image was retained')
        inspected = subprocess.run(['docker', 'image', 'inspect', '--format', '{{.Id}}', old['image']],
                                   capture_output=True, text=True, timeout=30)
        if inspected.returncode == 0 and inspected.stdout.strip() == old['image']:
            return old['image']
    log_path = out / (task.name + '.build.log')
    with log_path.open('w') as log:
        subprocess.run(['docker', 'build', '--network', 'default', '-t', tag, str(context)],
                       stdout=log, stderr=subprocess.STDOUT, timeout=1200, check=True)
    image = subprocess.check_output(['docker', 'image', 'inspect', '--format', '{{.Id}}', tag], text=True).strip()
    record = {'image': image, 'tag': tag, 'public_environment': str(context), 'files': tree(context)}
    write(out / (task.name + '.json'), record)
    return image


def run(row, args, images, env):
    dest = args.out / row['trial'] / 'reproduced'
    if (dest / 'process.json').exists():
        return row['trial'] + ': retained'
    dest.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    process = {'trial': row['trial'], 'task': row['task'], 'call': 'unknown'}
    container = None
    with tempfile.TemporaryDirectory(prefix='truth9584-reproduced-') as scratch:
        scratch = Path(scratch)
        try:
            trial = args.jobs / row['job'] / row['trial']
            identity = snapshot(trial, scratch)
            write(dest / 'snapshot.json', identity)
            candidate = json.loads(Path(row['input']).read_text())
            candidate['coverage'] = ('The full retained final /app snapshot is mounted read-only. All collected /app artifacts match it. '
                                     'The original public task environment is available, including its data outside /app. '
                                     'No hidden tests, writer report, or grade is supplied. The text file map is only a subset; inspect /app for binary or other files. '
                                     'The container has no network or GPU, a read-only root, 2 GiB RAM, and 128 MiB writable /tmp. '
                                     'Those limits can prevent a valid test. Do not treat such an environment failure as a candidate defect.')
            packet = {'candidate': candidate, 'candidate_identity': identity['snapshot']['archive']['sha256']}
            write(dest / 'input.json', packet)
            if row['task'] not in images:
                raise ValueError('Public task image unavailable')
            cmd = ['docker', 'run', '-d', '--rm', '--network', 'none', '--read-only', '--cap-drop', 'ALL',
                   '--security-opt', 'no-new-privileges', '--pids-limit', '128', '--memory', '2g', '--memory-swap', '2g',
                   '--cpus', '2', '--tmpfs', '/tmp:rw,size=128m', '--mount', f'type=bind,src={scratch / "app"},dst=/app,readonly',
                   '--workdir', '/app', '--env', 'HOME=/tmp', '--env', 'PYTHONDONTWRITEBYTECODE=1',
                   '--env', 'QT_QPA_PLATFORM=offscreen', '--env', 'MPLCONFIGDIR=/tmp/mpl',
                   '--label', 'openagents.candidate-review=1', '--label', 'openagents.candidate-identity=' + packet['candidate_identity'],
                   '--entrypoint', 'sleep', images[row['task']], '400']
            container = subprocess.check_output(cmd, text=True, timeout=30).strip()
            with (dest / 'process.log').open('w') as log:
                p = subprocess.run([str(args.binary), 'checks', 'reproduced-review', '--input', str(dest / 'input.json'),
                                    '--container', container, '--out', str(dest)], env=env, stdout=log,
                                   stderr=subprocess.STDOUT, timeout=340)
            process.update(exit=p.returncode, image=images[row['task']], candidate_identity=packet['candidate_identity'])
            if (dest / 'review.json').exists():
                review = json.loads((dest / 'review.json').read_text())
                process.update(call=review['call'], score=review['score'], review_sha256=sha(dest / 'review.json'))
            if tree(scratch) != identity['files']:
                process.update(call='unknown', error='Candidate changed during read-only review')
        except (OSError, ValueError, subprocess.SubprocessError, tarfile.TarError) as error:
            process['error'] = str(error)
        finally:
            if container:
                cleanup = subprocess.run(['docker', 'rm', '-f', container], capture_output=True, text=True, timeout=30)
                process['cleanup_exit'] = cleanup.returncode
            process['seconds'] = time.monotonic() - started
            write(dest / 'process.json', process)
    return row['trial'] + ': ' + process['call'] + (' (' + process['error'] + ')' if process.get('error') else '')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['manifest', 'jobs', 'out', 'binary']:
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--workers', type=int, default=2)
    a = p.parse_args()
    rows = json.loads(a.manifest.read_text())
    env = dict(os.environ)
    env['TYPESAFE_API_KEY'] = json.loads((Path.home() / '.openagents/jev.json').read_text())['api_key']
    env['CODEX_AUTH_JSON_PATH'] = str(Path.home() / '.codex/auth.json')
    images = {}
    for row in rows:
        if row['task'] in images or (a.out / row['trial'] / 'reproduced/process.json').exists():
            continue
        try:
            images[row['task']] = environment(row, a.jobs, a.out / 'reproduced-images')
            print(row['task'] + ': image retained', flush=True)
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            write(a.out / 'reproduced-images' / (row['task'].split('/')[-1] + '.error.json'), {'error': str(error)})
    with concurrent.futures.ThreadPoolExecutor(max_workers=a.workers) as pool:
        for result in pool.map(lambda r: run(r, a, images, env), rows):
            print(result, flush=True)


if __name__ == '__main__':
    main()

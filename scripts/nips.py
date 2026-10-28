#!/usr/bin/env python3
"""Sync and verify exact upstream NIP trees. Downloaded files are inert sources."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCES = [('official', 'https://github.com/nostr-protocol/nips', '.', 'master'),
           ('block', 'https://github.com/block/buzz', 'docs/nips', 'main')]


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args])


def read_tree(repo, commit, subdir):
    if not re.fullmatch('[0-9a-f]{40}', commit):
        raise ValueError('An exact SHA-1 commit is required')
    tree = commit if subdir == '.' else commit + ':' + subdir
    result = {}
    for item in git(repo, 'ls-tree', '-r', '-z', tree).split(b'\0'):
        if not item: continue
        meta, raw_path = item.split(b'\t', 1)
        mode, kind, oid = meta.split()
        path = raw_path.decode('utf-8')
        parts = PurePosixPath(path).parts
        if not parts or any(p in ('.', '..', '.git') for p in parts) or path.startswith('/'):
            raise ValueError('Unsafe upstream path: ' + path)
        if kind != b'blob' or mode not in (b'100644', b'100755'):
            raise ValueError('Unsupported upstream entry: ' + path)
        result[path] = git(repo, 'cat-file', 'blob', oid.decode())
    if not result or not any(path.endswith('.md') for path in result):
        raise ValueError('Upstream NIP tree is empty')
    return dict(sorted(result.items()))


def hashes(files):
    return {path: hashlib.sha256(data).hexdigest() for path, data in files.items()}


def lane_files(path):
    result = {}
    if not path.is_dir() or path.is_symlink(): raise ValueError('Missing or linked lane: ' + str(path))
    for file in sorted(path.rglob('*')):
        if file.is_symlink(): raise ValueError('Linked lane entry: ' + str(file))
        if file.is_file(): result[file.relative_to(path).as_posix()] = file.read_bytes()
    return result


def compare(actual, expected, name):
    # The only local lane addition permitted is the Block index, which has no
    # upstream README. An upstream README always takes precedence.
    actual = dict(actual)
    if name == 'block' and 'README.md' not in expected: actual.pop('README.md', None)
    if set(actual) != set(expected):
        raise ValueError(f'{name}: missing {sorted(set(expected)-set(actual))}; extra {sorted(set(actual)-set(expected))}')
    changed = [p for p in expected if actual[p] != expected[p]]
    if changed: raise ValueError(name + ': changed bytes: ' + ', '.join(changed))


def write_lane(path, files, name):
    readme = path/'README.md'
    local = readme.read_bytes() if name == 'block' and readme.is_file() and not readme.is_symlink() else None
    if path.is_symlink(): raise ValueError('Refusing linked destination: ' + str(path))
    if path.exists(): shutil.rmtree(path)
    path.mkdir(parents=True)
    for relative, data in files.items():
        destination = path/relative; destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    if local is not None and 'README.md' not in files: readme.write_bytes(local)


def read_manifest(root):
    manifest = json.loads((root/'nips/manifest.json').read_text())
    rows = manifest['sources']
    expected = [(name, repo, subdir, branch) for name, repo, subdir, branch in SOURCES]
    if [(r['name'], r['repo'], r['subdir'], r['branch']) for r in rows] != expected:
        raise ValueError('Unexpected NIP upstream configuration')
    return manifest


def offline(root):
    manifest = read_manifest(root)
    lock = json.loads((root/'nips/upstream-lock.json').read_text())
    if lock['schema'] != 1 or set(lock['sources']) != {r['name'] for r in manifest['sources']}:
        raise ValueError('Unexpected NIP lock schema/lanes')
    for row in manifest['sources']:
        name = row['name']; entry = lock['sources'][name]
        if entry['commit'] != row['commit'] or len(entry['files']) != row['files']:
            raise ValueError('Pin/lock mismatch: ' + name)
        actual = lane_files(root/'nips'/name)
        compare(hashes(actual), entry['files'], name)
        count = sum(p.endswith('.md') and p != 'README.md' for p in entry['files'])
        if count != row['specifications']: raise ValueError('Specification count mismatch: ' + name)
        for path, data in actual.items():
            if path.endswith('.json'): json.loads(data)
        print(f'{name}: verified {row["files"]} upstream files ({count} specifications) at {row["commit"]}')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('mode', choices=['sync', 'check'])
    p.add_argument('--offline', action='store_true')
    p.add_argument('--latest', action='store_true', help='Also require pins to equal the live branch heads')
    p.add_argument('--cache', type=Path, help='Existing Git clones under official/ and block/')
    a = p.parse_args()
    if a.offline and (a.mode != 'check' or a.latest): p.error('--offline requires check without --latest')
    if a.mode == 'check': offline(ROOT)
    if a.offline: return
    manifest = read_manifest(ROOT)
    lock = {'schema': 1, 'sources': {}}
    # Fetch/read both lanes before replacing either. A failed fetch cannot
    # leave one lane updated while the manifest still describes the old pair.
    prepared = []
    with tempfile.TemporaryDirectory(prefix='openagents-nips-') as directory:
        for row in manifest['sources']:
            name = row['name']; repo = (a.cache/name) if a.cache else Path(directory)/name
            if not a.cache:
                subprocess.run(['git','clone','--quiet','--bare','--depth','1','--branch',row['branch'],row['repo'],str(repo)],check=True)
            latest = git(repo, 'ls-remote', '--exit-code', row['repo'], 'refs/heads/'+row['branch']).decode().split()[0]
            commit = latest if a.mode == 'sync' else row['commit']
            # A cache supplies storage, never the definition of "latest".
            if subprocess.run(['git','-C',str(repo),'cat-file','-e',commit+'^{commit}'],stderr=subprocess.DEVNULL).returncode:
                subprocess.run(['git','-C',str(repo),'fetch','--quiet','--depth','1',row['repo'],commit],check=True)
            files = read_tree(repo, commit, row['subdir'])
            if a.mode == 'check':
                compare(lane_files(ROOT/'nips'/name), files, name)
                if a.latest and commit != latest: raise ValueError(name + ': a newer upstream commit is available: ' + latest)
                print(name + ': pinned upstream bytes match' + ('; latest branch head confirmed' if a.latest else ''))
                continue
            row.update(commit=commit, synced_at=datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                       tree_url=row['repo']+'/tree/'+commit+('' if row['subdir']=='.' else '/'+row['subdir']),
                       files=len(files), specifications=sum(path.endswith('.md') and path!='README.md' for path in files))
            lock['sources'][name] = {'commit':commit, 'files':hashes(files)}
            prepared.append((name, files))
        if a.mode == 'sync':
            for name, files in prepared: write_lane(ROOT/'nips'/name, files, name)
            (ROOT/'nips/manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
            (ROOT/'nips/upstream-lock.json').write_text(json.dumps(lock,indent=2)+'\n')
            offline(ROOT)


if __name__ == '__main__':
    try: main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error))

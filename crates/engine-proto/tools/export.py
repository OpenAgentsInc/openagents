#!/usr/bin/env python3
"""Export a small, hash-pinned binding bundle without Cargo or protoc."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parents[1]
SCHEMAS = REPO / 'proto' / 'openagents' / 'engine'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('destination', type=Path)
    parser.add_argument('--allow-dirty', action='store_true')
    args = parser.parse_args()
    dirty = bool(subprocess.check_output(['git', '-C', str(REPO), 'status', '--porcelain', '--', 'proto', 'crates/engine-proto', 'scripts/engine-proto.sh']))
    if dirty and not args.allow_dirty:
        raise SystemExit('Commit the contract before exporting its bindings')
    if args.destination.exists():
        raise SystemExit('Choose a new export directory')
    files = {}
    def add(source, target):
        path = args.destination / target
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, path)
        files[target] = hashlib.sha256(path.read_bytes()).hexdigest()
    for folder, target in [('generated/c', 'c'), ('generated/python', 'python'), ('vendor/nanopb', 'nanopb'), ('fixtures', 'fixtures'), ('data', 'data')]:
        for source in sorted((ROOT / folder).rglob('*')):
            if source.is_file() and '__pycache__' not in source.parts:
                add(source, str(Path(target) / source.relative_to(ROOT / folder)))
    for source in sorted(SCHEMAS.rglob('*')):
        if source.is_file():
            add(source, str(Path('proto/openagents/engine') / source.relative_to(SCHEMAS)))
    for name in ['toolchain.json', 'scene.descriptor.pb', 'README.md']:
        add(ROOT / name, name)
    add(ROOT / 'src/generated/openagents.engine.v1.rs', 'rust/openagents.engine.v1.rs')
    add(ROOT / 'tools/portable.py', 'python/engine_portable.py')
    manifest = {'format': 'openagents.engine-proto.bundle.v1', 'repository': 'https://github.com/OpenAgentsInc/openagents',
                'revision': subprocess.check_output(['git', '-C', str(REPO), 'rev-parse', 'HEAD'], text=True).strip(),
                'dirty': dirty, 'files': dict(sorted(files.items()))}
    (args.destination / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(f'Exported {len(files)} files to {args.destination}')


if __name__ == '__main__':
    main()

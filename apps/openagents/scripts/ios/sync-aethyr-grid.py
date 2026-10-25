#!/usr/bin/env python3
"""Pin the public Aethyr Grid build into the iOS app, or check its hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import re
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[2]
DEST=ROOT/'apps/ios/OpenAgents/Resources/AethyrGrid.bundle'
FILES=['grid.html','grid-ui.css','index.js','index.wasm','index.data','grid-nostr.js','grid-builds.js','grid-session.js','semantic.js','grid-nav.js','grid-ui.js','grid.js']


def check(root):
    manifest=json.loads((root/'manifest.json').read_text())
    if manifest.get('format')!='openagents.aethyr-grid.bundle.v1' or manifest.get('repository')!='https://github.com/OpenAgentsInc/aethyr-cpp' or not re.fullmatch('[0-9a-f]{40}',manifest.get('revision','')):
        raise ValueError('Expected a pinned Aethyr Grid release')
    if not set(FILES+['Info.plist']).issubset(manifest['files']) or len(manifest['files'])>40:
        raise ValueError('Incomplete Grid bundle')
    for name,record in manifest['files'].items():
        if Path(name).is_absolute() or '..' in Path(name).parts or (root/name).is_symlink():raise ValueError('Unsafe Grid bundle path')
        data=(root/name).read_bytes()
        if len(data)!=record['bytes'] or hashlib.sha256(data).hexdigest()!=record['sha256']:raise ValueError('Grid bundle hash mismatch: '+name)
    actual={str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}
    if actual!=set(manifest['files'])|{'manifest.json'}:raise ValueError('Unexpected Grid bundle files')
    for reference in re.findall(r'(?:src|href)="([^"#?]+)"',(root/'grid.html').read_text()):
        if '://' not in reference and reference!='index.html' and reference not in manifest['files']:
            raise ValueError('Missing Grid page asset: '+reference)
    return manifest


def sync(source):
    source=Path(source).resolve()
    if subprocess.check_output(['git','-C',str(source),'status','--porcelain','--','src','web','assets','tools','build.sh','CMakeLists.txt','third_party','licenses']):raise ValueError('Commit the Aethyr runtime before pinning it')
    revision=subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()
    subprocess.run([str(source/'build.sh')],cwd=source,check=True)
    stage=DEST.with_name('AethyrGrid.staging')
    if stage.exists():raise ValueError('Grid import is already in progress')
    stage.mkdir(parents=True)
    try:
        for name in FILES:shutil.copyfile(source/'build'/name,stage/name)
        shutil.copytree(source/'licenses',stage/'licenses')
        (stage/'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier':'com.openagents.grid.resources','CFBundleName':'AethyrGrid','CFBundleVersion':'1','CFBundleShortVersionString':'1.0','CFBundlePackageType':'BNDL'}))
        files={str(p.relative_to(stage)):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(stage.rglob('*')) if p.is_file()}
        manifest={'format':'openagents.aethyr-grid.bundle.v1','repository':'https://github.com/OpenAgentsInc/aethyr-cpp','revision':revision,'files':files}
        (stage/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        check(stage)
        if DEST.exists():shutil.rmtree(DEST)
        stage.replace(DEST)
    finally:
        if stage.exists():shutil.rmtree(stage)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('source',nargs='?');parser.add_argument('--check',action='store_true');args=parser.parse_args()
    if not args.check:
        if not args.source:parser.error('Specify the public Aethyr checkout')
        sync(args.source)
    manifest=check(DEST);print('Bundled Aethyr Grid: '+manifest['revision'])

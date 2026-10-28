#!/usr/bin/env python3
"""Pinned, explicit test-only C build; never runs downloaded build/install scripts."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import subprocess
import tarfile
import urllib.request

HERE=Path(__file__).resolve().parent

def sha(data): return hashlib.sha256(data).hexdigest()

def source_files(archive, pin):
    data=Path(archive).read_bytes()
    if len(data)!=pin['archive_bytes'] or sha(data)!=pin['archive_sha256']:
        raise ValueError('upstream archive identity mismatch')
    files={}
    with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as tar:
        members=tar.getmembers(); roots={PurePosixPath(m.name).parts[0] for m in members}
        if len(roots)!=1: raise ValueError('invalid archive root')
        for member in members:
            parts=PurePosixPath(member.name)
            if parts.is_absolute() or '..' in parts.parts or member.issym() or member.islnk():
                raise ValueError('unsafe archive member')
            if member.isdir(): continue
            if not member.isfile() or len(parts.parts)<2 or member.size>16*1024*1024:
                raise ValueError('unsupported archive member')
            name=PurePosixPath(*parts.parts[1:]).as_posix()
            if name in files: raise ValueError('duplicate archive member')
            files[name]=tar.extractfile(member).read()
    for name,expected in pin['reviewed_inputs'].items():
        if sha(files[name])!=expected: raise ValueError('reviewed input identity mismatch: '+name)
    return files

def materialize(archive, destination, pin):
    files=source_files(archive,pin)
    if destination.is_symlink(): raise ValueError('cached source root is a symlink')
    if destination.exists():
        actual={x.relative_to(destination).as_posix():x for x in destination.rglob('*') if x.is_file() or x.is_symlink()}
        if set(actual)!=set(files): raise ValueError('cached source inventory changed')
        for name,path in actual.items():
            if path.is_symlink() or path.read_bytes()!=files[name]: raise ValueError('cached source changed: '+name)
    else:
        for name,data in files.items():
            path=destination/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
    return sha(json.dumps({n:sha(d) for n,d in sorted(files.items())},sort_keys=True).encode())

def build(archive, cache, reviewed, sanitize=False):
    if not reviewed: raise ValueError('review source/build inputs, then explicitly pass --reviewed; no configure/CMake/install is executed')
    pin=json.loads((HERE/'UPSTREAM.json').read_text()); source=cache/'source'
    tree=materialize(archive,source,pin)
    target=cache/'build';target.mkdir(parents=True,exist_ok=True)
    compiler=shutil.which(os.environ.get('CC','clang'))
    if not compiler: raise ValueError('no C compiler')
    extension='.dylib' if platform.system()=='Darwin' else '.so'
    out=target/('libsecp_oracle'+extension)
    flags=['-std=c11','-O2','-fPIC','-shared','-Wall','-Wextra','-Werror','-Wno-unused-function',
           '-DECMULT_WINDOW_SIZE=15','-DCOMB_BLOCKS=11','-DCOMB_TEETH=6',
           '-DENABLE_MODULE_ECDH','-DENABLE_MODULE_EXTRAKEYS','-DENABLE_MODULE_SCHNORRSIG',
           '-I'+str(source/'include'),'-I'+str(source/'src')]
    if sanitize: flags+=['-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']
    units=[source/'src/secp256k1.c',source/'src/precomputed_ecmult.c',source/'src/precomputed_ecmult_gen.c',HERE/'oracle.c']
    subprocess.run([compiler,*flags,*map(str,units),'-o',str(out)],check=True)
    report={'schema':'coax.secp-oracle-build.v1','source_commit':pin['commit'],'archive_sha256':pin['archive_sha256'],
            'source_tree_sha256':tree,'compiler':subprocess.check_output([compiler,'--version'],text=True).splitlines()[0],
            'target':platform.platform(),'flags':flags,'units':{str(p.relative_to(source)) if p.is_relative_to(source) else p.name:sha(p.read_bytes()) for p in units},
            'adapter_sha256':sha((HERE/'oracle.c').read_bytes()),'artifact_sha256':sha(out.read_bytes()),'artifact':str(out)}
    (target/'build.json').write_text(json.dumps(report,indent=2)+'\n')
    return report

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--fetch',action='store_true',help='explicitly download the pinned archive into ignored cache')
    p.add_argument('--archive',type=Path,help='reuse an already downloaded archive with exact pinned identity')
    p.add_argument('--reviewed',action='store_true',help='confirm source/build inputs were inspected before this explicit C compilation')
    p.add_argument('--sanitize',action='store_true')
    p.add_argument('--cache',type=Path,default=HERE/'.cache')
    args=p.parse_args();pin=json.loads((HERE/'UPSTREAM.json').read_text());args.cache.mkdir(parents=True,exist_ok=True)
    archive=args.archive or args.cache/'secp256k1.tar.gz'
    if args.fetch:
        if args.archive: p.error('--archive and --fetch are mutually exclusive')
        req=urllib.request.Request(pin['archive_url'],headers={'User-Agent':'OpenAgents-test-oracle'})
        with urllib.request.urlopen(req,timeout=45) as response:data=response.read(16*1024*1024+1)
        if sha(data)!=pin['archive_sha256']: raise ValueError('download identity mismatch')
        archive.write_bytes(data)
    print(json.dumps(build(archive,args.cache,args.reviewed,args.sanitize),sort_keys=True))

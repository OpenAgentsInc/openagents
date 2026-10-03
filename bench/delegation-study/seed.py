#!/usr/bin/env python3
"""Build a baseline package on the shared target and export its named artifacts."""
import argparse
import fcntl
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import time
import tomllib

import run_remote as runner
from seed_manifest import SEED_POLICY, digest, inventory, validate_seed

TARGET='/home/executor/target'
MAX_SEED_BYTES=8*1024**3
MIN_COPY_HEADROOM_BYTES=2*1024**3


def package_name(manifest,workspace,toolchain):
    path=PurePosixPath(manifest)
    bindings=[(Path(workspace),PurePosixPath('/workspace'))]+[(Path(a),PurePosixPath(b)) for a,b in toolchain.get('read_only_mounts',[])]
    for host,visible in sorted(bindings,key=lambda pair:len(pair[1].parts),reverse=True):
        try:relative=path.relative_to(visible)
        except ValueError:continue
        name=tomllib.loads((host/relative).read_text())['package']['name']
        if not isinstance(name,str) or not re.fullmatch(r'[a-zA-Z0-9_-]+',name):raise ValueError('Invalid package name')
        return name
    raise ValueError('Cargo reported a manifest outside the admitted source mounts')


def collect(rows,target,workspace,toolchain):
    """Select files explicitly named by Cargo and fixed companions for each unit.

    Final executables, build-script output directories, and their fingerprints
    are excluded. Cargo regenerates them; libraries retain most reusable work.
    """
    target=Path(target).resolve(strict=True)
    selected=set()
    def add(relative):
        relative=PurePosixPath(relative)
        if relative.is_absolute() or '..' in relative.parts:raise ValueError('Invalid Cargo artifact path')
        path=target/relative
        if not path.exists():return
        if path.is_symlink() or not path.is_file() or path.resolve()!=path:
            raise ValueError('A reported Cargo artifact traverses a symlink or is not a file')
        selected.add(relative.as_posix())
    for row in rows:
        if row.get('reason')!='compiler-artifact' or row.get('profile',{}).get('test') is True:continue
        if not set(row.get('target',{}).get('kind',[])).intersection(('lib','rlib','proc-macro','cdylib','dylib')):continue
        package=package_name(row['manifest_path'],workspace,toolchain)
        for filename in row.get('filenames',[]):
            try:relative=PurePosixPath(filename).relative_to(TARGET)
            except ValueError:raise ValueError('Cargo reported an artifact outside the shared target')
            if not relative.parts or relative.parts[0]!='debug' or any(p in ('build','incremental','.fingerprint') for p in relative.parts):
                continue
            if relative.suffix not in ('.rlib','.rmeta','.so','.dylib','.dll'):
                continue
            add(relative)
            basename=relative.name
            stem=re.sub(r'\.(rlib|rmeta|so|dylib|dll|exe)$','',basename)
            if re.search(r'\.(rlib|rmeta|so|dylib|dll)$',basename) and stem.startswith('lib'):stem=stem[3:]
            add(relative.parent/(stem+'.d'))
            match=re.search(r'-([0-9a-f]{16})$',stem)
            if not match:continue
            fingerprint=PurePosixPath('debug/.fingerprint')/(package+'-'+match[1])
            name=row['target']['name'].replace('-','_')
            kind=row['target']['kind']
            is_test=row.get('profile',{}).get('test',False)
            if 'test' in kind:unit='test-integration-test-'+name
            elif 'bin' in kind:unit=('test-bin-' if is_test else 'bin-')+name
            elif any(x in kind for x in ('lib','rlib','proc-macro')):unit=('test-lib-' if is_test else 'lib-')+name
            elif 'example' in kind:unit=('test-example-' if is_test else 'example-')+name
            else:continue
            for companion in ('invoked.timestamp',unit,unit+'.json','dep-'+unit):
                add(fingerprint/companion)
    return sorted(selected)


def copy_budget(target,files,output):
    total=sum((Path(target)/relative).stat().st_size for relative in files)
    if total>MAX_SEED_BYTES:raise ValueError('The library seed exceeds the 8 GiB export bound')
    if shutil.disk_usage(output).free<total+MIN_COPY_HEADROOM_BYTES:
        raise ValueError('Insufficient free space for the seed and 2 GiB retention headroom')
    return total


def save_result(output,result):
    temporary=output/'result.pending.json'
    with temporary.open('w') as handle:
        handle.write(json.dumps(result,indent=2)+'\n');handle.flush();os.fsync(handle.fileno())
    temporary.replace(output/'result.json')


def rustdoc_executable(toolchain):
    declared=toolchain.get('environment',{}).get('RUSTDOC')
    if not isinstance(declared,str) or not Path(declared).is_absolute():
        raise ValueError('RUSTDOC must name an absolute pinned toolchain executable')
    path=Path(declared)
    if path.resolve(strict=True)!=path or not path.is_file() or not os.access(path,os.X_OK):
        raise ValueError('RUSTDOC must name the canonical executable, not a rustup proxy or symlink')
    return path


def build(config,output):
    began=time.monotonic();os.umask(0o077)
    output.mkdir(parents=True,exist_ok=False)
    result={'schema':'openagents.delegation.seed-build.v1','status':'incomplete','source_commit':config['source_commit']}
    save_result(output,result)
    target=Path(config['shared_target']).resolve(strict=True)
    lock=(target/'.ds4-calibration.lock').open('a')
    try:
        fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
        commit=config['source_commit']
        if not re.fullmatch(r'[0-9a-f]{40}',commit):raise ValueError('A full source commit is required')
        packages=config['packages']
        if not packages or any(not re.fullmatch(r'[a-zA-Z0-9_-]+',p) for p in packages):raise ValueError('Invalid baseline package names')
        toolchain=config['toolchain']
        rustdoc=rustdoc_executable(toolchain)
        archive=output/'source.tar'
        archive_started=time.monotonic()
        with archive.open('wb') as handle:
            subprocess.run(['git','-C',config['repository'],'archive','--format=tar',commit],stdout=handle,stderr=subprocess.PIPE,check=True)
        result.update(archive_wall_s=time.monotonic()-archive_started,source_archive_bytes=archive.stat().st_size)
        workspace,home=output/'workspace',output/'home'
        workspace.mkdir();home.mkdir();(home/'.cargo').mkdir()
        export_started=time.monotonic()
        result.update(runner.export(archive,workspace))
        result['export_wall_s']=time.monotonic()-export_started
        snapshot_started=time.monotonic()
        result['snapshot_commit']=runner.initialize_snapshot(workspace,home,commit)
        result['git_snapshot_wall_s']=time.monotonic()-snapshot_started
        # Git archives preserve old mtimes. Advance all source mtimes so a newer
        # unit with the same Cargo hash cannot satisfy this baseline build.
        refreshed=time.time_ns()
        for source in workspace.rglob('*'):
            if source.is_file():os.utime(source,ns=(refreshed,refreshed))
        result['source_mtimes_refreshed_unix_ns']=refreshed
        base=runner.base_arguments(workspace,home,toolchain)+['--bind',str(target),TARGET,'--setenv','CARGO_TARGET_DIR',TARGET,'--setenv','CARGO_INCREMENTAL','0','--setenv','CARGO_NET_OFFLINE','true','--chdir','/workspace','--']
        versions={}
        for name,argv in [('cargo',['cargo','--version']),('rustc',['rustc','-Vv']),('rustdoc',[str(rustdoc),'-Vv'])]:
            versions[name]=subprocess.check_output(base+argv,stderr=subprocess.PIPE,text=True,timeout=30).strip()
        command=['cargo','test','--locked','--offline','--no-run','--message-format=json']
        for package in packages:command+=['-p',package]
        start=time.monotonic()
        with (output/'cargo.jsonl').open('wb') as stdout,(output/'cargo.stderr').open('wb') as stderr:
            check=subprocess.run(base+command,stdout=stdout,stderr=stderr,timeout=config.get('timeout_s',1200))
        result.update(build_wall_s=time.monotonic()-start,exit_code=check.returncode,toolchain=versions)
        save_result(output,result)
        if check.returncode:raise ValueError('The historical baseline did not build')
        rows=[json.loads(line) for line in (output/'cargo.jsonl').read_text().splitlines() if line.strip()]
        if not any(x.get('reason')=='build-finished' and x.get('success') is True for x in rows):
            raise ValueError('Cargo did not report a successful baseline build')
        workspace_units=[x for x in rows if x.get('reason')=='compiler-artifact' and x.get('manifest_path','').startswith('/workspace/')]
        if not workspace_units or any(x.get('fresh') is not False for x in workspace_units):
            raise ValueError('Cargo reused a workspace unit instead of compiling the baseline source')
        result['recompiled_workspace_units']=len(workspace_units)
        files=collect(rows,target,workspace,toolchain)
        result['selected_seed_bytes']=copy_budget(target,files,output)
        seed=output/'seed';destination=seed/'target';destination.mkdir(parents=True)
        for relative in files:
            dest=destination/relative;dest.parent.mkdir(parents=True,exist_ok=True)
            shutil.copy2(target/relative,dest)
        manifest={'schema':'openagents.delegation.cargo-seed.v1','seed_policy':SEED_POLICY,'build_environment':toolchain.get('environment',{}),'source_commit':commit,'source_archive_sha256':digest(archive),'packages':packages,'cargo_command':command,'toolchain':versions,'cargo_json_sha256':digest(output/'cargo.jsonl'),'files':inventory(destination),'omitted':['Final executables and their fingerprints','Build-script output directories and their fingerprints','Incremental compilation state','Unreported files, other units, and diagnostic output files']}
        manifest.update(rustdoc_path=str(rustdoc),rustdoc_sha256=digest(rustdoc))
        (seed/'seed-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
        manifest_sha=digest(seed/'seed-manifest.json')
        validate_seed(seed,manifest_sha,commit)
        result.update(status='complete',seed_manifest_sha256=manifest_sha,seed_files=len(files),seed_bytes=sum(x['bytes'] for x in manifest['files'].values()))
    except Exception as error:
        result.update(status='infrastructure_error',error_type=type(error).__name__,error=str(error)[:400])
    finally:
        lock.close()
        result['total_wall_s']=time.monotonic()-began
        save_result(output,result)
    return result


def main():
    parser=argparse.ArgumentParser();parser.add_argument('config');parser.add_argument('output');args=parser.parse_args()
    result=build(json.loads(Path(args.config).read_text()),Path(args.output).resolve())
    print(json.dumps(result))
    raise SystemExit(0 if result['status']=='complete' else 1)


if __name__=='__main__':main()

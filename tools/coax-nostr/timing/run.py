#!/usr/bin/env python3
"""Explicit timing acquisition/oracle tooling; never part of the app build."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shlex
import shutil
import struct
import subprocess
import time

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
NAMES=['fixed','branch','index']
MAX=2000000
def run(*args,cwd=None,timeout=90):
    result=subprocess.run([str(a) for a in args],cwd=cwd,capture_output=True,text=True,timeout=timeout)
    if result.returncode:raise RuntimeError(f'{args[0]} exited{result.returncode}: {result.stderr[-3000:]}')
    return result.stdout
def sha(path):
    with Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def summarize(groups):
    means=[math.fsum(g)/len(g) for g in groups]
    variances=[math.fsum((v-m)**2 for v in g)/(len(g)-1) for g,m in zip(groups,means)]
    denominator=sum(v/len(g) for v,g in zip(variances,groups))
    t=(means[0]-means[1])/math.sqrt(denominator) if denominator else 0
    if not denominator:status=5 if means[0]==means[1] else 1
    else:status=1 if abs(t)>=10 else 2
    if sum(map(len,groups))<1000000:status=3
    noise=[]
    for values in groups:
        ordered=sorted(values);n=len(values)
        noise.append({'minimum_ns':ordered[0],'median_ns':ordered[n//2],'p99_ns':ordered[math.ceil(n*.99)-1],'maximum_ns':ordered[-1],'zero_ticks':values.count(0)})
    return {'n0':len(groups[0]),'n1':len(groups[1]),'mean0':means[0],'mean1':means[1],'variance0':variances[0],'variance1':variances[1],'t':t,'status':status,'degenerate':not denominator,'noise':noise}
def decode(path):
    size=Path(path).stat().st_size
    if size<12 or size>12+9*MAX:raise ValueError('raw size budget')
    data=Path(path).read_bytes()
    if data[:4]!=b'CT01':raise ValueError('raw version')
    count=struct.unpack_from('<Q',data,4)[0]
    if count<4 or count>MAX or len(data)!=12+count*9:raise ValueError('raw count/size')
    groups=[[],[]]
    for group,ns in struct.iter_unpack('<BQ',data[12:]):
        if group>1 or ns>1000000000:raise ValueError('invalid observation')
        groups[group].append(ns)
    if min(map(len,groups))<2:raise ValueError('insufficient class count')
    return summarize(groups)
def verify(path,observed):
    independent=decode(path)
    for name in ['n0','n1','status','degenerate']:
        if independent[name]!=observed[name]:raise AssertionError(f'{name} disagreement')
    for name in ['mean0','mean1','variance0','variance1','t']:
        if not math.isclose(independent[name],observed[name],rel_tol=2e-8,abs_tol=1e-8):raise AssertionError(f'{name} disagreement')
    return independent
def local(directory):
    exe=directory/'timing-arm64'
    flags=['-std=c11','-O2']
    run('cc',*flags,directory/'native.c','-lm','-o',exe)
    metadata={'architecture':platform.machine(),'os':platform.system()+' '+platform.release(),'cpu':run('sysctl','-n','machdep.cpu.brand_string').strip(),'compiler':run('cc','--version').splitlines()[0],'flags':flags,'binary_sha256':sha(exe),'virtualization':'native host; no emulation requested','environment':'normal shared developer machine; governor, load and other processes unchanged; no hard affinity available'}
    rows=[]
    for index,name in enumerate(NAMES):
        path=directory/f'arm64-{name}.bin'
        observed=json.loads(run(exe,index,path))
        (directory/f'arm64-{name}.json').write_text(json.dumps(observed,indent=2)+'\n')
        independent=verify(path,observed)
        if index and observed['status']!=1:raise AssertionError(f'planted {name} not detected on arm64')
        rows.append({'fixture':name,'coax':observed,'independent':independent,'raw_sha256':sha(path),'raw_file':path.name})
    return {'target':metadata,'runs':rows}
def linux(directory,target):
    ssh=['ssh','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','ConnectTimeout=8',target]
    scp=['scp','-q','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','ConnectTimeout=8']
    remote=run(*ssh,'mktemp -d /tmp/coax-public-timing-XXXXXXXX').strip()
    if not remote.startswith('/tmp/coax-public-timing-') or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/-' for c in remote):raise RuntimeError('unexpected temporary path')
    quoted=shlex.quote(remote)
    try:
        run(*scp,directory/'timing.c',directory/'native.c',target+':'+remote+'/')
        run(*ssh,f'cd {quoted} && cc -std=c11 -O2 native.c -lm -o timing',timeout=90)
        metadata={'architecture':run(*ssh,'uname -m').strip(),'os':run(*ssh,'uname -sr').strip(),'compiler':run(*ssh,'cc --version').splitlines()[0],'cpu':run(*ssh,"sed -n 's/^model name[[:space:]]*: //p' /proc/cpuinfo | head -n 1").strip(),'flags':['-std=c11','-O2'],'environment':'existing native Linux device; own process pinned to first allowed CPU; governor and other processes unchanged'}
        rows=[]
        for index,name in enumerate(NAMES):
            filename=f'x86_64-{name}.bin'
            observed=json.loads(run(*ssh,f'cd {quoted} && ./timing {index} {shlex.quote(filename)}'))
            path=directory/filename
            run(*scp,target+':'+remote+'/'+filename,path)
            (directory/f'x86_64-{name}.json').write_text(json.dumps(observed,indent=2)+'\n')
            independent=verify(path,observed)
            if index and observed['status']!=1:raise AssertionError(f'planted {name} not detected on x86-64')
            rows.append({'fixture':name,'coax':observed,'independent':independent,'raw_sha256':sha(path),'raw_file':path.name})
        run(*scp,target+':'+remote+'/timing',directory/'timing-x86_64')
        metadata['binary_sha256']=sha(directory/'timing-x86_64')
        if metadata['architecture']!='x86_64':raise AssertionError('actual x86-64 required')
        return {'target':metadata,'runs':rows}
    finally:
        run(*ssh,'rm -rf -- '+quoted)
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,default=ROOT/'coax/bin/nostr-timing-2026-10-28')
    parser.add_argument('--ssh-target',help='Existing trusted user@Linux-host; strict noninteractive SSH only')
    parser.add_argument('--skip-local',action='store_true')
    args=parser.parse_args();directory=args.output.resolve();directory.mkdir(parents=True,exist_ok=True)
    compiler=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
    (directory/'timing.c').write_text(run(compiler,'c',HERE/'fixtures.coax','--library','--release'))
    shutil.copyfile(HERE/'native.c',directory/'native.c')
    report={'schema':'coax.public-timing-observations.v1','scope':'Public planted controls only; not production crypto, proof of constant time or secret approval','compiler':run(compiler,'version').strip(),'compiler_sha256':sha(compiler),'seed':20261028,'threshold_abs_t':10,'samples_per_fixture':1000000,'sources':{str(p.relative_to(ROOT)):sha(p) for p in [HERE/'model.coax',HERE/'fixtures.coax',HERE/'native.c',Path(__file__)]},'generated_c_sha256':sha(directory/'timing.c'),'retention':'Raw CT01 files and native objects in ignored local artifact directory; hashes in this committed report; no public secret data','targets':[]}
    if not args.skip_local:report['targets'].append(local(directory))
    if args.ssh_target:report['targets'].append(linux(directory,args.ssh_target))
    report['artifact_bytes']=sum(p.stat().st_size for p in directory.iterdir() if p.is_file())
    if report['artifact_bytes']>128*1024*1024:raise AssertionError('artifact budget128MiB exceeded')
    path=directory/'native-report.json';path.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n');print(path)
if __name__=='__main__':main()

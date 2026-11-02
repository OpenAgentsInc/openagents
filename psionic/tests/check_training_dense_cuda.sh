#!/usr/bin/env bash
# Run only the new dense kernels from an authenticated generated bundle.
# Usage: PSIONIC_NVRTC=/exact/libnvrtc.so check_training_dense_cuda.sh BUNDLE NEW_OUT
# Bundle manifest records its original Coax producer and current source hashes;
# this driver verifies retained bytes, compiles with the local C compiler and
# runs the actual CUDA kernels. It never starts training or stops another job.
set -euo pipefail
python3 - "$1" "$2" "$0" <<'PY'
import hashlib,json,os,pathlib,platform,shutil,signal,subprocess,sys,time
bundle=pathlib.Path(sys.argv[1]).resolve();out=pathlib.Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=False)
manifest=json.loads((bundle/'bundle.json').read_text());commands=[];start=time.monotonic();status='failed';error=None
required={'training-dense.c','training-dense.h','training-dense.cu','training-dense-native.c','training-dense-device.c','cuda-host.c','cuda-host.h'}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def inventory():
    assert set(manifest['files'])==required
    result={}
    for name in sorted(required):
        p=bundle/name;assert p.is_file() and not p.is_symlink() and p.resolve().parent==bundle
        result[name]=sha(p);assert result[name]==manifest['files'][name]
    return result
before=inventory();driver=pathlib.Path(sys.argv[3]).resolve();assert sha(driver)==manifest['driver_sha256'];cc=pathlib.Path(shutil.which(os.environ.get('COAX_CC','cc'))).resolve();tools={str(p):sha(p) for p in [cc,pathlib.Path(sys.executable).resolve(),driver]}
nvrtc=os.environ.get('PSIONIC_NVRTC')
if nvrtc:tools[str(pathlib.Path(nvrtc).resolve())]=sha(pathlib.Path(nvrtc).resolve())
def run(name,args):
    t=time.monotonic();p=subprocess.Popen([str(x) for x in args],cwd=bundle,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    try:a,b=p.communicate(timeout=830)
    except BaseException as exc:
        try:os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        a,b=p.communicate();(out/(name+'.stdout')).write_bytes(a);(out/(name+'.stderr')).write_bytes(b);commands.append(dict(name=name,args=list(map(str,args)),returncode=p.returncode,interrupted=type(exc).__name__,seconds=time.monotonic()-t));raise
    (out/(name+'.stdout')).write_bytes(a);(out/(name+'.stderr')).write_bytes(b);commands.append(dict(name=name,args=list(map(str,args)),returncode=p.returncode,seconds=time.monotonic()-t))
    if p.returncode:raise RuntimeError(name+' failed; raw logs retained')
    return a

def idle(name):
    raw=run(name,['nvidia-smi','--query-gpu=memory.used,utilization.gpu','--format=csv,noheader,nounits']).decode().strip().splitlines();assert len(raw)==1
    memory,util=map(int,raw[0].split(','));assert memory<=512 and util<=5,'another GPU workload is present; no launch admitted'
try:
    assert platform.system()=='Linux'
    idle('before-build')
    flags=['-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-Wall','-Wextra','-Werror','-Wno-unknown-pragmas','-Wno-unused-function','-Wno-unused-parameter','-Wno-missing-field-initializers','-Wno-misleading-indentation']
    binary=out/'training-dense-device'
    run('compile',[cc,*flags,'-I'+str(bundle),bundle/'training-dense.c',bundle/'cuda-host.c',bundle/'training-dense-device.c','-ldl','-lm','-o',binary])
    idle('before-launch');run('device',[binary,bundle/'training-dense.cu'])
    sanitizer=shutil.which(os.environ.get('COMPUTE_SANITIZER','compute-sanitizer'))
    if sanitizer:run('memcheck',[sanitizer,'--tool','memcheck','--error-exitcode','99',binary,bundle/'training-dense.cu'])
    else:(out/'memcheck.unavailable').write_text('compute-sanitizer unavailable; no device memcheck claim\n')
    run('after-device',['nvidia-smi','--query-gpu=name,memory.used,memory.total,utilization.gpu','--format=csv,noheader'])
    status='passed'
except BaseException as exc:error=type(exc).__name__+': '+str(exc)
after=inventory();tools_after={p:sha(pathlib.Path(p)) for p in tools}
if after!=before or tools_after!=tools:status='failed';error='source or local producer changed during verification'
receipt=dict(schema=1,status=status,error=error,scope='Actual CUDA dense forward and both operand VJPs, exact scalar oracle and buffer guards. No resident optimizer/whole step, BF16, decoder training, quality or throughput qualification.',machine=platform.node(),platform=platform.platform(),bundle_manifest=manifest,bundle_before=before,bundle_after=after,tools_before=tools,tools_after=tools_after,commands=commands,seconds=time.monotonic()-start,artifacts={str(p.relative_to(out)):dict(bytes=p.stat().st_size,sha256=sha(p)) for p in sorted(out.rglob('*')) if p.is_file()})
(out/'qualification.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(dict(status=status,receipt=str(out/'qualification.json'),error=error)))
if status!='passed':sys.exit(1)
PY

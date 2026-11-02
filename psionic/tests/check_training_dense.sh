#!/usr/bin/env bash
# Focused native oracle and generated CUDA inspection for the reusable dense
# training primitive. Does not repeat pure checks/mutation or launch a GPU.
# Usage: COAX=/path/to/coax COAX_CC=clang check_training_dense.sh NEW_OUT_DIR
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
out=${1:?usage: check_training_dense.sh NEW_OUT_DIR}
coax=${COAX:-$root/coax/bin/coax}
cc=${COAX_CC:-cc}
python3 - "$root" "$out" "$coax" "$cc" <<'PY'
import hashlib,json,os,pathlib,re,shutil,signal,subprocess,sys,time
root=pathlib.Path(sys.argv[1]);out=pathlib.Path(sys.argv[2]).resolve();out.mkdir(parents=True,exist_ok=False)
def resolve(value):
    found=shutil.which(value)
    if not found:raise ValueError('executable not found: '+value)
    return pathlib.Path(found).resolve()
coax=resolve(sys.argv[3]);cc=resolve(sys.argv[4]);commands=[]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def inventory():
    files={};todo=[root/'psionic/lib/device/training-dense.coax']
    while todo:
        p=todo.pop().resolve()
        if p in files:continue
        p.relative_to(root);files[p]=p.read_bytes()
        for path in re.findall(r'#load\s+"([^"]+)"',files[p].decode()):todo.append(p.parent/path)
    for p in [root/'psionic/tests/training-dense-native.c',root/'psionic/tests/check_training_dense.sh',root/'psionic/examples/training-dense.coax',root/'psionic/fixtures/training-dense.out']:files[p]=p.read_bytes()
    return {str(p.relative_to(root)):dict(bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for p,b in sorted(files.items())}
before=inventory();tools={str(p):digest(p) for p in [coax,cc]};start=time.monotonic();status='failed';error=None
def run(name,args):
    t=time.monotonic();p=subprocess.Popen([str(x) for x in args],cwd=root,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    try:stdout,stderr=p.communicate(timeout=830)
    except BaseException as exc:
        try:os.killpg(p.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        stdout,stderr=p.communicate();(out/(name+'.stdout')).write_bytes(stdout);(out/(name+'.stderr')).write_bytes(stderr);commands.append(dict(name=name,args=list(map(str,args)),returncode=p.returncode,seconds=time.monotonic()-t,interrupted=type(exc).__name__));raise
    (out/(name+'.stdout')).write_bytes(stdout);(out/(name+'.stderr')).write_bytes(stderr);commands.append(dict(name=name,args=list(map(str,args)),returncode=p.returncode,seconds=time.monotonic()-t))
    if p.returncode:raise RuntimeError(name+' failed; raw logs retained')
    return stdout
try:
    example=run('example',[coax,'run','psionic/examples/training-dense.coax'])
    assert example==(root/'psionic/fixtures/training-dense.out').read_bytes()
    generated=run('emit-c',[coax,'c','psionic/lib/device/training-dense.coax','--library','--release','--header',out/'training-dense.h']);(out/'training-dense.c').write_bytes(generated)
    cuda=run('emit-cuda',[coax,'cuda','psionic/lib/device/training-dense-kernels.coax','--release']);(out/'training-dense.cu').write_bytes(cuda)
    assert b'__shared__' in cuda and b'__syncthreads' in cuda
    for name in [b'gdk_forward',b'gdk_dx',b'gdk_dw']:assert name in cuda
    flags=['-std=c11','-ffp-contract=off','-fno-fast-math','-Wall','-Wextra','-Werror','-Wno-unknown-pragmas','-Wno-unused-function','-Wno-unused-parameter','-Wno-missing-field-initializers','-Wno-misleading-indentation']
    for mode,extra in [('O0',['-O0']),('O2',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-fno-sanitize-recover=all'])]:
        binary=out/('native-'+mode)
        run('compile-'+mode,[cc,*flags,*extra,'-I'+str(out),out/'training-dense.c',root/'psionic/tests/training-dense-native.c','-lm','-o',binary])
        run('native-'+mode,[binary])
    status='passed'
except BaseException as exc:error=type(exc).__name__+': '+str(exc)
after=inventory();tools_after={str(p):digest(p) for p in [coax,cc]}
if before!=after or tools!=tools_after:status='failed';error='source or producer changed during qualification'
receipt=dict(schema=1,status=status,error=error,scope='CPU execution of actual Coax-generated dense forward and both operand gradients against independent scalar arithmetic; generated CUDA inspected, not compiled by NVRTC or executed. No resident-step, GPU numerical, throughput or decoder-training qualification.',source_before=before,source_after=after,source_unchanged=before==after,tools_before=tools,tools_after=tools_after,tools_unchanged=tools==tools_after,commands=commands,seconds=time.monotonic()-start,artifacts={str(p.relative_to(out)):dict(bytes=p.stat().st_size,sha256=digest(p)) for p in sorted(out.rglob('*')) if p.is_file()})
(out/'qualification.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(dict(status=status,receipt=str(out/'qualification.json'),error=error)))
if status!='passed':sys.exit(1)
PY

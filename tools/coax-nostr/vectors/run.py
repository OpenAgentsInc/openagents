#!/usr/bin/env python3
"""Optional native vector tooling; never an app/compiler build dependency."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import shutil
import resource
import struct
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
CORPUS=ROOT/'coax/tests/vectors'
MAX_BYTES=65536
PLANNED=('sha256','hmac','hkdf','pbkdf2','chacha20','schnorr','ecdh','nip44','nip59','event','bech32','jcs')

def sha(data):return hashlib.sha256(data).hexdigest()

def read_rsp(path):
    data=path.read_bytes()
    if len(data)>1048576:raise ValueError('vector source exceeds reader budget')
    fields={};cases=[]
    for line in data.decode('ascii').splitlines()+['']:
        line=line.strip()
        if not line:
            if not fields:continue
            if set(fields)!= {'Len','Msg','MD'}:raise ValueError('incomplete vector row')
            if not re.fullmatch(r'\d+',fields['Len']):raise ValueError('invalid bit length')
            bits=int(fields['Len']);msg=bytes.fromhex(fields['Msg']);expected=bytes.fromhex(fields['MD'])
            if bits==0 and msg==b'\0':msg=b''
            if bits<0 or bits%8 or bits//8!=len(msg) or len(msg)>MAX_BYTES or len(expected)!=32:raise ValueError('invalid vector lengths')
            cases.append((f'{path.name}:{len(cases)}',msg,expected));fields={}
            if len(cases)>4096:raise ValueError('vector row budget exceeded')
        elif line.startswith('#') or line.startswith('['):continue
        else:
            key,sep,value=line.partition('=');key=key.strip()
            if not sep or key in fields:raise ValueError('duplicate or malformed vector field')
            fields[key]=value.strip()
    if not cases:raise ValueError('empty vector source')
    return cases

def records(cases):
    return b''.join(struct.pack('>I',len(msg))+msg+expected for _,msg,expected in cases)

def build(adapter,where,mode='checked',sanitize=True):
    compiler=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
    pinned=where/'coax';shutil.copyfile(compiler,pinned);pinned.chmod(0o755)
    compiler=pinned
    deps=json.loads(subprocess.check_output([str(compiler),'deps',str(adapter.resolve())],text=True))
    inputs=[Path(p) for category in ('sources','descriptors') for p in deps[category]]
    def input_hashes():
        return {str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else str(p):sha(p.read_bytes()) for p in inputs}
    before=input_hashes()
    args=[str(compiler),'c',str(adapter.resolve()),'--library','--header',str(where/'vector.h')]
    if mode=='release':args.append('--release')
    p=subprocess.run(args,capture_output=True,timeout=60)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    if input_hashes()!=before:raise RuntimeError('adapter dependency changed during generation')
    (where/'vector.c').write_bytes(p.stdout)
    cc=os.environ.get('CC','clang')
    flags=['-std=c11','-O1' if sanitize else '-O2','-g']
    if sanitize:flags+=['-fsanitize=address,undefined','-fno-sanitize-recover=all']
    command=[cc,*flags,str(where/'vector.c'),str(HERE/'driver.c'),'-I',str(where),'-lm','-o',str(where/'driver')]
    subprocess.run(command,check=True,capture_output=True,timeout=60)
    return where/'driver',dict(compiler_sha256=sha(compiler.read_bytes()),compiler_version=subprocess.check_output([str(compiler),'version'],text=True).strip(),adapter_sha256=sha(adapter.read_bytes()),generated_c_sha256=sha(p.stdout),driver_source_sha256=sha((HERE/'driver.c').read_bytes()),binary_sha256=sha((where/'driver').read_bytes()),flags=flags,mode=mode,sanitized=sanitize,dependencies=before,c_compiler=subprocess.check_output([cc,'--version'],text=True).splitlines()[0])

def execute(binary,cases):
    def limits():
        resource.setrlimit(resource.RLIMIT_CORE,(0,0))
        resource.setrlimit(resource.RLIMIT_FSIZE,(1048576,1048576))
    with tempfile.TemporaryFile() as output_file,tempfile.TemporaryFile() as error_file:
        p=subprocess.run([str(binary)],input=records(cases),stdout=output_file,stderr=error_file,timeout=60,preexec_fn=limits)
        output_file.seek(0);error_file.seek(0)
        output=output_file.read(1048576).decode(errors='replace');error=error_file.read(1048576).decode(errors='replace')
    if p.returncode:
        m=re.search(r'row (\d+)',error)
        identity=cases[int(m[1])][0] if m and int(m[1])<len(cases) else 'transport'
        raise RuntimeError(f'{identity}: {error.strip()} (exit {p.returncode})')
    lines=output.splitlines()
    if lines!=[f'ok {i}' for i in range(len(cases))]+[f'passed {len(cases)}']:raise RuntimeError('incomplete driver result')
    return len(cases)

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--suite',default='fingerprint-sha256')
    ap.add_argument('--adapter',type=Path)
    ap.add_argument('--mode',choices=['checked','release'],default='checked')
    ap.add_argument('--optimized',action='store_true',help='O2 without sanitizers; default is O1 ASan/UBSan')
    ap.add_argument('--report',type=Path)
    ap.add_argument('--list',action='store_true')
    args=ap.parse_args()
    if args.list:
        print(json.dumps({'fingerprint-sha256':'test fixture','sha256':'requires explicit Coax adapter',**{s:'not implemented' for s in PLANNED if s!='sha256'}},indent=2));return
    if args.suite not in ('fingerprint-sha256','sha256'):ap.error('suite not implemented; no skipped-success result')
    if args.suite=='sha256' and args.adapter is None:ap.error('sha256 requires an explicit implementation adapter')
    adapter=args.adapter or HERE/'fingerprint.coax'
    if not adapter.is_file():ap.error('adapter must be an explicit local Coax source')
    # Verify exact upstream input/provenance before parsing any vectors. No fetch.
    subprocess.run([sys.executable,str(CORPUS/'check_vectors.py')],check=True,capture_output=True,timeout=30)
    files=[CORPUS/'upstream/nist/SHA256ShortMsg.rsp',CORPUS/'upstream/nist/SHA256LongMsg.rsp']
    cases=[case for path in files for case in read_rsp(path)]
    with tempfile.TemporaryDirectory(prefix='coax-vectors-') as tmp:
        binary,evidence=build(adapter,Path(tmp),args.mode,not args.optimized)
        count=execute(binary,cases)
        evidence.update(schema=1,case_ids=[c[0] for c in cases],records_sha256=sha(records(cases)),suite=args.suite,scope='native vector execution; fingerprint fixture is not production crypto qualification',rows=count,sources={str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in files},status='passed')
        if args.report:args.report.write_text(json.dumps(evidence,indent=2)+'\n')
        print(f'{args.suite}: {count} rows passed ({args.mode}, '+('O2' if args.optimized else 'ASan/UBSan')+')')
if __name__=='__main__':
    try:main()
    except (ValueError,RuntimeError,subprocess.SubprocessError) as e:print(str(e),file=sys.stderr);sys.exit(1)

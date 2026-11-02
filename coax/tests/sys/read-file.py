#!/usr/bin/env python3
# Firm reason: 2 (independent compiler, ABI, diagnostic and native-effect oracles; no product implementation).
"""Real R2 file adapter: generated-header composition and OS/fault observations.
Python is test tooling only. Each command has an 840 second bound; outputs stay.
Supply a newly built R2 compiler explicitly; no seed/package is overwritten.
"""
import argparse,hashlib,json,os,signal,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
p=argparse.ArgumentParser();p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--profile',choices=['O0','O2','sanitized'],action='append');a=p.parse_args()
compiler=a.compiler.resolve();out=a.output.resolve();out.mkdir(parents=True,exist_ok=False)
cc=os.environ.get('CC','cc');profiles=a.profile or ['O0','O2','sanitized'];records=[]
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def command(argv,name,success=0,env=None,empty_stderr=False):
    directory=out/name;directory.mkdir();started=time.time();timed_out=False
    with (directory/'stdout.log').open('wb') as so,(directory/'stderr.log').open('wb') as se:
        proc=subprocess.Popen(list(map(str,argv)),cwd=ROOT,stdout=so,stderr=se,start_new_session=True,env=env)
        try:code=proc.wait(timeout=840)
        except subprocess.TimeoutExpired:
            timed_out=True;os.killpg(proc.pid,signal.SIGTERM)
            try:proc.wait(timeout=5)
            except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait()
            code=proc.returncode
    row={'name':name,'argv':list(map(str,argv)),'cwd':str(ROOT),'exit_code':code,'timed_out':timed_out,'elapsed_seconds':time.time()-started,'stdout_sha256':digest(directory/'stdout.log'),'stderr_sha256':digest(directory/'stderr.log'),'stderr_empty':(directory/'stderr.log').stat().st_size==0,'requires_empty_stderr':empty_stderr}
    (directory/'process.json').write_text(json.dumps(row,indent=2)+'\n');records.append(row)
    if timed_out or (code!=success if success is not None else code==0) or (empty_stderr and not row['stderr_empty']):raise RuntimeError('command failed: '+name)
    return (directory/'stdout.log').read_bytes()
source_paths=['coax/lib/sys/file-policy.coax','coax/lib/sys/read-file.coax','coax/platform/read-file-native.c','coax/platform/read-file-native.h','coax/tests/sys/read-file-fixture.coax','coax/tests/sys/read-file-native.c','coax/tests/sys/read-file-faults.h','coax/tests/sys/read-file.py']
pins={path:digest(ROOT/path) for path in source_paths};binary_hash=digest(compiler);completed=False
try:
    generated=command([compiler,'c',ROOT/'coax/tests/sys/read-file-fixture.coax','--library','--header',out/'read-file-fixture.h'],'emit-fixture')
    (out/'read-file-fixture.c').write_bytes(generated)
    for order in ['native-first','generated-first']:
        includes=['read-file-native.h','read-file-fixture.h'];
        if order=='generated-first':includes.reverse()
        source=out/(order+'.c');source.write_text('#include <stddef.h>\n'+''.join('#include "'+s+'"\n' for s in includes)+'_Static_assert(sizeof(coax_v_u8)==16 && sizeof(coax_v_s64)==16,"canonical numeric views");\n_Static_assert(offsetof(coax_v_u8,count)==8 && offsetof(coax_v_s64,count)==8,"canonical count offsets");\nint main(void){return 0;}\n')
        command([cc,'-std=c11','-Wall','-Wextra','-Werror','-I',out,'-I',ROOT/'coax/platform',source,'-o',out/order],'layout-'+order)
        command([out/order],'layout-run-'+order,empty_stderr=True)
    for view in ['u8','s64']:
        source=out/('mismatch-'+view+'.c');source.write_text('#define COAX_coax_v_'+view+'_DEFINED\n#include "read-file-native.h"\n')
        command([cc,'-std=c11','-I',ROOT/'coax/platform','-fsyntax-only',source],'layout-refuses-'+view,success=None)
        if ('incompatible Coax C type coax_v_'+view).encode() not in (out/('layout-refuses-'+view)/'stderr.log').read_bytes():raise RuntimeError('wrong header-composition refusal: '+view)
    for profile in profiles:
        flags=['-O0'] if profile=='O0' else ['-O2'] if profile=='O2' else ['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer']
        obj=out/('native-'+profile+'.o');binary=out/('read-file-'+profile)
        command([cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-include',ROOT/'coax/tests/sys/read-file-faults.h','-DCXFR_OPEN=fr_test_open','-DCXFR_STAT=fr_test_fstat','-DCXFR_READ=fr_test_read','-DCXFR_CLOSE=fr_test_close','-c',ROOT/'coax/platform/read-file-native.c','-o',obj],'compile-native-'+profile)
        command([cc,'-std=c11',*flags,'-I',out,'-I',ROOT/'coax/platform',out/'read-file-fixture.c',ROOT/'coax/tests/sys/read-file-native.c',obj,'-lm','-o',binary],'link-'+profile)
        command([binary],'run-'+profile,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'} if profile=='sanitized' else None,empty_stderr=True)
    completed=True
finally:
    unchanged=pins=={path:digest(ROOT/path) for path in source_paths} and binary_hash==digest(compiler)
    result={'schema':'coax.read-file.native.v1','complete':completed,'inputs_unchanged':unchanged,'compiler':str(compiler),'compiler_sha256':binary_hash,'profiles':profiles,'source_pins':pins,'commands':records,'scope':'24 real OS / explicitly injected fault cases per profile; canonical numeric-view header composition in both orders; incompatible guards refused; no browser or legacy fs_read rerun'}
    (out/'record.json').write_text(json.dumps(result,indent=2)+'\n')
    if completed and not unchanged:raise RuntimeError('inputs changed')

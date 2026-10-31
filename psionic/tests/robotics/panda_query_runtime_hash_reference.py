#!/usr/bin/env python3
"""NEW streaming wrapper vs independent hashlib; no old crypto/model gate."""
from pathlib import Path
import argparse,hashlib,json,os,struct,subprocess
ROOT=Path(__file__).resolve().parents[3]
def require(ok,msg):
    if not ok:raise RuntimeError(msg)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--reuse-partial',type=Path);a=p.parse_args();out=a.output.resolve();require(not out.exists(),'preserve proof');out.mkdir(parents=True);lengths=[0,1,63,64,65,1048575,1048576,1048577,2097169,16777216];expected=[]
    if a.reuse_partial:
        prior=json.loads((a.reuse_partial/'record.json').read_text());require(not prior['passed'] and prior['lengths']==lengths,'reuse partial context');raw=(a.reuse_partial/'input.bin').read_bytes();require(hashlib.sha256(raw).hexdigest()==prior['input_sha256'],'cached independent vectors');(out/'input.bin').write_bytes(raw);expected=[bytes.fromhex(x)for x in prior['expected_hex']]
    else:
        data=bytes(range(256))*65536;expected=[hashlib.sha256(data[:n]).digest()for n in lengths];(out/'input.bin').write_bytes(struct.pack('<II',0x31485150,10)+struct.pack('<10Q',*lengths)+b''.join(expected)+data)
    r={'schema':'psionic.panda-query-runtime.hash-qualification.v1','passed':False,'compiler_sha256':sha(a.compiler),'source_sha256':{str(p.relative_to(ROOT)):sha(p)for p in [Path(__file__).resolve(),ROOT/'psionic/tests/robotics/panda_query_runtime_hash_native.c',ROOT/'psionic/tests/robotics/panda-query-runtime-hash.coax']},'lengths':lengths,'expected_hex':[x.hex()for x in expected],'input_sha256':sha(out/'input.bin'),'commands':[],'modes':[],'SDK_API_calls':0,'NN_calls':0};record=out/'record.json'
    def save():record.write_text(json.dumps(r,indent=2)+'\n')
    def run(name,argv):
        x=subprocess.run([str(x)for x in argv],cwd=ROOT,capture_output=True,timeout=300);(out/(name+'.stdout')).write_bytes(x.stdout);(out/(name+'.stderr')).write_bytes(x.stderr);r['commands'].append({'name':name,'argv':[str(x)for x in argv],'exit':x.returncode,'stderr_bytes':len(x.stderr)});save();require(not x.returncode and not x.stderr,(name,x.stdout,x.stderr));return x.stdout
    try:
        for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();source=d/'panda-session.c';header=d/'panda-session.h'
            if mode=='sanitized':
                source.write_bytes((out/'debug/panda-session.c').read_bytes());header.write_bytes((out/'debug/panda-session.h').read_bytes())
            elif a.reuse_partial and(a.reuse_partial/mode/'panda-session.c').is_file():
                source.write_bytes((a.reuse_partial/mode/'panda-session.c').read_bytes());header.write_bytes((a.reuse_partial/mode/'panda-session.h').read_bytes());r['reused_partial_emission']={'source':str(a.reuse_partial),'receipt_sha256':sha(a.reuse_partial/'record.json'),'scope':'Test-tool SHA status constants corrected; unchanged product emission reused, no passed numerical mode previously existed.'}
            else:source.write_bytes(run('emit-'+mode,[a.compiler,'c','psionic/tests/robotics/panda-query-runtime.coax','--library',*(['--release']if mode=='release'else[]),'--header',header]))
            binary=d/'native';run('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-ffp-contract=off','-fno-fast-math','-fno-builtin-sin','-fno-builtin-cos','-I'+str(d),source,ROOT/'psionic/tests/robotics/panda_query_runtime_hash_native.c','-lm','-o',binary]);native=json.loads(run('run-'+mode,[binary,out/'input.bin']));r['modes'].append({'mode':mode,'native':native,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary)});save()
        r['passed']=True
    except BaseException as e:r['failure']=repr(e);raise
    finally:save()
    print(json.dumps({'passed':True,'record':str(record)}))
if __name__=='__main__':main()

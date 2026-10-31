#!/usr/bin/env python3
"""Independent fixed-source metadata/bitflip oracle. No SDK or NN calls."""
from pathlib import Path
import argparse,hashlib,json,os,re,struct,subprocess,time
ROOT=Path(__file__).resolve().parents[3]
def require(ok,msg):
    if not ok:raise RuntimeError(msg)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--scene',type=Path,required=True);a=p.parse_args();out=a.output.resolve();require(not out.exists(),'preserve proof');out.mkdir(parents=True);xml=a.scene.read_bytes();require(hashlib.sha256(xml).hexdigest()=='95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f','original pinned scene');public=ROOT/'psionic/fixtures/robotics/libero-panda/acquisition.json';manifest=json.loads(public.read_text());lookup={x['sha256']:x for x in manifest['assets']};seen=set();rows=[]
    for m in re.finditer(rb'\bfile="([^\"]+)"',xml):
        path=m.group(1)
        if path in seen:continue
        seen.add(path);digest=sha(Path(path.decode()));require(digest in lookup,'original public asset metadata');old=lookup[digest];require(Path(path.decode()).stat().st_size==old['bytes'],'asset extent');rows.append((m.start(1),len(path),digest))
    require(len(rows)==85 and len(seen)==85,'complete unique asset inventory');profile=ROOT/'psionic/fixtures/robotics/panda-query-runtime/asset-profile.json';frozen=json.loads(profile.read_text());require([(x['offset'],x['count'],x['sha256']) for x in frozen['assets']]==rows and frozen['source_manifest_sha256']==sha(public),'metadata profile independent original source binding')
    ids=['95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f','e1c2d3ee5192158b0f896456c1e312ea8ed5eef001862cae3d8c551ffb26eddc','f8c9735ed948d6830d1d3d31d50216ec14ca6a6e95c7346a4dc8300e1b8ffed4'];data=struct.pack('<II',0x314e5150,len(xml))+xml+b''.join(bytes.fromhex(x) for x in ids)+b''.join(bytes.fromhex(x[2]) for x in rows)+b''.join(struct.pack('<qq',x[0],x[1]) for x in rows);(out/'input.bin').write_bytes(data)
    sources=[Path(__file__),ROOT/'psionic/tests/robotics/panda_query_runtime_native.c',*sorted((ROOT/'psionic/tests/robotics').glob('panda-query-runtime*.coax'))];source_hashes={str(p.relative_to(ROOT)):sha(p) for p in sources};r={'schema':'psionic.panda-query-runtime.native-qualification.v1','passed':False,'compiler_sha256':sha(a.compiler),'source_sha256':source_hashes,'public_asset_manifest_sha256':sha(public),'asset_profile_sha256':sha(profile),'input_sha256':sha(out/'input.bin'),'SDK_calls':0,'NN_calls':0,'modes':[],'commands':[]};record=out/'record.json'
    def save():record.write_text(json.dumps(r,indent=2)+'\n')
    def run(name,args):
        start=time.monotonic();x=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(x.stdout);(out/(name+'.stderr')).write_bytes(x.stderr);r['commands'].append({'name':name,'argv':[str(x) for x in args],'exit':x.returncode,'seconds':time.monotonic()-start,'stderr_bytes':len(x.stderr)});save();require(x.returncode==0 and not x.stderr,(name,x.stdout,x.stderr));return x.stdout
    save()
    try:
        for mode,args,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();source=d/'panda-session.c';header=d/'panda-session.h';source.write_bytes(run('emit-'+mode,[a.compiler,'c','psionic/tests/robotics/panda-query-runtime.coax','--library',*args,'--header',header]));binary=d/'native';run('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-ffp-contract=off','-fno-fast-math','-fno-builtin-sin','-fno-builtin-cos','-I'+str(d),source,ROOT/'psionic/tests/robotics/panda_query_runtime_native.c','-lm','-o',binary]);native=json.loads(run('run-'+mode,[binary,out/'input.bin']));r['modes'].append({'mode':mode,'native':native,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary)});save()
        require(source_hashes=={str(p.relative_to(ROOT)):sha(p) for p in sources},'source frozen');r['passed']=True
    except BaseException as e:r['failure']=repr(e);raise
    finally:save()
    print(json.dumps({'passed':True,'record':str(record)}))
if __name__=='__main__':main()

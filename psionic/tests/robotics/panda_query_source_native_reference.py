#!/usr/bin/env python3
"""NEW selector only: fixed bit transport, prior SDK/owned sample references."""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
def require(ok,msg):
    if not ok:raise RuntimeError(msg)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();require(not out.exists(),'preserve proof');out.mkdir(parents=True);compiler=a.compiler.resolve();require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified target')
    sdk=ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics/sdk-pc-samples.f64';owned=ROOT/'psionic/fixtures/robotics/panda-owned-controller/owned-samples.f64';cases=ROOT/'psionic/fixtures/robotics/panda-inverse-dynamics/cases.f64';require(sha(sdk)=='29eff7a1f40f08a4c60ed57558706e744181c396045701d53f968779866f5abc' and sha(owned)=='1443b1519e20474a00463a29b423e335e82312a567b3f7913eab284de0746ba7' and sha(cases)=='684c892c72d4e6341e695c1b7bae22a5d5e7d01e3310f1349b98c5c0c8271b49','qualified input identity');s=struct.unpack('<2160d',sdk.read_bytes());o=struct.unpack('<2160d',owned.read_bytes());qva=struct.unpack('<648d',cases.read_bytes());require(all(math.isfinite(x) for x in s+o+qva),'all reference/input finite')
    data=bytearray(struct.pack('<II',0x31535150,15));expected=bytearray()
    for row in range(15):
        sample=s[row*144:(row+1)*144];raw=[0.]*173
        for src,dst,n in [(0,106,3),(3,109,9),(12,1,7),(19,8,7),(26,15,49),(75,64,21),(96,85,21),(117,118,6),(123,134,7),(130,141,7),(137,148,7)]:raw[dst:dst+n]=sample[src:src+n]
        data+=struct.pack('<173d',*raw)+struct.pack('<18d',*qva[row*27:row*27+18]);expected+=struct.pack('<144d',*sample)+struct.pack('<144d',*o[row*144:(row+1)*144])
    (out/'input.bin').write_bytes(data);sources=list(sorted((ROOT/'psionic/lib/robotics').glob('panda-query-source*.coax')))+[ROOT/'psionic/tests/robotics/panda_query_source_native.c',Path(__file__)];frozen={str(x.relative_to(ROOT)):sha(x) for x in sources};r={'schema':'psionic.panda-query-source.native-qualification.v1','passed':False,'compiler_sha256':sha(compiler),'source_sha256':frozen,'input_sha256':sha(out/'input.bin'),'expected_sha256':hashlib.sha256(expected).hexdigest(),'cases':15,'output_words':4320,'SDK_calls':0,'modes':[],'commands':[],'scope':'NEW selector fixture transport. The173word SDK testinput is reconstructed from independent144field SDK data with explicit zero unused lanes, not a captured fulltelemetry record.'}
    def save():(out/'record.json').write_text(json.dumps(r,indent=2)+'\n')
    def command(name,argv):
        x=subprocess.run([str(z) for z in argv],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(x.stdout);(out/(name+'.stderr')).write_bytes(x.stderr);r['commands'].append({'name':name,'argv':[str(z) for z in argv],'exit':x.returncode,'stderr_bytes':len(x.stderr)});save();require(x.returncode==0 and not x.stderr,(name,x.stdout.decode(errors='replace'),x.stderr.decode(errors='replace')));return x.stdout
    began=time.monotonic();save()
    try:
        blobs=[]
        for mode,extra,flags in [('debug',[],['-O0','-g']),('release',['--release'],['-O2']),('sanitized',[],['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            d=out/mode;d.mkdir();header=d/'panda-query-source.h';source=d/'panda-query-source.c';source.write_bytes(command('emit-'+mode,[compiler,'c','psionic/lib/robotics/panda-query-source.coax','--library',*extra,'--header',header]));binary=d/'native';command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(d),source,ROOT/'psionic/tests/robotics/panda_query_source_native.c','-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'sources.f64']));actual=(d/'sources.f64').read_bytes();require(len(actual)==4320*8 and all(math.isfinite(x) for x in struct.unpack('<4320d',actual)),'all actual finite/extent');require(actual==expected,'SDK mapping and owned selected fields bitexact');blobs.append(actual);r['modes'].append({'mode':mode,'native':native,'C_sha256':sha(source),'header_sha256':sha(header),'binary_sha256':sha(binary),'output_sha256':sha(d/'sources.f64'),'all4320words_bitexact':True});save()
        require(blobs[0]==blobs[1]==blobs[2] and frozen=={str(x.relative_to(ROOT)):sha(x) for x in sources},'same profile/source');r['passed']=True
    except BaseException as e:r['failure']=repr(e);raise
    finally:r['seconds']=time.monotonic()-began;save()
    print(json.dumps({'passed':True,'record':str(out/'record.json'),'seconds':r['seconds']}))
if __name__=='__main__':main()

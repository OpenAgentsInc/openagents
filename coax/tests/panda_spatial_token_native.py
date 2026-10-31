#!/usr/bin/env python3
"""One NEW actual Panda source-token witness; no old mutations/SDK/NN gates."""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,struct,subprocess,time
ROOT=Path(__file__).resolve().parents[2]
ATOL=2e-12
def require(ok,message):
    if not ok:raise RuntimeError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--frontend-build',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--prepared-unity',type=Path);args=parser.parse_args()
    build=args.frontend_build.resolve();out=args.output.resolve();require(not out.exists(),'output exists; retain prior evidence');out.mkdir(parents=True)
    fixture=ROOT/'coax/tests/spatial/panda-shift-consumer.coax';unity=ROOT/'coax/tests/panda-spatial-token-native.cpp';native=ROOT/'coax/tests/panda-spatial-token-oracle.c'
    inputs=[unity,native,Path(__file__).resolve(),fixture]+sorted((ROOT/'psionic/lib/robotics').glob('panda-fk-*.coax'))+[ROOT/'coax/lib/quantity.coax']
    frontend=sorted(p for folder in ('coax/src','coax/lib/compiler','coax/bootstrap') for p in (ROOT/folder).rglob('*') if p.is_file() and p.suffix in ('.cpp','.h','.c','.inc','.coax'))
    record={'schema':'coax.panda-spatial-token.native.v1','passed':False,'platform':platform.platform(),'machine':platform.machine(),'atol_frozen_before_execution':ATOL,'rtol':0,'sources_sha256':{str(p.relative_to(ROOT)):sha(p) for p in inputs},'frontend_sources_sha256':{str(p.relative_to(ROOT)):sha(p) for p in frontend},'frontend_binary_sha256':sha(build/'coax'),'frontend_inputs_sha256':{name:sha(build/name) for name in ('proto_importer.o','proto_importer.h','proto_runtime.inc')},'profiles':[],'commands':[],'scope':'Exactly one actual Mutator spatial-token site in unchanged dependency pfk_shift; only the NEW consumer check executes. Ordinary sites and old dependency checks are not executed. C++ seam does compiler effects only; independent zero-angle C endpoint constants plus source-derived position-negation/unchanged-attitude relations. No old numeric/SDK/NN or 195/281 mutation runs; no full compiler release qualification.'}
    def save():(out/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
    def command(name,items,directory=out):
        result=subprocess.run([str(x) for x in items],cwd=ROOT,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'})
        (directory/(name+'.stdout')).write_bytes(result.stdout);(directory/(name+'.stderr')).write_bytes(result.stderr)
        record['commands'].append({'name':name,'exit_status':result.returncode,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest(),'stderr_bytes':len(result.stderr)})
        require(result.returncode==0 and not result.stderr,(name,result.returncode,result.stderr.decode(errors='replace')));return result.stdout
    began=time.monotonic();save()
    try:
        executable=out/'unity-probe'
        if args.prepared_unity:
            supplied=args.prepared_unity.resolve();require(supplied.is_file(),'missing prepared unity binary');executable.write_bytes(supplied.read_bytes());executable.chmod(0o755)
            record['prepared_unity_reused']=True
        else:
            command('unity-build',[os.environ.get('CXX','c++'),'-std=c++20','-O1','-g','-Wall','-Wextra','-Wno-missing-field-initializers','-I',build,unity,build/'proto_importer.o','-o',executable]);record['prepared_unity_reused']=False
        record['unity_binary_sha256']=sha(executable)
        command('apply-emit-check-undo',[executable,fixture.relative_to(ROOT),out/'emitted'])
        emission=json.loads((out/'emitted/emission.json').read_text());require(emission['passed'] and emission['selected_spatial_mutants']==emission['killed']==1 and emission['survived']==emission['old_dependency_checks_executed']==0,'one NEW killed source mutation inventory');record['actual_mutation']=emission
        variants=('baseline','mutant');record['emitted_sha256']={v:{ext:sha(out/'emitted'/v/('panda-token.'+ext)) for ext in ('c','h')} for v in variants}
        require(record['emitted_sha256']['baseline']['h']==record['emitted_sha256']['mutant']['h'],'same public header')
        profile_payloads={};position_max=0.;changed_positions=[]
        for mode,flags in [('debug',['-O0']),('optimized',['-O2']),('asan-ubsan',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
            words={};payloads={}
            for v in variants:
                d=out/mode/v;d.mkdir(parents=True);binary=d/'probe';emitted=out/'emitted'/v
                command('build-'+mode+'-'+v,[os.environ.get('CC','cc'),'-std=c11','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos',*flags,'-I',emitted,emitted/'panda-token.c',native,'-lm','-o',binary],d)
                data=command('run-'+mode+'-'+v,[binary,'1' if v=='mutant' else '0'],d);require(len(data)==208*8,'complete208word sample');words[v]=struct.unpack('<208d',data);payloads[v]=data
                require(all(math.isfinite(x) for x in words[v]),'finite numeric outputs');record['profiles'].append({'mode':mode,'variant':v,'binary_sha256':sha(binary),'output_sha256':hashlib.sha256(data).hexdigest(),'output_words':208,'native_assertions':217,'independent_endpoint_comparisons':6})
            count=0
            for i in range(13):
                for axis in range(3):
                    index=i*16+axis;error=abs(words['mutant'][index]+words['baseline'][index]);position_max=max(position_max,error);require(error<=ATOL,'source-translation negation budget');count+=words['mutant'][index]!=words['baseline'][index]
                lo=(i*16+3)*8;hi=(i*16+16)*8;require(payloads['baseline'][lo:hi]==payloads['mutant'][lo:hi],'mutation changed quaternion/matrix words')
            require(count>0,'mutant did not change observed positions');changed_positions.append(count);profile_payloads[mode]=payloads;save()
        for v in variants:require(len({profile_payloads[mode][v] for mode in profile_payloads})==1,'scalar-libm cross-profile byte identity')
        record['position_negation']={'comparisons':117,'max_abs_error':position_max,'atol':ATOL,'changed_coordinate_values_per_profile':changed_positions,'passed':True}
        record['unchanged_attitude']={'quaternion_and_matrix_words':507,'byte_exact':True,'passed':True};record['native_observed_words']=1248;record['native_independent_endpoint_comparisons']=36;record['cross_profile_bytes_identical']=True
        require(all(sha(ROOT/name)==digest for name,digest in record['sources_sha256'].items()),'consumer/product source changed during proof');require(all(sha(ROOT/name)==digest for name,digest in record['frontend_sources_sha256'].items()),'frontend source changed during proof');require(sha(build/'coax')==record['frontend_binary_sha256'],'private frontend changed');record['passed']=True
    finally:record['duration_seconds']=time.monotonic()-began;save()
    print('Panda token: 1 actual source mutant killed; 1248 native words, 117 position relations, 507 unchanged attitude words')
if __name__=='__main__':main()

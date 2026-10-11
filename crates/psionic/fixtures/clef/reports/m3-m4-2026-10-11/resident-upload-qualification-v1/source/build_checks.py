"""Compile frozen affected packages, run CPU tests, and retain executables."""
import json,os,re,shutil,subprocess,sys,time
from pathlib import Path
import proof
HERE=proof.HERE; OUT=HERE/'build-1'; ROOT=proof.ROOT
TARGET=Path('@@CLEF_TARGET@@')

def frozen():
    manifest=proof.load(HERE/'source-manifest.json')
    proof.verify_files({str(ROOT/r['path']):r['sha256'] for r in manifest['files']})
    proof.verify_files(proof.load(HERE/'qualification-source.json')['files'])
    return manifest

def run():
    proof.require({'build','memory','disk'}<=set(os.environ.get('OPENAGENTS_LEASES','').split(',')),'build leases required')
    frozen(); env=dict(os.environ)
    for key in list(env):
        if key.startswith(('PSIONIC_CLEF_','DYLD_')) or key in ('LD_PRELOAD','LD_LIBRARY_PATH'):env.pop(key)
    env.update(CARGO_TARGET_DIR=str(TARGET),CARGO_BUILD_JOBS='2',RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
    rows=[];artifacts={};outputs={}
    def command(name,args):
        frozen();begin=time.monotonic();print('Starting '+name,flush=True)
        with (OUT/(name+'.log')).open('x') as log:r=subprocess.run(args,cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT)
        rows.append({'name':name,'command':args,'exit':r.returncode,'seconds':time.monotonic()-begin})
        proof.write(OUT/'checks.json',rows);print(json.dumps(rows[-1]),flush=True)
        proof.require(r.returncode==0,'check failed: '+name);frozen()
        return (OUT/(name+'.log')).read_text()
    base=['cargo','+1.97.1']; manifest=['--release','--manifest-path','crates/psionic/Cargo.toml']
    for package in ['psionic-catalog','psionic-models','psionic-serve']:
        name=package+'-build';log=command(name,base+['test']+manifest+['-p',package,'--lib','--no-run','--message-format=json-render-diagnostics','-j','2'])
        matches=[]
        for line in log.splitlines():
            try:row=json.loads(line)
            except json.JSONDecodeError:continue
            if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')==package.replace('-','_') and row.get('profile',{}).get('test') and row.get('executable'):matches.append(row['executable'])
        proof.require(len(set(matches))==1,'ambiguous test artifact: '+package);artifacts[package]=Path(matches[0])
    specifications=[('catalog-positional','psionic-catalog','positional_reads_',4),
                    ('models-forwarding','psionic-models','gguf_blob_artifact_pages_tensor_bytes_and_reports_buffered_fallback',1),
                    ('serve-clef-cpu','psionic-serve','clef::',1),
                    ('serve-metal-cpu','psionic-serve','qwen35::clef_metal::',1)]
    for name,package,filter_,minimum in specifications:
        log=command(name,[str(artifacts[package]),filter_,'--test-threads=1'])
        matches=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',log)
        proof.require(len(matches)==1 and int(matches[0][0])>=minimum and int(matches[0][1])==0,'missing expected test coverage: '+name)
        if name.startswith(('catalog-','models-')):proof.require(int(matches[0][0])==minimum,'test count differs: '+name)
        if name=='serve-metal-cpu':
            proof.require('positional_weight_upload_is_explicit_and_resident_only ... ok' in log,'mode CPU test not run')
            proof.require('positional_weight_upload_preserves_resident_bytes_and_gemm_outputs ... ignored' in log,'GPU fixture must remain ignored')
        rows[-1].update(passed=int(matches[0][0]),ignored=int(matches[0][2]),GPU_tests_executed=False);proof.write(OUT/'checks.json',rows)
    log=command('server',base+['build']+manifest+['-p','psionic-serve','--bin','psionic-openai-server','--message-format=json-render-diagnostics','-j','2'])
    matches=[]
    for line in log.splitlines():
        try:row=json.loads(line)
        except json.JSONDecodeError:continue
        if row.get('reason')=='compiler-artifact' and row.get('target',{}).get('name')=='psionic-openai-server' and row.get('executable'):matches.append(row['executable'])
    proof.require(len(set(matches))==1,'ambiguous server artifact');artifacts['server']=Path(matches[0])
    for key,name in [('server','psionic-openai-server'),('psionic-serve','psionic-serve-tests')]:
        source=artifacts[key];destination=OUT/name;digest=proof.sha(source)
        proof.require(not destination.exists(),'refuse replacing executable');shutil.copy2(source,destination)
        proof.require(proof.sha(destination)==digest,'binary copy differs')
        outputs[key]={'path':str(destination),'sha256':digest,'bytes':destination.stat().st_size,'source':str(source)}
    frozen();proof.write(OUT/'binaries.json',{'status':'complete','source_manifest_sha256':proof.sha(HERE/'source-manifest.json'),'source_manifest_unchanged':True,'artifacts':outputs})
    return 0
if __name__=='__main__':raise SystemExit(run())

#!/usr/bin/env python3
"""NEW recomputed-controller prefix: exactly two250step SDK executions, no NN."""
from pathlib import Path
import argparse,hashlib,json,math,os,re,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3]
PROFILE=ROOT/'psionic/fixtures/robotics/panda-query-source/prefix-experiment.json'
def require(ok,msg):
    if not ok:raise RuntimeError(msg)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def identity(p):return {'bytes':Path(p).stat().st_size,'sha256':sha(p)}
def finite(data,width,rows,fmt='d'):
    size=struct.calcsize(fmt);require(len(data)==width*rows*size,'exact extent');v=struct.unpack('<'+str(width*rows)+fmt,data);require(all(math.isfinite(x) for x in v),'ALL actual/reference finite before comparisons');return [v[i*width:(i+1)*width] for i in range(rows)]
def difference(a,b):require(len(a)==len(b) and all(math.isfinite(x) for x in a+b),'difference extent/finite');return max([abs(x-y) for x,y in zip(a,b)]+[0.])
def source_inventory():
    paths={PROFILE,Path(__file__).resolve(),ROOT/'psionic/platform/panda-env.c',ROOT/'psionic/platform/panda-env.h',ROOT/'psionic/tests/robotics/panda_query_source_prefix.c'}
    pending=[ROOT/'psionic/tests/robotics/panda-query-session.coax']
    while pending:
        p=pending.pop().resolve()
        if p in paths:continue
        require(p.is_relative_to(ROOT),'owned source closure')
        paths.add(p)
        pending += [(p.parent/x).resolve() for x in re.findall(r'#load\s*"([^"]+)"',p.read_text())]
    return {str(p.relative_to(ROOT)):sha(p) for p in sorted(paths)}
def authenticate(xml,initial,trace):
    profile=json.loads(PROFILE.read_text());case=profile['case'];require(profile['frozen_before_SDK_execution'] and profile['runs']==[{'kind':0,'steps':250},{'kind':1,'steps':250}],'frozen scope')
    for p,expected in [(xml,case['files']['scene.xml']),(initial,case['files']['init.f64']),(trace,case['recorded_trace']),(trace.parent/'record.json',case['original_case_ledger']),(xml.parent/'original.xml',case['files']['original.xml'])]:require(identity(p)==expected,'authenticated source '+str(p))
    x=finite(initial.read_bytes(),45,1)[0];require(0<=x[0]<=100,'initial SDKclock');raw=trace.read_bytes();require((len(raw)-64)%512==0 and len(raw)>=64+250*512,'original PTR1 shape');actions=[]
    for i in range(250):actions.extend(struct.unpack_from('<7f',raw,32+512*i+48))
    require(all(math.isfinite(x) for x in actions),'all recorded action words finite');return profile
def read_run(prefix,mode,profile):
    r=json.loads(prefix.with_suffix('.json').read_text());require(r['schema']=='psionic-panda-query-source-prefix-v1' and r['mode']==mode and r['steps']==250 and r['requests']==3 and r['origins']==[0,25,225] and r['holds']==1 and r['stale_refusals']==1 and r['cached_observations']==3 and r['fresh_observations']==10,'complete expected schedule');require(r['controller_recomputed'] and r['SDK_geometry_retained'] and not r['NN'] and not r['render'] and not r['task_outcome_evaluated'],'scope')
    states=finite(prefix.with_suffix('.states.f64').read_bytes(),45,251);controls=finite(prefix.with_suffix('.controls.f64').read_bytes(),9,250);observations=finite(prefix.with_suffix('.observations.f32').read_bytes(),8,3,'f');records=prefix.with_suffix('.records.ptr1').read_bytes();require(len(records)==250*512,'PTR1 record extent');snap=prefix.with_suffix('.snapshots.bin').read_bytes();require(len(snap)==13*112,'snapshot extent');snapshots=[]
    cached=[];fresh=[]
    for i in range(13):
        tag=struct.unpack_from('<4q',snap,i*112);values=finite(snap[i*112+32:(i+1)*112],10,1)[0];kind,k,last,qpos=tag;require(kind in [0,1] and k%25==0 and qpos==k and last==(max(0,k-1) if kind==0 else k),'cached/fresh source stamps');(cached if kind==0 else fresh).append(k);snapshots.append((tag,values))
    require(cached==[0,25,225] and fresh==list(range(0,250,25)),'observation inventory');paired=prefix.with_suffix('.controllers.bin').read_bytes();require(len(paired)==250*(9*8+60*8),'controller event extent');events=[];maxima={'torque':0.,'gripper':0.,'PcState_goal_home_grip':0.}
    for i in range(250):
        at=i*552;meta=struct.unpack_from('<9q',paired,at);values=finite(paired[at+72:at+552],60,1)[0];require(meta[0]==i and meta[1]==meta[2]==0 and meta[3]==meta[4] and meta[5]==meta[6] and meta[7]==meta[8]==1,'all normal status/ready/clips exactly agree');a,b=values[:30],values[30:];maxima['torque']=max(maxima['torque'],difference(a[:7],b[:7]));maxima['gripper']=max(maxima['gripper'],difference(a[7:9],b[7:9]));maxima['PcState_goal_home_grip']=max(maxima['PcState_goal_home_grip'],difference(a[9:],b[9:]));require(tuple(a[:9])==tuple(controls[i]),'applied primary control only');events.append((meta,values))
    for k,v in maxima.items():require(v<=profile['same_state_shadow_budgets_abs'][k],('local shadow budget',k,v))
    return {'summary':r,'states':states,'controls':controls,'observations':observations,'records':records,'snapshots':snapshots,'events':events,'same_state_shadow_max_abs':maxima,'payloads':{s:identity(prefix.with_suffix(s)) for s in ['.json','.states.f64','.controls.f64','.observations.f32','.records.ptr1','.snapshots.bin','.controllers.bin']}}
def compare(a,b,profile,initial):
    maxerr={'torque':0.,'gripper_control':0.,'qpos23':0.,'qvel21':0.,'snapshot_geometry':0.,'request_F32observation':0.};start=finite(initial.read_bytes(),45,1)[0];require(a['states'][0]==b['states'][0]==start,'same reset')
    for i,(x,y) in enumerate(zip(a['states'],b['states'])):
        require(struct.pack('<d',x[0])==struct.pack('<d',y[0]) and abs(x[0]-(start[0]+i*.002))<=2**-26,'SDKtimebits/logicalclock');maxerr['qpos23']=max(maxerr['qpos23'],difference(x[1:24],y[1:24]));maxerr['qvel21']=max(maxerr['qvel21'],difference(x[24:],y[24:]))
    for i,(x,y) in enumerate(zip(a['controls'],b['controls'])):
        maxerr['torque']=max(maxerr['torque'],difference(x[:7],y[:7]));maxerr['gripper_control']=max(maxerr['gripper_control'],difference(x[7:],y[7:]));require(a['events'][i][0]==b['events'][i][0],'crossrun status/clip/ready inventory');ra=a['records'][i*512:(i+1)*512];rb=b['records'][i*512:(i+1)*512];require(ra[:76]==rb[:76] and ra[508:]==rb[508:],'exact PTR1 logical/action/sourceflags')
    for (tag,x),(tag2,y) in zip(a['snapshots'],b['snapshots']):require(tag==tag2 and struct.pack('<d',x[0])==struct.pack('<d',y[0]),'exact request/source stamps');maxerr['snapshot_geometry']=max(maxerr['snapshot_geometry'],difference(x[1:],y[1:]))
    for x,y in zip(a['observations'],b['observations']):maxerr['request_F32observation']=max(maxerr['request_F32observation'],difference(x,y))
    for k,v in maxerr.items():require(v<=profile['cross_run_budgets_abs'][k],('crossrun budget',k,v))
    return maxerr
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--xml',type=Path,required=True);p.add_argument('--initial',type=Path,required=True);p.add_argument('--trace',type=Path,required=True);p.add_argument('--SDK',type=Path,default=Path.home()/'.cache/psionic/groot-reference-env/lib/python3.12/site-packages/mujoco');p.add_argument('--prepare-only',action='store_true');p.add_argument('--execute-prepared',action='store_true');a=p.parse_args();require(a.prepare_only!=a.execute_prepared,'choose prepare OR execute');out=a.output.resolve();compiler=a.compiler.resolve();sdk=a.SDK.resolve();profile=authenticate(a.xml,a.initial,a.trace);require(sha(compiler)=='a429ad59a39f1b77adee37dd09895d8f7fd36b3149bc3fdf6eed4131a2d4f818','actual qualifiedcompiler');lib=sdk/'libmujoco.3.3.1.dylib';require(sha(lib)=='f8c9735ed948d6830d1d3d31d50216ec14ca6a6e95c7346a4dc8300e1b8ffed4','SDKidentity');inventory=source_inventory();env=os.environ.copy();env['DYLD_LIBRARY_PATH']=str(sdk)+(':'+env['DYLD_LIBRARY_PATH'] if env.get('DYLD_LIBRARY_PATH') else '')
    def command(name,argv,record):
        t=time.monotonic();x=subprocess.run([str(z) for z in argv],cwd=ROOT,capture_output=True,timeout=300,env=env);(out/(name+'.stdout')).write_bytes(x.stdout);(out/(name+'.stderr')).write_bytes(x.stderr);record['commands'].append({'name':name,'argv':[str(z) for z in argv],'exit':x.returncode,'stderr_bytes':len(x.stderr),'seconds':time.monotonic()-t});(out/'record.json').write_text(json.dumps(record,indent=2)+'\n');require(x.returncode==0 and not x.stderr,(name,x.stdout.decode(errors='replace'),x.stderr.decode(errors='replace')));return x.stdout
    if a.prepare_only:
        require(not out.exists(),'preserve original execution');out.mkdir(parents=True);r={'schema':'psionic.panda-query-source.prefix-qualification.v1','passed':False,'prepared':False,'compiler_sha256':sha(compiler),'SDK_sha256':sha(lib),'profile_sha256':sha(PROFILE),'source_sha256':inventory,'inputs':{str(x):identity(x) for x in [a.xml,a.initial,a.trace,a.trace.parent/'record.json']},'commands':[],'SDK_execution_count':0};header=out/'panda-query-session.h';source=out/'panda-query-session.c';source.write_bytes(command('emit',[compiler,'c','psionic/tests/robotics/panda-query-session.coax','--library','--header',header],r));binary=out/'prefix';command('compile',[os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-Wno-deprecated-declarations','-I'+str(out),'-I'+str(ROOT/'psionic/platform'),'-I'+str(sdk/'include'),source,ROOT/'psionic/tests/robotics/panda_query_source_prefix.c',ROOT/'psionic/platform/panda-env.c',lib,'-framework','OpenGL','-lm','-o',binary],r);r.update(prepared=True,C_sha256=sha(source),header_sha256=sha(header),binary_sha256=sha(binary));(out/'record.json').write_text(json.dumps(r,indent=2)+'\n');print('Prepared ONLY; zero SDKqueries/steps. '+str(out/'record.json'));return
    r=json.loads((out/'record.json').read_text());require(r['prepared'] and not r['passed'] and r['SDK_execution_count']==0 and r['source_sha256']==inventory and r['profile_sha256']==sha(PROFILE) and r['C_sha256']==sha(out/'panda-query-session.c') and r['header_sha256']==sha(out/'panda-query-session.h') and r['binary_sha256']==sha(out/'prefix'),'immutable prepared inputs');r['runs']=[];r['SDK_execution_count']=1;(out/'record.json').write_text(json.dumps(r,indent=2)+'\n')
    try:
        for mode in [0,1]:
            prefix=out/('sdk' if mode==0 else 'owned');command('run-'+str(mode),[out/'prefix',a.xml,a.initial,a.trace,mode,prefix],r);run=read_run(prefix,mode,profile);r['runs'].append({k:v for k,v in run.items() if k in ['summary','same_state_shadow_max_abs','payloads']});(out/'record.json').write_text(json.dumps(r,indent=2)+'\n')
        left=read_run(out/'sdk',0,profile);right=read_run(out/'owned',1,profile);r['cross_run_max_abs']=compare(left,right,profile,a.initial);require(source_inventory()==inventory,'source frozen during execution');r['passed']=True;r['SDK_physics_steps_new']=500;r['neural_forwards']=0;r['render_calls']=0
    except BaseException as e:r['failure']=repr(e);raise
    finally:(out/'record.json').write_text(json.dumps(r,indent=2)+'\n')
    print(json.dumps({'passed':r['passed'],'cross_run_max_abs':r['cross_run_max_abs'],'record':str(out/'record.json')}))
if __name__=='__main__':main()

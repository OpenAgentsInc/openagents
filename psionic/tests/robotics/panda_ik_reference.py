#!/usr/bin/env python3
"""Independent fixed-position IK trajectory and native artifact proof; no SDK."""
from pathlib import Path
import argparse,copy,hashlib,json,math,os,platform,re,struct,subprocess,sys,time
from panda_inverse_dynamics_matrix import body_tangents
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/panda-ik';SOURCE=ROOT/'psionic/lib/robotics/panda-ik.coax';NATIVE=ROOT/'psionic/tests/robotics/panda_ik_native.c'
EXPERIMENT_SHA='bad874b0f8eca3bd9d58814541f0849e293571e5213c43653c5cb238faa00b85';PROFILE_SHA='d37cf705a7c6230eb575ce03c03524bf5652601062de0c2a5cb4ae12f6ddb8d5'
EVENT=struct.Struct('<4q11d2q');CASE=struct.Struct('<10dq')
def require(ok,message):
 if not ok:raise AssertionError(message)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def finite(values,n,label):
 require(len(values)==n and all(math.isfinite(x) for x in values),label+' exact finite extent');return list(values)
def graph(path,seen=None):
 if seen is None:seen={}
 path=Path(path).resolve()
 if path in seen:return seen
 seen[path]=sha(path)
 for name in re.findall(r'^\s*#load\s+"([^\"]+)"',path.read_text(),re.M):graph(path.parent/name,seen)
 return seen
def load_frozen():
 require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'original experiment identity');require(sha(FIXTURE/'oracle-profile.json')==PROFILE_SHA,'precomparison oracle profile identity');e=json.loads((FIXTURE/'experiment.json').read_text());p=json.loads((FIXTURE/'oracle-profile.json').read_text());require(p['original_experiment_sha256']==EXPERIMENT_SHA and p['budgets']==e['budgets'],'same original units/budgets');raw=(FIXTURE/'cases.bin').read_bytes();require(sha(FIXTURE/'cases.bin')==e['cases_sha256'] and len(raw)==49*CASE.size,'49 frozen cases');require(raw==b''.join(CASE.pack(*c['seed_radians'],*c['target_world_metres'],c['max_updates']) for c in e['cases']),'case JSON/binary identity')
 for name,digest in e['dependencies'].items():require(sha(ROOT/name)==digest,'source dependency '+name)
 model=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/sdk-model.json').read_text());require([j['range'] for j in model['arm_joints']]==e['joint_limits_radians'],'source joint ranges');require(model['site']['body_name']=='gripper0_eef' and model['site']['position']==[0,0,0] and model['site']['quaternion_wxyz']==[1,0,0,0],'grip site is identity-local EEF')
 cases=e['cases']+p['additional_native_contract_cases'];require(len(cases)==50,'49 original plus one explicit contract context')
 for c in cases:
  finite(c['seed_radians'],7,'seed');finite(c['target_world_metres'],3,'target');require(all(lo<=q<=hi for q,(lo,hi) in zip(c['seed_radians'],e['joint_limits_radians'])) and all(abs(x)<=10 for x in c['target_world_metres']) and 0<=c['max_updates']<=96,'declared admitted corpus')
 return e,p,cases

def position_jacobian(q,nodes):
 pose,partials=body_tangents(nodes,q)[-1];position=[pose[i][3] for i in range(3)];jacobian=[[partials[j][i][3] for j in range(7)] for i in range(3)];finite(position,3,'matrix position');finite([x for row in jacobian for x in row],21,'matrix tangent');return position,jacobian

def gaussian3(matrix,rhs):
 a=[list(row) for row in matrix];b=list(rhs)
 for k in range(3):
  pivot=max(range(k,3),key=lambda row:abs(a[row][k]));require(abs(a[pivot][k])>1e-18,'independent positive damped pivot');a[k],a[pivot]=a[pivot],a[k];b[k],b[pivot]=b[pivot],b[k]
  for row in range(k+1,3):
   factor=a[row][k]/a[k][k];a[row][k]=0.
   for col in range(k+1,3):a[row][col]-=factor*a[k][col]
   b[row]-=factor*b[k]
 x=[0.]*3
 for row in range(2,-1,-1):x[row]=(b[row]-math.fsum(a[row][col]*x[col] for col in range(row+1,3)))/a[row][row]
 finite(x,3,'Gaussian solution');residual=max(abs(math.fsum(matrix[i][j]*x[j] for j in range(3))-rhs[i]) for i in range(3));require(math.isfinite(residual) and residual<=2e-12,'frozen independent Gaussian residual');return x,residual

def dls(jacobian,error,damping=.05,cap=.1):
 regularization=damping*damping;matrix=[[math.fsum(jacobian[row][k]*jacobian[col][k] for k in range(7))+(regularization if row==col else 0.) for col in range(3)] for row in range(3)];weighted,residual=gaussian3(matrix,error);delta=[math.fsum(jacobian[row][j]*weighted[row] for row in range(3)) for j in range(7)];finite(delta,7,'independent delta');largest=max(abs(x) for x in delta)
 if largest>cap:delta=[x*(cap/largest) for x in delta]
 return delta,residual

def state(q,target,nodes):
 position,jacobian=position_jacobian(q,nodes);error=[target[i]-position[i] for i in range(3)];norm=math.sqrt(math.fsum(x*x for x in error));finite([norm],1,'matrix residual');return {'angles':list(q),'position':position,'error':norm,'jacobian':jacobian,'residual':error}

def advance(current,target,nodes,limits):
 delta,linear_residual=dls(current['jacobian'],current['residual']);proposed=[x+y for x,y in zip(current['angles'],delta)];projected=sum(x<lo or x>hi for x,(lo,hi) in zip(proposed,limits));q=[min(hi,max(lo,x)) for x,(lo,hi) in zip(proposed,limits)];result=state(q,target,nodes);result['projected']=projected;result['linear_residual']=linear_residual;return result

def poison():return [float(1001+i) for i in range(11)],1012,1013

def event(case,kind,stage,status,current=None,updates=0,projected=0):
 if status:values,u,p=poison()
 else:values=current['angles']+current['position']+[current['error']];u,p=updates,projected
 finite(values,11,'oracle event');return [case,kind,stage,status,*values,u,p]

def prepare(out):
 out.mkdir(parents=True,exist_ok=False);began=time.monotonic();record={'schema':'psionic.panda-ik.independent-oracle.v1','passed':False,'SDK_calls':0,'Coax_calls':0,'simulation_steps':0,'NN_calls':0,'events':0}
 try:
  e,p,cases=load_frozen();nodes=json.loads((ROOT/'psionic/fixtures/robotics/panda-fk/source-profile.json').read_text())['nodes'];require(len(nodes)==12,'fixed12 transform chain');diag=[[float(i==j)*(i+1) for j in range(7)] for i in range(3)];delta,res=dls(diag,[.03,-.04,.02]);literal=[.03/1.0025,-.08/4.0025,.06/9.0025]+[0.]*4;require(max(abs(x-y) for x,y in zip(delta,literal))<1e-14,'independent DLS diagonal literal');delta,_=dls(diag,[10.,10.,10.]);require(abs(max(abs(x) for x in delta)-.1)<1e-15 and delta[1]<.1 and delta[2]<.1,'one global cap literal');zero,_=dls([[0.]*7 for _ in range(3)],[.03,-.04,.02]);require(zero==[0.]*7,'damped null space literal')
  events=[];summaries=[];linear=[];target_errors=[]
  for index,c in enumerate(cases):
   source=c.get('source');target=c['target_world_metres']
   if isinstance(source,dict) and 'path' in source:
    raw=(ROOT/source['path']).read_bytes();require(len(raw)==144*8,'retained PcSample extent');values=finite(struct.unpack('<144d',raw),144,'retained target sample');require(target==values[:3] and source['q_radians']==values[12:19],'retained target/seed-source metadata')
   if isinstance(source,dict) and 'independent_target_radians' in source:
    xyz,_=position_jacobian(source['independent_target_radians'],nodes);err=max(abs(x-y) for x,y in zip(xyz,target));require(err<=2e-12,'frozen reachable target generation');target_errors.append(err)
   original=state(c['seed_radians'],target,nodes);current=original;trace=[];count=0
   while current['error']>1e-6 and len(trace)<c['max_updates']:
    current=advance(current,target,nodes,e['joint_limits_radians']);trace.append(current);count+=current['projected'];linear.append(current['linear_residual'])
   status=0 if current['error']<=1e-6 else 6;require((status==0)==(c['kind']=='reachable'),'predeclared convergence inventory '+c['name']);events.append(event(index,0,0,status,current,len(trace),count))
   if index in p['standalone_step_case_indices']:
    one=trace[0] if trace else advance(original,target,nodes,e['joint_limits_radians']);events.append(event(index,1,1,0,one,1,one['projected']))
   for stage,value in enumerate(trace,1):events.append(event(index,2,stage,0,value,1,value['projected']))
   events.append(event(index,3,len(trace),status,current,0,0));summaries.append({'case':index,'name':c['name'],'status':status,'updates':len(trace),'projected_total':count,'initial_error':original['error'],'final_error':current['error'],'trace':[{'angles':x['angles'],'position':x['position'],'error':x['error'],'projected':x['projected'],'linear_residual':x['linear_residual']} for x in trace]})
  blob=b''.join(EVENT.pack(*row) for row in events);(out/'expected.bin').write_bytes(blob);(out/'trajectory.json').write_text(json.dumps(summaries,indent=2)+'\n');record.update({'passed':True,'cases':50,'original_cases':49,'extra_native_cases':1,'standalone_steps':35,'events':len(events),'trajectory_updates':sum(x['updates'] for x in summaries),'solve_status_histogram':{'ok':sum(x['status']==0 for x in summaries),'not_converged':sum(x['status']==6 for x in summaries)},'projection_events':sum(x['projected_total'] for x in summaries),'max_Gaussian_residual_metres':max(linear),'max_reachable_target_generation_error_metres':max(target_errors),'extra_active_projection':{k:v for k,v in summaries[-1].items() if k!='trace'},'expected_sha256':sha(out/'expected.bin'),'trajectory_sha256':sha(out/'trajectory.json'),'source_sha256':{str(Path(__file__).resolve().relative_to(ROOT)):sha(__file__),**e['dependencies']},'experiment_sha256':EXPERIMENT_SHA,'oracle_profile_sha256':PROFILE_SHA,'counts_exact':True,'budgets':e['budgets']})
 except BaseException as error:record['failure']=repr(error);raise
 finally:record['seconds']=time.monotonic()-began;(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
 print(json.dumps({'passed':True,'record':str(out/'record.json'),'events':record['events'],'seconds':record['seconds']}))

def decode(raw):
 require(len(raw)%EVENT.size==0,'whole136-byte events');rows=[EVENT.unpack_from(raw,i) for i in range(0,len(raw),EVENT.size)]
 for row in rows:finite(row[4:15],11,'actual/expected event fields')
 return rows

def compare_events(actual_raw,expected_raw):
 actual=decode(actual_raw);expected=decode(expected_raw);require(len(actual)==len(expected),'exact complete event inventory');errors={'radians':[],'position_metres':[],'error_metres':[]};successful=0;refused=0
 for index,(a,b) in enumerate(zip(actual,expected)):
  require(a[:4]==b[:4],f'exact case/kind/stage/status event{index}');require(a[15:]==b[15:],f'exact update/projection counts event{index}')
  if a[3]:require(actual_raw[index*EVENT.size+32:(index+1)*EVENT.size]==expected_raw[index*EVENT.size+32:(index+1)*EVENT.size],f'full unpublished finite poison event{index}');refused+=1
  else:
   successful+=1
   for name,lo,hi in [('radians',4,11),('position_metres',11,14),('error_metres',14,15)]:errors[name].extend(abs(x-y) for x,y in zip(a[lo:hi],b[lo:hi]))
 metrics={}
 for name,values in errors.items():
  finite(values,len(values),'finite differences '+name);require(values and max(values)<=2e-10,'original frozen '+name+' tolerance');metrics[name]={'passed':True,'values':len(values),'max_abs_error':max(values),'atol':2e-10,'rtol':0}
 return {'passed':True,'events':len(actual),'successful_events':successful,'unpublished_events':refused,'exact_status_and_counters':True,'metrics':metrics,'finite_F64_words':len(actual)*11}

def reader_guards(actual,expected):
 # Late fields are checked before maxima, including expected fields.
 negative=0
 for data,is_actual in [(actual,True),(expected,False)]:
  for bad in [data[:-1],data+b'\x00']:
   try:compare_events(bad,expected) if is_actual else compare_events(actual,bad)
   except AssertionError:negative+=1
  for v in [math.nan,math.inf,-math.inf]:
   bad=bytearray(data);struct.pack_into('<d',bad,len(data)-24,v)
   try:compare_events(bad,expected) if is_actual else compare_events(actual,bad)
   except AssertionError:negative+=1
 require(negative==10,'all actual/expected late nonfinite and extent guards');return {'passed':True,'negative_cases':10,'late_error_field':True,'expected_and_actual':True}

def native_proof(out,compiler):
 out.mkdir(parents=True,exist_ok=False);began=time.monotonic();record={'schema':'psionic.panda-ik.native-qualification.v1','passed':False,'SDK_calls':0,'NN_calls':0,'simulation_steps':0,'oracle_cases_regenerated':0,'compiler_sha256':sha(compiler),'compiler_input_fingerprint':('580cc4b7c058db8b828df2d13b43eb4feb38af53f4d692430943d51cca81e036' if sha(compiler)=='a429ad59a39f1b77adee37dd09895d8f7fd36b3149bc3fdf6eed4131a2d4f818' else None),'platform':platform.platform(),'commands':[],'modes':[]}
 def save():(out/'record.json').write_text(json.dumps(record,indent=2)+'\n')
 def command(name,argv):
  argv=[str(x) for x in argv];start=time.monotonic();p=subprocess.run(argv,cwd=ROOT,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'});(out/(name+'.stdout')).write_bytes(p.stdout);(out/(name+'.stderr')).write_bytes(p.stderr);record['commands'].append({'name':name,'argv':argv,'exit':p.returncode,'seconds':time.monotonic()-start,'stdout_sha256':sha(out/(name+'.stdout')),'stderr_sha256':sha(out/(name+'.stderr'))});save();require(p.returncode==0 and not p.stderr,'command success/empty stderr '+name);return p.stdout
 try:
  require(platform.system()=='Darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin arm64 scalar-F64 profile');e,p,cases=load_frozen();oracle=json.loads((FIXTURE/'oracle-qualification.json').read_text());require(oracle['passed'] and oracle['cases']==50 and oracle['events']==363 and oracle['trajectory_updates']==228 and oracle['standalone_steps']==35,'independent reference inventory');require(sha(FIXTURE/'oracle-expected.bin')==oracle['expected_sha256'] and sha(FIXTURE/'oracle-trajectory.json')==oracle['trajectory_sha256'],'frozen prepared reference bytes');expected=(FIXTURE/'oracle-expected.bin').read_bytes();decode(expected)
  deps=graph(SOURCE);deps[NATIVE]=sha(NATIVE);deps[Path(__file__).resolve()]=sha(__file__)
  for name,digest in e['dependencies'].items():deps[ROOT/name]=digest
  for f in FIXTURE.iterdir():
   if f.is_file():deps[f]=sha(f)
  record['source_sha256_before_execution']={str(k.relative_to(ROOT)):v for k,v in sorted(deps.items())};record['experiment_sha256']=EXPERIMENT_SHA;record['oracle_profile_sha256']=PROFILE_SHA;record['oracle_receipt_sha256']=sha(FIXTURE/'oracle-qualification.json');record['original_cases']=49;record['extra_native_contract_cases']=1
  checks=json.loads((FIXTURE/'checks-qualification.json').read_text());units=checks.get('source_unit_refusals',checks.get('unit_refusals',None));require(checks['passed'] and units is not None and {x['name'] for x in units['checks']}=={'angle','target_unit','target_frame','output'} and all(x['refused'] and x['exit']!=0 for x in units['checks']),'carried source/unit proof inventory');require(all(sha(ROOT/name)==entry['sha256'] for name,entry in checks['sources'].items()),'exact unchanged source/check fixtures');record['source_checks_receipt_sha256']=sha(FIXTURE/'checks-qualification.json');record['source_refusals_carried_without_rerun']=units
  inputs=struct.pack('<II',0x314b4950,50)+b''.join(CASE.pack(*c['seed_radians'],*c['target_world_metres'],c['max_updates']) for c in cases)+struct.pack('<7d',*[x[0] for x in e['joint_limits_radians']])+struct.pack('<7d',*[x[1] for x in e['joint_limits_radians']])+struct.pack('<7d',*e['home_radians'])+struct.pack('<3d',*e['cases'][8]['target_world_metres']);(out/'input.bin').write_bytes(inputs);record['input_sha256']=sha(out/'input.bin');(out/'executed-driver.py').write_bytes(Path(__file__).read_bytes());(out/'executed-native.c').write_bytes(NATIVE.read_bytes());emitted={}
  for mode,extra in [('debug',[]),('release',['--release'])]:
   d=out/mode;d.mkdir();c=d/'panda-ik.c';h=d/'panda-ik.h';c.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));match=re.search(r'coax_t_PikStatus pik_solve\(coax_v_f64 seed, (\w+) target,',h.read_text());require(match is not None,'derive actual emitted target-view type');emitted[mode]=(c,h,match.group(1))
  blobs=[]
  for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   d=out/mode;d.mkdir(exist_ok=True);c,h,target_view=emitted[source_mode];binary=d/'native';command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-DPIK_TARGET_VIEW='+target_view,'-I'+str(h.parent),c,NATIVE,'-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'events.bin']));require(native['passed'] and native['cases']==50 and native['events']==363 and native['trajectory_updates']==228 and native['record_bytes']==136 and native['limit_boundaries']==14 and native['target_boundaries']==6 and native['nonfinite_poisons']==50 and native['alias_cases']==8 and native['fpcr']==0,'complete native event/guard inventory');blob=(d/'events.bin').read_bytes();metrics=compare_events(blob,expected);blobs.append(blob);record['modes'].append({'mode':mode,'native':native,'comparison':metrics,'C_sha256':sha(c),'header_sha256':sha(h),'target_view_type':target_view,'binary_sha256':sha(binary),'output_sha256':sha(d/'events.bin')});save()
  require(blobs[0]==blobs[1]==blobs[2],'all three whole event streams byte-identical');record['reader_guard_probes']=reader_guards(blobs[0],expected);require(all(sha(k)==v for k,v in deps.items()) and sha(compiler)==record['compiler_sha256'],'source/compiler/reference stability');record['modes_byte_identical']=True;record['passed']=True
 except BaseException as error:record['failure']=repr(error);raise
 finally:record['seconds']=time.monotonic()-began;save()
 print(json.dumps({'passed':True,'record':str(out/'record.json'),'seconds':record['seconds']}))

def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--compiler',type=Path);ap.add_argument('--out',type=Path,required=True);ap.add_argument('--prepare-only',action='store_true');args=ap.parse_args()
 if args.prepare_only:return prepare(args.out.resolve())
 require(args.compiler is not None,'explicit qualified compiler required');return native_proof(args.out.resolve(),args.compiler.resolve())
if __name__=='__main__':main()

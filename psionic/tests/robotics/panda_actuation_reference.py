#!/usr/bin/env python3
"""New offline actuator/joint-passive proof, never SDK acquisition or simulation."""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,re,struct,subprocess,sys,time
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/panda-actuation';SOURCE=ROOT/'psionic/lib/robotics/panda-actuation.coax';NATIVE=ROOT/'psionic/tests/robotics/panda_actuation_native.c'
EXPERIMENT_SHA='4da210b559aed5f07ad4ab8d5ae5a7393c26fef0885d16dee19057ec605fbe5e'
ROW=struct.Struct('<18d2q')
def require(ok,message):
 if not ok:raise AssertionError(message)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def finite(values,n,label):
 require(len(values)==n and all(math.isfinite(v) for v in values),label+' complete finite words before maxima');return list(values)
def words(raw,n,label):
 require(len(raw)==n*8,label+' exact byte extent');return finite(struct.unpack('<'+str(n)+'d',raw),n,label)
def graph(p,seen=None):
 if seen is None:seen={}
 p=p.resolve()
 if p in seen:return seen
 seen[p]=sha(p)
 for name in re.findall(r'^\s*#load\s+"([^"]+)"',p.read_text(),re.M):graph(p.parent/name,seen)
 return seen
def clip(value,lo,hi):return (lo,1) if value<lo else (hi,1) if value>hi else (value,0)
def scalar(case):
 q,v,u=case['q'],case['v'],case['ctrl'];act=[];passive=[];controls=0;forces=0
 for i in range(7):
  limit=80 if i<5 else 12;x,n=clip(u[i],-limit,limit);controls+=n;act.append(x);passive.append(0.0-(.1 if i<5 else .01)*v[i])
 for i in range(2):
  low,high=(0,.04) if i==0 else (-.04,0);target,n=clip(u[7+i],low,high);controls+=n
  # Match the separate fixed-gain product then affine bias order. No k*(u-q).
  bias=(0.0+(-1000.0)*q[7+i])+0.0*v[7+i];force=1000.0*target+bias;x,n=clip(force,-20,20);forces+=n;act.append(x);passive.append(0.0-100.0*v[7+i])
 return finite(act,9,'scalar actuator'),finite(passive,9,'scalar joint passive'),controls,forces
def compare(actual,expected,atol,label):
 finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' reference');require(len(actual)>0,label+' nonempty');err=max(abs(a-b) for a,b in zip(actual,expected));require(err<=atol,f'{label}: {err:.17g} exceeds frozen {atol:g}');return {'passed':True,'values':len(actual),'max_abs_error':err,'atol':atol,'rtol':0}
def decode(raw):
 require(len(raw)==18*ROW.size,'exact eighteen160-byte records');out=[]
 for i in range(18):
  row=ROW.unpack_from(raw,i*ROW.size);finite(row[:18],18,'actual row '+str(i));require(0<=row[18]<=9 and 0<=row[19]<=2,'exact clip count domain');out.append(row)
 return out
def guards():
 good=ROW.pack(*([0.]*18),0,0)*18;decode(good);bad=[good[:-1],good+ROW.pack(*([0.]*18),0,0)]
 for x in (float('nan'),float('inf'),-float('inf')):
  b=bytearray(good);struct.pack_into('<d',b,17*160+17*8,x);bad.append(bytes(b))
 b=bytearray(good);struct.pack_into('<q',b,len(b)-8,3);bad.append(bytes(b))
 for b in bad:
  try:decode(b)
  except AssertionError:pass
  else:raise AssertionError('decoder admitted late poison')
 return {'passed':True,'refused_cases':len(bad),'poison':'last force word/count and wrong extents'}
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();compiler=a.compiler.resolve();require(not out.exists(),'preserve earlier evidence');out.mkdir(parents=True)
 r={'schema':'psionic.panda-actuation.qualification.v1','passed':False,'SDK_calls':0,'simulation_steps':0,'commands':[],'modes':[]}
 def save():(out/'record.json').write_text(json.dumps(r,indent=2)+'\n')
 def command(name,argv):
  began=time.monotonic();x=subprocess.run([str(v) for v in argv],cwd=ROOT,capture_output=True,timeout=180);(out/(name+'.stdout')).write_bytes(x.stdout);(out/(name+'.stderr')).write_bytes(x.stderr);r['commands'].append({'name':name,'argv':[str(v) for v in argv],'exit':x.returncode,'seconds':time.monotonic()-began,'stderr_bytes':len(x.stderr),'stdout_sha256':hashlib.sha256(x.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(x.stderr).hexdigest()});save();require(x.returncode==0 and not x.stderr,(name,x.stdout.decode(errors='replace'),x.stderr.decode(errors='replace')));return x.stdout
 began=time.monotonic();save()
 try:
  require(sys.platform=='darwin' and platform.machine()=='arm64' and sys.byteorder=='little','qualified Darwin-arm64 profile');require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA,'pre-capture/pre-implementation experiment');e=json.loads((FIXTURE/'experiment.json').read_text());require(e['frozen_before_capture_and_implementation'] and e['case_count']==18 and e['budgets']['actuator_force']=={'atol':5e-11,'rtol':0},'frozen scope/budgets');require(sha(FIXTURE/'cases.f64')==e['case_sha256'],'exact frozen input bits');words((FIXTURE/'cases.f64').read_bytes(),18*27,'all cases');require((FIXTURE/'cases.f64').read_bytes()==b''.join(struct.pack('<27d',*c['q'],*c['v'],*c['ctrl']) for c in e['cases']),'all named cases bind every bit')
  from panda_actuation_sdk_reference import validate_model
  receipt=json.loads((FIXTURE/'sdk-reference.json').read_text());require(receipt['passed'],'original SDK capture passed')
  for name,item in receipt['artifacts'].items():require(sha(FIXTURE/name)==item['sha256'] and (FIXTURE/name).stat().st_size==item['bytes'],'captured SDK artifact '+name)
  model=json.loads((FIXTURE/'sdk-model.json').read_text());r['supported_profile']=validate_model(model,e);sdk=words((FIXTURE/'sdk-values.f64').read_bytes(),18*162,'all original SDK force and future diagnostic words');expected=[scalar(c) for c in e['cases']]
  transmission={key:[] for key in ('length','velocity','moment','control_after')}
  for i,case in enumerate(e['cases']):
   original=sdk[i*162:(i+1)*162];transmission['length']+=original[63:72];transmission['velocity']+=original[72:81];transmission['moment']+=original[81:90];transmission['control_after']+=original[90:99]
  r['SDK_transmission_control_integrity']={'length':compare(transmission['length'],[x for c in e['cases'] for x in c['q']],0,'unit-gear absolute joint length'),'velocity':compare(transmission['velocity'],[x for c in e['cases'] for x in c['v']],0,'unit-gear joint velocity'),'moment':compare(transmission['moment'],[1.]*162,0,'one exact sparse unit-gear moment per actuator'),'control_after':compare(transmission['control_after'],[x for c in e['cases'] for x in c['ctrl']],0,'SDK retains unclamped caller control bytes as values')}
  desired_act=[x for row in expected for x in row[0]];desired_passive=[x for row in expected for x in row[1]];sdk_act=[];sdk_general=[];sdk_joint=[];sdk_passive=[];sdk_recomposed=[]
  for i,row in enumerate(expected):
   original=sdk[i*162:(i+1)*162];sdk_act+=original[:9];sdk_general+=original[9:18];sdk_joint +=[original[18+j]+original[27+j] for j in range(9)];sdk_passive+=original[54:63];sdk_recomposed +=[((original[18+j]+original[27+j])+original[45+j])+original[36+j] for j in range(9)]
  r['scalar_oracle_vs_SDK']={'actuator':compare(desired_act,sdk_act,5e-11,'scalar vs actuation-space SDK force'),'generalized':compare(desired_act,sdk_general,5e-11,'unit-gear scalar vs generalized SDK force'),'joint_passive':compare(desired_passive,sdk_joint,5e-11,'scalar joint spring+damper only')};r['SDK_full_passive_recomposition_diagnostic']=compare(sdk_recomposed,sdk_passive,2e-10,'SDK diagnostic passive recomposition, not owned full passive');r['guard_checks']=guards();r['experiment_sha256']=EXPERIMENT_SHA;r['SDK_reference_sha256']=sha(FIXTURE/'sdk-reference.json');r['numeric_profile']=e['numeric_profile'];r['nonclaims']=e['nonclaims'];r['compiler_sha256']=sha(compiler)
  deps=graph(SOURCE);deps.update({Path(__file__).resolve():sha(Path(__file__)),NATIVE:sha(NATIVE),Path(__file__).with_name('panda_actuation_sdk_reference.py'):sha(Path(__file__).with_name('panda_actuation_sdk_reference.py'))})
  for name in ('experiment.json','cases.f64','sdk-reference.json','sdk-capture-profile.json','sdk-model.json','sdk-values.f64'):deps[FIXTURE/name]=sha(FIXTURE/name)
  r['source_sha256']={str(k.relative_to(ROOT)):v for k,v in sorted(deps.items())};(out/'input.bin').write_bytes(struct.pack('<II',0x31464150,18)+(FIXTURE/'cases.f64').read_bytes());(out/'expected.json').write_text(json.dumps(expected,indent=2)+'\n');r['input_sha256']=sha(out/'input.bin');r['expected_sha256']=sha(out/'expected.json')
  for name in ('arm','target','rate','output'):
   path=ROOT/('psionic/tests/robotics/panda_actuation_bad_'+name+'.coax');deps[path]=sha(path);x=subprocess.run([str(compiler),'check',str(path.relative_to(ROOT))],cwd=ROOT,capture_output=True,timeout=120);(out/('refusal-'+name+'.stderr')).write_bytes(x.stderr);require(x.returncode!=0 and b'error[E0317]' in x.stderr,'precise source unit/result refusal '+name);r.setdefault('source_refusals',[]).append({'name':name,'exit':x.returncode,'diagnostic':'E0317','source_sha256':sha(path),'stderr_sha256':hashlib.sha256(x.stderr).hexdigest()})
  r['source_sha256']={str(k.relative_to(ROOT)):v for k,v in sorted(deps.items())};(out/'executed-driver.py').write_bytes(Path(__file__).read_bytes());(out/'executed-native.c').write_bytes(NATIVE.read_bytes());emitted={}
  for mode,extra in [('debug',[]),('release',['--release'])]:
   d=out/mode;d.mkdir();c=d/'panda-actuation.c';h=d/'panda-actuation.h';c.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]));emitted[mode]=(c,h)
  blobs=[]
  for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   d=out/mode;d.mkdir(exist_ok=True);c,h=emitted[source_mode];binary=d/'native';command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(h.parent),c,NATIVE,'-lm','-o',binary]);native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'forces.bin']));require(native['passed'] and native['cases']==18 and native['record_bytes']==160 and native['position_rate_boundaries']==36 and native['control_boundaries']==18 and native['alias_cases']==4,'complete native coverage inventory');blob=(d/'forces.bin').read_bytes();actual=decode(blob);blobs.append(blob);act=[];passive=[]
   for got,want in zip(actual,expected):require(got[18:]==want[2:],'exact independent clipping counts');act+=got[:9];passive+=got[9:18]
   metrics={'actuator_SDK':compare(act,sdk_act,5e-11,'owned actuator force'),'generalized_SDK':compare(act,sdk_general,5e-11,'owned generalized actuator force'),'joint_passive_SDK':compare(passive,sdk_joint,5e-11,'owned joint-passive force'),'actuator_scalar':compare(act,desired_act,5e-11,'independent scalar actuator force'),'joint_passive_scalar':compare(passive,desired_passive,5e-11,'independent scalar joint-passive force')};r['modes'].append({'mode':mode,'native':native,'metrics':metrics,'clip_counts_exact':True,'all_actual_force_words_finite':324,'C_sha256':sha(c),'header_sha256':sha(h),'binary_sha256':sha(binary),'output_sha256':sha(d/'forces.bin')});save()
  require(blobs[0]==blobs[1]==blobs[2],'all three complete force/count streams byte-identical');require(all(sha(k)==v for k,v in deps.items()) and sha(compiler)==r['compiler_sha256'],'source/compiler/reference changed');r['native_modes_byte_identical']=True;r['passed']=True
 except BaseException as error:r['failure']=repr(error);raise
 finally:r['seconds']=time.monotonic()-began;save()
 print(json.dumps({'passed':True,'record':str(out/'record.json'),'seconds':r['seconds']}))
if __name__=='__main__':main()

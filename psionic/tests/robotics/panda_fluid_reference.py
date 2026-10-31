#!/usr/bin/env python3
"""NEW offline PFL native proof. Consumes frozen independent/SDK facts; no simulation."""
from pathlib import Path
import argparse,hashlib,json,math,os,platform,re,struct,subprocess,sys,time
from panda_fluid_oracle import validate_pm_profile
ROOT=Path(__file__).resolve().parents[3];FIXTURE=ROOT/'psionic/fixtures/robotics/panda-fluid';SOURCE=ROOT/'psionic/lib/robotics/panda-fluid.coax';NATIVE=ROOT/'psionic/tests/robotics/panda_fluid_native.c'
EXPERIMENT_SHA='165382c310c8ed28025d378e413efa1fdfa190426be5f5f6d7e2e50da533f297';ADDENDUM_SHA='773c3a35e3cb0e2a8073653ca163295081fd5bbd03a50d2b07e48b0a59c225ef';EXPECTED_SHA='98d023fc4f7cde12dafc37e6acb4cb5d869eccb0d58dbfffc3da7c73ef672e68'
ROW=struct.Struct('<q27d')
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
 for name in re.findall(r'^\s*#load\s+"([^\"]+)"',p.read_text(),re.M):graph(p.parent/name,seen)
 return seen
def decode(raw):
 require(len(raw)==42*ROW.size,'exact42 full records');out=[]
 for i in range(42):
  row=ROW.unpack_from(raw,i*ROW.size);require(row[0]==0,'exact successful status');out.append(finite(row[1:],27,'native row'+str(i)))
 return out
def compare(actual,expected,atol,label,unit):
 finite(actual,len(expected),label+' actual');finite(expected,len(actual),label+' expected');require(actual,label+' nonempty');error=max(abs(x-y) for x,y in zip(actual,expected));require(error<=atol,f'{label}: {error:.17g} exceeds frozen {atol:g}');return {'passed':True,'values':len(actual),'max_abs_error':error,'atol':atol,'rtol':0,'unit':unit}
def field_metrics(actual,expected,prefix):
 result={}
 for field,name in enumerate(('fluid','joint_passive','passive')):
  for begin,end,coordinate,unit in [(0,7,'hinge','joule/radian'),(7,9,'slide','newton')]:
   result[name+'.'+coordinate]=compare([row[field*9+j] for row in actual for j in range(begin,end)],[row[field*9+j] for row in expected for j in range(begin,end)],5e-10,prefix+'.'+name+'.'+coordinate,unit)
 return result
def guard_probes(raw,expected):
 decode(raw);words(expected,1134,'expected proof table');rejected=0
 for data,reader in [(raw,decode),(expected,lambda x:words(x,1134,'expected poison'))]:
  for bad in [data[:-8],data+b'\x00'*8]:
   try:reader(bad)
   except AssertionError:rejected+=1
  for value in [math.nan,math.inf,-math.inf]:
   poisoned=bytearray(data);struct.pack_into('<d',poisoned,len(data)-8,value)
   try:reader(poisoned)
   except AssertionError:rejected+=1
 require(rejected==10,'all actual/expected late NaN/Inf and extent poisons rejected');return {'passed':True,'negative_cases':10,'late_last_force_word':True,'expected_and_actual':True}

def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--compiler',type=Path,default=ROOT/'coax/bin/coax');ap.add_argument('--out',type=Path,required=True);ap.add_argument('--resume-failed-sanitizer',action='store_true',help='continue only the previously failed sanitizer startup; retain exact passed debug/release proof');ap.add_argument('--prepared-debug',action='store_true',help='adopt the already emitted/compiled NEW debug setup, never repeat it');args=ap.parse_args();out=args.out.resolve();compiler=args.compiler.resolve()
 require(platform.system()=='Darwin' and platform.machine()=='arm64','declared Darwin arm64 F64/FPCR profile only')
 if args.resume_failed_sanitizer:require(out.is_dir() and (out/'record.json').exists(),'existing partial new proof')
 elif args.prepared_debug:require(out.is_dir() and not (out/'record.json').exists(),'fresh prepared NEW proof directory')
 else:out.mkdir(parents=True,exist_ok=False)
 began=time.monotonic();r={'schema':'psionic.panda-fluid.native-proof.v1','passed':False,'SDK_calls':0,'steps':0,'render_calls':0,'NN_calls':0,'oracle_cases_regenerated':0,'compiler':str(compiler),'compiler_sha256':sha(compiler),'compiler_input_fingerprint':'580cc4b7c058db8b828df2d13b43eb4feb38af53f4d692430943d51cca81e036','platform':platform.platform(),'modes':[],'commands':[]}
 if args.resume_failed_sanitizer:
  old=(out/'record.json').read_bytes();r=json.loads(old);require([m['mode'] for m in r['modes']]==['debug','release'] and r['compiler_sha256']==sha(compiler),'only unchanged passed profiles can carry');(out/'prior-sanitizer-startup-failure.json').write_bytes(old);r['prior_failure_record_sha256']=hashlib.sha256(old).hexdigest();r['sanitizer_configuration_followup']='Darwin does not support detect_leaks=1; startup aborted before product. Set detect_leaks=0, retain all prior files/results, run only existing sanitized binary. No leak-detector claim.';r.pop('failure',None)
 def save():(out/'record.json').write_text(json.dumps(r,indent=2)+'\n')
 def command(name,argv):
  name=('resume-'+name) if args.resume_failed_sanitizer else name;argv=[str(x) for x in argv];started=time.monotonic();p=subprocess.run(argv,cwd=ROOT,capture_output=True,timeout=180,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0:abort_on_error=1','UBSAN_OPTIONS':'halt_on_error=1:print_stacktrace=1'});(out/(name+'.stdout')).write_bytes(p.stdout);(out/(name+'.stderr')).write_bytes(p.stderr);r['commands'].append({'name':name,'argv':argv,'exit':p.returncode,'seconds':time.monotonic()-started,'stdout_sha256':sha(out/(name+'.stdout')),'stderr_sha256':sha(out/(name+'.stderr'))});save();require(p.returncode==0 and not p.stderr,'command passed with empty stderr: '+name);return p.stdout
 try:
  require(sha(FIXTURE/'experiment.json')==EXPERIMENT_SHA and sha(FIXTURE/'support-addendum-v1.json')==ADDENDUM_SHA,'frozen original/support identities');exp=json.loads((FIXTURE/'experiment.json').read_text());require(sha(FIXTURE/'cases.f64')==exp['case_sha256'],'frozen cases');require(sha(FIXTURE/'oracle-expected.f64')==EXPECTED_SHA,'independent force table identity');oracle=json.loads((FIXTURE/'oracle-qualification.json').read_text());require(oracle['passed'] and oracle['expected_values_generated_without_product_or_SDK'],'independent proof passed');
  for name,entry in oracle['artifacts'].items():require(sha(FIXTURE/name)==entry['sha256'] and (FIXTURE/name).stat().st_size==entry['bytes'],'frozen oracle artifact '+name)
  for name,digest in exp['reference_dependencies'].items():require(sha(ROOT/name)==digest,'frozen reference '+name)
  deps=graph(SOURCE);deps[NATIVE]=sha(NATIVE);deps[Path(__file__).resolve()]=sha(Path(__file__));deps[ROOT/'psionic/tests/robotics/panda_fluid_oracle.py']=sha(ROOT/'psionic/tests/robotics/panda_fluid_oracle.py')
  for f in FIXTURE.iterdir():
   if f.is_file():deps[f]=sha(f)
  for name,digest in exp['reference_dependencies'].items():deps[ROOT/name]=digest
  model=json.loads((ROOT/'psionic/fixtures/robotics/panda-actuation/sdk-model.json').read_text());reference=words((FIXTURE/'oracle-expected.f64').read_bytes(),42*27,'independent table');expected=[reference[i*27:(i+1)*27] for i in range(42)];diagnostics=json.loads((FIXTURE/'oracle-diagnostics.json').read_text());r['frozen_experiment_sha256']=EXPERIMENT_SHA;r['support_addendum_sha256']=ADDENDUM_SHA;r['independent_oracle_receipt_sha256']=sha(FIXTURE/'oracle-qualification.json');r['cached_contexts']={'cases':18,'unique_qv':7,'moving_unique_qv':2};r['new_owned_matrix_contexts']=24
  sdk=words((ROOT/'psionic/fixtures/robotics/panda-actuation/sdk-values.f64').read_bytes(),18*162,'retained SDK');sdk_expected=[]
  for i in range(18):row=sdk[i*162:(i+1)*162];sdk_expected.append(row[45:54]+[row[18+j]+row[27+j] for j in range(9)]+row[54:63])
  (out/'input.bin').write_bytes(struct.pack('<II',0x314c4650,42)+(FIXTURE/'cases.f64').read_bytes());r['input_sha256']=sha(out/'input.bin');(out/'executed-driver.py').write_bytes(Path(__file__).read_bytes());(out/'executed-native.c').write_bytes(NATIVE.read_bytes())
  for name in (() if args.resume_failed_sanitizer else ('q','frequency','moment','output')):
   f=ROOT/('psionic/tests/robotics/panda_fluid_bad_'+name+'.coax');deps[f]=sha(f);p=subprocess.run([str(compiler),'check',str(f.relative_to(ROOT))],cwd=ROOT,capture_output=True,timeout=120);(out/('refusal-'+name+'.stderr')).write_bytes(p.stderr);code='E0325' if name in ('frequency','moment') else 'E0317';require(p.returncode!=0 and re.findall(rb'error\[([A-Z0-9]+)\]',p.stderr)==[code.encode()],'one precise unit/result refusal '+name);r.setdefault('source_refusals',[]).append({'name':name,'exit':p.returncode,'diagnostic':code,'source_sha256':sha(f),'stderr_sha256':sha(out/('refusal-'+name+'.stderr'))})
  positive=ROOT/'psionic/tests/robotics/panda_fluid_units.coax';deps[positive]=sha(positive)
  if not args.resume_failed_sanitizer:command('positive-units',[compiler,'test',positive.relative_to(ROOT),'--only','fluid explicit one-radian'])
  for name in ('q','frequency','moment','output'):
   f=ROOT/('psionic/tests/robotics/panda_fluid_bad_'+name+'.coax');deps[f]=sha(f)
   if args.resume_failed_sanitizer:require(any(x['name']==name and x['source_sha256']==sha(f) for x in r['source_refusals']),'unchanged passed source refusal')
  emitted={}
  for mode,extra in [('debug',[]),('release',['--release'])]:
   d=out/mode;d.mkdir(exist_ok=True);c=d/'panda-fluid.c';h=d/'panda-fluid.h'
   if args.resume_failed_sanitizer:
    prior=next(x for x in r['modes'] if x['mode']==mode);require(sha(c)==prior['C_sha256'] and sha(h)==prior['header_sha256'],'exact passed emission carry')
   elif mode=='debug' and args.prepared_debug:
    require(c.is_file() and h.is_file() and (d/'native').is_file(),'prepared debug artifacts');r['adopted_NEW_preparation']={'reason':'preceding exact Coax emission/native compile passed via tools.exec before orchestrator; no native execution had occurred','C_sha256':sha(c),'header_sha256':sha(h),'binary_sha256':sha(d/'native'),'compiler_sha256':sha(compiler),'emit_argv':[str(compiler),'c',str(SOURCE.relative_to(ROOT)),'--library','--header',str(h)],'compile_flags':['-std=c11','-O0','-g','-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos']}
   else:c.write_bytes(command('emit-'+mode,[compiler,'c',SOURCE.relative_to(ROOT),'--library',*extra,'--header',h]))
   emitted[mode]=(c,h)
  blobs=[];profile_blobs=[]
  for mode,source_mode,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
   d=out/mode;d.mkdir(exist_ok=True);c,h=emitted[source_mode];binary=d/'native'
   if args.resume_failed_sanitizer and mode in ('debug','release'):
    prior=next(x for x in r['modes'] if x['mode']==mode);require(sha(binary)==prior['binary_sha256'] and sha(d/'forces.bin')==prior['output_sha256'] and sha(d/'pm-profile.bin')==prior['PM_metadata_sha256'],'exact passed native artifact carry');blobs.append((d/'forces.bin').read_bytes());profile_blobs.append((d/'pm-profile.bin').read_bytes());continue
   if not args.resume_failed_sanitizer and not (mode=='debug' and args.prepared_debug):command('compile-'+mode,[os.environ.get('CC','cc'),'-std=c11',*flags,'-fno-fast-math','-ffp-contract=off','-fno-builtin-sin','-fno-builtin-cos','-I'+str(h.parent),c,NATIVE,'-lm','-o',binary])
   native=json.loads(command('run-'+mode,[binary,out/'input.bin',d/'forces.bin',d/'pm-profile.bin']));require(native['passed'] and native['cases']==42 and native['record_bytes']==224 and native['position_rate_boundaries']==36 and native['alias_cases']==4 and native['nonfinite_poisons']==90 and native['fpcr']==0,'native coverage inventory');blob=(d/'forces.bin').read_bytes();actual=decode(blob);blobs.append(blob);metadata=(d/'pm-profile.bin').read_bytes();profile_blobs.append(metadata);profile=validate_pm_profile(metadata,model);metrics={'independent_matrix':field_metrics(actual,expected,'matrix'),'retained_SDK':field_metrics(actual[:18],sdk_expected,'SDK')};powers=[];passive_powers=[]
   for i,row in enumerate(actual):
    v=exp['cases'][i]['v'];fluid_power=math.fsum(row[j]*v[j] for j in range(9));passive_power=math.fsum(row[18+j]*v[j] for j in range(9));finite([fluid_power,passive_power],2,'derived actual power');require(fluid_power<=5e-9 and passive_power<=5e-9,'nonpositive zero-wind dissipative power');powers.append(fluid_power);passive_powers.append(passive_power)
   body_power=[x['body_power'] for x in diagnostics];oracle_passive=[x['passive_power'] for x in diagnostics];power_metrics={'virtual_work':compare(powers,body_power,5e-9,'actual generalized vs independent COM-wrench power','watt'),'passive':compare(passive_powers,oracle_passive,5e-9,'actual complete passive power','watt'),'max_fluid_power_W':max(powers),'max_passive_power_W':max(passive_powers)}
   r['modes'].append({'mode':mode,'native':native,'metrics':metrics,'power':power_metrics,'PM_source_metadata':profile,'all_actual_force_words_finite':1134,'C_sha256':sha(c),'header_sha256':sha(h),'binary_sha256':sha(binary),'output_sha256':sha(d/'forces.bin'),'PM_metadata_sha256':sha(d/'pm-profile.bin')});save()
  require(blobs[0]==blobs[1]==blobs[2],'all native status/force streams byte-identical');require(profile_blobs[0]==profile_blobs[1]==profile_blobs[2],'all source metadata streams byte-identical');r['reader_guard_probes']=guard_probes(blobs[0],(FIXTURE/'oracle-expected.f64').read_bytes());require(all(sha(k)==v for k,v in deps.items()) and sha(compiler)==r['compiler_sha256'],'source/compiler/reference stability');r['source_sha256']={str(k.relative_to(ROOT)):v for k,v in sorted(deps.items())};r['native_modes_byte_identical']=True;r['passed']=True
 except BaseException as error:r['failure']=repr(error);raise
 finally:r['seconds']=time.monotonic()-began;save()
 print(json.dumps({'passed':True,'record':str(out/'record.json'),'seconds':r['seconds']}))
if __name__=='__main__':main()

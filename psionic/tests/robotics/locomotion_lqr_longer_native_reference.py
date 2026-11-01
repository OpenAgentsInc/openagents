#!/usr/bin/env python3
"""New continuation ABI guards only; old math is carried by source/C closure."""
from __future__ import annotations
import argparse,hashlib,importlib.util,json,os,platform,re,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];F=ROOT/'psionic/fixtures/robotics/locomotion-lqr-longer';OLD=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
ENTRY=ROOT/'psionic/tests/robotics/locomotion-lqr-longer-native.coax';NATIVE=ROOT/'psionic/tests/robotics/locomotion_lqr_longer_native.c';OLD_NATIVE=ROOT/'psionic/tests/robotics/locomotion_lqr_native.c'
EXPERIMENT='9dc911e27fe377e4fa5f219f9ac577ca6c01a183c380ba40d200844d65e3ff87'
def need(x,t):
 if not x:raise ValueError(t)
def identity(p):b=Path(p).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def load(p):return json.loads(Path(p).read_text())
def write(p,x):Path(p).write_text(json.dumps(x,indent=2,allow_nan=False)+'\n')
def graph(p,r=None):
 r={} if r is None else r;p=Path(p).resolve();name=str(p.relative_to(ROOT))
 if name not in r:
  r[name]=identity(p)
  for q in re.findall(r'#load\s+"([^\"]+)"',p.read_text()):graph(p.parent/q,r)
 return r
def snapshot():
 need(identity(F/'experiment.json')['sha256']==EXPERIMENT,'sealed extension experiment');e=load(F/'experiment.json');oldq=load(OLD/'native-qualification.json');olde=load(OLD/'experiment.json');pinned={}
 for mapping in [e['metadata_inputs'],oldq['source'],olde['carried']]:
  for name,h in mapping.items():need(identity(ROOT/name)==h,'carried original '+name);pinned[str(ROOT/name)]=h
 for n in ['experiment.json','contract.md','input-freeze.json','numeric-carry.json','audit-freeze.json','audit-guard-qualification.json','source-qualification.json','unit-refusals.json','old-call-closure-binding.json']:
  p=F/n;need(p.is_file(),'frozen prerequisite '+n);pinned[str(p)]=identity(p)
 if (F/'old-native-closure-tokens.json').exists():
  pub=load(F/'native-publication.json');need(identity(F/'old-native-closure-tokens.json')==pub['old_C_closure_manifest'],'authenticated published C closure manifest');pinned[str(F/'old-native-closure-tokens.json')]=identity(F/'old-native-closure-tokens.json');pinned[str(F/'native-publication.json')]=identity(F/'native-publication.json')
 q=load(F/'source-qualification.json');need(q['passed'] is True,'new source proof')
 for name,h in q['source'].items():need(identity(ROOT/name)==h,'new qualified source '+name)
 source=graph(ENTRY)
 for p in [NATIVE,OLD_NATIVE,Path(__file__).resolve()]:source[str(p.relative_to(ROOT))]=identity(p)
 return e,{'source':source,'frozen':pinned}
# Generated-C token comparison permits ONLY local temporary alpha-renaming and
# diagnostic merged-line→physical-source mapping. Numeric literals/operators stay.
TOKEN=re.compile(r'/\*.*?\*/|//[^\n]*|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'|[A-Za-z_]\w*|(?:0[xX][0-9A-Fa-f.]+(?:[pP][+-]?\d+)?|\d+(?:\.\d*)?(?:[eE][+-]?\d+)?)|[^\s]',re.S)
def tokens(s):return [x.group() for x in TOKEN.finditer(s) if not x.group().startswith(('//','/*'))]
def functions(text):
 out={}
 for m in re.finditer(r'(?m)^[A-Za-z_][A-Za-z0-9_ \t*]*[ \t*]([A-Za-z_]\w*)\s*\(([^\n;{}]*)\)\s*\{',text):
  start=m.start();depth=0;end=None
  for t in TOKEN.finditer(text,m.end()-1):
   v=t.group()
   if v=='{':depth+=1
   elif v=='}':
    depth-=1
    if depth==0:end=t.end();break
  if end:out[m.group(1)]=(m.group(2),tokens(text[start:end]))
 return out
def normalized(text):
 fs=functions(text);lines=[]
 for a,b,c,d,path in re.findall(r'\{(\d+),(\d+),(\d+),(\d+),"([^"]+)"\}',text):lines.append((int(a),int(b),int(c),int(d),path))
 diagnostics={n for n,(args,_) in fs.items() if re.fullmatch(r'coax_(?:trap|index|slice_check|byte|substr|to_f32|(?:cast|ftoi|elem|mkview|reslice|to_t)_[A-Za-z0-9_]+)',n) and re.search(r'int32_t\s+(?:l|line)\s*$',args)}
 diagnostics.update('coax_'+op+'_'+sign+str(width) for op in ['add','sub','mul','neg','div','mod','shl','shr'] for sign in ['s','u'] for width in [8,16,32,64])
 def location(n):
  for a,b,c,d,p in lines:
   if a<=n<=b:return 'DIAGNOSTIC:'+p+':'+str(c if d else c+n-a)
  return 'DIAGNOSTIC:'+str(n)
 result={}
 for name,(args,ts) in fs.items():
  ts=list(ts)
  for i,t in enumerate(ts):
   if t not in diagnostics or i+1>=len(ts) or ts[i+1]!='(':continue
   depth=0;last=i+2;close=None
   for j in range(i+1,len(ts)):
    if ts[j]=='(':depth+=1
    elif ts[j]==')':
     depth-=1
     if depth==0:close=j;break
    elif ts[j]==',' and depth==1:last=j+1
   if close is not None and close-last==1 and ts[last].isdigit():ts[last]=location(int(ts[last]))
  temps={};declared=set()
  for i,t in enumerate(ts):
   if not re.fullmatch(r'coax_(?:t|c|i|lo|hi)\d+',t):continue
   prior=ts[i-1] if i else '';prior=ts[i-2] if prior=='*' and i>1 else prior
   if re.fullmatch(r'(?:double|float|_Bool|u?int(?:8|16|32|64)_t|size_t|coax_[A-Za-z0-9_]+)',prior):declared.add(t)
  for i,t in enumerate(ts):
   if t in declared:ts[i]=temps.setdefault(t,'LOCAL_TEMP_'+str(len(temps)))
  result[name]=ts
 return result
ROOT_FUNCTIONS=['rle_rollout','rlt_epoch','rlp_coefficients','rls_evaluate','rls_score','coax_p_rlt_state_ok','coax_p_rlt_detach']
def digest_tokens(ts):return {'tokens':len(ts),'sha256':hashlib.sha256('\0'.join(ts).encode()).hexdigest()}
def support(text,header=False):
 lines=text.splitlines(True);kept=[];macros=[];i=0
 while i<len(lines):
  line=lines[i]
  if line.lstrip().startswith('#') or line.startswith('COAX_INT_OPS('):
   block=line;i+=1
   while block.rstrip().endswith('\\') and i<len(lines):block+=lines[i];i+=1
   macros.append(digest_tokens(tokens(block)));continue
  kept.append(line);i+=1
 text=''.join(kept)
 text=re.sub(r'static const char \*coax_source_path\s*=\s*"(?:\\.|[^"\\])*";','',text)
 text=re.sub(r'struct coax_source_range\s*\{[^}]*\};','',text)
 text=re.sub(r'static const struct coax_source_range coax_source_ranges\[\]\s*=\s*\{.*?\n\};','',text,flags=re.S)
 # Remove actual function definitions, preserving nonfunction globals/types.
 intervals=[]
 for m in re.finditer(r'(?m)^[A-Za-z_][A-Za-z0-9_ \t*]*[ \t*]([A-Za-z_]\w*)\s*\(([^\n;{}]*)\)\s*\{',text):
  depth=0
  for t in TOKEN.finditer(text,m.end()-1):
   if t.group()=='{':depth+=1
   elif t.group()=='}':
    depth-=1
    if depth==0:intervals.append((m.start(),t.end()));break
 for a,b in sorted(intervals,reverse=True):text=text[:a]+text[b:]
 text=re.sub(r'(?m)^extern "C" \{\s*$|^\}\s*$','',text)
 ts=tokens(text);depth=0;part=[];decl=[]
 for token in ts:
  part.append(token)
  if token=='{':depth+=1
  if token=='}':depth-=1
  if token==';' and depth==0:
   prototype='(' in part and '=' not in part and part[0] not in ['typedef','struct','enum','COAX_LAYOUT']
   if header or not prototype:decl.append(digest_tokens(part))
   part=[]
 return {'preprocessor_and_integer_instantiations':macros,'declarations':sorted(decl,key=lambda x:(x['sha256'],x['tokens']))}
def header_guards(text):
 lines=text.splitlines(True);outside=[];blocks={};i=0
 while i<len(lines):
  m=re.match(r'#ifdef (COAX_[A-Za-z0-9_]+_DEFINED)\s*$',lines[i])
  if not m:
   if lines[i].lstrip().startswith('#'):outside.append(digest_tokens(tokens(lines[i])))
   i+=1;continue
  name=m.group(1);need(name not in blocks,'duplicate/overriding type guard');depth=0;body=[]
  while i<len(lines):
   line=lines[i];body.append(line);i+=1
   if re.match(r'\s*#(?:if|ifdef|ifndef)\b',line):depth+=1
   if re.match(r'\s*#endif\b',line):
    depth-=1
    if depth==0:break
  need(depth==0,'complete guarded ABI declaration');blocks[name]=digest_tokens(tokens(''.join(body)))
 return {'outside_macros_ordered':outside,'named_type_blocks':blocks}
def subset(actual,expected,role):
 from collections import Counter
 a=Counter((x['sha256'],x['tokens']) for x in actual);e=Counter((x['sha256'],x['tokens']) for x in expected);need(not(e-a),'changed/missing '+role);return len(expected)
def closure_manifest(text,header):
 fs=normalized(text);pending=list(ROOT_FUNCTIONS);seen=set()
 while pending:
  name=pending.pop();need(name in fs,'old function definition '+name)
  if name in seen:continue
  seen.add(name);t=fs[name];pending.extend(t[i] for i in range(len(t)-1) if t[i+1]=='(' and t[i] in fs and t[i] not in seen)
 raw=functions(text);runtime={n:digest_tokens(t) for n,(args,t) in raw.items() if n in seen and n.startswith('coax_') and re.search(r'int32_t\s+(?:l|line)\s*$',args)}
 omitted=sorted(n for n,(args,t) in raw.items() if n not in seen and n.startswith('coax_') and re.search(r'int32_t\s+(?:l|line)\s*$',args))
 return {'functions':{n:digest_tokens(fs[n]) for n in sorted(seen)},'C_support':support(text),'header_support':support(header,True),'header_guard_blocks':header_guards(header),'diagnostic_runtime_bodies':runtime,'unreachable_diagnostic_helpers':omitted}
def compare_closure(text,header,manifest):
 # Validate runtime line-taking helper bodies BEFORE normalizing any call sites.
 raw=functions(text)
 for n,h in manifest['diagnostic_runtime_bodies'].items():need(n in raw and digest_tokens(raw[n][1])==h,'exact diagnostic runtime body '+n)
 cs,hs=support(text),support(header,True);counts={}
 for role,actual,want in [('C_macros',cs['preprocessor_and_integer_instantiations'],manifest['C_support']['preprocessor_and_integer_instantiations']),('C_globals_types',cs['declarations'],manifest['C_support']['declarations']),('header_macros',hs['preprocessor_and_integer_instantiations'],manifest['header_support']['preprocessor_and_integer_instantiations']),('header_types_layouts_prototypes',hs['declarations'],manifest['header_support']['declarations'])]:
  if role=='C_macros':need(actual==want,'exact ordered C runtime macro/support sequence without overrides')
  counts[role]=subset(actual,want,role)
 hg=header_guards(header);want=manifest['header_guard_blocks'];need(hg['outside_macros_ordered']==want['outside_macros_ordered'],'exact ordered header outer directives');need(all(hg['named_type_blocks'].get(n)==h for n,h in want['named_type_blocks'].items()),'exact old whole header type/schema/guard blocks; only independent block order differs');counts['whole_header_guard_blocks']=len(want['named_type_blocks'])
 fs=normalized(text);changed=[n for n,h in manifest['functions'].items() if n not in fs or digest_tokens(fs[n])!=h];need(not changed,'changed carried generated-C closure '+repr(changed));return {'functions':len(manifest['functions']),'changed':changed,'omitted_UNREACHABLE_diagnostic_helpers':[n for n in manifest['unreachable_diagnostic_helpers'] if n not in raw],'support':counts,'normalization':'declared local temp/loop alpha names and verified generated-runtime diagnostic physical source lines ONLY'}
def main():
 p=argparse.ArgumentParser();p.add_argument('--coax',required=True,type=Path);p.add_argument('--output',required=True,type=Path);p.add_argument('--original-math',type=Path,default=Path('/tmp/locomotion-lqr-native-first'));p.add_argument('--plan',action='store_true');p.add_argument('--resume',action='store_true');a=p.parse_args();out=a.output.resolve();need(a.resume or not out.exists(),'preserve first attempt');out.mkdir(parents=True,exist_ok=a.resume);r={'schema':'psionic.locomotion-lqr-longer.native.v1','passed':False,'plan_only':a.plan,'commands':[],'modes':{},'full_batches_plant_optimizer_RNG_tape':0,'old_tests_math_model_calls':0};began=time.monotonic()
 if a.resume:
  r=load(out/'first-attempt.record.json');need(not r['passed'] and 'old function definition rlt_detach' in r.get('error',''),'only documented failed metadata phase');r.pop('error');r['resume']={'carry':identity(out/'debug-carry.json'),'changed_driver':identity(__file__),'debug_emission_repeated':False,'metadata_failures':[identity(out/'first-attempt.record.json'),identity(out/'second-attempt.record.json'),identity(out/'third-attempt.record.json'),identity(out/'fourth-attempt.record.json')],'scope':'Actual private-symbol spellings corrected; only unreachable pruned enum conversion helpers omitted from body binding. Reachable closure/macros/types remain exact.'}
 def save():write(out/'record.json',r)
 def cmd(label,args,env=None):
  x={'label':label,'argv':list(map(str,args))};r['commands'].append(x);start=time.monotonic()
  try:
   z=subprocess.run(x['argv'],cwd=ROOT,capture_output=True,env=env,timeout=180);(out/(label+'.stdout')).write_bytes(z.stdout);(out/(label+'.stderr')).write_bytes(z.stderr);x.update(exit=z.returncode,seconds=time.monotonic()-start);need(z.returncode==0 and not z.stderr,label+' exit/empty stderr');return z.stdout
  except subprocess.TimeoutExpired as e:(out/(label+'.stdout')).write_bytes(e.stdout or b'');(out/(label+'.stderr')).write_bytes(e.stderr or b'');x['timed_out']=True;raise
  finally:save()
 save()
 try:
  e,before=snapshot();
  if a.resume:
   carry=load(out/'debug-carry.json');need(before['frozen']==carry['frozen'] and identity(a.coax)==carry['compiler'],'passed debug frozen/compiler identities');need(all(before['source'][n]==h for n,h in carry['source'].items() if n!=str(Path(__file__).relative_to(ROOT))),'only driver resolver changed');need(before['source'][str(NATIVE.relative_to(ROOT))]==carry['source'][str(NATIVE.relative_to(ROOT))],'unchanged native C');
  r.update(before,experiment=identity(F/'experiment.json'),compiler=identity(a.coax));need(platform.system()=='Darwin' and platform.machine()=='arm64' and sys.byteorder=='little','named host')
  if not a.plan:
   
   if not a.resume:r['host_compiler']=cmd('host-compiler',[os.environ.get('CC','cc'),'--version']).decode()
   oldq=load(OLD/'native-qualification.json');r['emissions']={};manifests={};em={}
   published=F/'old-native-closure-tokens.json'
   for mode,extra in [('debug',[]),('release',['--release'])]:
    d=out/mode;c,h=d/'locomotion-lqr-longer.c',d/'locomotion-lqr-longer.h';alias=d/'locomotion-lqr.h'
    if a.resume and mode=='debug':
     need(all(identity(d/n)==v for n,v in carry['debug_artifacts'].items()) and identity(out/'emit-debug.stdout')==carry['debug_stdout'],'passed debug C/H retained')
    else:
     d.mkdir();c.write_bytes(cmd('emit-'+mode,[a.coax,'c',ENTRY.relative_to(ROOT),'--library',*extra,'--header',h]));alias.write_bytes(h.read_bytes())
    need(identity(alias)==identity(h),'private old-basename header alias')
    if published.exists():manifest=load(published)['modes'][mode]
    else:
     old=a.original_math/mode/'locomotion-lqr.c';need(identity(old)==oldq['emissions'][mode]['C'],'exact original908 generated source');old_header=a.original_math/mode/'locomotion-lqr.h';need(identity(old_header)==oldq['emissions'][mode]['header'],'exact original header');manifest=closure_manifest(old.read_text(),old_header.read_text())
    manifests[mode]=manifest;r['emissions'][mode]={'C':identity(c),'header':identity(h),'header_alias':identity(alias),'carried_closure':compare_closure(c.read_text(),h.read_text(),manifest)};em[mode]=(c,h)
   write(out/'old-native-closure-tokens.json',{'schema':'psionic.locomotion-lqr-longer.old-C-closure.v1','original_math_qualification':identity(OLD/'native-qualification.json'),'roots':ROOT_FUNCTIONS,'modes':manifests})
   common=['-std=c11','-Wall','-Wextra','-Werror','-ffp-contract=off','-fno-fast-math',*['-fno-builtin-'+x for x in ['exp','log','sqrt','cos','pow']]];payload=[]
   for mode,sm,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
    d=out/('native-'+mode);d.mkdir();c,h=em[sm];cc=os.environ.get('CC','cc');cmd('compile-generated-'+mode,[cc,*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',d/'coax.o']);cmd('compile-effects-'+mode,[cc,*common,*flags,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',d/'native']);env=os.environ.copy();env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');report=json.loads(cmd('execute-guards-'+mode,[d/'native','guards',d],env));need(report['passed'] and report['full_batches']==0,'guard-only native scope');raw=(d/'synthetic-resume.bin').read_bytes();need(len(raw)==440,'synthetic resumed state extent');payload.append(raw);r['modes'][mode]={'native':report,'binary':identity(d/'native'),'C':identity(c),'header':identity(h),'synthetic_resume':identity(d/'synthetic-resume.bin')};save()
   need(all(x==payload[0] for x in payload),'all3guard outputs exact');r['all_mode_output_bytes_exact']=True
   for mode,(c,h) in em.items():need(identity(c)==r['emissions'][mode]['C'] and identity(h)==r['emissions'][mode]['header'] and identity(h.parent/'locomotion-lqr.h')==identity(h),'final generated identities')
  _,after=snapshot();need(before==after and identity(a.coax)==r['compiler'],'whole context unchanged');r['passed']=True
 except BaseException as x:r['error']=repr(x);raise
 finally:r['seconds']=time.monotonic()-began;r['driver']=identity(__file__);save()
 print(json.dumps({'passed':True,'record':identity(out/'record.json'),'plan_only':a.plan}))
if __name__=='__main__':main()

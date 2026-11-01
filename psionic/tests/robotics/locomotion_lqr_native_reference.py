#!/usr/bin/env python3
"""Fixture-only native LQR qualification. Explicit learning is a separate tool."""
from __future__ import annotations
import argparse,hashlib,importlib.util,json,math,os,platform,re,struct,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
F=ROOT/'psionic/fixtures/robotics/locomotion-lqr'
ENTRY=ROOT/'psionic/tests/robotics/locomotion-lqr-native.coax'
NATIVE=ROOT/'psionic/tests/robotics/locomotion_lqr_native.c'
PRODUCER=ROOT/'psionic/tests/robotics/locomotion_lqr_reference.py'
EXPERIMENT='8f002e3f4192a29f249e3a0e3ec68ef50d29542eb91197b96eaaec5bf117194c'
FORMATS={'actor':'<q3d3q','critic':'<q2d','transition':'<q8d2q3d2q','projection':'<qd','epoch':'<q3d2q3d4q','eval':'<q7dq','score':'<q23d2q'}
FLOATS={'actor':[1,2,3],'critic':[1,2],'transition':list(range(1,9))+[11,12,13],'projection':[1],'epoch':[1,2,3,6,7,8],'eval':list(range(1,8)),'score':list(range(1,24))}
COUNTS={'actor':112,'critic':9,'transition':81,'projection':54,'epoch':24,'eval':24,'score':27}
ROW=struct.Struct('<8d2q3d2q')
def need(c,t):
 if not c:raise ValueError(t)
def identity(p):
 b=Path(p).read_bytes();return {'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def load(p):return json.loads(Path(p).read_text())
def write(p,x):Path(p).write_text(json.dumps(x,indent=2,allow_nan=False)+'\n')
def finite(xs,n,role):need(len(xs)==n and all(math.isfinite(x) for x in xs),role+' ALL finite/exact extent')
def records(raw,fmt,n,role):
 f=struct.Struct(fmt);need(len(raw)==n*f.size,role+' extent before decode');return list(f.iter_unpack(raw))
def bits(x):return struct.pack('<d',x)
def graph(p,r=None):
 r={} if r is None else r;p=Path(p).resolve();name=str(p.relative_to(ROOT))
 if name not in r:
  r[name]=identity(p)
  for q in re.findall(r'#load\s+"([^\"]+)"',p.read_text()):graph(p.parent/q,r)
 return r
def reader():
 s=importlib.util.spec_from_file_location('independent_lqr',PRODUCER);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def snapshot(oracle):
 need(identity(F/'experiment.json')['sha256']==EXPERIMENT,'sealed fixture/learning recipe')
 e,se,ref=load(F/'experiment.json'),load(F/'input-freeze.json'),load(oracle/'reference.json')
 need(se['producer']==identity(PRODUCER) and se['experiment']==identity(F/'experiment.json'),'whole producer freeze')
 need(ref['passed'] is True and ref['schema']=='psionic.locomotion-lqr.reference.v1' and ref['producer']==se['producer'] and ref['experiment']==se['experiment'] and ref['counts']==e['counts'],'first reference association')
 pinned={}
 for name,h in e['input_files'].items():need(identity(F/name)==h,'frozen input '+name);pinned[str(F/name)]=h
 for name,h in e['carried'].items():need(identity(ROOT/name)==h,'carried unchanged proof '+name);pinned[str(ROOT/name)]=h
 for name,h in ref['outputs'].items():need(identity(oracle/name)==h,'immutable expected '+name);pinned[str(oracle/name)]=h
 need(identity(F/'contract.md')==e['contract'],'sealed contract')
 q=load(F/'source-qualification.json');need(q['passed'] is True and identity(F/'source-qualification.json')==e['new_source_qualification'] and identity(F/'unit-refusals.json')==e['new_unit_refusals'],'new source/unit proof identities')
 for name,h in q['source_files'].items():need(identity(ROOT/name)==h,'qualified new source '+name)
 for name in ['experiment.json','input-freeze.json','contract.md','source-qualification.json','unit-refusals.json']:
  pinned[str(F/name)]=identity(F/name)
 pinned[str(oracle/'reference.json')]=identity(oracle/'reference.json');pinned[str(PRODUCER)]=identity(PRODUCER)
 source=graph(ENTRY)
 for p in [NATIVE,Path(__file__)]:source[str(p.relative_to(ROOT))]=identity(p)
 return e,{'source':source,'frozen':pinned}
def decode(raw,kind):
 rows=records(raw,FORMATS[kind],COUNTS[kind],kind);finite([r[i] for r in rows for i in FLOATS[kind]],len(rows)*len(FLOATS[kind]),kind)
 need(all(r[0]==0 for r in rows),kind+' declared successful fixture statuses')
 return rows
def decode_coefficients(raw,n,role):
 rows=records(raw,'<10dq',n,role);finite([x for r in rows for x in r[:10]],10*n,role);need(all(r[10] in [0,1] for r in rows),role+' exact flag domain');return rows
def decode_rollouts(raw):
 need(len(raw)==7*61880,'rollout fixed extent');out=[];values=[]
 for i in range(7):
  status=struct.unpack_from('<q',raw,i*61880)[0];need(status==0,'rollout status');body=raw[i*61880+8:(i+1)*61880];h=struct.unpack_from('<3q3d',body)
  need(1<=h[0]<=16 and 1<=h[1]<=32 and h[2]==h[0]*h[1],'rollout header dimensions');values+=list(h[3:]);rows=[ROW.unpack_from(body,48+j*120) for j in range(512)];worlds=[struct.unpack_from('<d2Q',body,61488+j*24) for j in range(16)]
  for r in rows:values+=list(r[:8])+list(r[10:13])
  values += [w[0] for w in worlds];out.append((body,h,rows,worlds))
 finite(values,7*(3+512*11+16),'ALL active and unused rollout fields');return out
def close(a,b,absolute=2e-12,relative=2e-12,role='ordinary'):
 need(math.isfinite(a) and math.isfinite(b),'finite comparison '+role);err=abs(a-b);need(err<=absolute+relative*abs(b),role+' scalar budget');return err
def compare(actual,oracle,e,m):
 expected={k:decode((oracle/(k+'-expected.bin')).read_bytes(),k) for k in FORMATS}
 got={k:decode(actual[k+'-actual.bin'],k) for k in FORMATS};roll_expected=decode_rollouts((oracle/'rollout-expected.bin').read_bytes());roll_actual=decode_rollouts(actual['rollout-actual.bin'])
 probes={kind:decode_coefficients(actual[kind+'-coefficients.bin'],e['counts'][kind+'_rows'],kind+' coefficients') for kind in ['actor','epoch']}
 for kind,rows in probes.items():finite([x for r in rows for x in r[:10]],10*len(rows),kind+' ALL coefficients before reductions')
 inverse=records(actual['inverse-sigma.f64'],'<d',1,'inverse')[0][0];finite([inverse],1,'inverse')
 ac=m.read_records(F/'actor-cases.bin','<2d2q');ar=m.read_records(F/'actor-rows.bin','<4d');ec=m.read_records(F/'epoch-cases.bin','<4d4q');er=m.read_records(F/'epoch-rows.bin','<4d')
 # Admit the COMPLETE coefficient corpus before interpreting any local VJP.
 coeff_reports={};bound=[]
 for name,cases,rows in [('actor',ac,ar),('epoch',ec,er)]:
  reports=[]
  for c in cases:
   K,at,n=(c[0],c[2],c[3]) if name=='actor' else (c[1],c[6],c[7]);reports.append(m.admit_coefficients(K,rows[at:at+n],probes[name][at:at+n],inverse))
  coeff_reports[name]={'rows':len(rows),'density_absolute':[max(r['density_absolute'][j] for r in reports) for j in range(4)],'rho_relative':max(r['rho_relative'] for r in reports)}
 metrics={'actor':[0.,0.,0.],'epoch_loss_gradient':[0.,0.],'epoch_candidate':0.,'ordinary':{},'rollout':0.,'coefficients':coeff_reports}
 for i,(K,old,at,n) in enumerate(ac):
  vals,mags,clip=m.conditional_block(K,ar[at:at+n],probes['actor'][at:at+n],inverse);a=got['actor'][i];need(a[4:]==(n,7+13*n,clip),'actor exact rows/nodes/clips')
  for j in range(3):metrics['actor'][j]=max(metrics['actor'][j],close(a[j+1],vals[j],8e-12+4e-13*mags[j],0.,'analytic raw-sum actor VJP/loss/proxy'))
 for i,c in enumerate(ec):
  old,K,mm,vv,t,skip,at,n=c;totals=[[],[]];mags=[0.,0.];clip=0
  for j in range(0,n,64):
   vals,ms,cc=m.conditional_block(K,er[at+j:at+min(n,j+64)],probes['epoch'][at+j:at+min(n,j+64)],inverse);clip+=cc
   for z in range(2):totals[z].append(vals[z]);mags[z]+=ms[z]
  values=[math.fsum(x)/n for x in totals];a=got['epoch'][i]
  for j,column in enumerate([6,7]):metrics['epoch_loss_gradient'][j]=max(metrics['epoch_loss_gradient'][j],close(a[column],values[j],8e-12+4e-13*mags[j]/n,0.,'independent epoch mean'))
  # The observed gradient is used ONLY after the independent derivative bound.
  candidate,m2,v2,t2,sk2=m.adam_policy((K,mm,vv,t,skip),a[7]);need(bits(a[2])==bits(m2) and bits(a[3])==bits(v2),'exact conditional Adam moments');need(a[4:6]==(t2,sk2),'retained Adam step/skipped')
  metrics['epoch_candidate']=max(metrics['epoch_candidate'],close(a[8],candidate,role='independently bounded Adam candidate'))
  projected=m.project(old,a[8]);need(bits(a[1])==bits(projected),'exact observed-candidate projection');need(a[9:]==(int(a[8]!=projected),n,(n+63)//64,clip),'epoch exact projection/row/block/clip counts')
 for kind in ['critic','transition','projection','eval']:
  maxerr=0.
  for a,b in zip(got[kind],expected[kind]):
   for j in range(len(a)):
    if j in FLOATS[kind]:
     if kind=='projection':need(bits(a[j])==bits(b[j]),'exact projection fixtures')
     else:maxerr=max(maxerr,close(a[j],b[j],role=kind))
    else:need(a[j]==b[j],kind+' exact integer fields')
  metrics['ordinary'][kind]=maxerr
 # Independently bound Pstar, THEN exact local-RN source score and flag policy.
 realP,_=m.real_benchmark()
 for i,((K,),a) in enumerate(zip(m.read_records(F/'score-inputs.f64','<d'),got['score'])):
  close(a[2],float(realP),role='independent Pstar accuracy');want=m.quality_policy(K,a[2]);need(struct.pack('<23d2q',*a[1:])==struct.pack('<23d2q',*want),'exact conditional quality fields/flags')
 world_inputs=m.read_records(F/'rollout-worlds.bin','<d2Q');configs=m.read_records(F/'rollout-cases.bin','<2qd2q')
 for (body,h,rows,worlds),(ebody,eh,ers,ews),conf in zip(roll_actual,roll_expected,configs):
  W,H,K,at,n=conf;need(h[:3]==(W,H,W*H),'frozen rollout case identity');need(body[48+W*H*120:61488]==bytes((512-W*H)*120) and body[61488+W*24:]==bytes((16-W)*24),'unused canonicalzero rollout slots')
  for j in range(3,6):close(h[j],eh[j],role='rollout header')
  for j,(a,b) in enumerate(zip(rows[:W*H],ers[:W*H])):
   for col in list(range(8))+[10,11,12]:metrics['rollout']=max(metrics['rollout'],close(a[col],b[col],2e-11 if col in [11,12] else 2e-12,2e-12,'rollout field'))
   need((a[8],a[9],a[13],a[14])==(0,0,1,int(j%H!=H-1)),'exact GAE masks and source flags')
  for w in range(W):
   close(worlds[w][0],ews[w][0],role='next world x');need(worlds[w][1:]==ews[w][1:],'exact RNG next seed/counter')
 for seed in [0,1,7]:
  raw=actual['initial-'+str(seed)+'.bin'];need(len(raw)==440,'initial state extent');K,mm,vv,t,skip=struct.unpack_from('<3d2q',raw);finite([K,mm,vv],3,'initial optimizer');need((K,mm,vv,t,skip)==(.25,0.,0.,0,0),'initial optimizer fields');need(struct.unpack_from('<qQ',raw,424)==(0,seed),'initial labels')
  for w in range(16):need(struct.unpack_from('<d2Q',raw,40+24*w)==(e['training']['initial'][w],seed,w*8192),'independent initial worlds/counter partitions')
 return metrics

def reader_guards():
 count=0
 for kind in FORMATS:
  fmt=struct.Struct(FORMATS[kind]);rows=[0]*len(fmt.unpack(bytes(fmt.size)));base=bytearray(fmt.pack(*rows)*COUNTS[kind])
  for cut in [-1,1]:
   raw=base[:len(base)-1] if cut<0 else base+b'\0'
   try:decode(raw,kind)
   except ValueError:count+=1
   else:raise ValueError('reader extent poison admitted')
  for j in FLOATS[kind]:
   bad=bytearray(base);struct.pack_into('<Q',bad,8*j,0x7ff8000000000001)
   try:decode(bad,kind)
   except ValueError:count+=1
   else:raise ValueError('reader NaN poison admitted')
 coefficient_base=count+2
 coefficient=bytearray(88)
 for raw in [coefficient[:-1],coefficient+b'\0',*[bytearray(coefficient) for _ in range(10)],bytearray(coefficient)]:
  index=count
  if len(raw)==88:
   lane=index-coefficient_base
   if lane<10:struct.pack_into('<Q',raw,lane*8,0x7ff8000000000001)
   else:struct.pack_into('<q',raw,80,3)
  try:decode_coefficients(raw,1,'synthetic coefficient')
  except ValueError:count+=1
  else:raise ValueError('coefficient reader poison admitted')
 raw=bytearray(7*61880)
 for i in range(7):struct.pack_into('<3q3d',raw,i*61880+8,1,1,1,.5,0.,0.)
 for bad in [raw[:-1],raw+b'\0',bytearray(raw)]:
  if len(bad)==len(raw):struct.pack_into('<Q',bad,6*61880+8+61488+15*24,0x7ff8000000000001)
  try:decode_rollouts(bad)
  except ValueError:count+=1
  else:raise ValueError('rollout reader poison admitted')
 return {'passed':True,'poisons_refused':count,'scope':'exact extents, every ordinary/coefficient numeric role, and late unused rollout payload'}
def main():
 p=argparse.ArgumentParser();p.add_argument('--coax',type=Path,default=Path(os.environ.get('COAX',ROOT/'coax/bin/coax')));p.add_argument('--oracle',type=Path,default=F);p.add_argument('--output',required=True,type=Path);p.add_argument('--plan',action='store_true');a=p.parse_args();out=a.output.resolve();need(not out.exists(),'preserve first attempt');out.mkdir(parents=True)
 r={'schema':'psionic.locomotion-lqr.native.v1','passed':False,'plan_only':a.plan,'commands':[],'modes':{},'learning_calls':0,'old_gates_model_SDK_calls':0};write(out/'record.json',r);began=time.monotonic()
 def command(label,argv,env=None):
  item={'label':label,'argv':list(map(str,argv))};r['commands'].append(item);t=time.monotonic()
  try:
   q=subprocess.run(item['argv'],cwd=ROOT,env=env,capture_output=True,timeout=180);item.update(exit=q.returncode,seconds=time.monotonic()-t);(out/(label+'.stdout')).write_bytes(q.stdout);(out/(label+'.stderr')).write_bytes(q.stderr);need(q.returncode==0 and not q.stderr,label+' exit/empty stderr');return q.stdout
  except subprocess.TimeoutExpired as x:
   item.update(timed_out=True,seconds=time.monotonic()-t);(out/(label+'.stdout')).write_bytes(x.stdout or b'');(out/(label+'.stderr')).write_bytes(x.stderr or b'');raise
  finally:write(out/'record.json',r)
 try:
  e,before=snapshot(a.oracle);r.update(before,experiment=identity(F/'experiment.json'),reference=identity(a.oracle/'reference.json'),compiler=identity(a.coax),counts=e['counts'])
  if not a.plan:
   need(platform.system()=='Darwin' and platform.machine()=='arm64' and sys.byteorder=='little','named scalar host profile');m=reader();expected_outputs={n:(a.oracle/n).read_bytes() for n in load(a.oracle/'reference.json')['outputs'] if n.endswith(('.bin','.f64'))};m.finite_expected_outputs(expected_outputs);r['reader_guards']=reader_guards();r['host_compiler']=command('host-compiler-version',[os.environ.get('CC','cc'),'--version']).decode();r['emissions']={};em={}
   for mode,extra in [('debug',[]),('release',['--release'])]:
    d=out/mode;d.mkdir();c,h=d/'locomotion-lqr.c',d/'locomotion-lqr.h';c.write_bytes(command('emit-'+mode,[a.coax,'c',ENTRY.relative_to(ROOT),'--library',*extra,'--header',h]));em[mode]=(c,h);r['emissions'][mode]={'C':identity(c),'header':identity(h)}
   common=['-std=c11','-Wall','-Wextra','-Werror','-ffp-contract=off','-fno-fast-math',*['-fno-builtin-'+x for x in ['exp','log','sqrt','cos','pow']]];payloads=[]
   for mode,sm,flags in [('debug','debug',['-O0','-g']),('release','release',['-O2']),('sanitized','debug',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
    d=out/('native-'+mode);d.mkdir();c,h=em[sm];cc=os.environ.get('CC','cc');command('compile-generated-'+mode,[cc,*common,'-Wno-unused-parameter',*flags,'-c',c,'-o',d/'coax.o']);command('compile-effects-'+mode,[cc,*common,*flags,'-I'+str(h.parent),NATIVE,d/'coax.o','-lm','-o',d/'native']);env=os.environ.copy();env.update(ASAN_OPTIONS='detect_leaks=0:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1');report=json.loads(command('execute-'+mode,[d/'native','fixture',F,d],env));need(report['passed'] and report['successful_full_training_batches']==0,'fixture-only scope')
    names=[*[k+'-actual.bin' for k in FORMATS],'rollout-actual.bin','actor-coefficients.bin','epoch-coefficients.bin','inverse-sigma.f64',*[f'initial-{s}.bin' for s in [0,1,7]]];streams={n:(d/n).read_bytes() for n in names};cmp=compare(streams,a.oracle,e,m);r['modes'][mode]={'native':report,'comparison':cmp,'binary':identity(d/'native'),'C':identity(c),'header':identity(h),'outputs':{n:identity(d/n) for n in names}};payloads.append(streams);write(out/'record.json',r)
   need(all(x==payloads[0] for x in payloads),'all fixture/coefficient/initial streams byte-identical across profiles');r['all_mode_output_bytes_exact']=True
   for mode,(c,h) in em.items():need(r['emissions'][mode]=={'C':identity(c),'header':identity(h)},'final emitted identities')
  _,after=snapshot(a.oracle);need(before==after and identity(a.coax)==r['compiler'],'before/after whole context');r['passed']=True
 except BaseException as x:r['error']=type(x).__name__+': '+str(x);raise
 finally:r['seconds']=time.monotonic()-began;write(out/'record.json',r)
 print(json.dumps({'passed':True,'record':str(out/'record.json'),'plan_only':a.plan}))
if __name__=='__main__':main()

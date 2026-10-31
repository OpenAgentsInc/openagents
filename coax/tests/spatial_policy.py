#!/usr/bin/env python3
"""Independent explicit relation-table oracle; no spatial producer imports."""
from collections import Counter
from pathlib import Path
import hashlib,itertools,json,os,platform,random,struct,subprocess,tempfile,time
ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'coax/lib/compiler/spatial-policy.coax';NATIVE=ROOT/'coax/tests/spatial-policy-native.c'
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
MAX=(1<<31)-1;SEED=0x5350415449414c33;ROWS=98064
FRAME=(1,1,1,32);METRE=(0,1,0,0,1,1);RAW=(0,0,0,32);ZERO=(0,)*4
FRAMES={(h,c,t,r) for h,c,t,r in itertools.product([-1,1],[1,2],[1,2],[32,64])}|{(1,3,t,r) for t,r in itertools.product([1,2],[32,64])}
RELATION={(0,1,2):1,(0,2,1):1,(0,2,2):2,(1,1,1):2,(1,1,2):1,(1,2,2):2,(2,2,0):2,(2,0,2):2,(3,2,0):2}
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def valid_type(a):return a[0] in [0,1,2] and a[3] in [32,64] and ((a[1:3]==(0,0)) if a[0]==0 else all(1<=x<=MAX for x in a[1:3]))
def valid_unit(u):return all(-8<=x<=8 for x in u[:4]) and all(1<=x<=1048576 for x in u[4:])
def expected(row):
 dispatch,op=row[:2];f=tuple(row[2:6]);a=tuple(row[6:10]);b=tuple(row[10:14]);u=row[14:20]
 if dispatch==0:return (0,*ZERO,int(f in FRAMES))
 if dispatch==1:return (0,*ZERO,int(valid_type(a)))
 if not valid_type(a) or (dispatch==3 and not valid_type(b)):return (1,*ZERO,0)
 if dispatch==2:
  status=0
  if a[0]!=0:
   if f not in FRAMES or not valid_unit(u):status=2
   elif a[0]==1 and u[:4]!=(0,1,0,0):status=3
   elif a[3]<f[3]:status=4
  return (status,*ZERO,0)
 if dispatch==4:return (0,*a,0) if a[0]==2 else (6,*ZERO,0)
 out=RELATION.get((op,a[0],b[0]))
 if out is None:return (6,*ZERO,0)
 if a[3]!=b[3]:return (4,*ZERO,0)
 if a[0]!=0 and b[0]!=0:
  if a[1]!=b[1]:return (5,*ZERO,0)
  if a[2]!=b[2]:return (3,*ZERO,0)
 source=a if a[0]!=0 else b
 return (0,out,*source[1:],0)
def cases():
 rows=[]
 def add(dispatch,op=0,f=FRAME,a=RAW,b=RAW,u=METRE):rows.append((dispatch,op,*f,*a,*b,*u))
 def frame(f):
  add(0,f=f);add(2,f=f,a=(1,1,2,32));add(2,f=f,a=(2,1,2,64))
 for h,c,t,r in itertools.product([-1,1],range(256),[1,2],[32,64]):frame((h,c,t,r))
 for h,c,t,r in itertools.product([-1,1],[1,2,3],range(256),[32,64]):frame((h,c,t,r))
 for h,c,t,r in itertools.product([-(1<<63),-2,0,2,(1<<63)-1],[1,2,3],[1,2],[32,64]):frame((h,c,t,r))
 for h,c,t,r in itertools.product([-1,1],[1,2,3],[1,2],[-(1<<63),0,31,33,63,65,(1<<63)-1]):frame((h,c,t,r))
 for k,r in itertools.product(range(256),[0,31,32,33,63,64,65,(1<<63)-1]):
  a=(k,0,0,r) if k==0 else (k,1,2,r)
  for dispatch in [1,2,4]:add(dispatch,a=a)
 ids=[-(1<<63),-1,0,1,2,MAX,MAX+1,(1<<63)-1]
 for k,f,u,r in itertools.product(range(3),ids,ids,[32,64]):
  a=(k,f,u,r)
  for dispatch in [1,2,4]:add(dispatch,a=a)
  for op in [0,2]:add(3,op=op,a=a);add(3,op=op,b=a)
 for axis in range(6):
  values=[-9,-8,0,1,8,9,-(1<<63),(1<<63)-1] if axis<4 else [-1,0,1,2,1048576,1048577,-(1<<63),(1<<63)-1]
  for value,k,r,minrep in itertools.product(values,[0,1,2],[32,64],[32,64]):
   u=list(METRE);u[axis]=value;a=(k,0,0,r) if k==0 else (k,1,2,r)
   add(2,f=(1,1,1,minrep),a=a,u=u)
 for op,k1,k2,width,facts in itertools.product(range(256),range(3),range(3),[32,64],range(8)):
  a=(k1,1,2,width) if k1 else (0,0,0,width)
  b=(k2,3 if facts&2 else 1,4 if facts&4 else 2,96-width if facts&1 else width) if k2 else (0,0,0,96-width if facts&1 else width)
  add(3,op=op,a=a,b=b)
 rng=random.Random(SEED)
 for _ in range(12000):
  def descriptor():
   k=rng.randrange(3);r=rng.choice([32,64]);return (k,rng.choice([1,2,MAX]),rng.choice([1,3,MAX]),r) if k else (0,0,0,r)
  a=descriptor();b=descriptor();add(3,op=rng.randrange(9),a=a,b=b);add(4,a=a)
  u=tuple(rng.randrange(-8,9) for _ in range(4))+(rng.randrange(1,1048577),rng.randrange(1,1048577))
  add(2,f=rng.choice(tuple(sorted(FRAMES))),a=a,u=u)
 return rows
def main():
 work=Path(os.environ.get('COAX_SPATIAL_POLICY_RECORD_DIR',tempfile.mkdtemp(prefix='coax-spatial-policy-')));work.mkdir(parents=True,exist_ok=True)
 rows=cases();assert len(rows)==ROWS,(len(rows),ROWS)
 # Every native enum transport value fits u8, including undeclared nonmembers.
 assert all(0<=r[1]<=255 and 0<=r[3]<=255 and 0<=r[4]<=255 and 0<=r[6]<=255 and 0<=r[10]<=255 for r in rows)
 want=[expected(r) for r in rows];assert set(r[0] for r in want)==set(range(7))
 (work/'inputs.bin').write_bytes(struct.pack('<Q',len(rows))+b''.join(struct.pack('<20q',*r) for r in rows))
 truth=b''.join(struct.pack('<6q',*r) for r in want);(work/'expected.bin').write_bytes(truth)
 record={'schema':'coax-spatial-policy-qualification-v1','seed':SEED,'rows':len(rows),'dispatch_counts':dict(Counter(['frame_valid','type_valid','admit','binary','negate'][r[0]] for r in rows)),
  'status_counts':dict(Counter(['ok','type','metadata','unit','precision','frame','operation'][r[0]] for r in want)),
  'oracle':'Independent finite relation table, explicit allowed frame set and arbitrary-precision metadata validation; no producer imports or coordinate arithmetic.',
  'scope':'Pure M3 metadata/operator producer only, no parser/checker/source operators, physical coordinates, ABI of points/vectors, transforms or temporal epochs.',
  'platform':platform.platform(),'machine':platform.machine(),'compiler_sha256':sha(COMPILER),'input_sha256':sha(work/'inputs.bin'),'expected_sha256':sha(work/'expected.bin'),
  'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in [SOURCE,NATIVE,Path(__file__).resolve(),ROOT/'coax/lib/compiler/quantity.coax']},'commands':[],'modes':[]}
 def command(name,args):
  start=time.monotonic();p=subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,timeout=180)
  (work/(name+'.stdout')).write_bytes(p.stdout);(work/(name+'.stderr')).write_bytes(p.stderr)
  record['commands'].append({'name':name,'arguments':[str(x) for x in args],'exit':p.returncode,'elapsed_seconds':time.monotonic()-start});(work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
  assert p.returncode==0,(name,p.stderr.decode(errors='replace'))
  if name.startswith('run-'):assert not p.stderr,(name,p.stderr.decode(errors='replace'))
  return p
 record['compiler_version']=command('compiler-version',[COMPILER,'version']).stdout.decode().strip();cc=os.environ.get('CC','cc');record['c_compiler_version']=command('cc-version',[cc,'--version']).stdout.decode()
 generated=command('generate-c',[COMPILER,'c',SOURCE,'--library','--header',work/'spatial-policy.h']);(work/'spatial-policy.c').write_bytes(generated.stdout)
 record['generated_c_sha256']=sha(work/'spatial-policy.c');record['generated_header_sha256']=sha(work/'spatial-policy.h')
 for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer'])]:
  command('compile-'+mode,[cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-I'+str(work),work/'spatial-policy.c',NATIVE,'-lm','-o',work/mode])
  command('run-'+mode,[work/mode,work/'inputs.bin',work/(mode+'.bin')]);have=(work/(mode+'.bin')).read_bytes();assert len(have)==len(truth),(mode,len(have),len(truth))
  if have!=truth:
   for i,(actual,expected_row) in enumerate(zip(struct.iter_unpack('<6q',have),want)):assert actual==expected_row,(mode,i,rows[i],actual,expected_row)
   raise AssertionError('different transcript')
  record['modes'].append({'mode':mode,'passed':True,'exact_results':len(rows),'exact_words':len(rows)*6,'output_sha256':sha(work/(mode+'.bin')),'stderr_bytes':0})
 record['passed']=True;(work/'receipt.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'passed':True,'rows':len(rows),'words_per_mode':len(rows)*6,'record':str(work/'receipt.json')}))
if __name__=='__main__':main()

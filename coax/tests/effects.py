#!/usr/bin/env python3
"""Focused compiler effect inventory checks; no unrelated compiler gate runs.

The policy oracle computes reachability with Floyd-Warshall and enumerates
simple paths for shortest lexicographic witnesses, independently of Coax's
BFS and topological-removal implementation. CLI checks use resolved programs.
"""
from pathlib import Path
import hashlib,json,os,random,struct,subprocess,tempfile,unittest
ROOT=Path(__file__).resolve().parents[2]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
POLICY_COMPILER=Path(os.environ.get('COAX_POLICY_COMPILER',COMPILER)).resolve()
MASK={1:5,2:6,3:4,4:13,5:20,6:36,7:68,8:132,9:260,10:13,11:4,12:4}
POISON=1431655765

def command(args,**kw):
 return subprocess.run([str(x) for x in args],cwd=ROOT,capture_output=True,text=True,timeout=180,**kw)

def oracle(n,edges,sites,root,wrapping):
 status=1 if n<1 or not 0<=root<n else 4 if any(not(0<=a<n and 0<=b<n) for a,b in edges) else 5 if any(not(0<=p<n and 1<=k<=12) for p,k in sites) else 0
 if status:return (status,0,0,False,0,[POISON]*n,[POISON]*n,[POISON]*9)
 reach=[[False]*n for _ in range(n)]
 for a,b in edges:reach[a][b]=True
 for k in range(n):
  for i in range(n):
   for j in range(n):reach[i][j]=reach[i][j] or(reach[i][k] and reach[k][j])
 live={i for i in range(n) if i==root or reach[root][i]}
 paths={root:()}
 def enumerate_paths(node,path,visited):
  for i,(a,b) in enumerate(edges):
   if a!=node or b in visited:continue
   candidate=path+(i,)
   if b not in paths or(len(candidate),candidate)<(len(paths[b]),paths[b]):paths[b]=candidate
   enumerate_paths(b,candidate,visited|{b})
 enumerate_paths(root,(),{root})
 parent=[-2]*n;via=[-1]*n;parent[root]=-1
 for node,path in paths.items():
  if path:parent[node]=edges[path[-1]][0];via[node]=path[-1]
 bits=0;w=[]
 for bit in range(9):
  candidates=[]
  for i,(p,k) in enumerate(sites):
   own=0 if wrapping and k==10 else MASK[k]
   if p in live and own&(1<<bit):candidates.append((len(paths[p]),paths[p],i))
  w.append(min(candidates)[2] if candidates else -1)
  if candidates:bits|=1<<bit
 cyclic={i for i in live if reach[i][i]}
 surviving={j for j in live if any(i==j or reach[i][j] for i in cyclic)}
 return(0,bits,len(live),bool(cyclic),len(live-surviving),parent,via,w)

class PolicyNative(unittest.TestCase):
 def test_independent_policy_all_native_modes(self):
  work=Path(os.environ.get('COAX_EFFECTS_RECORD_DIR',tempfile.mkdtemp(prefix='coax-effects-')));work.mkdir(parents=True,exist_ok=True)
  rng=random.Random(0xEF169);cases=[]
  for c in range(240):
   n=1+c%7;edges=[(rng.randrange(n),rng.randrange(n)) for _ in range(c%15)];sites=[(rng.randrange(n),rng.randrange(1,13)) for _ in range(c%12)]
   cases.append((n,edges,sites,rng.randrange(n),c%2))
  cases += [(2,[(0,2)],[],0,0),(2,[(-1,1)],[],0,0),(2,[],[(2,1)],0,0),(2,[],[(0,0)],0,0),(2,[],[(0,13)],0,0),(2,[],[],2,0),(0,[],[],0,0)]
  data=struct.pack('<I',len(cases))
  for n,edges,sites,root,wrap in cases:
   data+=struct.pack('<5q',n,len(edges),len(sites),root,wrap)+b''.join(struct.pack('<2q',*x) for x in edges+sites)
  (work/'cases.bin').write_bytes(data)
  c=command([POLICY_COMPILER,'c','coax/lib/compiler/effects.coax','--library','--header',work/'effects-policy.h']);self.assertEqual(c.returncode,0,c.stderr);(work/'policy.c').write_text(c.stdout)
  receipt={'schema':'coax-effects-native-v1','cases':len(cases),'oracle':'Floyd-Warshall and independently enumerated simple paths','compiler':str(POLICY_COMPILER),'compiler_sha256':hashlib.sha256(POLICY_COMPILER.read_bytes()).hexdigest(),'modes':[]}
  expected=[oracle(*case) for case in cases];blobs=[]
  for mode,flags in [('debug',['-O0','-g']),('release',['-O2']),('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   result=command([os.environ.get('CC','cc'),'-std=c11',*flags,'-ffp-contract=off','-fno-fast-math','-I'+str(work),work/'policy.c','coax/tests/effects-native.c','-lm','-o',work/mode]);self.assertEqual(result.returncode,0,result.stderr)
   result=command([work/mode,work/'cases.bin',work/(mode+'.bin')]);self.assertEqual(result.returncode,0,result.stderr)
   blob=(work/(mode+'.bin')).read_bytes();blobs.append(blob);at=0
   for case,want in zip(cases,expected):
    status,bits,reached,recursive,removed=struct.unpack_from('<IIqIq',blob,at);at+=28;n=case[0]
    p=list(struct.unpack_from('<'+'q'*n,blob,at));at+=8*n;v=list(struct.unpack_from('<'+'q'*n,blob,at));at+=8*n;w=list(struct.unpack_from('<9q',blob,at));at+=72
    self.assertEqual((status,bits,reached,bool(recursive),removed,p,v,w),want,(mode,case))
   self.assertEqual(at,len(blob));receipt['modes'].append({'mode':mode,'passed':True,'sha256':hashlib.sha256(blob).hexdigest()})
  self.assertEqual(blobs[0],blobs[1]);self.assertEqual(blobs[0],blobs[2]);receipt['passed']=True;(work/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print('effects native receipt:',work/'receipt.json')

class EffectsCLI(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory(prefix='coax-effects-cli-');self.addCleanup(self.tmp.cleanup);self.work=Path(self.tmp.name)
 def report(self,body,proc='root',*flags):
  p=self.work/'case.coax';p.write_text(body+'\nmain :: () {}\n');r=command([COMPILER,'deps',p,'--library','--effects','--proc',proc,*flags]);self.assertEqual(r.returncode,0,r.stderr);return json.loads(r.stdout)
 def kinds(self,r):return {x['kind']:x for x in r['procedures'][0]['effects']}
 def test_transitive_foreign_defer_and_unreachable_procedure(self):
  r=self.report('io :: (x:int)->int #foreign; leaf :: () { r:=io(7); } root :: () { defer { leaf(); } } other :: () { x:int=---; }')
  k=self.kinds(r);self.assertIn('unknown_effects',k);self.assertIn('foreign_call',k);self.assertNotIn('uninitialized_storage',k);self.assertEqual([x['name'] for x in k['foreign_call']['path']],['root','leaf','io'])
 def test_generic_instantiation_follows_resolved_body(self):
  r=self.report('#import "Basic"; helper :: (x:$T) { print("%",x); } root :: () { helper(7); }')
  k=self.kinds(r);self.assertIn('host_io',k);p=k['host_io']['path'];self.assertEqual([x['name'] for x in p],['root','helper']);self.assertTrue(p[1]['instance_key']);self.assertEqual(r['uninstantiated_templates'],0);self.assertEqual(r['generic_templates'],1)
 def test_profile_changes_only_conditional_overflow(self):
  body='root :: (x:int)->int { return x+1; }';a=self.kinds(self.report(body));b=self.kinds(self.report(body,'root','--arithmetic','wrapping'));self.assertIn('may_trap',a);self.assertNotIn('may_trap',b)
  r=self.report('root :: (v:[]int,i:int)->int { return v[i]; }','root','--arithmetic','wrapping');self.assertIn('may_trap',self.kinds(r));self.assertIn('borrowed_read',self.kinds(r))
 def test_owned_arithmetic_is_not_given_a_resource_proof(self):
  r=self.report('root :: (x:f64)->f64 { return x*2; }');self.assertEqual(r['procedures'][0]['effects_mask'],0);self.assertIn('not an allocation or blocking proof',r['unknown_policy'])
 def test_borrowed_write_and_math_obligations(self):
  r=self.report('root :: (v:[]f64,x:f64) { v[0]=sqrt(x); }');k=self.kinds(r);self.assertIn('borrowed_write',k);self.assertIn('intrinsic_resources_unqualified',k);self.assertIn('unknown_effects',k)
 def test_recursive_call_graph_is_inspected_without_execution(self):
  r=self.report('a :: (x:int)->int { if x==0 { return 0; } return b(x-1); } b :: (x:int)->int { return a(x); } root :: ()->int { return a(7); }');self.assertTrue(r['procedures'][0]['reachable_recursion'])
 def test_compile_time_calls_do_not_become_runtime_edges(self):
  r=self.report('c :: ()->int { return 7; } K :: #run c(); root :: ()->int { return K; }');self.assertEqual(r['procedures'][0]['reachable_procedures'],1)
 def test_foreach_reads_and_return_lifetimes_remain_visible(self):
  r=self.report('root :: (v:[]f64)->f64 { total:f64=0; for x:v { total+=x; } return total; }');self.assertIn('borrowed_read',self.kinds(r))
  r=self.report('root :: (v:[]f64)->[]f64 { return v; }');self.assertIn('unknown_effects',self.kinds(r));self.assertEqual(self.kinds(r)['unknown_effects']['site_kind'],11)
 def test_uninstantiated_template_does_not_pollute_runtime_closure(self):
  r=self.report('#import "Basic"; unused :: (x:$T) { print("%",x); } root :: ()->int { return 7; }');self.assertEqual(r['procedures'][0]['effects_mask'],0);self.assertEqual(r['uninstantiated_templates'],1)
 def test_loaded_callee_keeps_physical_witness_file(self):
  (self.work/'leaf.coax').write_text('io :: () #foreign; leaf :: () { io(); }')
  r=self.report('#load "leaf.coax"; root :: () { leaf(); }');k=self.kinds(r);self.assertIn('foreign_call',k);self.assertTrue(k['foreign_call']['site']['unit'].endswith('leaf.coax'));self.assertEqual([x['name'] for x in k['foreign_call']['path']],['root','leaf','io'])
 def test_report_binds_exact_source_bytes(self):
  r=self.report('root :: (x:f64)->f64 { return x*2; }');self.assertEqual(len(r['sources']),1);source=r['sources'][0];self.assertEqual(source['sha256'],hashlib.sha256(Path(source['unit']).read_bytes()).hexdigest())
 def test_inspection_is_opt_in_and_product_c_is_unchanged(self):
  p=self.work/'product.coax';p.write_text('root :: (x:f64)->f64 { return x*x+2; } main :: () {}')
  reference=Path(os.environ.get('COAX_REFERENCE_COMPILER',ROOT/'coax/bin/coax')).resolve()
  a=command([reference,'c',p]);b=command([COMPILER,'c',p]);self.assertEqual(a.returncode,0,a.stderr);self.assertEqual(b.returncode,0,b.stderr);self.assertEqual(a.stdout,b.stdout)
  r=command([COMPILER,'deps',p,'--effects']);self.assertEqual(r.returncode,0,r.stderr);self.assertEqual({x['name'] for x in json.loads(r.stdout)['procedures']},{'root','main'})
 def test_missing_proc_and_conflicting_modes_refuse(self):
  p=self.work/'case.coax';p.write_text('root :: () {}')
  for flags in [('--effects','--proc','absent'),('--effects','--lowering'),('--lowering','--effects'),('--declarations','--effects'),('--effects','--modules')]:
   r=command([COMPILER,'deps',p,'--library',*flags]);self.assertNotEqual(r.returncode,0);self.assertEqual(r.stdout,'')

if __name__=='__main__':unittest.main()

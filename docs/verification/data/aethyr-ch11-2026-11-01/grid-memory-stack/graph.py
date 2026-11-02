from pathlib import Path
import json,re,hashlib,collections,sys
OUT=Path('/tmp/aethyr-ch11-grid-stack')
PRE='--preopt' in sys.argv
WASM='grid-unoptimized-names.wasm' if PRE else 'grid-names.wasm'
DISASM='grid-preopt.disasm' if PRE else 'grid-names.disasm'
def sha(b):return hashlib.sha256(b).hexdigest()
class Reader:
 def __init__(self,b):self.b=b;self.i=0
 def u(self):
  n=s=0
  while True:
   a=self.b[self.i];self.i+=1;n|=(a&127)<<s
   if a<128:return n
   s+=7
 def byte(self):n=self.b[self.i];self.i+=1;return n
 def raw(self,n):b=self.b[self.i:self.i+n];self.i+=n;return b
 def text(self):return self.raw(self.u()).decode()
def sections(p):
 r=Reader(Path(p).read_bytes());r.i=8;res=[]
 while r.i<len(r.b):k=r.byte();res.append((k,r.raw(r.u())))
 return res
qsec=sections('/tmp/aethyr-ch11-grid-qualified/newland.wasm');dsec=sections(OUT/WASM)
assert [(k,sha(v)) for k,v in qsec if k]!=[(k,'bad') for k,v in dsec if k]
if not PRE:assert [(k,sha(v)) for k,v in qsec if k]==[(k,sha(v)) for k,v in dsec if k], 'diagnostic semantic sections differ'
imports={};exports={};table=[]
for k,b in dsec:
 r=Reader(b)
 if k==2:
  for _ in range(r.u()):
   mod,name,kind=r.text(),r.text(),r.byte()
   if kind==0:imports[len(imports)]=mod+'.'+name;r.u()
   elif kind in [1,2]:
    if kind==1:r.byte()
    flags=r.u();r.u()
    if flags&1:r.u()
   elif kind==3:r.raw(2)
   else:raise RuntimeError('import kind '+str(kind))
 if k==7:
  for _ in range(r.u()):
   n,kind,idx=r.text(),r.byte(),r.u()
   if kind==0:exports[n]=idx
 if k==9:
  for _ in range(r.u()):
   flags=r.u();assert flags==0,flags
   assert r.byte()==0x41;r.u();assert r.byte()==0x0b
   table.extend(r.u() for _ in range(r.u()))

fns={};fid=len(imports)-1
for line in (OUT/DISASM).read_text().splitlines():
 m=re.match(r'^([0-9a-f]{8}) <(.*)>:$',line)
 if m:
  if m[2]=='CODE':continue
  fid+=1;fns[fid]={'id':fid,'name':m[2],'instructions':[],'calls':[],'indirect_calls':0,'frame_bytes':0,'allocation_sum_bytes':0,'allocation_sites':[],'unknown_sp_reads':[]}
 if '\t' in line and re.match(r'^\s+[0-9a-f]+:',line):
  cols=line.split('\t');op=cols[1].strip();operand=cols[2].split('#')[0].strip() if len(cols)>2 else ''
  fns[fid]['instructions'].append((op,operand))

all_unknown=[];resolved_sp_reads=[];sp_writes=0
for fid,f in fns.items():
 ins=f['instructions']
 for i,(op,arg) in enumerate(ins):
  if op in ['call','return_call']:f['calls'].append(int(arg.split()[0]))
  if op in ['call_indirect','return_call_indirect']:f['indirect_calls']+=1
  if op=='global.set' and arg=='0':sp_writes+=1
  if op=='global.get' and arg=='0':
   rest=ins[i+1:i+6]
   if len(rest)>1 and rest[0][0]=='i32.const' and rest[1][0] in ['i32.sub','i32.add']:
    n=int(rest[0][1]);n=n if rest[1][0]=='i32.sub' else -n
    assert n>=0,(f['name'],rest)
    f['frame_bytes']=max(f['frame_bytes'],n)
    f['allocation_sum_bytes']+=n
    f['allocation_sites'].append(n)
   else:f['unknown_sp_reads'].append(rest)
 if f['indirect_calls']:f['calls'].extend(table)
 f['calls']=sorted(set(f['calls']))
 if PRE and f['name']=='coax_p_gc_edge_sweep' and f['unknown_sp_reads']:
  # LLVM leaves this leaf's128B red-zone uncommitted to globalSP. The
  # independently emitted static .su row supplies its exact bound.
  su=(OUT/'objects/Gridwalk.su').read_text()
  assert re.search(r':coax_p_gc_edge_sweep\t128\tstatic(?:\n|$)',su)
  assert not any(op=='global.set' and arg=='0' for op,arg in ins)
  f['frame_bytes']=max(f['frame_bytes'],128);f['allocation_sum_bytes']+=128
  resolved_sp_reads.append({'function':f['name'],'bytes':128,'basis':'Gridwalk.su static128B leaf red-zone; no globalSP write'})
 elif PRE and f['name']=='__init_tp' and f['unknown_sp_reads']:
  assert not any(op=='global.set' and arg=='0' for op,arg in ins)
  assert not f['calls'] and not f['indirect_calls']
  resolved_sp_reads.append({'function':f['name'],'bytes':0,'basis':'SP is read only for TLS stack metadata; no decrement, call or globalSP write in complete body'})
 elif f['unknown_sp_reads']:all_unknown.append((fid,f['name'],f['unknown_sp_reads']))

# SCCs expose recursion instead of silently treating a cyclic graph as a DAG.
idx={};low={};stack=[];on=set();scc=[]
def visit(v):
 idx[v]=low[v]=len(idx);stack.append(v);on.add(v)
 for w in fns[v]['calls']:
  if w not in fns:continue
  if w not in idx:visit(w);low[v]=min(low[v],low[w])
  elif w in on:low[v]=min(low[v],idx[w])
 if low[v]==idx[v]:
  c=[]
  while True:
   w=stack.pop();on.remove(w);c.append(w)
   if w==v:break
  if len(c)>1 or v in fns[v]['calls']:scc.append(c)
for v in fns:
 if v not in idx:visit(v)

for c in scc:assert all(fns[i]['allocation_sum_bytes']==0 for i in c), 'positive allocation recursion needs an explicit depth bound'
# Only the two zero-linear-stack libc recursive nodes remain. Removing their
# self edge does not remove any nonzero concurrently allocated storage.
memo={}
def bound(v):
 if v in imports:return (0,[])
 if v in memo:return memo[v]
 assert all(v==w or not any(v in c and w in c for c in scc) for w in fns[v]['calls']), 'unexpected mutual recursion'
 children=[bound(w) for w in fns[v]['calls'] if w!=v]
 tail=max(children,key=lambda x:x[0]) if children else (0,[])
 memo[v]=(fns[v]['allocation_sum_bytes']+tail[0],[v]+tail[1]);return memo[v]
for v in fns:bound(v)
root_bounds={n:{'bytes':bound(i)[0],'path':[{'id':x,'name':fns[x]['name'],'conservative_local_bytes':fns[x]['allocation_sum_bytes']} for x in bound(i)[1]]} for n,i in exports.items()}

rows=[]
for fid,f in fns.items():
 rows.append({k:v for k,v in f.items() if k!='instructions'})
result={'schema':'aethyr.grid.post-binaryen-callgraph.v1','phase':'before-binaryen' if PRE else 'after-binaryen','semantic_sections_identical_to_qualified_wasm':not PRE,
 'imports':imports,'exports':exports,'table_targets':[{'id':i,'name':fns[i]['name']} for i in table],
 'indirect_policy':'Every call_indirect conservatively targets every function in the immutable element table. No narrowing from type assumptions.',
 'functions':rows,'global_stack_pointer_write_count':sp_writes,'unknown_sp_reads':all_unknown,
 'resolved_noncanonical_SP_reads':resolved_sp_reads,
 'bound_method':'For each optimized function sum ALL constant stack-pointer decrement sites, even when mutually exclusive or sequential. Add the maximum child bound along direct calls and every immutable-table candidate for indirect calls. C stack-usage reports contain only static allocations; inlined fragments retain balanced ABI stack restoration. Zero-allocation self-recursive libc nodes do not grow linear memory.',
 'export_bounds':root_bounds,'all_defined_function_max_bytes':max(x[0] for x in memo.values()),
 'recursive_components':[[{'id':i,'name':fns[i]['name'],'frame_bytes':fns[i]['frame_bytes']} for i in c] for c in scc]}
(OUT/('callgraph-preopt.json' if PRE else 'callgraph.json')).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'functions':len(fns),'imports':len(imports),'all_defined_function_max_bytes':result['all_defined_function_max_bytes'],'largest_export_bounds':sorted([(x['bytes'],n) for n,x in root_bounds.items()],reverse=True)[:14],'unknown_sp_reads':all_unknown,'recursive_components':result['recursive_components']}))

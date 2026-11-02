from pathlib import Path
import hashlib,json,tarfile,io,shutil,subprocess,datetime,re
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book');TMP=Path('/tmp/aethyr-ch11-grid-stack')
OUT=ROOT/'docs/verification/data/aethyr-ch11-2026-11-01/grid-memory-stack'
OUT.mkdir(parents=True,exist_ok=True)
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for name in ['qualification.json','callgraph.json','callgraph-preopt.json','stack-write-shapes.json','capacity-growth.json','driver-commands.txt','names-commands.json','analyze.py','graph.py','capacity.py','retain.py']:
 dest=OUT/('c-only-compilation.json' if name=='qualification.json' else name);shutil.copyfile(TMP/name,dest)
for name,src in [('memory.json','/tmp/aethyr-ch11-grid-memory.json'),('memory-owners.json','/tmp/aethyr-ch11-grid-memory-owners.json')]:shutil.copyfile(src,OUT/name)
for p in (TMP/'objects').glob('*.su'):
 d=OUT/'stack-usage'/p.name;d.parent.mkdir(exist_ok=True);shutil.copyfile(p,d)
inputs={}
for p in Path('/tmp/aethyr-ch11-grid-qualified/admitted').glob('*'):
 if p.suffix in ['.c','.h']:inputs['admitted/'+p.name]=p
recipe=json.load(open(TMP/'qualification.json'))
for name in recipe['source_sha256']:
 p=Path(name)
 if p.is_relative_to(ROOT):inputs['source/'+str(p.relative_to(ROOT))]=p
for p in ['apps/openagents-web/analytics-face.h','apps/openagents-web/edit-face.h','apps/openagents-web/newland-reader-bridge.h','coax/web/project/build-home.sh','coax/web/toolchain.sh','coax/web/host.js']:
 inputs['source/'+p]=ROOT/p
for p in ['grid.map','grid-names.map','grid.disasm','grid-names.disasm','grid-preopt.disasm','grid-names.wasm','grid-unoptimized-names.wasm','grid-map.wasm']:
 inputs['diagnostics/'+p]=TMP/p
inputs['qualified/newland.wasm']=Path('/tmp/aethyr-ch11-grid-qualified/newland.wasm')
inputs['qualified/build.json']=Path('/tmp/aethyr-ch11-grid-qualified/qualification.json')
inventory={n:sha(p) for n,p in inputs.items()}
archive=OUT/'inputs-and-diagnostics.tar.gz'
with tarfile.open(archive,'w:gz') as t:
 for n,p in sorted(inputs.items()):
  data=p.read_bytes();info=tarfile.TarInfo(n);info.size=len(data);info.mtime=0;t.addfile(info,io.BytesIO(data))
for n,p in inputs.items():assert sha(p)==inventory[n],n
(OUT/'archive-index.json').write_text(json.dumps({'files':inventory,'archive_sha256':sha(archive)},indent=2)+'\n')
g=json.load(open(TMP/'callgraph.json'));cap=json.load(open(TMP/'capacity-growth.json'));mem=json.load(open('/tmp/aethyr-ch11-grid-memory.json'));writes=json.load(open(TMP/'stack-write-shapes.json'))
assert not g['unknown_sp_reads'] and not writes['unknown_shapes']
assert all(x['ends_with_unreachable'] for x in writes['allocation_shapes_without_any_restore'])
assert all(sum(x['frame_bytes'] for x in c)==0 for c in g['recursive_components'])
su=[]
for p in (TMP/'objects').glob('*.su'):
 for line in p.read_text().splitlines():
  a=line.split('\t');su.append({'function':a[0].rsplit(':',1)[-1],'bytes':int(a[1]),'kind':a[2]})
assert all(x['kind']=='static' for x in su)
record={'schema':'aethyr.grid.memory-stack-static-audit.v1','status':'measured-current-artifact-and-conservative-stack-bound; no build setting changed',
 'observed_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'qualified_wasm_sha256':mem['wasm_sha256'],'qualified_compiler_sha256':'1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa',
 'scope':'Exact retained default Grid artifact, admitted Coax C/headers and original platform C. Only Clang stack-usage/map/debug-name diagnostic emission; no Coax checks or GPU rendering repeated. Original semantic Wasm sections are byte-identical to names-retained optimized diagnostic.',
 'memory':{'minimum_pages':mem['declared']['min'],'maximum_pages':mem['declared']['max'],'heap_base':mem['heapBase'],'state_bytes':mem['stateSize'],'vertex_bytes':mem['arena']['vertexBytes'],'startup_arena_need':mem['arena']['need'],'excess_to_64MiB':mem['arena']['excess']},
 'stack':{'reserved_bytes':8388608,'c_functions_with_static_usage':len(su),'c_dynamic_allocations':0,'largest_C_frame':max(su,key=lambda x:x['bytes']),
 'pre_binaryen_maximum_callchain_bytes':json.load(open(TMP/'callgraph-preopt.json'))['all_defined_function_max_bytes'],
 'pre_binaryen_note':'The two noncanonical SP reads are a128B static leaf red-zone (Coax gc_edge_sweep, confirmed by .su) and zero-allocation libc TLS metadata. They do not determine the maximum2,102,784B path. Post-Binaryen analysis below has no unknown SP reads.',
 'optimized_defined_functions':len(g['functions']),'exported_roots':len(g['exports']),'reader_export_roots':sum(n.startswith('nl_') for n in g['exports']),
 'optimized_conservative_maximum_bytes':g['all_defined_function_max_bytes'],'largest_export':'coaxial_frame','bound_method':g['bound_method'],
 'allocation_writes':writes['allocation_writes'],'restore_writes':writes['restore_writes'],'unknown_SP_write_shapes':0,
 'noreturn_unrestored_frames':writes['allocation_shapes_without_any_restore'],
 'recursion':{'components':g['recursive_components'],'fflush':'Recursion occurs only in the null-FILE branch; each recursive call is guarded by a nonzero loaded FILE pointer and enters the nonrecursive branch. Maximum two active instances, zero linear-stack allocation.',
 'frexp':'The nonzero subnormal branch multiplies by2^64 before its one recursive call. Even the least binary64 subnormal becomes normal, so recursion depth is at most two. Both instances have zero linear-stack allocation.'},
 'assumptions':'Valid Wasm/C ABI execution and memory integrity; the observed single-thread browser imports do not synchronously re-enter this Wasm module. The implicit VM operand/native call stack is separate from the linear-memory linker reservation.'},
 'capacity_512_only':cap,
 'recommendation':{'grid_only_stack_bytes':4194304,'reason':'Exceeds both pre- and post-Binaryen conservative bounds. Even the larger pre-Binaryen bound plus all derived512-row local growth leaves2,059,264bytes slack. Preserves400k output vertices and all package/data capacities. This audit does not change a build flag or declare the final upstream-integrated module qualified.',
 'followup':'After an authorized Grid-only link setting change, relink the already admitted C/headers, independently remeasure minpages/heap/arena, run actual startup/render, and retain artifact/source identity. Later upstream code/data changes need their own final artifact measurement.'},
 'tools':{str(p):sha(p) for p in [Path('/opt/homebrew/opt/llvm/bin/clang'),Path('/opt/homebrew/bin/wasm-ld'),Path('/opt/homebrew/bin/wasm-opt'),Path('/opt/homebrew/opt/llvm/bin/llvm-objdump'),Path('/opt/homebrew/Cellar/llvm/23.1.4/etc/clang/wasm32-unknown-wasip1.cfg')]},
 'archive':{'path':str(archive.relative_to(ROOT)),'sha256':sha(archive),'bytes':archive.stat().st_size,'file_index':'archive-index.json'},
 'files':{str(p.relative_to(OUT)):{'sha256':sha(p),'bytes':p.stat().st_size} for p in sorted(OUT.rglob('*')) if p.is_file() and p.name not in ['qualification.json']}}
(OUT/'qualification.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'out':str(OUT.relative_to(ROOT)),'archive_bytes':archive.stat().st_size,'optimized_stack_bound':g['all_defined_function_max_bytes'],'recommended_stack':4194304,'static_512_growth':cap['static_growth']['total']}))

#!/usr/bin/env python3
"""Full pinned Unicode NFKC corpus against actual emitted Coax implementations.

Expected normalized text comes only from the official corpus. Python supplies
UTF-8 encoding and I/O, never its installed normalization implementation.
"""
if not __debug__:raise RuntimeError('tests require Python assertions')
import hashlib,json,os,platform,re,struct,subprocess,sys,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT/'tools/coax-unicode'));import tables
COMPILER=ROOT/'coax/bin/coax';SOURCE=ROOT/'coax/lib/text/nfkc.coax'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(*args):
 p=subprocess.run(list(map(str,args)),capture_output=True,text=True)
 if p.returncode:raise RuntimeError(f'{args[0]} exited {p.returncode}\n{p.stdout}\n{p.stderr}')
 return p.stdout
def corpus(source):
 rows=[];part1=set();part='';official=0;phrase=None
 for line in source['NormalizationTest.txt'].splitlines():
  line=line.split('#',1)[0].strip()
  if not line:continue
  if line.startswith('@'):part=line;continue
  columns=[tuple(int(cp,16)for cp in field.split())for field in line.split(';')[:5]]
  if len(columns)!=5:raise ValueError('normalization corpus row')
  encoded=[bytes(''.join(chr(cp)for cp in field),'utf-8')for field in columns];expected=encoded[3]
  for text in encoded:rows.append((text,expected))
  if columns[0]==(0xfdfa,):phrase=expected
  if part=='@Part1':
   if len(columns[0])!=1:raise ValueError('Part1 identity inventory')
   part1.add(columns[0][0])
  official+=1
 identities=0
 for cp in sorted(tables.model(source)['assigned']-part1):
  if 0xd800<=cp<=0xdfff:continue
  text=chr(cp).encode();rows.append((text,text));identities+=1
 if phrase is None:raise ValueError('missing official compatibility-expansion oracle')
 # Independently composed boundary cases: ASCII, the official Arabic phrase
 # repeated between class-zero boundaries, and a long reverse-class sequence.
 rows.extend([(b'A'*1024,b'A'*1024),('ﷺ'.encode()*341+b'A',phrase*341+b'A'),(('A'+'̕'*256+'̀'*255).encode(),('À'+'̀'*254+'̕'*256).encode())])
 if any(len(a)>1024 or len(b)>73728 for a,b in rows):raise ValueError('official case exceeds declared byte profile')
 return rows,{'official_rows':official,'nfkc_column_invariants':official*5,'omitted_assigned_scalar_identities':identities,'additional_boundary_sequences':3}
def main():
 start=time.monotonic();pinned=tables.read_pinned();generated,stats=tables.generate(pinned)
 if tables.TARGET.read_text()!=generated:raise ValueError('Unicode table drift')
 rows,counts=corpus(pinned);report={'schema':'coax.unicode-nfkc-conformance.v1','unicode_version':'18.0.0','annex_revision':58,'compiler':run(COMPILER,'version').strip(),'compiler_sha256':sha(COMPILER),'host':platform.platform(),'native_compiler':run('cc','--version').splitlines()[0],'native_target':run('cc','-dumpmachine').strip(),'wasm_compiler':run('/opt/homebrew/opt/llvm/bin/clang','--version').splitlines()[0],'ios_simulator_sdk':run('xcrun','--sdk','iphonesimulator','--show-sdk-version').strip(),'source_manifest_sha256':sha(tables.CORPUS/'SOURCES.json'),'scope':'bounded correctness and conformance; not secret erasure, constant time, physical iOS or production approval',**counts,'tables':stats,'native':[],'wasm':[]}
 deps=json.loads(run(COMPILER,'deps',SOURCE));sources={str(Path(f).relative_to(ROOT)):sha(f)for f in deps['sources']}
 for folder in [HERE,ROOT/'tools/coax-unicode']:
  for f in folder.iterdir():
   if f.is_file()and f.suffix in ['.c','.cjs','.py']:sources[str(f.relative_to(ROOT))]=sha(f)
 with tempfile.TemporaryDirectory(prefix='coax-unicode-')as tmp:
  d=Path(tmp);casefile=d/'cases.bin'
  with casefile.open('wb')as f:
   f.write(struct.pack('<II',0x314b464e,len(rows)))
   for a,b in rows:f.write(struct.pack('<II',len(a),len(b)));f.write(a);f.write(b)
  report['casefile_sha256']=sha(casefile);report['casefile_bytes']=casefile.stat().st_size
  for mode,flags in [('checked',[]),('release',['--release'])]:
   cfile=d/(mode+'.c');mapping=d/(mode+'.map.json');cfile.write_text(run(COMPILER,'c',SOURCE,'--library','--header',d/'nfkc.h','--source-map',mapping,*flags))
   meta=json.loads(mapping.read_text())
   for name,want in meta['source_hashes'].items():
    if sha(name)!=want or sources[str(Path(name).relative_to(ROOT))]!=want:raise ValueError('generated source identity')
   exe=d/(mode+'.test');run('cc','-std=c11','-O2','-I'+str(d),cfile,HERE/'native.c','-lm','-o',exe)
   begun=time.monotonic();out=run(exe,casefile).strip();report['native'].append({'profile':mode,'result':out,'elapsed_seconds':time.monotonic()-begun,'generated_c_sha256':sha(cfile),'source_map_sha256':sha(mapping),'artifact_sha256':sha(exe),'boundary_timings_ns':[{'case':int(i),'nanoseconds':int(ns)}for i,ns in re.findall(r'boundary_timing (\d+) (\d+)',out)]})
   stack=d/(mode+'.stack.o');run('cc','-std=c11','-O2','-fstack-usage','-c',cfile,'-o',stack);usage=stack.with_suffix('.su').read_text();report['native'][-1]['compiler_stack_usage']=usage;report['native'][-1]['largest_reported_single_function_frame_bytes']=max(map(int,re.findall(r'\t(\d+)\t',usage)))
   print(mode,'native full corpus passed',flush=True)
   sanitized=d/(mode+'.asan');run('cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),cfile,HERE/'native.c','-lm','-o',sanitized);run(sanitized)
   module=d/(mode+'.wasm');clang='/opt/homebrew/opt/llvm/bin/clang';sysroot='/opt/homebrew/opt/wasi-libc/share/wasi-sysroot';resources='/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'
   run(clang,'--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources,'-std=c11','-O2','-I'+str(d),cfile,HERE/'wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',module)
   begun=time.monotonic();result=json.loads(run('node',HERE/'wasm.cjs',module,casefile));report['wasm'].append({'profile':mode,**result,'elapsed_seconds':time.monotonic()-begun,'artifact_sha256':sha(module),'stack_bytes':1048576});print(mode,'Wasm full corpus passed',flush=True)
   sdk=run('xcrun','--sdk','iphonesimulator','--show-sdk-path').strip();obj=d/(mode+'.ios.o');run('xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',cfile,'-o',obj);report['native'][-1].update(sanitizers='ASan+UBSan actual boundary calls passed',ios='arm64 Simulator compilation only',ios_object_sha256=sha(obj))
 for name,want in sources.items():
  if sha(ROOT/name)!=want:raise ValueError('input changed: '+name)
 if sha(COMPILER)!=report['compiler_sha256']:raise ValueError('compiler changed')
 tables.read_pinned()
 if sha(tables.CORPUS/'SOURCES.json')!=report['source_manifest_sha256']:raise ValueError('source pin manifest changed')
 report['sources']=sources;report['elapsed_seconds']=time.monotonic()-start
 text=json.dumps(report,indent=2,sort_keys=True)+'\n'
 if os.environ.get('COAX_UNICODE_REPORT'):Path(os.environ['COAX_UNICODE_REPORT']).write_text(text)
 print(text,end='')
if __name__=='__main__':main()

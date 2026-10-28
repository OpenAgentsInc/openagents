#!/usr/bin/env python3
"""R0: actual Cast.bin kernels, independent pinned reference and raw timings.
Maintenance/measurement tooling only, not a compiler or application dependency.
"""
from pathlib import Path
import argparse, hashlib, json, os, platform, re, subprocess, time
ROOT=Path(__file__).resolve().parents[2]
BASE=Path(__file__).resolve().parent

def checked(args,**kw):
 p=subprocess.run(list(map(str,args)),capture_output=True,**kw)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 return p.stdout

def compiler_observation(args):
 # macOS time reports child peak RSS in bytes. It observes the compiler only,
 # not a cumulative resource.getrusage maximum from earlier subprocesses.
 p=subprocess.run(['/usr/bin/time','-l',*map(str,args)],capture_output=True)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 peak=re.search(rb'(\d+)\s+maximum resident set size',p.stderr)
 if not peak:raise RuntimeError('missing native compiler RSS observation')
 return p.stdout,int(peak[1])

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True)
 parser.add_argument('--repeats',type=int,default=9);parser.add_argument('--sanitize',action='store_true')
 parser.add_argument('--enforce-budget',action='store_true');args=parser.parse_args()
 if not 1<=args.repeats<=100:parser.error('repeats must be 1..100')
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
 contract=json.loads((BASE/'contract.json').read_text())
 asset=ROOT/contract['asset']
 assert hashlib.sha256(asset.read_bytes()).hexdigest()==contract['asset_sha256'],'corpus changed: review its baseline'
 for name,digest in contract['files'].items():
  assert hashlib.sha256((BASE/'reference'/name).read_bytes()).hexdigest()==digest,'native reference changed: '+name
 assert hashlib.sha256((BASE/'reader-f32.inc').read_bytes()).hexdigest()==contract['reader_adapter_sha256'],'reference adapter changed'
 source=(BASE/'reference/anim/anim.cpp').read_text()
 start=source.index('Rigid Data::skin(');end=source.index('\nnamespace {',start)
 source=source[:start]+(BASE/'reader-f32.inc').read_text()+source[end:]
 (out/'reference.cpp').write_text(source)
 compiler=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax')).resolve()
 cc=os.environ.get('CC','cc');cxx=os.environ.get('CXX','c++')
 report={'schema_version':1,'source_commit':checked(['git','-C',ROOT,'rev-parse','HEAD']).decode().strip(),
  'working_tree_dirty':bool(checked(['git','-C',ROOT,'status','--porcelain']).strip()),
  'compiler':checked([compiler,'version']).decode().strip(),'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),'system':platform.platform(),
  'machine':checked(['sysctl','-n','machdep.cpu.brand_string']).decode().strip(),
  'c_toolchain':checked([cc,'--version']).decode().splitlines()[0],
  'cache_scope':'fresh outputs and repeated single-shot invocations; OS caches uncontrolled, no incremental compiler','contract':contract,'sanitized':args.sanitize,'repeats':args.repeats,'modes':{}}
 report['sources_sha256']={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [ROOT/'verse/cast.coax',ROOT/'verse/player.coax',ROOT/'verse/benchmarks.coax',BASE/'native.cpp',BASE/'reader-f32.inc',ROOT/'coax/src/main.cpp',ROOT/'coax/src/emit_c.cpp',ROOT/'coax/src/coax.h']}
 for mode in ['debug','app','release']:
  work=out/mode;work.mkdir(exist_ok=True);flags=['-O0' if mode=='debug' else '-O2','-g','-ffp-contract=off']
  if args.sanitize:flags+=['-fsanitize=address,undefined','-fno-omit-frame-pointer']
  timings={};objects=[]
  for name in ['Cast','Player','Benchmarks']:
   before=time.monotonic();cmd=[compiler,'c',ROOT/'verse'/f'{name.lower()}.coax','--library','--header',work/f'{name}.h','--debug-info','--source-map',work/f'{name}.map.json','--timings',work/f'{name}.compile.json']
   if mode=='release':cmd+=['--release']
   generated,peak=compiler_observation(cmd);(work/f'{name}.c').write_bytes(generated)
   timings[name]={'coax_wall_seconds':time.monotonic()-before,'compiler_peak_rss_bytes':peak,'compiler_phases':json.loads((work/f'{name}.compile.json').read_text()),'repeated_codegen':[]}
   for _ in range(2):
    before=time.monotonic();again,peak=compiler_observation(cmd);assert again==generated
    timings[name]['repeated_codegen'].append({'wall_seconds':time.monotonic()-before,'compiler_peak_rss_bytes':peak,'compiler_phases':json.loads((work/f'{name}.compile.json').read_text())})
   before=time.monotonic();obj=work/f'{name}.o'
   checked([cc,'-std=c11',*flags,'-Dmalloc=r0_malloc','-Dcalloc=r0_calloc','-Drealloc=r0_realloc','-Dfree=r0_free','-c',work/f'{name}.c','-o',obj]);timings[name]['c_compile_seconds']=time.monotonic()-before;objects.append(obj)
   before=time.monotonic();checked([cc,'-std=c11',*flags,'-Dmalloc=r0_malloc','-Dcalloc=r0_calloc','-Drealloc=r0_realloc','-Dfree=r0_free','-c',work/f'{name}.c','-o',obj]);timings[name]['repeated_c_compile_seconds']=time.monotonic()-before
  before=time.monotonic();reference_obj=work/'reference.o'
  checked([cxx,'-std=c++20',*flags,'-I',BASE/'reference/anim','-c',out/'reference.cpp','-o',reference_obj]);timings['reference_compile_seconds']=time.monotonic()-before
  before=time.monotonic();harness_obj=work/'harness.o'
  checked([cxx,'-std=c++20',*flags,'-I',work,'-I',BASE,'-c',BASE/'native.cpp','-o',harness_obj]);timings['harness_compile_seconds']=time.monotonic()-before
  before=time.monotonic();exe=work/'bench'
  checked([cxx,*flags,*objects,reference_obj,harness_obj,'-o',exe]);timings['link_seconds']=time.monotonic()-before
  result=json.loads(checked([exe,asset,str(args.repeats)]))
  # Recompute quantiles independently, not against the previous Coax output.
  for metric in result['metrics'].values():
   samples=sorted(metric['samples_ns']);n=len(samples)
   median=samples[n//2] if n%2 else (samples[n//2-1]+samples[n//2])/2
   assert metric['median_ns']==median
   assert metric['p95_ns']==samples[(n*95+99)//100-1]
  result['build']=timings;result['coax_arithmetic']='wrapping runtime integers' if mode=='release' else 'checked runtime integers';result['native_optimization']=flags;report['modes'][mode]=result
  (out/f'{mode}.json').write_text(json.dumps(result,indent=2)+'\n')
 (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
 for mode,r in report['modes'].items():
  print(f"{mode}: pose {r['metrics']['coax_pose']['median_ns']/1000:.3f} us; native {r['metrics']['native_pose']['median_ns']/1000:.3f} us; ratio {r['coax_native_median_ratio']:.3f}; max error {r['max_palette_error']:.8g}; Coax allocations {r['metrics']['coax_pose']['allocations']}")
 print(out/'report.json')
 if args.enforce_budget and any(not r['proposal_5_percent_met'] for r in report['modes'].values()):raise SystemExit(1)
if __name__=='__main__':main()

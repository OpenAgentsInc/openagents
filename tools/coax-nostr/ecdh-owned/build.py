#!/usr/bin/env python3
"""Build one experimental owner-only ECDH library, refusing exposed internals."""
import argparse,ctypes,hashlib,json,os,pathlib,platform,shutil,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3];COMPILER=ROOT/'coax/bin/coax';P=ROOT/'coax/lib/crypto/platform'
API=['coax_ecdh_owned_'+name for name in ('acquire','start','collect','retry','cancel','close')]
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def run(*args):
 result=subprocess.run([str(a) for a in args],capture_output=True,text=True,env={'PATH':'/usr/bin:/bin:/opt/homebrew/bin','LC_ALL':'C'})
 if result.returncode:raise RuntimeError('build tool failed: '+str(args[0])+': '+result.stderr[-3000:])
 return result.stdout
def publish_exclusive(source,destination):
 # Darwin renamex_np with RENAME_EXCL atomically refuses an existing target.
 # Both directories are on the destination's filesystem.
 libc=ctypes.CDLL(None,use_errno=True);rename=libc.renamex_np
 rename.argtypes=[ctypes.c_char_p,ctypes.c_char_p,ctypes.c_uint];rename.restype=ctypes.c_int
 if rename(os.fsencode(source),os.fsencode(destination),4)!=0:
  error=ctypes.get_errno();raise OSError(error,os.strerror(error),str(destination))

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--target',choices=['native','wasm'],required=True);parser.add_argument('--release',action='store_true');parser.add_argument('--out',type=pathlib.Path,required=True);args=parser.parse_args()
 out=args.out.absolute()
 if out.exists() or not out.parent.is_dir():raise ValueError('output must be a new directory in an existing parent')
 if platform.system()!='Darwin':raise ValueError('exclusive publication and export verification are currently supported on macOS only')
 run(ROOT/'coax/provenance.sh','verify',ROOT,COMPILER,ROOT/'coax/bin/provenance-check',ROOT/'coax/bin/provenance.record')
 headers=[P/'ecdh_owned.h'];inputs=[COMPILER,ROOT/'coax/provenance.sh',P/'ecdh_owned.c',P/'zeroize.c',*headers,pathlib.Path(__file__).resolve()];hashes={str(p):sha(p) for p in inputs}
 with tempfile.TemporaryDirectory(prefix='.coax-ecdh-build-',dir=out.parent) as td:
  tmp=pathlib.Path(td);work=tmp/'work';work.mkdir();artifact=tmp/'artifact';artifact.mkdir();sources=[]
  for name,src in [('ecdh','ecdh_owned_adapter.coax'),('zeroize-policy','zeroize.coax'),('ecdh_lifecycle','ecdh_lifecycle.coax')]:
   c=work/(name+'.c');m=work/(name+'.map.json');options=['--release'] if args.release else []
   run(COMPILER,'c',ROOT/'coax/lib/crypto'/src,'--library','--header',work/(name+'.h'),'--source-map',m,'-o',c,*options);sources.append(c)
   for path,digest in json.loads(m.read_text())['source_hashes'].items():
    if sha(path)!=digest or path in hashes and hashes[path]!=digest:raise ValueError('source-map mismatch')
    hashes[path]=digest
  sources += [P/'zeroize.c',P/'ecdh_owned.c'];common=['-std=c11','-O2','-I'+str(work),'-I'+str(P)]
  if args.target=='native':
   compiler='/usr/bin/cc';output=artifact/'libcoax_ecdh_owned.dylib';run(compiler,*common,'-shared','-fPIC','-fvisibility=hidden','-Wl,-install_name,@rpath/libcoax_ecdh_owned.dylib',*sources,'-lm','-o',output)
   symbols={line.split()[-1].removeprefix('_') for line in run('/usr/bin/nm','-gU',output).splitlines() if line.split()}
   if symbols!=set(API):raise ValueError('unexpected native exports: '+repr(sorted(symbols)))
   install=run('/usr/bin/otool','-D',output).splitlines()
   if len(install)!=2 or install[1].strip()!='@rpath/libcoax_ecdh_owned.dylib':raise ValueError('unexpected native install name')
  else:
   compiler='/opt/homebrew/opt/llvm/bin/clang';output=artifact/'coax_ecdh_owned.wasm';node='/opt/homebrew/bin/node'
   run(compiler,'--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',*common,*sources,'-mexec-model=reactor','-Wl,--export-memory,--export=malloc,--export=free,-z,stack-size=1048576',*[f'-Wl,--export={name}' for name in API],'-lm','-o',output)
   js='const fs=require("fs");let m=new WebAssembly.Module(fs.readFileSync(process.argv[1]));console.log(JSON.stringify(WebAssembly.Module.exports(m).map(x=>x.name)))'
   symbols=set(json.loads(run(node,'-e',js,output)))
   if symbols!=set(API)|{'memory','_initialize','malloc','free'}:raise ValueError('unexpected Wasm exports: '+repr(sorted(symbols)))
   hashes[node]=sha(node);hashes['/opt/homebrew/bin/wasm-ld']=sha('/opt/homebrew/bin/wasm-ld')
  hashes[compiler]=sha(compiler);shutil.copyfile(headers[0],artifact/'ecdh_owned.h')
  if any(sha(path)!=digest for path,digest in hashes.items()):raise ValueError('source or tool changed during build')
  report={'schema':'coax.ecdh-owned-public-build.v1','scope':'experimental owner export boundary; stable tool/files, truthful caller ownership and synchronous wipe are premises; no target or production approval','target':args.target,'profile':'release' if args.release else 'checked','compiler_version':run(COMPILER,'version').strip(),'inputs':hashes,'exports':sorted(symbols),'artifact_sha256':sha(output),'public_header_sha256':sha(artifact/'ecdh_owned.h')}
  (artifact/'manifest.json').write_text(json.dumps(report,indent=2)+'\n')
  publish_exclusive(artifact,out)
 print(out)
if __name__=='__main__':main()

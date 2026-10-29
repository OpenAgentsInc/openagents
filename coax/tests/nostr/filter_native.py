#!/usr/bin/env python3
"""Native/Wasm event-filter matrix compared to an independent Boolean oracle."""
import ast,ctypes as C,json,os,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
source=ROOT/'coax/lib/nostr/filter.coax';fixture=ROOT/'coax/tests/nostr/filter_fixture.c'
line=(ROOT/'coax/tests/nostr/event_fixtures.coax').read_text().splitlines()[2]
text=ast.literal_eval(line.split('::',1)[1].strip()[:-1])
def expected(m):
 return (not m&1 or (not m&2048 and bool(m&2))) and (not m&4 or bool(m&8)) and (not m&16 or bool(m&32)) and (not m&64 or bool(m&128)) and not m&256 and not m&512
with tempfile.TemporaryDirectory(prefix='coax-filter-') as t:
 d=Path(t);(d/'event.h').write_text('#define FILTER_EVENT '+json.dumps(text,ensure_ascii=True)+'\n')
 for mode in ('checked','release'):
  gen=d/'generated.c';cmd=[str(COMPILER),'c',str(source),'--library','-o',str(gen)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True)
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),'-include',str(d/'event.h'),str(fixture),'-lm','-o',str(lib)],check=True)
  api=C.CDLL(str(lib));f=api.filter_case;f.argtypes=[C.c_int];f.restype=C.c_int
  api.filter_two_tags.restype=C.c_int;assert api.filter_two_tags()==0
  write=api.filter_serialize;write.argtypes=[C.POINTER(C.c_uint8),C.c_int64];write.restype=C.c_int
  buf=(C.c_uint8*512)();n=write(buf,512);assert n>0
  assert json.loads(bytes(buf[:n]))=={'ids':['53d4f42ea78a74ab5ab47515040729f9ae9348f4ed05b59d07c7d24fb90116dc'],'kinds':[1,42],'#x':['a"b\né'],'since':-1,'limit':0}
  assert write(buf,1)==-1
  for m in range(4096):
   got=f(m);assert got==int(expected(m)),(mode,m,got,expected(m))
  print(mode,'4096 independently modeled event-filter combinations passed',flush=True)
  sanitized=d/(mode+'-sanitized');subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-DFILTER_STANDALONE','-I'+str(d),'-include',str(d/'event.h'),str(fixture),'-lm','-o',str(sanitized)],check=True)
  subprocess.run([str(sanitized)],check=True);print(mode,'ASan/UBSan matrix passed',flush=True)
  wasm=d/(mode+'.wasm');subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFILTER_WASM','-I'+str(d),'-include',str(d/'event.h'),str(fixture),'-mexec-model=reactor','-Wl,-z,stack-size=2097152,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/filter_wasm.cjs'),str(wasm)],check=True)

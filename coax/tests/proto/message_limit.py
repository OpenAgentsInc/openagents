#!/usr/bin/env python3
"""Protoc-derived message caps; test native/Wasm behavior at the shared cap."""
import ctypes as C
import json, os, pathlib, re, subprocess, sys, tempfile
if not __debug__: raise SystemExit('tests require Python assertions')
R=pathlib.Path(__file__).resolve().parents[3]
COAX=pathlib.Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
class View(C.Structure):
    _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
def run(*args):
    p=subprocess.run([str(x) for x in args],capture_output=True)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    return p.stdout
def header(n):
    if not n:return []
    value=n-4;out=[10]
    while value>=128:out.append((value&127)|128);value>>=7
    out.append(value)
    assert len(out)==4
    return out
if any(a not in ('--oracle','--wasm','--sanitize') for a in sys.argv[1:]):raise SystemExit('usage: message_limit.py [--oracle] [--wasm] [--sanitize]')
with tempfile.TemporaryDirectory(prefix='coax-message-limit-') as t:
    d=pathlib.Path(t);pb=R/'coax/tests/proto/message_limit.pb'
    if '--oracle' in sys.argv:
        descriptor=d/'message_limit.pb';run(os.environ.get('PROTOC','protoc'),'-I'+str(R/'coax/tests/proto'),'-I'+str(R/'proto'),'--include_imports','--descriptor_set_out='+str(descriptor),'message_limit.proto')
        assert descriptor.read_bytes()==pb.read_bytes()
    generated=run(COAX,'proto',pb,'--prefix','T').decode()
    for name in ('Absent','Zero','Below','Equal','Above'):
        block=re.search(r'T_'+name+r'__check :: .*?return T__pb_Check\.\{left=left\};\}',generated,re.S)
        assert block,name
        text=block.group()
        admission='if !T__pb_message_admitted(depth,wire.count) {return T__pb_Check.{error=T__pb_LIMIT};}'
        assert text.count(admission)==1,name
        guards=re.findall(r'if wire\.count>(\d+) \{return T__pb_Check\.\{error=T__pb_LIMIT\};\}',text)
        assert guards==(['1048575'] if name=='Below' else []),(name,guards)
        if guards:assert text.index(admission)<text.index('if wire.count>'),name
    source=d/'limit.coax';source.write_text('#import_proto "'+str(pb)+'" as T;\n'+''.join('check_'+str(i)+' :: (v:[]u8)->u32 #export_c {return T_'+name+'_decode(v).error;}\n' for i,name in enumerate(('Absent','Zero','Below','Equal','Above'))))
    cases=[]
    for i,name in enumerate(('Absent','Zero','Below','Equal','Above')):
        for n in (0,1048575,1048576,1048577):
            error=2 if n>1048576 or (name=='Below' and n>1048575) else 0
            cases.append({'name':name+' '+str(n),'which':i,'n':n,'header':header(n),'error':error})
    (d/'cases.json').write_text(json.dumps(cases))
    if '--sanitize' in sys.argv:
        harness=['#include <stdint.h>',' #include "test.h"','static uint8_t data[1048577];','int main(void){']
        for c in cases:
            writes=''.join('data['+str(i)+']='+str(x)+';' for i,x in enumerate(c['header']))
            harness.append(writes+'if(check_'+str(c['which'])+'((coax_v_u8){data,'+str(c['n'])+'})!='+str(c['error'])+')return 1;')
        harness.append('return 0;}')
        (d/'sanitizer.c').write_text('\n'.join(harness)+'\n')
    if '--wasm' in sys.argv:
        (d/'wasm.c').write_text('#include "test.h"\nstatic uint8_t data[1048577];\nuint8_t *limit_input(void){return data;}\nuint32_t limit_eval(int64_t n,uint32_t w){coax_v_u8 v={data,n};switch(w){case 0:return check_0(v);case 1:return check_1(v);case 2:return check_2(v);case 3:return check_3(v);default:return check_4(v);}}\n')
    for mode,flags in [('checked',[]),('release',['--release'])]:
        out=d/(mode+'.c');out.write_bytes(run(COAX,'c',source,'--library','--header',d/'test.h',*flags))
        libfile=d/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',out,'-o',libfile)
        lib=C.CDLL(str(libfile));functions=[getattr(lib,'check_'+str(i)) for i in range(5)]
        for fn in functions:fn.argtypes=[View];fn.restype=C.c_uint32
        for c in cases:
            arr=(C.c_uint8*max(c['n'],1))();arr[:len(c['header'])]=c['header']
            assert functions[c['which']](View(arr,c['n']))==c['error'],c['name']
        print(mode,len(cases),'message-cap cases passed')
        if '--sanitize' in sys.argv:
            exe=d/(mode+'-sanitizer');run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-I'+str(d),out,d/'sanitizer.c','-o',exe);run(exe)
            print(mode,len(cases),'sanitized message-cap cases passed')
        if '--wasm' in sys.argv:
            wasm=d/(mode+'.wasm')
            run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor','-I'+str(d),out,d/'wasm.c','-Wl,-z,stack-size=2097152,--export=limit_input,--export=limit_eval','-o',wasm)
            run('node',R/'coax/tests/proto/message_limit_wasm.cjs',wasm,d/'cases.json')
            print(mode,len(cases),'Wasm message-cap cases passed')

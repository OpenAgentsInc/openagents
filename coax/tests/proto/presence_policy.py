#!/usr/bin/env python3
"""Regression cases for descriptor policies and duplicate/error ordering."""
import ctypes as C
import json, os, pathlib, subprocess, sys, tempfile
if not __debug__: raise SystemExit('tests require Python assertions')
R=pathlib.Path(__file__).resolve().parents[3]
COAX=pathlib.Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
class View(C.Structure):
    _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
def run(*args):
    p=subprocess.run([str(x) for x in args],capture_output=True)
    if p.returncode: raise RuntimeError(p.stderr.decode(errors='replace'))
    return p.stdout
invalid=subprocess.run([str(COAX),'proto',str(R/'coax/tests/proto/presence_policy_invalid.pb'),'--prefix','T'],capture_output=True)
assert invalid.returncode!=0 and b'invalid policy option for field kind' in invalid.stderr
collision=subprocess.run([str(COAX),'proto',str(R/'coax/tests/proto/presence_policy_collision.pb'),'--prefix','T'],capture_output=True)
assert collision.returncode!=0 and b'generated name collision' in collision.stderr
cases=[
 ('valid',b'\x0a\x01z\x10\x01\x1a\x01x',0,0),
 ('missing required',b'\x10\x01',0,5),
 ('packed empty is not a value',b'\x0a\x01z\x12\x00',0,5),
 ('packed limit',b'\x0a\x01z\x12\x02\x01\x02',0,0),
 ('protoc packed fixture',b'\x0a\x01z\x12\x02\x01\x02\x1a\x01x',0,0),
 ('packed over limit',b'\x0a\x01z\x12\x03\x01\x02\x03',0,2),
 ('unbounded repeated values',b'\x0a\x01z\x10\x01\x32\x01a\x32\x01b',0,0),
 ('duplicate before bad wire',b'\x0a\x01z\x08\x00\x10\x01',0,3),
 ('oneof conflict before bad wire',b'\x0a\x01z\x10\x01\x1a\x01x\x20\x00',0,3),
 ('same oneof member before bad wire',b'\x0a\x01z\x10\x01\x1a\x01x\x18\x00',0,3),
 ('synthetic optional duplicate',b'\x0a\x01z\x10\x01\x2a\x01x\x28\x00',0,3),
 ('required oneof member missing',b'\x0a\x01x',1,5),
 ('required oneof member present',b'\x12\x01y',1,0),
 ('required oneof conflict before bad wire',b'\x12\x01y\x08\x00',1,3),
 ('required oneof duplicate before bad wire',b'\x12\x01y\x10\x00',1,3),
 ('required child missing',b'',2,5),
 ('required child field missing',b'\x0a\x00',2,5),
 ('required child field present',b'\x0a\x03\x0a\x01x',2,0),
]
with tempfile.TemporaryDirectory(prefix='coax-presence-policy-') as t:
    d=pathlib.Path(t)
    if any(arg not in ('--oracle','--wasm') for arg in sys.argv[1:]) or len(sys.argv[1:])!=len(set(sys.argv[1:])):
        raise SystemExit('usage: presence_policy.py [--oracle] [--wasm]')
    if '--oracle' in sys.argv:
        protoc=os.environ.get('PROTOC','protoc')
        for name in ('presence_policy','presence_policy_invalid','presence_policy_collision'):
            descriptor=d/(name+'.pb')
            run(protoc,'-I'+str(R/'coax/tests/proto'),'-I'+str(R/'proto'),'--include_imports','--descriptor_set_out='+str(descriptor),name+'.proto')
            assert descriptor.read_bytes()==(R/'coax/tests/proto'/(name+'.pb')).read_bytes(),name
        encoded=subprocess.run([protoc,'-I'+str(R/'coax/tests/proto'),'-I'+str(R/'proto'),'--encode=coax.test.presence.Cases','presence_policy.proto'],input=b'strict:"z" bounded:1 bounded:2 a:"x"\n',capture_output=True,check=True).stdout
        assert encoded==b'\x0a\x01z\x12\x02\x01\x02\x1a\x01x'
        print('protoc descriptors and encoded case agree')
    source=d/'test.coax'
    source.write_text('#import_proto "'+str(R/'coax/tests/proto/presence_policy.pb')+'" as T;\ncheck_cases :: (wire:[]u8)->u32 #export_c {return T_Cases_decode(wire).error;}\ncheck_choice :: (wire:[]u8)->u32 #export_c {return T_RequiredChoice_decode(wire).error;}\ncheck_nested :: (wire:[]u8)->u32 #export_c {return T_NestedRequired_decode(wire).error;}\ncheck_build :: ()->u32 #export_c {a:[256]u8;b:[256]u8;m:=T_Cases_set_strict(T_Cases_new(),"z",a[..]);if m.error!=0 {return m.error;}m=T_Cases_append_bounded(m,1,b[..]);return T_Cases_decode(m.wire).error;}\n')
    if '--wasm' in sys.argv:
        (d/'cases.json').write_text(json.dumps([{'name':name,'wire':list(data),'which':which,'error':expected} for name,data,which,expected in cases]))
        (d/'wasm.c').write_text('#include "test.h"\nstatic uint8_t data[4096];\nuint8_t *policy_input(void){return data;}\nuint32_t policy_eval(int64_t n,uint32_t which){coax_v_u8 v={data,n};return which==0?check_cases(v):which==1?check_choice(v):check_nested(v);}\nuint32_t policy_build(void){return check_build();}\n')
    for mode,flags in [('checked',[]),('release',['--release'])]:
        out=d/(mode+'.c');out.write_bytes(run(COAX,'c',source,'--library','--header',d/'test.h',*flags))
        libfile=d/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',out,'-o',libfile)
        lib=C.CDLL(str(libfile));functions=[lib.check_cases,lib.check_choice,lib.check_nested]
        for function in functions: function.argtypes=[View];function.restype=C.c_uint32
        for name,data,which,expected in cases:
            arr=(C.c_uint8*len(data)).from_buffer_copy(data)
            got=functions[which](View(arr,len(data)))
            assert got==expected,(mode,name,got,expected)
        lib.check_build.restype=C.c_uint32
        assert lib.check_build()==0,(mode,'partial setters must allow incomplete construction')
        print(mode,len(cases),'descriptor policy cases passed')
        if '--wasm' in sys.argv:
            wasm=d/(mode+'.wasm')
            run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor','-I'+str(d),out,d/'wasm.c','-Wl,-z,stack-size=2097152,--export=policy_input,--export=policy_eval,--export=policy_build','-o',wasm)
            run('node',R/'coax/tests/proto/presence_policy_wasm.cjs',wasm,d/'cases.json')
            print(mode,len(cases),'Wasm descriptor policy cases passed')

#!/usr/bin/env python3
"""Descriptor -> Coax -> C11 -> native wire tests; optional pinned protoc oracle.
Normal compiler tests require only Python's standard library and a C compiler.
PROTOC=/path/to/protoc-29.3 python3 coax/tests/proto_import.py --oracle
also checks descriptor reproducibility and both directions with official protoc.
"""
import ctypes as C
import hashlib
import json
import os
import shutil
from pathlib import Path
import struct
import subprocess as S
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]
COAX=ROOT/'coax/bin/coax'
ORACLE='--oracle' in sys.argv
if ORACLE:sys.argv.remove('--oracle')

def var(n):
    n &= 2**64-1
    out=bytearray()
    while n>=128:out.append((n&127)|128);n>>=7
    return bytes(out)+bytes([n])
def v(n,value):return var(n*8)+var(value)
def b(n,value):return var(n*8+2)+var(len(value))+value

def run(args,**kw):return S.run(list(map(str,args)),capture_output=True,timeout=60,**kw)
def checked(args,**kw):
    r=run(args,**kw)
    if r.returncode:raise AssertionError(r.stderr.decode(errors='replace'))
    return r.stdout

SOURCE='''
#import_proto "NOSTR" as N;
#import_proto "TYPES" as X;
#import_proto "VERSE" as V;
Buffer :: struct { bytes:[] u8; }
inspect_proto :: (input:Buffer,op:u32)->u64 #export_c {
 w:=input.bytes;
 if op==0 {return cast(u64) N_Event_decode(w).error;}
 if op==1 {return cast(u64) N_Event_get_kind(N_Event_decode(w)).value;}
 if op==2 {return N_Event_get_created_at(N_Event_decode(w)).value;}
 if op==3 {return cast(u64) X_Scalars_decode(w).error;}
 if op==23 {return cast(u64) X_Scalars_decode_minimal(w).error;}
 s:=X_Scalars_decode(w);
 if op==4 {return X_Scalars_get_u64(s).value;}
 if op==5 {return cast,trunc(u64) X_Scalars_get_zig64(s).value;}
 if op==6 {return cast(u64) f32_bits(X_Scalars_get_real32(s).value);}
 if op==7 {return f64_bits(X_Scalars_get_real64(s).value);}
 if op==8 {return cast(u64) X_Scalars_case_choice(s).value;}
 if op==9 {if X_Scalars_get_opt(s).present {return 1;}return 0;}
 if op==10 {n:u64=0; iterator:=X_Scalars_iter_numbers(s);while iterator.has_value {n+=1;iterator=X_Scalars_next_numbers(iterator);}return n;}
 if op==11 {if N_Filter_get_kinds(N_Filter_decode(w)).present {return 1;}return 0;}
 if op==12 {return cast(u64) N_NullableString_case_value(N_NullableString_decode(w)).value;}
 if op==13 {return cast(u64) N_Audience_case_form(N_Audience_decode(w)).value;}
 if op==14 {return cast(u64) N_ChannelMutation_decode(w).error;}
 if op==15 {return cast(u64) N_AgentAuthentication_decode(w).error;}
 if op==16 {n:u64=0;iterator:=N_Event_iter_tags(N_Event_decode(w));while iterator.has_value {tag:=N_Tag_iter_values(iterator.value);while tag.has_value {n+=1;tag=N_Tag_next_values(tag);}iterator=N_Event_next_tags(iterator);}return n;}
 if op==18 {return cast(u64) N_Tag_decode(w).error;}
 if op==19 {return cast(u64) V_PeerState_decode(w).error;}
 if op==20 {return cast(u64) V_PresenceSnapshot_decode(w).error;}
 if op==17 {return cast(u64) N_Event_unknown(N_Event_decode(w),0).number;}
 return 999;
}
transform_proto :: (input:Buffer,output:Buffer,op:u32)->s64 #export_c {
 w:=input.bytes;out:=output.bytes;
 if op==0 {m:=N_Event_set_kind(N_Event_decode(w),0,out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==1 {m:=X_Scalars_encode(X_Scalars_decode(w),out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==2 {
 a:[2048] u8;b:[2048] u8;child_bytes:[64] u8;
 m:=X_Scalars_new();
 m=X_Scalars_set_real64(m,f64_from_bits(9221120237041090626),a[..]);
 m=X_Scalars_set_real32(m,f32_from_bits(2147483648),b[..]);
 m=X_Scalars_set_i64(m,-9223372036854775808,a[..]);
 m=X_Scalars_set_u64(m,18446744073709551615,b[..]);
 m=X_Scalars_set_i32(m,-2147483648,a[..]);
 m=X_Scalars_set_fixed64_value(m,18446744073709551615,b[..]);
 m=X_Scalars_set_fixed32_value(m,4294967295,a[..]);
 m=X_Scalars_set_enabled(m,true,b[..]);
 m=X_Scalars_set_text(m,"hello",a[..]);
 blob:=[3] u8.[0,255,1];m=X_Scalars_set_blob(m,blob[..],b[..]);
 m=X_Scalars_set_u32(m,4294967295,a[..]);
 m=X_Scalars_set_state(m,-1,b[..]);
 m=X_Scalars_set_sf32(m,-2147483648,a[..]);
 m=X_Scalars_set_sf64(m,-9223372036854775808,b[..]);
 m=X_Scalars_set_zig32(m,-2147483648,a[..]);
 m=X_Scalars_set_zig64(m,-9223372036854775808,b[..]);
 m=X_Scalars_append_numbers(m,-1,a[..]);m=X_Scalars_append_numbers(m,2,b[..]);
 inner:=X_Inner_set_text(X_Inner_new(),"nested",child_bytes[..]);
 m=X_Scalars_set_child(m,inner,a[..]);m=X_Scalars_append_children(m,inner,b[..]);
 m=X_Scalars_set_rank(m,0,a[..]);m=X_Scalars_set_opt(m,0,b[..]);
 encoded:=X_Scalars_encode(m,out);if encoded.error!=0 {return -cast(s64)encoded.error;}return encoded.wire.count;
 }
 if op==3 {m:=X_Scalars_set_title(X_Scalars_decode(w),"changed",out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==4 {m:=X_Scalars_append_numbers(X_Scalars_decode(w),-2,out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==5 {m:=N_Event_encode(N_Event_decode(w),out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==6 {m:=N_Filter_set_kinds(N_Filter_new(),N_KindList_new(),out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==7 {m:=N_NullableString_set_null(N_NullableString_new(),0,out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 if op==8 {bad:=[1] u8.[255];m:=X_Inner_set_text(X_Inner_decode(w),cast(string)bad[..],out);return -cast(s64)m.error;}
 if op==9 {id:=[1] u8.[0];m:=N_Event_set_id(N_Event_decode(w),id[..],out);return -cast(s64)m.error;}
 if op==10 {m:=X_Scalars_clear_opt(X_Scalars_decode(w),out);if m.error!=0 {return -cast(s64)m.error;}return m.wire.count;}
 return -99;
}
'''
HOST='''#include "wire.h"
uint64_t inspect(const uint8_t*p,int64_t n,uint32_t op){coax_t_Buffer b={0};b.bytes.data=(uint8_t*)p;b.bytes.count=n;return inspect_proto(b,op);}
int64_t transform(const uint8_t*p,int64_t n,uint8_t*out,int64_t cap,uint32_t op){coax_t_Buffer b={0},o={0};b.bytes.data=(uint8_t*)p;b.bytes.count=n;o.bytes.data=out;o.bytes.count=cap;return transform_proto(b,o,op);}
'''

class ProtoImportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax proto ');cls.d=Path(cls.temp.name)
        cls.source=cls.d/'wire.coax'
        cls.source.write_text(SOURCE.replace('NOSTR',str(ROOT/'proto/descriptors/nostr.pb')).replace('TYPES',str(ROOT/'coax/tests/proto/types.pb')).replace('VERSE',str(ROOT/'proto/descriptors/verse.pb')))
        (cls.d/'wire.c').write_bytes(checked([COAX,'c',cls.source,'--library','--header',cls.d/'wire.h']))
        (cls.d/'host.c').write_text(HOST)
        checked(['cc','-std=c11','-pedantic-errors','-O2','-shared','-fPIC',cls.d/'wire.c',cls.d/'host.c','-lm','-o',cls.d/'wire.dylib'])
        cls.lib=C.CDLL(str(cls.d/'wire.dylib'))
        cls.lib.inspect.argtypes=[C.c_void_p,C.c_int64,C.c_uint32];cls.lib.inspect.restype=C.c_uint64
        cls.lib.transform.argtypes=[C.c_void_p,C.c_int64,C.c_void_p,C.c_int64,C.c_uint32];cls.lib.transform.restype=C.c_int64
        cls.event=(ROOT/'proto/nostr/fixtures/owner-attested.event.pb').read_bytes()
        (cls.d/'release.c').write_bytes(checked([COAX,'c',cls.source,'--library','--release']))
        checked(['cc','-std=c11','-pedantic-errors','-O2','-shared','-fPIC',cls.d/'release.c',cls.d/'host.c','-lm','-o',cls.d/'release.dylib'])
        cls.release=C.CDLL(str(cls.d/'release.dylib'))
        cls.release.inspect.argtypes=cls.lib.inspect.argtypes;cls.release.inspect.restype=C.c_uint64
        cls.release.transform.argtypes=cls.lib.transform.argtypes;cls.release.transform.restype=C.c_int64
    @classmethod
    def tearDownClass(cls):cls.temp.cleanup()
    def inspect(self,data,op):return self.lib.inspect(C.c_char_p(data),len(data),op)
    def transform(self,data,op,capacity=4096):
        buf=C.create_string_buffer(capacity+16);buf.raw=b'Z'*(capacity+16)
        count=self.lib.transform(C.c_char_p(data),len(data),buf,capacity,op)
        self.assertEqual(buf.raw[capacity:],b'Z'*16)
        return count,buf.raw[:max(0,count)]
    def test_actual_event_roundtrip_unknowns_and_edit(self):
        wire=self.event+v(500,2**64-1)+b(501,b'opaque')
        self.assertEqual(self.inspect(wire,0),0)
        self.assertEqual(self.inspect(wire,1),1)
        self.assertGreater(self.inspect(wire,16),0)
        self.assertEqual(self.inspect(wire,17),500)
        n,out=self.transform(wire,5);self.assertEqual(out,wire);self.assertEqual(n,len(wire))
        n,out=self.transform(wire,0);self.assertGreater(n,0);self.assertEqual(self.inspect(out,0),0);self.assertEqual(self.inspect(out,1),0)
        self.assertTrue(out.endswith(v(4,0)));self.assertIn(v(500,2**64-1)+b(501,b'opaque'),out)
        self.assertEqual(self.transform(wire,9)[0],-2)
    def test_release_wire_parity(self):
        lib=self.lib
        try:
            vectors=[self.event,self.event+v(500,22),b'',v(21,0)+v(22,0),b(17,var(1)+var(4)),b'\xff',self.transform(b'',2)[1]]
            for data in vectors:
                expected=[self.inspect(data,op) for op in range(21)]
                outputs=[self.transform(data,op) for op in range(11)]
                self.lib=self.release
                self.assertEqual([self.inspect(data,op) for op in range(21)],expected)
                self.assertEqual([self.transform(data,op) for op in range(11)],outputs)
                self.lib=lib
        finally:self.lib=lib

    def test_presence_oneof_and_empty_containers(self):
        self.assertEqual(self.inspect(b'',11),0)
        n,out=self.transform(b'',6);self.assertEqual(out,b(4,b''));self.assertEqual(self.inspect(out,11),1)
        n,out=self.transform(b'',7);self.assertEqual(out,v(2,0));self.assertEqual(self.inspect(out,12),2)
        self.assertEqual(self.inspect(b(1,b'one'),13),1) # Block identity Audience.
        self.assertEqual(self.inspect(b'',14),5) # Block channel required original.
        self.assertEqual(self.inspect(b'',15),5) # Block agent required authentication.
        original=(ROOT/'proto/nostr/fixtures/owner-attested.event.json').read_bytes()
        signed=b(1,original)+b(2,self.event)
        self.assertEqual(self.inspect(b(1,signed)+b(2,b'group'),14),0)
        authentication=b(1,signed)+b(2,b'wss://relay.example')+b(3,b'challenge')
        self.assertEqual(self.inspect(b(1,authentication),15),0) # Projection, not verified admission.
        n,out=self.transform(v(21,0)+v(22,0),3)
        self.assertEqual(out,v(22,0)+b(20,b'changed'));self.assertEqual(self.inspect(out,8),20)
        self.assertEqual(self.inspect(out,9),1)
        n,out=self.transform(out,10);self.assertEqual(self.inspect(out,9),0)
    def test_scalars_and_bounded_native_writers(self):
        n,out=self.transform(b'',2);self.assertGreater(n,0);self.assertEqual(self.inspect(out,3),0)
        self.assertEqual(self.inspect(out,4),2**64-1);self.assertEqual(self.inspect(out,5),2**63)
        self.assertEqual(self.inspect(out,6),0x80000000);self.assertEqual(self.inspect(out,7),0x7ff8000000000042)
        self.assertEqual(self.inspect(out,10),2)
        self.assertEqual(self.transform(out,1)[1],out)
        for size in (0,1,n-1):self.assertEqual(self.transform(b'',2,size)[0],-2)
        self.assertEqual(self.transform(b'',8)[0],-4)
        self.assertEqual(self.inspect(v(4,2**53+1),4),2**53+1)
        # Open enums retain unknown numbers, not a checked closed-enum cast.
        self.assertEqual(self.inspect(v(12,773),3),0)
    def test_packed_unpacked_and_nested(self):
        wire=b(17,var(1)+var(4))+v(17,6)+b(19,b(1,b'a'))+b(19,b(1,b'b'))
        self.assertEqual(self.inspect(wire,3),0);self.assertEqual(self.inspect(wire,10),3)
        n,out=self.transform(wire,4);self.assertEqual(self.inspect(out,10),4)
        deep=b''
        for _ in range(66):deep=b(23,deep)
        self.assertEqual(self.inspect(deep,3),2)
        self.assertEqual(self.inspect(b(17,b'\x80'),3),1)
    def test_malformed_limits_and_duplicate_policy(self):
        malformed=[b'\0',b'\x80',b'\x08'+b'\xff'*9+b'\x02',b'\x0b',b'\x0a\xff\xff\xff\xff\x0f',b'\x09'+b'\0'*7]
        for wire in malformed:self.assertNotEqual(self.inspect(wire,3),0)
        for text in (b'\xff',b'\xc0\x80',b'\xed\xa0\x80',b'\xf4\x90\x80\x80',b'\xe2\x82'):
            self.assertEqual(self.inspect(b(9,text),3),4)
        self.assertEqual(self.inspect(v(4,0)+v(4,1),3),3)
        self.assertEqual(self.inspect(v(21,1)+b(20,b'hi'),3),3)
        self.assertEqual(self.inspect(v(11,2**32),3),7)
        self.assertEqual(self.inspect(self.event+v(4,0),0),3)
        self.assertEqual(self.inspect(self.event[:-1],0),1)
        self.assertEqual(self.inspect(b'',0),5)
        self.assertEqual(self.inspect(b(1,b'bad'),0),2)
        self.assertEqual(self.inspect(b'\x00'*1048577,3),2)
        self.assertEqual(self.inspect(b(17,b'\0'*524288),3),2)
        self.assertEqual(self.inspect(b(1,b'a'*65),3),1) # double cannot be length-delimited.
    def test_descriptor_policy_constraints(self):
        self.assertEqual(self.inspect(b(1,b'x')*64,18),0)
        self.assertEqual(self.inspect(b(1,b'x')*65,18),2)
        peer=b(1,b'actor')+b(2,b'')+v(4,0)
        self.assertEqual(self.inspect(peer,19),0)
        for value in (float('nan'),float('inf'),3.2,-3.2):
            self.assertEqual(self.inspect(peer+var(3*8+1)+struct.pack('<d',value),19),7)
        self.assertEqual(self.inspect(b(1,b'')+b(2,b'')+v(4,0),19),7)
        self.assertEqual(self.inspect(b(1,b'a\x00b')+b(2,b'')+v(4,0),19),7)
        snapshot=b(1,b(1,b'world'))+v(3,0)
        self.assertEqual(self.inspect(snapshot+b(2,peer)*256,20),0)
        self.assertEqual(self.inspect(snapshot+b(2,peer)*257,20),2)

    def test_binary_intrinsics(self):
        src=self.d/'bits.coax';src.write_text('''#import "Basic";
#assert #run f32_bits(f32_from_bits(2147483648)) == 2147483648;
#assert #run f64_bits(f64_from_bits(4607182418800017408)) == 4607182418800017408;
main :: () {a:[4] u8;b:[4] u8;
 print("% % % %\\n",bytes_overlap(a[..],a[1..<3]),bytes_overlap(a[..],b[..]),bytes_overlap(a[..],a[0..<0]),f32_bits(f32_from_bits(2139095040)));
}''')
        for mode in ([],['--release']):
            r=run([COAX,'run',src,*mode]);self.assertEqual(r.returncode,0,r.stderr.decode());self.assertEqual(r.stdout,b'true false false 2139095040\n')
        for expr,code in [('f32_bits(1.0f64)','E0378'),('bytes_overlap(1,2)','E0378'),('f32_from_bits()','E0378'),('#run f64_from_bits(9218868437227405312)','E0412'),('#run bytes_overlap("a","a")','E0404')]:
            src.write_text('main :: () { x:='+expr+'; }')
            r=run([COAX,'check',src]);self.assertNotEqual(r.returncode,0);self.assertIn(code.encode(),r.stderr)
        src.write_text('f :: (x:f32)->f32 {return f32_from_bits(f32_bits(x));}')
        self.assertIn(b'E0377',run([COAX,'metal',src]).stderr)

    def test_optional_minimal_varint_policy(self):
        for wire in (b'\xa0\x00\x00',b'\x20\x80\x00',b'\x4a\x81\x00a',b(18,b'\x0a\x81\x00a'),b(17,b'\x81\x00')):
            self.assertEqual(self.inspect(wire,3),0)
            self.assertEqual(self.inspect(wire,23),1)
        self.assertEqual(self.inspect(v(8,2),3),0)
        self.assertEqual(self.inspect(v(8,2),23),7)
        self.assertEqual(self.inspect(self.transform(b'',2)[1],23),0)

    def test_aliased_and_small_output(self):
        buf=C.create_string_buffer(self.event,4096);before=buf.raw
        self.assertEqual(self.lib.transform(buf,len(self.event),buf,4096,0),-6)
        self.assertEqual(buf.raw,before)
        self.assertEqual(self.transform(self.event,0,1)[0],-2)
    def test_all_profiles_and_driver_diagnostics(self):
        for name in ('nostr-event','block-agent','block-identity','block-channel','coax-foundation','phone-agent','ios-home','metareal','verse'):
            src=self.d/'profile.coax';src.write_text('#import_proto "'+str(ROOT/'proto/descriptors'/f'{name}.pb')+'" as P;\nmain :: () {}')
            r=run([COAX,'check',src]);self.assertEqual(r.returncode,0,r.stderr.decode())
        for source,code in [('#import_proto "missing.pb" as P;','E0502'),('#import_proto "a";','E0501'),('#import_proto "a" as P; #import_proto "a" as P;','E0501')]:
            src=self.d/'bad.coax';src.write_text(source+'\nmain :: () {}')
            r=run([COAX,'check',src,'--json']);self.assertNotEqual(r.returncode,0)
            diagnostics=[json.loads(x) for x in r.stderr.decode().splitlines()]
            self.assertEqual(diagnostics[0]['code'],code)
        src=self.d/'metal.coax';src.write_text('#import_proto "a" as P;')
        self.assertIn(b'E0504',run([COAX,'metal',src]).stderr)
        # Expanded imports preserve the original user's line numbers.
        src=self.d/'line.coax';src.write_text('#import_proto "'+str(ROOT/'coax/tests/proto/types.pb')+'" as P;\nmain :: () { nope(); }')
        r=run([COAX,'check',src,'--json']);self.assertEqual(json.loads(r.stderr.splitlines()[0])['line'],2)
        src.write_text('#import_proto "'+str(ROOT/'coax/tests/proto/types.pb')+'" as P;\n#import_proto "'+str(ROOT/'coax/tests/proto/types.pb')+'" as Q;\nmain :: () {m:=P_Inner_new();n:=Q_Inner_new();}')
        self.assertEqual(run([COAX,'check',src]).returncode,0)
        env=dict(os.environ,PATH=str(COAX.parent)+os.pathsep+os.environ['PATH'])
        self.assertEqual(run(['coax','check',src],env=env).returncode,0)
    def test_import_provenance_temporary_paths_and_generated_locations(self):
        src=self.d/'provenance.coax'
        descriptor=ROOT/'coax/tests/proto/types.pb'
        src.write_text(f'#import_proto "{descriptor}" as P;\nmain :: () {{}}')
        temp=self.d/'host temp';temp.mkdir()
        env=dict(os.environ,TMPDIR=str(temp))
        r=run([COAX,'c',src],env=env);self.assertEqual(r.returncode,0,r.stderr.decode())
        first=r.stdout.splitlines()[0].decode()
        self.assertIn('descriptor_sha256='+hashlib.sha256(descriptor.read_bytes()).hexdigest(),first)
        self.assertIn('generator=coax0-native-v1',first)
        self.assertIn('compiler='+checked([COAX,'--version']).decode().strip().replace(' ','-'),first)
        core=(ROOT/'coax/lib/proto/wire.coax').read_bytes()
        body=core.split(b'// @proto-runtime-checks\n')[0]+(ROOT/'coax/lib/proto/memory.coax').read_bytes()
        self.assertIn('runtime_sha256='+hashlib.sha256(body).hexdigest(),first)
        self.assertNotIn('python=',first);self.assertEqual(list(temp.iterdir()),[])
        # The duplicate emitted declaration is located at the import, not at
        # a fictitious source line thousands of lines beyond this short file.
        src.write_text(f'#import_proto "{descriptor}" as P;\nP_Scalars :: struct {{}}\nmain :: () {{}}')
        r=run([COAX,'check',src,'--json'],env=env);self.assertEqual(r.returncode,1)
        errors=[json.loads(line) for line in r.stderr.splitlines()]
        self.assertTrue(any(x['line']==1 and 'generated proto import P' in x['message'] for x in errors),errors)
        self.assertEqual(list(temp.iterdir()),[])

    def test_schema_declaration_characterization(self):
        baseline=json.loads((ROOT/'coax/tests/proto/import-baselines.json').read_text())
        for name,expected in baseline['profiles'].items():
            descriptor=ROOT/name
            self.assertEqual(hashlib.sha256(descriptor.read_bytes()).hexdigest(),expected['descriptor_sha256'],name)
            source=checked([COAX,'proto',descriptor,'--prefix',baseline['prefix']])
            body=source.split(b'// @proto-descriptor-declarations\n',1)[1]
            self.assertEqual(hashlib.sha256(body).hexdigest(),expected['declarations_sha256'],name)
            self.assertNotIn(b'#authority;',source)
            self.assertNotIn(b'@proto-runtime-checks',source)

    def test_relocated_compiler_needs_no_interpreter_or_helpers(self):
        isolated=self.d/'standalone';isolated.mkdir()
        executable=isolated/'coax';shutil.copy2(COAX,executable)
        empty_path=isolated/'empty';empty_path.mkdir()
        env=dict(os.environ,PATH=str(empty_path),COAX_PROTO_TOOL='/no/tool',COAX_PROTO_PYTHON='/no/python',TMPDIR='/no/tmp')
        # Actual iOS source, not just a synthetic empty message. Both C and
        # header generation must work from the relocated binary with PATH empty.
        source=ROOT/'metareal/home.coax';header=isolated/'Home.h'
        r=run([executable,'c',source,'--library','--header',header],env=env)
        self.assertEqual(r.returncode,0,r.stderr.decode())
        self.assertIn(b'generator=coax0-native-v1',r.stdout)
        self.assertIn(b'metareal_home_init',header.read_bytes())
        fixture=ROOT/'coax/tests/proto/types.pb';generated=isolated/'types.coax'
        r=run([executable,'proto',fixture,'--prefix','Alone','-o',generated],env=env)
        self.assertEqual(r.returncode,0,r.stderr.decode());self.assertIn(b'Alone_Scalars',generated.read_bytes())
        self.assertEqual(set(x.name for x in isolated.iterdir()),{'coax','empty','Home.h','types.coax'})

    def test_importer_rejects_unsupported_descriptors(self):
        # Descriptor fixtures are built with the wire format itself, no host protobuf dependency.
        def descriptor(message,syntax=b'proto3'):
            return b(1,b(1,b'test.proto')+b(4,message)+b(12,syntax))
        good=b(1,b'M')
        bads=[b'\xff',descriptor(good,b'proto2'),descriptor(good+b(7,v(7,1))), # map_entry
              descriptor(good+b(2,b(1,b'x')+v(3,1)+v(4,1)+v(5,10))), # group
              descriptor(good+b(7,b(51001,b'not an integer')))]
        for i,wire in enumerate(bads):
            path=self.d/f'bad{i}.pb';path.write_bytes(wire)
            src=self.d/'bad.coax';src.write_text(f'#import_proto "{path}" as P;\nmain :: () {{}}')
            r=run([COAX,'check',src,'--json']);self.assertNotEqual(r.returncode,0)
            self.assertEqual(json.loads(r.stderr.splitlines()[0])['code'],'E0502')
    def test_namespaced_leaf_collisions(self):
        path=self.d/'names.pb'
        files=[b(1,b(1,package+b'.proto')+b(2,package)+b(4,b(1,b'Item'))+b(12,b'proto3')) for package in (b'one',b'two')]
        path.write_bytes(b''.join(files))
        src=self.d/'names.coax';src.write_text(f'#import_proto "{path}" as P;\nmain :: () {{a:=P_one_Item_new();b:=P_two_Item_new();}}')
        r=run([COAX,'check',src]);self.assertEqual(r.returncode,0,r.stderr.decode())
        # A leaf that collides with a generated helper is rejected explicitly.
        path.write_bytes(b(1,b(1,b'test.proto')+b(4,b(1,b'Item'))+b(4,b(1,b'Item_Option'))+b(12,b'proto3')))
        r=run([COAX,'check',src]);self.assertIn(b'E0502',r.stderr)

    def test_native_descriptor_bounds_and_cli_diagnostics(self):
        def descriptor(message):return b(1,b(1,b'test.proto')+b(4,message)+b(12,b'proto3'))
        cases=[descriptor(b(1,b'M'*257)),v(99,0)*40001,
               descriptor(b(1,b'M')+b(2,b(1,b'x')+v(3,1)+v(4,1)+v(5,5)+v(17,2))),
               descriptor(b(1,b'M')+b(2,b(1,b'x')+v(3,1)+v(4,1)+v(5,5)+var(8*8+1)+b'\0'*8)),
               b'\0'*(8*1024*1024+1)]
        for i,wire in enumerate(cases):
            descriptor_file=self.d/f'native-bad-{i}.pb';descriptor_file.write_bytes(wire)
            r=run([COAX,'proto',descriptor_file,'--prefix','P','--json'])
            self.assertEqual(r.returncode,1,r.stdout.decode())
            self.assertEqual(json.loads(r.stderr)['code'],'E0502')
        r=run([COAX,'proto',ROOT/'coax/tests/proto/types.pb','--prefix','bad-prefix','--json'])
        self.assertEqual(r.returncode,1);self.assertEqual(json.loads(r.stderr)['code'],'E0502')

    def test_native_importer_under_sanitizers(self):
        harness=self.d/'native.cpp'
        code='#include '+json.dumps(str(ROOT/'coax/src/proto.cpp'))+'\n#include <cassert>\n#include <fstream>\n#include <iterator>\nint main(int argc,char **argv){\n'
        # Independent hashlib oracle, including both SHA padding boundaries.
        for size in (0,3,55,56,57,63,64,65,1000):
            data=b'x'*size
            code+='assert(coax::proto::sha256(std::string('+str(size)+",'x'))=="+json.dumps(hashlib.sha256(data).hexdigest())+');\n'
        code+='''uint32_t state=17;std::string random;
for(int n=0;n<1024;n++){random.resize((size_t)n);for(char &c:random){state=state*1664525+1013904223;c=(char)(state>>24);}
 auto result=coax::proto::import_descriptor(random,"Fuzz","test");assert(result.source.size()<=coax::proto::MAX_SOURCE);}
for(int arg=1;arg<argc;arg++){std::ifstream file(argv[arg],std::ios::binary);std::string raw((std::istreambuf_iterator<char>(file)),{});
 assert(!raw.empty());assert(coax::proto::import_descriptor(raw,"Fuzz","test").error.empty());
 for(int i=0;i<64;i++){std::string changed=raw;state=state*1664525+1013904223;size_t pos=state%changed.size();changed[pos]^=(char)(1u<<(i%8));
  auto result=coax::proto::import_descriptor(changed,"Fuzz","test");assert(result.source.size()<=coax::proto::MAX_SOURCE);}}
return 0;}'''
        harness.write_text(code)
        executable=self.d/'native-sanitized'
        checked([os.environ.get('CXX','c++'),'-std=c++20','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I',ROOT/'coax/bin',harness,'-o',executable])
        descriptors=sorted((ROOT/'proto/descriptors').glob('*.pb'))+[ROOT/'coax/tests/proto/types.pb']
        r=run([executable,*descriptors],env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0',UBSAN_OPTIONS='halt_on_error=1'))
        self.assertEqual(r.returncode,0,r.stderr.decode())

    def test_sanitizer_target_and_fuzz(self):
        # Run generated C, not the host importer, under ASan+UBSan.
        host=self.d/'san.c';host.write_text(HOST+'''
#include <assert.h>
int main(void){uint8_t w[512],out[4096];uint32_t state=17;
for(int n=0;n<512;n++){for(int i=0;i<n;i++){state=state*1664525+1013904223;w[i]=(uint8_t)(state>>24);}
(void)inspect(w,n,3);(void)transform(w,n,out,sizeof out,1);}
assert(transform(w,0,out,sizeof out,2)>0);return 0;}
''')
        checked(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',self.d/'wire.c',host,'-lm','-o',self.d/'san'])
        r=run([self.d/'san'],env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0'));self.assertEqual(r.returncode,0,r.stderr.decode())
    @unittest.skipUnless(ORACLE,'run with --oracle and pinned PROTOC for official interoperability')
    def test_official_protoc_oracle(self):
        protoc=os.environ['PROTOC']
        self.assertEqual(checked([protoc,'--version']).strip(),b'libprotoc 29.3')
        rebuilt=checked([protoc,'-I',ROOT/'coax/tests/proto','--include_imports','--descriptor_set_out=/dev/stdout','types.proto'])
        self.assertEqual(rebuilt,(ROOT/'coax/tests/proto/types.pb').read_bytes())
        n,wire=self.transform(b'',2)
        text=checked([protoc,'-I',ROOT/'coax/tests/proto','--decode=coax.test.Scalars','types.proto'],input=wire)
        for expected in (b'i64: -9223372036854775808',b'u64: 18446744073709551615',b'zig32: -2147483648',b'zig64: -9223372036854775808',b'opt: 0',b'text: "nested"',b'state: NEGATIVE',b'real64: nan',b'real32: -0',b'i32: -2147483648',b'fixed64_value: 18446744073709551615',b'fixed32_value: 4294967295',b'enabled: true',b'text: "hello"',b'sf32: -2147483648',b'sf64: -9223372036854775808',b'u32: 4294967295',b'numbers: -1',b'numbers: 2'):
            self.assertIn(expected,text)
        external=checked([protoc,'-I',ROOT/'coax/tests/proto','--encode=coax.test.Scalars','types.proto'],input=b'u64: 18446744073709551615\nzig64: -9223372036854775808\nreal32: -0\nnumbers: [-1, 0, 1]\nrank: 0\nopt: 0\n')
        self.assertEqual(self.inspect(external,4),2**64-1);self.assertEqual(self.inspect(external,5),2**63)
        self.assertEqual(self.inspect(external,6),2**31);self.assertEqual(self.inspect(external,10),3)
        n,edited=self.transform(self.event,0)
        decoded=checked([protoc,'-I',ROOT/'proto','--decode=nostr.event.v1alpha1.Event','nostr/event/v1alpha1/event.proto'],input=edited)
        self.assertIn(b'kind: 0',decoded);self.assertIn(b'sig:',decoded)

if __name__=='__main__':unittest.main()

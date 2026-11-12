#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python protobuf json_format and C sanitizers check the generated mapper)
"""The #import_proto JSON mapper through the generated C ABI (coax/proto.md).

json_map.proto -> json_map.pb -> generated Coax -> C11, checked and release.
Every case in json_map_cases.json runs both ways: JSON to protobuf wire bytes
and wire bytes to JSON, byte for byte, with sentinels after each buffer and
every short buffer refused with LIMIT.

  --sanitize  the same cases, every shorter capacity in an exactly sized heap
              buffer, and a seeded mutation sweep, in C under ASan and UBSan
  --oracle    Python protobuf's json_format (proto/toolchain.json's version)
              agrees on every case marked oracle; with PROTOC=protoc, the
              descriptor is rebuilt byte for byte as well
"""
import ctypes as C
import json, os, pathlib, random, subprocess, sys, tempfile
if not __debug__: raise SystemExit('tests require Python assertions')
R=pathlib.Path(__file__).resolve().parents[3]
COAX=pathlib.Path(os.environ.get('COAX_TEST_BIN',str(R/'coax/bin/coax')))
HERE=R/'coax/tests/proto'
PB=HERE/'json_map.pb'
CASES=json.loads((HERE/'json_map_cases.json').read_text())
MESSAGES=['Inner','Scalars','Attempt','Nulls']
if any(a not in ('--oracle','--sanitize') for a in sys.argv[1:]):raise SystemExit('usage: json_map.py [--oracle] [--sanitize]')
LIMIT,ALIAS=2,6

def run(*args,**kw):
    p=subprocess.run([str(x) for x in args],capture_output=True,**kw)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    return p.stdout

def source():
    out=['#import_proto "'+str(PB)+'" as J;','#load "'+str(R/'coax/lib/proto/json.coax')+'";','Buffer :: struct { bytes:[] u8; }',
         'jm_from_json :: (which:u32,text:Buffer,out:Buffer)->s64 #export_c #effects(all) {',
         ' nodes:[512] JvNode;t:=cast(string)text.bytes;o:=out.bytes;e:u32=99;n:=0;']
    for i,m in enumerate(MESSAGES):out.append(' if which=='+str(i)+' {m:=J_'+m+'_from_json(t,nodes[..],o);e=m.error;n=m.wire.count;}')
    out+=[' if e!=0 {return -cast(s64)e;}return cast(s64)n;','}',
          'jm_to_json :: (which:u32,wire:Buffer,out:Buffer)->s64 #export_c #effects(all) {',
          ' w:=wire.bytes;o:=out.bytes;v:=J__pb_View.{error=99};']
    for i,m in enumerate(MESSAGES):out.append(' if which=='+str(i)+' {v=J_'+m+'_to_json(J_'+m+'_decode(w),o);}')
    out+=[' if v.error!=0 {return -cast(s64)v.error;}return cast(s64)v.wire.count;','}']
    return '\n'.join(out)+'\n'

HOST='''#include "jm.h"
int64_t from_json(uint32_t w,const uint8_t*p,int64_t n,uint8_t*out,int64_t cap){coax_t_Buffer b={0},o={0};b.bytes.data=(uint8_t*)p;b.bytes.count=n;o.bytes.data=out;o.bytes.count=cap;return jm_from_json(w,b,o);}
int64_t to_json(uint32_t w,const uint8_t*p,int64_t n,uint8_t*out,int64_t cap){coax_t_Buffer b={0},o={0};b.bytes.data=(uint8_t*)p;b.bytes.count=n;o.bytes.data=out;o.bytes.count=cap;return jm_to_json(w,b,o);}
'''

def directions():
    """(name, message index, direction, input bytes, expected bytes or -error)."""
    for c in CASES['round_trips']:
        w=MESSAGES.index(c['message']);text=c['json'].encode();wire=bytes.fromhex(c['wire'])
        yield c['name']+' (from JSON)',w,'from',text,wire
        yield c['name']+' (to JSON)',w,'to',wire,text
    for c in CASES['reads']:yield c['name'],MESSAGES.index(c['message']),'from',c['json'].encode('utf-8','surrogateescape'),bytes.fromhex(c['wire'])
    for c in CASES['writes']:yield c['name'],MESSAGES.index(c['message']),'to',bytes.fromhex(c['wire']),c['json'].encode()
    for c in CASES['errors']:
        text=c['json'].encode('latin-1') if '\xff' in c['json'] else c['json'].encode()
        yield c['name'],MESSAGES.index(c['message']),'from',text,-c['error']

def check_library(lib,label):
    for fn in (lib.from_json,lib.to_json):
        fn.argtypes=[C.c_uint32,C.c_char_p,C.c_int64,C.c_void_p,C.c_int64];fn.restype=C.c_int64
    count=0
    for name,w,way,data,want in directions():
        fn=lib.from_json if way=='from' else lib.to_json
        cap=4096;buf=C.create_string_buffer(cap+16);buf.raw=b'Z'*(cap+16)
        got=fn(w,data,len(data),buf,cap)
        assert buf.raw[cap:]==b'Z'*16,name
        if isinstance(want,int):assert got==want,(label,name,got,want)
        else:
            assert got==len(want) and buf.raw[:got]==want,(label,name,got,buf.raw[:max(got,0)],want)
            # every shorter buffer is refused with LIMIT and nothing past it changes
            for short in range(len(want)):
                b=C.create_string_buffer(short+16);b.raw=b'Z'*(short+16)
                assert fn(w,data,len(data),b,short)==-LIMIT,(label,name,short)
                assert b.raw[short:]==b'Z'*16,(label,name,short)
        count+=1
    # The input and the output may not overlap, partly or wholly.
    text=b'{"u32":1}';buf=C.create_string_buffer(64);buf.raw=text+b'\0'*(64-len(text))
    base=C.addressof(buf)
    assert lib.from_json(1,C.cast(base,C.c_char_p),len(text),base+4,32)==-ALIAS
    wire=bytes.fromhex('4801');buf.raw=wire+b'\0'*62
    assert lib.to_json(1,C.cast(base,C.c_char_p),len(wire),base+1,32)==-ALIAS
    # Writing needs a complete, valid message.
    assert lib.to_json(1,b'\x48',1,C.create_string_buffer(64),64)==-1
    print(label,count,'JSON mapper cases passed, with every short buffer refused')

def sanitizer(d,c_source):
    lines=['#include <stdint.h>','#include <stdio.h>','#include <stdlib.h>','#include <string.h>','#include "jm.h"',HOST.split('\n',1)[1]]
    lines.append('static int check(int index,uint32_t w,int from,const uint8_t*in,int64_t n,const uint8_t*want,int64_t want_n,int64_t error){')
    lines.append(' for(int64_t cap=error?4096:0;cap<=(error?4096:want_n);cap++){uint8_t*out=malloc(cap?cap:1);uint8_t*copy=malloc(n?n:1);memcpy(copy,in,n);')
    lines.append('  int64_t got=from?from_json(w,copy,n,out,cap):to_json(w,copy,n,out,cap);')
    lines.append('  int ok=error?got==-error:(cap<want_n?got==-2:(got==want_n&&!memcmp(out,want,want_n)));free(out);free(copy);')
    lines.append('  if(!ok){fprintf(stderr,"case %d at capacity %lld: got %lld\\n",index,(long long)cap,(long long)got);return 1;}}')
    lines.append(' return 0;}')
    lines.append('int main(void){int bad=0;')
    for i,(name,w,way,data,want) in enumerate(directions()):
        a=','.join(map(str,data)) or '0';lines.append('static const uint8_t in'+str(i)+'[]={'+a+'};')
        if isinstance(want,int):lines.append('bad|=check('+str(i)+','+str(w)+','+str(int(way=='from'))+',in'+str(i)+','+str(len(data))+',0,0,'+str(-want)+');')
        else:
            b=','.join(map(str,want)) or '0'
            lines.append('{static const uint8_t want[]={'+b+'};bad|=check('+str(i)+','+str(w)+','+str(int(way=='from'))+',in'+str(i)+','+str(len(data))+',want,'+str(len(want))+',0);}')
    # A seeded sweep: each case's input with one byte changed, every message, both ways.
    rng=random.Random(13237);inputs=[data for _,_,_,data,_ in directions() if data]
    lines.append('static uint8_t scratch[8192];')
    for k in range(400):
        data=bytearray(rng.choice(inputs));data[rng.randrange(len(data))]=rng.randrange(256)
        a=','.join(map(str,data))
        lines.append('{static const uint8_t m[]={'+a+'};for(uint32_t w=0;w<4;w++){(void)from_json(w,m,'+str(len(data))+',scratch,sizeof scratch);(void)to_json(w,m,'+str(len(data))+',scratch,sizeof scratch);}}')
    lines.append('return bad;}')
    (d/'sanitizer.c').write_text('\n'.join(lines)+'\n')
    exe=d/'sanitized'
    run(os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-fno-omit-frame-pointer','-I'+str(d),c_source,d/'sanitizer.c','-lm','-o',exe)
    run(exe,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0'))
    print('sanitized: every case at every capacity and a 400-input mutation sweep under ASan and UBSan')

def oracle():
    from google.protobuf import descriptor_pb2,descriptor_pool,json_format,message_factory
    if os.environ.get('PROTOC'):
        rebuilt=run(os.environ['PROTOC'],'-I'+str(HERE),'-I'+str(R/'proto'),'--include_imports','--descriptor_set_out=/dev/stdout','json_map.proto')
        assert rebuilt==PB.read_bytes(),'json_map.pb is not what protoc builds from json_map.proto'
    pool=descriptor_pool.DescriptorPool()
    for f in descriptor_pb2.FileDescriptorSet.FromString(PB.read_bytes()).file:pool.Add(f)
    def make(n):return message_factory.GetMessageClass(pool.FindMessageTypeByName('coax.test.json.'+n))()
    def text(m):return json.dumps(json_format.MessageToDict(m),separators=(',',':'),ensure_ascii=False)
    count=0
    for c in CASES['round_trips']+CASES['reads']+CASES['writes']:
        if not c['oracle']:continue
        wire=bytes.fromhex(c['wire'])
        if 'json' in c and c in CASES['round_trips']+CASES['reads']:
            m=json_format.Parse(c['json'],make(c['message']),ignore_unknown_fields=True)
            assert m.SerializeToString()==wire,(c['name'],m.SerializeToString().hex(),c['wire'])
        if c in CASES['round_trips']+CASES['writes']:
            m=make(c['message']);m.ParseFromString(wire)
            assert text(m)==c['json'],(c['name'],text(m),c['json'])
        count+=1
    for c in CASES['errors']:
        if not c['oracle']:continue
        text=c['json'].encode('latin-1') if 'ÿ' in c['json'] else c['json']
        try:json_format.Parse(text,make(c['message']),ignore_unknown_fields=True)
        except (json_format.ParseError,ValueError,UnicodeError):count+=1;continue
        raise AssertionError('Python protobuf accepted '+c['name'])
    print('oracle: Python protobuf json_format agrees on',count,'cases')

with tempfile.TemporaryDirectory(prefix='coax-json-map-') as t:
    d=pathlib.Path(t);src=d/'jm.coax';src.write_text(source())
    (d/'host.c').write_text(HOST)
    for mode,flags in [('checked',[]),('release',['--release'])]:
        out=d/(mode+'.c');out.write_bytes(run(COAX,'c',src,'--library','--header',d/'jm.h',*flags))
        lib=d/(mode+'.so');run(os.environ.get('CC','cc'),'-std=c11','-O2','-shared','-fPIC','-I'+str(d),out,d/'host.c','-lm','-o',lib)
        check_library(C.CDLL(str(lib)),mode)
        if '--sanitize' in sys.argv and mode=='checked':sanitizer(d,out)
    if '--oracle' in sys.argv:oracle()

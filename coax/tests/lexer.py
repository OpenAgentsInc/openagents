#!/usr/bin/env python3
"""Actual compiled Coax lexer: ABI/spans/recovery, independent decimal oracles,
and native address/bounds fuzzing. Python is test tooling, never bootstrap."""
import ctypes as C
from decimal import Decimal, localcontext
import math
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
COAX=ROOT/'bin/coax'

class Text(C.Structure): _fields_=[('data',C.c_void_p),('count',C.c_int64)]
class Loc(C.Structure): _fields_=[(n,C.c_int64) for n in ('line','column','offset')]
class Token(C.Structure):
    _fields_=[('kind',C.c_uint8),('begin',C.c_int64),('end',C.c_int64),('loc',Loc),('integer',C.c_uint64),('floating',C.c_uint64),('width',C.c_int64),('decoded',C.c_bool),('decoded_count',C.c_int64)]
class Diagnostic(C.Structure): _fields_=[('code',Text),('message',Text),('fix',Text),('loc',Loc)]
class Item(C.Structure): _fields_=[('has_token',C.c_bool),('token',Token),('diagnostic',Diagnostic)]
class State(C.Structure): _fields_=[(n,C.c_int64) for n in ('pos','line','line_start','mode','start','start_line','start_column','decoded_count')]
class StateView(C.Structure): _fields_=[('data',C.POINTER(State)),('count',C.c_int64)]
class BytesView(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]

def checked(args,**kwargs):
    r=subprocess.run(list(map(str,args)),capture_output=True,timeout=180,**kwargs)
    if r.returncode: raise AssertionError(r.stderr.decode(errors='replace'))
    return r.stdout

def raw(t): return C.string_at(t.data,t.count) if t.count else b''
def float_bits(value): return struct.unpack('>Q',struct.pack('>d',value))[0]

class Lexer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp=tempfile.TemporaryDirectory(prefix='coax-native-lexer-');cls.d=Path(cls.tmp.name)
        cls.source=cls.d/'lexer.c';cls.header=cls.d/'lexer.h'
        cls.source.write_bytes(checked([COAX,'c',ROOT/'lib/compiler/lexer.coax','--library','--header',cls.header]))
        checked([os.environ.get('CC','cc'),'-std=c11','-O1','-g','-shared','-fPIC',cls.source,'-lm','-o',cls.d/'lexer.dylib'])
        cls.lib=C.CDLL(str(cls.d/'lexer.dylib'))
        cls.lib.LX_decimal.argtypes=[Text];cls.lib.LX_decimal.restype=C.c_uint64
        cls.lib.LX_begin.argtypes=[C.c_int64];cls.lib.LX_begin.restype=State
        cls.lib.LX_next.argtypes=[Text,StateView,BytesView,BytesView];cls.lib.LX_next.restype=Item
        cls.lib.LX_name.argtypes=[C.c_uint32];cls.lib.LX_name.restype=Text
    @classmethod
    def tearDownClass(cls): cls.tmp.cleanup()

    def number(self,text):
        data=text.encode();buf=C.create_string_buffer(data)
        return self.lib.LX_decimal(Text(C.addressof(buf),len(data)))

    def tokens(self,data):
        buf=C.create_string_buffer(data);state=self.lib.LX_begin(len(data))
        decoded=(C.c_uint8*(len(data)+1))();message=(C.c_uint8*64)();tokens=[];errors=[]
        for _ in range(len(data)+3):
            r=self.lib.LX_next(Text(C.addressof(buf),len(data)),StateView(C.pointer(state),1),BytesView(decoded,len(decoded)),BytesView(message,len(message)))
            if r.diagnostic.code.count:
                d=r.diagnostic;errors.append((raw(d.code),raw(d.message),raw(d.fix),(d.loc.line,d.loc.column,d.loc.offset)))
            if r.has_token:
                t=r.token
                self.assertTrue(0<=t.begin<=t.end<=len(data))
                tokens.append((raw(self.lib.LX_name(t.kind)),data[t.begin:t.end],t.integer,t.floating,t.width,bytes(decoded[:t.decoded_count]) if t.decoded else None,(t.loc.line,t.loc.column,t.loc.offset)))
                if t.kind==0:return tokens,errors
        self.fail('lexer did not reach EOF within its progress bound')

    def test_locations_and_decoded_binary_strings(self):
        tokens,errors=self.tokens(b'//x\n#test @tag "a\\0\\n" 0x10 1.25f32')
        self.assertEqual(errors,[])
        self.assertEqual([t[0] for t in tokens],[b'directive',b'annotation',b'string',b'integer',b'float',b'end of file'])
        self.assertEqual(tokens[0][1],b'test');self.assertEqual(tokens[0][-1],(2,1,4))
        self.assertEqual(tokens[2][5],b'a\0\n');self.assertEqual(tokens[3][2],16)
        self.assertEqual(tokens[4][3:5],(float_bits(1.25),32))

    def test_type_parameter_sigil(self):
        tokens,errors=self.tokens(b'(a:[] $T) $ $9')
        self.assertEqual([t[0] for t in tokens],[b"'('",b'identifier',b"':'",b"'['",b"']'",b"'$' name",b"')'",b'integer',b'end of file'])
        self.assertEqual(tokens[5][1],b'T');self.assertEqual(tokens[5][-1],(1,7,6))
        self.assertEqual([(e[0],e[3]) for e in errors],[(b'E0108',(1,11,10)),(b'E0108',(1,13,12))])

    def test_recovery_and_longest_match(self):
        tokens,errors=self.tokens(b'/*/**/*/ ?A 2..<3 "a\\qb"')
        self.assertEqual([e[0] for e in errors],[b'E0108',b'E0105'])
        self.assertEqual([t[0] for t in tokens],[b'identifier',b'integer',b"'..<'",b'integer',b'string',b'end of file'])
        self.assertEqual(tokens[0][1],b'A');self.assertEqual(tokens[-2][5],b'ab')
        tokens,errors=self.tokens(b'"bad\nnext')
        self.assertEqual(errors[0][0],b'E0106');self.assertEqual(tokens[1][1],b'next');self.assertEqual(tokens[1][-1],(2,1,5))

    def test_independent_decimal_oracle(self):
        rng=random.Random(29)
        for _ in range(10000):
            digits=''.join(str(rng.randrange(10)) for _ in range(rng.randrange(1,100)))
            at=rng.randrange(len(digits)+1);text=digits[:at]+'.'+digits[at:]+'e'+str(rng.randrange(-400,400))
            self.assertEqual(self.number(text),float_bits(float(text)),text)
        for text in ('1_2.3_4e+2','1e999999999999999999999','0e999999999999999999999','1e-999999999999999999999'):
            self.assertEqual(self.number(text),float_bits(float(text.replace('_',''))),text)

    def test_exact_midpoints_and_more_than_800_significant_digits(self):
        cases=[0,1,2,3,0x000fffffffffffff,0x0010000000000000,0x3fefffffffffffff,0x3ff0000000000000,0x7feffffffffffffe]
        with localcontext() as context:
            context.prec=1800
            for bits in cases:
                a=struct.unpack('>d',struct.pack('>Q',bits))[0]
                b=struct.unpack('>d',struct.pack('>Q',bits+1))[0]
                midpoint=(Decimal.from_float(a)+Decimal.from_float(b))/2
                delta=Decimal(10)**(midpoint.adjusted()-1000)
                for value in (midpoint-delta,midpoint,midpoint+delta):
                    text=format(value,'e')
                    self.assertEqual(self.number(text),float_bits(float(text)),text)
            # Overflow has a finite rounding boundary even though its upper
            # binary64 endpoint is infinity. Construct that boundary exactly.
            maximum=Decimal.from_float(float.fromhex('0x1.fffffffffffffp1023'))
            midpoint=(maximum+Decimal(2)**1024)/2
            delta=Decimal(10)**(midpoint.adjusted()-1000)
            for value in (midpoint-delta,midpoint,midpoint+delta):
                text=format(value,'e')
                self.assertEqual(self.number(text),float_bits(float(text)),text)

    def test_native_lexer_under_sanitizers(self):
        host=self.d/'fuzz.c'
        host.write_text('''#include "lexer.h"
#include <assert.h>
#include <stdint.h>
static void inspect(uint8_t *input,int n){
 uint8_t decoded[258],message[64];coax_t_LX_State state=LX_begin(n);
 for(int i=0;i<n+3;i++){
  coax_t_LX_Item item=LX_next((coax_string){(char*)input,n},(coax_v_t_LX_State){&state,1},(coax_v_u8){decoded,n+1},(coax_v_u8){message,64});
  if(item.has_token){assert(item.token.kind<=COAX_LX_Kind_k_dollar);assert(item.token.text_begin>=0&&item.token.text_end>=item.token.text_begin&&item.token.text_end<=n);assert(item.token.decoded_count<=n);
   if(item.token.kind==COAX_LX_Kind_k_eof)return;}
  else assert(item.diagnostic.code.count>0);
 }assert(0&&"lexer failed to make progress");
}
int main(void){uint8_t input[257];uint32_t r=49;
 for(int i=0;i<65536;i++){input[0]=(uint8_t)(i>>8);input[1]=(uint8_t)i;inspect(input,2);}
 for(int i=0;i<10000;i++){int n=i%257;for(int j=0;j<n;j++){r=r*1664525+1013904223;input[j]=(uint8_t)(r>>24);}inspect(input,n);}
 return 0;}
''')
        exe=self.d/'san'
        checked([os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',self.source,host,'-lm','-o',exe])
        checked([exe],env=dict(os.environ,ASAN_OPTIONS='detect_leaks=0',UBSAN_OPTIONS='halt_on_error=1'))

if __name__=='__main__':unittest.main()

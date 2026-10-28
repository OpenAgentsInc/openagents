#!/usr/bin/env python3
"""Actual Coax parser: independent tree assertions and native storage/stack fuzz.
Python is test tooling only; neither bootstrap nor app generation invokes it.
"""
import ctypes as C
import os
from pathlib import Path
import random
import struct
import tempfile
import unittest
import lexer as LX

ROOT=Path(__file__).resolve().parents[1]
class Loc(C.Structure):_fields_=[(n,C.c_int64) for n in ('line','column','offset')]
class Token(C.Structure):_fields_=[('kind',C.c_uint32),('text',LX.Text),('loc',Loc),('value',C.c_uint64),('floating',C.c_double),('width',C.c_int64),('raw_count',C.c_int64)]
class Node(C.Structure):
    _fields_=[('kind',C.c_uint32),('loc',Loc),('name',LX.Text),('name2',LX.Text),('loc2',Loc)]+[(n,C.c_int64) for n in ('a','b','c','d','head','tail','next')]+[('op',C.c_uint32),('flag',C.c_bool),('flag2',C.c_bool),('value',C.c_uint64),('floating',C.c_double),('width',C.c_int64)]
class State(C.Structure):_fields_=[(n,C.c_int64) for n in ('pos','used','depth')]+[('code',LX.Text),('loc',Loc),('fix',LX.Text),('length',C.c_int64)]
class Result(C.Structure):_fields_=[('ok',C.c_bool),('root',C.c_int64)]
def view(T):
    class V(C.Structure):_fields_=[('data',C.POINTER(T)),('count',C.c_int64)]
    return V

class Parser(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        LX.Lexer.setUpClass()
        cls.tmp=tempfile.TemporaryDirectory(prefix='coax-native-parser-');cls.d=Path(cls.tmp.name)
        cls.source=cls.d/'parser.c';cls.header=cls.d/'parser.h'
        cls.source.write_bytes(LX.checked([ROOT/'bin/coax','c',ROOT/'lib/compiler/parser.coax','--library','--header',cls.header]))
        LX.checked([os.environ.get('CC','cc'),'-std=c11','-O1','-g','-shared','-fPIC',cls.source,'-lm','-o',cls.d/'parser.dylib'])
        cls.lib=C.CDLL(str(cls.d/'parser.dylib'))
        cls.lib.PS_parse.argtypes=[LX.Text,view(Token),view(Node),view(State),view(C.c_uint8)]
        cls.lib.PS_parse.restype=Result
    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup();LX.Lexer.tearDownClass()
    def parse(self,source,capacity=None):
        data=source.encode();buf=C.create_string_buffer(data);state=LX.Lexer.lib.LX_begin(len(data))
        decoded=(C.c_uint8*(len(data)+1))();message=(C.c_uint8*64)();tokens=[];keep=[buf]
        for _ in range(len(data)+3):
            r=LX.Lexer.lib.LX_next(LX.Text(C.addressof(buf),len(data)),LX.StateView(C.pointer(state),1),LX.BytesView(decoded,len(decoded)),LX.BytesView(message,64))
            self.assertEqual(r.diagnostic.code.count,0,source)
            if r.has_token:
                t=r.token;text=bytes(decoded[:t.decoded_count]) if t.decoded else data[t.begin:t.end]
                owned=C.create_string_buffer(text);keep.append(owned)
                tokens.append(Token(t.kind,LX.Text(C.addressof(owned),len(text)),Loc(t.loc.line,t.loc.column,t.loc.offset),t.integer,struct.unpack('d',struct.pack('Q',t.floating))[0],t.width,t.end-t.begin))
                if t.kind==0:break
        tokens=(Token*len(tokens))(*tokens);nodes=(Node*(capacity if capacity is not None else len(tokens)*3+1))();s=(State*1)();out=(C.c_uint8*(len(data)+512))()
        args=self.lib.PS_parse.argtypes
        r=self.lib.PS_parse(LX.Text(C.addressof(buf),len(data)),args[1](tokens,len(tokens)),args[2](nodes,len(nodes)),args[3](s,1),args[4](out,len(out)))
        # Retain all input owners for strings/views returned in the tree/state.
        return r,nodes,s[0],bytes(out[:s[0].length]),keep
    def test_precedence_and_associativity(self):
        r,n,s,_,keep=self.parse('x :: a + b * c - d;')
        self.assertTrue(r.ok);e=n[n[n[r.root].head].a]
        self.assertEqual((e.kind,e.op),(23,35))
        self.assertEqual((n[e.a].kind,n[e.a].op),(23,34))
        self.assertEqual((n[n[e.a].b].kind,n[n[e.a].b].op),(23,36))
        r,n,s,msg,keep=self.parse('x :: a < b < c;')
        self.assertFalse(r.ok);self.assertEqual(LX.raw(s.code),b'E0203')
        self.assertEqual((s.loc.line,s.loc.column,s.loc.offset),(1,12,11))
        self.assertEqual(msg,b'comparisons cannot be chained')
    def test_random_parenthesized_expression_trees(self):
        rng=random.Random(197)
        ops=[('+',34),('-',35),('*',36),('/',37),('%',38),('&',39),('|',40),('^',41),('<<',42),('>>',43)]
        def expression(depth):
            if depth==0 or rng.random()<0.3:
                v=rng.randrange(100);return str(v),v
            op,k=rng.choice(ops);a,x=expression(depth-1);b,y=expression(depth-1)
            return '('+a+op+b+')',(k,x,y)
        def tree(n,i):
            v=n[i]
            if v.kind==16:return v.value
            self.assertEqual(v.kind,23)
            return v.op,tree(n,v.a),tree(n,v.b)
        for _ in range(200):
            source,expected=expression(5);r,n,s,msg,keep=self.parse('x :: '+source+';')
            self.assertTrue(r.ok,msg);self.assertEqual(tree(n,n[n[r.root].head].a),expected)
    def test_admission_and_names(self):
        r,n,s,msg,keep=self.parse('#authority; #forall x:u8, y:0..<3 \n { #assert true; }')
        self.assertTrue(r.ok,msg);self.assertTrue(n[r.root].flag)
        check=n[n[r.root].head];self.assertEqual(LX.raw(check.name),b'x:u8, y:0..<3')
        self.assertTrue(check.flag);proc=n[check.a];self.assertEqual(LX.raw(proc.name),LX.raw(check.name))
        r,n,s,msg,keep=self.parse('x :: "a\\0b";')
        self.assertTrue(r.ok);self.assertEqual(LX.raw(n[n[n[r.root].head].a].name),b'a\0b')
        r,n,s,msg,keep=self.parse('x :: "a\\0b"')
        self.assertFalse(r.ok);self.assertEqual(s.loc.column,12)
    def test_node_and_nesting_limits(self):
        source='f :: () { x := 1+2; return; }'
        r,n,s,_,keep=self.parse(source);self.assertTrue(r.ok);needed=s.used+1
        for capacity in range(needed):
            r,n,s,msg,keep=self.parse(source,capacity)
            self.assertFalse(r.ok);self.assertEqual(LX.raw(s.code),b'E0221')
        r,n,s,msg,keep=self.parse(source,needed);self.assertTrue(r.ok,msg)
        for source in ('x :: '+'('*1000+'1'+')'*1000+';', 'f :: () {'+'{'*1000+'}'*1000+'}', 'x :: '+'!'*1000+'true;', 'S :: struct {x:'+ '[] '*1000+'u8;}'):
            r,n,s,msg,keep=self.parse(source)
            self.assertFalse(r.ok);self.assertEqual(LX.raw(s.code),b'E0221');self.assertEqual(s.depth,0)
    def test_native_bridge_under_sanitizers(self):
        # The complete compiler links the actual generated compiler modules.
        # This covers pointer/value-list reconstruction as well as the C kernel.
        obj=self.d/'modules.o';exe=self.d/'coax-sanitized'
        flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']
        LX.checked([os.environ.get('CC','cc'),'-std=c11',*flags,'-c',ROOT/'bin/proto_importer.c','-o',obj])
        LX.checked([os.environ.get('CXX','c++'),'-std=c++20',*flags,'-I',ROOT/'bin',ROOT/'src/main.cpp',obj,'-o',exe])
        for source in (ROOT/'lib/compiler/parser.coax',ROOT.parent/'metareal/home.coax',ROOT.parent/'metareal/grid.coax'):
            LX.checked([exe,'check',source],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})
    def test_native_parser_under_sanitizers(self):
        host=self.d/'fuzz.c'
        host.write_text(r'''#include "lexer.h"
#include "parser.h"
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint32_t rng=273;
static uint32_t next(void){rng^=rng<<13;rng^=rng>>17;rng^=rng<<5;return rng;}
static void inspect(uint8_t *input,int count){
 uint8_t *decoded=calloc(count+1,1),*owned=calloc(count+1,1),message[64];int used=0,nt=0;
 coax_t_PS_Token *tokens=calloc(count+1,sizeof(*tokens));coax_t_LX_State lex=LX_begin(count);
 for(int j=0;j<count+3;j++){
  coax_t_LX_Item item=LX_next((coax_string){(char*)input,count},(coax_v_t_LX_State){&lex,1},(coax_v_u8){decoded,count+1},(coax_v_u8){message,64});
  if(item.diagnostic.code.count)goto done;
  if(!item.has_token)continue;coax_t_LX_Token v=item.token;
  coax_t_PS_Token t={.kind=v.kind,.text={(char*)input+v.text_begin,v.text_end-v.text_begin},.loc={v.loc.line,v.loc.column,v.loc.offset},.value=v.int_value,.width=v.float_bits,.raw_count=v.text_end-v.text_begin};
  memcpy(&t.floating,&v.float_value,8);
  if(v.decoded){assert(used+v.decoded_count<=count);memcpy(owned+used,decoded,v.decoded_count);t.text=(coax_string){(char*)owned+used,v.decoded_count};used+=v.decoded_count;}
  tokens[nt++]=t;if(!v.kind)break;
 }
 assert(nt>0&&tokens[nt-1].kind==0);
 coax_t_PS_Caps caps=PS_caps(nt,count);
 coax_t_PS_Import *requests=calloc(caps.imports,sizeof(*requests));
 coax_t_PS_Imports imports=PS_imports((coax_v_t_PS_Token){tokens,nt},(coax_v_t_PS_Import){requests,caps.imports},0);
 if(!imports.code.count)assert(imports.count>=0&&imports.count<=8&&imports.count<=caps.imports);
 free(requests);
 coax_t_PS_Node *nodes=calloc(caps.nodes,sizeof(*nodes));uint8_t *out=calloc(caps.message+1,1);out[caps.message]=0x7d;
 coax_t_PS_State state={0};coax_t_PS_Result r=PS_parse((coax_string){(char*)input,count},(coax_v_t_PS_Token){tokens,nt},(coax_v_t_PS_Node){nodes,caps.nodes},(coax_v_t_PS_State){&state,1},(coax_v_u8){out,caps.message});
 assert(state.depth==0&&state.pos>=0&&state.pos<nt&&state.used<caps.nodes&&out[caps.message]==0x7d);
 if(r.ok){assert(!state.code.count&&nodes[r.root].kind==74);for(int i=1;i<=state.used;i++){coax_t_PS_Node n=nodes[i];assert(n.a>=0&&n.a<=state.used&&n.b>=0&&n.b<=state.used&&n.c>=0&&n.c<=state.used&&n.d>=0&&n.d<=state.used&&n.head>=0&&n.head<=state.used&&n.tail>=0&&n.tail<=state.used&&n.next>=0&&n.next<=state.used);}}
 else assert(state.code.count>0);
 free(nodes);free(out);
 done:free(tokens);free(decoded);free(owned);
}
int main(void){
 uint8_t input[1024];for(int i=0;i<65536;i++){input[0]=i>>8;input[1]=i;inspect(input,2);}
 for(int i=0;i<10000;i++){int n=next()%512;for(int j=0;j<n;j++)input[j]=next();inspect(input,n);}
 const char *program="S :: struct {x:[2] u8;} f :: (x:int)->int {if x>0 {return -x;} else {for i:0..<3 {x+=i;} return cast(s32) x;}} #forall a:u8 {#assert a<256;}";
 int size=strlen(program);for(int i=0;i<size;i++)inspect((uint8_t*)program,i);
 for(int i=0;i<5000;i++){memcpy(input,program,size);input[next()%size]=(uint8_t)next();inspect(input,size);}
 const char *directive="#import_proto \"x.pb\" as X; #import_proto \"y.pb\" as Y;";
 int dn=strlen(directive);for(int i=0;i<=dn;i++)inspect((uint8_t*)directive,i);
 for(int i=0;i<1000;i++){memcpy(input,directive,dn);input[next()%dn]=(uint8_t)next();inspect(input,dn);}
 return 0;
}''')
        LX.checked([os.environ.get('CC','cc'),'-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I',self.d,'-I',LX.Lexer.d,host,self.source,LX.Lexer.source,'-lm','-o',self.d/'fuzz'])
        LX.checked([self.d/'fuzz'],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})

if __name__=='__main__':unittest.main()

#!/usr/bin/env python3
"""Coax semantic checker: independent numeric/typed-tree oracles and native ABI.
Python is test tooling only. The bootstrap and target compilation never use it.
"""
import ctypes as C
import math
import os
from pathlib import Path
import random
import struct
import tempfile
import unittest
import lexer as LX
import parser as PS

ROOT=Path(__file__).resolve().parents[1]
class Loc(C.Structure):_fields_=[(n,C.c_int64) for n in ('line','column','offset')]
class Node(C.Structure):
    _fields_=[('kind',C.c_uint32),('loc',Loc),('name',LX.Text),('name2',LX.Text),('loc2',Loc)]+[(n,C.c_int64) for n in ('a','b','c','d','head','tail','next')]+[('op',C.c_uint32),('flag',C.c_bool),('flag2',C.c_bool),('value',C.c_uint64),('floating',C.c_uint64),('type',C.c_int64),('field_index',C.c_int64),('proc',C.c_int64),('constant',C.c_int64),('negative',C.c_bool),('folded',C.c_bool),('builtin_print',C.c_bool),('byte_intrinsic',C.c_uint32),('intrinsic',C.c_uint32),('mark',C.c_int64),('reached',C.c_bool),('metal_done',C.c_bool),('metal_active',C.c_bool),('skip_authority',C.c_bool),('scope',C.c_int64),('collides',C.c_bool),('clash',C.c_int64)]
class Type(C.Structure):_fields_=[('kind',C.c_uint8),('name',LX.Text),('size',C.c_int64),('alignment',C.c_int64),('bits',C.c_int64),('is_signed',C.c_bool)]+[(n,C.c_int64) for n in ('elem','length','decl','id','scope')]
class Local(C.Structure):_fields_=[('name',LX.Text),('type',C.c_int64),('loc',Loc),('immutable',LX.Text),('local_backing',C.c_bool)]
class Error(C.Structure):_fields_=[('loc',Loc),('code',LX.Text),('message',LX.Text),('fix',LX.Text)]
class State(C.Structure):_fields_=[(n,C.c_int64) for n in ('used','type_count','composite_count','local_count','byte_count','error_count','current','constant_depth','loop_depth','defer_depth','unevaluated')]+[('basic',C.c_bool),('depth',C.c_int64),('fatal',C.c_bool),('metal_depth',C.c_int64),('clashes',C.c_int64)]
class Options(C.Structure):_fields_=[(n,C.c_bool) for n in ('library','metal','authority')]
class Alias(C.Structure):_fields_=[('scope',C.c_int64),('name',LX.Text),('target',C.c_int64)]
class Export(C.Structure):_fields_=[('scope',C.c_int64),('name',LX.Text),('loc',Loc)]
class Context(C.Structure):_fields_=[('nodes',PS.view(Node)),('types',PS.view(Type)),('locals',PS.view(Local)),('errors',PS.view(Error)),('bytes',PS.view(C.c_uint8)),('state',PS.view(State)),('modules',PS.view(LX.Text)),('aliases',PS.view(Alias)),('exports',PS.view(Export))]

class Checker(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        PS.Parser.setUpClass();cls.parser=PS.Parser('test_admission_and_names')
        cls.tmp=tempfile.TemporaryDirectory(prefix='coax-checker-');cls.d=Path(cls.tmp.name)
        cls.source=cls.d/'checker.c';cls.header=cls.d/'checker.h'
        cls.source.write_bytes(LX.checked([ROOT/'bin/coax','c',ROOT/'lib/compiler/checker.coax','--library','--header',cls.header]))
        LX.checked([os.environ.get('CC','cc'),'-std=c11','-O1','-shared','-fPIC',cls.source,'-lm','-o',cls.d/'checker.dylib'])
        cls.lib=C.CDLL(str(cls.d/'checker.dylib'))
        cls.lib.CK_meta_capacity.argtypes=[Context._fields_[0][1],C.c_int64];cls.lib.CK_meta_capacity.restype=C.c_int64
        cls.lib.CK_run.argtypes=[Context,C.c_int64,Options];cls.lib.CK_run.restype=C.c_bool
        cls.lib.CM_float.argtypes=[C.c_uint32,C.c_uint64,C.c_uint64];cls.lib.CM_float.restype=C.c_uint64
        cls.lib.CM_convert.argtypes=[C.c_uint64,C.c_uint8,C.c_uint8];cls.lib.CM_convert.restype=C.c_uint64
    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup();PS.Parser.tearDownClass()
    def check(self,source,metal=False):
        result,parsed,s,msg,keep=self.parser.parse(source);self.assertTrue(result.ok,msg)
        n=s.used+1;nodes=(Node*(n*3+16))();types=(Type*(n*3+16))();locals_=(Local*(n+16))();errors=(Error*(n*8+16))();storage=(C.c_uint8*(len(source)*8+n*512+4096))();state=(State*1)()
        for i in range(n):
            v=parsed[i];x=nodes[i];x.field_index=-1
            for field,T in PS.Node._fields_:
                if field in ('width','end'):continue
                value=getattr(v,field)
                if field in ('loc','loc2'):value=Loc(value.line,value.column,value.offset)
                if field=='floating':value=struct.unpack('Q',struct.pack('d',value))[0]
                setattr(x,field,value)

            if v.kind==17:x.type=6 if v.width==32 else 7 if v.width==64 else 0
            if v.kind==69:x.field_index=0
        extra=self.lib.CK_meta_capacity(Context._fields_[0][1](nodes,len(nodes)),n-1)
        if extra:
            expanded=(Node*(len(nodes)+extra))()
            C.memmove(expanded,nodes,C.sizeof(nodes));nodes=expanded
        arrays=[nodes,types,locals_,errors,storage,state];views=[T for _,T in Context._fields_];ctx=Context(*[V(a,len(a)) for V,a in zip(views,arrays)])
        ok=self.lib.CK_run(ctx,n-1,Options(True,metal,parsed[1].flag))
        return ok,nodes,types,errors,state[0],(keep,parsed,*arrays)
    def test_independent_layout_and_constant_typing(self):
        source='S :: struct {a:u8;b:int;c:[3]u16;} E :: enum s8 {a= -128;b;} f :: ()->u8 {return 2*3+1;}'
        ok,n,t,e,s,owners=self.check(source);self.assertTrue(ok)
        # Natural LP64 layout, enum widths, canonical literal folding.
        self.assertEqual((t[16].size,t[16].alignment,t[17].size,t[17].bits),(24,8,1,8))
        fields=[x for x in n[:s.used+1] if x.kind==69]
        self.assertEqual([x.field_index for x in fields],[0,8,16])
        folded=[x for x in n[:s.used+1] if x.kind==16 and x.type==12 and x.value==7]
        self.assertEqual(len(folded),1)
    def test_independent_policy_errors(self):
        cases=[('f :: () {x:u8=256;}','E0350'),('f :: () {x:=1/0;}','E0407'),
               ('f :: (x:int) {x=2;}','E0344'),('S :: struct {x:S;}','E0353'),
               ('f :: () {x:=missing;}','E0306'),('f :: ()->u8 {}','E0338'),
               ('f :: (x:f32)->f32 {return f(x);}','E0377')]
        for source,code in cases:
            ok,n,t,e,s,owners=self.check(source,code=='E0377')
            self.assertFalse(ok);self.assertIn(code,[LX.raw(x.code).decode() for x in e[:s.error_count]])
            self.assertEqual(s.depth,0)
    def test_local_backed_borrow_returns(self):
        cases=[
            'f :: ()->[] u8 {a:[4]u8;return a[..];}',
            'f :: ()->[] u8 {a:[4]u8;v:=a[..];copy:=v;return copy[1..<3];}',
            'f :: (input:[]u8)->[]u8 {a:[4]u8;v:=input;v=a[..];return v;}',
            'f :: (input:[]u8,b:bool)->[]u8 {a:[4]u8;return ifx b then input else a[..];}',
            'f :: ()->string {a:[4]u8;text:=cast(string)a[..];return text[1..<3];}',
            'S :: struct {a:[4]u8;} f :: ()->[]u8 {s:S;return s.a[..];}',
            'f :: ()->[]u8 {a:[2][4]u8;return a[1][..];}',
            'f :: (input:[]u8,b:bool)->[]u8 {a:[4]u8;v:=input;if b {v=a[..];}return v;}',
            'f :: ()->[]u8 {v:[]u8;a:[4]u8;v=a[..];return v;}',
            'f :: (input:[]u8)->[]u8 {a:[4]u8;v:=a[..];v=input;return v;}',
            'f :: ()->string {text:="";a:[4]u8;text=cast(string)a[..];return text;}',
            'f :: (input:[4]u8)->[]u8 {copy:=input;return copy[..];}',
        ]
        for source in cases:
            with self.subTest(source=source):
                ok,n,t,e,s,owners=self.check(source)
                self.assertFalse(ok)
                self.assertEqual([LX.raw(x.code).decode() for x in e[:s.error_count]],['E0383'])
    def test_borrowed_parameter_returns_remain_valid(self):
        cases=[
            'f :: (input:[]u8)->[]u8 {return input;}',
            'f :: (input:[]u8)->[]u8 {v:=input;copy:=v;return copy[1..<3];}',
            'f :: (input:[]u8)->string {return cast(string)input;}',
            'f :: (text:string)->string {copy:=text;return copy[1..<3];}',
            'f :: (input:[]u8,other:[]u8,b:bool)->[]u8 {return ifx b then input else other;}',
            'S :: struct {a:[4]u8;} f :: (input:[]S)->[]u8 {return input[0].a[..];}',
            'S :: struct {a:[]u8;} f :: (input:[]u8)->[]u8 {s:=S.{a=input};return s.a;}',
            'f :: (input:[]u8)->[]u8 {a:=[1][]u8.[input];return a[0];}',
            'f :: ()->[4]u8 {a:[4]u8;return a;}',
            'f :: ()->string {return "resident";}',
        ]
        for source in cases:
            with self.subTest(source=source):
                ok,n,t,e,s,owners=self.check(source)
                self.assertTrue(ok,[LX.raw(x.message).decode() for x in e[:s.error_count]])
    def test_ieee_arithmetic_and_binary32_against_host_oracle(self):
        rng=random.Random(985)
        unpack=lambda b:struct.unpack('>d',struct.pack('>Q',b))[0]
        bits=lambda x:struct.unpack('>Q',struct.pack('>d',x))[0]
        for _ in range(3000):
            a,b=rng.getrandbits(64),rng.getrandbits(64);x,y=unpack(a),unpack(b)
            if math.isnan(x) or math.isnan(y):continue
            for op in (34,35,36,37):
                if op==37 and y==0:continue
                answer={34:lambda:x+y,35:lambda:x-y,36:lambda:x*y,37:lambda:x/y}[op]()
                got=self.lib.CM_float(op,a,b)
                self.assertTrue((math.isnan(answer) and math.isnan(unpack(got))) or got==bits(answer),(op,hex(a),hex(b),hex(got)))
            try:answer=struct.unpack('>I',struct.pack('>f',x))[0]
            except OverflowError:answer=((a>>63)<<31)|0x7f800000
            self.assertEqual(self.lib.CM_convert(a,64,32),answer)
    def test_actual_kernel_capacity_and_types_under_sanitizers(self):
        host=self.d/'host.c'
        host.write_text(r'''#include "checker.h"
#include <assert.h>
#include <string.h>
int main(void){
 coax_t_CK_Node nodes[64]={0};coax_t_CK_Type types[64]={0};coax_t_CK_Local locals[64]={0};coax_t_CK_Error errors[16]={0};uint8_t bytes[4096]={0};coax_t_CK_State state={0};
 nodes[1].kind=COAX_CK_Tag_file;nodes[1].head=2;
 nodes[2].kind=COAX_CK_Tag_proc;nodes[2].name=(coax_string){"f",1};nodes[2].a=6;nodes[2].b=3;nodes[2].type=1;
 nodes[3].kind=COAX_CK_Tag_stmt_block;nodes[3].head=4;
 nodes[4].kind=COAX_CK_Tag_stmt_return;nodes[4].a=5;
 nodes[5].kind=COAX_CK_Tag_expr_int;nodes[5].value=3;
 nodes[6].kind=COAX_CK_Tag_type_named;nodes[6].name=(coax_string){"u8",2};
 coax_t_CK_Context c={{nodes,64},{types,64},{locals,64},{errors,16},{bytes,4096},{&state,1}};
 assert(CK_run(c,6,(coax_t_CK_Options){.library=1}));assert(nodes[2].type==12 && nodes[5].type==12 && nodes[5].value==3);assert(types[12].size==1 && types[12].bits==8);
 for(int n=0;n<16;n++){c.types.count=n;assert(!CK_run(c,6,(coax_t_CK_Options){.library=1}));}
 c.types.count=64;for(int n=0;n<=6;n++){c.nodes.count=n;assert(!CK_run(c,6,(coax_t_CK_Options){.library=1}));}
 c.nodes.count=64;c.state.count=0;assert(!CK_run(c,6,(coax_t_CK_Options){.library=1}));return 0;
}''')
        exe=self.d/'host';flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']
        LX.checked([os.environ.get('CC','cc'),'-std=c11',*flags,self.source,host,'-lm','-o',exe])
        LX.checked([exe],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})

if __name__=='__main__':unittest.main()

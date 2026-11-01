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
class Unit(C.Structure):_fields_=[(n,C.c_int64) for n in ('mass','length','time','angle','numerator','denominator')]
class Frame(C.Structure):_fields_=[('handedness',C.c_int64),('convention',C.c_uint8),('continuity',C.c_uint8),('min_rep',C.c_int64)]
class Spatial(C.Structure):_fields_=[('kind',C.c_uint8),('frame',C.c_int64),('unit',C.c_int64),('rep',C.c_int64)]
class Node(C.Structure):
    _fields_=[('kind',C.c_uint32),('loc',Loc),('name',LX.Text),('name2',LX.Text),('loc2',Loc)]+[(n,C.c_int64) for n in ('a','b','c','d','head','tail','next')]+[('op',C.c_uint32),('flag',C.c_bool),('flag2',C.c_bool),('value',C.c_uint64),('floating',C.c_uint64),('type',C.c_int64),('field_index',C.c_int64),('proc',C.c_int64),('constant',C.c_int64),('negative',C.c_bool),('folded',C.c_bool),('builtin_print',C.c_bool),('byte_intrinsic',C.c_uint32),('intrinsic',C.c_uint32),('mark',C.c_int64),('reached',C.c_bool),('metal_done',C.c_bool),('metal_active',C.c_bool),('skip_authority',C.c_bool),('scope',C.c_int64),('collides',C.c_bool),('clash',C.c_int64),('effects',C.c_uint8),('own_effects',C.c_uint8),('effects_known',C.c_bool),('generic',C.c_bool),('origin',C.c_int64),('pristine',C.c_int64),('chain',C.c_int64),('key',LX.Text)]
    _fields_ += [('quantity',C.c_uint8),('unit',Unit),('unit_form',C.c_int64),('unit_link',C.c_int64),('unit_generated',C.c_bool),('frame_decl',C.c_bool),('frame',Frame),('spatial_link',C.c_int64),('lowering',C.c_uint8),('mutation_status',C.c_uint8),('mutation_op',C.c_uint32),('mutation_proc',C.c_int64)]
    _fields_ += [(n,C.c_int64) for n in ('borrow_binding','borrow_owner','borrow_writes','borrow_returns','borrow_calls','borrow_next')]+[('borrow_status',C.c_uint8),('borrow_query',C.c_int64)]
    _fields_ += [('retain_head',C.c_int64),('retain_count',C.c_int64),('retain_mark',C.c_uint8),('retain_incomplete',C.c_bool),('retain_relation',C.c_bool)]
class Type(C.Structure):_fields_=[('kind',C.c_uint8),('name',LX.Text),('size',C.c_int64),('alignment',C.c_int64),('bits',C.c_int64),('is_signed',C.c_bool)]+[(n,C.c_int64) for n in ('elem','length','decl','id','scope','need','carrier','unit')]+[('space',Spatial),('retain_presence',C.c_uint8),('retain_mutable',C.c_uint8),('retain_fields',C.c_int64)]
class Local(C.Structure):_fields_=[('name',LX.Text),('type',C.c_int64),('loc',Loc),('immutable',LX.Text),('local_backing',C.c_bool),('decl',C.c_int64)]
class Error(C.Structure):_fields_=[('loc',Loc),('code',LX.Text),('message',LX.Text),('fix',LX.Text)]
class State(C.Structure):_fields_=[(n,C.c_int64) for n in ('used','type_count','composite_count','local_count','byte_count','error_count','current','constant_depth','loop_depth','defer_depth','unevaluated')]+[('basic',C.c_bool),('depth',C.c_int64),('fatal',C.c_bool),('metal_depth',C.c_int64),('clashes',C.c_int64),('generic',C.c_int64),('site',C.c_uint8),('instances',C.c_int64),('instance_tail',C.c_int64),('instance_count',C.c_int64),('quantity_device',C.c_bool),('unit_units',C.c_int64),('spatial_types',C.c_int64),('spatial_helpers',C.c_int64),('retain_report_count',C.c_int64)]
class Options(C.Structure):_fields_=[(n,C.c_bool) for n in ('library','metal','authority','quantity_device')]
class Alias(C.Structure):_fields_=[('scope',C.c_int64),('name',LX.Text),('target',C.c_int64)]
class Export(C.Structure):_fields_=[('scope',C.c_int64),('name',LX.Text),('loc',Loc)]
class BorrowStep(C.Structure):_fields_=[('kind',C.c_uint8),('index',C.c_int64)]
class BorrowSteps(C.Structure):_fields_=[('e',BorrowStep*8)]
class BorrowPath(C.Structure):_fields_=[('count',C.c_int64),('steps',BorrowSteps)]
class BorrowOrigin(C.Structure):_fields_=[('kind',C.c_uint8),('root',C.c_int64),('role',C.c_uint8),('type',C.c_int64),('path',BorrowPath)]
class BorrowQuery(C.Structure):_fields_=[('kind',C.c_uint8),('decl',C.c_int64),('type',C.c_int64),('path',BorrowPath),('status',C.c_uint8),('start',C.c_int64),('count',C.c_int64),('next',C.c_int64),('retain_classes',C.c_bool)]
class BorrowCaps(C.Structure):_fields_=[('queries',C.c_int64),('origins',C.c_int64),('work',C.c_int64)]
class RetainEffect(C.Structure):_fields_=[('source',BorrowOrigin),('destination',BorrowOrigin),('value_type',C.c_int64),('witness',C.c_int64),('next',C.c_int64),('source_storage',C.c_bool)]
class RetainObservation(C.Structure):_fields_=[('kind',C.c_uint8),('owner',C.c_int64),('site',C.c_int64),('facts_complete',C.c_bool),('relation_complete',C.c_bool),('forbidden',C.c_bool),('pending',C.c_int64)]
class RetainCaps(C.Structure):_fields_=[('effects',C.c_int64),('reports',C.c_int64),('work',C.c_int64)]
class Context(C.Structure):_fields_=[('nodes',PS.view(Node)),('types',PS.view(Type)),('locals',PS.view(Local)),('errors',PS.view(Error)),('bytes',PS.view(C.c_uint8)),('state',PS.view(State)),('modules',PS.view(LX.Text)),('aliases',PS.view(Alias)),('exports',PS.view(Export)),('borrow_queries',PS.view(BorrowQuery)),('borrow_origins',PS.view(BorrowOrigin)),('retain_effects',PS.view(RetainEffect)),('retain_reports',PS.view(RetainObservation))]

def transport_records():
    return [('coax_t_CK_Loc',Loc),('coax_t_Q_Unit',Unit),('coax_t_SP_Frame',Frame),('coax_t_SP_Type',Spatial),('coax_t_CK_Node',Node),('coax_t_CK_Type',Type),('coax_t_CK_Local',Local),('coax_t_CK_Error',Error),('coax_t_CK_State',State),('coax_t_CK_Options',Options),('coax_t_CK_Alias',Alias),('coax_t_CK_Export',Export),('coax_t_BS_Step',BorrowStep),('coax_t_BS_Path',BorrowPath),('coax_t_BS_Origin',BorrowOrigin),('coax_t_CK_BorrowQuery',BorrowQuery),('coax_t_CK_BorrowCaps',BorrowCaps),('coax_t_CK_RetainEffect',RetainEffect),('coax_t_CK_RetainObservation',RetainObservation),('coax_t_CK_RetainCaps',RetainCaps),('coax_t_CK_Context',Context)]

class TransportABI(unittest.TestCase):
    def test_generated_seed_headers_match_ctypes(self):
        # Header-only probes are safe even when the Python graph stride is stale.
        # Collect both failures rather than reaching either kernel with bad memory.
        for header,records in [('parser',PS.transport_records()),('checker',transport_records())]:
            with self.subTest(header=header):
                PS.assert_native_layouts(ROOT/'bootstrap'/f'{header}.h',records)

class Checker(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        PS.Parser.setUpClass();cls.parser=PS.Parser('test_admission_and_names')
        cls.tmp=tempfile.TemporaryDirectory(prefix='coax-checker-');cls.d=Path(cls.tmp.name)
        cls.source=cls.d/'checker.c';cls.header=cls.d/'checker.h'
        cls.source.write_bytes(LX.checked([ROOT/'bin/coax','c',ROOT/'lib/compiler/checker.coax','--library','--header',cls.header]))
        PS.assert_native_layouts(cls.header,transport_records())
        LX.checked([os.environ.get('CC','cc'),'-std=c11','-O1','-shared','-fPIC',cls.source,'-lm','-o',cls.d/'checker.dylib'])
        cls.lib=C.CDLL(str(cls.d/'checker.dylib'))
        cls.lib.CK_meta_capacity.argtypes=[Context._fields_[0][1],C.c_int64];cls.lib.CK_meta_capacity.restype=C.c_int64
        cls.lib.CK_run.argtypes=[Context,C.c_int64,Options];cls.lib.CK_run.restype=C.c_bool
        cls.lib.CK_borrow_caps.argtypes=[C.c_int64];cls.lib.CK_borrow_caps.restype=BorrowCaps
        cls.lib.CK_retain_caps.argtypes=[C.c_int64];cls.lib.CK_retain_caps.restype=RetainCaps
        cls.lib.CM_float.argtypes=[C.c_uint32,C.c_uint64,C.c_uint64];cls.lib.CM_float.restype=C.c_uint64
        cls.lib.CM_convert.argtypes=[C.c_uint64,C.c_uint8,C.c_uint8];cls.lib.CM_convert.restype=C.c_uint64
    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup();PS.Parser.tearDownClass()
    def check(self,source,metal=False,quantity_device=False):
        result,parsed,s,msg,keep=self.parser.parse(source);self.assertTrue(result.ok,msg)
        n=s.used+1;nodes=(Node*(n*3+16))();types=(Type*(n*3+16))();locals_=(Local*(n+16))();errors=(Error*(n*8+16))();storage=(C.c_uint8*(len(source)*8+n*512+4096))();state=(State*1)()
        for i in range(n):
            v=parsed[i];x=nodes[i];x.field_index=-1
            for field,T in PS.Node._fields_:
                if field in ('width','end'):continue
                value=getattr(v,field)
                if field in ('loc','loc2'):value=Loc(value.line,value.column,value.offset)
                if field=='floating':value=struct.unpack('Q',struct.pack('d',value))[0]
                if field=='unit':value=Unit(*value.e)
                if field=='frame':value=Frame(*value.e)
                setattr(x,field,value)

            if v.kind==17:x.type=6 if v.width==32 else 7 if v.width==64 else 0
            if v.kind==69:x.field_index=0
        extra=self.lib.CK_meta_capacity(Context._fields_[0][1](nodes,len(nodes)),n-1)
        if extra:
            expanded=(Node*(len(nodes)+extra))()
            C.memmove(expanded,nodes,C.sizeof(nodes));nodes=expanded
        arrays=[nodes,types,locals_,errors,storage,state];views=[T for _,T in Context._fields_];ctx=Context(*[V(a,len(a)) for V,a in zip(views,arrays)])
        borrow_caps=self.lib.CK_borrow_caps(n)
        queries=(BorrowQuery*borrow_caps.queries)();origins=(BorrowOrigin*borrow_caps.origins)()
        context_views=dict(Context._fields_)
        ctx.borrow_queries=context_views['borrow_queries'](queries,len(queries));ctx.borrow_origins=context_views['borrow_origins'](origins,len(origins))
        retain_caps=self.lib.CK_retain_caps(n);effects=(RetainEffect*retain_caps.effects)()
        ctx.retain_effects=context_views['retain_effects'](effects,len(effects));ctx.retain_reports=context_views['retain_reports']()
        ok=self.lib.CK_run(ctx,n-1,Options(True,metal,parsed[1].flag,quantity_device))
        return ok,nodes,types,errors,state[0],(keep,parsed,*arrays,queries,origins,effects)
    def test_independent_layout_and_constant_typing(self):
        source='S :: struct {a:u8;b:int;c:[3]u16;} E :: enum s8 {a= -128;b;} f :: ()->u8 {return 2*3+1;}'
        ok,n,t,e,s,owners=self.check(source);self.assertTrue(ok)
        # Natural LP64 layout, enum widths, canonical literal folding.
        self.assertEqual((t[16].size,t[16].alignment,t[17].size,t[17].bits),(24,8,1,8))
        fields=[x for x in n[:s.used+1] if x.kind==69]
        self.assertEqual([x.field_index for x in fields],[0,8,16])
        folded=[x for x in n[:s.used+1] if x.kind==16 and x.type==12 and x.value==7]
        self.assertEqual(len(folded),1)
    def test_quantity_graph_transport_and_device_option(self):
        source='Speed :: struct @unit(mass=0,length=1,time=-1,angle=0,numerator=3,denominator=2) {} Quantity :: struct($U:copyable,$R:numeric) @builtin_quantity {value:$R;} f :: ()->f64 {a:Quantity(Speed,f64)=Quantity.{value=4};return a.value;}'
        ok,nodes,types,errors,state,owners=self.check(source)
        self.assertTrue(ok,[LX.raw(e.message) for e in errors[:state.error_count]])
        unit=next(n for n in nodes[:state.used+1] if n.kind==66 and LX.raw(n.name)==b'Speed')
        self.assertEqual(unit.quantity,1)
        self.assertEqual([getattr(unit.unit,field) for field,_ in Unit._fields_],[0,1,-1,0,3,2])
        quantities=[t for t in types[:state.type_count] if t.carrier]
        self.assertTrue(quantities)
        self.assertTrue(all(t.carrier==7 and t.unit==unit.type for t in quantities))
        self.assertFalse(state.quantity_device)
        ok,nodes,types,errors,state,owners=self.check(source,quantity_device=True)
        self.assertFalse(ok)
        self.assertTrue(state.quantity_device)
        self.assertIn('E0828',[LX.raw(e.code).decode() for e in errors[:state.error_count]])
    def test_spatial_graph_transport_and_device_option(self):
        source='World :: struct @frame(handedness=right,convention=cartesian,continuity=continuous,min_precision=f64) {} Metre :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=1,denominator=1) {} f :: (p:Point3(World,Metre,f64),v:Vector3(World,Metre,f64))->Point3(World,Metre,f64) {return p+v;}'
        ok,nodes,types,errors,state,owners=self.check(source)
        self.assertTrue(ok,[LX.raw(e.message) for e in errors[:state.error_count]])
        frame=next(n for n in nodes[:state.used+1] if n.kind==66 and LX.raw(n.name)==b'World')
        self.assertTrue(frame.frame_decl)
        self.assertEqual(frame.quantity,0)
        self.assertEqual((frame.frame.handedness,frame.frame.convention,frame.frame.continuity,frame.frame.min_rep),(1,1,1,64))
        spaces=[t for t in types[:state.type_count] if t.space.kind]
        self.assertEqual(sorted((t.space.kind,t.space.rep) for t in spaces),[(1,64),(2,64)])
        self.assertTrue(all(t.space.frame==frame.type and t.carrier==0 and t.size==24 and t.alignment==8 for t in spaces))
        self.assertEqual(len({t.space.unit for t in spaces}),1)
        calls=[n for n in nodes[:state.used+1] if n.kind==21 and n.mark==2 and n.proc]
        self.assertEqual(len(calls),1)
        self.assertFalse(state.quantity_device)
        ok,nodes,types,errors,state,owners=self.check(source,quantity_device=True)
        self.assertFalse(ok)
        self.assertTrue(state.quantity_device)
        self.assertIn('E0836',[LX.raw(e.code).decode() for e in errors[:state.error_count]])
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
    def test_return_borrow_completeness_is_observed_not_assumed(self):
        fixtures=ROOT/'tests/borrow-summaries'
        for filename,names in [('manual/recursive-only.coax',('recursive','through')),('manual/helper-local-state.coax',('through',)),('manual/helper-alias-local-state.coax',('through',)),('manual/call-projected-local-write.coax',('through',)),('manual/record-held-helper-state.coax',('through',))]:
            with self.subTest(file=filename):
                ok,n,t,e,s,owners=self.check((fixtures/filename).read_text())
                self.assertTrue(ok,[LX.raw(x.message).decode() for x in e[:s.error_count]])
                procedures={LX.raw(x.name).decode():x for x in n[:s.used+1] if x.kind==64}
                for name in names:self.assertEqual(procedures[name].borrow_status,2,name+' remains explicitly manual')
        source='select :: (out:[]u8,ignored:[]u8)->[]u8 {return out;} through :: (out:[]u8)->[]u8 {local:[4]u8;return select(out,local[..]);}'
        ok,n,t,e,s,owners=self.check(source);self.assertTrue(ok,[LX.raw(x.message).decode() for x in e[:s.error_count]])
        self.assertEqual({LX.raw(x.name).decode():x.borrow_status for x in n[:s.used+1] if x.kind==64},{'select':1,'through':1})
        ok,n,t,e,s,owners=self.check((fixtures/'negative/local-and-unknown-call.coax').read_text())
        self.assertFalse(ok)
        self.assertEqual([(LX.raw(x.code).decode(),x.loc.line,x.loc.column) for x in e[:s.error_count]],[('E0383',5,10)])
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
#include <stdlib.h>
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
 c.nodes.count=64;memset(nodes,0,sizeof(nodes));memset(types,0,sizeof(types));memset(locals,0,sizeof(locals));memset(errors,0,sizeof(errors));memset(&state,0,sizeof(state));
 nodes[1].kind=COAX_CK_Tag_file;nodes[1].head=2;
 nodes[2].kind=COAX_CK_Tag_proc;nodes[2].name=(coax_string){"identity",8};nodes[2].head=3;nodes[2].a=4;nodes[2].b=6;
 nodes[3].kind=COAX_CK_Tag_param;nodes[3].name=(coax_string){"input",5};nodes[3].a=4;
 nodes[4].kind=COAX_CK_Tag_type_view;nodes[4].b=5;nodes[5].kind=COAX_CK_Tag_type_named;nodes[5].name=(coax_string){"u8",2};
 nodes[6].kind=COAX_CK_Tag_stmt_block;nodes[6].head=7;nodes[7].kind=COAX_CK_Tag_stmt_return;nodes[7].a=8;
 nodes[8].kind=COAX_CK_Tag_expr_ident;nodes[8].name=(coax_string){"input",5};
 assert(CK_run(c,8,(coax_t_CK_Options){.library=1}) && nodes[2].borrow_status==1);
 coax_t_CK_BorrowCaps b=CK_borrow_caps(9);assert(b.queries>0 && b.origins>0 && b.work>0);
 coax_t_CK_BorrowQuery *queries=calloc((size_t)b.queries,sizeof(*queries));coax_t_BS_Origin *origins=calloc((size_t)b.origins,sizeof(*origins));assert(queries && origins);
 c.borrow_queries=(coax_v_t_CK_BorrowQuery){queries,b.queries};c.borrow_origins=(coax_v_t_BS_Origin){origins,b.origins};
 assert(CK_run(c,8,(coax_t_CK_Options){.library=1}) && nodes[2].borrow_status==1);
 for(int mode=0;mode<3;mode++){
  c.borrow_queries.count=mode==0?0:mode==2?-1:b.queries;c.borrow_origins.count=mode==1?0:b.origins;
  assert(!CK_run(c,8,(coax_t_CK_Options){.library=1}));assert(state.error_count>0 && errors[0].code.count==5 && !memcmp(errors[0].code.data,"E0381",5));
 }
 free(queries);free(origins);c.borrow_queries=(coax_v_t_CK_BorrowQuery){0};c.borrow_origins=(coax_v_t_BS_Origin){0};
 c.state.count=0;assert(!CK_run(c,8,(coax_t_CK_Options){.library=1}));return 0;
}''')
        exe=self.d/'host';flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer']
        LX.checked([os.environ.get('CC','cc'),'-std=c11',*flags,self.source,host,'-lm','-o',exe])
        LX.checked([exe],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'})

if __name__=='__main__':unittest.main()

#!/usr/bin/env python3
"""Actual aggregate source, alias and bounded-control audit integration."""
import ctypes as C
import json
import hashlib
import os
from pathlib import Path
import random
import tempfile
import unittest
from aggregate import AggregateAudit
from check import ROOT,COMPILER,run,metadata_sha

class AggregateSource(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-aggregate-cases-');cls.directory=Path(cls.temp.name);cls.tool=AggregateAudit();cls.observations=[]
    @classmethod
    def tearDownClass(cls):
        evidence={'schema':'coax.aggregate-source-cases.v1','scope':'source dataflow; guard/object/JIT/production gates remain open','test_driver_sha256':metadata_sha(__file__),'cases':cls.observations}
        reports=[item['report'] for item in cls.observations if 'report' in item]
        common=['compiler','compiler_sha256','audit_artifact_sha256','audit_input_sha256','classification','profile','object_timing_approved','checked_runtime_guards_proved','aggregate_memory_loops_supported','production_crypto_approved']
        evidence['audit']={key:reports[0][key] for key in common}
        evidence['typed_inventories']={}
        for report in reports:
            if 'resolved_type_inventory' in report:evidence['typed_inventories'][report['resolved_type_sha256']]=report.pop('resolved_type_inventory')
            for key in common:report.pop(key,None)
        def normalize(value):
            if isinstance(value,str):return value.replace(str(ROOT.resolve()),'$REPO').replace(str(cls.directory.resolve()),'$CASES')
            if isinstance(value,list):return [normalize(item) for item in value]
            if isinstance(value,dict):return {normalize(key):normalize(item) for key,item in value.items()}
            return value
        Path(os.environ.get('COAX_AGGREGATE_REPORT','/tmp/coax-aggregate-source-tests.json')).write_text(json.dumps(normalize(evidence),indent=2)+'\n');cls.tool.close();cls.temp.cleanup()
    def case(self,name,text,error):
        p=self.directory/(name+'.coax');p.write_text(text);r=self.tool.audit(p,'candidate');self.assertEqual(r['error'],error,(name,r));self.observations.append({'case':name,'expected':error,'report':r});return r
    def test_aggregate_secret_flow_and_alias_negatives(self):
        pair='Pair :: struct {words:[2]u64;} '
        fixtures=[
         ('typed_aggregate_binary',pair+'candidate :: (x:Pair)->Pair {return x+x;}',1),
         ('typed_aggregate_cast',pair+'candidate :: (x:Pair)->u64 {return cast(u64)x;}',1),
         ('typed_aggregate_cast_target',pair+'candidate :: (x:u64)->Pair {return cast(Pair)x;}',1),
         ('typed_intrinsic_aggregate',pair+'candidate :: (x:Pair)->Pair {return value_barrier(x);}',1),
         ('public_unknown_ifx_laundering','FLAGS :: [1]bool.[true]; candidate :: (x:u64)->u64 {r:=ifx FLAGS[0] then x else 0;return ifx r==0 then 1 else 2;}',6),
         ('field_branch',pair+'candidate :: (a:Pair)->u64 {return ifx a.words[0]==0 then 1 else 2;}',6),
         ('array_index',pair+'candidate :: (a:Pair,i:u64)->u64 {return a.words[cast(int)i];}',7),
         ('loop_bound',pair+'candidate :: (a:Pair)->u64 {r:u64;for i:0..<cast(int)a.words[0] {r+=1;}return r;}',6),
         ('copy_laundering',pair+'candidate :: (x:u64)->u64 {a:Pair;a.words[0]=x;b:=a;b.words[1]=0;return ifx b.words[0]==0 then 1 else 2;}',6),
         ('view_write','write :: (v:[]u64,x:u64)->bool {v[0]=x;return true;} candidate :: (x:u64)->u64 {a:[2]u64;v:=a[..];ok:=write(v,x);return ifx a[0]==0 then 1 else 2;}',6),
         ('whole_array_alias','candidate :: (x:u64)->u64 {a:[2]u64;v:[]u64=a[..];b:=[2]u64.[x,0];a=b;return ifx v[0]==0 then 1 else 2;}',6),
         ('loop_view_backedge','write :: (v:[]u64,x:u64)->bool {v[0]=x;return true;} candidate :: (x:u64)->u64 {a:[2]u64;v:=a[..];for i:0..<2 {if a[0]==0 {}ok:=write(v,x);}return x;}',6),
         ('public_loop_hull_laundering','candidate :: (x:u64)->u64 {for i:0..<2 {if i==0 {}else {if x==0 {}}}return x;}',5),
         ('loop_scalar_backedge','candidate :: (x:u64)->u64 {a:u64;for i:0..<2 {if a==0 {}a=x;}return a;}',6),
         ('foreign','effect :: (x:u64)->u64 #foreign; candidate :: (x:u64)->u64 {return effect(x);}',9),
         ('vartime','read_vartime :: (x:u64)->u64 {return x;} candidate :: (x:u64)->u64 {return read_vartime(x);}',8),
         ('public_only','decode_public :: (x:u64)->u64 {return x;} candidate :: (x:u64)->u64 {return decode_public(1);}',8),
         ('secret_induction_write','candidate :: (x:u64)->u64 {for i:0..<2 {i=cast(int)x;}return x;}',1),
         ('public_induction_write','candidate :: (x:u64)->u64 {for i:0..<2 {i=0;}return x;}',1),
         ('secret_while','candidate :: (x:u64)->u64 {while x>0 {}return x;}',6),
         ('public_while','candidate :: (x:u64)->u64 {while false {}return x;}',5),
         ('unrepresented_inferred_view','candidate :: (x:u64)->u64 {a:[2]u64;v:=a[..];return x;}',10),
         ('view_rebind','candidate :: (x:u64)->u64 {a:[2]u64;b:[2]u64;v:[]u64=a[..];v=b[..];return x;}',5),
         ('signed_secret','candidate :: (x:int)->int {return x;}',10),
         ('inline_view_container','Box :: struct {v:[]u64;number:u64;} candidate :: (x:u64)->u64 {return Box.{number=x}.number;}',10),
         ('view_field','Box :: struct {v:[]u64;} candidate :: (x:Box)->u64 {return 0;}',10),
         ('truncated_public_branch','candidate :: (x:u64)->u64 {t:=cast,trunc(u8)256;if t==0 {return ifx x==0 then 1 else 2;}return x;}',5),
        ]
        for name,text,error in fixtures:self.case(name,text,error)
    def test_assertions_are_explicit_unresolved_source_hashed_obligations(self):
        r=self.case('secret_assert','candidate :: (x:u64)->u64 {#assert x!=0;return x;}',14)
        self.assertFalse(r['accepted']);self.assertTrue(r['supported_algorithm_dataflow']);self.assertEqual(r['source_status'],'analyzed-with-guard-obligations')
        self.assertFalse(r['checked_runtime_guards_proved']);self.assertFalse(r['production_crypto_approved']);self.assertEqual(len(r['authored_guard_obligations']),1)
        guard=r['authored_guard_obligations'][0];self.assertEqual(guard['classification'],'secret-derived');self.assertEqual(guard['source_sha256'],r['source_sha256'][guard['file']])
        self.case('known_public_assert','candidate :: (x:u64)->u64 {#assert 1<2;return x;}',0)
        self.case('false_public_assert','candidate :: (x:u64)->u64 {#assert false;return x;}',5)
    def test_actual_crypto_closures_and_public_only_exclusion(self):
        for file,entry,error in [('secp_scalar.coax','sc_mul',0),('secp_field.coax','sf_multiply',14),('secp_field.coax','sf_inverse',14),('secp_group.coax','sg_choose',0),('secp_field.coax','sf_inverse_vartime',8),('secp_group.coax','sg_decode_public',8)]:
            r=self.tool.audit(ROOT/'coax/lib/crypto'/file,entry);self.assertEqual(r['error'],error,(entry,r));self.assertFalse(r['production_crypto_approved']);self.assertTrue(r['source_sha256']);canonical=json.dumps(r['resolved_type_inventory'],sort_keys=True,separators=(',',':')).encode();self.assertEqual(r['resolved_type_sha256'],hashlib.sha256(canonical).hexdigest());self.observations.append({'case':entry,'report':r})
    def test_masked_aggregate_executes_checked_and_optimized(self):
        text='''Pair :: struct {words:[2]u64;}
pick :: (mask:u64,a:Pair,b:Pair)->Pair {r:Pair;for i:0..<2 {r.words[i]=b.words[i]^((a.words[i]^b.words[i])&value_barrier(mask));}return r;}
candidate :: (mask:u64,a:u64,b:u64)->u64 #export_c {x:Pair;y:Pair;x.words[0]=a;y.words[0]=b;return pick(mask,x,y).words[0];}'''
        r=self.case('execute',text,0);p=self.directory/'execute.coax';rng=random.Random(8091)
        for mode,flags in [('checked',[]),('release',['--release'])]:
            c=self.directory/(mode+'.c');c.write_text(run(COMPILER,'c',p,'--library',*flags).stdout);artifact=self.directory/(mode+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',artifact)
            lib=C.CDLL(str(artifact));lib.candidate.argtypes=[C.c_uint64]*3;lib.candidate.restype=C.c_uint64
            for _ in range(2000):
                mask,a,b=[rng.getrandbits(64) for _ in range(3)];self.assertEqual(lib.candidate(mask,a,b),b^((a^b)&mask))
        self.observations.append({'case':'native_aggregate_mask_oracle','checked_cases':2000,'optimized_cases':2000,'seed':8091})
    def test_alias_write_positive_control_and_real_bridge_sanitizers(self):
        p=self.directory/'lost-alias.coax';p.write_text('candidate :: (x:u64)->u64 {a:[2]u64;v:[]u64=a[..];b:=[2]u64.[x,0];a=b;return ifx v[0]==0 then 1 else 2;}')
        self.assertEqual(self.tool.audit(p,'candidate')['error'],6)
        original=(self.tool.directory/'policy.c').read_text();lines=[line for line in original.splitlines() if '.taint' in line and 'l_target).region' in line and 'l_mask' in line]
        self.assertEqual(len(lines),1);mutant=self.directory/'mutant-policy.c';mutant.write_text(original.replace(lines[0],'  /* deliberately lost aggregate alias write */',1))
        obj=self.directory/'mutant-policy.o';run('cc','-std=c11','-O2','-fPIC','-c',mutant,'-o',obj)
        artifact=self.directory/'lost-alias.dylib';run('c++','-std=c++20','-O2','-shared','-fPIC','-I'+str(self.tool.directory),self.tool.directory/'frontend.o',obj,ROOT/'tools/coax-nostr/secret-check/bridge-aggregate.cpp','-o',artifact)
        lib=C.CDLL(str(artifact));bridge=self.tool.lib.ag_bridge;lib.ag_bridge.argtypes=bridge.argtypes;lib.ag_bridge.restype=bridge.restype;self.tool.lib.ag_bridge=lib.ag_bridge
        try:broken=self.tool.audit(p,'candidate')
        finally:self.tool.lib.ag_bridge=bridge
        self.assertEqual(broken['error'],0);self.observations.append({'case':'seeded_lost_alias_detected','expected_error':6,'broken_error':broken['error'],'detected':True,'mutant_c_sha256':metadata_sha(mutant)})
        host=self.directory/'sanitized.cpp';host.write_text('''#include "policy.h"
#include <cassert>
extern "C" coax_t_ScResult ag_bridge(const coax_string*,int64_t,const coax_t_ScType*,int64_t,coax_string,coax_t_AgGuard*,int64_t,int64_t*);
int main(){const char source[]="candidate :: (x:u64)->u64 {#assert x!=0;return x;}";coax_string sources[]={{source,sizeof(source)-1}};
coax_t_ScType t{};t.procedure={"candidate",9};t.procedure_line=1;t.name={"x",1};t.line=1;t.role={"parameter",9};t.type={"u64",3};
coax_t_AgGuard guards[256]{};int64_t count=0;auto r=ag_bridge(sources,1,&t,1,{"candidate",9},guards,256,&count);
assert(r.error==14&&count==1&&guards[0].mask==1);assert(ag_bridge(sources,-1,&t,1,{"candidate",9},guards,256,&count).error==2);return 0;}''')
        flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(self.tool.directory)]
        for name in ['frontend','policy']:run('cc','-std=c11',*flags,'-c',self.tool.directory/(name+'.c'),'-o',self.directory/(name+'-san.o'))
        artifact=self.directory/'sanitized';run('c++','-std=c++20',*flags,self.directory/'frontend-san.o',self.directory/'policy-san.o',ROOT/'tools/coax-nostr/secret-check/bridge-aggregate.cpp',host,'-o',artifact);run(artifact)
        self.observations.append({'case':'aggregate_bridge_sanitized','address_sanitizer':True,'undefined_sanitizer':True,'secret_assertion_error':14,'guard_origin':1,'negative_count_error':2})
    def test_pinned_compiler_identity_refuses_before_or_after_validation(self):
        p=self.directory/'compiler-bound.coax';p.write_text('candidate :: (x:u64)->u64 {return x;}')
        invalid=self.directory/'never-executed-compiler';invalid.write_text('not an executable compiler')
        original=self.tool.compiler;self.tool.compiler=invalid
        try:before=self.tool.audit(p,'candidate')
        finally:self.tool.compiler=original
        self.assertEqual(before['error'],13);self.assertFalse(before['accepted'])
        try:after=self.tool.audit(p,'candidate',after_compile=lambda:setattr(self.tool,'compiler',invalid))
        finally:self.tool.compiler=original
        self.assertEqual(after['error'],13);self.assertFalse(after['accepted'])
        self.observations.extend([{'case':'compiler_swap_before_validation','report':before},{'case':'compiler_swap_after_validation','report':after}])
if __name__=='__main__':unittest.main()

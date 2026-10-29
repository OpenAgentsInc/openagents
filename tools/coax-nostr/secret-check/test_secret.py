#!/usr/bin/env python3
"""Real frontend/source-closure tests, independent of abstract policy fixtures."""
import ctypes as C
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import unittest
from check import Audit,ROOT,COMPILER,run,metadata_sha

class SecretSource(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-secret-cases-');cls.directory=Path(cls.temp.name);cls.tool=Audit();cls.observations=[]
    @classmethod
    def tearDownClass(cls):
        raw={'schema':'coax.secret-source-cases.v1','scope':'unsigned scalar source policy only; aggregate/loop/object timing gates remain open',
             'test_driver_sha256':metadata_sha(__file__),'cases':cls.observations}
        reports=[case['report'] for case in cls.observations if 'report' in case]
        common=['compiler','compiler_sha256','audit_artifact_sha256','audit_input_sha256','classification','profile','object_timing_approved','checked_runtime_guards_proved','aggregate_memory_loops_supported']
        raw['audit']={key:reports[0][key] for key in common}
        for report in reports:
            for key in common:report.pop(key,None)
        def normalize(value):
            if isinstance(value,str):return value.replace(str(ROOT.resolve()),'$REPO').replace(str(cls.directory.resolve()),'$CASES')
            if isinstance(value,list):return [normalize(item) for item in value]
            if isinstance(value,dict):return {normalize(key):normalize(item) for key,item in value.items()}
            return value
        Path(os.environ.get('COAX_SECRET_REPORT','/tmp/coax-secret-source-tests.json')).write_text(json.dumps(normalize(raw),indent=2)+'\n')
        cls.tool.close();cls.temp.cleanup()
    def case(self,name,text,expected):
        path=self.directory/(name+'.coax');path.write_text(text)
        report=self.tool.audit(path,'candidate');self.assertEqual(report['error'],expected,(name,report))
        self.observations.append({'case':name,'expected':expected,'report':report});return report
    def test_explicit_run_step_input_is_bounded_and_recorded(self):
        source=self.directory/'run-step-input.coax';source.write_text('value :: ()->u64 {return 7;} public_value :: #run value(); candidate :: (x:u64)->u64 #export_c {return x;}')
        low=self.tool.audit(source,'candidate',run_steps=1);self.assertFalse(low['accepted']);self.assertEqual(low['compiler_run_steps'],1)
        allowed=self.tool.audit(source,'candidate',run_steps=100000);self.assertTrue(allowed['accepted'],allowed);self.assertEqual(allowed['compiler_run_steps'],100000)
        default=self.tool.audit(source,'candidate');self.assertTrue(default['accepted'],default);self.assertIsNone(default['compiler_run_steps'])
        for invalid in [0,250000001,True,'100']:
            with self.assertRaises(ValueError):self.tool.audit(source,'candidate',run_steps=invalid)
        for script in ['check.py','aggregate.py']:
            result=subprocess.run(['python3',str(ROOT/'tools/coax-nostr/secret-check'/script),str(source),'--entry','candidate','--run-steps','250000001'],capture_output=True,text=True)
            self.assertEqual(result.returncode,2);self.assertIn('between 1 and 250000000',result.stderr)
        self.observations.append({'case':'explicit_compiler_run_steps','default':None,'refused_budget':1,'accepted_budget':100000,'out_of_range_refused':True})
    def test_real_secret_control_indices_calls_and_unsupported_flow(self):
        cases=[
          ('ifx','candidate :: (x:u64)->u64 {return ifx x==0 then 1 else 2;}',6),
          ('short_circuit','candidate :: (x:u64)->bool {return x!=0 && x<2;}',6),
          ('index','candidate :: (a:[]u64,i:int)->u64 {return a[i];}',7),
          ('public_memory','candidate :: (a:[]u64)->u64 {return a[0];}',5),
          ('shift','candidate :: (x:u64,i:int)->u64 {return x<<i;}',12),
          ('vartime','read_vartime :: (x:u64)->u64 {return x;} candidate :: (x:u64)->u64 {return read_vartime(1);}',8),
          ('foreign','read :: (x:u64)->u64 #foreign; candidate :: (x:u64)->u64 {return read(x);}',9),
          ('transitive','leaf :: (x:u64)->u64 {return ifx x==0 then 1 else 2;} middle :: (x:u64)->u64 {return leaf(x);} candidate :: (x:u64)->u64 {return middle(x);}',6),
          ('recursion','again :: (x:u64)->u64 {return again(x);} candidate :: (x:u64)->u64 {return again(x);}',11),
          ('while','candidate :: (x:u64)->u64 {n:=x;while n>0 {n-=1;}return n;}',6),
          ('public_loop','candidate :: (x:u64)->u64 {for i:0..<2 {}return x;}',5),
          ('aggregate','Pair :: struct {a:u64;b:u64;} candidate :: (x:Pair)->u64 {return x.a;}',5),
          ('uninitialized','candidate :: (x:u64)->u64 {a:u64=---;return x^a;}',5),
          ('narrow_cast','candidate :: (x:u64)->u32 {return cast(u32)x;}',5),
          ('division','candidate :: (x:u64,y:u64)->u64 {return x/y;}',5),
          ('compound','candidate :: (x:u64)->u64 {a:=x;a^=0;return ifx a==0 then 1 else 2;}',6),
          ('float','candidate :: (x:f64)->f64 {return x;}',10),
          ('no_secret_syntax','#secret; candidate :: (x:u64)->u64 {return x;}',1),
          ('empty_entry','candidate :: ()->u64 {return 1;}',4),
        ]
        cases.append(('expression_depth_limit','candidate :: (x:bool)->bool {return '+'!('*16+'x'+')'*16+';}',2))
        for name,text,error in cases:self.case(name,text,error)
    def test_real_accepted_masking_public_conditions_and_actual_crypto_helpers(self):
        cases=[
          ('masked','candidate :: (m:u64,a:u64,b:u64)->u64 #export_c {return b^((a^b)&value_barrier(m));}'),
          ('public_ifx','candidate :: (x:u64)->u64 {return ifx true then x else 0;}'),
          ('clear_assignment','candidate :: (x:u64)->u64 {a:=x;a=0;return ifx a==0 then 1 else 2;}'),
          ('truncation','candidate :: (x:u64)->u32 {return cast,trunc(u32)x;}'),
          ('comments','// if secret / array[secret] / named_vartime are only a comment\ncandidate :: (x:u64)->u64 {return value_barrier(x);}')]
        cases.append(('expression_depth_boundary','candidate :: (x:bool)->bool {return '+'!('*15+'x'+')'*15+';}'))
        for name,text in cases:self.case(name,text,0)
        for entry in ['cb_ct_select32','cb_ct_select64']:
            report=self.tool.audit(ROOT/'coax/lib/crypto/bytes.coax',entry);self.assertTrue(report['accepted'],report);self.assertFalse(report['object_timing_approved']);self.observations.append({'case':entry,'report':report})
        # cb_mask uses checked multiplication on a secret; the arithmetic
        # correction refuses it conservatively (test_revalidate.py shows the
        # emitted checked multiply). It is no longer an accepted helper.
        report=self.tool.audit(ROOT/'coax/lib/crypto/bytes.coax','cb_mask');self.assertEqual(report['error'],5,report);self.observations.append({'case':'cb_mask_checked_multiply_refused','report':report})
        # Real scalar/field loops are explicitly outside this first profile.
        report=self.tool.audit(ROOT/'coax/lib/crypto/secp_scalar.coax','sc_mul');self.assertFalse(report['accepted']);self.observations.append({'case':'actual_scalar_not_admitted','report':report})
    def test_loaded_closure_hashes_helper_changes_and_postcompile_drift(self):
        directory=self.directory/'spaces in paths';directory.mkdir();helper=directory/'part.coax';source=directory/'root.coax'
        helper.write_text('safe :: (x:u64)->u64 {return value_barrier(x);}')
        source.write_text('#load "part.coax"; candidate :: (x:u64)->u64 {return safe(x);}')
        first=self.tool.audit(source,'candidate');self.assertTrue(first['accepted'],first)
        self.assertEqual(set(first['source_sha256']),{str(source.resolve()),str(helper.resolve())});self.assertEqual(first['reached_procedures'],2)
        helper.write_text('safe :: (x:u64)->u64 {return ifx x==0 then 1 else 2;}')
        changed=self.tool.audit(source,'candidate');self.assertEqual(changed['error'],6);self.assertNotEqual(first['source_sha256'],changed['source_sha256'])
        helper.write_text('safe :: (x:u64)->u64 {return value_barrier(x);}')
        drift=self.tool.audit(source,'candidate',after_compile=lambda:helper.write_text(helper.read_text()+'\n// concurrent edit\n'))
        self.assertEqual(drift['error'],13);self.assertFalse(drift['accepted'])
        self.observations.extend([{'case':'loaded_closure','report':first},{'case':'changed_helper','report':changed},{'case':'postcompile_source_drift','report':drift}])
    def test_accepted_masking_executes_checked_and_optimized_against_independent_bits(self):
        path=self.directory/'execute.coax';path.write_text('#load "'+str(ROOT/'coax/lib/crypto/bytes.coax')+'"; candidate :: (m:u64,a:u64,b:u64)->u64 #export_c {return cb_ct_select64(m,a,b);}')
        self.assertTrue(self.tool.audit(path,'candidate')['accepted'])
        rng=random.Random(1751)
        for mode,flags in [('checked',[]),('release',['--release'])]:
            c=self.directory/(mode+'.c');c.write_text(run(COMPILER,'c',path,'--library',*flags).stdout);artifact=self.directory/(mode+'.dylib')
            run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',artifact);lib=C.CDLL(str(artifact));lib.candidate.argtypes=[C.c_uint64]*3;lib.candidate.restype=C.c_uint64
            for _ in range(2000):
                mask,a,b=[rng.getrandbits(64) for _ in range(3)];self.assertEqual(lib.candidate(mask,a,b),b^((a^b)&mask))
        self.observations.append({'case':'native_masked_select_oracle','checked_cases':2000,'release_cases':2000,'optimization':'-O2','oracle':'Python bit operations','seed':1751})
    def test_lost_origin_positive_control_is_caught_by_real_source_oracle(self):
        # Deliberately break emitted policy, never the real frontend or target.
        # The oracle is an actual secret ifx source, not an AST/flow manifest.
        path=self.directory/'lost-origin.coax';path.write_text('candidate :: (x:u64)->u64 {return ifx x==0 then 1 else 2;}')
        self.assertEqual(self.tool.audit(path,'candidate')['error'],6)
        policy=self.tool.directory/'policy.c';original=policy.read_text()
        lines=[line for line in original.splitlines() if 'return ' in line and 'l_environment' in line and ').mask;' in line]
        self.assertEqual(len(lines),1);mutant=self.directory/'mutant-policy.c';mutant.write_text(original.replace(lines[0],'      return UINT64_C(0);',1))
        obj=self.directory/'mutant-policy.o';run('cc','-std=c11','-O2','-fPIC','-c',mutant,'-o',obj)
        artifact=self.directory/'lost-origin.dylib';run('c++','-std=c++20','-O2','-shared','-fPIC','-I'+str(self.tool.directory),self.tool.directory/'frontend.o',obj,ROOT/'tools/coax-nostr/secret-check/bridge.cpp','-o',artifact)
        lib=C.CDLL(str(artifact));original_bridge=self.tool.lib.sc_bridge
        lib.sc_bridge.argtypes=original_bridge.argtypes;lib.sc_bridge.restype=original_bridge.restype
        self.tool.lib.sc_bridge=lib.sc_bridge
        try:broken=self.tool.audit(path,'candidate')
        finally:self.tool.lib.sc_bridge=original_bridge
        self.assertEqual(broken['error'],0,'seeded lost-origin defect must produce the false acceptance the oracle detects')
        self.observations.append({'case':'seeded_lost_origin_detected','expected_error':6,'broken_error':broken['error'],'detected':broken['error']!=6,'mutated_representation':'generated C identifier origin return replaced by zero; source policy unchanged'})
    def test_native_frontend_and_policy_under_sanitizers(self):
        tool=self.tool;host=self.directory/'sanitize.cpp'
        host.write_text('''#include "policy.h"
#include <cassert>
extern "C" coax_t_ScResult sc_bridge(const coax_string*,int64_t,const coax_t_ScType*,int64_t,coax_string);
int main(){const char source[]="candidate :: (x:u64)->u64 {return ifx x==0 then 1 else 2;}";
coax_string sources[]={{source,sizeof(source)-1}};coax_t_ScType t{};t.procedure={"candidate",9};t.procedure_line=1;t.name={"x",1};t.line=1;t.role={"parameter",9};t.type={"u64",3};
auto result=sc_bridge(sources,1,&t,1,{"candidate",9});assert(result.error==6);assert(sc_bridge(sources,-1,&t,1,{"candidate",9}).error==2);return 0;}''')
        flags=['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(tool.directory)]
        for name in ['frontend','policy']:run('cc','-std=c11',*flags,'-c',tool.directory/(name+'.c'),'-o',self.directory/(name+'.o'))
        output=self.directory/'sanitized';run('c++','-std=c++20',*flags,self.directory/'frontend.o',self.directory/'policy.o',ROOT/'tools/coax-nostr/secret-check/bridge.cpp',host,'-o',output);run(output)
        self.observations.append({'case':'real_frontend_policy_sanitized','address_sanitizer':True,'undefined_sanitizer':True,'source_secret_ifx_error':6,'negative_source_count_error':2})

if __name__=='__main__':unittest.main()

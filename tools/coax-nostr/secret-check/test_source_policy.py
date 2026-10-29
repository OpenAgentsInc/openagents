#!/usr/bin/env python3
"""Focused lexer-token admission tests with a conditional-capable compiler."""
import ctypes as C
import json
import os
from pathlib import Path
import tempfile
import unittest
from aggregate import AggregateAudit
from check import Audit,ROOT,COMPILER,String,run,metadata_sha

CONDITIONAL=Path(os.environ.get('COAX_SOURCE_POLICY_COMPILER',COMPILER)).resolve()
class SourcePolicy(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-source-policy-');cls.directory=Path(cls.temp.name)
        cls.tools=[Audit(compiler=CONDITIONAL),AggregateAudit(compiler=CONDITIONAL)];cls.observations=[]
    @classmethod
    def tearDownClass(cls):
        data={'schema':'coax.source-token-admission.v1','compiler_sha256':metadata_sha(CONDITIONAL),'test_driver_sha256':metadata_sha(__file__),'cases':cls.observations}
        Path(os.environ.get('COAX_SOURCE_POLICY_REPORT','/tmp/coax-source-policy-native.json')).write_text(json.dumps(data,indent=2)+'\n')
        for tool in cls.tools:tool.close()
        cls.temp.cleanup()
    def source(self,name,text):
        p=self.directory/(name+'.coax');p.write_text(text);return p
    def test_real_conditional_modes_validate_then_both_audits_explicitly_refuse(self):
        p=self.source('conditional','candidate :: (x:u64)->u64 {#if BUILD_DEBUG {return x;}else{return 0;}}')
        development=run(CONDITIONAL,'c',p,'--library').stdout;release=run(CONDITIONAL,'c',p,'--library','--release').stdout
        self.assertNotEqual(development,release,'fixture must exercise actual conditional edge selection')
        for tool in self.tools:
            r=tool.audit(p,'candidate');self.assertEqual(r['error'],5,r);self.assertFalse(r['accepted'])
            self.observations.append({'case':'conditional-refused','profile':r['profile'],'error':r['error'],'line':r['line'],'source_sha256':r['source_sha256']})
    def test_ordinary_runtime_if_and_string_contents_remain_ordinary_tokens(self):
        text=self.source('string','LABEL :: "#if"; candidate :: (x:u64)->u64 {return x;}')
        runtime=self.source('runtime','candidate :: (x:u64)->u64 {if true {return x;}else{return 0;}}')
        for tool in self.tools:self.assertTrue(tool.audit(text,'candidate')['accepted'])
        self.assertTrue(self.tools[1].audit(runtime,'candidate')['accepted'])
        # Scalar-v1 never supported statement-if; this token guard does not
        # silently upgrade that separate flow profile.
        self.assertEqual(self.tools[0].audit(runtime,'candidate')['error'],5)
        self.observations.append({'case':'ordinary-tokens','string_both_profiles':True,'aggregate_runtime_if':True,'scalar_if_still_unsupported':True})
    def test_conditional_in_an_unused_loaded_unit_is_still_refused(self):
        helper=self.source('hidden','Cell :: struct {value:u64;#if BUILD_DEBUG {extra:u64;}}')
        root=self.source('root','#load "hidden.coax"; candidate :: (x:u64)->u64 {return x;}')
        run(CONDITIONAL,'c',root,'--library')
        for tool in self.tools:
            r=tool.audit(root,'candidate');self.assertEqual(r['error'],5,r);self.assertEqual(r['file'],str(helper.resolve()));self.assertFalse(r['accepted'])
            self.observations.append({'case':'loaded-conditional-refused','profile':r['profile'],'error':r['error'],'source_sha256':r['source_sha256']})
    def test_generated_native_predicate_uses_lexer_kind_not_text_substrings(self):
        for tool in self.tools:
            fn=tool.lib.audit_source_token_allowed;fn.argtypes=[C.c_bool,String];fn.restype=C.c_bool
            for directive,text,expected in [(False,b'if',True),(False,b'#if',True),(True,b'if',False),(True,b'run',True)]:
                buf=C.create_string_buffer(text);self.assertEqual(fn(directive,String(C.cast(buf,C.c_void_p),len(text))),expected)
        self.observations.append({'case':'native-token-predicate','both_profiles':True,'truth_table_cases':8})
if __name__=='__main__':unittest.main()

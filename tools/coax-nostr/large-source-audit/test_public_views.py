#!/usr/bin/env python3
"""Source-bound view premises and public-only taint/effect adversarial cases."""
import copy
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from aggregate import AggregateAudit
from check import ROOT, metadata_sha

class PublicViews(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-public-views-');cls.root=Path(cls.temp.name);cls.tool=AggregateAudit();cls.observations=[]
    @classmethod
    def tearDownClass(cls):
        def normalize(x):
            if isinstance(x,str):return x.replace(str(ROOT.resolve()),'$REPO').replace(str(cls.root.resolve()),'$CASES')
            if isinstance(x,list):return [normalize(v) for v in x]
            if isinstance(x,dict):return {normalize(k):normalize(v) for k,v in x.items()}
            return x
        report={'schema':'coax.public-view-source-tests.v1','scope':'conditional source analysis; no physical ownership, target or production approval','test_driver_sha256':metadata_sha(__file__),'observations':normalize(cls.observations)}
        Path(os.environ.get('COAX_PUBLIC_VIEW_REPORT','/tmp/coax-large-public-view-source-tests.json')).write_text(json.dumps(report,indent=2)+'\n');cls.tool.close();cls.temp.cleanup()
    def source(self,name,text,params):
        p=self.root/(name+'.coax');p.write_text(text);baseline=self.tool.audit(p,'candidate')
        rows=[]
        for n,t,c,access,extent in params:
            rows.append({'name':n,'type':t,'classification':c,'view':{'element_type':t[3:],'extent':extent,'access':access} if access else None})
        manifest={'schema':'coax.secret-large-classification-views.v3','entry':'candidate','sources':baseline['source_sha256'],'parameters':rows,'views_disjoint':True,'live_aligned_exclusive':True,'public_only':True,'compiler_sha256':self.tool.compiler_fingerprint,'audit_inputs':self.tool.inputs,'audit_artifact_sha256':metadata_sha(self.tool.artifact),'profile':'native-c-checked-library'}
        return p,baseline,manifest
    def audit(self,p,m,error):
        r=self.tool.audit(p,'candidate',classification=m);self.assertEqual(r['error'],error,r);self.observations.append({'case':p.name,'expected':error,'report':r});return r
    def test_view_read_and_default_refusal(self):
        for cls,mask in [('public',0),('secret',1)]:
            p,baseline,m=self.source('read_'+cls,'candidate :: (v:[]u8)->u8 #export_c {return v[0];}', [('v','[] u8',cls,'read',2)])
            self.assertFalse(baseline['accepted']);r=self.audit(p,m,0);self.assertEqual(r['return_dependency_mask'],mask);self.assertIn('caller supplies live aligned',r['view_assumption'])
    def test_checked_cast_on_secret_view(self):
        # v2: a range-contained (widening) checked cast of a secret is admitted
        # and keeps its dependency; every other secret checked cast refuses.
        p,_,m=self.source('secret_cast','candidate :: (v:[]u8)->u64 #export_c {return cast(u64)v[0];}', [('v','[] u8','secret','read',2)])
        r=self.audit(p,m,0);self.assertTrue(r['accepted']);self.assertEqual(r['return_dependency_mask'],1)
        p,_,m=self.source('tainted_alias_cast','candidate :: (v:[]u8,x:u8)->u64 #export_c {w:[]u8=v[0..<1];v[0]=x;return cast(u64)w[0];}', [('v','[] u8','public','write',2),('x','u8','secret',None,0)])
        r=self.audit(p,m,0);self.assertTrue(r['accepted']);self.assertEqual(r['return_dependency_mask'],2)
        # Classified scalars are u64, so narrower secrets come from explicit truncation.
        for name,body,ret,error in [
            ('widen_u8_u32','b:=cast,trunc(u8)x;return cast(u32)b;','u32',0),
            ('widen_u8_u64','b:=cast,trunc(u8)x;return cast(u64)b;','u64',0),
            ('widen_u8_int','b:=cast,trunc(u8)x;return cast(int)b;','int',5),
            ('widen_u32_u64','w:=cast,trunc(u32)x;return cast(u64)w;','u64',0),
            ('widen_u32_int','w:=cast,trunc(u32)x;return cast(int)w;','int',5),
            ('narrow_u64_u32','return cast(u32)x;','u32',5),
            ('narrow_u64_u8','return cast(u8)x;','u8',5),
            ('narrow_u32_u8','w:=cast,trunc(u32)x;return cast(u8)w;','u8',5),
            ('sign_u64_int','return cast(int)x;','int',5),
            ('identity_u64','return cast(u64)x;','u64',5),
            ('narrow_after_widen','b:=cast,trunc(u8)x;return cast(u8)(cast(u32)b<<1);','u8',5)]:
            text='candidate :: (x:u64)->'+ret+' #export_c {'+body+'}'
            p,_,m=self.source(name,text,[('x','u64','secret',None,0)]);r=self.audit(p,m,error);self.assertEqual(r['accepted'],error==0,name)
            if error==0:self.assertEqual(r['return_dependency_mask'],1,name)
            else:self.assertEqual(r['error_name'],'unsupported',name)
        # v3: unsigned ranges prove checked + and - and narrowing casts safe, with
        # loop-carried ranges from an inductive invariant; u64 * always refuses.
        for name,body,error in [
            ('carry_chain','carry:=cast(u64)0;for i:0..<8 {wide:=(x&0xFFFFFFFF)+(x>>32)+carry;carry=wide>>32;}return carry;',0),
            ('borrow_chain','borrow:=cast(u64)0;for i:0..<8 {wide:=(cast(u64)1<<32)+(x&0xFFFFFFFF)-(x>>32)-borrow;borrow=1-(wide>>32);}return borrow;',0),
            ('growing_sum','acc:=cast(u64)0;for i:0..<8 {acc=acc+(x&1);}return acc;',5),
            ('u64_mul','return (x&1)*3;',5),
            ('u32_mul','w:=cast,trunc(u32)x;return cast(u64)((w&0xFFFF)*(w&0xFFFF));',0),
            ('u32_mul_overflow','w:=cast,trunc(u32)x;return cast(u64)((w&0x1FFFF)*(w&0xFFFF));',5),
            ('narrow_masked','return cast(u64)(cast(u32)(x&0xFFFFFFFF));',0),
            ('narrow_shifted','return cast(u64)(cast(u32)(x>>32));',0),
            ('add_overflow','return x+1;',5),
            ('sub_underflow','return (x&0xFF)-(x>>60);',5),
            ('sub_safe','return (cast(u64)256+(x&0xFF))-(x>>60);',0),
            ('carry_escapes','carry:=cast(u64)0;for i:0..<8 {carry=(x&0xFFFFFFFF)+carry;}return carry;',5)]:
            text='candidate :: (x:u64)->u64 #export_c {'+body+'}'
            p,_,m=self.source(name,text,[('x','u64','secret',None,0)]);r=self.audit(p,m,error);self.assertEqual(r['accepted'],error==0,name)
            if error==0:self.assertEqual(r['return_dependency_mask'],1,name)
            else:self.assertEqual(r['error_name'],'unsupported',name)
        p,_,m=self.source('narrow_view_shift','candidate :: (v:[]u8)->u8 #export_c {return cast(u8)(cast(u32)v[0]<<1);}',[('v','[] u8','secret','read',2)])
        r=self.audit(p,m,5);self.assertFalse(r['accepted'])
        p,_,m=self.source('public_narrow','candidate :: (x:u64)->u32 #export_c {return cast(u32)x;}',[('x','u64','public',None,0)])
        r=self.audit(p,m,0);self.assertEqual(r['return_dependency_mask'],0)
    def test_taint_alias_and_readonly_effects(self):
        cases=[
          ('branch','candidate :: (v:[]u8)->u8 #export_c {if v[0]==0 {return 1;}return 2;}',[('v','[] u8','secret','read',2)],6),
          ('index','candidate :: (v:[]u8,i:u64)->u8 #export_c {return v[cast,trunc(int)i];}',[('v','[] u8','public','read',2),('i','u64','secret',None,0)],7),
          ('readonly','candidate :: (v:[]u8)->u8 #export_c {v[0]=1;return v[0];}',[('v','[] u8','public','read',2)],5),
          ('readonly_callee','write :: (v:[]u8)->u8 {v[0]=1;return 0;} candidate :: (v:[]u8)->u8 #export_c {return write(v);}',[('v','[] u8','public','read',2)],5),
          ('readonly_slice','write :: (v:[]u8)->u8 {v[0]=1;return 0;} candidate :: (v:[]u8)->u8 #export_c {w:[]u8=v[0..<1];return write(w);}',[('v','[] u8','public','read',2)],5),
          ('tainted_public','read_public :: (v:[]u8)->u8 {return v[0];} candidate :: (v:[]u8,x:u8)->u8 #export_c {v[0]=x;return read_public(v);}',[('v','[] u8','public','write',2),('x','u8','secret',None,0)],8),
          ('tainted_slice_alias','read_public :: (v:[]u8)->u8 {return v[0];} candidate :: (v:[]u8,x:u8)->u8 #export_c {w:[]u8=v[0..<1];v[0]=x;return read_public(w);}',[('v','[] u8','public','write',2),('x','u8','secret',None,0)],8),
          ('tainted_alias_branch','candidate :: (v:[]u8,x:u8)->u8 #export_c {w:[]u8=v[0..<1];w[0]=x;if v[0]==0 {return 1;}return 0;}',[('v','[] u8','public','write',2),('x','u8','secret',None,0)],6),
          ('secret_public','read_public :: (v:[]u8)->u8 {return v[0];} candidate :: (v:[]u8)->u8 #export_c {return read_public(v);}',[('v','[] u8','secret','read',2)],8),
          ('forbidden_public','read_vartime :: (v:[]u8)->u8 {return v[0];} candidate :: (v:[]u8)->u8 #export_c {return read_vartime(v);}',[('v','[] u8','public','read',2)],8),
          ('forbidden_transitive','read_vartime :: (v:[]u8)->u8 {return v[0];} outer_public :: (v:[]u8)->u8 {return read_vartime(v);} candidate :: (v:[]u8)->u8 #export_c {return outer_public(v);}',[('v','[] u8','public','read',2)],8),
          ('secret_branch_public_helper','read_public :: (v:[]u8)->u8 {return v[0];} candidate :: (v:[]u8)->u8 #export_c {if v[0]==0 {return read_public(v);}return 0;}',[('v','[] u8','secret','read',2)],6),
          ('foreign','external :: (x:u8)->u8 #foreign; candidate :: (v:[]u8)->u8 #export_c {return external(v[0]);}',[('v','[] u8','public','read',2)],9),
          ('bounds','candidate :: (v:[]u8)->u8 #export_c {return v[2];}',[('v','[] u8','public','read',2)],2),
          ('zero_trip','candidate :: (v:[]u8)->u8 #export_c {for i:0..<0 {}return v[0];}',[('v','[] u8','public','read',2)],5),
          ('view_escape','candidate :: (v:[]u8)->[]u8 #export_c {return v;}',[('v','[] u8','public','read',2)],5),
        ]
        for name,text,params,error in cases:
            with self.subTest(name=name):p,_,m=self.source(name,text,params);self.audit(p,m,error)
    def test_public_callee_and_writable_view(self):
        cases=[('public_call','read_public :: (v:[]u8)->u8 {return v[0];} candidate :: (v:[]u8)->u8 #export_c {return read_public(v);}','read'),('write','candidate :: (v:[]u8)->u8 #export_c {v[0]=1;return v[0];}','write')]
        for n,text,access in cases:
            p,_,m=self.source(n,text,[('v','[] u8','public',access,2)]);self.audit(p,m,0)
        p,_,m=self.source('secret_copy','candidate :: (s:[]u8,d:[]u8)->u8 #export_c {d[0]=s[0];return d[0];}', [('s','[] u8','secret','read',2),('d','[] u8','public','write',2)])
        self.assertEqual(self.audit(p,m,0)['return_dependency_mask'],1)
        p,_,m=self.source('public_guard','candidate :: (v:[]u8)->u8 #export_c {#assert v[0]==0;return 0;}', [('v','[] u8','public','read',2)])
        r=self.audit(p,m,14);self.assertFalse(r['accepted']);self.assertEqual([g['origin_mask'] for g in r['authored_guard_obligations']],[0])
    def test_manifest_and_tool_identity_mismatches_refuse(self):
        p,_,m=self.source('manifest','candidate :: (v:[]u8)->u8 #export_c {return v[0];}',[('v','[] u8','public','read',2)]);self.audit(p,m,0)
        variants=[]
        for key,value in [('views_disjoint',False),('live_aligned_exclusive',False),('public_only',False),('compiler_sha256','0'*64),('audit_artifact_sha256','0'*64),('audit_inputs',{}),('profile','release')]:
            x=copy.deepcopy(m);x[key]=value;variants.append(x)
        for key,value in [('extent',0),('extent',True),('extent',1025),('element_type','u64'),('access','unknown')]:
            x=copy.deepcopy(m);x['parameters'][0]['view'][key]=value;variants.append(x)
        x=copy.deepcopy(m);x['parameters'][0]['name']='other';variants.append(x)
        x=copy.deepcopy(m);x['sources'][str(p)]='0'*64;variants.append(x)
        for x in variants:self.audit(p,x,13)
    def test_cli_handshake_binds_this_process_and_refuses_changed_identity(self):
        p,_,_=self.source('handshake','candidate :: (v:[]u8)->u8 #export_c {return v[0];}', [('v','[] u8','secret','read',2)])
        for changed in (False,True):
            proc=subprocess.Popen([sys.executable,str(ROOT/'tools/coax-nostr/large-source-audit/aggregate.py'),str(p),'--entry','candidate','--classification-stdio'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            try:
                hello=json.loads(proc.stdout.readline());self.assertEqual(hello['schema'],'coax.secret-classification-handshake.v1')
                m={k:hello[k] for k in ('entry','sources','compiler_sha256','audit_inputs','audit_artifact_sha256','profile')};m.update(schema='coax.secret-large-classification-views.v3',views_disjoint=True,live_aligned_exclusive=True,public_only=True,parameters=[{'name':'v','type':'[] u8','classification':'secret','view':{'element_type':'u8','extent':2,'access':'read'}}])
                if changed:m['audit_artifact_sha256']='0'*64
                out,err=proc.communicate(json.dumps(m)+'\n',timeout=120);r=json.loads(out)
                self.assertEqual(proc.returncode,1 if changed else 0,err);self.assertEqual(r['error'],13 if changed else 0,r)
                if not changed:self.assertEqual(r['return_dependency_mask'],1)
            finally:
                if proc.poll() is None:proc.kill();proc.wait()
    def test_loaded_artifact_substitution_with_new_hash_refuses(self):
        p,_,m=self.source('artifact_swap','candidate :: (v:[]u8)->u8 #export_c {return v[0];}', [('v','[] u8','secret','read',2)])
        original=self.tool.artifact.read_bytes();old_hash=self.tool.artifact_fingerprint;poison=original+b'changed';new_hash=hashlib.sha256(poison).hexdigest()
        def replace(data):
            path=self.tool.artifact.with_suffix('.replacement');path.write_bytes(data);os.replace(path,self.tool.artifact)
        try:
            replace(poison);changed=copy.deepcopy(m);changed['audit_artifact_sha256']=new_hash;r=self.audit(p,changed,13);self.assertEqual(r['audit_artifact_sha256'],old_hash)
            replace(original);r=self.tool.audit(p,'candidate',classification=m,after_compile=lambda:replace(poison));self.assertEqual(r['error'],13,r);self.assertEqual(r['audit_artifact_sha256'],old_hash)
        finally:replace(original)
        self.assertEqual(self.audit(p,m,0)['return_dependency_mask'],1)
    def test_real_ecdh_entry_passes_widening_and_refuses_checked_arithmetic(self):
        p=ROOT/'coax/lib/crypto/ecdh.coax';base=self.tool.audit(p,'ecdh_stage')
        self.assertFalse(base['accepted']);self.assertEqual(base['error_name'],'unsupported')
        rows=[{'name':'secret','type':'[] u8','classification':'secret','view':{'element_type':'u8','extent':32,'access':'read'}},{'name':'peer','type':'[] u8','classification':'public','view':{'element_type':'u8','extent':32,'access':'read'}},{'name':'work','type':'[] EcdhScratch','classification':'public','view':{'element_type':'EcdhScratch','extent':1,'access':'write'}}]
        m={'schema':'coax.secret-large-classification-views.v3','entry':'ecdh_stage','sources':base['source_sha256'],'parameters':rows,'views_disjoint':True,'live_aligned_exclusive':True,'public_only':True,'compiler_sha256':self.tool.compiler_fingerprint,'audit_inputs':self.tool.inputs,'audit_artifact_sha256':metadata_sha(self.tool.artifact),'profile':'native-c-checked-library'}
        r=self.tool.audit(p,'ecdh_stage',classification=m);self.assertEqual(r['error'],5,r);self.assertFalse(r['accepted']);self.assertLess(r['visited_nodes'],262144)
        # v3 also proves the scalar carry chains; cb_mask's checked u64 multiply
        # of a secret bit still refuses (its emitted check branches on the bit).
        self.assertTrue(r['file'].endswith('/crypto/bytes.coax'));self.assertEqual((r['line'],r['column']),(179,73));self.assertEqual(r['reached_procedures'],33)
        self.assertEqual([(g['line'],g['origin_mask']) for g in r['authored_guard_obligations']],[(239,0),(492,0)])
        self.observations.append({'case':'real_ecdh_entry','expected':5,'report':r})
if __name__=='__main__':unittest.main()

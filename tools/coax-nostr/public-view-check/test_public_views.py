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
        Path(os.environ.get('COAX_PUBLIC_VIEW_REPORT','/tmp/coax-public-view-source-tests.json')).write_text(json.dumps(report,indent=2)+'\n');cls.tool.close();cls.temp.cleanup()
    def source(self,name,text,params):
        p=self.root/(name+'.coax');p.write_text(text);baseline=self.tool.audit(p,'candidate')
        rows=[]
        for n,t,c,access,extent in params:
            rows.append({'name':n,'type':t,'classification':c,'view':{'element_type':t[3:],'extent':extent,'access':access} if access else None})
        manifest={'schema':'coax.secret-classification-views.v2','entry':'candidate','sources':baseline['source_sha256'],'parameters':rows,'views_disjoint':True,'live_aligned_exclusive':True,'public_only':True,'compiler_sha256':self.tool.compiler_fingerprint,'audit_inputs':self.tool.inputs,'audit_artifact_sha256':metadata_sha(self.tool.artifact),'profile':'native-c-checked-library'}
        return p,baseline,manifest
    def audit(self,p,m,error):
        r=self.tool.audit(p,'candidate',classification=m);self.assertEqual(r['error'],error,r);self.observations.append({'case':p.name,'expected':error,'report':r});return r
    def test_view_read_and_default_refusal(self):
        for cls,mask in [('public',0),('secret',1)]:
            p,baseline,m=self.source('read_'+cls,'candidate :: (v:[]u8)->u8 #export_c {return v[0];}', [('v','[] u8',cls,'read',2)])
            self.assertFalse(baseline['accepted']);r=self.audit(p,m,0);self.assertEqual(r['return_dependency_mask'],mask);self.assertIn('caller supplies live aligned',r['view_assumption'])
    def test_checked_cast_on_secret_view(self):
        p,_,m=self.source('secret_cast','candidate :: (v:[]u8)->u64 #export_c {return cast(u64)v[0];}', [('v','[] u8','secret','read',2)])
        r=self.audit(p,m,5);self.assertEqual(r['error_name'],'unsupported');self.assertFalse(r['accepted'])
        p,_,m=self.source('tainted_alias_cast','candidate :: (v:[]u8,x:u8)->u64 #export_c {w:[]u8=v[0..<1];v[0]=x;return cast(u64)w[0];}', [('v','[] u8','public','write',2),('x','u8','secret',None,0)])
        r=self.audit(p,m,5);self.assertEqual(r['error_name'],'unsupported');self.assertFalse(r['accepted'])
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
            proc=subprocess.Popen([sys.executable,str(ROOT/'tools/coax-nostr/public-view-check/aggregate.py'),str(p),'--entry','candidate','--classification-stdio'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            try:
                hello=json.loads(proc.stdout.readline());self.assertEqual(hello['schema'],'coax.secret-classification-handshake.v1')
                m={k:hello[k] for k in ('entry','sources','compiler_sha256','audit_inputs','audit_artifact_sha256','profile')};m.update(schema='coax.secret-classification-views.v2',views_disjoint=True,live_aligned_exclusive=True,public_only=True,parameters=[{'name':'v','type':'[] u8','classification':'secret','view':{'element_type':'u8','extent':2,'access':'read'}}])
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
    def test_real_ecdh_entry_refuses_secret_checked_cast(self):
        p=ROOT/'coax/lib/crypto/ecdh.coax';base=self.tool.audit(p,'ecdh_stage')
        self.assertFalse(base['accepted']);self.assertEqual(base['error_name'],'unsupported')
        rows=[{'name':'secret','type':'[] u8','classification':'secret','view':{'element_type':'u8','extent':32,'access':'read'}},{'name':'peer','type':'[] u8','classification':'public','view':{'element_type':'u8','extent':32,'access':'read'}},{'name':'work','type':'[] EcdhScratch','classification':'public','view':{'element_type':'EcdhScratch','extent':1,'access':'write'}}]
        m={'schema':'coax.secret-classification-views.v2','entry':'ecdh_stage','sources':base['source_sha256'],'parameters':rows,'views_disjoint':True,'live_aligned_exclusive':True,'public_only':True,'compiler_sha256':self.tool.compiler_fingerprint,'audit_inputs':self.tool.inputs,'audit_artifact_sha256':metadata_sha(self.tool.artifact),'profile':'native-c-checked-library'}
        r=self.tool.audit(p,'ecdh_stage',classification=m);self.assertEqual(r['error'],5,r);self.assertFalse(r['accepted']);self.assertLess(r['visited_nodes'],262144)
        self.assertTrue(r['file'].endswith('/crypto/bytes.coax'));self.assertEqual((r['line'],r['column']),(108,28))
        self.assertEqual([(g['line'],g['origin_mask']) for g in r['authored_guard_obligations']],[(239,0),(492,0)])
        self.observations.append({'case':'real_ecdh_entry','expected':5,'report':r})
if __name__=='__main__':unittest.main()

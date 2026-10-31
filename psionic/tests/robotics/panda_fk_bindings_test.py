#!/usr/bin/env python3
"""Only NEW metadata guard checks; no native numerical or SDK execution."""
import copy,json,unittest
from pathlib import Path
import panda_fk_reference as ref
class Bindings(unittest.TestCase):
    def setUp(self):
        self.profile=json.loads((ref.FIXTURE/'source-profile.json').read_text());self.record=json.loads((ref.FIXTURE/'sdk-reference.json').read_text())
        self.files={name:(ref.FIXTURE/name).read_bytes() for name in ('sdk-profile.json','sdk-model.json','sdk-cases.f64','sdk-poses.f64')}
    def test_actual_reference_identity(self):
        ref.verify_sdk_receipt(self.record,self.profile,self.files)
    def test_each_binding_refuses_alteration(self):
        changes=[('passed',False),('sdk_version','3.3.0'),('source_original_xml_sha256','0'*64),('source_rewritten_xml_sha256','0'*64),('forward_calls',8),('integration_steps',1),('case_shape',[8,7]),('pose_shape',[7,203]),('absolute_tolerance_frozen_before_capture',1e-10),('body_names',[])]
        for key,value in changes:
            with self.subTest(field=key):
                bad=copy.deepcopy(self.record);bad[key]=value
                with self.assertRaises(RuntimeError):ref.verify_sdk_receipt(bad,self.profile,self.files)
    def test_payload_and_path_changes_refuse(self):
        for name in self.files:
            with self.subTest(file=name):
                changed=dict(self.files);changed[name]=changed[name][:-1]+bytes([changed[name][-1]^1])
                with self.assertRaises(RuntimeError):ref.verify_sdk_receipt(self.record,self.profile,changed)
        bad=copy.deepcopy(self.record);bad['fixtures']['../other']=bad['fixtures']['sdk-model.json']
        with self.assertRaises(RuntimeError):ref.verify_sdk_receipt(bad,self.profile,self.files)
if __name__=='__main__':unittest.main()

#!/usr/bin/env python3
"""Focused optional runner tests, including real incorrect native candidates."""
from pathlib import Path
import importlib.util
import subprocess
import tempfile
import unittest

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('vector_runner',HERE/'run.py')
v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)

class Runner(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp=tempfile.TemporaryDirectory(prefix='vector-runner-test-');cls.d=Path(cls.tmp.name)
        cls.cases=[c for p in ['SHA256ShortMsg.rsp','SHA256LongMsg.rsp'] for c in v.read_rsp(v.CORPUS/'upstream/nist'/p)]
        cls.driver,cls.evidence=v.build(HERE/'fingerprint.coax',cls.d)
    @classmethod
    def tearDownClass(cls):cls.tmp.cleanup()
    def test_all_pinned_sha_rows_execute_with_sanitizers(self):
        self.assertEqual(len(self.cases),129)
        self.assertEqual(v.execute(self.driver,self.cases),129)
        self.assertTrue(self.evidence['sanitized'])
        self.assertIn('coax/lib/proto/fingerprint.coax',self.evidence['dependencies'])
    def test_empty_vector_is_zero_bytes_not_zero_byte(self):
        self.assertEqual(self.cases[0][1],b'')
        self.assertEqual(self.cases[0][2].hex(),'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855')
    def test_one_changed_expected_row_is_reported(self):
        rows=list(self.cases);name,msg,digest=rows[7]
        rows[7]=(name,msg,bytes([digest[0]^1])+digest[1:])
        with self.assertRaisesRegex(RuntimeError,name):v.execute(self.driver,rows)
    def test_incorrect_actual_native_candidate_is_detected(self):
        with tempfile.TemporaryDirectory() as tmp:
            d=Path(tmp);src=d/'bad.coax'
            src.write_text('vector_digest :: (input:[]u8,out:[]u8)->bool #export_c { for i:0..<32 {out[i]=0;} return true;}')
            driver,_=v.build(src,d)
            with self.assertRaisesRegex(RuntimeError,'digest mismatch'):v.execute(driver,self.cases[:1])
    def test_empty_and_truncated_transport_refuse(self):
        for raw in (b'',b'\0',b'\0\0\0\x01'):
            p=subprocess.run([str(self.driver)],input=raw,capture_output=True)
            self.assertNotEqual(p.returncode,0)
    def test_rsp_malformed_duplicate_and_empty_refuse(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'bad.rsp'
            for data in ('','# comment\n','Len = 0\nLen = 0\nMsg = 00\nMD = '+'00'*32,'Len = 7\nMsg = 00\nMD = '+'00'*32,'Len = 8\nMsg = 00\nMD = 00'):
                p.write_text(data)
                with self.assertRaises(ValueError):v.read_rsp(p)
    def test_unsupported_suite_is_failure_not_skipped_success(self):
        for suite in ['all','hmac','sha256']:
            p=subprocess.run(['python3',str(HERE/'run.py'),'--suite',suite],capture_output=True,text=True)
            self.assertNotEqual(p.returncode,0)
    def test_release_optimized_driver_has_same_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            driver,metadata=v.build(HERE/'fingerprint.coax',Path(tmp),'release',False)
            self.assertFalse(metadata['sanitized']);self.assertEqual(v.execute(driver,self.cases),129)
    def test_explicit_sha_suite_adapter_runs_real_module_contract(self):
        with tempfile.TemporaryDirectory() as tmp:
            report=Path(tmp)/'report.json'
            p=subprocess.run(['python3',str(HERE/'run.py'),'--suite','sha256','--adapter',str(HERE/'fingerprint.coax'),'--report',str(report)],capture_output=True,text=True)
            self.assertEqual(p.returncode,0,p.stderr);self.assertIn('129 rows passed',p.stdout)
            self.assertTrue(report.is_file())
if __name__=='__main__':unittest.main()

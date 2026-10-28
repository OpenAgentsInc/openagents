#!/usr/bin/env python3
"""Only test-oracle plumbing. No Coax crypto implementation exists in this task."""
import csv
import json
import os
from pathlib import Path
import tempfile
import shutil
import unittest
import sys

from oracle import P, N, G, field, scalar, point_mul, encode_point, schnorr_verify, NativeOracle, DEFAULT_LIBRARY
from differential import cases, compare, smoke, candidate_run

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]

class IntegerOracleTests(unittest.TestCase):
    def test_modulus_edges_and_carries(self):
        self.assertEqual(field('mul',P-1,P-1),1)
        self.assertEqual(scalar('mul',N-1,N-1),1)
        self.assertEqual(field('add',P-1,1),0)
        self.assertEqual(scalar('add',N-1,1),0)
        for modulus, fn in [(P,field),(N,scalar)]:
            self.assertEqual(fn('inverse',modulus-1),modulus-1)
            with self.assertRaises(ValueError): fn('inverse',0)
        self.assertIsNone(point_mul(N,G))
        self.assertEqual(point_mul(1,G),G)

    def test_carry_fault_is_detected_and_recorded(self):
        request={'module':'field','op':'mul','a':format(P-1,'064x'),'b':format(P-1,'064x')}
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'regressions.jsonl'
            wrong=format(((P-1)*(P-1)&((1<<256)-1))%P,'064x')
            self.assertFalse(compare(request,{'ok':True,'value':wrong},path))
            record=json.loads(path.read_text())
            self.assertEqual(record['request'],request)
            self.assertEqual(record['expected']['value'],format(1,'064x'))

    def test_pinned_seed_generation_is_reproducible(self):
        self.assertEqual(cases(123,8),cases(123,8))
        self.assertNotEqual(cases(123,8),cases(124,8))

class PublicAPIOracleTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path=Path(os.environ.get('COAX_SECP_ORACLE_LIB',DEFAULT_LIBRARY))
        if not path.is_file(): raise RuntimeError('run build_oracle.py --fetch --reviewed first (no implicit network/build in tests)')
        cls.native=NativeOracle(path)

    @classmethod
    def tearDownClass(cls):
        cls.native.close()

    def test_changed_native_artifact_is_refused_before_loading(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);target=root/self.native.path.name
            shutil.copyfile(self.native.path,target)
            shutil.copyfile(self.native.path.parent/'build.json',root/'build.json')
            with target.open('ab') as stream:stream.write(b'wrong artifact')
            with self.assertRaisesRegex(ValueError,'artifact/provenance drift'):
                NativeOracle(target)

    def test_all_official_schnorr_rows(self):
        path=ROOT/'coax/tests/vectors/upstream/bip340/test-vectors.csv'
        with path.open() as stream: rows=list(csv.DictReader(stream))
        self.assertEqual(len(rows),19)
        for row in rows:
            msg=bytes.fromhex(row['message']);sig=bytes.fromhex(row['signature']);pk=bytes.fromhex(row['public key'])
            valid=row['verification result']=='TRUE'
            with self.subTest(index=row['index']):
                self.assertEqual(self.native.verify(msg,pk,sig),valid)
                self.assertEqual(schnorr_verify(msg,pk,sig),valid)
                if row['secret key']:
                    self.assertEqual(self.native.sign(msg,bytes.fromhex(row['secret key']),bytes.fromhex(row['aux_rand'])),sig)

    def test_raw_x_and_default_digest_are_distinct(self):
        sk=(3).to_bytes(32,'big');peer=point_mul(7,G)[0].to_bytes(32,'big')
        raw=self.native.ecdh(sk,peer)
        self.assertEqual(raw,point_mul(21,G)[0].to_bytes(32,'big'))
        wrong=self.native.ecdh(sk,peer,default_hash=True)
        self.assertNotEqual(wrong,raw)
        request={'module':'ecdh','secret':sk.hex(),'peer':peer.hex()}
        with tempfile.TemporaryDirectory() as d:
            self.assertFalse(compare(request,{'ok':True,'value':wrong.hex()},Path(d)/'wrong.jsonl'))

    def test_invalid_keys_and_lifts_refuse(self):
        for secret,peer in [(bytes(32),G[0].to_bytes(32,'big')),(N.to_bytes(32,'big'),G[0].to_bytes(32,'big')),((3).to_bytes(32,'big'),P.to_bytes(32,'big'))]:
            self.assertIsNone(self.native.ecdh(secret,peer))

    def test_bounded_cross_oracle_smoke(self):
        result=smoke(self.native,seed=20261028,count=32)
        self.assertGreater(result['comparisons'],100)
        self.assertEqual(result['failures'],0)

    def test_candidate_protocol_and_seeded_faults(self):
        fixture=HERE/'fixtures/reference_candidate.py'
        with tempfile.TemporaryDirectory() as d:
            for fault in ('none','carry','default-hash'):
                with self.subTest(fault=fault):
                    report=candidate_run(self.native,[sys.executable,str(fixture),'--fault',fault],20261028,8,0,Path(d)/(fault+'.jsonl'),5)
                    self.assertEqual(report['failures'],0 if fault=='none' else 1)
                    if fault!='none':
                        record=json.loads((Path(d)/(fault+'.jsonl')).read_text())
                        self.assertEqual(record['request']['module'],'field' if fault=='carry' else 'ecdh')

    def test_candidate_timeout_and_output_budget_refuse(self):
        with tempfile.TemporaryDirectory() as d:
            for program in ('import time; time.sleep(10)','import sys; sys.stdout.write("x"*2000000)'):
                report=candidate_run(self.native,[sys.executable,'-c',program],1,1,0,Path(d)/'bad.jsonl',0.2)
                self.assertEqual(report['failures'],1)

if __name__=='__main__':unittest.main()

#!/usr/bin/env python3
"""The whole Nostr signing entry under classification v4 (#4828).

Runs the large profile over nostr_sign_step, the signer's complete closure,
and checks that analysis completes with only declared obligations: every
remaining item is a *_declassify call in schnorr_sign.coax, an authored guard
whose origin is public, or the one public output: out, which receives the
published signature. Writes the report for the record."""
import json
import os
import unittest
from pathlib import Path
from aggregate import AggregateAudit
from check import ROOT, metadata_sha

SIGNER = ROOT/'coax/lib/crypto/schnorr_sign.coax'
PUBLIC_FIELDS = [['SignWork','stage'],['SignWork','challenge'],['SignWork','message'],['SignPrepare','public_key'],
                 ['SignNoncePoint','r'],['SsvWork','stage'],['SsvWork','bound'],['SsvWork','peer'],
                 ['ShaOwned','used'],['ShaOwned','phase'],['ShaOwned','total']]
PARAMETERS = [('key','[] u8','secret',32,'read'),('aux','[] u8','secret',32,'read'),('event_id','[] u8','public',32,'read'),
              ('work','[] SignWork','secret',1,'write'),('out','[] u8','public',64,'write'),('overlap','bool','public',0,None)]
STEPS = 250000000

class SignerEntry(unittest.TestCase):
    def test_whole_nostr_entry_has_only_declared_obligations(self):
        tool = AggregateAudit()
        try:
            base = tool.audit(SIGNER, 'nostr_sign_step', run_steps=STEPS)
            rows = [{'name':n,'type':t,'classification':c,'view':({'element_type':t[3:],'extent':e,'access':a} if a else None)} for n,t,c,e,a in PARAMETERS]
            manifest = {'schema':'coax.secret-large-classification-views.v4','entry':'nostr_sign_step','sources':base['source_sha256'],
                        'parameters':rows,'views_disjoint':True,'live_aligned_exclusive':True,'public_only':True,
                        'compiler_sha256':tool.compiler_fingerprint,'audit_inputs':tool.inputs,'audit_artifact_sha256':metadata_sha(tool.artifact),
                        'profile':'native-c-checked-library','public_fields':PUBLIC_FIELDS}
            r = tool.audit(SIGNER, 'nostr_sign_step', classification=manifest, run_steps=STEPS)
        finally:
            tool.close()
        out = os.environ.get('COAX_SIGNER_ENTRY_REPORT')
        if out: Path(out).write_text(json.dumps({'manifest':manifest,'report':r}, indent=2)+'\n')
        self.assertEqual(r['error_name'], 'guard-obligations', r.get('file'))
        self.assertEqual(r['return_dependency_mask'], 0)
        self.assertGreaterEqual(r['reached_procedures'], 100)
        declassified = [g for g in r['authored_guard_obligations'] if g['kind'] == 'declassification']
        guards = [g for g in r['authored_guard_obligations'] if g['kind'] == 'authored-assertion']
        outputs = [g for g in r['authored_guard_obligations'] if g['kind'] == 'public-output']
        self.assertEqual(len(declassified) + len(guards) + len(outputs), len(r['authored_guard_obligations']))
        # The only public output is the entry's out parameter (bit 4 of the
        # parameters): no public field ends holding a secret.
        entry_line = next(i+1 for i,l in enumerate(SIGNER.read_text().splitlines()) if l.startswith('nostr_sign_step ::'))
        self.assertEqual([(g['file'].endswith('/crypto/schnorr_sign.coax'), g['line']) for g in outputs], [(True, entry_line)])
        self.assertTrue(outputs[0]['origin_mask'] & 1)
        self.assertTrue(declassified and all(g['file'].endswith('/crypto/schnorr_sign.coax') for g in declassified))
        self.assertTrue(all(g['origin_mask'] == 0 for g in guards))
        source = SIGNER.read_text().splitlines()
        for g in declassified:
            self.assertIn('_declassify(', source[g['line']-1], g)

if __name__ == '__main__':
    unittest.main()

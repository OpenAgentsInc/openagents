import copy,importlib.util,json,tempfile,unittest
from pathlib import Path
SPEC=importlib.util.spec_from_file_location('mesh_band_tool',Path(__file__).with_name('check_mesh_bands.py'));m=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(m)
class MeshBandTooling(unittest.TestCase):
 def test_complete_check_identity_and_refusals(self):
  with tempfile.TemporaryDirectory()as d:
   d=Path(d);log=d/'checks.log';log.write_text('ok #test alpha\n21 checks, 21 passed, 0 failed\n');receipt=d/'receipt.json';sources={'/frozen/mesh.coax':'a','/frozen/rules.coax':'b'};command=['coax/bin/coax','test','verse/icecream/mesh.coax','--library','--steps','4000000000'];good=dict(schema='aethyr.icecream.mesh.checks.v1',exit=0,compiler_sha256='compiler',source_sha256=sources,command=command,log_file=str(log),log_sha256=m.sha(log));receipt.write_text(json.dumps(good));self.assertEqual(m.admit_reused_checks(receipt,sources,'compiler',command)['exit'],0)
   mutations=[lambda q:q.update(source_sha256={}),lambda q:q['source_sha256'].pop('/frozen/rules.coax'),lambda q:q.update(command=command+['--only','alpha']),lambda q:q.update(compiler_sha256='other'),lambda q:q.update(exit=1),lambda q:q.update(log_sha256='wrong'),lambda q:q.update(schema='other')]
   for change in mutations:
    q=copy.deepcopy(good);change(q);receipt.write_text(json.dumps(q))
    with self.assertRaises(ValueError):m.admit_reused_checks(receipt,sources,'compiler',command)
   for text in ['1 of 21 checks, 1 passed, 0 failed\n','21 checks, 20 passed, 0 failed\n','FAIL #test alpha\n21 checks, 21 passed, 0 failed\n']:
    log.write_text(text);q=copy.deepcopy(good);q['log_sha256']=m.sha(log);receipt.write_text(json.dumps(q))
    with self.assertRaises(ValueError):m.admit_reused_checks(receipt,sources,'compiler',command)
 def test_independent_patch_refuses_source_drift(self):
  source='prefix\n'+m.CPP_SPANS_OLD+'\n'+m.BAND_OLD+'\n'+m.CPP_POST_OLD+'\nsuffix'
  for anchor in [m.BAND_OLD,m.CPP_SPANS_OLD,m.CPP_POST_OLD]:
   for text in [source.replace(anchor,''),source+'\n'+anchor]:
    with self.assertRaises(ValueError):m.repaired_cpp(text)
  changed=m.repaired_cpp(source);self.assertTrue(changed.startswith('prefix\n'));self.assertTrue(changed.endswith('\nsuffix'));self.assertIn('for (size_t j=lowerBegin;j<upperBegin;++j)',changed);self.assertIn('trimCovered(capBands[0]',changed)
if __name__=='__main__':unittest.main()

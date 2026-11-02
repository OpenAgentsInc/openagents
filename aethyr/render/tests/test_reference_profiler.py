#!/usr/bin/env python3
import unittest
import tempfile
import hashlib
from pathlib import Path
from profile_reference_frame import render_command,instrument,admitted_driver
class ProfilingTransport(unittest.TestCase):
 def test_historical_driver_requires_the_exact_recorded_content(self):
  with tempfile.TemporaryDirectory()as temp:
   path=Path(temp)/'retained.c';path.write_bytes(b'exact recorded transport')
   m={'source_sha256':{'aethyr/render/tests/reference-frame-native.c':hashlib.sha256(path.read_bytes()).hexdigest()}}
   self.assertEqual(admitted_driver(path,m),path.resolve())
   path.write_bytes(b'changed transport')
   with self.assertRaisesRegex(ValueError,'driver differs'):admitted_driver(path,m)
 def test_final_provenance_command_is_not_a_renderer_execution(self):
  exe=Path('/tmp/renderer');draw=[str(exe),'frame.config','image','receipt','nearest-even','8','viewport','0','rgba16f','complex-world'];m={'commands':[['coax','deps','entry'],draw,['coax','source-digest','root']]}
  self.assertEqual(render_command(m,exe),draw)
  with self.assertRaises(ValueError):render_command({'commands':m['commands']+[draw]},exe)
  with self.assertRaises(ValueError):render_command({'commands':m['commands'][:1]},exe)
 def test_instrumentation_replaces_definitions_and_keeps_original_returns(self):
  text='static coax_t_RRVertices coax_p_rr_vertices(int x);\nstatic coax_t_RRShaded coax_p_rr_shade(int x);\nstatic coax_t_RRVertices coax_p_rr_vertices(int x) {return v;}\nstatic coax_t_RRShaded coax_p_rr_shade(int x) {return f;}\n'
  out=instrument(text);self.assertIn('rfc_original_rr_vertices(int x) {return v;}',out);self.assertIn('rfc_original_rr_shade(int x) {return f;}',out)
  self.assertIn('rfc_note(l_tree.data,0,q.steps);',out);self.assertIn('rfc_note(l_tree.data,1,q.steps);',out);self.assertEqual(out.count('return q;}'),2)
  with self.assertRaises(ValueError):instrument(text+text)
if __name__=='__main__':unittest.main()

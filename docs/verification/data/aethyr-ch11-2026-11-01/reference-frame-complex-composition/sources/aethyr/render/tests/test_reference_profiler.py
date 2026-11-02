#!/usr/bin/env python3
import unittest
from pathlib import Path
from profile_reference_frame import render_command,instrument
class ProfilingTransport(unittest.TestCase):
 def test_final_provenance_command_is_not_a_renderer_execution(self):
  exe=Path('/tmp/renderer');draw=[str(exe),'frame.config','image','receipt','nearest-even','8','viewport','0','rgba16f','complex-world'];m={'commands':[['coax','deps','entry'],draw,['coax','source-digest','root']]}
  self.assertEqual(render_command(m,exe),draw)
  with self.assertRaises(ValueError):render_command({'commands':m['commands']+[draw]},exe)
  with self.assertRaises(ValueError):render_command({'commands':m['commands'][:1]},exe)
 def test_instrumentation_replaces_definitions_and_keeps_original_returns(self):
  text='static coax_t_RRVertices coax_p_rr_vertices(int x);\nstatic coax_t_RRShaded coax_p_rr_shade(int x);\nstatic coax_t_RRVertices coax_p_rr_vertices(int x) {return v;}\nstatic coax_t_RRShaded coax_p_rr_shade(int x) {return f;}\n'
  out=instrument(text);self.assertIn('rfc_original_rr_vertices(int x) {return v;}',out);self.assertIn('rfc_original_rr_shade(int x) {return f;}',out)
  self.assertIn('rfc_note(l_tree.data,0,q.steps);return q;',out);self.assertIn('rfc_note(l_tree.data,1,q.steps);return q;',out)
  with self.assertRaises(ValueError):instrument(text+text)
if __name__=='__main__':unittest.main()

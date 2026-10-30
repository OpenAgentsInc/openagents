"""Adversarial checks of measurement completeness, not crypto approval."""
import tempfile
import unittest
from pathlib import Path

from compare import fixtures, observation, verify_sources
from seed_gate import INJECTION, rejects_seed, seed


def xml(kind="UninitCondition", finished=True):
    end = '<status><state>FINISHED</state></status>' if finished else ''
    error = f'''<error><unique>0x0</unique><kind>{kind}</kind>
      <what>Conditional jump depends on uninitialised value</what><stack>
      <frame><ip>0x123</ip><fn>sw_key</fn><file>schnorr_sign.coax</file><line>195</line></frame>
      <frame><ip>0x456</ip><fn>schnorr_sign_step</fn></frame></stack></error>'''
    return '<valgrindoutput><protocolversion>4</protocolversion><protocoltool>memcheck</protocoltool>'+(error if kind else '')+end+'</valgrindoutput>'


class Measurements(unittest.TestCase):
    def test_reports_preserve_context_and_do_not_mean_approval(self):
        r = observation(9, xml(), '{"ok":true}', control=False)
        self.assertEqual(r['secret_flow_contexts'], 1)
        self.assertEqual(r['errors'][0]['frames'][0]['line'], 195)
        self.assertEqual(len(r['errors'][0]['frames']), 2)
        self.assertNotIn('crypto_approved', r)

    def test_missing_report_or_incomplete_run_is_an_error(self):
        for text in ['', '<broken', xml(finished=False), xml().replace('memcheck', 'other'),
                     xml().replace('<protocolversion>4', '<protocolversion>3')]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                observation(9, text, '{"ok":true}', control=False)

    def test_current_valgrind_xml_protocol_is_supported(self):
        current = xml().replace('<protocolversion>4', '<protocolversion>6')
        self.assertEqual(observation(9, current, '{"ok":true}', control=False)['secret_flow_contexts'], 1)

    def test_native_failure_and_unexpected_memory_errors_are_not_leaks(self):
        for code, report, output in [(9, xml(), '{"ok":false}'), (3, xml(), '{"ok":true}'),
                                     (0, xml(), '{"ok":true}'), (9, xml(), 'not json'),
                                     (9, xml('InvalidRead'), '{"ok":true}')]:
            with self.subTest(code=code, output=output), self.assertRaises(ValueError):
                observation(code, report, output, control=False)

    def test_control_must_detect_a_deliberate_leak(self):
        clean = xml(kind=None)
        self.assertEqual(observation(0, clean, '{"ok":true}', control=False)['secret_flow_contexts'], 0)
        with self.assertRaises(ValueError):
            observation(0, clean, '{"ok":true}', control=True)
        self.assertEqual(observation(9, xml(), '{"ok":true}', control=True)['secret_flow_contexts'], 1)

    def test_source_drift_refuses_measurement(self):
        import hashlib
        with tempfile.TemporaryDirectory() as temp:
            p = Path(temp)/'source.coax'; p.write_bytes(b'old')
            hashes = {str(p): hashlib.sha256(b'old').hexdigest()}
            verify_sources(hashes)
            p.write_bytes(b'new')
            with self.assertRaises(ValueError):
                verify_sources(hashes)

    def test_pinned_signing_rows_and_negative_paths_are_present(self):
        cases = fixtures()
        self.assertEqual([c['name'] for c in cases if c['mode']=='valid'],
                         ['vector-0','vector-1','vector-2','vector-3','vector-15','vector-16','vector-17','vector-18'])
        self.assertEqual({len(bytes.fromhex(c['message'])) for c in cases if c['mode']=='valid'}, {0,1,17,32,100})
        self.assertTrue({'key-zero','key-order','key-max','short-key','overlap','message-change','candidate-change','positive-control'} <= {c['mode'] for c in cases})

    def test_seed_changes_only_one_definition_and_refuses_ambiguous_input(self):
        source = '#line 10 "test.coax"\n_Bool sm_equation(coax_v_u8 l_key, int n);\n_Bool sm_equation(coax_v_u8 l_key, int n) {return 1;}\n'
        base, mutant = seed(source)
        self.assertNotIn('#line', base)
        self.assertEqual(mutant.replace(INJECTION, ''), base)
        self.assertEqual(mutant.count(INJECTION), 1)
        for bad in ['', source+source]:
            with self.assertRaises(ValueError):
                seed(bad)

    def test_seed_build_failure_cannot_count_as_detection(self):
        complete = 'calls 6 status 2 sig e907..\n'
        for output in ['', 'compiler failed', 'calls 1 status 0']:
            with self.assertRaises(ValueError):
                rejects_seed([dict(exit_code=0, stdout=complete), dict(exit_code=1, stdout=output)])
        self.assertFalse(rejects_seed([dict(exit_code=0, stdout=complete)]*2))
        self.assertTrue(rejects_seed([dict(exit_code=0, stdout=complete), dict(exit_code=1, stdout=complete)]))


if __name__ == '__main__':
    unittest.main()

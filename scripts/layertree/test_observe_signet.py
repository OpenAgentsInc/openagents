"""Check measurement recovery without loading credentials or running a CLI."""
import ast
from pathlib import Path
import unittest

tree = ast.parse(Path(__file__).with_name('observe-signet.py').read_text())
definition = next(node for node in tree.body
                  if isinstance(node, ast.FunctionDef) and node.name == 'restart_late_window')
namespace = {}
exec(compile(ast.Module(body=[definition], type_ignores=[]), '<measurement>', 'exec'), namespace)
restart = namespace['restart_late_window']
WEEK = 7 * 86400


class Measurement(unittest.TestCase):
    def test_a_late_confirmation_retains_history_and_requires_a_new_full_week(self):
        late = {'txid': 'original-refresh', 'height': 612, 'gap_blocks': 7}
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + WEEK,
                 'last_confirmation_height': 612, 'late_heartbeats': [late],
                 'elapsed_week_observed_at_unix': 100 + WEEK}
        self.assertTrue(restart(state, 800, 2 * WEEK))
        self.assertEqual(state['interrupted_soaks'][0]['began_at_unix'], 100)
        self.assertEqual(state['interrupted_soaks'][0]['late_heartbeats'], [late])
        self.assertEqual(state['late_heartbeats'], [late])
        self.assertEqual(state['soak_began_at_unix'], 800)
        self.assertEqual(state['soak_ends_at_unix'], 800 + WEEK)
        self.assertNotIn('elapsed_week_observed_at_unix', state)
        self.assertFalse(restart(state, 900, 2 * WEEK))
        self.assertEqual(state['soak_began_at_unix'], 800)
        state['late_heartbeats'].append({'txid': 'another-refresh', 'height': 619, 'gap_blocks': 7})
        self.assertTrue(restart(state, 1000, 2 * WEEK))
        self.assertEqual(len(state['interrupted_soaks']), 2)
        self.assertEqual(state['soak_ends_at_unix'], 1000 + WEEK)

    def test_insufficient_time_is_explicit_and_keeps_the_original_kill_deadline(self):
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + WEEK,
                 'kill_at_unix': WEEK, 'deadline_unix': WEEK + 2 * 86400,
                 'last_confirmation_height': 612, 'phase': 'soak',
                 'late_heartbeats': [{'txid': 'original-refresh', 'gap_blocks': 7}]}
        self.assertTrue(restart(state, 2000, WEEK))
        self.assertFalse(state['qualification_window_fits_before_kill'])
        self.assertEqual(state['kill_at_unix'], WEEK)
        self.assertEqual(state['deadline_unix'], WEEK + 2 * 86400)
        self.assertEqual(state['phase'], 'soak')

    def test_a_later_miss_does_not_erase_an_already_completed_clean_week(self):
        completed = {'began_at_unix': 100, 'ended_at_unix': 100 + WEEK,
                     'observed_at_unix': 101 + WEEK, 'observed_height': 2000}
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + WEEK,
                 'last_confirmation_height': 2007, 'qualified_soak': completed,
                 'late_heartbeats': [{'txid': 'later-refresh', 'height': 2007, 'gap_blocks': 7}]}
        self.assertFalse(restart(state, WEEK + 1000, 2 * WEEK))
        self.assertEqual(state['qualified_soak'], completed)
        self.assertEqual(state['soak_began_at_unix'], 100)
        self.assertEqual(len(state['late_heartbeats']), 1)


if __name__ == '__main__':
    unittest.main()

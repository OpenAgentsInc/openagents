"""Regression checks for unknown costs and zero-pass accounting."""
import unittest
from measure import counted_cost, per_pass, measure


class Accounting(unittest.TestCase):
    def usage(self, amount, lower, unknown=0, jev=0.001):
        return {'cost': {'amount_usd': amount, 'lower_bound_usd': lower, 'unknown_calls': unknown},
                'components': {'jev': {'cost_usd': jev}}}

    def test_open_request_counts_the_bound(self):
        self.assertAlmostEqual(counted_cost(self.usage(None, 0.02, 1)), 0.091)
        self.assertEqual(counted_cost(self.usage(None, 0.12, 1)), 0.12)

    def test_missing_evidence_is_not_zero(self):
        with self.assertRaises(ValueError):
            counted_cost(self.usage(None, None, 1))
        with self.assertRaises(ValueError):
            counted_cost(self.usage(None, 0.02, 1, None))
        with self.assertRaises(ValueError):
            counted_cost(self.usage(None, 0.02))

    def test_known_zero_is_distinct(self):
        self.assertEqual(counted_cost(self.usage(0, 0, jev=0)), 0)

    def test_no_pass_has_no_efficiency_ratio(self):
        self.assertIsNone(per_pass(0.1, 0))
        self.assertIsNone(per_pass(120, 0))

    def test_retained_cohort_is_complete_and_keeps_unknowns(self):
        result = measure()
        self.assertEqual(len(result['rows']), 18)
        self.assertEqual(sum(r['any_candidate_passes'] is None for r in result['rows']), 12)
        self.assertEqual(result['totals']['unknown_calls'], 6)
        self.assertTrue(all(r['cost_per_pass_usd'] is None for r in result['tasks']))
        self.assertGreater(result['totals']['counted_cost_usd'], result['totals']['recorded_lower_bound_usd'])
        self.assertLess(result['totals']['counted_cost_usd'], 3)


if __name__ == '__main__':
    unittest.main()

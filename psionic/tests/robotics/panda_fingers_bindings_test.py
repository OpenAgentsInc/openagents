#!/usr/bin/env python3
"""New numerical-proof guards, without SDK/native or old FK execution."""
import copy,json,math,unittest
import panda_fingers_reference as oracle

class ProofGuards(unittest.TestCase):
    def test_comparison_finite_in_both_directions(self):
        for bad in (math.inf,-math.inf,math.nan):
            for actual,expected in (([0.,bad],[0.,0.]),([0.,0.],[0.,bad])):
                with self.subTest(actual=actual,expected=expected):
                    with self.assertRaisesRegex(RuntimeError,'nonfinite'):oracle.errors(actual,expected)
    def test_extent_and_frozen_bound(self):
        for actual,expected in (([],[]),([0.],[]),([0.],[0.,0.])):
            with self.subTest(actual=actual,expected=expected):
                with self.assertRaisesRegex(RuntimeError,'extent'):oracle.errors(actual,expected)
        self.assertTrue(oracle.errors([oracle.ATOL],[0.])['passed'])
        with self.assertRaisesRegex(RuntimeError,'budget exceeded'):oracle.errors([math.nextafter(oracle.ATOL,math.inf)],[0.])
    def test_fractional_zero_error_is_preserved(self):
        value=oracle.errors([0.,-0.,1.],[0.,0.,1.]);self.assertEqual(value['values'],3);self.assertEqual(value['max_budget_fraction'],0.)
    def test_new_sdk_nominal_order_guard(self):
        receipt=json.loads((oracle.FIXTURE/'sdk-reference.json').read_text());names=receipt['body_names'];oracle.verify_sdk_order(receipt,names)
        for kind in ('swapped','missing','site'):
            bad=copy.deepcopy(receipt)
            if kind=='swapped':bad['body_names'][12],bad['body_names'][14]=bad['body_names'][14],bad['body_names'][12]
            if kind=='missing':bad['body_names'].pop()
            if kind=='site':bad['site_name']='robot0_right_hand'
            with self.subTest(kind=kind):
                with self.assertRaisesRegex(RuntimeError,'nominal pose order'):oracle.verify_sdk_order(bad,names)

if __name__=='__main__':unittest.main()

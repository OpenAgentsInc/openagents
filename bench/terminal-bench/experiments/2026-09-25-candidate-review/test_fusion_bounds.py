"""Check that acquisition bounds preserve the frozen classifier's decisions."""
import importlib.util
import itertools
import math
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('bounds',Path(__file__).with_name('fusion_bounds.py'))
bounds=importlib.util.module_from_spec(spec);spec.loader.exec_module(bounds)

class BoundsTest(unittest.TestCase):
    def test_every_missing_or_present_answer_is_inside_the_bounds(self):
        for weights in itertools.product([-2.,-.3,.2,1.7],repeat=4):
            model=dict(features=['a','b'],weights=list(weights),bias=.13,fail_at=.6)
            lo,hi=bounds.bounds(model,{},['a','b'])
            for a,b in itertools.product([None,0.,.1,.5,.9,1.],repeat=2):
                actual,_=bounds.bounds(model,{'a':a,'b':b},[])
                self.assertLessEqual(lo,actual+1e-14)
                self.assertGreaterEqual(hi,actual-1e-14)
                call,_,needed=bounds.verdict(model,{},['a','b'])
                if not needed:
                    self.assertEqual(call,'fail' if actual>=model['fail_at'] else None)

    def test_acquire_only_when_some_answer_can_change_the_call(self):
        model=dict(features=['a'],weights=[1.,0.],bias=0.,fail_at=.6)
        self.assertTrue(bounds.verdict(model,{},['a'])[2])
        model['bias']=2.
        self.assertEqual(bounds.verdict(model,{},['a'])[0],'fail')
        self.assertFalse(bounds.verdict(model,{},['a'])[2])
        model['bias']=-2.
        self.assertIsNone(bounds.verdict(model,{},['a'])[0])
        self.assertFalse(bounds.verdict(model,{},['a'])[2])

    def test_no_observation_abstains_and_invalid_evidence_is_rejected(self):
        model=dict(features=['a'],weights=[1.,0.],bias=10.,fail_at=.6)
        self.assertIsNone(bounds.verdict(model,{},[])[0])
        for value in [math.nan,math.inf,-.1,1.1]:
            with self.assertRaises(ValueError):bounds.bounds(model,{'a':value},[])
        with self.assertRaises(ValueError):bounds.bounds(model,{},['unrecognized'])

if __name__=='__main__':unittest.main()

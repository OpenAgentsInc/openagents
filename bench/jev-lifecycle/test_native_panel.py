"""Check serial admission and accounting stops without provider calls."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run_native_panel as panel


class PanelTests(unittest.TestCase):
    def exercise(self, values):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        master = root/'master'; master.write_text('synthetic-test-token'); master.chmod(0o600)
        probe = root/'probe.json'; probe.write_text(json.dumps(dict(model_completed=True,
            execution_closed=True, provider_accounting_complete=True,
            model='claude-sonnet-5-5', served_models=['claude-sonnet-5-5'])))
        schedule=[]
        for i,(task,rep,arm) in enumerate(panel.EXPECTED):
            config=root/f'config-{i}.json'
            config.write_text(json.dumps(dict(run_id=str(i),task_id=task,arm=arm,credential_file=str(root/'token'))))
            schedule.append(dict(position=i+1,run_id=str(i),task_id=task,repetition=rep,arm=arm,
                config=dict(path=str(config),sha256=panel.pilot.sha(config.read_bytes())),output=str(root/f'run-{i}')))
        plan=root/'plan.json'; plan.write_text(json.dumps(dict(status='frozen_before_execution',
            schedule=schedule,capability_probe=dict(path=str(probe.resolve()),sha256=panel.pilot.sha(probe.read_bytes())),policy=dict(panel_observed_cost_admission_stop_usd=24))))
        launched=[]
        def execute(name,argv,output,timeout):
            i=len(launched); launched.append(i)
            row=values[min(i,len(values)-1)]
            (root/'token').unlink()
            out=Path(argv[-1]);out.mkdir()
            (out/'pilot.json').write_text(json.dumps(dict(run_id=str(i),accepted=False,**row)))
            return dict(exit_code=0,timed_out=False)
        with patch.object(panel.pilot,'validate'),patch.object(panel.pilot,'verify_modules'),patch.object(panel.pilot.trial,'process_phase',side_effect=execute):
            result=panel.run(plan,root/'panel',master,probe)
        return result,launched

    def test_failed_patches_still_count_and_continue(self):
        result,launched=self.exercise([dict(safe_to_continue=True,accounting_complete=True,cost_upper_usd=.1)])
        self.assertEqual(result['status'],'complete');self.assertEqual(len(launched),12)
        self.assertAlmostEqual(result['cost_upper_usd'],1.2)

    def test_overshoot_retained_then_stops(self):
        result,launched=self.exercise([dict(safe_to_continue=True,accounting_complete=True,cost_upper_usd=25)])
        self.assertEqual(result['status'],'stopped_cost_admission');self.assertEqual(len(launched),1)
        self.assertEqual(result['cost_upper_usd'],25)

    def test_unknown_cost_stops(self):
        result,launched=self.exercise([dict(safe_to_continue=False,accounting_complete=False,cost_upper_usd=None)])
        self.assertEqual(result['status'],'stopped_unknown_cost');self.assertEqual(len(launched),1)
        self.assertIsNone(result['cost_upper_usd'])

    def test_unclosed_or_failed_cleanup_stops(self):
        result,launched=self.exercise([dict(safe_to_continue=False,accounting_complete=True,cost_upper_usd=.1)])
        self.assertEqual(result['status'],'stopped_incomplete_attempt');self.assertEqual(len(launched),1)

    def test_schedule_change_refused(self):
        with self.assertRaises(ValueError):
            panel.validate_plan(dict(status='frozen_before_execution',schedule=[]))


if __name__=='__main__':unittest.main()

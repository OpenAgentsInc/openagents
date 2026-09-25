"""The cheap arm only reads instruction-derived, retained output paths."""
import copy
import json
from pathlib import Path
import unittest

from archive_checks import narrow, plan_digest


class PlanTests(unittest.TestCase):
    def test_retained_rust_plan_digest_matches_before_selection(self):
        path = Path(__file__).parent.parent / '2026-09-25-executed-contract-checks/records/sound-change-cascade/plan.json'
        plan = json.loads(path.read_text())
        selected = narrow(plan)
        self.assertEqual([i['id'] for i in selected['items']], ['K1', 'K2', 'K3', 'K4'])
        self.assertNotEqual(selected['digest'], plan['digest'])
        self.assertEqual(selected['digest'], plan_digest(selected))
        bad = copy.deepcopy(plan)
        bad['items'][0]['path'] = '/app/other'
        with self.assertRaises(ValueError):
            narrow(bad)

    def test_never_runs_commands_or_reads_unretained_or_candidate_authored_paths(self):
        base = {'id': 'K1', 'kind': 'path', 'source': 'instruction',
                'path': '/app/result.json', 'span': 'write /app/result.json',
                'expect': {'expect': 'exists'}}
        variants = [base,
                    dict(base, kind='command', command='touch /app/changed'),
                    dict(base, command='echo ignored'), dict(base, source='/app/README.md'),
                    dict(base, path='/outside/result.json'), dict(base, path='/app/../outside'),
                    dict(base, not_executable='optional')]
        plan = {'schema': 'openagents.coder-one.contract-plan.v1', 'items': variants,
                'task': 'fixture', 'workdir': '/app', 'instruction': 'fixture', 'jev': []}
        plan['digest'] = plan_digest(plan)
        self.assertEqual(narrow(plan)['items'], [base])


if __name__ == '__main__':
    unittest.main()

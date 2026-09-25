"""Unknown prices remain explicit, and retained replies are counted once."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class CostTests(unittest.TestCase):
    def test_unknown_price_and_repeated_reply(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            records = root / 'records/reproduced'
            records.mkdir(parents=True)
            reply = {'id': 'fixture-response', 'model': 'fixture-model',
                     'usage': {'input': 3, 'output': 2}, 'cost_usd': None}
            (records / 'reply-1.json').write_text(json.dumps(reply))
            (records / 'reply-copy.json').write_text(json.dumps(reply))
            output = root / 'costs.json'
            command = [sys.executable, str(Path(__file__).with_name('costs.py')),
                       str(root / 'records'), str(output)]
            subprocess.run(command, capture_output=True, text=True, check=True)
            ledger = json.loads(output.read_text())
            self.assertEqual(ledger['native_requests_with_usage'], 1)
            self.assertEqual(ledger['native_responses_with_unknown_cost'], ['fixture-response'])
            self.assertEqual(len(ledger['native_cost_unknown_records']), 2)
            self.assertEqual(ledger['known_native_list_price_usd'], 0)
            reply['cost_usd'] = 1.25
            (records / 'reply-copy.json').write_text(json.dumps(reply))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('conflicting usage', result.stderr)


if __name__ == '__main__':
    unittest.main()

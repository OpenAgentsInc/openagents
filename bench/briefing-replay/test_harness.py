#!/usr/bin/env python3
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
import replay
import verify_remote
import instruction_guard

FAKE = r'''import json, os, sys
print(json.dumps({'type':'system','subtype':'init','tools':['Read','Edit','Write','Glob','Grep'],'model':'claude-opus-5-5'}), flush=True)
for n, line in enumerate(sys.stdin, 1):
    mode = os.environ.get('REPLAY_FAKE_MODE', '')
    if mode == 'broken-repair' and n == 2:
        raise SystemExit(7)
    if mode == 'invalid-json':
        print('invalid JSON', flush=True)
        continue
    event = {'type':'result','subtype':'success','is_error':False,'total_cost_usd':0.25 if n == 1 else 0.4}
    if mode == 'model-error':
        event['is_error'] = True
        event['subtype'] = 'error_during_execution'
    if mode == 'malformed-result':
        del event['subtype']
    print(json.dumps(event), flush=True)
'''

class HarnessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='replay-synthetic-')
        self.root = Path(self.tmp.name).resolve()
        self.repo = self.root / 'source'
        self.repo.mkdir()
        (self.repo / 'AGENTS.md').write_text('Keep this complete instruction.\n')
        (self.repo / 'src').mkdir()
        (self.repo / 'src/lib.rs').write_text('pub fn sample() {}\n')
        self.git('init', '-q')
        self.git('add', '.')
        self.git('-c', 'user.name=Synthetic', '-c', 'user.email=synthetic@example.invalid', '-c', 'core.hooksPath=/dev/null', 'commit', '-qm', 'Synthetic baseline')
        self.commit = self.git('rev-parse', 'HEAD').strip()
        manifest, block = instruction_guard.freeze(self.repo, ['AGENTS.md'], self.commit)
        for name, data in [('manifest.json', json.dumps(manifest).encode()), ('block.txt', block), ('instructions.txt', b'Operational preface.\n' + block), ('task.txt', b'Change sample behavior.\n'), ('brief.txt', b'Optional source evidence.\n')]:
            (self.root / name).write_bytes(data)
        bindir = self.root / 'bin'
        bindir.mkdir()
        cli = bindir / 'claude'
        cli.write_text('#!' + sys.executable + '\n' + FAKE)
        cli.chmod(0o700)
        self.config = {'source_repo': str(self.repo), 'source_commit': self.commit, 'workspace_parent': str(self.root / 'workspaces'), 'instruction_manifest': str(self.root / 'manifest.json'), 'instruction_block': str(self.root / 'block.txt'), 'instructions_file': str(self.root / 'instructions.txt'), 'task_file': str(self.root / 'task.txt'), 'brief_file': str(self.root / 'brief.txt'), 'allowed_roots':['src'], 'remote_source_repo':str(self.repo)}
        self.env = mock.patch.dict(os.environ, {'PATH':str(bindir) + os.pathsep + os.environ['PATH'], 'REPLAY_FAKE_MODE':''})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.repo, text=True, stderr=subprocess.DEVNULL)

    def execute(self, verifier, mode='', condition='control', warmup=False):
        output = self.root / ('output-' + str(len(list(self.root.glob('output-*')))))
        with mock.patch.object(replay, 'verify', side_effect=verifier), mock.patch.dict(os.environ, {'REPLAY_FAKE_MODE':mode}), contextlib.redirect_stdout(io.StringIO()):
            result = replay.run(self.config, condition, output, warmup)
        self.assertEqual(json.loads((output/'result.json').read_text()), result)
        return result

    def passed(self, *args):
        return {'passed': True, 'checks':[]}

    def test_verifier_error_retains_finished_charge(self):
        result = self.execute(RuntimeError('Synthetic verifier failure'))
        self.assertEqual(result['cost_usd_list_estimate'], 0.25)
        self.assertTrue(result['cost_complete'])
        self.assertFalse(result['accepted'])
        self.assertEqual(result['attempts'][0]['verification_error']['type'], 'RuntimeError')
        self.assertIn('retained_workspace', result)

    def test_cumulative_cost_is_not_summed(self):
        result = self.execute([{'passed':False, 'checks':[{'name':'acceptance','exit_code':1,'log':'Synthetic failure'}]}, {'passed':True,'checks':[]}])
        self.assertTrue(result['accepted'])
        self.assertEqual(result['cost_usd_list_estimate'], 0.4)
        self.assertAlmostEqual(result['repair_cost_usd_list_estimate'], 0.15)
        self.assertEqual(result['input_turns_sent'], 2)
        self.assertNotIn('retained_workspace', result)

    def test_interrupted_repair_retains_lower_bound(self):
        result = self.execute([{'passed':False,'checks':[]}], mode='broken-repair')
        self.assertFalse(result['accepted'])
        self.assertFalse(result['cost_complete'])
        self.assertEqual(result['cost_status'], 'incomplete_lower_bound')
        self.assertEqual(result['cost_usd_list_estimate'], 0.25)
        self.assertEqual(result['input_turns_sent'], 2)
        self.assertEqual(result['session_results'], 1)

    def test_error_or_malformed_result_cannot_be_accepted(self):
        for mode in ['model-error', 'malformed-result', 'invalid-json']:
            with self.subTest(mode=mode):
                result = self.execute(self.passed, mode=mode)
                self.assertFalse(result['accepted'])
                self.assertFalse(result['completed'])

    def test_instruction_changes_are_rejected_after_verification(self):
        def mutate(config, workspace, *args):
            (workspace/'AGENTS.md').write_text('Shortened.\n')
            return {'passed':True,'checks':[]}
        result = self.execute(mutate)
        self.assertFalse(result['accepted'])
        self.assertIn('instruction integrity', result['attempts'][0]['verification']['scope_failure'])

    def test_invalid_instruction_block_stops_before_executor(self):
        (self.root/'instructions.txt').write_text('Shortened instructions.\n')
        result = self.execute(self.passed)
        self.assertFalse(result['executor_started'])
        self.assertEqual(result['cost_usd_list_estimate'], 0)
        self.assertEqual(result['cost_status'], 'not_started')

    def test_prompt_prefix_and_warmup_share_instruction_files(self):
        control = self.execute(self.passed)
        treatment = self.execute(self.passed, condition='treatment')
        warmup = self.execute(self.passed, warmup=True)
        self.assertEqual(control['common_input_sha256'], treatment['common_input_sha256'])
        self.assertEqual(control['instructions_sha256'], warmup['instructions_sha256'])
        self.assertTrue(warmup['completed'])
        self.assertEqual(warmup['attempts'][0].get('verification'), None)

    def test_pin_lock_revision_and_allowed_roots(self):
        request = {'config':self.config, 'remote_root':str(self.root/'remote')}
        with verify_remote.pinned_repo(request) as paths:
            self.assertEqual((paths[1]/'AGENTS.md').read_text(), 'Keep this complete instruction.\n')
            with self.assertRaisesRegex(RuntimeError, 'holds this benchmark root'):
                with verify_remote.pinned_repo(request):
                    pass
        with verify_remote.pinned_repo(request):
            pass
        changed = {'config':dict(self.config, allowed_roots=['AGENTS.md']), 'remote_root':request['remote_root']}
        with self.assertRaisesRegex(ValueError, 'source pin'):
            with verify_remote.pinned_repo(changed):
                pass
        (self.repo/'AGENTS.md').write_text('New source.\n')
        self.git('add', '.')
        self.git('-c', 'user.name=Synthetic', '-c', 'user.email=synthetic@example.invalid', '-c', 'core.hooksPath=/dev/null', 'commit', '-qm', 'Next source')
        changed['config'] = dict(self.config, source_commit=self.git('rev-parse','HEAD').strip())
        with self.assertRaisesRegex(ValueError, 'source pin'):
            with verify_remote.pinned_repo(changed):
                pass

    def test_existing_unpinned_repo_and_traversal_are_rejected(self):
        root = self.root/'remote'
        (root/'repo').mkdir(parents=True)
        request = {'config':self.config, 'remote_root':str(root)}
        with self.assertRaisesRegex(ValueError, 'source pin'):
            with verify_remote.pinned_repo(request):
                pass
        for name in ['../escape', '/escape', 'src/../escape', './src', '.']:
            self.assertFalse(replay.allowed(name, ['src']))
            with self.assertRaises(ValueError):
                verify_remote.relative_path(name)
        (self.repo/'src/link').symlink_to(self.root)
        with self.assertRaises(ValueError):
            verify_remote.inside(self.repo, 'src/link/escape')

if __name__ == '__main__':
    unittest.main()

"""Check measurement recovery without loading credentials or running a CLI."""
import ast
from pathlib import Path
from datetime import datetime, timezone, timedelta
from types import SimpleNamespace
from unittest.mock import Mock, patch
import json
import unittest

tree = ast.parse(Path(__file__).with_name('observe-signet.py').read_text())
definitions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and
               node.name in {'restart_late_window', 'use_24_hour_window', 'schedule_qualified_kill',
                             'qualify_soak'}]
namespace = {}
exec(compile(ast.Module(body=definitions, type_ignores=[]), '<measurement>', 'exec'), namespace)
restart = namespace['restart_late_window']
DAY = 86400


class Measurement(unittest.TestCase):
    def test_qualification_requires_24_hours_and_a_recent_confirmed_heartbeat(self):
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + DAY,
                 'last_confirmation_height': 612}
        qualify = namespace['qualify_soak']
        self.assertFalse(qualify(state, 99 + DAY, 615, 12))
        self.assertNotIn('qualified_soak', state)
        self.assertFalse(qualify(state, 100 + DAY, 618, 12))
        self.assertNotIn('qualified_soak', state)
        self.assertTrue(qualify(state, 100 + DAY, 615, 12))
        self.assertEqual(state['qualified_soak']['ended_at_unix'], 100 + DAY)

    def test_a_late_confirmation_retains_history_and_requires_a_new_full_day(self):
        late = {'txid': 'original-refresh', 'height': 612, 'gap_blocks': 7}
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + DAY,
                 'last_confirmation_height': 612, 'late_heartbeats': [late],
                 'elapsed_week_observed_at_unix': 100 + DAY}
        self.assertTrue(restart(state, 800, 2 * DAY))
        self.assertEqual(state['interrupted_soaks'][0]['began_at_unix'], 100)
        self.assertEqual(state['interrupted_soaks'][0]['late_heartbeats'], [late])
        self.assertEqual(state['late_heartbeats'], [late])
        self.assertEqual(state['soak_began_at_unix'], 800)
        self.assertEqual(state['soak_ends_at_unix'], 800 + DAY)
        self.assertNotIn('elapsed_week_observed_at_unix', state)
        self.assertFalse(restart(state, 900, 2 * DAY))
        self.assertEqual(state['soak_began_at_unix'], 800)
        state['late_heartbeats'].append({'txid': 'another-refresh', 'height': 619, 'gap_blocks': 7})
        self.assertTrue(restart(state, 1000, 2 * DAY))
        self.assertEqual(len(state['interrupted_soaks']), 2)
        self.assertEqual(state['soak_ends_at_unix'], 1000 + DAY)

    def test_insufficient_time_is_explicit_and_keeps_the_original_kill_deadline(self):
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + DAY,
                 'kill_at_unix': DAY, 'deadline_unix': DAY + 2 * 86400,
                 'last_confirmation_height': 612, 'phase': 'soak',
                 'late_heartbeats': [{'txid': 'original-refresh', 'gap_blocks': 7}]}
        self.assertTrue(restart(state, 2000, DAY))
        self.assertFalse(state['qualification_window_fits_before_kill'])
        self.assertEqual(state['kill_at_unix'], DAY)
        self.assertEqual(state['deadline_unix'], DAY + 2 * 86400)
        self.assertEqual(state['phase'], 'soak')

    def test_a_later_miss_does_not_erase_an_already_completed_clean_day(self):
        completed = {'began_at_unix': 100, 'ended_at_unix': 100 + DAY,
                     'observed_at_unix': 101 + DAY, 'observed_height': 2000}
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + DAY,
                 'last_confirmation_height': 2007, 'qualified_soak': completed,
                 'late_heartbeats': [{'txid': 'later-refresh', 'height': 2007, 'gap_blocks': 7}]}
        self.assertFalse(restart(state, DAY + 1000, 2 * DAY))
        self.assertEqual(state['qualified_soak'], completed)
        self.assertEqual(state['soak_began_at_unix'], 100)
        self.assertEqual(len(state['late_heartbeats']), 1)

    def test_shortening_retains_start_and_interrupted_history(self):
        interrupted = [{'began_at_unix': 1, 'planned_end_at_unix': 7 * DAY}]
        state = {'soak_began_at_unix': 100, 'soak_ends_at_unix': 100 + 7 * DAY,
                 'kill_at_unix': 8 * DAY, 'interrupted_soaks': interrupted,
                 'late_heartbeats': [{'txid': 'late'}]}
        namespace['use_24_hour_window'](state)
        self.assertEqual(state['soak_began_at_unix'], 100)
        self.assertEqual(state['soak_ends_at_unix'], 100 + DAY)
        self.assertEqual(state['previous_soak_ends_at_unix'], 100 + 7 * DAY)
        self.assertEqual(state['interrupted_soaks'], interrupted)
        self.assertEqual(state['late_heartbeats'], [{'txid': 'late'}])
        namespace['use_24_hour_window'](state)
        self.assertEqual(state['previous_soak_ends_at_unix'], 100 + 7 * DAY)

    def test_deletion_waits_for_qualification_and_reuses_time_after_unknown_reply(self):
        run = Mock(return_value=SimpleNamespace(returncode=1, stderr='response lost'))
        save = Mock()
        namespace.update(args=SimpleNamespace(kill_scheduler=Path('/scheduler'), kill_at=8 * DAY),
                         save=save, state_path=Path('/state'),
                         subprocess=SimpleNamespace(run=run), datetime=datetime,
                         timezone=timezone, json=json)
        schedule = namespace['schedule_qualified_kill']
        state = {}
        schedule(state, DAY)
        run.assert_not_called()
        state['qualified_soak'] = {'began_at_unix': 0, 'ended_at_unix': DAY}
        with self.assertRaises(RuntimeError):
            schedule(state, DAY)
        self.assertEqual(state['qualified_kill_at_unix'], DAY + 300)
        save.assert_called_once()
        first = run.call_args
        run.return_value = SimpleNamespace(returncode=0, stdout='{"name":"task-clean24"}')
        schedule(state, DAY + 100)
        self.assertEqual(run.call_args, first)
        self.assertEqual(state['qualified_kill_task'], 'task-clean24')
        schedule(state, DAY + 200)
        self.assertEqual(run.call_count, 2)


class KillScheduling(unittest.TestCase):
    def exercise(self, *, wrong_instance=False, existing=False, fail_create=False):
        source = Path(__file__).with_name('schedule-kill.sh').read_text()
        source = source.split("reschedule)\n", 1)[1].split("<<'PY'\n", 1)[1].split('\nPY\n', 1)[0]
        timestamp = (datetime.now(timezone.utc) + timedelta(minutes=5)).replace(microsecond=0)
        instance = '6613632645341139193'
        name = f'delete-{instance}-clean24'
        task = {'name': name, 'scheduleTime': timestamp.isoformat(), 'httpRequest': {
            'url': 'https://compute.googleapis.com/compute/v1/projects/openagentsgemini/zones/us-central1-a/instances/oa-layertree-signet-tdx',
            'httpMethod': 'DELETE', 'oauthToken': {
                'serviceAccountEmail': 'oa-layertree-kill@openagentsgemini.iam.gserviceaccount.com'}}}
        tasks = {f'delete-{instance}': {'name': f'delete-{instance}'}}
        if existing:
            tasks[name] = task
        actions = []

        def run(command, **_kwargs):
            actions.append(command)
            if command[1:4] == ['compute', 'instances', 'describe']:
                return SimpleNamespace(returncode=0, stdout='different' if wrong_instance else instance)
            action, task_name = command[2:4]
            if action == 'describe':
                if task_name not in tasks:
                    return SimpleNamespace(returncode=1, stderr='NOT_FOUND')
                return SimpleNamespace(returncode=0, stdout=json.dumps(tasks[task_name]))
            if action == 'create-http-task':
                if fail_create:
                    return SimpleNamespace(returncode=1, stderr='unavailable')
                tasks[task_name] = task
            elif action == 'delete':
                del tasks[task_name]
            else:
                self.fail(command)
            return SimpleNamespace(returncode=0, stdout='')

        with patch('subprocess.run', side_effect=run), patch('sys.argv', ['schedule', timestamp.isoformat()]), patch('builtins.print'):
            try:
                exec(compile(source, '<kill-scheduler>', 'exec'), {})
            except (AssertionError, RuntimeError):
                self.assertTrue(wrong_instance or fail_create)
            else:
                self.assertFalse(wrong_instance or fail_create)
        return tasks, actions

    def test_replacement_is_created_and_verified_before_old_task_is_deleted(self):
        tasks, actions = self.exercise()
        self.assertEqual(list(tasks), ['delete-6613632645341139193-clean24'])
        names = [action[2] for action in actions]
        self.assertLess(names.index('create-http-task'), names.index('delete'))

    def test_uncertain_reply_reuses_existing_task(self):
        tasks, actions = self.exercise(existing=True)
        self.assertEqual(len(tasks), 1)
        self.assertFalse(any(action[2] == 'create-http-task' for action in actions))

    def test_wrong_vm_never_creates_or_deletes_tasks(self):
        tasks, actions = self.exercise(wrong_instance=True)
        self.assertEqual(list(tasks), ['delete-6613632645341139193'])
        self.assertEqual(len(actions), 1)

    def test_failed_replacement_preserves_original_backstop(self):
        tasks, actions = self.exercise(fail_create=True)
        self.assertEqual(list(tasks), ['delete-6613632645341139193'])
        self.assertFalse(any(action[2] == 'delete' for action in actions))


if __name__ == '__main__':
    unittest.main()

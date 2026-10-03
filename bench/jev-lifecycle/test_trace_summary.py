"""Synthetic traces verify descriptive counts without exposing private text."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import uuid

import trace_summary as trace


def assistant(mid, content, parent=None, **extra):
    return {'type':'assistant','message':{'id':mid,'content':content},'parent_tool_use_id':parent,**extra}


def tool(tid, name, inputs=None):
    return {'type':'tool_use','id':tid,'name':name,'input':inputs or {}}


def result(tid, failed=False):
    return {'type':'user','message':{'content':[{'type':'tool_result','tool_use_id':tid,'is_error':failed,'content':'PRIVATE OUTPUT'}]}}


class TraceTests(unittest.TestCase):
    def summary(self, events, suffix=b''):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'events.jsonl'
            path.write_bytes(b''.join(json.dumps(e).encode()+b'\n' for e in events)+suffix)
            return trace.summarize(path)

    def test_message_and_tool_ids_deduplicate_partial_records_and_keep_child_lane(self):
        message=assistant('m1',[tool('t1','Read',{'file_path':'/PRIVATE/path'})])
        full=assistant('m1',[tool('t1','Read',{'file_path':'/PRIVATE/path'}),tool('t2','Grep')])
        observed=self.summary([message,full,full,assistant('m2',[tool('t3','Edit')],parent='delegated'),result('t1'),result('t1'),
            {'type':'stream_event','event':{'type':'content_block_delta'}}, {'type':'result','num_turns':9}])
        self.assertEqual(observed['unique_assistant_message_ids'],2)
        self.assertEqual(observed['assistant_records'],4)
        self.assertEqual(observed['tool_use_occurrences'],3)
        self.assertEqual(observed['lanes']['root']['tool_use_occurrences'],2)
        self.assertEqual(observed['lanes']['child']['direct_edit_tools'],1)
        self.assertEqual(observed['tool_results']['observed_ids'],1)
        self.assertEqual(observed['cli_reported_num_turns'],9)
        self.assertNotIn('PRIVATE',json.dumps(observed))

    def test_bash_multi_actions_and_exact_repeats_are_separate_from_call_count(self):
        command='cd /workspace && cargo fmt --check && cargo +1.97.1 test -p synthetic; rg TODO src'
        observed=self.summary([assistant('m1',[tool('t1','Bash',{'command':command})]),result('t1',True),
            assistant('m2',[tool('t2','Edit')]),assistant('m3',[tool('t3','Bash',{'command':command})]),result('t3')])
        bash=observed['bash']
        self.assertEqual(bash['calls'],2)
        self.assertEqual(bash['literal_requested_actions']['cargo_fmt'],2)
        self.assertEqual(bash['literal_requested_actions']['cargo_test'],2)
        self.assertEqual(bash['calls_by_requested_category']['cargo_test'],2)
        self.assertEqual(bash['repeat_occurrences_after_first'],1)
        group=bash['repeated_exact_payload_groups'][0]
        self.assertEqual(group['tool_ordinals'],[1,3])
        self.assertEqual(group['observed_error_results'],1)
        self.assertNotIn(command,json.dumps(observed))
        self.assertIn('not a waste estimate',' '.join(observed['limits']))

    def test_quoted_mentions_are_not_command_positions_and_wrappers_are_bounded(self):
        counts,complete=trace.command_requests("printf '%s' '&&' cargo test; echo 'cargo check'; env CARGO_HOME=/tmp timeout 30 cargo check; bash -lc 'cargo fmt && cargo test'")
        self.assertTrue(complete)
        self.assertEqual(counts['cargo_check'],1)
        self.assertEqual(counts['cargo_fmt'],1)
        self.assertEqual(counts['cargo_test'],1)
        self.assertEqual(trace.command_requests('echo cargo test # cargo check\ncargo build')[0]['cargo_test'],0)
        self.assertEqual(trace.command_requests('echo cargo test # cargo check\ncargo build')[0]['cargo_build'],1)
        counts,complete=trace.command_requests('cargo test>/tmp/log 2>&1 | tail -20; cargo check &>/tmp/check')
        self.assertTrue(complete)
        self.assertEqual(counts['cargo_test'],1)
        self.assertEqual(counts['cargo_check'],1)
        self.assertEqual(counts['literal_command_positions'],3)

    def test_complex_shell_forms_abstain_instead_of_inventing_executions(self):
        for command in ("python3 - <<'PY'\ncargo test\nPY", 'for item in a b; do cargo test; done',
                        'echo $(cargo test)', 'eval "cargo test"', "bash -c 'unterminated"):
            with self.subTest(command=command):
                counts,complete=trace.command_requests(command)
                self.assertFalse(complete)
                self.assertEqual(counts['cargo_test'],0)
        counts,complete=trace.command_requests('python3 /PRIVATE/script.py && cargo test')
        self.assertTrue(complete)
        self.assertEqual(counts['opaque_script'],1)
        self.assertEqual(counts['cargo_test'],1)

    def test_missing_ids_malformed_tail_and_conflicting_command_stay_visible(self):
        observed=self.summary([assistant(None,[tool(None,'Read')]),
            assistant('m1',[tool('same','Bash',{'command':'cargo test'})]),
            assistant('m1',[tool('same','Bash',{'command':'cargo build'})])],b'{invalid')
        self.assertEqual(observed['status'],'partial')
        self.assertFalse(observed['assistant_count_has_all_message_ids'])
        self.assertFalse(observed['tool_counts_have_all_ids'])
        self.assertEqual(observed['parse_errors']['tool_input_conflict'],1)
        self.assertEqual(observed['parse_errors']['malformed_record'],1)
        self.assertNotIn('cargo_test',observed['bash']['literal_requested_actions'])

    def test_unknown_tool_names_and_outputs_are_never_published(self):
        observed=self.summary([assistant('m1',[tool('t1','PRIVATE_TOOL_SECRET'),tool('t2',['malformed'])]),result('t1')])
        self.assertEqual(observed['lanes']['root']['tool_names'],{'Other':2})
        self.assertNotIn('PRIVATE',json.dumps(observed))

    def test_missing_stream_is_not_zero_and_byte_bounds_are_explicit(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'events.jsonl'
            self.assertFalse(trace.summarize(path)['available'])
            path.write_bytes(b'{}\n'+b' '*100)
            with patch.object(trace,'MAX_LINE',20):observed=trace.summarize(path)
            self.assertEqual(observed['status'],'partial')
            self.assertIsNone(observed['source_sha256'])
            self.assertEqual(observed['bytes_scanned'],3)
        observed=self.summary([{'type':'stream_event','event':{}}])
        self.assertIn('only_stream_deltas_without_assistant_records',observed['parse_errors'])

    def test_panel_preserves_missing_attempts_and_never_uses_remote_output_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);runs=root/'runs';rid=str(uuid.uuid4())
            row={'run_id':rid,'position':1,'task_id':'alternative-beta','repetition':1,'arm':'bare','output':'/PRIVATE/remote'}
            plan=root/'plan.json';plan.write_text(json.dumps({'schedule':[row]}))
            report=trace.build(plan,runs)
            self.assertEqual(report['scheduled_attempts'],1)
            self.assertEqual(report['available_traces'],0)
            self.assertFalse(report['rows'][0]['trace']['available'])
            self.assertNotIn('/PRIVATE',json.dumps(report))


if __name__=='__main__':unittest.main()

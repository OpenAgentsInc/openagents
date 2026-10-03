"""Check metadata admission, source identities, and equal bounded rendering."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import context
import spans
from test_context import Fixture, small_files, small_declarations


def fixture(directory, files=None, declarations=None):
    files = small_files() if files is None else files
    item = Fixture(directory, files, small_declarations() if declarations is None else declarations)
    for entry in item.index['files']:
        lines = files[entry['path']].splitlines(keepends=True)
        entry['line_count'] = len(lines)
        for decl in entry.get('syntax', {}).get('declarations', []):
            rng = decl['declaration']
            rng['start_byte'] = len(''.join(lines[:rng['start_line'] - 1]).encode())
            rng['end_byte'] = len(''.join(lines[:rng['end_line']]).encode())
            raw = files[entry['path']].encode()
            decl['signature'] = {'start_byte': rng['start_byte'], 'end_byte': raw.index(b'{', rng['start_byte'])}
    return item


def catalog(item):
    return spans.catalog(item.repo, item.rev, item.index, item.task)


class SpanTests(unittest.TestCase):
    def test_metadata_keeps_functions_without_body_budget_or_private_fields(self):
        files = small_files()
        files['crates/widget/src/lib.rs'] = 'fn change() { /* BODY_ONLY_SECRET */ }\nfn other() {}\n'
        declarations = small_declarations(); declarations['crates/widget/src/lib.rs'] = [('change', 1, 1), ('other', 2, 2)]
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory, files, declarations)
            item.task.update(checker='PRIVATE CHECKER', reference='PRIVATE FIX')
            value = catalog(item)
            self.assertEqual(len(value['candidates']), 3)
            self.assertNotIn('BODY_ONLY', json.dumps(value))
            self.assertNotIn('PRIVATE', json.dumps(spans.state(value)))
            self.assertEqual({r['role'] for r in value['candidates']}, {'implementation', 'test'})

    def test_role_caps_and_file_diversity_have_explicit_omissions(self):
        files = small_files(); declarations = small_declarations()
        files['crates/widget/src/lib.rs'] = ''.join(f'fn widget_change_{i}() {{}}\n' for i in range(90))
        declarations['crates/widget/src/lib.rs'] = [(f'widget_change_{i}', i + 1, i + 1) for i in range(90)]
        files['crates/widget/src/neighbor.rs'] = 'fn minor() {}\n'
        declarations['crates/widget/src/neighbor.rs'] = [('minor', 1, 1)]
        files['outside.rs'] = 'fn widget_change() {}\n'; declarations['outside.rs'] = [('widget_change', 1, 1)]
        with tempfile.TemporaryDirectory() as directory:
            value = catalog(fixture(directory, files, declarations))
            self.assertEqual(value['coverage']['roles']['implementation'], 64)
            self.assertIn('minor', {r['name'] for r in value['candidates']})
            self.assertNotIn('outside.rs', {r['path'] for r in value['candidates']})
            self.assertEqual(value['coverage']['omitted_units'], 27)
            self.assertTrue(all(r['reason'] == 'role_catalog_limit' for r in value['omissions']))

    def test_public_operations_are_not_crowded_out_by_constructors_or_getters(self):
        files = small_files(); declarations = small_declarations()
        lines = [f"pub fn get_widget_{i}(&self) -> &str {{ todo!() }}\n" for i in range(90)]
        lines += ["pub fn new(value: Value) -> Self { todo!() }\n",
                  "pub fn interpret(value: Value) -> Result<Self> { todo!() }\n",
                  "pub async fn send(&self, request: Request) -> Result<Reply> { todo!() }\n",
                  "pub fn close(&mut self) { }\n", "fn widget_lifecycle() { }\n"]
        files['crates/widget/src/lib.rs'] = ''.join(lines)
        declarations['crates/widget/src/lib.rs'] = [(f"get_widget_{i}", i+1, i+1) for i in range(90)] + [(n, i, i) for n, i in [('new',91),('interpret',92),('send',93),('close',94),('widget_lifecycle',95)]]
        with tempfile.TemporaryDirectory() as directory:
            value = catalog(fixture(directory, files, declarations))
            for name in ['interpret', 'send', 'close']:
                self.assertEqual(next(r for r in value['candidates'] if r['name'] == name)['public_api_tier'], 2)
            self.assertNotIn('new', {r['name'] for r in value['candidates']})
            self.assertIn('widget_lifecycle', {r['name'] for r in value['candidates']})
            self.assertNotIn('todo!', json.dumps(spans.state(value)))

    def test_request_contract_prefix_is_identical_and_all_rendered_bytes_count(self):
        files = small_files(); files['docs/rules.md'] = '# Contract\n' + 'Behavior requirement.\n' * 400
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory, files)
            item.task['required_public_readings'] = [{'path': 'docs/rules.md'}]
            value = catalog(item)
            baseline = spans.pack(item.repo, value)
            chosen = {k: 'none' for k in value['clauses']}
            changed = spans.pack(item.repo, value, choices=chosen,
                                 scores={r['id']: -r['baseline_score'] for r in value['candidates']})
            for output in [baseline, changed]:
                self.assertLessEqual(output['payload_bytes'], 16384)
                self.assertEqual(output['sha256'], context.digest(output['text'].encode()))
                doc = next(r for r in output['selected'] if r['role'] == 'document')
                self.assertEqual(doc['completeness'], 'partial_file')
                self.assertEqual(doc['source_sha256'], next(r for r in baseline['selected'] if r['role'] == 'document')['source_sha256'])
                self.assertLessEqual(doc['source_bytes'], spans.CONTRACT_BYTES)
            self.assertIn('Complete applicable instructions are supplied separately', baseline['text'])

    def test_exact_function_read_keeps_attributes_and_ignores_dirty_worktree(self):
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory)
            value = catalog(item)
            (item.repo/'crates/widget/tests/behavior.rs').write_text('PRIVATE DIRTY SOURCE')
            output = spans.pack(item.repo, value)
            test = next(r for r in output['selected'] if r['role'] == 'test')
            self.assertEqual(test['completeness'], 'complete_declaration')
            self.assertEqual(test['start_line'], 1)
            self.assertIn('#[test]\nfn state_transition()', output['text'])
            self.assertNotIn('PRIVATE', output['text'])

    def test_oversized_function_is_a_labeled_exact_leading_slice(self):
        files = small_files()
        files['crates/widget/src/lib.rs'] = 'fn change() {\n' + '    // α padding\n' * 600 + '}\n'
        declarations = small_declarations(); declarations['crates/widget/src/lib.rs'] = [('change', 1, 602)]
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory, files, declarations); value = catalog(item)
            output = spans.pack(item.repo, value)
            row = next(r for r in output['selected'] if r['role'] == 'implementation')
            self.assertEqual(row['completeness'], 'partial_declaration')
            self.assertEqual(row['full_end_line'], 602)
            self.assertLessEqual(row['source_bytes'], spans.SLICE_BYTES)
            expected = ''.join(files[row['path']].splitlines(keepends=True)[row['start_line']-1:row['end_line']])
            self.assertEqual(row['source_sha256'], context.digest(expected.encode()))

    def test_catalog_and_response_corruption_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory); value = catalog(item)
            corrupt = copy.deepcopy(value); corrupt['candidates'][0]['start_line'] = 999
            with self.assertRaisesRegex(ValueError, 'catalog changed'): spans.pack(item.repo, corrupt)
            with self.assertRaisesRegex(ValueError, 'every exact clause'): spans.pack(item.repo, value, choices={})
            with self.assertRaisesRegex(ValueError, 'valid pointer'): spans.pack(item.repo, value, choices={k:'invented' for k in value['clauses']})
            with self.assertRaisesRegex(ValueError, 'finite score'): spans.pack(item.repo, value, scores={r['id']:float('nan') for r in value['candidates']})
            item.index['files'][0]['blob'] = 'a' * 40
            # A scoped source entry is checked, regardless of its lexical rank.
            source = next(e for e in item.index['files'] if e['path'].endswith('lib.rs'))
            source['blob'] = 'b' * 40
            with self.assertRaisesRegex(ValueError, 'binding'): catalog(item)

    def test_choices_are_complete_bounded_and_contain_no_generated_actions(self):
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory); item.task['prompt'] = 'Repair widget state. Preserve compatibility. Work only in the assigned package.'
            value = catalog(item); qs = spans.questions(value)
            self.assertEqual(set(qs), set(value['clauses']))
            for question in qs.values():
                self.assertEqual(question['type'], 'choice')
                self.assertIn('none', question['criteria'])
                self.assertLessEqual(len(question['criteria']), 255)
            self.assertEqual(len(spans.questions(value, include_scores=True)), len(qs) + len(value['candidates']))
            self.assertEqual(set(spans.deterministic_choices(value)), set(qs))
            self.assertNotIn('text', spans.state(value)['catalog'][0])

    def test_read_pointer_cap_and_omissions_are_independent_of_body_fit(self):
        files = small_files(); declarations = small_declarations()
        files['crates/widget/src/lib.rs'] = ''.join(f'fn change_{i}() {{}}\n' for i in range(40))
        declarations['crates/widget/src/lib.rs'] = [(f'change_{i}', i + 1, i + 1) for i in range(40)]
        with tempfile.TemporaryDirectory() as directory:
            item = fixture(directory, files, declarations); value = catalog(item)
            output = spans.pack(item.repo, value)
            self.assertEqual(output['read_pointers'], 24)
            self.assertEqual(sum(r['reason']=='read_pointer_limit' for r in output['omissions']), 17)
            self.assertLessEqual(output['payload_bytes'], spans.PACK_BYTES)


if __name__ == '__main__':
    unittest.main()

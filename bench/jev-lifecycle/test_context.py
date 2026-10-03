"""Exercise source provenance, admission, partial evidence, and shared pack limits."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import context


class Fixture:
    def __init__(self, directory, files, declarations):
        self.repo = Path(directory)
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        for name, text in files.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(text.encode())
        subprocess.run(["git", "-C", str(self.repo), "add", "."], check=True)
        subprocess.run(["git", "-C", str(self.repo), "-c", "user.name=Test", "-c", "user.email=test@example.test", "commit", "-qm", "Fixture"], check=True)
        self.rev = context.git(self.repo, "rev-parse", "HEAD").decode().strip()
        entries = []
        for path, text in files.items():
            entry = {"path": path, "blob": context.git(self.repo, "rev-parse", self.rev + ":" + path).decode().strip(),
                     "sha256": context.digest(text.encode()), "size": len(text.encode()), "terms": sorted(context.terms(text))}
            if path.endswith(".rs"):
                entry["syntax"] = {"declarations": [{"name": name.split("::")[-1], "qualified_name": name,
                    "kind": "function_item", "parse_has_error": False,
                    "declaration": {"start_line": start, "end_line": end}} for name, start, end in declarations.get(path, [])]}
            entries.append(entry)
        self.index = {"commit": self.rev, "syntax": {"extractor_version": "briefing-lab-rust-v1"}, "files": entries}
        self.task = {"id": "synthetic", "title": "Repair the widget lifecycle", "prompt": "Correct the widget state transition and retain its tests.",
                     "packages": ["widget"], "allowed_paths": ["crates/widget/"], "source_commit": self.rev}

    def assemble(self, **kwargs):
        return context.assemble(self.repo, self.rev, self.index, self.task, **kwargs)


def small_files():
    return {"Cargo.toml": '[workspace]\nmembers = ["crates/widget"]\n',
            "crates/widget/Cargo.toml": '[package]\nname = "widget"\nversion = "0.1.0"\n',
            "crates/widget/src/lib.rs": "pub fn change() { }\n",
            "crates/widget/tests/behavior.rs": "#[test]\nfn state_transition() { }\n"}


def small_declarations():
    return {"crates/widget/src/lib.rs": [("change", 1, 1)],
            "crates/widget/tests/behavior.rs": [("state_transition", 2, 2)]}


class BatchReadTests(unittest.TestCase):
    def test_batch_matches_single_reads_including_empty_unicode_and_duplicate_blobs(self):
        files = {"empty": "", "unicode": "α\n", "same": "α\n"}
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, {})
            bindings = context.tree(fixture.repo, fixture.rev)
            expected = {p: context.read_blob(fixture.repo, p, bindings) for p in files}
            self.assertEqual(context.read_blobs(fixture.repo, files, bindings), expected)
            self.assertEqual(context.read_blobs(fixture.repo, [], bindings), {})

    def test_oversize_batch_stops_before_content_read(self):
        blob = "a" * 40
        header = f"{blob} blob {2 * 1024 * 1024 + 1}\n".encode()
        with patch.object(context.subprocess, "check_output", return_value=header) as run:
            with self.assertRaisesRegex(ValueError, "2 MiB"):
                context.read_blobs("unused", ["large"], {"large": blob})
            self.assertEqual(run.call_count, 1)

    def test_batch_rejects_identity_size_utf8_and_trailing_corruption(self):
        blob = "a" * 40
        header = f"{blob} blob 2\n".encode()
        invalid = [b"b" * 40 + b" blob 2\nab\n", header + b"a", header + b"abX",
                   header + b"ab\nextra", header + b"\xff\xff\n"]
        for payload in invalid:
            with self.subTest(payload=payload):
                with patch.object(context.subprocess, "check_output", side_effect=[header, payload]):
                    with self.assertRaises((ValueError, UnicodeDecodeError)):
                        context.read_blobs("unused", ["file"], {"file": blob})
        with patch.object(context.subprocess, "check_output", return_value=b"missing\n"):
            with self.assertRaisesRegex(ValueError, "identity"):
                context.read_blobs("unused", ["file"], {"file": blob})


class ContextTests(unittest.TestCase):
    def test_package_admission_reserves_implementation_and_test_without_global_noise(self):
        files = small_files()
        declarations = small_declarations()
        for n in range(60):
            path = f"bench/noise{n}.rs"
            files[path] = "fn widget_lifecycle_state_transition() {}\n"
            declarations[path] = [("widget_lifecycle_state_transition", 1, 1)]
        files['docs/widget.md'] = '# Widget lifecycle\nThe transition keeps its state.\n'
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, declarations)
            fixture.task['prompt'] += ' Read `docs/widget.md`.'
            result = fixture.assemble()
            self.assertEqual(result['coverage']['candidate_roles'], {'implementation': 1, 'test': 1, 'document': 1})
            self.assertFalse(any(r['path'].startswith('bench/') for r in result['candidates']))
            self.assertEqual(result['coverage']['files_read'], 3)
            self.assertIn('#[test]\n', next(r['text'] for r in result['candidates'] if r['role'] == 'test'))

    def test_explicit_file_admits_its_package_when_task_has_no_package_list(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, small_files(), small_declarations())
            fixture.task.update(packages=[], allowed_paths=[], prompt='Fix `crates/widget/src/lib.rs`.')
            result = fixture.assemble()
            self.assertEqual({r['role'] for r in result['candidates']}, {'implementation', 'test'})
            self.assertIn('explicit_file', result['candidates'][0]['admission_reasons'])

    def test_named_root_document_does_not_admit_whole_workspace(self):
        files = small_files(); files['README.md'] = '# Widget\n'
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, small_declarations())
            fixture.task.update(packages=[], allowed_paths=[], prompt='Read `README.md`.')
            result = fixture.assemble()
            self.assertEqual([r['path'] for r in result['candidates']], ['README.md'])

    def test_only_committed_bytes_and_public_task_fields_enter_context(self):
        files = small_files()
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, small_declarations())
            fixture.task.update(checker='PRIVATE ORACLE', reference_patch='PRIVATE SOLUTION')
            expected = fixture.assemble()
            (fixture.repo/'crates/widget/src/lib.rs').write_text('PRIVATE WORKTREE CHANGE')
            actual = fixture.assemble()
            self.assertEqual(expected['candidate_pool_sha256'], actual['candidate_pool_sha256'])
            self.assertNotIn('PRIVATE', json.dumps(actual))
            for row in actual['candidates']:
                lines = files[row['path']].splitlines(keepends=True)
                self.assertEqual(row['text'], ''.join(lines[row['start_line']-1:row['end_line']]))
                self.assertEqual(row['source_sha256'], hashlib.sha256(row['text'].encode()).hexdigest())

    def test_index_path_and_content_corruption_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, small_files(), small_declarations())
            entry = next(x for x in fixture.index['files'] if x['path'].endswith('lib.rs'))
            original = entry['blob']; entry['blob'] = 'a'*40
            with self.assertRaisesRegex(ValueError, 'binding'):
                fixture.assemble()
            entry['blob'] = original; entry['sha256'] = 'b'*64
            with self.assertRaisesRegex(ValueError, 'bytes'):
                fixture.assemble()

    def test_oversized_declaration_retains_exact_slice_and_read_pointer(self):
        files = small_files()
        files['crates/widget/src/lib.rs'] = 'pub fn change() {\n' + ''.join('    // harmless padding α\n' for _ in range(400)) + '    // widget state transition\n}\n'
        declarations = small_declarations(); declarations['crates/widget/src/lib.rs'] = [('change', 1, 403)]
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, declarations)
            result = fixture.assemble()
            row = next(r for r in result['candidates'] if r['role'] == 'implementation')
            self.assertEqual(row['completeness'], 'partial_declaration')
            self.assertLessEqual(len(row['text'].encode()), context.SLICE_BYTES)
            self.assertEqual(row['full_end_line'], 403)
            self.assertIn('widget state transition', row['text'])
            self.assertEqual(result['coverage']['oversized_units'], 1)
            self.assertTrue(any(c['reason'] == 'partial_unit_needs_targeted_read' for c in result['catalog']))
            preview = context.read_source(fixture.repo, fixture.rev, row['path'], row['start_line'], 120)
            self.assertTrue(preview['truncated'])
            self.assertEqual(preview['blob'], row['blob'])
            self.assertLessEqual(len(preview['text'].encode()), 120)
            self.assertTrue(preview['text'].endswith('\n'))

    def test_oversized_single_line_is_a_pointer_not_clipped_code(self):
        files = small_files(); files['crates/widget/src/lib.rs'] = 'pub fn change() { /*' + 'x'*7000 + '*/ }\n'
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, small_declarations())
            result = fixture.assemble()
            row = next(r for r in result['candidates'] if r['role'] == 'implementation')
            self.assertEqual(row['completeness'], 'targeted_read_only')
            self.assertEqual(row['text'], '')
            self.assertIsNone(row['end_line'])
            read = context.read_source(fixture.repo, fixture.rev, row['path'], max_bytes=300)
            self.assertEqual(read['reason'], 'line_exceeds_read_budget')
            self.assertEqual(read['text'], '')

    def test_equal_pack_budget_same_pool_and_exact_score_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, small_files(), small_declarations())
            result = fixture.assemble()
            baseline = context.pack(result, budget=850)
            winner = next(r['id'] for r in result['candidates'] if r['id'] not in baseline['selected_ids'])
            scores = {r['id']: int(r['id'] == winner) for r in result['candidates']}
            changed = context.pack(result, scores, budget=850)
            self.assertIn(winner, changed['selected_ids'])
            self.assertNotEqual(baseline['selected_ids'], changed['selected_ids'])
            for packed in (baseline, changed):
                self.assertEqual(packed['candidate_pool_sha256'], result['candidate_pool_sha256'])
                self.assertLessEqual(packed['payload_bytes'], 850)
            with self.assertRaises(ValueError):context.pack(result, {winner: 1})
            scores[winner] = float('nan')
            with self.assertRaises(ValueError):context.pack(result, scores)
            result['candidates'][0]['text'] = 'tampered'
            with self.assertRaises(ValueError):context.pack(result)

    def test_candidate_budget_reports_omissions_and_keeps_both_code_roles(self):
        files = small_files(); declarations = small_declarations()
        for n in range(8):
            path = f'crates/widget/src/unit{n}.rs'; files[path] = f'fn widget{n}() {{}}\n'; declarations[path] = [(f'widget{n}',1,1)]
        with tempfile.TemporaryDirectory() as directory:
            fixture = Fixture(directory, files, declarations)
            result = fixture.assemble(max_candidates=2)
            self.assertEqual(result['coverage']['candidate_roles'], {'implementation':1,'test':1})
            self.assertEqual(result['coverage']['omitted_units'],8)
            self.assertEqual(result['coverage']['omission_reasons'], {'candidate_count_budget':8})
            self.assertEqual(len(result['catalog']),8)
            self.assertLessEqual(result['candidate_state_bytes'],context.STATE_BYTES)

    def test_state_budget_and_input_order_are_deterministic(self):
        files=small_files(); declarations=small_declarations()
        for n in range(12):
            path=f'crates/widget/src/other{n}.rs';files[path]=f'fn other{n}() {{ /*'+ 'state '*200 +'*/ }\n';declarations[path]=[(f'other{n}',1,1)]
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory,files,declarations)
            first=fixture.assemble(state_bytes=4096)
            fixture.index['files'].reverse()
            second=fixture.assemble(state_bytes=4096)
            self.assertEqual(first['candidates'],second['candidates'])
            self.assertEqual(first['catalog'],second['catalog'])
            self.assertLessEqual(first['candidate_state_bytes'],4096)
            self.assertGreater(first['coverage']['omission_reasons']['candidate_state_budget'],0)

    def test_inline_tests_are_a_separate_role(self):
        files=small_files();files['crates/widget/src/lib.rs']='pub fn change() {}\n#[test]\nfn check_widget() {}\n'
        declarations=small_declarations();declarations['crates/widget/src/lib.rs']=[('change',1,1),('check_widget',3,3)]
        with tempfile.TemporaryDirectory() as directory:
            result=Fixture(directory,files,declarations).assemble()
            inline=next(r for r in result['candidates'] if r['name']=='check_widget')
            self.assertEqual(inline['role'],'test')
            self.assertEqual(inline['start_line'],2)

    def test_read_rejects_traversal_and_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory,small_files(),small_declarations())
            for path in ('../outside','/etc/passwd','.git/config','missing'):
                with self.assertRaises(ValueError):context.read_source(fixture.repo,fixture.rev,path)
            with self.assertRaises(ValueError):context.read_source(fixture.repo,fixture.rev,'crates/widget/src/lib.rs',start_line=999)


if __name__ == '__main__':unittest.main()

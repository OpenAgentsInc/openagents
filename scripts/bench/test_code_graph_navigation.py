"""Synthetic checks for the local history census; no real history is a fixture."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "navigation", Path(__file__).with_name("code-graph-navigation.py"))
navigation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(navigation)


class NavigationTests(unittest.TestCase):
    def test_static_wrappers_do_not_execute_commands(self):
        source = '''text(await tools.exec_command({"cmd":"rg model crates/demo/src/lib.rs"}));
        await tools.exec_command({cmd: 'cat crates/demo/src/lib.rs'});
        await tools.exec_command({cmd: dynamicCommand});'''
        operations, unknown = navigation.operations("exec", source)
        self.assertEqual([kind for kind, _ in operations], ["search", "read"])
        self.assertFalse(unknown)
        self.assertEqual(navigation.operations("exec", "dynamicCommand()"), ([], True))

    def test_cutoff_deduplication_and_public_path_filter(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "history.jsonl"
            def call(identity, command, date="2026-10-14T12:00:00Z"):
                return {"type": "assistant", "cwd": str(root), "timestamp": date,
                        "message": {"content": [{"type": "tool_use", "id": identity,
                                    "name": "Bash", "input": {"command": command}}]}}
            rows = [call("a", "cat crates/demo/src/lib.rs docs/private.md"),
                    call("a", "cat crates/demo/src/lib.rs docs/private.md"),
                    call("b", "cat crates/demo/src/lib.rs docs/private.md"),
                    call("c", "apply_patch saved-patch"),
                    call("d", "rg future", "2026-10-15T00:00:00Z")]
            path.write_text("\n".join(json.dumps(row) for row in rows) + "\n")
            result, reason = navigation.census_file(
                path, "claude", root, navigation.timestamp("2026-10-15T00:00:00Z"),
                {"crates/demo/src/lib.rs"})
            self.assertIsNone(reason)
            self.assertEqual(result["totals"]["tool_calls"], 3)
            self.assertEqual(result["totals"]["duplicate_calls"], 1)
            self.assertEqual(result["totals"]["repeated_navigation_operations"], 1)
            self.assertEqual(result["before_first_edit"], 2)
            self.assertEqual(dict(result["paths"]), {"crates/demo/src/lib.rs": 2})
            self.assertNotIn("private", json.dumps(navigation.summarize([result])))

    def test_codex_metadata_excludes_other_repositories(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "history.jsonl"
            path.write_text(json.dumps({"type": "session_meta", "payload": {
                "cwd": "/unrelated-repository"}}) + "\n")
            row, reason = navigation.census_file(
                path, "codex", root, navigation.timestamp("2026-10-15T00:00:00Z"), set())
            self.assertIsNone(row)
            self.assertEqual(reason, "other_repository")

    def test_codex_custom_and_function_call_shapes(self):
        custom = {"type": "response_item", "payload": {"type": "custom_tool_call",
                  "call_id": "x", "name": "exec", "input": 'tools.exec_command({cmd:"ls"})'}}
        ordinary = {"type": "response_item", "payload": {"type": "function_call",
                    "call_id": "y", "name": "exec_command", "arguments": '{"cmd":"pwd"}'}}
        for record in (custom, ordinary):
            _, name, args = next(navigation.calls(record, "codex"))
            self.assertEqual(navigation.operations(name, args)[0][0][0], "list")


if __name__ == "__main__":
    unittest.main()

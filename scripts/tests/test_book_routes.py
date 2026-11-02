"""Independent, tiny-fixture checks for book-route evidence and selective reuse.

These tests never run a real Coax compiler, numerical gate, model or network.
"""
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "book_routes_under_test", Path(__file__).resolve().parents[1] / "book_routes.py")
routes = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(routes)


class RouteEvidence(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for name, data in {"a.coax": b"source a", "b.coax": b"source b",
                           "shared.coax": b"shared", "a.out": b"alpha\n",
                           "b.out": b"beta\n", "host.py": b"host", "d": b"descriptor"}.items():
            (self.root / name).write_bytes(data)
        self.compiler = self.root / "stub-coax"
        self.compiler.write_bytes(b"stub compiler identity")
        self.tools = {"coax": {"sha256": routes.file_sha(self.compiler), "version": "stub1"},
                      "cc": {"sha256": "c" * 64, "version": "stub cc1"}}
        self.steps = [self.step("a"), self.step("b")]
        self.route = {"id": "fixture-route", "title": "Tiny evidence fixture", "book": "aethyr",
                      "scope": "Stub transcripts only", "limitations": ["No product qualification"],
                      "prerequisites": {}, "chapters": [], "evidence": [], "steps": self.steps}
        self.dep_calls = []
        self.run_calls = []
        self.tool_patch = patch.object(routes, "tool_context", side_effect=lambda compiler: copy.deepcopy(self.tools))
        self.tool_patch.start()
        self.addCleanup(self.tool_patch.stop)
        self.revision_patch = patch.object(routes, "git_revision", return_value="observed-fixture-revision")
        self.revision_patch.start()
        self.addCleanup(self.revision_patch.stop)

    def step(self, name):
        return {"id": name, "kind": "coax-run", "source": name + ".coax",
                "stdout": name + ".out", "exit_status": 0, "modes": ["debug"]}

    def dependencies(self, root, step, compiler, mode):
        self.dep_calls.append((step["id"], mode))
        return sorted(set(routes.step_paths(step) + ["shared.coax"]))

    def execute(self, argv, root, env=None, timeout=120):
        self.run_calls.append(list(argv))
        name = Path(argv[2]).stem
        return subprocess.CompletedProcess(argv, 0, (self.root / (name + ".out")).read_bytes(), b"")

    def qualify(self, name, previous=None, execute=None, target=None):
        with patch.object(routes, "dependency_inputs", side_effect=self.dependencies), \
             patch.object(routes, "execute", side_effect=execute or self.execute):
            return routes.run_route(self.root, self.route, self.compiler, self.root / name,
                                    previous=previous, target=target or {"machine": "fixture-cpu"})

    def fingerprint(self, step=None, **kwargs):
        with patch.object(routes, "dependency_inputs", side_effect=self.dependencies):
            return routes.fingerprint_step(self.root, step or self.steps[0], self.compiler,
                                           {"machine": "fixture-cpu"}, **kwargs)

    def manifest(self):
        path = self.root / "routes.json"
        path.write_text(json.dumps({"schema_version": 1, "routes": [self.route]}))
        return path

    def test_unchanged_reuses_without_deps_and_private_drift_is_selective(self):
        first = self.qualify("first.json")
        self.assertEqual((first["status"], first["executed_runs"]), ("passed", 2))
        self.dep_calls.clear(); self.run_calls.clear()
        unchanged = self.qualify("unchanged.json", self.root / "first.json")
        self.assertEqual((unchanged["retained_runs"], self.dep_calls, self.run_calls), (2, [], []))
        (self.root / "a.coax").write_bytes(b"source a edited")
        changed = self.qualify("changed.json", self.root / "unchanged.json")
        self.assertEqual(changed["status"], "passed")
        self.assertEqual([(r["key"], r["retained"]) for r in changed["steps"]],
                         [("a:debug", False), ("b:debug", True)])
        self.assertEqual(self.dep_calls, [("a", "debug")])
        self.assertEqual(len(self.run_calls), 1)

    def test_edit_of_an_earlier_input_before_final_publication_fails(self):
        def change_earlier(argv, root, env=None, timeout=120):
            result = self.execute(argv, root, env, timeout)
            if argv[2] == "b.coax":
                (self.root / "a.coax").write_bytes(b"changed while later step executes")
            return result
        result = self.qualify("drift.json", execute=change_earlier)
        self.assertEqual(result["status"], "failed")
        saved = routes.read_json(self.root / "drift.json")
        self.assertEqual(saved["status"], "failed")
        self.assertTrue((self.root / saved["steps"][0]["stdout"]["path"]).is_file())

    def test_dependency_metadata_requires_descriptor_list(self):
        graph = {"schema_version": 1, "sources": ["a.coax"], "descriptors": "d"}
        with patch.object(routes, "execute", return_value=subprocess.CompletedProcess([], 0, json.dumps(graph).encode(), b"")):
            with self.assertRaises(routes.RouteError):
                routes.dependency_inputs(self.root, self.steps[0], self.compiler, "debug")

    def test_shared_dependency_and_fixture_invalidate_only_consumers(self):
        self.qualify("first.json")
        (self.root / "a.out").write_bytes(b"updated fixture\n")
        self.dep_calls.clear(); self.run_calls.clear()
        result = self.qualify("fixture.json", self.root / "first.json")
        self.assertEqual([r["retained"] for r in result["steps"]], [False, True])
        self.assertEqual(self.dep_calls, [("a", "debug")])
        (self.root / "shared.coax").write_bytes(b"changed shared dependency")
        self.dep_calls.clear(); self.run_calls.clear()
        result = self.qualify("shared.json", self.root / "fixture.json")
        self.assertEqual((result["status"], result["executed_runs"]), ("passed", 2))
        self.assertEqual(self.dep_calls, [("a", "debug"), ("b", "debug")])

    def test_recipe_mode_options_environment_and_tool_target_identity(self):
        original = self.fingerprint()
        for field, value in [("run_steps", 123), ("env", {"FEATURE": "enabled"}),
                             ("timeout", 7), ("exit_status", 3)]:
            with self.subTest(field=field):
                changed = dict(self.steps[0], **{field: value})
                self.assertNotEqual(self.fingerprint(changed), original)
        two_modes = dict(self.steps[0], modes=["debug", "release"])
        self.assertNotEqual(self.fingerprint(two_modes, mode="debug"), self.fingerprint(two_modes, mode="release"))
        with patch.object(routes, "dependency_inputs", side_effect=self.dependencies):
            changed_target = routes.fingerprint_step(self.root, self.steps[0], self.compiler, {"machine": "other-cpu"})
        self.assertNotEqual(changed_target, original)
        for tool in ["coax", "cc"]:
            old = copy.deepcopy(self.tools[tool])
            self.tools[tool]["sha256"] = "1" * 64
            self.assertNotEqual(self.fingerprint(), original)
            self.tools[tool] = old
            self.tools[tool]["version"] += " changed"
            self.assertNotEqual(self.fingerprint(), original)
            self.tools[tool] = old

    def test_source_edit_recomputes_added_and_removed_dependency_closure(self):
        extra = self.root / "extra.coax"; extra.write_bytes(b"extra")
        old = self.fingerprint()
        (self.root / "a.coax").write_bytes(b"now loads extra")
        with patch.object(routes, "dependency_inputs", return_value=["a.coax", "a.out", "shared.coax", "extra.coax"]) as deps:
            added = routes.fingerprint_step(self.root, self.steps[0], self.compiler,
                                            {"machine": "fixture-cpu"}, previous=old)
        deps.assert_called_once()
        self.assertIn("extra.coax", added["inputs"])
        (self.root / "a.coax").write_bytes(b"no longer loads extra")
        removed = self.fingerprint(previous=added)
        self.assertNotIn("extra.coax", removed["inputs"])

    def test_missing_input_and_symlink_escape_cannot_acquire_pass(self):
        for path in ["../outside", "/tmp/outside", "a/../a.coax", "./a.coax", "a//b", "a\\b"]:
            with self.subTest(path=path), self.assertRaises(routes.RouteError):
                routes.checked_path(self.root, path)
        with tempfile.TemporaryDirectory() as other:
            target = Path(other) / "outside"; target.write_bytes(b"external")
            (self.root / "escape").symlink_to(target)
            with self.assertRaises(routes.RouteError):
                routes.checked_path(self.root, "escape")
        (self.root / "a.coax").unlink()
        result = self.qualify("missing.json")
        self.assertEqual(result["status"], "failed")
        self.assertEqual(self.run_calls, [])

    def test_symlink_retargeting_with_identical_bytes_invalidates_resolution(self):
        (self.root / "copy-one").write_bytes(b"source a")
        (self.root / "copy-two").write_bytes(b"source a")
        (self.root / "a.coax").unlink(); (self.root / "a.coax").symlink_to("copy-one")
        before = self.fingerprint()
        (self.root / "a.coax").unlink(); (self.root / "a.coax").symlink_to("copy-two")
        after = self.fingerprint(previous=before)
        self.assertEqual(before["inputs"], after["inputs"])
        self.assertNotEqual(before["resolutions"], after["resolutions"])

    def test_malformed_cached_closure_cannot_skip_dependency_discovery(self):
        original = self.fingerprint()
        for mutate in [lambda x: x["inputs"].pop("shared.coax"),
                       lambda x: x.update(closure_sha256="0" * 64),
                       lambda x: x["resolutions"].update({"a.coax": "b.coax"})]:
            damaged = copy.deepcopy(original); mutate(damaged)
            self.dep_calls.clear()
            current = self.fingerprint(previous=damaged)
            self.assertEqual(self.dep_calls, [("a", "debug")])
            self.assertEqual(current, original)

    def test_retained_origin_preserves_actual_original_observation(self):
        first = self.qualify("first.json")
        first_sha = routes.file_sha(self.root / "first.json")
        with patch.object(routes, "git_revision", return_value="later-revision"):
            second = self.qualify("second.json", self.root / "first.json")
        third = self.qualify("third.json", self.root / "second.json")
        self.assertEqual(second["observed_revision"], "later-revision")
        for row in second["steps"] + third["steps"]:
            self.assertEqual(row["origin"], {"receipt_sha256": first_sha,
                                            "observed_revision": first["observed_revision"]})
        self.assertIn("not a claim", second["revision_scope"])

    def test_reuse_requires_complete_matching_logs_and_actual_status(self):
        row = self.qualify("first.json")["steps"][0]
        fp = row["fingerprint"]
        self.assertTrue(routes.reusable(row, fp, self.root))
        cases = [dict(row, status="failed"), dict(row, exit_status=1), dict(row, exit_status=False),
                 {k: v for k, v in row.items() if k != "stderr"}]
        for bad in cases:
            with self.subTest(prior=bad):
                self.assertFalse(routes.reusable(bad, fp, self.root))
        for key, value in [("sha256", "not-a-sha"), ("bytes", True), ("bytes", -1),
                           ("path", "../outside"), ("path", "missing.log")]:
            bad = copy.deepcopy(row); bad["stdout"][key] = value
            self.assertFalse(routes.reusable(bad, fp, self.root))
        log = self.root / row["stdout"]["path"]; log.write_bytes(b"tampered")
        self.assertFalse(routes.reusable(row, fp, self.root))

    def test_failed_transcript_exit_or_diagnostics_are_persisted_and_never_reused(self):
        for i, (code, output, error) in enumerate([(1, b"alpha\n", b""), (0, b"wrong", b""), (0, b"alpha\n", b"diagnostic")]):
            with self.subTest(case=i):
                result = self.qualify(f"failed-{i}.json", execute=lambda *a, **kw: subprocess.CompletedProcess(a[0], code, output, error))
                self.assertEqual(result["status"], "failed")
                row = result["steps"][0]
                self.assertEqual((self.root / row["stdout"]["path"]).read_bytes(), output)
                self.assertEqual((self.root / row["stderr"]["path"]).read_bytes(), error)
                self.run_calls.clear()
                retry = self.qualify(f"retry-{i}.json", self.root / f"failed-{i}.json")
                self.assertEqual((retry["status"], retry["executed_runs"]), ("passed", 2))
                self.assertEqual(len(self.run_calls), 2)

    def test_timeout_and_interruption_preserve_raw_partial_output(self):
        timeout = subprocess.TimeoutExpired(["stub"], 1, output=b"partial-out", stderr=b"partial-err")
        def expire(*args, **kwargs): raise timeout
        result = self.qualify("timeout.json", execute=expire)
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["steps"][0]["timeout"])
        interrupt = KeyboardInterrupt("fixture interrupted")
        interrupt.output, interrupt.stderr = b"interrupt-out", b"interrupt-err"
        def stop(*args, **kwargs): raise interrupt
        with self.assertRaises(KeyboardInterrupt):
            self.qualify("interrupted.json", execute=stop)
        saved = routes.read_json(self.root / "interrupted.json")
        self.assertEqual(saved["status"], "interrupted")
        for record, expected in [(result, (b"partial-out", b"partial-err")),
                                 (saved, (b"interrupt-out", b"interrupt-err"))]:
            row = record["steps"][0]
            self.assertEqual(tuple((self.root / row[k]["path"]).read_bytes() for k in ["stdout", "stderr"]), expected)

    def test_final_log_rehash_refuses_earlier_output_tampering(self):
        def tamper(argv, root, env=None, timeout=120):
            result = self.execute(argv, root, env, timeout)
            if argv[2] == "b.coax":
                (self.root / "tampered.assets/a-debug.stdout").write_bytes(b"edited after first success")
            return result
        result = self.qualify("tampered.json", execute=tamper)
        self.assertEqual(result["status"], "failed")
        self.assertIn("output changed", result["error"])

    def test_receipt_integrity_and_exclusive_publication(self):
        self.qualify("first.json")
        original = (self.root / "first.json").read_bytes()
        with self.assertRaises(FileExistsError):
            self.qualify("first.json")
        self.assertEqual((self.root / "first.json").read_bytes(), original)
        damaged = routes.read_json(self.root / "first.json")
        damaged["steps"][0]["status"] = "failed"
        (self.root / "damaged.json").write_text(json.dumps(damaged))
        with self.assertRaisesRegex(routes.RouteError, "integrity"):
            self.qualify("bad-reuse.json", self.root / "damaged.json")
        self.assertFalse((self.root / "bad-reuse.json").exists())

    def test_metadata_check_never_invokes_tools_or_examples(self):
        manifest = self.manifest()
        with patch.object(sys, "argv", ["book_routes.py", "--root", str(self.root), "--manifest", str(manifest), "--check"]), \
             patch.object(routes, "execute") as execute, \
             patch.object(routes, "tool_context") as tools, redirect_stdout(io.StringIO()) as output:
            routes.main()
        execute.assert_not_called(); tools.assert_not_called()
        self.assertIn("metadata valid", output.getvalue())

    def test_duplicate_metadata_keys_ids_and_empty_routes_refuse(self):
        manifest = self.manifest()
        manifest.write_text('{"schema_version":1,"schema_version":1,"routes":[]}')
        with self.assertRaisesRegex(routes.RouteError, "duplicate"):
            routes.load_manifest(self.root, manifest)
        for routes_value in [[], [self.route, self.route]]:
            manifest.write_text(json.dumps({"schema_version": 1, "routes": routes_value}))
            with self.assertRaises(routes.RouteError):
                routes.load_manifest(self.root, manifest)
        empty = dict(self.route, steps=[])
        with self.assertRaises(routes.RouteError):
            routes.run_route(self.root, empty, self.compiler, self.root / "empty.json")

    def test_dependency_discovery_binds_mode_flags_and_rejects_escape(self):
        step = dict(self.steps[0], modes=["debug", "release"], run_steps=321)
        graph = {"schema_version": 1, "sources": [str(self.root / "a.coax")], "descriptors": ["d"]}
        with patch.object(routes, "execute", return_value=subprocess.CompletedProcess([], 0, json.dumps(graph).encode(), b"")) as execute:
            inputs = routes.dependency_inputs(self.root, step, self.compiler, "release")
        self.assertIn("d", inputs)
        self.assertEqual(execute.call_args.args[0], [str(self.compiler), "deps", "a.coax", "--library", "--release", "--run-steps", "321"])
        graph["sources"] = ["../outside"]
        with patch.object(routes, "execute", return_value=subprocess.CompletedProcess([], 0, json.dumps(graph).encode(), b"")):
            with self.assertRaises(routes.RouteError):
                routes.dependency_inputs(self.root, step, self.compiler, "release")

    def test_command_argv_host_script_and_declared_inputs_are_identity(self):
        (self.root / "tools").mkdir(); (self.root / "tools/host").write_bytes(b"inert host fixture")
        step = {"id": "host", "kind": "command", "argv": ["tools/host", "literal;argument"],
                "sources": ["a.coax"], "inputs": ["tools/host"], "stdout": "a.out",
                "exit_status": 0, "modes": ["native"]}
        original = self.fingerprint(step)
        self.assertEqual(routes.row_command(step, "native", self.compiler), step["argv"])
        self.assertNotEqual(self.fingerprint(dict(step, argv=["tools/host", "different"])), original)
        (self.root / "tools/host").write_bytes(b"host changed")
        changed = self.fingerprint(step, previous=original)
        self.assertNotEqual(changed["tools"]["command"], original["tools"]["command"])

    def test_running_receipt_is_written_before_each_command(self):
        observations = []
        def observe(argv, root, env=None, timeout=120):
            proof = routes.read_json(self.root / "running.json")
            observations.append((proof["status"], [r["status"] for r in proof["steps"]]))
            self.assertEqual(proof["steps"][-1]["command"], argv)
            self.assertEqual(env["LC_ALL"], "C")
            self.assertEqual(env["COAX"], str(self.compiler.resolve()))
            self.assertTrue(Path(env["TMPDIR"]).is_dir())
            return self.execute(argv, root, env, timeout)
        result = self.qualify("running.json", execute=observe)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(observations, [("running", ["running"]), ("running", ["passed", "running"])])

    def test_changed_tool_or_runner_during_execution_cannot_pass(self):
        def change_tool(argv, root, env=None, timeout=120):
            result = self.execute(argv, root, env, timeout)
            self.tools["cc"]["sha256"] = "f" * 64
            return result
        self.assertEqual(self.qualify("tool-drift.json", execute=change_tool)["status"], "failed")
        original_sha = routes.file_sha
        altered = False
        def selected_sha(path):
            if Path(path).resolve() == Path(routes.__file__).resolve() and altered:
                return "0" * 64
            return original_sha(path)
        def change_runner(argv, root, env=None, timeout=120):
            nonlocal altered
            result = self.execute(argv, root, env, timeout)
            altered = True
            return result
        with patch.object(routes, "file_sha", side_effect=selected_sha):
            result = self.qualify("runner-drift.json", execute=change_runner)
        self.assertEqual(result["status"], "failed")

    def test_earlier_command_tool_rehashed_before_final_publication(self):
        (self.root / "tools").mkdir(); host = self.root / "tools/host"; host.write_bytes(b"inert host")
        self.steps[0] = {"id": "host", "kind": "command", "argv": ["tools/host"],
                         "sources": ["a.coax"], "inputs": ["tools/host"], "stdout": "a.out",
                         "exit_status": 0, "modes": ["native"]}
        def change_earlier_tool(argv, root, env=None, timeout=120):
            if argv[0] == "tools/host":
                return subprocess.CompletedProcess(argv, 0, b"alpha\n", b"")
            host.write_bytes(b"earlier tool edited while later step executes")
            return self.execute(argv, root, env, timeout)
        result = self.qualify("earlier-tool.json", execute=change_earlier_tool)
        self.assertEqual(result["status"], "failed")
        self.assertIn("earlier input changed", result["error"])

    def test_retained_log_copy_is_rechecked_at_final_publication(self):
        self.qualify("first.json")
        (self.root / "b.coax").write_bytes(b"b needs fresh execution")
        def damage_retained(argv, root, env=None, timeout=120):
            (self.root / "retained-tamper.assets/a-debug.stdout").write_bytes(b"damaged retained copy")
            return self.execute(argv, root, env, timeout)
        result = self.qualify("retained-tamper.json", self.root / "first.json", execute=damage_retained)
        self.assertEqual(result["status"], "failed")
        self.assertTrue(result["steps"][0]["retained"])

    def test_real_process_argv_is_literal_and_timeout_keeps_partial_output(self):
        literal = "literal;$(not-a-shell-command)"
        result = routes.execute([sys.executable, "-I", "-c", "import sys; print(sys.argv[1])", literal], self.root, timeout=2)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, (literal + "\n").encode(), b""))
        with self.assertRaises(subprocess.TimeoutExpired) as caught:
            routes.execute([sys.executable, "-I", "-c", "import time; print('partial',flush=True); time.sleep(10)"], self.root, timeout=0.2)
        self.assertEqual(caught.exception.output, b"partial\n")

    def test_execution_timeout_and_mode_bounds_are_explicit(self):
        for timeout in [0, 841]:
            with self.subTest(timeout=timeout), self.assertRaises(routes.RouteError):
                routes.run_route(self.root, self.route, self.compiler, self.root / "bounds.json", timeout=timeout)
        with self.assertRaises(routes.RouteError):
            self.fingerprint(mode="native")
        for env in [{"PATH": "/unbound"}, {"COAX": "other"}, {"feature": "wrong-name"}]:
            with self.subTest(env=env), self.assertRaises(routes.RouteError):
                self.fingerprint(dict(self.steps[0], env=env))

    def test_bash_script_is_explicit_declared_checkout_input(self):
        (self.root / "listed.sh").write_bytes(b"inert listed script")
        (self.root / "unlisted.sh").write_bytes(b"inert unlisted script")
        step = {"id": "bash-host", "kind": "command", "argv": ["bash", "listed.sh"],
                "sources": ["a.coax"], "inputs": ["listed.sh"], "stdout": "a.out",
                "exit_status": 0, "modes": ["native"]}
        routes.validate_step(self.root, step)
        for argv in [["bash", "unlisted.sh"], ["bash", "-c", "printf hidden"],
                     ["bash"], ["bash", "missing.sh"], ["bash", "../outside.sh"]]:
            with self.subTest(argv=argv), self.assertRaises(routes.RouteError):
                routes.validate_step(self.root, dict(step, argv=argv))

    def test_path_and_home_changes_invalidate_fingerprint_fast_path(self):
        with patch.dict(os.environ, {"PATH": "/fixture/path-one", "HOME": "/fixture/home-one"}):
            original = self.fingerprint()
        for changed in [{"PATH": "/fixture/path-two", "HOME": "/fixture/home-one"},
                        {"PATH": "/fixture/path-one", "HOME": "/fixture/home-two"}]:
            with self.subTest(environment=changed), patch.dict(os.environ, changed):
                self.dep_calls.clear()
                current = self.fingerprint(previous=original)
                self.assertNotEqual(current, original)
                self.assertEqual(self.dep_calls, [("a", "debug")])


if __name__ == "__main__":
    unittest.main()

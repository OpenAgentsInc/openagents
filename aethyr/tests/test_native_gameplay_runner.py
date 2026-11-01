#!/usr/bin/env python3
"""Failure-handling checks for native integration tooling; no gameplay is simulated."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

MODULE_PATH = Path(__file__).with_name('check_native_gameplay.py')
SPEC = importlib.util.spec_from_file_location('native_gameplay_runner', MODULE_PATH)
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class ReceiptTests(unittest.TestCase):
    def test_saved_receipt_refuses_source_and_artifact_drift(self):
        for label in ('fixture.coax', 'Coax compiler', 'emitted C', 'native binary'):
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                source = Path(temporary) / 'input'
                receipt = Path(temporary) / 'receipt.json'
                source.write_bytes(b'admitted')
                expected = {label: (source, RUNNER.digest(source.read_bytes()))}
                RUNNER.finalize_receipt(receipt, {'status': 'passed'}, expected)
                self.assertEqual(json.loads(receipt.read_text())['status'], 'passed')
                source.write_bytes(b'changed after admission')
                RUNNER.finalize_receipt(receipt, {'status': 'passed'}, expected)
                saved = json.loads(receipt.read_text())
                self.assertEqual(saved['status'], 'failed')
                self.assertIn(label, saved['source_changes'] + saved['artifact_changes'])

    def test_reuse_binds_successful_emission_and_all_identity_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            code = Path(temporary) / 'program.c'
            code.write_bytes(b'C fixture is read, never executed')
            admitted = {'entry': 'entry.coax', 'compiler_sha256': 'compiler', 'coax_flags': [],
                        'sources': {'entry.coax': 'source'}, 'c_file': str(code),
                        'c_sha256': RUNNER.digest(code.read_bytes()), 'command': {'exit_code': 0}}
            arguments = ('entry.coax', 'compiler', [], {'entry.coax': 'source'})
            self.assertEqual(RUNNER.validate_emission(admitted, *arguments), code)
            changes = {'entry': 'other.coax', 'compiler_sha256': 'other compiler',
                       'coax_flags': ['--release'], 'sources': {'entry.coax': 'changed'},
                       'c_sha256': 'changed', 'command': {'exit_code': 1}}
            for key, value in changes.items():
                with self.subTest(key=key):
                    invalid = copy.deepcopy(admitted)
                    invalid[key] = value
                    with self.assertRaises(RuntimeError):
                        RUNNER.validate_emission(invalid, *arguments)

    def test_finite_wrapper_cannot_skip_an_unchanged_inner_body(self):
        body = '{\n  #assert true;\n}\n'
        row = {'procedure': 'aethyr_native_deadgears_0', 'kind': 'forall', 'cases': 10,
               'original_bindings': ' k: 0..<10 ', 'body_sha256': RUNNER.digest(body.encode()),
               'original_name': 'fixture'}
        source = row['procedure'] + ' :: () #export_c {\n  for k: 0..<10 ' + body + '}\n'
        RUNNER.original_body(source, row)
        for invalid in (source.replace('{\n  for', '{\n  return;\n  for', 1),
                        source.replace('{\n  for', '{\n  if false {\n  for', 1) + '}\n'):
            with self.assertRaises(RuntimeError):
                RUNNER.original_body(invalid, row)


def running(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    state = subprocess.run(['ps', '-p', str(pid), '-o', 'stat='], capture_output=True,
                           text=True, timeout=2).stdout.strip()
    return bool(state) and not state.startswith('Z')


class ProcessGroupTests(unittest.TestCase):
    def test_timeout_and_signals_reap_worker_and_terminate_grandchild(self):
        for kind in ('timeout', 'interrupt', 'terminate'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                marker = directory / 'ready.json'
                child = "import signal,time;signal.signal(signal.SIGTERM,signal.SIG_IGN);signal.signal(signal.SIGINT,signal.SIG_IGN);time.sleep(30)"
                worker = directory / 'worker.py'
                worker.write_text(
                    'import json,os,pathlib,signal,subprocess,sys,time\n'
                    'marker=pathlib.Path(sys.argv[1])\n'
                    'marker.write_text(json.dumps({"parent":os.getpid(),"child":0}))\n'
                    f'child=subprocess.Popen([sys.executable,"-c",{child!r}])\n'
                    'marker.write_text(json.dumps({"parent":os.getpid(),"child":child.pid}))\n'
                    'signal.signal(signal.SIGTERM,signal.SIG_IGN)\n'
                    'signal.signal(signal.SIGINT,signal.SIG_IGN)\n'
                    'time.sleep(30)\n')
                wrapper = directory / 'wrapper.py'
                wrapper.write_text(
                    'import importlib.util,os,sys\n'
                    f'spec=importlib.util.spec_from_file_location("runner",{str(MODULE_PATH)!r})\n'
                    'module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)\n'
                    f'with open({str(directory / "child.log")!r},"wb") as output:\n'
                    f' code=module.run_process_group([sys.executable,{str(worker)!r},{str(marker)!r}],'
                    f'cwd={str(directory)!r},stdout=output,stderr=output,environment=os.environ.copy(),timeout=2)\n'
                    'sys.exit(code)\n')
                process = subprocess.Popen([sys.executable, str(wrapper)], stdout=subprocess.DEVNULL,
                                           stderr=subprocess.DEVNULL, start_new_session=True)
                pids = None
                try:
                    deadline = time.monotonic() + 4
                    while time.monotonic() < deadline:
                        if marker.exists():
                            try:
                                observed = json.loads(marker.read_text())
                            except json.JSONDecodeError:
                                observed = {}
                            if observed.get('child'):
                                pids = observed
                                break
                        time.sleep(0.02)
                    self.assertIsNotNone(pids, 'worker did not reach its independent readiness marker')
                    if kind != 'timeout':
                        os.kill(process.pid, signal.SIGINT if kind == 'interrupt' else signal.SIGTERM)
                    self.assertNotEqual(process.wait(timeout=5), 0)
                    deadline = time.monotonic() + 2
                    while time.monotonic() < deadline and any(running(pid) for pid in pids.values()):
                        time.sleep(0.02)
                    self.assertFalse(any(running(pid) for pid in pids.values()), 'isolated descendants survived cleanup')
                finally:
                    if marker.exists():
                        try:
                            parent = json.loads(marker.read_text()).get('parent')
                            if parent:
                                os.killpg(parent, signal.SIGKILL)
                        except (ProcessLookupError, json.JSONDecodeError):
                            pass
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=2)


if __name__ == '__main__':
    unittest.main()

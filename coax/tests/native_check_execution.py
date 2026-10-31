#!/usr/bin/env python3
"""Independent C1 baseline, AST-artifact and bounded process observations."""
import hashlib
import copy
import base64
import importlib.util
import json
import os
from pathlib import Path
import signal
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


COAX = Path(__file__).resolve().parents[1]
REPO = COAX.parent
FIXTURES = COAX / "tests/native-check-execution"
TRUSTED = Path(os.environ.get('COAX_EXECUTION_TEST_COMPILER',
                             os.environ.get('COAX_TEST_COMPILER', COAX / 'bin/coax'))).resolve()


def load(name, path):
    existing = sys.modules.get(name)
    if existing is not None and Path(existing.__file__).resolve() == path.resolve():
        return existing
    if str(path.parent) not in sys.path:
        sys.path.insert(0, str(path.parent))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def serializable(value):
    if isinstance(value, bytes):
        return {"hex": value.hex()}
    if isinstance(value, Path):
        return str(value)
    raise TypeError(type(value).__name__)


class RecordedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.record_root = Path(os.environ.get("COAX_EXECUTION_TEST_RECORD_DIR") or
                               tempfile.mkdtemp(prefix="coax-native-execution-"))
        cls.record_root.mkdir(parents=True, exist_ok=True)

    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix=self._testMethodName + "-", dir=self.record_root))
        self.started = time.monotonic()
        self.observations = []

    def tearDown(self):
        record = {
            "method": self._testMethodName, "seconds": time.monotonic() - self.started,
            "harness_sha256": digest(__file__), "observations": self.observations,
            "source_sha256": {str(path): digest(path) for path in FIXTURES.glob('*.coax')},
            "fixture_sha256": {str(path): digest(path) for path in FIXTURES.iterdir() if path.is_file()},
        }
        if TRUSTED.exists():
            record['trusted_compiler'] = {'path': str(TRUSTED), 'sha256': digest(TRUSTED)}
        (self.dir / "test-receipt.json").write_text(json.dumps(record, indent=2, default=serializable) + "\n")


class HostProcessTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.process = load("coax_execution_bounded_process", COAX / "tools/bounded_process.py")

    def child(self, *arguments, **options):
        command = [sys.executable, str(FIXTURES / "host-child.py"), *arguments]
        return self.observe(command, **options)

    def observe(self, command, **options):
        defaults = {"cwd": str(REPO), "env": None, "timeout_s": 2,
                    "max_output_bytes": 4096, "deadline": time.monotonic() + 30, "quiet_faults": False}
        defaults.update(options)
        result = self.process.run(command, **defaults)
        self.observations.append(result)
        self.assertIsInstance(result["stdout"], bytes)
        self.assertIsInstance(result["stderr"], bytes)
        self.assertLessEqual(len(result["stdout"]) + len(result["stderr"]), defaults["max_output_bytes"])
        return result

    def test_exact_output_cap_is_complete_and_successful(self):
        record = self.child("exact", "1024", max_output_bytes=1024)
        self.assertTrue(record["started"])
        self.assertEqual(record["returncode"], 0)
        self.assertEqual(record["stdout"], b"x" * 1024)
        self.assertEqual(record["stderr"], b"")
        self.assertFalse(record["output_limit"])
        self.assertFalse(record["timeout"])
        self.assertTrue(record["stdout_eof"] and record["stderr_eof"])

    def test_real_launch_failure_is_distinct_from_started_sigabrt(self):
        missing = self.observe([str(self.dir / "absent-program")])
        self.assertFalse(missing["started"])
        self.assertIsNone(missing["returncode"])
        self.assertIsNotNone(missing["launch_errno"])
        self.assertTrue(missing["launch_error"])
        fault = self.child("abort", quiet_faults=True)
        self.assertTrue(fault["started"])
        self.assertEqual(fault["returncode"], -signal.SIGABRT)
        self.assertIsNone(fault["launch_errno"])
        self.assertFalse(fault["timeout"] or fault["output_limit"])

    def test_stdout_stderr_and_aggregate_floods_are_bounded(self):
        for stream in ("stdout", "stderr", "both"):
            with self.subTest(stream=stream):
                record = self.child("flood", stream, max_output_bytes=1024)
                self.assertTrue(record["started"])
                self.assertTrue(record["output_limit"])
                self.assertFalse(record["timeout"])
                self.assertGreater(record["observed_stdout_bytes"] + record["observed_stderr_bytes"], 1024)
                if stream == "stdout":
                    self.assertTrue(record["stdout_limit"])
                    self.assertEqual(record["stderr"], b"")
                if stream == "stderr":
                    self.assertTrue(record["stderr_limit"])
                    self.assertEqual(record["stdout"], b"")

    def test_actual_timeout_kills_pipe_holding_descendants(self):
        started = time.monotonic()
        record = self.child("tree", timeout_s=0.2)
        self.assertLess(time.monotonic() - started, 2)
        self.assertTrue(record["started"] and record["timeout"])
        self.assertTrue(record["group_killed"])
        self.assertIsNotNone(record["returncode"])
        identities = json.loads(record["stdout"])
        for pid in identities.values():
            state = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)],
                                   text=True, capture_output=True, timeout=2)
            self.assertTrue(not state.stdout.strip() or state.stdout.strip().startswith("Z"),
                            f"fixture process {pid} survived: {state.stdout}")

    def test_expired_deadline_never_starts_child_and_live_deadline_limits_it(self):
        record = self.child("sleep", deadline=time.monotonic() - 1)
        self.assertFalse(record["started"])
        self.assertTrue(record["deadline_exhausted"])
        self.assertIsNone(record["returncode"])
        started = time.monotonic()
        record = self.child("sleep", timeout_s=2, deadline=started + 0.15)
        self.assertLess(time.monotonic() - started, 1)
        self.assertTrue(record["started"] and record["deadline_exhausted"])
        self.assertFalse(record["timeout"])
        self.assertIsNotNone(record["returncode"])

    def test_eof_ready_after_wall_budget_is_still_observed_as_timeout(self):
        factory = self.process.selectors.DefaultSelector
        class DelayedRealSelector:
            def __init__(self):
                self.actual = factory()
            def __getattr__(self, name):
                return getattr(self.actual, name)
            def select(self, timeout=None):
                ready = self.actual.select(timeout)
                if ready:
                    time.sleep(0.08)
                    # Query real readiness again; no events or process outcomes are fabricated.
                    return self.actual.select(0)
                return ready
        with mock.patch.object(self.process.selectors, 'DefaultSelector', DelayedRealSelector):
            record = self.observe(['/usr/bin/true'], timeout_s=0.04)
        self.assertTrue(record['started'])
        self.assertEqual(record['returncode'], 0)
        self.assertTrue(record['stdout_eof'] and record['stderr_eof'])
        self.assertGreaterEqual(record['elapsed_s'], 0.08)
        self.assertTrue(record['timeout'], 'Completion after the actual wall budget is inconclusive')


class ReportPreservationTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load('coax_native_check_execution', COAX / 'tools/native_check_execution.py')

    def report_inputs(self, directory):
        files = {}
        for name in ('coax', 'native-cc', 'python-launcher', 'quiet_crash.py', 'schema.pb', 'baseline-schema.pb', 'original.coax',
                     'SDKSettings.json', 'current.c', 'current.h', 'current.dylib',
                     'cached.c', 'cached.h', 'cached.dylib'):
            path = directory / name
            path.write_bytes(('disposable C1 input ' + name + '\n').encode())
            files[name] = path
        record = {'schema': 'coax.native-checks.execution.v1', 'outcome': 'not-checked',
                  'files': [{'file': str(files['original.coax']), 'profiles': {},
                    'baseline_input_manifest': {'sources': [], 'descriptors': [
                        {'path': str(files['baseline-schema.pb']), 'sha256': digest(files['baseline-schema.pb'])}]},
                    'emission_manifest': {'sources': [], 'descriptors': [
                        {'path': str(files['schema.pb']), 'sha256': digest(files['schema.pb'])}]}}],
                  'compiler': str(files['coax']),
                  'toolchain': {'resolved_program': str(files['native-cc']),
                    'version_observation': {'launched_argv': [str(files['python-launcher']),
                        str(files['quiet_crash.py']), '--', str(files['native-cc']), '--version']},
                    'sdk': {'settings_files': [{'path': str(files['SDKSettings.json']),
                        'sha256': digest(files['SDKSettings.json']), 'error': None}]}},
                  'policy': {'c': str(files['current.c']), 'header': str(files['current.h']),
                    'library': str(files['current.dylib']), 'sources': {},
                    'retained_bootstrap': {'c': str(files['cached.c']), 'header': str(files['cached.h']),
                                           'library': str(files['cached.dylib'])}}}
        return files, record

    def test_reports_refuse_known_native_cc_sdk_and_cached_policy_aliases(self):
        for role in ('native-cc', 'SDKSettings.json', 'cached.c', 'cached.h', 'cached.dylib',
                     'python-launcher', 'quiet_crash.py', 'schema.pb', 'baseline-schema.pb'):
            for alias in ('same', 'hardlink', 'symlink'):
                with self.subTest(role=role, alias=alias):
                    directory = self.dir / (role + '-' + alias)
                    directory.mkdir()
                    files, record = self.report_inputs(directory)
                    target = files[role]
                    original = target.read_bytes()
                    destination = target if alias == 'same' else directory / 'report-alias.json'
                    if alias == 'hardlink':
                        os.link(target, destination)
                    elif alias == 'symlink':
                        destination.symlink_to(target)
                    refusal = None
                    try:
                        self.execution.write_report(destination, record, compiler=files['coax'])
                    except ValueError as error:
                        refusal = str(error)
                    self.observations.append({'role': role, 'alias': alias, 'refusal': refusal,
                        'target': str(target), 'destination': str(destination),
                        'original_sha256': hashlib.sha256(original).hexdigest(),
                        'target_sha256_after': digest(target), 'destination_sha256_after': digest(destination)})
                    self.assertEqual(target.read_bytes(), original, 'report overwrote a known consumed input')
                    self.assertEqual(destination.read_bytes(), original, 'report replaced an input alias')
                    self.assertIsNotNone(refusal)
                    if alias == 'symlink':
                        self.assertTrue(destination.is_symlink())

    def test_preexisting_pid_temporary_sentinel_is_preserved_on_exclusive_open_failure(self):
        files, record = self.report_inputs(self.dir)
        destination = self.dir / 'report.json'
        destination.write_bytes(b'original report sentinel\n')
        temporary = destination.with_name(destination.name + '.' + str(os.getpid()) + '.tmp')
        temporary.write_bytes(b'preexisting other-writer sentinel\n')
        with self.assertRaises(FileExistsError):
            self.execution.write_report(destination, record, compiler=files['coax'])
        self.assertEqual(destination.read_bytes(), b'original report sentinel\n')
        self.assertEqual(temporary.read_bytes(), b'preexisting other-writer sentinel\n')


class ObservationPolicyTests(RecordedTests):
    """Declared independent byte/process vectors, never native execution receipts."""
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load('coax_native_check_execution', COAX / 'tools/native_check_execution.py')
        cls.vectors = json.loads((FIXTURES / 'ep1-vectors.json').read_text())

    def expected(self):
        identity = self.vectors['identity']
        return {'ordinal': identity['ordinal'], 'declaration_index': identity['declaration_index'],
                'line': identity['line'], 'column': identity['column'],
                'name': bytes.fromhex(identity['name_hex']), 'source': bytes.fromhex(identity['source_hex'])}

    def observation(self, vector='valid', **changes):
        wire = bytes.fromhex(self.vectors['execution'][vector])
        result = {'argv': ['declared-wire-fixture'], 'launched_argv': ['declared-wire-fixture'],
                  'cwd': str(REPO), 'started': True, 'pid': 12345, 'returncode': 0,
                  'launch_errno': None, 'launch_error': None, 'stdout': wire, 'stderr': b'',
                  'elapsed_s': 0.01, 'timeout': False, 'deadline_exhausted': False,
                  'stdout_limit': False, 'stderr_limit': False, 'output_limit': False,
                  'observed_stdout_bytes': len(wire), 'observed_stderr_bytes': 0,
                  'group_killed': False, 'kill_errno': None, 'read_errno': None,
                  'stdout_eof': True, 'stderr_eof': True,
                  'limits': {'stdout': 1024, 'stderr': 1024, 'total': 1024},
                  'timeout_s': 10, 'deadline': time.monotonic() + 10}
        result.update(changes)
        return result

    def classify(self, process):
        result = self.execution.classify_observation(self.expected(), process)
        self.observations.append({'declared_transport_fixture': True,
                                  'process_facts': process, 'policy_classification': result})
        return result

    def test_independent_complete_vector_preserves_nul_utf8_and_identity(self):
        expected = self.expected()
        self.assertEqual(expected['name'], b'alpha\0omega\xce\xbb')
        self.assertEqual(expected['source'], b'/a.coax')
        self.assertEqual(self.classify(self.observation())['outcome'], 'passed')

    def test_malformed_truncated_or_wrong_identity_success_streams_are_infrastructure(self):
        for name in ('truncated_end', 'orphan_end', 'changed_identity_end'):
            with self.subTest(vector=name):
                self.assertEqual(self.classify(self.observation(name))['outcome'], 'infrastructure-error')
        for wire in (self.observation()['stdout'] + b'\0', b'not EP1'):
            with self.subTest(wire=wire.hex()):
                self.assertEqual(self.classify(self.observation(stdout=wire,
                    observed_stdout_bytes=len(wire)))['outcome'], 'infrastructure-error')

    def test_complete_end_with_nonzero_exit_is_not_a_passing_check(self):
        for status in (7, -signal.SIGABRT):
            with self.subTest(status=status):
                self.assertEqual(self.classify(self.observation(returncode=status))['outcome'],
                                 'infrastructure-error')

    def test_started_begin_and_signal_is_raw_counterexample_without_recognized_trap(self):
        record = self.classify(self.observation('begun_only', returncode=-signal.SIGABRT))
        self.assertEqual(record['outcome'], 'counterexample')
        self.assertFalse(record.get('recognized_trap', False))
        self.assertIsNone(record.get('assertion_index'))

    def test_real_limit_facts_remain_inconclusive(self):
        changes = ({'timeout': True}, {'deadline_exhausted': True},
                   {'output_limit': True, 'stdout_limit': True,
                    'limits': {'stdout': 60, 'stderr': 60, 'total': 60},
                    'observed_stdout_bytes': 61})
        for flags in changes:
            with self.subTest(flags=flags):
                record = self.classify(self.observation('begun_only', returncode=-signal.SIGKILL,
                                                        group_killed=True, **flags))
                self.assertEqual(record['outcome'], 'inconclusive')

    def test_missing_start_read_error_or_missing_eof_cannot_qualify(self):
        policy = self.execution.get_policy(compiler=TRUSTED)
        self.assertEqual(self.execution.OUTCOMES[policy.phase(None)], 'infrastructure-error')
        cases = ({'started': False, 'pid': None, 'returncode': None, 'launch_errno': 2,
                  'launch_error': 'declared ENOENT', 'stdout': b'', 'observed_stdout_bytes': 0,
                  'stdout_eof': False, 'stderr_eof': False},
                 {'read_errno': 5}, {'stdout_eof': False}, {'stderr_eof': False})
        for changes in cases:
            with self.subTest(changes=changes):
                self.assertEqual(self.classify(self.observation(**changes))['outcome'],
                                 'infrastructure-error')

    def test_descriptor_manifests_require_explicit_lists_and_fresh_file_hashes(self):
        # Declared producer/process vectors test identity policy, not a native
        # qualification or the protobuf frontend. The byte mutations are real.
        policy = self.execution.get_policy(compiler=TRUSTED)
        source = (self.dir / 'declared-source.coax').resolve()
        source.write_text('#test "declared source" { #assert 3 == 3; }\n')
        descriptor = (self.dir / 'declared.pb').resolve()
        initial = b'descriptor input A\n'
        descriptor.write_bytes(initial)
        source_facts = [{'path': str(source), 'sha256': digest(source)}]
        evidence = {'sources': source_facts}
        manifest = {'schema_version': 1, 'command': 'test', 'backend': 'evaluator',
                    'compiler': 'declared-input-producer', 'runtime_arithmetic': 'checked',
                    'interpreter_arithmetic': 'checked', 'build_profile': 'development',
                    'success': True, 'exit_status': 0, 'sources': source_facts, 'descriptors': []}
        process = self.observation(stdout=b'', stderr=b'', observed_stdout_bytes=0)
        def emission_of(value):
            emitted = copy.deepcopy(value)
            emitted.update(command='checks-c', backend='c11')
            return emitted
        self.assertEqual(policy.baseline_inputs(evidence, manifest, process, manifest['compiler']), 0)
        self.assertEqual(policy.input_descriptors(manifest, emission_of(manifest)), 0)
        wrong_backend = copy.deepcopy(manifest)
        wrong_backend['backend'] = 'c11'
        self.assertNotEqual(policy.baseline_inputs(evidence, wrong_backend, process, manifest['compiler']), 0)
        coherent = copy.deepcopy(manifest)
        coherent['descriptors'] = [{'path': str(descriptor), 'sha256': digest(descriptor)}]
        self.assertEqual(policy.baseline_inputs(evidence, coherent, process, manifest['compiler']), 0)
        self.assertEqual(policy.input_descriptors(coherent, emission_of(coherent)), 0)
        for wrong in ('missing', None, {}, 'not a descriptor list'):
            malformed = copy.deepcopy(coherent)
            if wrong == 'missing':
                malformed.pop('descriptors')
            else:
                malformed['descriptors'] = wrong
            self.assertNotEqual(policy.baseline_inputs(evidence, malformed, process, manifest['compiler']), 0)
            self.assertNotEqual(policy.input_descriptors(malformed, emission_of(coherent)), 0)
            self.assertNotEqual(policy.input_descriptors(coherent, emission_of(malformed)), 0)
        descriptor.write_bytes(b'descriptor input B\n')
        later = copy.deepcopy(coherent)
        later['descriptors'][0]['sha256'] = digest(descriptor)
        self.assertNotEqual(policy.input_descriptors(coherent, emission_of(later)), 0)
        self.assertNotEqual(policy.baseline_inputs(evidence, coherent, process, manifest['compiler']), 0)
        descriptor.write_bytes(initial)
        self.assertEqual(policy.input_descriptors(coherent, emission_of(coherent)), 0)
        captured_end = self.classify(self.observation())
        self.assertEqual(captured_end['outcome'], 'passed')
        descriptor.write_bytes(b'descriptor changed after declared END capture\n')
        after = policy.input_descriptors(coherent, emission_of(coherent))
        self.assertNotEqual(after, 0)
        self.assertNotEqual(policy.aggregate([captured_end['outcome_code'], after]), 0)
        self.observations.append({'declared_input_identity_vectors': True,
                                  'descriptor': str(descriptor), 'final_observed_sha256': digest(descriptor)})


class ExecutionTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load("coax_native_check_execution", COAX / "tools/native_check_execution.py")
        cls.good = None
        cls.good_commands = None

    def run_fixture(self, name, **options):
        replay = os.environ.get('COAX_EXECUTION_REPLAY_RECEIPT')
        if replay:
            receipt = json.loads(Path(replay).read_text())
            self.assertEqual(receipt['method'], self._testMethodName)
            result = next(value for value in receipt['observations']
                          if isinstance(value, dict) and value.get('schema') == 'coax.native-checks.execution.v1')
            self.assertEqual(result['compiler_sha256'], digest(TRUSTED))
            self.assertEqual(Path(result['files'][0]['file']).resolve(), (FIXTURES / (name + '.coax')).resolve())
            self.assertEqual(result['files'][0]['source_sha256'], digest(FIXTURES / (name + '.coax')))
            self.observations.append({'assertion_replay_of_real_receipt': replay, 'retained_record': result})
            return result
        counter = getattr(self, '_fixture_calls', 0)
        self._fixture_calls = counter + 1
        result = self.execution.run([FIXTURES / (name + '.coax')], keep=self.dir / f'artifacts-{counter}',
                                    compiler=TRUSTED, **options)
        self.observations.append(result)
        return result

    def known_good(self):
        cls = type(self)
        if cls.good is None:
            cache = os.environ.get('COAX_EXECUTION_GOOD_RECEIPT')
            bindings = {str(path): digest(path) for path in (
                TRUSTED, FIXTURES / 'identity.coax', FIXTURES / 'loaded.coax',
                COAX / 'tools/native_check_execution.py', COAX / 'tools/bounded_process.py',
                COAX / 'tools/native_execution_policy.py',
                COAX / 'tools/native-check-observer.c', COAX / 'tools/native-check-observer.h',
                COAX / 'lib/compiler/native-execution.coax', COAX / 'lib/compiler/native-checks.coax',
                COAX / 'lib/compiler/mutation-schedule.coax', COAX / 'lib/compiler/budget.coax')}
            if cache and Path(cache).exists():
                def decode(value):
                    return bytes.fromhex(value['hex']) if set(value) == {'hex'} else value
                saved = json.loads(Path(cache).read_text(), object_hook=decode)
                self.assertEqual(saved['bindings'], bindings, 'Saved positive tool/source identity changed')
                cls.good, cls.good_commands = saved['record'], saved['commands']
            else:
                commands = []
                execute = self.execution.process.run
                def observe(argv, *arguments, **options):
                    commands.append([str(value) for value in argv])
                    return execute(argv, *arguments, **options)
                with mock.patch.object(self.execution.process, 'run', side_effect=observe):
                    record = self.run_fixture('identity')
                self.assertEqual(record['outcome'], 'passed', record)
                cls.good, cls.good_commands = record, commands
                if cache:
                    Path(cache).write_text(json.dumps({'bindings': bindings, 'record': record,
                        'commands': commands}, indent=2, default=serializable) + '\n')
        self.observations.append({'retained_positive_record': cls.good,
                                  'retained_positive_commands': cls.good_commands})
        return cls.good

    def test_real_original_ast_execution_passes_with_exact_identities(self):
        record = self.known_good()
        if os.environ.get('COAX_EXECUTION_RED_RECORD'):
            Path(os.environ['COAX_EXECUTION_RED_RECORD']).write_text(
                json.dumps(record, indent=2, default=serializable) + '\n')
        self.assertEqual(record['outcome'], 'passed',
                         'Passing imported, indented and duplicate-name checks need actual AST execution.')
        self.assertEqual(record['schema'], 'coax.native-checks.execution.v1')
        self.assertEqual(len(record['files']), 1)
        item = record['files'][0]
        self.assertIn('baseline_input_manifest', item)
        self.assertIsInstance(item['baseline_input_manifest']['descriptors'], list)
        evidence = item['baseline']
        self.assertEqual(evidence['verdict'], 'passed')
        self.assertIsNone(evidence['selection']['only'])
        self.assertEqual(evidence['compiler']['sha256'], digest(TRUSTED))
        original = (FIXTURES / 'identity.coax').resolve()
        self.assertEqual(Path(evidence['input']).resolve(), original)
        sources = {str(Path(row['path']).resolve()): row['sha256'] for row in evidence['sources']}
        self.assertEqual(sources[str(original)], digest(original))
        self.assertEqual(sources[str((FIXTURES / 'loaded.coax').resolve())], digest(FIXTURES / 'loaded.coax'))
        commands = type(self).good_commands
        baselines = [argv for argv in commands if len(argv) > 2 and argv[1] == 'test'
                     and Path(argv[2]).resolve() == original]
        emissions = [argv for argv in commands if len(argv) > 2 and argv[1] == 'checks-c'
                     and Path(argv[2]).resolve() == original]
        self.assertEqual(len(baselines), 1)
        self.assertNotIn('--only', baselines[0])
        self.assertIn('--profile-report', baselines[0])
        self.assertEqual(baselines[0][baselines[0].index('--profile-report') + 1], '/dev/stdout')
        self.assertEqual(len(emissions), 1)
        artifact = item['artifact']
        for field in ('c', 'observer', 'policy_c', 'policy_header'):
            self.assertEqual(digest(artifact[field]), artifact[field + '_sha256'])
        expected_name = 'receipt [x] quote" slash\\ nul\0 tail λ'
        expected = {(str(original), 2, 1, 'same display'),
                    (str(original), 3, 3, expected_name),
                    (str((FIXTURES / 'loaded.coax').resolve()), 2, 1, 'same display')}
        actual = {(str(Path(row['at']['file']).resolve()), row['at']['line'], row['at']['col'], row['name'])
                  for row in item['native_coverage'] if row['kind'] == '#test'}
        self.assertEqual(actual, expected)
        self.assertTrue(all(row['selection'] == 'selected-test' for row in item['native_coverage']))
        self.assertEqual(set(item['profiles']), {'-O0', '-O2'})
        for profile in item['profiles'].values():
            self.assertEqual(profile['outcome'], 'passed')
            self.assertEqual(digest(profile['binary']), profile['binary_sha256'])
            self.assertEqual(profile['compile']['returncode'], 0)
            self.assertEqual(profile['execution']['outcome'], 'passed')
            self.assertEqual(len(profile['execution']['checks']), 3)

    def test_failed_original_baselines_cannot_reach_native_emission(self):
        for name in ('baseline-failure', 'authority-failure', 'native-failure'):
            with self.subTest(source=name):
                commands = []
                execute = self.execution.process.run
                def observe(argv, *arguments, **options):
                    commands.append([str(value) for value in argv])
                    return execute(argv, *arguments, **options)
                with mock.patch.object(self.execution.process, 'run', side_effect=observe):
                    record = self.run_fixture(name, only='passing selected')
                self.assertNotEqual(record['outcome'], 'passed')
                original = (FIXTURES / (name + '.coax')).resolve()
                baselines = [argv for argv in commands if len(argv) > 2 and argv[1] == 'test'
                             and Path(argv[2]).resolve() == original]
                self.assertEqual(len(baselines), 1)
                self.assertNotIn('--only', baselines[0])
                self.assertFalse(any(len(argv) > 1 and argv[1] == 'checks-c' for argv in commands))
                self.assertFalse(any(any(Path(value).name in ('native-check-observer.c', 'checks.c')
                                             for value in argv) for argv in commands))
                self.assertFalse(any(len(argv) > 1 and argv[1] in ('metadata', 'one') for argv in commands))
                self.assertFalse(record['files'][0]['profiles'])
                self.assertEqual(record['files'][0]['baseline']['selection']['only'], None)

    def test_above_default_steps_are_refused_before_user_source_execution(self):
        commands = []
        execute = self.execution.process.run
        source = (FIXTURES / 'identity.coax').resolve()
        def observe(argv, *arguments, **options):
            values = [str(value) for value in argv]
            commands.append(values)
            if len(values) > 2 and values[1] in ('test', 'checks-c') and Path(values[2]).resolve() == source:
                self.fail('A refused step budget reached user-source execution')
            return execute(argv, *arguments, **options)
        with mock.patch.object(self.execution.process, 'run', side_effect=observe):
            record = self.run_fixture('identity', steps=20_000_001)
        self.observations.append({'pre_effect_step_rejection_commands': commands})
        self.assertNotEqual(record['outcome'], 'passed')
        self.assertIn('step budget', record['why'])
        self.assertFalse(record['files'])

    def test_only_qualifies_exact_requested_scope_with_original_indices(self):
        source = (FIXTURES / 'identity.coax').resolve()
        loaded = (FIXTURES / 'loaded.coax').resolve()
        name = b'receipt [x] quote" slash\\ nul\0 tail \xce\xbb'
        cases = [('tail λ', [{'ordinal': 0, 'declaration_index': 1, 'line': 3, 'column': 3,
                              'name': name, 'source': str(source).encode()}]),
                 ('same display', [{'ordinal': 0, 'declaration_index': 0, 'line': 2, 'column': 1,
                                    'name': b'same display', 'source': str(source).encode()},
                                   {'ordinal': 1, 'declaration_index': 2, 'line': 2, 'column': 1,
                                    'name': b'same display', 'source': str(loaded).encode()}])]
        for query, expected in cases:
            with self.subTest(query=query):
                observations = []
                execute = self.execution.process.run
                def observe(argv, *arguments, **options):
                    result = execute(argv, *arguments, **options)
                    observations.append(([str(value) for value in argv], copy.deepcopy(result)))
                    return result
                with mock.patch.object(self.execution.process, 'run', side_effect=observe):
                    record = self.run_fixture('identity', only=query)
                self.assertEqual(record['outcome'], 'passed', record)
                item = record['files'][0]
                self.assertIsNone(item['baseline']['selection']['only'])
                selected = [row for row in item['native_coverage'] if row['selection'] == 'selected-test']
                unselected = [row for row in item['native_coverage'] if row['selection'] == 'not-selected']
                self.assertEqual(len(selected), len(expected))
                self.assertEqual(len(selected) + len(unselected), 3)
                for profile in item['profiles'].values():
                    binary = profile['binary']
                    metadata = [raw for argv, raw in observations if argv[:2] == [str(binary), 'metadata']]
                    self.assertEqual(len(metadata), 1)
                    header = b'COAXEP1\0' + struct.pack('<III', 1, 20, len(expected))
                    self.assertEqual(metadata[0]['stdout'], header + b''.join(
                        self.independent_frame(2, row) for row in expected))
                    self.assertEqual(len(profile['execution']['checks']), len(expected))

    def test_selected_forall_is_an_explicit_native_coverage_gap(self):
        record = self.run_fixture('selected-forall', only='selected quantified')
        self.assertNotEqual(record['outcome'], 'passed')
        item = record['files'][0]
        self.assertEqual(item['baseline']['verdict'], 'passed')
        quantified = [row for row in item['native_coverage'] if row['kind'] == '#forall']
        self.assertEqual(len(quantified), 1)
        self.assertEqual(quantified[0]['selection'], 'selected-forall')
        self.assertEqual(item['native_selection']['outcome'], 'not-checked')
        self.assertFalse(item['profiles'])

    def test_successful_authority_aggregate_stays_evaluator_only(self):
        record = self.run_fixture('authority-pass')
        self.assertEqual(record['outcome'], 'passed', record)
        item = record['files'][0]
        authority = [row for row in item['baseline']['claims'] if row['kind'] == '#authority']
        self.assertEqual(len(authority), 1)
        self.assertEqual(authority[0]['cases'], 0)
        self.assertEqual(authority[0]['steps'], 0)
        self.assertIsNone(authority[0]['at'])
        self.assertEqual(len([row for row in item['native_coverage'] if row['kind'] == '#test']), 1)
        self.assertTrue(all(row['selection'] == 'evaluator-only'
                            for row in item['native_coverage'] if row['kind'] == '#authority'))
        for profile in item['profiles'].values():
            self.assertEqual(len(profile['execution']['checks']), 1)

    def test_missing_native_cc_reports_real_launch_failure(self):
        missing = self.dir / 'missing-native-cc'
        record = self.run_fixture('identity', cc=[str(missing)])
        self.assertEqual(record['outcome'], 'infrastructure-error')
        chain = record['toolchain']
        self.assertIsNone(chain['resolved_program'])
        self.assertTrue(chain['resolution_error'])
        for field in ('version_observation', 'target_observation'):
            process = chain[field]
            output = base64.b64decode(process['stdout']['base64'], validate=True)
            self.assertEqual(output, b'')
            self.assertEqual(len(output), process['stdout']['bytes'])
            self.assertEqual(hashlib.sha256(output).hexdigest(), process['stdout']['sha256'])
            if process['started']:
                self.assertNotEqual(process['launched_argv'], process['argv'])
                self.assertTrue(any(Path(value).name == 'quiet_crash.py' for value in process['launched_argv']))
                self.assertEqual(process['returncode'], 127)
                self.assertIsNone(process['launch_errno'])
            else:
                self.assertTrue(process['launch_errno'])
                self.assertIsNone(process['returncode'])
        self.assertFalse(record['files'][0]['profiles'])

    def test_actual_cc_diagnostic_and_nonzero_status_are_retained(self):
        header = FIXTURES / 'compile-error.h'
        cc = shutil.which(os.environ.get('COAX_NATIVE_CC', 'cc'))
        self.assertIsNotNone(cc)
        attempts = []
        execute = self.execution.process.run
        def observe(argv, *arguments, **options):
            result = execute(argv, *arguments, **options)
            if str(header) in [str(value) for value in argv] and '-std=c11' in argv:
                attempts.append(copy.deepcopy(result))
            return result
        with mock.patch.object(self.execution.process, 'run', side_effect=observe):
            record = self.run_fixture('identity', cc=[cc, '-include', str(header)])
        self.assertEqual(record['outcome'], 'infrastructure-error')
        self.assertTrue(attempts)
        self.assertTrue(all(row['started'] and row['returncode'] != 0 for row in attempts))
        self.assertTrue(any(b'C1_fixture_requires_a_truthful_compile_failure' in row['stderr'] for row in attempts))
        self.observations.append({'actual_native_cc_errors': attempts})

    def test_missing_binary_never_qualifies_as_executed(self):
        expected = {'ordinal': 0, 'declaration_index': 0, 'line': 2, 'column': 1,
                    'name': b'started native failure',
                    'source': str((FIXTURES / 'native-failure.coax').resolve()).encode()}
        record = self.execution.observe_artifact(self.dir / 'missing-binary', [expected])
        self.observations.append({'transport_only_missing_binary': record})
        self.assertEqual(record['outcome'], 'infrastructure-error')

    def transport_fixture(self, mode, **options):
        identity = json.loads((FIXTURES / 'ep1-vectors.json').read_text())['identity']
        expected = {key: identity[key] for key in ('ordinal', 'declaration_index', 'line', 'column')}
        expected.update(name=bytes.fromhex(identity['name_hex']), source=bytes.fromhex(identity['source_hex']))
        attempts = []
        execute = self.execution.process.run
        fixture = FIXTURES / 'transport-child.py'
        def observe(argv, *arguments, **settings):
            result = execute(argv, *arguments, **settings)
            if str(argv[0]) == str(fixture):
                attempts.append(copy.deepcopy(result))
            return result
        with mock.patch.dict(os.environ, {'COAX_EXECUTION_TEST_TRANSPORT_MODE': mode}), \
                mock.patch.object(self.execution.process, 'run', side_effect=observe):
            result = self.execution.observe_artifact(fixture, [expected], **options)
        self.observations.append({'broken_transport_fixture': mode, 'result': result,
                                  'actual_processes': attempts})
        self.assertEqual(len(attempts), 1)
        self.assertTrue(attempts[0]['started'])
        return result, attempts[0]

    def test_internal_transport_applies_actual_time_and_output_caps(self):
        record, process = self.transport_fixture('sleep', timeout_s=0.2)
        self.assertLess(process['elapsed_s'], 2)
        self.assertTrue(process['timeout'])
        self.assertEqual(record['outcome'], 'inconclusive')
        record, process = self.transport_fixture('output', max_output_bytes=16)
        self.assertTrue(process['output_limit'])
        self.assertLessEqual(len(process['stdout']) + len(process['stderr']), 16)
        self.assertEqual(record['outcome'], 'inconclusive')

    def test_actual_broken_success_stream_and_complete_end_nonzero_remain_infrastructure(self):
        for mode in ('truncated-success', 'malformed-success', 'complete-nonzero'):
            with self.subTest(mode=mode):
                record, process = self.transport_fixture(mode)
                self.assertEqual(process['returncode'], 7 if mode == 'complete-nonzero' else 0)
                self.assertEqual(record['outcome'], 'infrastructure-error')

    @staticmethod
    def independent_frame(kind, expected):
        payload = struct.pack('<6I', expected['ordinal'], expected['declaration_index'],
                              expected['line'], expected['column'],
                              len(expected['name']), len(expected['source']))
        payload += expected['name'] + expected['source']
        return b'COAXEP1\0' + struct.pack('<II', kind, 16 + len(payload)) + payload

    def test_actual_started_failing_coax_body_is_raw_transport_counterexample(self):
        # This explicitly bypasses default run admission only to observe transport;
        # the default failed-baseline case above must never run this body.
        cache = os.environ.get('COAX_EXECUTION_GOOD_RECEIPT')
        if cache and Path(cache).exists():
            saved = json.loads(Path(cache).read_text())
            good = saved['record']['files'][0]
            # Only immutable compiled inputs are reused here; the old producer
            # Python identity is retained, never relabeled as a fresh run.
            for path, old_hash in saved['bindings'].items():
                if Path(path).suffix != '.py':
                    self.assertEqual(digest(path), old_hash)
            for field in ('c', 'observer', 'policy_c', 'policy_header'):
                self.assertEqual(digest(good['artifact'][field]), good['artifact'][field + '_sha256'])
            self.observations.append({'retained_immutable_artifacts': good['artifact'],
                                      'original_producer_bindings': saved['bindings']})
        else:
            good = self.known_good()['files'][0]
        policy_c = Path(good['artifact']['policy_c'])
        observer = Path(good['artifact']['observer'])
        source = (FIXTURES / 'native-failure.coax').resolve()
        emitted = self.dir / 'actual-failing-body.c'
        binary = self.dir / 'actual-failing-body'
        deadline = time.monotonic() + 30
        def command(argv, quiet=False):
            result = self.execution.process.run(argv, cwd=str(REPO), env=None, timeout_s=10,
                max_output_bytes=65536, deadline=deadline, quiet_faults=quiet)
            self.observations.append({'actual_artifact_command': result})
            return result
        emission = command([str(TRUSTED), 'checks-c', str(source), '-o', str(emitted)])
        self.assertEqual(emission['returncode'], 0, emission)
        cc = shutil.which(os.environ.get('COAX_NATIVE_CC', 'cc'))
        built = command([cc, '-std=c11', '-O0', '-ffp-contract=off', '-fno-fast-math',
            '-I', str(policy_c.parent), str(emitted), str(policy_c), str(observer), '-lm', '-o', str(binary)])
        self.assertEqual(built['returncode'], 0, built)
        expected = {'ordinal': 0, 'declaration_index': 0, 'line': 2, 'column': 1,
                    'name': b'started native failure', 'source': str(source).encode()}
        metadata = command([str(binary), 'metadata'], quiet=True)
        catalog = bytes.fromhex('434f415845503100010000001400000001000000')
        catalog += self.independent_frame(2, expected)
        self.assertEqual(metadata['returncode'], 0, metadata)
        self.assertEqual(metadata['stdout'], catalog)
        attempts = []
        execute = self.execution.process.run
        def observe(argv, *arguments, **options):
            result = execute(argv, *arguments, **options)
            if Path(argv[0]).resolve() == binary.resolve() and len(argv) > 1 and argv[1] == 'one':
                attempts.append(copy.deepcopy(result))
            return result
        with mock.patch.object(self.execution.process, 'run', side_effect=observe):
            record = self.execution.observe_artifact(binary, [expected], deadline=deadline)
        self.observations.append({'transport_only_actual_failing_body': record,
                                  'c_sha256': digest(emitted), 'binary_sha256': digest(binary)})
        self.assertEqual(record['outcome'], 'counterexample')
        self.assertEqual(len(attempts), 1)
        process = attempts[0]
        self.assertTrue(process['started'])
        self.assertEqual(process['returncode'], -signal.SIGABRT)
        self.assertEqual(process['stdout'], self.independent_frame(3, expected))
        self.assertIn(b'native-failure.coax', process['stderr'])
        self.assertFalse(record.get('recognized_trap', False))
        self.assertIsNone(record.get('assertion_index'))


if __name__ == "__main__":
    unittest.main()

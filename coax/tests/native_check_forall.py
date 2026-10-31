#!/usr/bin/env python3
"""Independent C2 finite-domain, original-AST and native-progress expectations."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import signal
import sys
import tempfile
import time
import unittest
from unittest import mock


COAX = Path(__file__).resolve().parents[1]
REPO = COAX.parent
FIXTURES = COAX / 'tests/native-check-forall'
TRUSTED = Path(os.environ.get('COAX_FORALL_TEST_COMPILER',
                             os.environ.get('COAX_TEST_COMPILER', COAX / 'bin/coax'))).resolve()


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


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


def serialized(value):
    if isinstance(value, bytes):
        return {'hex': value.hex()}
    if isinstance(value, Path):
        return str(value)
    raise TypeError(type(value).__name__)


class RecordedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.record_root = Path(os.environ.get('COAX_FORALL_RECORD_DIR') or
                               tempfile.mkdtemp(prefix='coax-native-forall-'))
        cls.record_root.mkdir(parents=True, exist_ok=True)
        cls.process = load('coax_forall_bounded_process', COAX / 'tools/bounded_process.py')

    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix=self._testMethodName + '-', dir=self.record_root))
        self.started = time.monotonic()
        self.observations = []
        self.emissions = {}
        self.fixtures_before = {str(p): digest(p) for p in FIXTURES.iterdir() if p.is_file()}
        self.compiler_before = digest(TRUSTED) if TRUSTED.exists() else None

    def tearDown(self):
        record = {'method': self._testMethodName, 'seconds': time.monotonic() - self.started,
                  'harness_sha256': digest(__file__), 'observations': self.observations,
                  'fixtures_before': self.fixtures_before,
                  'fixtures_after': {str(p): digest(p) for p in FIXTURES.iterdir() if p.is_file()}}
        if TRUSTED.exists():
            record['compiler'] = {'path': str(TRUSTED), 'sha256': digest(TRUSTED)}
        policy = getattr(self, 'policy', None)
        if policy is not None:
            record['policy_identity'] = policy.identity
        (self.dir / 'test-receipt.json').write_text(json.dumps(record, indent=2, default=serialized) + '\n')
        self.assertEqual(record['fixtures_before'], record['fixtures_after'], 'original fixture bytes changed')
        if self.compiler_before:
            self.assertEqual(digest(TRUSTED), self.compiler_before, 'compiler changed during a focused case')

    def compiler(self, *arguments):
        return self.command([str(TRUSTED), *map(str, arguments)])

    def command(self, argv):
        result = self.process.run(list(map(str, argv)), cwd=str(REPO), env=None,
                                  timeout_s=10, max_output_bytes=1048576,
                                  deadline=time.monotonic() + 30, quiet_faults=True)
        self.observations.append(result)
        return result


class CompilerTests(RecordedTests):
    def emit(self, name, *options):
        output = self.dir / ('checks-' + str(len(self.observations)) + '.c')
        result = self.compiler('checks-c', FIXTURES / (name + '.coax'), '--verification-abi', '2',
                               *options, '-o', output)
        self.assertEqual(result['returncode'], 0, result['stderr'].decode(errors='replace'))
        self.assertTrue(output.is_file())
        return output

    def link(self, emitted, optimization):
        binary = self.dir / ('native-' + str(len(self.observations)) + optimization)
        result = self.command([*shlex.split(os.environ.get('CC', 'cc')), '-std=c11', optimization,
                               '-ffp-contract=off', '-fno-fast-math', emitted,
                               FIXTURES / 'table-driver.c', '-lm', '-o', binary])
        self.assertEqual(result['returncode'], 0, result['stderr'].decode(errors='replace'))
        self.observations.append({'c': str(emitted), 'c_sha256': digest(emitted),
                                  'binary': str(binary), 'binary_sha256': digest(binary)})
        return binary

    def metadata(self, binary):
        result = self.command([binary, 'metadata'])
        self.assertEqual(result['returncode'], 0, result['stderr'].decode(errors='replace'))
        rows = []
        for line in result['stdout'].decode('ascii').splitlines():
            fields = line.split('\t')
            if fields[0] == 'C':
                ordinal, index, kind, cases, source_line, column = map(int, fields[1:7])
                self.assertEqual(ordinal, len(rows))
                rows.append({'declaration_index': index, 'kind': kind, 'case_count': cases,
                             'line': source_line, 'column': column, 'name': bytes.fromhex(fields[7]),
                             'source': bytes.fromhex(fields[8]), 'bindings': []})
            elif fields[0] == 'B':
                ordinal, index, kind, width, signed, first, count, value_count = map(int, fields[1:9])
                self.assertEqual(index, len(rows[ordinal]['bindings']))
                rows[ordinal]['bindings'].append({'kind': kind, 'width': width, 'is_signed': signed,
                    'first': first, 'count': count, 'value_count': value_count,
                    'name': bytes.fromhex(fields[9]), 'type_name': bytes.fromhex(fields[10]), 'values': []})
            elif fields[0] == 'E':
                ordinal, binding, index, bits = map(int, fields[1:5])
                values = rows[ordinal]['bindings'][binding]['values']
                self.assertEqual(index, len(values))
                values.append({'bits': bits, 'label': bytes.fromhex(fields[5])})
            else:
                self.fail('Unknown independent metadata row: ' + line)
        self.assertTrue(rows)
        return rows

    def test_abi2_cli_emits_original_quantified_table(self):
        result = self.compiler('checks-c', FIXTURES / 'order.coax', '--verification-abi', '2')
        self.assertEqual(result['returncode'], 0, result['stderr'].decode(errors='replace'))
        self.assertIn(b'coax_verification_checks_v2', result['stdout'])
        self.assertNotIn(b'int main(', result['stdout'])

    def test_typed_domain_metadata_and_original_wrapper_carriers_at_o0_o2(self):
        emitted = self.emit('typed-domains')
        expected_names = [b'signed byte', b'unsigned byte', b'signed word', b'unsigned word',
                          b'sparse enum declaration order', b'minimum int range',
                          b'maximum int range', b'folded constant range']
        expected_counts = [256, 256, 65536, 65536, 12, 2, 2, 4]
        vectors = [
            [0, 1, 18446744073709551488], [0, 1, 127], [1, 1, 0], [1, 1, 255],
            [2, 1, 18446744073709518848], [2, 1, 32767], [3, 1, 0], [3, 1, 65535],
            [4, 2, 9223372036854775808, 18446744073709551615],
            [4, 2, 18446744073709551609, 3], [4, 2, 9223372036854775807, 9223372036854775808],
            [5, 1, 9223372036854775808], [5, 1, 9223372036854775809],
            [6, 1, 9223372036854775805], [6, 1, 9223372036854775806],
            [7, 1, 18446744073709551615], [7, 1, 2],
        ]
        carrier_file = self.dir / 'independent-tuples.txt'
        carrier_file.write_text(''.join(' '.join(map(str, row)) + '\n' for row in vectors))
        previous = None
        for optimization in ('-O0', '-O2'):
            binary = self.link(emitted, optimization)
            rows = self.metadata(binary)
            self.assertEqual([r['name'] for r in rows], expected_names)
            self.assertEqual([r['declaration_index'] for r in rows], list(range(8)))
            self.assertEqual([r['kind'] for r in rows], [2] * 8)
            self.assertEqual([r['case_count'] for r in rows], expected_counts)
            for row, kind, width, signed, first, count, name in zip(rows[:4], (2,3,2,3), (8,8,16,16),
                    (1,0,1,0), (-128,0,-32768,0), (256,256,65536,65536), (b's8',b'u8',b's16',b'u16')):
                binding = row['bindings'][0]
                self.assertEqual({k: binding[k] for k in ('kind','width','is_signed','first','count','name','type_name','values')},
                    {'kind':kind,'width':width,'is_signed':signed,'first':first,'count':count,
                     'name':b'value','type_name':name,'values':[]})
            self.assertEqual(rows[4]['bindings'][0]['values'], [
                {'bits':9223372036854775808,'label':b'minimum'},
                {'bits':18446744073709551609,'label':b'negative'},
                {'bits':0,'label':b'zero'}, {'bits':9223372036854775807,'label':b'maximum'}])
            self.assertEqual(rows[4]['bindings'][1]['values'], [
                {'bits':18446744073709551615,'label':b'maximum'},
                {'bits':3,'label':b'three'}, {'bits':9223372036854775808,'label':b'high'}])
            for row, first, count in zip(rows[5:], (-9223372036854775808,9223372036854775805,-1), (2,2,4)):
                self.assertEqual({key:row['bindings'][0][key] for key in ('kind','width','is_signed','first','count','type_name')},
                                 {'kind':5,'width':64,'is_signed':1,'first':first,'count':count,'type_name':b'int'})
            if previous is not None:
                self.assertEqual(rows, previous)
            previous = rows
            guards = self.command([binary, 'guards'])
            self.assertEqual((guards['returncode'], guards['stdout']), (0, b'guarded 8 original wrappers\n'))
            called = self.command([binary, 'vectors', carrier_file])
            self.assertEqual((called['returncode'], called['stdout']), (0, b'ran 17 original body calls\n'))

    def test_duplicate_imported_nul_identity_and_filter_preserve_declaration_indices(self):
        emitted = self.emit('identity')
        binary = self.link(emitted, '-O0')
        rows = self.metadata(binary)
        self.assertEqual([r['declaration_index'] for r in rows], [0,1,2,3])
        self.assertEqual([r['kind'] for r in rows], [1,2,2,2])
        self.assertEqual([r['case_count'] for r in rows], [1,3,4,2])
        self.assertEqual([r['name'] for r in rows], [b'duplicate original identity'] * 2 +
                         ['property quote" nul\0 tail λ'.encode(), b'duplicate original identity'])
        self.assertEqual(rows[3]['source'], str((FIXTURES / 'loaded.coax').resolve()).encode())
        self.assertEqual(rows[2]['source'], str((FIXTURES / 'identity.coax').resolve()).encode())
        self.assertEqual((rows[2]['line'], rows[2]['column']), (4,3))
        self.assertEqual([b['name'] for b in rows[2]['bindings']], [b'offset', b'enabled'])
        selected = self.emit('identity', '--only', 'tail λ')
        selected_rows = self.metadata(self.link(selected, '-O0'))
        self.assertEqual(selected_rows, [rows[2]])

    def test_invalid_source_domains_and_aliases_refuse_without_replacing_output(self):
        for name, code in (('empty-range','E0379'), ('decreasing-range','E0379'),
                           ('empty-enum','E0379'), ('excessive-range','E0400'),
                           ('excessive-product','E0400'), ('unsupported-wide','E0379'),
                           ('enum-aliases','E0363')):
            with self.subTest(source=name):
                output = self.dir / (name + '.c')
                output.write_bytes(b'untouched verification output\n')
                result = self.compiler('checks-c', FIXTURES / (name + '.coax'),
                                       '--verification-abi', '2', '-o', output)
                self.assertNotEqual(result['returncode'], 0)
                self.assertIn(code.encode(), result['stderr'])
                self.assertIn((name + '.coax:').encode(), result['stderr'])
                self.assertEqual(output.read_bytes(), b'untouched verification output\n')

    def test_exact_product_cap_is_metadata_only_and_default_abi1_still_refuses_forall(self):
        emitted = self.emit('exact-product')
        rows = self.metadata(self.link(emitted, '-O0'))
        self.assertEqual(rows[0]['case_count'], 16777216)
        self.assertEqual([b['count'] for b in rows[0]['bindings']], [256,65536])
        ordinary = self.compiler('checks-c', FIXTURES / 'order.coax')
        self.assertNotEqual(ordinary['returncode'], 0)
        self.assertIn(b'E0413', ordinary['stderr'])

    def test_native_arity_limit_is_distinct_from_singleton_case_product(self):
        original = self.compiler('test', FIXTURES / 'arity-65.coax', '--library')
        self.assertEqual(original['returncode'], 0, original['stderr'].decode(errors='replace'))
        emitted = self.emit('arity-64')
        binary = self.link(emitted, '-O0')
        rows = self.metadata(binary)
        self.assertEqual(rows[0]['case_count'], 1)
        self.assertEqual(len(rows[0]['bindings']), 64)
        self.assertEqual([b['name'] for b in rows[0]['bindings']],
                         [('b%02d' % i).encode() for i in range(64)])
        carrier = self.dir / 'arity-64.txt'
        carrier.write_text('0 64 ' + ' '.join(['0'] * 64) + '\n')
        called = self.command([binary, 'vectors', carrier])
        self.assertEqual((called['returncode'], called['stdout']), (0, b'ran 1 original body calls\n'))
        refused = self.compiler('checks-c', FIXTURES / 'arity-65.coax', '--verification-abi', '2')
        self.assertNotEqual(refused['returncode'], 0)
        self.assertIn(b'E0413', refused['stderr'])
        self.assertIn(b'arity-65.coax:', refused['stderr'])

    def test_original_authority_failure_is_unfiltered_by_abi2_selection(self):
        output = self.dir / 'authority.c'
        output.write_bytes(b'untouched authority output\n')
        refused = self.compiler('checks-c', FIXTURES / 'authority-failure.coax',
                                '--verification-abi', '2', '--only', 'selected authority passing', '-o', output)
        self.assertNotEqual(refused['returncode'], 0)
        self.assertIn(b'authority-failure.coax:', refused['stderr'])
        self.assertEqual(output.read_bytes(), b'untouched authority output\n')

    def test_original_generic_closure_excludes_unrelated_product_main_foreign_and_defaults(self):
        emitted = self.emit('unrelated-product')
        text = emitted.read_text()
        self.assertNotIn('coax_p_main', text)
        self.assertNotIn('unrelated_foreign', text)
        carrier = self.dir / 'closure-tuples.txt'
        carrier.write_text('0 1 18446744073709551615\n0 1 0\n0 1 1\n')
        for optimization in ('-O0','-O2'):
            binary = self.link(emitted, optimization)
            observed = self.command([binary, 'vectors', carrier])
            self.assertEqual((observed['returncode'], observed['stdout']), (0, b'ran 3 original body calls\n'))


class DomainPolicyTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load('coax_forall_native_check_execution', COAX / 'tools/native_check_execution.py')
        cls.policy = cls.execution.get_forall_policy(compiler=TRUSTED,
                                                   directory=cls.record_root / 'domain-policy')
        cls.vectors = json.loads((FIXTURES / 'domain-vectors.json').read_text())

    def domain(self, kind, bits, signed, **options):
        result = self.policy.domain(kind, bits, signed, **options)
        self.observations.append({'domain_request': [kind, bits, signed, options], 'result': result})
        return result

    def tuple(self, domains, ordinal):
        result = self.policy.tuple(domains, ordinal)
        self.observations.append({'ordinal': ordinal, 'result': result})
        return result

    def test_scalar_domains_have_independent_endpoint_and_zero_crossing_words(self):
        for vector in self.vectors['scalars']:
            with self.subTest(kind=vector['kind'], bits=vector['bits']):
                domain = self.domain(vector['kind'], vector['bits'], vector['is_signed'])
                self.assertEqual(domain['reason'], 0)
                self.assertEqual(domain['count'], vector['count'])
                self.assertEqual(domain['first'], vector['first'])
                for ordinal, value in vector['samples']:
                    self.assertEqual(self.tuple([domain], ordinal), {'reason': 0, 'values': [value]})

    def test_sparse_declared_enum_words_preserve_order_and_duplicate_entries(self):
        for vector in self.vectors['enums']:
            domain = self.domain(4, vector['bits'], vector['is_signed'], enum_values=vector['values'])
            self.assertEqual(domain['reason'], 0)
            self.assertEqual(domain['count'], len(vector['values']))
            actual = [self.tuple([domain], ordinal)['values'][0] for ordinal in range(domain['count'])]
            self.assertEqual(actual, vector['values'])

    def test_int64_range_edges_and_exact_case_cap_use_widened_counts(self):
        for vector in self.vectors['ranges']:
            domain = self.domain(5, 64, True, lo=vector['lo'], hi=vector['hi'])
            self.assertEqual(domain['reason'], 0)
            self.assertEqual(domain['count'], vector['count'])
            for ordinal, value in vector['samples']:
                self.assertEqual(self.tuple([domain], ordinal), {'reason': 0, 'values': [value]})
        for lo, hi, reason in ((2, 2, 2), (2, -1, 2), (0, 16777217, 4),
                              (-9223372036854775808, 9223372036854775807, 4)):
            self.assertEqual(self.domain(5, 64, True, lo=lo, hi=hi)['reason'], reason)
        self.assertEqual(self.domain(4, 64, False, enum_values=())['reason'], 3)
        self.assertEqual(self.domain(2, 32, True)['reason'], 1)

    def test_cartesian_order_bounds_and_product_refusal_are_not_host_enumeration(self):
        boolean = self.domain(1, 8, False)
        negative = self.domain(5, 64, True, lo=-2, hi=1)
        actual = [self.tuple([boolean, negative], ordinal)['values'] for ordinal in range(6)]
        self.assertEqual(actual, self.vectors['last_binding_fastest'])
        self.assertEqual(self.tuple([boolean, negative], 6)['reason'], 7)
        self.assertEqual(self.tuple([], 0), {'reason': 0, 'values': []})
        self.assertEqual(self.tuple([], 1)['reason'], 7)
        word = self.domain(3, 16, False)
        self.assertEqual(self.tuple([word, word], 0)['reason'], 5)


class EvaluatorTests(RecordedTests):
    def test_original_evaluator_claims_match_pre_fd_control_and_known_first_failing_tuple(self):
        reference = json.loads((FIXTURES / 'evaluator-reference.json').read_text())
        source = FIXTURES / 'typed-domains.coax'
        self.assertEqual(digest(source), reference['source_sha256'])
        evidence = self.dir / 'original-evidence.json'
        result = self.compiler('test', source, '--library', '--evidence', evidence)
        self.assertEqual(result['returncode'], 0, result['stderr'].decode(errors='replace'))
        actual = json.loads(evidence.read_text())
        self.observations.append({'raw_evidence':actual, 'reference':reference})
        self.assertEqual(actual['verdict'], reference['verdict'])
        claims = []
        for claim in actual['claims']:
            claim = dict(claim)
            claim['at'] = dict(claim['at'])
            physical = Path(claim['at']['file'])
            self.assertEqual((REPO / physical).resolve(), source.resolve())
            claim['at']['file'] = physical.name
            claims.append(claim)
        self.assertEqual(claims, reference['claims'])
        failure_evidence = self.dir / 'failing-evidence.json'
        failure = self.compiler('test', FIXTURES / 'failing.coax', '--library', '--evidence', failure_evidence)
        self.assertNotEqual(failure['returncode'], 0)
        failed = json.loads(failure_evidence.read_text())
        self.observations.append({'failed_original_evidence':failed,
                                  'scope':'Original failed baseline only; no native qualification'})
        self.assertNotEqual(failed['verdict'], 'passed')
        self.assertEqual(len(failed['claims']), 1)
        self.assertEqual(failed['claims'][0]['cases'], 6)
        self.assertEqual(failed['claims'][0]['outcome'], 'counterexample')
        self.assertIn(b'offset = 0, enabled = true', failure['stdout'] + failure['stderr'])


class ExecutionTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load('coax_forall_native_check_execution', COAX / 'tools/native_check_execution.py')

    def run_record(self, source, **options):
        result = self.execution.run([FIXTURES / (source + '.coax')], abi=2, compiler=TRUSTED,
                                    keep=self.dir / ('artifacts-' + str(len(self.observations))), **options)
        self.observations.append(result)
        self.assertEqual(result['schema'], 'coax.native-checks.execution.v2')
        return result

    def assert_profile_cases(self, result, expected):
        self.assertEqual(result['outcome'], 'passed')
        item = result['files'][0]
        self.assertEqual(item['baseline']['verdict'], 'passed')
        self.assertNotIn('--only', item['baseline_execution']['argv'])
        self.assertEqual(set(item['profiles']), {'-O0', '-O2'})
        for profile in item['profiles'].values():
            self.assertEqual(profile['outcome'], 'passed')
            checks = profile['execution']['checks']
            self.assertEqual([check['case_count'] for check in checks], expected)
            self.assertEqual([check['completed_cases'] for check in checks], expected)
            self.assertEqual([check['outcome'] for check in checks], ['passed'] * len(expected))
            pids = [check['process']['pid'] for check in checks]
            self.assertTrue(all(isinstance(pid, int) and pid > 0 for pid in pids))
            self.assertEqual(len(set(pids)), len(expected), 'each property needs a fresh native child')
        return item

    def test_opt_in_abi2_qualifies_original_quantified_scope(self):
        try:
            result = self.run_record('order')
        except Exception as error:
            self.observations.append({'api_error': type(error).__name__, 'message': str(error)})
            raise
        item = self.assert_profile_cases(result, [6])
        self.assertEqual([c['cases'] for c in item['baseline']['claims'] if c['kind'] == '#forall'], [6])

    def test_full_narrow_integer_domains_complete_under_original_baseline_and_native_profiles(self):
        item = self.assert_profile_cases(self.run_record('typed-domains'), [256,256,65536,65536,12,2,2,4])
        self.assertEqual([c['cases'] for c in item['baseline']['claims'] if c['kind'] == '#forall'],
                         [256,256,65536,65536,12,2,2,4])

    def test_duplicate_imported_and_nul_properties_use_one_fresh_child_each(self):
        self.assert_profile_cases(self.run_record('identity'), [1,3,4,2])

    def test_selected_native_property_keeps_full_original_baseline(self):
        item = self.assert_profile_cases(self.run_record('identity', only='tail λ'), [4])
        self.assertEqual(len([c for c in item['baseline']['claims'] if c['kind'] in ('#test','#forall')]), 4)
        for profile in item['profiles'].values():
            self.assertEqual(profile['execution']['checks'][0]['identity']['declaration_index'], 2)

    def test_failing_original_quantified_authority_and_unselected_baselines_stop_native_effects(self):
        self.execution.get_forall_policy(compiler=TRUSTED)
        actual_run = self.execution.process.run
        for source, options in (('failing', {}), ('authority-failure', {'only':'selected authority passing'}),
                                ('unselected-failure', {'only':'selected passing property'})):
            calls = []
            def observed(argv, *args, **kwargs):
                calls.append(list(map(str, argv)))
                return actual_run(argv, *args, **kwargs)
            with self.subTest(source=source), mock.patch.object(self.execution.process, 'run', side_effect=observed):
                record = self.run_record(source, **options)
            self.observations.append({'source':source, 'actual_commands':calls})
            self.assertNotEqual(record['outcome'], 'passed')
            self.assertNotEqual(record['files'][0]['baseline']['verdict'], 'passed')
            self.assertFalse(record['files'][0].get('profiles'))
            self.assertFalse(any(len(argv) > 1 and argv[1] == 'checks-c' for argv in calls))
            self.assertFalse(any(any(Path(value).name in ('checks.c','native-forall-observer.c')
                                     for value in argv) for argv in calls))
            for argv in calls:
                if len(argv) > 1 and argv[1] == 'test':
                    self.assertNotIn('--only', argv)

    def test_low_original_step_exhaustion_is_not_native_qualification(self):
        record = self.run_record('low-steps', steps=1)
        self.assertNotEqual(record['outcome'], 'passed')
        self.assertNotEqual(record['files'][0]['baseline']['verdict'], 'passed')
        self.assertFalse(record['files'][0].get('profiles'))

    def test_above_default_step_budget_never_executes_user_source(self):
        self.execution.get_forall_policy(compiler=TRUSTED)
        actual_run = self.execution.process.run
        calls = []
        def observed(argv, *args, **kwargs):
            calls.append(list(map(str, argv)))
            return actual_run(argv, *args, **kwargs)
        refusal = None
        record = None
        with mock.patch.object(self.execution.process, 'run', side_effect=observed):
            try:
                record = self.run_record('order', steps=20000001)
            except ValueError as error:
                refusal = str(error)
        self.observations.append({'refusal':refusal, 'actual_commands':calls})
        self.assertTrue(refusal or record['outcome'] != 'passed')
        self.assertFalse(any(len(argv) > 1 and argv[1] in ('test','checks-c')
                             and str(FIXTURES / 'order.coax') in argv for argv in calls))


class ArtifactObservationTests(RecordedTests):
    """Transport-only observations: these deliberately failing baselines never qualify."""
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load('coax_forall_native_check_execution', COAX / 'tools/native_check_execution.py')
        cls.policy = cls.execution.get_forall_policy(compiler=TRUSTED,
                                                   directory=cls.record_root / 'artifact-policy')

    def artifact(self, source, optimization):
        emitted = self.emissions.get(source)
        if emitted is None:
            emitted = self.dir / (source + '.c')
            emission = self.compiler('checks-c', FIXTURES / (source + '.coax'), '--verification-abi', '2', '-o', emitted)
            self.assertEqual(emission['returncode'], 0, emission['stderr'].decode(errors='replace'))
            self.emissions[source] = emitted
        observer = COAX / 'tools/native-forall-observer.c'
        binary = self.dir / (source + optimization)
        compiled = self.command([*shlex.split(os.environ.get('CC', 'cc')), '-std=c11', optimization,
            '-ffp-contract=off', '-fno-fast-math', emitted, self.policy.c, observer,
            '-I', Path(self.policy.header).parent, '-lm', '-o', binary])
        self.assertEqual(compiled['returncode'], 0, compiled['stderr'].decode(errors='replace'))
        self.observations.append({'scope':'artifact-only; no qualifying baseline', 'c':str(emitted),
            'c_sha256':digest(emitted), 'binary':str(binary), 'binary_sha256':digest(binary),
            'observer_sha256':digest(observer), 'policy_c_sha256':digest(self.policy.c),
            'policy_header_sha256':digest(self.policy.header)})
        return binary

    def test_malformed_progress_size_selector_and_stale_slots_never_invoke_body(self):
        binary = self.artifact('failure-first', '-O0')
        cases = [b'', bytes(32768), bytes(32770), bytes(32769),
                 b'\x02' + bytes(32768), b'\xff\x01' + bytes(32767),
                 b'\xff' + bytes(16384) + b'\x01' + bytes(16383)]
        for index, original in enumerate(cases):
            with self.subTest(vector=index):
                progress = self.dir / ('malformed-' + str(index) + '.bin')
                progress.write_bytes(original)
                observation = self.command([binary, 'one', '0', progress])
                self.assertNotEqual(observation['returncode'], 0)
                self.assertNotEqual(observation['returncode'], -signal.SIGABRT,
                                    'the deliberately failing original body executed')
                self.assertEqual(progress.read_bytes(), original, 'refusal changed unadmitted shared bytes')

    def test_actual_failing_tuple_is_original_artifact_transport_not_baseline_qualification(self):
        baseline = self.compiler('test', FIXTURES / 'failing.coax', '--library')
        self.assertNotEqual(baseline['returncode'], 0)
        self.assertIn(b'offset = 0, enabled = true', baseline['stdout'] + baseline['stderr'])
        for optimization in ('-O0','-O2'):
            binary = self.artifact('failing', optimization)
            metadata = self.command([binary, 'metadata'])
            self.assertEqual(metadata['returncode'], 0, metadata['stderr'].decode(errors='replace'))
            checks = self.policy.decode_catalog(metadata['stdout'])
            self.assertEqual(len(checks), 1)
            self.assertEqual(checks[0]['case_count'], 8)
            expected = [{'identity': {'ordinal':0, 'declaration_index':0,
                'name':b'original failing tuple', 'source':str((FIXTURES / 'failing.coax').resolve()).encode(),
                'line':3, 'column':1}, 'kind':2, 'cases':8}]
            self.assertEqual(self.policy.metadata(expected, metadata['stdout'], metadata), 0)
            record = self.execution.observe_forall_artifact(binary, checks, policy=self.policy)
            self.observations.append(record)
            self.assertEqual(record['outcome'], 'counterexample')
            self.assertIn('Artifact observations only', record['scope'])
            self.assertEqual(len(record['checks']), 1)
            failed = record['checks'][0]
            self.assertEqual(failed['outcome'], 'counterexample')
            self.assertEqual(failed['case_count'], 8)
            self.assertEqual(failed['completed_cases'], 5)
            self.assertEqual(failed['failing_case'], 5)
            self.assertEqual(failed['values'], [0,1])
            self.assertEqual(failed['binding_names'], ['offset','enabled'])
            self.assertIn('offset = 0', failed['witness'])
            self.assertIn('enabled = true', failed['witness'])
            self.assertEqual(failed['source'], str((FIXTURES / 'failing.coax').resolve()))
            self.assertEqual(failed['process']['returncode'], -signal.SIGABRT)
            self.assertFalse(failed.get('recognized_trap', False))


class CaptureBudgetTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.execution = load('coax_forall_native_check_execution', COAX / 'tools/native_check_execution.py')
        cls.policy = cls.execution.get_forall_policy(compiler=TRUSTED,
                                                   directory=cls.record_root / 'capture-policy')

    def test_insufficient_shared_capture_budget_is_inconclusive_before_native_launch(self):
        # Declared transport facts only. This catalog was not emitted by a compiler,
        # and this sentinel file is not claimed to be a native executable.
        catalog = [{'ordinal':0, 'declaration_index':0, 'name':b'declared capture property',
            'source':b'declared-capture-only.coax', 'line':1, 'column':1,
            'kind':2, 'cases':6, 'case_count':6, 'bindings':[
                {'name':b'enabled', 'type_name':b'bool', 'members':[],
                 'domain':{'kind':1,'bits':8,'is_signed':False,'first':0,'count':2,'values':[]}},
                {'name':b'offset', 'type_name':b'int', 'members':[],
                 'domain':{'kind':5,'bits':64,'is_signed':True,'first':-2,'count':3,'values':[]}}]}]
        binary = self.dir / 'declared-nonexecutable-sentinel'
        binary.write_bytes(b'No native execution is allowed for this declared transport vector.\n')
        initial = self.policy.initial_region()
        self.assertEqual(len(initial), 32769)
        session = mock.Mock()
        session.remaining_bytes = 32769
        session.run = mock.Mock(side_effect=AssertionError('capture refusal must precede any native launch'))
        quantified = sys.modules['native_forall_execution']
        with mock.patch.object(quantified.common, 'Session', return_value=session):
            record = self.execution.observe_forall_artifact(binary, catalog, policy=self.policy)
        self.observations.append({'scope':'Declared transport fixture; no native execution observation',
                                  'declared_catalog':catalog, 'result':record})
        session.run.assert_not_called()
        self.assertEqual(record['outcome'], 'inconclusive')
        self.assertEqual(len(record['checks']), 1)
        check = record['checks'][0]
        self.assertIsNone(check['process'])
        self.assertEqual(check['outcome'], 'inconclusive')
        self.assertEqual(check['completed_cases'], 0)
        self.assertEqual(check['progress_capture_admission'],
                         {'outcome':'inconclusive','remaining_bytes':32769,'requested_bytes':32770})
        self.assertNotIn('timeout', check)
        self.assertNotIn('stdout_limit', check)
        self.assertNotIn('stderr_limit', check)
        self.assertEqual(session.remaining_bytes, 32769)


if __name__ == '__main__':
    unittest.main()

"""Typed transport for the Coax semantic experiment policy; no verdict fallback."""
import ctypes as C
import signal

from native_execution_policy import (Arena, BaseResult, Emission, Sources, String,
    Identity, Process, Bytes, I, U, B, OUTCOMES, PolicyUnavailable, integer,
    boolean, object_value, process_fact)
from native_forall_policy import (ForallPolicy, Observation, Domain, Check, Domains, Words, L,
    catalog_facts, domain_fact, text_bytes)


class Trap(C.Structure):
    _fields_ = [('ok', B), ('reason', U), ('ordinal', I),
                ('declaration_index', I), ('line', I), ('path', String)]


class Expected(C.Structure):
    _fields_ = [('identity', Identity), ('reason', U), ('line', I), ('path', String)]


class Result(C.Structure):
    _fields_ = [('outcome', U), ('recognized', B), ('baseline_qualified', B)]


class Signature(C.Structure):
    _fields_ = [('optimization', U), ('identity', Identity), ('reason', U),
                ('line', I), ('path', String), ('seed_id', String)]


class Ordinal(C.Structure):
    _fields_ = [('reason', U), ('ordinal', L)]


class Reduction(C.Structure):
    _fields_ = [('reason', U), ('attempts', I), ('budget', I),
                ('binding', I), ('choice', I), ('done', B)]


class Candidate(C.Structure):
    _fields_ = [('state', Reduction), ('offered', B), ('ordinal', L)]


def result_copy(value):
    return {'outcome_code': int(value.outcome), 'outcome': OUTCOMES[value.outcome],
            'recognized_trap': bool(value.recognized),
            'baseline_qualified': bool(value.baseline_qualified)}


def reduction_copy(value):
    return {name: bool(getattr(value, name)) if name == 'done' else int(getattr(value, name))
            for name, _ in Reduction._fields_}


def expected_fact(arena, check, expected):
    return Expected(arena.identity(check), u32(expected['kind']),
                    integer(expected['line']), arena.string(str(expected['source'])
                        if not isinstance(expected['source'], bytes) else expected['source']))


def signature_fact(arena, value):
    return Signature(u32(value['optimization']), arena.identity(value['identity']),
        u32(value['kind']), integer(value['line']), arena.string(value['source']),
        arena.string(value['seed_id']))


def u32(value):
    # Preserve malformed host representations instead of modulo-narrowing them
    # into a valid Coax reason/profile. UINT32_MAX is outside those vocabularies.
    return value if type(value) is int and 0 <= value < 4294967295 else 4294967295


class SemanticsPolicy(ForallPolicy):
    source_name = 'native-semantics.coax'
    input_names = (source_name, *ForallPolicy.input_names)

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        try:
            for name, args, result in (
                ('SM_reason', [String], U), ('SM_trap_limit', [], I),
                ('SM_decode_trap', [Bytes], Trap),
                ('SM_trap', [Expected, Bytes, Observation, Process, I], Result),
                ('SM_baseline', [BaseResult], Result),
                ('SM_diagnostic', [Expected, String, String, I, I], B),
                ('SM_qualified', [U, B, B], B),
                ('SM_seed_id', [String], B), ('SM_probe', [U], U),
                ('SM_baseline_inputs', [BaseResult, Emission, Sources, Sources, Sources,
                                       B, B, Process, String], U),
                ('SM_ordinal', [Domains, Words], Ordinal),
                ('SM_reduce_start', [Domains, Words, I], Reduction),
                ('SM_reduce_next', [Reduction, Domains, Words, Words], Candidate),
                ('SM_reduce_status', [Reduction, I], U),
                ('SM_accept', [Signature, Signature, U, B], B),
                ('SM_replay', [Check, Domains, L, Bytes, Process], Observation),
                ('SM_seed', [Bytes, Bytes], I),
            ):
                self._bind(name, args, result)
        except (AttributeError, OSError) as error:
            raise PolicyUnavailable(str(error), self.observations) from error

    def qualified(self, outcome, baseline_passed, artifact_only):
        return bool(self.dll.SM_qualified(OUTCOMES.index(outcome),
            baseline_passed is True, artifact_only is not False))

    def seed_id(self, value):
        arena = Arena()
        return bool(self.dll.SM_seed_id(arena.string(value)))

    def probe(self, outcome):
        return int(self.dll.SM_probe(OUTCOMES.index(outcome)))

    def failed_baseline(self, validation):
        fact = BaseResult(validation['valid'], OUTCOMES.index(validation['outcome']), validation['reason'])
        return result_copy(self.dll.SM_baseline(fact))

    def failed_inputs(self, validation, baseline_record, manifest, observation, version):
        arena = Arena()
        baseline, manifest = object_value(baseline_record), object_value(manifest)
        item = Emission(integer(manifest.get('schema_version')), *[arena.string(manifest.get(n))
            for n in ('command', 'backend', 'compiler', 'runtime_arithmetic',
                      'interpreter_arithmetic', 'build_profile')],
            boolean(manifest.get('success')), integer(manifest.get('exit_status')),
            arena.string(manifest.get('emitted_sha256')))
        fact = BaseResult(validation['valid'], OUTCOMES.index(validation['outcome']), validation['reason'])
        evidence, profile, descriptors = baseline.get('sources'), manifest.get('sources'), manifest.get('descriptors')
        return int(self.dll.SM_baseline_inputs(fact, item, self._sources(arena, evidence),
            self._sources(arena, profile), self._sources(arena, descriptors),
            type(evidence) is list and type(profile) is list, type(descriptors) is list,
            process_fact(observation), arena.string(version)))

    def diagnostic(self, check, expected, diagnostics):
        arena = Arena()
        value = diagnostics[0] if len(diagnostics) == 1 else {}
        return bool(self.dll.SM_diagnostic(expected_fact(arena, check, expected),
            arena.string(value.get('code')), arena.string(value.get('file')),
            integer(value.get('line')), len(diagnostics)))

    def decode_trap(self, raw):
        arena = Arena()
        value = self.dll.SM_decode_trap(arena.bytes(raw))
        return {'ok': bool(value.ok), 'kind': int(value.reason), 'ordinal': int(value.ordinal),
            'declaration_index': int(value.declaration_index), 'line': int(value.line),
            'source': text_bytes(value.path)}

    def classify(self, check, expected, region, process, replay_index=None):
        arena = Arena()
        checks, bindings, _ = catalog_facts(arena, [check])
        if replay_index is None:
            native = self.dll.QE_observe(checks.data[0], bindings, arena.bytes(region), process_fact(process))
        else:
            domains = arena.array(Domain,
                [domain_fact(arena, binding['domain']) for binding in check['bindings']])
            native = self.dll.SM_replay(checks.data[0], domains, replay_index,
                                       arena.bytes(region), process_fact(process))
        matched = self.dll.SM_trap(expected_fact(arena, check, expected),
            arena.bytes(process['stdout']), native, process_fact(process), int(signal.SIGABRT))
        words = [int.from_bytes(region[native.values_offset + i*8:native.values_offset + i*8 + 8], 'little')
                 for i in range(native.values_count)] if native.valid else []
        if replay_index is not None and native.valid:
            words = self.tuple([binding['domain'] for binding in check['bindings']], replay_index)['values']
        return {**result_copy(matched), 'native_outcome': OUTCOMES[native.outcome],
            'native_outcome_code': int(native.outcome), 'valid': bool(native.valid),
            'values': words, 'case_index': int(native.state.case_index),
            'completed_cases': int(native.state.completed), 'case_count': int(native.state.total),
            'trap': self.decode_trap(process['stdout'])}

    def seed(self, raw):
        arena = Arena()
        output = arena.bytes(bytes(len(raw)))
        written = int(self.dll.SM_seed(arena.bytes(raw), output))
        if written < 0:
            raise ValueError('Coax refused the verification-only seed artifact')
        return C.string_at(output.data, written)

    def ordinal(self, check, values):
        arena = Arena()
        domains = arena.array(Domain,
            [domain_fact(arena, binding['domain']) for binding in check['bindings']])
        result = self.dll.SM_ordinal(domains, arena.array(L, values))
        return {'reason': int(result.reason), 'ordinal': int(result.ordinal)}

    def reduce_start(self, check, values, budget=32):
        arena = Arena()
        domains = arena.array(Domain,
            [domain_fact(arena, binding['domain']) for binding in check['bindings']])
        return reduction_copy(self.dll.SM_reduce_start(domains, arena.array(L, values), integer(budget)))

    def reduce_next(self, state, check, values):
        arena = Arena()
        domains = arena.array(Domain,
            [domain_fact(arena, binding['domain']) for binding in check['bindings']])
        output = arena.array(L, [0] * len(values))
        result = self.dll.SM_reduce_next(Reduction(**state), domains, arena.array(L, values), output)
        return {'state': reduction_copy(result.state), 'offered': bool(result.offered),
            'ordinal': int(result.ordinal),
            'values': [int(output.data[i]) for i in range(output.count)]}

    def reduce_status(self, state, count):
        return int(self.dll.SM_reduce_status(Reduction(**state), integer(count)))

    def accept(self, expected, actual, observation):
        arena = Arena()
        return bool(self.dll.SM_accept(signature_fact(arena, expected), signature_fact(arena, actual),
            observation['native_outcome_code'], observation['recognized_trap']))

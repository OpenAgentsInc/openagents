"""Typed transport for the Coax finite-domain and quantified-execution policy."""
import ctypes as C

from native_execution_policy import (
    Policy, Arena, Identity, Process, Claim, Plan, Bytes, String,
    I, U, B, OUTCOMES, copy_identity, process_fact, view_type, integer, PolicyUnavailable,
)

L = C.c_uint64
Words = view_type(L)


class Domain(C.Structure):
    _fields_ = [('kind', U), ('bits', U), ('is_signed', B), ('first', I),
                ('count', L), ('values', Words)]


class DomainResult(C.Structure):
    _fields_ = [('reason', U), ('domain', Domain)]


class Product(C.Structure):
    _fields_ = [('reason', U), ('total', L)]


class Expected(C.Structure):
    _fields_ = [('identity', Identity), ('kind', U), ('cases', L)]


class Check(C.Structure):
    _fields_ = [('identity', Identity), ('kind', U), ('cases', L),
                ('binding_start', I), ('binding_count', I)]


class Binding(C.Structure):
    _fields_ = [('name', String), ('type_name', String), ('domain', Domain),
                ('member_start', I), ('member_count', I)]


class Member(C.Structure):
    _fields_ = [('bits', L), ('label', String)]


class Sizes(C.Structure):
    _fields_ = [('outcome', U), ('checks', I), ('bindings', I), ('members', I)]


class State(C.Structure):
    _fields_ = [('reason', U), ('event', U), ('row', I), ('declaration_index', I),
                ('case_index', L), ('total', L), ('arity', I), ('completed', L)]


class Observation(C.Structure):
    _fields_ = [('outcome', U), ('valid', B), ('state', State), ('slot', I),
                ('values_offset', I), ('values_count', I)]


Domains, Expecteds, Checks, Bindings, Members = map(view_type, (Domain, Expected, Check, Binding, Member))


def text_bytes(value):
    return C.string_at(value.data, value.count)


def copy_domain(value):
    return {'kind': int(value.kind), 'bits': int(value.bits), 'is_signed': bool(value.is_signed),
            'first': int(value.first), 'count': int(value.count),
            'values': [int(value.values.data[i]) for i in range(value.values.count)]}


def domain_fact(arena, value):
    return Domain(value['kind'], value['bits'], value['is_signed'], value['first'],
                  value['count'], arena.array(L, value.get('values', [])))


def catalog_facts(arena, catalog):
    checks, bindings, members = [], [], []
    for item in catalog:
        start = len(bindings)
        for binding in item['bindings']:
            member_start = len(members)
            for member in binding.get('members', []):
                members.append(Member(member['bits'], arena.string(member['label'])))
            bindings.append(Binding(arena.string(binding['name']), arena.string(binding['type_name']),
                domain_fact(arena, binding['domain']), member_start, len(members) - member_start))
        checks.append(Check(arena.identity(item), item['kind'], item['cases'], start, len(bindings) - start))
    return arena.array(Check, checks), arena.array(Binding, bindings), arena.array(Member, members)


class ForallPolicy(Policy):
    source_name = 'quantified-execution.coax'
    file_stem = 'quantified-check-execution-policy'
    input_names = ('quantified-execution.coax', 'check-domains.coax', 'native-execution.coax',
                   'native-checks.coax', 'mutation-schedule.coax', 'budget.coax')

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        try:
            self._bind('FD_domain', [U, U, B, I, I, I], DomainResult)
            self._bind('FD_combine', [L, L], Product)
            self._bind('FD_tuple', [Domains, L, Words], U)
            self._bind('QE_plan', [view_type(Claim), String, B, Bytes], Plan)
            self._bind('QE_expected', [view_type(Claim), Bytes, I], Expected)
            self._bind('QE_sizes', [Bytes], Sizes)
            self._bind('QE_decode', [Bytes, Checks, Bindings, Members, Words], U)
            self._bind('QE_metadata', [Expecteds, Checks, Bindings, Members, Process], U)
            self._bind('QE_observe', [Check, Bindings, Bytes, Process], Observation)
            self._bind('QE_witness', [Check, Bindings, Members, Words, Bytes], I)
            self._bind('QE_abi', [I], B)
            self._bind('QE_region_size', [], I)
            self._bind('QE_initialize', [Bytes], B)
            self._bind('QE_capture', [I, I], U)
        except (AttributeError, OSError) as error:
            raise PolicyUnavailable(str(error), self.observations) from error

    def abi(self, value):
        return bool(self.dll.QE_abi(integer(value)))

    def capture(self, remaining, requested):
        return int(self.dll.QE_capture(integer(remaining), integer(requested)))

    def initial_region(self):
        arena = Arena()
        output = arena.bytes(bytes(self.dll.QE_region_size()))
        if not self.dll.QE_initialize(output):
            raise ValueError('Coax refused the quantified progress storage')
        return C.string_at(output.data, output.count)

    def domain(self, kind, bits, is_signed, lo=0, hi=0, enum_values=()):
        result = self.dll.FD_domain(kind, bits, is_signed, lo, hi, len(enum_values))
        value = copy_domain(result.domain)
        value['values'] = list(enum_values)
        return {'reason': int(result.reason), **value}

    def combine(self, total, count):
        result = self.dll.FD_combine(total, count)
        return {'reason': int(result.reason), 'total': int(result.total)}

    def tuple(self, domains, ordinal):
        arena = Arena()
        facts = arena.array(Domain, [domain_fact(arena, value) for value in domains])
        values = arena.array(L, [0] * len(domains))
        reason = int(self.dll.FD_tuple(facts, ordinal, values))
        return {'reason': reason, 'values': [int(values.data[i]) for i in range(values.count)] if reason == 0 else []}

    def select(self, record, only):
        arena = Arena()
        claims = self._claims(arena, record.get('claims'))
        mask = arena.array(C.c_uint8, [0] * claims.count)
        plan = self.dll.QE_plan(claims, arena.string(only), only is not None, mask)
        expected = []
        if plan.outcome == 0:
            for ordinal in range(plan.count):
                value = self.dll.QE_expected(claims, mask, ordinal)
                expected.append({'identity': copy_identity(value.identity), 'kind': int(value.kind), 'cases': int(value.cases)})
        return {'outcome': OUTCOMES[plan.outcome], 'count': int(plan.count),
                'mask': [int(mask.data[i]) for i in range(mask.count)], 'expected': expected,
                'identities': [value['identity'] for value in expected]}

    def _decode(self, arena, raw):
        wire = arena.bytes(raw)
        sizes = self.dll.QE_sizes(wire)
        if sizes.outcome != 0:
            return int(sizes.outcome), None
        checks = arena.array(Check, [Check() for _ in range(sizes.checks)])
        bindings = arena.array(Binding, [Binding() for _ in range(sizes.bindings)])
        members = arena.array(Member, [Member() for _ in range(sizes.members)])
        words = arena.array(L, [0] * sizes.members)
        code = int(self.dll.QE_decode(wire, checks, bindings, members, words))
        return code, (checks, bindings, members) if code == 0 else None

    def metadata(self, expected, raw, observation):
        phase = self.phase(observation)
        if phase:
            return phase
        arena = Arena()
        code, decoded = self._decode(arena, raw)
        if code:
            return code
        facts = arena.array(Expected, [Expected(arena.identity(value['identity']), value['kind'], value['cases']) for value in expected])
        return int(self.dll.QE_metadata(facts, *decoded, process_fact(observation)))

    def decode_catalog(self, raw):
        arena = Arena()
        code, decoded = self._decode(arena, raw)
        if code:
            raise ValueError('Coax refused the quantified check inventory')
        checks, bindings, members = decoded
        result = []
        for ordinal in range(checks.count):
            check = checks.data[ordinal]
            entry = {**copy_identity(check.identity), 'kind': int(check.kind),
                     'cases': int(check.cases), 'case_count': int(check.cases), 'bindings': []}
            for i in range(check.binding_start, check.binding_start + check.binding_count):
                binding = bindings.data[i]
                entry['bindings'].append({'name': text_bytes(binding.name), 'type_name': text_bytes(binding.type_name),
                    'domain': copy_domain(binding.domain),
                    'members': [{'bits': int(members.data[j].bits), 'label': text_bytes(members.data[j].label)}
                                for j in range(binding.member_start, binding.member_start + binding.member_count)]})
            result.append(entry)
        return result

    def observe(self, check, region, observation):
        arena = Arena()
        checks, bindings, members = catalog_facts(arena, [check])
        result = self.dll.QE_observe(checks.data[0], bindings, arena.bytes(region), process_fact(observation))
        state = result.state
        words = []
        if result.valid:
            words = [int.from_bytes(region[result.values_offset + 8*i:result.values_offset + 8*i + 8], 'little')
                     for i in range(result.values_count)]
        return {'outcome_code': int(result.outcome), 'outcome': OUTCOMES[result.outcome],
                'valid': bool(result.valid), 'event': int(state.event), 'case_count': int(state.total),
                'completed_cases': int(state.completed), 'case_index': int(state.case_index),
                'failing_case': int(state.case_index) if result.valid and result.outcome == 1 else None,
                'values': words, 'slot': int(result.slot)}

    def witness(self, check, values):
        arena = Arena()
        checks, bindings, members = catalog_facts(arena, [check])
        words = arena.array(L, values)
        needed = int(self.dll.QE_witness(checks.data[0], bindings, members, words, arena.bytes(b'')))
        if needed < 0:
            raise ValueError('Coax refused the quantified witness facts')
        output = arena.bytes(bytes(needed))
        written = int(self.dll.QE_witness(checks.data[0], bindings, members, words, output))
        if written != needed:
            raise ValueError('Coax witness output did not match its admitted size')
        return C.string_at(output.data, needed).decode('utf-8', 'strict')

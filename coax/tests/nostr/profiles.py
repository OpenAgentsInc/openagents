#!/usr/bin/env python3
"""Focused native/independent-oracle and source/drift checks for Nostr profiles.

This is explicit maintenance tooling, never a compiler/app build dependency.
"""
import ctypes as C
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'coax/lib/nostr/profiles.coax'
MATRIX = ROOT / 'coax/lib/nostr/profiles.json'
GENERATOR = ROOT / 'coax/tests/nostr/profile_matrix.coax'
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT / 'coax/bin/coax'))
# Independent fixture, taken from existing proto options, JSON parser and socket.
LIMITS = [
    [0, 524288, 262144, 0, 2048, 64, 64, 1048576, 0, 0, 0, 1048576],
    [0, 1048576, 0, 65536, 0, 0, 16, 1048576, 0, 0, 0, 0],
    [262144, 262144, 65536, 65536, 2000, 16, 16, 262144, 64, 512, 1048576, 0],
]
NAMES = ['core-read', 'core-write', 'phone-signer', 'openagents-host', 'block-identity', 'block-channel', 'agent-private', 'key-backup']
# Native has proven wire codecs/JSON shape; Wasm has a socket probe only.
AVAILABLE = [[3, 0, 32], [3, 0, 32], [0, 0, 0], [1, 0, 0], [1, 0, 0], [1, 0, 0], [1, 0, 0], [0, 0, 0]]
REQUIRED = [198, 124, 796, 1028, 2052, 4100, 8212, 16384]

def run(*args, **kw):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, **kw)
    if result.returncode: raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr or result.stdout}')
    return result

class View(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint64)), ('count', C.c_int64)]

class String(C.Structure):
    _fields_ = [('data', C.c_char_p), ('count', C.c_int64)]

def view(values):
    data = (C.c_uint64 * len(values))(*values)
    return data, View(data, len(values))

def bounds_oracle(consumer, values):
    return 0 <= consumer < len(LIMITS) and len(values) == 12 and all(0 <= value <= cap for value, cap in zip(values, LIMITS[consumer]))

def config_oracle(consumer, values):
    if not bounds_oracle(consumer, values): return False
    if any((cap > 0) != (value > 0) for value, cap in zip(values, LIMITS[consumer])): return False
    if values[2] > values[1] or values[3] > values[1]: return False
    if values[0] and values[1] > values[0]: return False
    return True

def validate_inventory(document, lib, canonical):
    """Validate tooling's proposed advertised support/budgets through Coax.

    Evidence itself is pinned by the canonical registry, never supplied as a
    trusted Boolean by this document. This does not admit a Nostr operation.
    """
    if document.get('schema') != 1: raise ValueError('unsupported inventory schema')
    for key in ['sources', 'evidence', 'targets', 'capabilities', 'measure_order', 'safe_integer']:
        if document.get(key) != canonical.get(key): raise ValueError('canonical source/evidence metadata changed')
    profiles = document.get('profiles', [])
    if [p.get('name') for p in profiles] != NAMES: raise ValueError('unknown/missing profile')
    for i, profile in enumerate(profiles):
        for key in ['nips', 'optional_nips', 'consumer', 'source_mask']:
            if profile.get(key) != canonical['profiles'][i].get(key): raise ValueError('profile provenance changed')
        required = lib.np_required(i)
        if profile.get('required') != required: raise ValueError('required capabilities changed')
        targets = profile.get('targets', [])
        if [t.get('target') for t in targets] != [0, 1, 2]: raise ValueError('unknown/missing target')
        for target in targets:
            t = target['target']; claimed = target.get('available')
            if target.get('state') != canonical['profiles'][i]['targets'][t]['state']: raise ValueError('target implementation state changed')
            if type(claimed) is not int or not 0 <= claimed < 2**32: raise ValueError('invalid capabilities')
            if claimed and not lib.np_can_advertise(i, t, claimed): raise ValueError('unsupported advertised capability')
            if type(target.get('supported')) is not bool: raise ValueError('invalid support flag')
            if target['supported'] and not lib.np_can_advertise(i, t, required): raise ValueError('unsupported full profile')
    consumers = document.get('consumers', [])
    if [c.get('name') for c in consumers] != ['proto-reference', 'json-reader', 'project-web']: raise ValueError('unknown/missing consumer')
    for i, consumer in enumerate(consumers):
        if consumer.get('state') != canonical['consumers'][i].get('state'): raise ValueError('consumer implementation state changed')
        values = consumer.get('limits', [])
        if any(type(v) is not int or not 0 <= v < 2**64 for v in values): raise ValueError('invalid budget')
        data, v = view(values)
        if not lib.np_configuration(i, v): raise ValueError('inconsistent/excessive consumer budget')
    return True

class Profiles(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='coax-nostr-profiles-')
        cls.dir = Path(cls.temp.name)
        header = cls.dir / 'profiles.h'
        emitted = run(COMPILER, 'c', SOURCE, '--library', '--header', header).stdout
        (cls.dir/'profiles.c').write_text(emitted)
        run('cc', '-std=c11', '-O2', '-shared', '-fPIC', cls.dir/'profiles.c', '-lm', '-o', cls.dir/'profiles.dylib')
        cls.lib = C.CDLL(str(cls.dir/'profiles.dylib'))
        for name in ['np_bounds', 'np_configuration']:
            fn = getattr(cls.lib, name); fn.argtypes = [C.c_int64, View]; fn.restype = C.c_bool
        for name in ['np_capabilities', 'np_required']:
            fn = getattr(cls.lib, name); fn.argtypes = [C.c_int64] * (2 if name == 'np_capabilities' else 1); fn.restype = C.c_uint32
        cls.lib.np_can_advertise.argtypes = [C.c_int64, C.c_int64, C.c_uint32]; cls.lib.np_can_advertise.restype = C.c_bool
        cls.lib.np_safe_integer.argtypes = [C.c_int64]; cls.lib.np_safe_integer.restype = C.c_bool
        # Exercise the actual shipped JSON reader together with the new policy.
        # Its heavy transcript checks exceed the compiler's emission budget in
        # this combined unit. Copy the exact runtime prefix, not a rewritten
        # parser; the full JSON suite belongs to its owning task.
        json_runtime = cls.dir/'json_runtime.coax'
        original = (ROOT/'coax/lib/nostr/json.coax').read_text()
        assert original.count('// ---- checks') == 1
        json_runtime.write_text(original.split('// ---- checks', 1)[0])
        bridge = cls.dir/'json_profile.coax'
        bridge.write_text(f'#load {json.dumps(str(SOURCE))};\n#load "json_runtime.coax";\n' + '''
np_json_projection :: (text: string) -> int #export_c {
  parsed := json_int(text);
  if !parsed.ok { return 0; }
  return ifx np_safe_integer(parsed.value) then 3 else 1;
}
''')
        (cls.dir/'bridge.c').write_text(run(COMPILER, 'c', bridge, '--library').stdout)
        run('cc', '-std=c11', '-O2', '-shared', '-fPIC', cls.dir/'bridge.c', '-lm', '-o', cls.dir/'bridge.dylib')
        cls.bridge = C.CDLL(str(cls.dir/'bridge.dylib'))
        cls.bridge.np_json_projection.argtypes = [String]; cls.bridge.np_json_projection.restype = C.c_int64
        run(COMPILER, 'c', GENERATOR, '-o', cls.dir/'matrix.c')
        run('cc', '-std=c11', '-O2', cls.dir/'matrix.c', '-lm', '-o', cls.dir/'matrix')
        cls.generated = run(cls.dir/'matrix').stdout
        cls.matrix = json.loads(cls.generated)
    @classmethod
    def tearDownClass(cls): cls.temp.cleanup()

    def test_checked_in_matrix_is_generated(self):
        self.assertEqual(MATRIX.read_text(), self.generated, 'regenerate with profiles.py --write')
        self.assertEqual([p['name'] for p in self.matrix['profiles']], NAMES)
        self.assertTrue(all(p['consumer'] and p['nips'] for p in self.matrix['profiles']))
        self.assertEqual([c['limits'] for c in self.matrix['consumers']], LIMITS)
        self.assertTrue(validate_inventory(self.matrix, self.lib, self.matrix))

    def test_native_bounds_against_independent_boundary_fixture(self):
        for consumer in [-1, 0, 1, 2, 3]:
            cap = LIMITS[max(0, min(consumer, 2))]
            candidates = [[0]*12, cap[:], cap[:-1], cap+[0]]
            for field, maximum in enumerate(cap):
                for value in [0, max(0, maximum-1), maximum, maximum+1, 2**64-1]:
                    values = cap[:]; values[field] = value; candidates.append(values)
            for values in candidates:
                data, v = view(values)
                self.assertEqual(self.lib.np_bounds(consumer, v), bounds_oracle(consumer, values), (consumer, values))
                self.assertEqual(self.lib.np_configuration(consumer, v), config_oracle(consumer, values), (consumer, values))
        # Coherent individual maxima can still describe an incoherent frame.
        for values in [[1, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0], [100, 1, 2, 1, 1, 1, 1, 1, 1, 1, 1, 0]]:
            data, v = view(values)
            self.assertTrue(self.lib.np_bounds(2, v))
            self.assertFalse(self.lib.np_configuration(2, v))

    def test_claims_match_evidence_not_requested_flags(self):
        for p in range(-1, 9):
            for t in range(-1, 4):
                available = AVAILABLE[p][t] if 0 <= p < 8 and 0 <= t < 3 else 0
                self.assertEqual(self.lib.np_capabilities(p, t), available)
                self.assertFalse(self.lib.np_can_advertise(p, t, self.lib.np_required(p)))
                for claim in [0, 1, 2, 3, 4, 32, 64, 127, 2**32-1]:
                    self.assertEqual(self.lib.np_can_advertise(p, t, claim), claim != 0 and claim & ~available == 0)
        for p, required in enumerate(REQUIRED): self.assertEqual(self.lib.np_required(p), required)

    def test_integer_bound_is_stricter_than_json_parser(self):
        for v in [-2**63, -2**53-1, -2**53, -2**53+1, -1, 0, 1, 2**53-1, 2**53, 2**53+1, 2**63-1]:
            self.assertEqual(self.lib.np_safe_integer(v), -(2**53-1) <= v <= 2**53-1)
        for text, expected in [('9007199254740991', 3), ('9007199254740992', 1), ('-9007199254740991', 3), ('-9007199254740992', 1), ('9007199254740993', 0), ('1.0', 0)]:
            encoded=text.encode(); self.assertEqual(self.bridge.np_json_projection(String(encoded, len(encoded))), expected)

    def test_native_sanitizers_and_wrapping_profile(self):
        host = self.dir/'host.c'
        host.write_text('''#include "profiles.h"
#include <assert.h>
int main(void) {
  for (int c=0;c<3;c++) {
    uint64_t values[12];
    for(int f=0;f<12;f++) values[f]=np_limit(c,f);
    assert(np_configuration(c,(coax_v_u64){values,12}));
    for(int f=0;f<12;f++) {
      uint64_t cap=values[f]; values[f]=cap+1;
      assert(!np_bounds(c,(coax_v_u64){values,12})); values[f]=cap;
    }
  }
  assert(!np_bounds(0,(coax_v_u64){0,-1}));
  assert(!np_safe_integer(INT64_MIN) && !np_safe_integer(INT64_MAX));
  assert(np_safe_integer(9007199254740991LL) && !np_safe_integer(9007199254740992LL));
  assert(np_can_advertise(0,0,3) && !np_can_advertise(0,0,4));
  assert(np_can_advertise(0,2,32) && !np_can_advertise(0,1,32));
  return 0;
}''')
        run('cc', '-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer', self.dir/'profiles.c', host, '-lm', '-o', self.dir/'sanitized')
        run(self.dir/'sanitized')
        release = self.dir/'release.c'
        release.write_text(run(COMPILER, 'c', SOURCE, '--library', '--release').stdout)
        run('cc', '-std=c11', '-O2', release, host, '-lm', '-o', self.dir/'release')
        run(self.dir/'release')

    def test_source_pins_and_real_evidence(self):
        lock = json.loads((ROOT/'nips/upstream-lock.json').read_text())
        for source in self.matrix['sources']:
            if source['name'] in lock['sources']:
                self.assertEqual(source['pin'], lock['sources'][source['name']]['commit'])
                for name, digest in lock['sources'][source['name']]['files'].items():
                    self.assertEqual(hashlib.sha256((ROOT/'nips'/source['name']/name).read_bytes()).hexdigest(), digest)
            else:
                h = hashlib.sha256()
                for p in sorted((ROOT/'nips/openagents').rglob('*')):
                    if p.is_file(): h.update(str(p.relative_to(ROOT)).encode()+b'\0'+p.read_bytes()+b'\0')
                self.assertEqual(source['pin'], h.hexdigest())
        for evidence in self.matrix['evidence']:
            self.assertTrue((ROOT/evidence['source']).is_file())
            self.assertTrue((ROOT/evidence['checks']).is_file())
            run('git', 'cat-file', '-e', evidence['commit']+'^{commit}', cwd=ROOT)
            pinned = subprocess.check_output(['git', 'show', evidence['commit']+':'+evidence['source']], cwd=ROOT)
            self.assertEqual(evidence['sha256'], hashlib.sha256(pinned).hexdigest())

    def test_limits_remain_bound_to_their_existing_consumers(self):
        event = (ROOT/'proto/nostr/event/v1alpha1/event.proto').read_text()
        def option(message, field, name):
            body = re.search(r'\bmessage '+message+r'\s*\{([^}]*)\}', event, re.S)[1]
            line = next(line for line in body.splitlines() if re.search(r'\b'+field+r'\s*=', line))
            return int(re.search(r'\.'+name+r'\)\s*=\s*(\d+)', line)[1])
        self.assertEqual(LIMITS[0][1], option('SignedEvent', 'original_json', 'max_bytes'))
        self.assertEqual(LIMITS[0][2], option('Event', 'content', 'max_bytes'))
        self.assertEqual(LIMITS[0][4], option('Event', 'tags', 'max_count'))
        self.assertEqual(LIMITS[0][5], option('Tag', 'values', 'max_count'))
        parser = (ROOT/'coax/lib/nostr/json.coax').read_text()
        for field, name in [(1, 'JSON_MAX_INPUT'), (3, 'JSON_MAX_STRING'), (6, 'JSON_DEPTH')]:
            self.assertEqual(LIMITS[1][field], int(re.search(name+r' :: (\d+);', parser)[1]))
        socket = (ROOT/'apps/openagents-web/project-live.coax').read_text()
        self.assertEqual(LIMITS[2][0], int(re.search(r'LIVE_INBOX :: (\d+);', socket)[1]))
        wire = (ROOT/'coax/lib/proto/wire.coax').read_text()
        self.assertEqual(LIMITS[0][6], int(re.search(r'PB_MAX_DEPTH :: (\d+);', wire)[1]))
        self.assertEqual(LIMITS[0][11], int(re.search(r'PB_MAX_BYTES :: (\d+);', wire)[1]))

    def test_seeded_claim_and_limit_faults_are_detected(self):
        # Reject actual machine-readable inventories carrying seeded faults.
        def modified(change):
            document = copy.deepcopy(self.matrix); change(document)
            with self.assertRaises(ValueError): validate_inventory(document, self.lib, self.matrix)
        modified(lambda d: d['profiles'][0]['targets'][0].update(supported=True))
        modified(lambda d: d['profiles'][0]['targets'][0].update(available=4))
        modified(lambda d: d['profiles'][0]['targets'][0].update(state='supported'))
        modified(lambda d: d['profiles'][0].update(required=1))
        modified(lambda d: d['consumers'][2]['limits'].__setitem__(0, 262145))
        modified(lambda d: d['consumers'][2]['limits'].__setitem__(0, 1))
        modified(lambda d: d['consumers'][2]['limits'].__setitem__(11, 1))
        modified(lambda d: d['sources'][0].update(pin='unreviewed'))
        modified(lambda d: d['evidence'][0].update(commit='unreviewed'))
        modified(lambda d: d['profiles'][0].update(nips='all'))
        modified(lambda d: d['consumers'][2].update(state='production'))
        self.assertFalse(bounds_oracle(2, LIMITS[0]))
        values=LIMITS[2][:]; values[0]=LIMITS[2][0]-1
        self.assertFalse(config_oracle(2, values))
        self.assertFalse(self.lib.np_can_advertise(0, 0, 4))
        self.assertFalse(self.lib.np_safe_integer(2**53))

if __name__ == '__main__':
    if sys.argv[1:] == ['--write']:
        with tempfile.TemporaryDirectory(prefix='coax-profile-matrix-') as d:
            p=Path(d); (p/'matrix.c').write_text(run(COMPILER, 'c', GENERATOR).stdout)
            run('cc', '-std=c11', '-O2', p/'matrix.c', '-lm', '-o', p/'matrix')
            generated=run(p/'matrix').stdout
            json.loads(generated)
            MATRIX.write_text(generated)
        print('updated', MATRIX.relative_to(ROOT))
    elif len(sys.argv) == 3 and sys.argv[1] == '--validate':
        Profiles.setUpClass()
        try:
            validate_inventory(json.loads(Path(sys.argv[2]).read_text()), Profiles.lib, Profiles.matrix)
            print('inventory claims and consumer limits accepted')
        finally: Profiles.tearDownClass()
    else: unittest.main()

#!/usr/bin/env python3
"""Public event-verifier comparison; this is not NIP-98 request parity.

Uses fixed, signed throwaway fixtures. Never reads a user key, changes the API,
or treats either implementation's answer as an authorization decision.
Requires the compiler and native libsecp256k1/OpenSSL development packages.
"""
import base64
import copy
import ctypes as C
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

from secp_field import ROOT, COMPILER, View, array, view
from event import Event
from make_event_fixtures import canonical
from oracle import schnorr_verify

DATA = ROOT / 'coax/tests/nostr/data/api-public-events.json'

class ApiEvent(C.Structure):
    _fields_ = [('id', C.c_char * 65), ('pubkey', C.c_char * 65),
                ('created_at', C.c_longlong), ('kind', C.c_int),
                ('u', C.c_char * 1024), ('method', C.c_char * 16),
                ('payload', C.c_char * 65)]

def run(*args):
    return subprocess.run([str(x) for x in args], check=True,
                          capture_output=True, text=True).stdout

def encoded(event):
    return json.dumps(event, ensure_ascii=False, separators=(',', ':'))

class IndependentFixtureCheck(unittest.TestCase):
    def test_fixture_signatures_with_independent_bigint_oracle(self):
        for name, event in json.loads(DATA.read_text()).items():
            with self.subTest(case=name):
                digest = hashlib.sha256(canonical(event)).digest()
                self.assertEqual(digest.hex(), event['id'])
                self.assertTrue(schnorr_verify(digest, bytes.fromhex(event['pubkey']), bytes.fromhex(event['sig'])))

class PublicApiEventParity(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='coax-api-event-')
        d = Path(cls.temp.name)
        flags = shlex.split(run('pkg-config', '--cflags', '--libs', 'libsecp256k1', 'openssl'))
        run('cc', '-std=c11', '-O2', '-shared', '-fPIC', ROOT / 'coax/web/api/nostr.c', '-o', d / 'api.so', *flags)
        cls.api = C.CDLL(str(d / 'api.so'))
        cls.api.nostr_verify_event.argtypes = [C.c_char_p, C.c_size_t, C.POINTER(ApiEvent), C.c_char_p, C.c_size_t]
        cls.api.nostr_verify_event.restype = C.c_bool
        cls.api.nip98_check.argtypes = [C.c_char_p, C.c_char_p, C.c_char_p, C.c_void_p,
                                        C.c_size_t, C.c_longlong, C.POINTER(ApiEvent), C.c_char_p, C.c_size_t]
        cls.api.nip98_check.restype = C.c_bool
        cls.coax = []
        for mode, extra in [('checked', []), ('release', ['--release'])]:
            source = d / (mode + '.c')
            source.write_text(run(COMPILER, 'c', ROOT / 'coax/lib/nostr/event.coax', '--library', '--header', d / (mode + '.h'), *extra))
            run('cc', '-std=c11', '-O2', '-shared', '-fPIC', source, '-lm', '-o', d / (mode + '.so'))
            lib = C.CDLL(str(d / (mode + '.so')))
            lib.nostr_event_verify_vartime.argtypes = [View, View, C.c_bool]
            lib.nostr_event_verify_vartime.restype = Event
            cls.coax.append((mode, lib))

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_signed_and_invalid_public_fixtures(self):
        fixtures = json.loads(DATA.read_text())
        basic = encoded(fixtures['basic'])
        wrong_sig = copy.deepcopy(fixtures['basic']); wrong_sig['sig'] = '00' * 64
        wrong_content = copy.deepcopy(fixtures['basic']); wrong_content['content'] = 'modified'
        # Expected outcomes are observations, including documented disagreements;
        # they do not decide which implementation is safe or protocol-conformant.
        cases = [
            ('basic', basic, True, 0),
            ('utf8', encoded(fixtures['utf8']), True, 0),
            ('bad_signature', encoded(wrong_sig), False, 8),
            ('bad_content', encoded(wrong_content), False, 7),
            ('missing_signature', encoded({k: v for k, v in fixtures['basic'].items() if k != 'sig'}), False, 2),
            ('trailing_data', basic + ' false', False, 1),
            ('unknown_field', basic[:-1] + ',"extra":true}', False, 0),
            ('duplicate_kind', basic.replace('"kind":27235', '"kind":27235,"kind":27235'), True, 1),
            ('embedded_nul', encoded(fixtures['nul']), False, 0),
            ('seventeen_tags', encoded(fixtures['tags17']), False, 0),
            ('negative_time', encoded(fixtures['negative_time']), True, 2),
        ]
        for name, text, api_want, coax_error in cases:
            with self.subTest(case=name):
                raw = text.encode('utf-8'); api_event = ApiEvent(); error = C.create_string_buffer(256)
                api_ok = self.api.nostr_verify_event(raw, len(raw), C.byref(api_event), error, len(error))
                self.assertEqual(api_ok, api_want, (name, error.value))
                for mode, lib in self.coax:
                    input_bytes = array(raw); output = array(bytes(262144))
                    event = lib.nostr_event_verify_vartime(view(input_bytes), view(output), False)
                    self.assertEqual(event.error, coax_error, (name, mode))
                    self.assertEqual(event.verified, coax_error == 0, (name, mode))
                    if api_ok and coax_error == 0:
                        self.assertEqual(bytes(event.id).hex(), api_event.id.decode(), name)
                        self.assertEqual(bytes(event.public_key).hex(), api_event.pubkey.decode(), name)
                        self.assertEqual(event.kind, api_event.kind, name)
                        self.assertEqual(event.created_at, api_event.created_at, name)

    def test_current_nip98_request_guards(self):
        basic = encoded(json.loads(DATA.read_text())['basic'])
        duplicate = basic.replace('"kind":27235', '"kind":27235,"kind":27235')
        cases = [
            ('valid', basic, b'POST', b'https://example.test/api', 1761750000, True),
            ('duplicate_field', duplicate, b'POST', b'https://example.test/api', 1761750000, True),
            ('wrong_method', basic, b'GET', b'https://example.test/api', 1761750000, False),
            ('wrong_url', basic, b'POST', b'https://example.test/other', 1761750000, False),
            ('too_old', basic, b'POST', b'https://example.test/api', 1761750061, False),
        ]
        for name, text, method, url, now, expected in cases:
            with self.subTest(case=name):
                auth = b'Nostr ' + base64.b64encode(text.encode('utf-8'))
                result = ApiEvent(); error = C.create_string_buffer(256); body = C.create_string_buffer(b'')
                ok = self.api.nip98_check(auth, method, url, C.cast(body, C.c_void_p), 0, now,
                                          C.byref(result), error, len(error))
                self.assertEqual(ok, expected, (name, error.value))

if __name__ == '__main__':
    unittest.main()

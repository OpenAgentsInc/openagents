#!/usr/bin/env python3
"""Public fixture comparison only; not a deployable NIP-98 auth adapter.

The same immutable event bytes go to legacy NIP-98 and Coax public event
verification. Only after Coax accepts them does this test extract tags from
those same bytes and call the field observer. No production key, live endpoint,
replay rule, session, authorization, or production cutover is involved.
"""
import base64
import copy
import ctypes as C
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import api_event_parity as prior
from api_event_parity import ApiEvent, DATA, encoded, run
from secp_field import ROOT, COMPILER, View, array, view
from make_event_fixtures import canonical
from oracle import schnorr_verify

class PublicRequestObservation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Reuse the real independent legacy C and Coax public event verifier
        # build recipes, not their earlier test results.
        prior.PublicApiEventParity.setUpClass.__func__(cls)
        d = Path(cls.temp.name)
        cls.observers = []
        for mode, flags in [('checked', []), ('release', ['--release'])]:
            source = d / ('observe-' + mode + '.c')
            source.write_text(run(COMPILER, 'c', ROOT / 'coax/lib/nostr/nip98-observe.coax', '--library', '--header', d / (mode + '-observe.h'), *flags))
            run('cc', '-std=c11', '-O2', '-shared', '-fPIC', source, '-lm', '-o', d / ('observe-' + mode + '.so'))
            lib = C.CDLL(str(d / ('observe-' + mode + '.so')))
            lib.n98_observe.argtypes = [C.c_int64] * 3 + [View] * 6 + [C.c_int64] * 3 + [C.c_bool]
            lib.n98_observe.restype = C.c_bool
            cls.observers.append((mode, lib))

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    @staticmethod
    def tags_from_same_original(raw):
        def unique(pairs):
            output = {}
            for key, value in pairs:
                if key in output:
                    raise ValueError('duplicate object member')
                output[key] = value
            return output
        event = json.loads(raw, object_pairs_hook=unique)
        counts = {'u': 0, 'method': 0, 'payload': 0}
        fields = {name: '' for name in counts}
        malformed = False
        for tag in event['tags']:
            if not tag or tag[0] not in counts:
                continue
            key = tag[0]
            counts[key] += 1
            if len(tag) != 2 or not isinstance(tag[1], str):
                malformed = True
            else:
                fields[key] = tag[1]
        return event, counts, fields, malformed

    def legacy(self, raw, method, url, body, now):
        authorization = b'Nostr ' + base64.b64encode(raw)
        out = ApiEvent(); error = C.create_string_buffer(256); b = C.create_string_buffer(body)
        return bool(self.api.nip98_check(authorization, method.encode(), url.encode(), C.cast(b, C.c_void_p), len(body), now, C.byref(out), error, len(error)))

    def coax_observation(self, raw, method, url, body, now):
        result = []
        for (mode, verifier), (observer_mode, observer) in zip(self.coax, self.observers):
            self.assertEqual(mode, observer_mode)
            input_bytes = array(raw); out = array(bytes(262144))
            event = verifier.nostr_event_verify_vartime(view(input_bytes), view(out), False)
            self.assertEqual(bytes(input_bytes), raw, 'verifier must not mutate original')
            if event.error != 0 or not event.verified:
                result.append(False)
                continue
            original = C.string_at(event.original.data, event.original.count)
            self.assertEqual(original, raw, 'tag extraction must use the exact verified bytes')
            parsed, counts, fields, malformed = self.tags_from_same_original(original)
            self.assertEqual(event.kind, parsed['kind'])
            self.assertEqual(event.created_at, parsed['created_at'])
            self.assertEqual(bytes(event.id).hex(), parsed['id'])
            self.assertEqual(bytes(event.public_key).hex(), parsed['pubkey'])
            # Python is test tooling only. Its SHA-256 hashes the same body sent
            # to legacy nip98_check. The Coax observer does not perform hashing.
            values = [fields['u'], fields['method'], fields['payload'], url, method, hashlib.sha256(body).hexdigest()]
            buffers = [array(value.encode('utf-8')) for value in values]
            result.append(bool(observer.n98_observe(event.kind, event.created_at, now, *[view(b) for b in buffers], counts['u'], counts['method'], counts['payload'], malformed)))
        self.assertEqual(result[0], result[1], 'checked/release must agree')
        return result[0]

    def test_source_bound_public_request_fixtures(self):
        fixtures = json.loads(DATA.read_text())
        extra = json.loads((ROOT / 'coax/tests/nostr/data/api-request-observation-public.json').read_text())
        for event in extra.values():
            digest = hashlib.sha256(canonical(event)).digest()
            self.assertEqual(digest.hex(), event['id'])
            self.assertTrue(schnorr_verify(digest, bytes.fromhex(event['pubkey']), bytes.fromhex(event['sig'])))
        basic = encoded(fixtures['basic'])
        invalid_signature = copy.deepcopy(fixtures['basic']); invalid_signature['sig'] = '00' * 64
        modified = copy.deepcopy(fixtures['basic']); modified['content'] = 'modified'
        duplicate = basic.replace('"kind":27235', '"kind":1,"kind":27235')
        unknown = basic[:-1] + ',"extra":true}'
        self.assertNotEqual(duplicate, basic)
        cases = [
            # name, original JSON, method, URL, body, now, legacy, observation
            ('valid', basic, 'POST', 'https://example.test/api', b'', 1761750000, True, True),
            ('no_payload_nonempty_body', basic, 'POST', 'https://example.test/api', b'not-bound', 1761750000, True, True),
            ('invalid_signature', encoded(invalid_signature), 'POST', 'https://example.test/api', b'', 1761750000, False, False),
            ('modified_content', encoded(modified), 'POST', 'https://example.test/api', b'', 1761750000, False, False),
            ('wrong_method', basic, 'GET', 'https://example.test/api', b'', 1761750000, False, False),
            ('ascii_method_case', basic, 'post', 'https://example.test/api', b'', 1761750000, True, True),
            ('wrong_url', basic, 'POST', 'https://example.test/other', b'', 1761750000, False, False),
            ('old_boundary', basic, 'POST', 'https://example.test/api', b'', 1761750060, True, True),
            ('too_old', basic, 'POST', 'https://example.test/api', b'', 1761750061, False, False),
            ('future_boundary', basic, 'POST', 'https://example.test/api', b'', 1761749940, True, True),
            ('too_future', basic, 'POST', 'https://example.test/api', b'', 1761749939, False, False),
            ('matching_body', encoded(fixtures['payload']), 'POST', 'https://example.test/api', b'hello', 1761750000, True, True),
            ('mismatching_body', encoded(fixtures['payload']), 'POST', 'https://example.test/api', b'changed', 1761750000, False, False),
            ('empty_body', encoded(fixtures['payload']), 'POST', 'https://example.test/api', b'', 1761750000, False, False),
            # Explicitly retain known policy divergences; do not normalize them.
            ('duplicate_json_kind', duplicate, 'POST', 'https://example.test/api', b'', 1761750000, True, False),
            ('duplicate_url_last_match', encoded(fixtures['url_last_correct']), 'POST', 'https://example.test/api', b'', 1761750000, True, False),
            ('duplicate_url_last_wrong', encoded(fixtures['url_last_wrong']), 'POST', 'https://example.test/api', b'', 1761750000, False, False),
            ('duplicate_method_last_match', encoded(extra['duplicate_method']), 'POST', 'https://example.test/api', b'', 1761750000, True, False),
            ('present_empty_payload', encoded(extra['empty_payload']), 'POST', 'https://example.test/api', b'', 1761750000, True, False),
            ('extra_url_tag_field', encoded(extra['extra_url_field']), 'POST', 'https://example.test/api', b'', 1761750000, True, False),
            ('unknown_field', unknown, 'POST', 'https://example.test/api', b'', 1761750000, False, True),
        ]
        for name, text, method, url, body, now, legacy_want, observed_want in cases:
            with self.subTest(case=name):
                raw = text.encode('utf-8')
                self.assertEqual(self.legacy(raw, method, url, body, now), legacy_want)
                self.assertEqual(self.coax_observation(raw, method, url, body, now), observed_want)

class SanitizedObserver(unittest.TestCase):
    def test_checked_and_release_native_sanitized_observer(self):
        with tempfile.TemporaryDirectory(prefix='coax-nip98-sanitized-') as temp:
            d = Path(temp)
            for mode, flags in [('checked', []), ('release', ['--release'])]:
                source = d / (mode + '.c'); header = d / 'observe.h'; target = d / mode
                source.write_text(run(COMPILER, 'c', ROOT / 'coax/lib/nostr/nip98-observe.coax', '--library', '--header', header, *flags))
                run('cc', '-std=c11', '-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer', '-I'+str(d), source,
                    ROOT / 'coax/tests/nostr/api_request_observation_sanitized.c', '-lm', '-o', target)
                run(target)

class WasmObserver(unittest.TestCase):
    def test_checked_and_release_wasm_observer(self):
        digest = hashlib.sha256(b'hello').hexdigest()
        default = {'kind': 27235, 'created': 1000, 'now': 1000,
                   'fields': ['u', 'POST', '', 'u', 'post', digest],
                   'uc': 1, 'mc': 1, 'pc': 0, 'malformed': False}
        rows = []
        def add(name, expected, **changes):
            row = dict(default); row.update(changes); row.update(name=name, expected=expected); rows.append(row)
        add('valid', True)
        add('kind', False, kind=1)
        add('old_boundary', True, created=940)
        add('old_outside', False, created=939)
        add('future_boundary', True, created=1060)
        add('future_outside', False, created=1061)
        add('duplicate_url', False, uc=2)
        add('missing_method', False, mc=0)
        add('malformed_tag', False, malformed=True)
        add('matching_payload', True, fields=['u', 'POST', digest, 'u', 'post', digest], pc=1)
        add('mismatching_payload', False, fields=['u', 'POST', digest, 'u', 'post', 'a'*64], pc=1)
        add('present_empty_payload', False, pc=1)
        add('url_mismatch', False, fields=['other', 'POST', '', 'u', 'post', digest])
        add('method_mismatch', False, fields=['u', 'GET', '', 'u', 'post', digest])
        with tempfile.TemporaryDirectory(prefix='coax-nip98-wasm-') as temp:
            d = Path(temp); fixtures = d / 'cases.json'; fixtures.write_text(json.dumps(rows))
            for mode, flags in [('checked', []), ('release', ['--release'])]:
                source = d / (mode + '.c'); header = d / 'observe.h'; target = d / (mode + '.wasm')
                source.write_text(run(COMPILER, 'c', ROOT / 'coax/lib/nostr/nip98-observe.coax', '--library', '--header', header, *flags))
                run('/opt/homebrew/opt/llvm/bin/clang', '--target=wasm32-wasip1',
                    '--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
                    '-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',
                    '-std=c11', '-O2', '-I'+str(d), source,
                    ROOT / 'coax/tests/nostr/api_request_observation_wasm.c',
                    '-mexec-model=reactor', '-Wl,--import-memory,--export-memory,--max-memory=8388608,--export=__heap_base,-z,stack-size=1048576',
                    '-lm', '-o', target)
                report = json.loads(run('node', ROOT / 'coax/tests/nostr/api_request_observation_wasm.cjs', target, fixtures))
                self.assertEqual(report['cases'], len(rows)); self.assertEqual(report['failures'], 0)

if __name__ == '__main__':
    unittest.main()

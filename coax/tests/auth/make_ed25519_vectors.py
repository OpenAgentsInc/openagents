#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published vectors outside Coax)
"""Writes coax/tests/auth/ed25519_vectors.coax, the Ed25519 vectors that
ed25519_vectors_test.coax runs, from pinned upstream files:

- RFC 8032 section 7.1: TEST 1, 2, 3, 1024 and SHA(abc) (secret key, public
  key, message, signature), in coax/tests/vectors/upstream/rfc/rfc8032.txt;
- Wycheproof ed25519_test.json, all 210 cases, in .../upstream/wycheproof/;
- the twelve "Taming the many EdDSAs" cases of ed25519-speccheck
  (cases.json), each with the verdict of the LibSodium row of its
  README.md's results table, in .../upstream/ed25519-speccheck/.

  python3 -I coax/tests/auth/make_ed25519_vectors.py

Each file's SHA-256 must equal its pin below. Nothing is hashed, signed or
verified here: every key, message, signature and result is copied.
Wycheproof's 29 "acceptable" cases take the verdict in ACCEPTABLE, which is
libsodium 1.0.20's (ed25519_libsodium.py checks this table against it)."""
import hashlib, json, re
from pathlib import Path

UP = Path(__file__).resolve().parents[1] / 'vectors/upstream'
PINS = {
    'rfc/rfc8032.txt': 'ed63657ff389301282b169b0abde9b5dd2c7e4d524fdfa5da6ff3094fc93c4c3',
    'wycheproof/ed25519_test.json': 'ea80ffebac085e21cb9d3e903e7fa2b448d96702da5b31f061d0b83b430b74b3',
    'ed25519-speccheck/cases.json': '08e47a36d9aead288664930505584f353fff113ab854f2800db1e4f5b3540450',
    'ed25519-speccheck/README.md': '8aefa01d4217c77b73b63dabb0bfb98d075dd5593a88025ee59213caf737e65c',
}
# Wycheproof's acceptable cases: tcIds 152-159 are small-order keys, which
# libsodium (and ed25519.coax) refuse; 160-180 are mixed-order keys whose
# signatures satisfy the cofactorless equation too, so both verify them.
ACCEPTABLE = {tc: False for tc in range(152, 160)}
ACCEPTABLE.update({tc: True for tc in range(160, 181)})


def read(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data


def rfc():
    """The five Ed25519 tests of section 7.1, as dicts of hex strings."""
    text = read('rfc/rfc8032.txt').decode('ascii')
    body = text[text.index('\n7.1.  Test Vectors for Ed25519\n'):text.index('\n7.2.  Test Vectors for Ed25519ctx\n')]
    # Drop page footers and headers, which split long hex blocks.
    lines = [l for l in body.split('\n') if not l.startswith('Josefsson & Liusvaara') and not l.startswith('RFC 8032 ') and '\f' not in l]
    tests, current, field = [], None, None
    for line in lines:
        s = line.strip()
        if s.startswith('-----TEST '):
            current = {'name': s[len('-----TEST '):], 'secret': '', 'public': '', 'message': '', 'signature': ''}
            tests.append(current); field = None
        elif current is None: continue
        elif s == 'ALGORITHM:': field = 'alg'
        elif s == 'SECRET KEY:': field = 'secret'
        elif s == 'PUBLIC KEY:': field = 'public'
        elif s.startswith('MESSAGE (length'): field = 'message'; current['length'] = int(re.search(r'length (\d+) byte', s).group(1))
        elif s == 'SIGNATURE:': field = 'signature'
        elif field == 'alg':
            if s: assert s == 'Ed25519', s
        elif field and re.fullmatch(r'[0-9a-f]+', s): current[field] += s
    assert [t['name'] for t in tests] == ['1', '2', '3', '1024', 'SHA(abc)'], [t['name'] for t in tests]
    for t in tests:
        assert len(t['secret']) == 64 and len(t['public']) == 64 and len(t['signature']) == 128, t['name']
        assert len(t['message']) == 2 * t['length'], t['name']
    return tests


def wycheproof():
    doc = json.loads(read('wycheproof/ed25519_test.json'))
    assert doc['algorithm'] == 'EDDSA' and doc['schema'] == 'eddsa_verify_schema_v1.json' and doc['numberOfTests'] == 210
    keys, cases = [], []
    for group in doc['testGroups']:
        assert group['type'] == 'EddsaVerify' and group['publicKey']['curve'] == 'edwards25519'
        keys.append(group['publicKey']['pk'])
        for t in group['tests']:
            assert '\\' not in t['comment']
            if t['result'] == 'acceptable': valid = ACCEPTABLE[t['tcId']]
            else: assert t['result'] in ('valid', 'invalid') and t['tcId'] not in ACCEPTABLE; valid = t['result'] == 'valid'
            cases.append({'id': t['tcId'], 'key': len(keys) - 1, 'msg': t['msg'], 'sig': t['sig'], 'valid': valid,
                          'result': t['result'], 'comment': t['comment'].replace('"', '\\"')})
    assert len(cases) == 210 and sum(c['result'] == 'acceptable' for c in cases) == len(ACCEPTABLE)
    return keys, cases


def speccheck():
    cases = json.loads(read('ed25519-speccheck/cases.json'))
    assert len(cases) == 12
    row = [l for l in read('ed25519-speccheck/README.md').decode('utf-8').split('\n') if l.startswith('|LibSodium ')]
    assert len(row) == 1, row
    verdicts = [v.strip() for v in row[0].strip('|').split('|')[1:]]
    assert len(verdicts) == 12 and set(verdicts) <= {'V', 'X'}, verdicts
    return [{'msg': c['message'], 'key': c['pub_key'], 'sig': c['signature'], 'valid': v == 'V'} for c, v in zip(cases, verdicts)]


def render():
    tests = rfc()
    keys, cases = wycheproof()
    spec = speccheck()
    b = lambda v: 'true' if v else 'false'
    out = ['// Generated by make_ed25519_vectors.py from pinned RFC 8032, Wycheproof and',
           '// ed25519-speccheck files (coax/tests/vectors/SOURCES.md). Published values,',
           '// copied, not computed. Test data only. valid is the published result;',
           "// Wycheproof's acceptable cases (result 2) take libsodium's verdict.",
           'EdRfc :: struct {name:string;secret:string;key:string;msg:string;sig:string;}',
           'EdCase :: struct {id:int;key:int;msg:string;sig:string;valid:bool;result:int;comment:string;}',
           'EdTaming :: struct {msg:string;key:string;sig:string;valid:bool;}',
           '// RFC 8032 section 7.1.',
           'ED_RFC :: [%d] EdRfc.[\n%s];' % (len(tests), ',\n'.join(
               '  EdRfc.{name="%s",secret="%s",key="%s",msg="%s",sig="%s"}' % (t['name'], t['secret'], t['public'], t['message'], t['signature']) for t in tests)),
           '// Wycheproof ed25519_test.json (result: 0 invalid, 1 valid, 2 acceptable).',
           'ED_WP_KEYS :: [%d] string.[\n%s];' % (len(keys), ',\n'.join('  "%s"' % k for k in keys)),
           'ED_WP_CASES :: [%d] EdCase.[\n%s];' % (len(cases), ',\n'.join(
               '  EdCase.{id=%d,key=%d,msg="%s",sig="%s",valid=%s,result=%d,comment="%s"}' % (
                   c['id'], c['key'], c['msg'], c['sig'], b(c['valid']), {'invalid': 0, 'valid': 1, 'acceptable': 2}[c['result']], c['comment'])
               for c in cases)),
           '// ed25519-speccheck cases 0 to 11, with the LibSodium row of its README.',
           'ED_TAMING :: [%d] EdTaming.[\n%s];' % (len(spec), ',\n'.join(
               '  EdTaming.{msg="%s",key="%s",sig="%s",valid=%s}' % (c['msg'], c['key'], c['sig'], b(c['valid'])) for c in spec))]
    return '\n'.join(out) + '\n'


if __name__ == '__main__':
    Path(__file__).with_name('ed25519_vectors.coax').write_text(render())

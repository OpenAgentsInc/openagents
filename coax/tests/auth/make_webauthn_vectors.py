# Firm reason: 2
# Writes coax/tests/auth/webauthn_vectors.coax from pinned upstream files:
#   - the W3C WebAuthn Level 3 source (upstream/w3c/webauthn-index.bs), its
#     "Test Vectors" section: each registration and authentication pair;
#   - py_webauthn's registration and authentication tests
#     (upstream/py_webauthn/*.py): real browser responses;
#   - Wycheproof's rsa_signature_2048_sha256_test.json (RS256).
# Each file's SHA-256 is checked against manifest.json first. The expected
# result of each case is stated here, next to the rule that gives it; the
# Coax test then checks webauthn.coax and rsa_verify.coax against it.
#
#   python3 -I coax/tests/auth/make_webauthn_vectors.py
import base64, hashlib, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
VECTORS = os.path.join(HERE, '..', 'vectors')
OUT = os.path.join(HERE, 'webauthn_vectors.coax')

def pinned(path):
    manifest = json.load(open(os.path.join(VECTORS, 'manifest.json')))
    data = open(os.path.join(VECTORS, path), 'rb').read()
    want = [f['sha256'] for f in manifest['files'] if f['path'] == path]
    if want != [hashlib.sha256(data).hexdigest()]:
        sys.exit('pin mismatch: ' + path)
    return data

def b64url(text):
    return base64.urlsafe_b64decode(text + '=' * (-len(text) % 4))

def coax(text):
    return '"' + text.replace('\\', '\\\\').replace('"', '\\"') + '"'

# ---- CBOR, only far enough to find authData and the credential key
def head(b, i):
    ai = b[i] & 31
    w = {24: 1, 25: 2, 26: 4, 27: 8}.get(ai, 0)
    return b[i] >> 5, (int.from_bytes(b[i+1:i+1+w], 'big') if w else ai), i + 1 + w

def skip(b, i):
    left = 1
    while left:
        major, arg, i = head(b, i); left -= 1
        if major in (2, 3): i += arg
        elif major == 4: left += arg
        elif major == 5: left += 2 * arg
    return i

def auth_data_of(att):
    major, pairs, i = head(att, 0)
    for _ in range(pairs):
        k_end = skip(att, i); key = att[i:k_end]; v_end = skip(att, k_end)
        if key == b'\x68authData':
            _, n, start = head(att, k_end)
            return att[start:start + n]
        i = v_end
    raise ValueError('no authData')

def credential_key(auth):
    n = int.from_bytes(auth[53:55], 'big')
    at = 55 + n
    return auth[at:skip(auth, at)]

def cose_alg(key):
    _, pairs, i = head(key, 0)
    for _ in range(pairs):
        k_end = skip(key, i); v_end = skip(key, k_end)
        if key[i:k_end] == b'\x03':
            major, arg, _ = head(key, k_end)
            return -1 - arg if major == 1 else arg
        i = v_end

# Error codes of webauthn.coax.
OK, TYPE, CROSS, ATTESTATION, KEY, ALGORITHM = 'WA_OK', 'WA_TYPE', 'WA_CROSS_ORIGIN', 'WA_ATTESTATION', 'WA_KEY', 'WA_ALGORITHM'

def client_rule(client):
    # crossOrigin true or a topOrigin refuses before anything else.
    data = json.loads(client)
    return CROSS if data.get('crossOrigin') or 'topOrigin' in data else OK

# ---- the W3C test vectors
def spec_cases():
    text = pinned('upstream/w3c/webauthn-index.bs').decode()
    a = text.index('<!-- GENERATED CONTENT: Use test-vectors/webauthn-test-vectors.py -->')
    z = text.index('<!-- END GENERATED CONTENT', a)
    cases = []
    for section in re.split(r'\n## ', text[a:z])[1:]:
        name = re.search(r'\{#sctn-test-vectors-([^}]+)\}', section).group(1)
        if 'Authentication=]' not in section:
            continue
        reg, auth = section.split('[=authentication ceremony|Authentication=]')
        get = lambda block, field: bytes.fromhex(re.search(r"^" + field + r" = h'([0-9a-f]*)'", block, re.M).group(1))
        att = get(reg, 'attestationObject'); reg_client = get(reg, 'clientDataJSON')
        auth_data = auth_data_of(att); key = credential_key(auth_data)
        # Registration: the client data first, then fmt "none" only.
        reg_expect = client_rule(reg_client)
        if reg_expect == OK and not att.startswith(bytes.fromhex('a363666d74646e6f6e65')):
            reg_expect = ATTESTATION
        # Assertion: the client data, then the key: ES256 and RS256 verify,
        # EdDSA keys parse but have no verifier yet, other algorithms refuse.
        get_client = get(auth, 'clientDataJSON')
        alg = cose_alg(key)
        auth_expect = client_rule(get_client)
        if auth_expect == OK:
            auth_expect = OK if alg in (-7, -257) else ALGORITHM if alg == -8 else KEY
        cases.append(dict(name=name, reg_challenge=get(reg, 'challenge').hex(), reg_client=reg_client.hex(),
            attestation=att.hex(), reg_expect=reg_expect, key=key.hex(), be='true' if auth_data[32] & 8 else 'false',
            get_challenge=get(auth, 'challenge').hex(), auth_data=get(auth, 'authenticatorData').hex(),
            get_client=get_client.hex(), signature=get(auth, 'signature').hex(), auth_expect=auth_expect))
    return cases

# ---- py_webauthn's real responses
def test_body(text, name):
    a = text.index('    def ' + name + '(')
    z = text.find('\n    def ', a + 10)
    return text[a:z if z > 0 else len(text)]

def field(body, key):
    return re.search(r'"' + key + r'":\s*"([^"]*)"', body).group(1)

def call_arg(body, key):
    m = re.search(key + r'\s*=\s*base64url_to_bytes\(\s*"([^"]*)"', body)
    return m.group(1)

def pyw_cases():
    reg_text = pinned('upstream/py_webauthn/test_verify_registration_response.py').decode()
    auth_text = pinned('upstream/py_webauthn/test_verify_authentication_response.py').decode()
    regs = []
    # (test, expected): a none attestation registers; packed and a cut object refuse.
    for name, expect in [('test_verifies_none_attestation_response', OK),
                         ('test_raises_exception_on_invalid_attestation_type', ATTESTATION),
                         ('test_raises_on_bad_attestationObject', ATTESTATION)]:
        body = test_body(reg_text, name)
        regs.append(dict(name=name, attestation=b64url(field(body, 'attestationObject')).hex(),
            client=b64url(field(body, 'clientDataJSON')).hex(), challenge=b64url(call_arg(body, 'challenge')).hex(),
            origin='http://localhost:5000', rp='localhost', expect=expect))
    gets = []
    # (test, uv, expected): EC2 and RSA verify, OKP waits for Ed25519.
    for name, uv, expect in [('test_verify_authentication_response_with_EC2_public_key', 'false', OK),
                             ('test_verify_authentication_response_with_RSA_public_key', 'true', OK),
                             ('test_verify_authentication_response_with_OKP_public_key', 'false', ALGORITHM),
                             ('test_raises_exception_on_incorrect_public_key', 'false', 'WA_SIGNATURE')]:
        body = test_body(auth_text, name)
        count = int(re.search(r'sign_count = (\d+)', body).group(1))
        auth = b64url(field(body, 'authenticatorData'))
        gets.append(dict(name=name, key=b64url(call_arg(body, 'credential_public_key')).hex(), auth_data=auth.hex(),
            client=b64url(field(body, 'clientDataJSON')).hex(), signature=b64url(field(body, 'signature')).hex(),
            challenge=b64url(call_arg(body, 'challenge')).hex(), stored=count, uv=uv,
            be='true' if auth[32] & 8 else 'false', expect=expect))
    return regs, gets

# ---- Wycheproof RS256 (e = 65537 groups; rsa_verify_sha256 takes no other exponent)
def rsa_cases():
    data = json.loads(pinned('upstream/wycheproof/rsa_signature_2048_sha256_test.json'))
    keys, cases, skipped = [], [], 0
    for group in data['testGroups']:
        if group['publicKey']['publicExponent'] != '010001':
            skipped += len(group['tests']); continue
        modulus = group['publicKey']['modulus']
        while modulus.startswith('00'): modulus = modulus[2:]
        keys.append(modulus)
        for t in group['tests']:
            # "acceptable" cases are refused: the encoding check is exact.
            cases.append(dict(id=t['tcId'], key=len(keys) - 1, msg=t['msg'], sig=t['sig'],
                valid='true' if t['result'] == 'valid' else 'false', comment=t['comment']))
    return keys, cases, skipped

def main():
    spec = spec_cases(); regs, gets = pyw_cases(); keys, rsa, skipped = rsa_cases()
    o = ['// Generated by coax/tests/auth/make_webauthn_vectors.py from pinned files; do not edit.',
         'WaSpec :: struct { name: string; reg_challenge: string; reg_client: string; attestation: string; reg_expect: int; key: string; be: bool;',
         '                   get_challenge: string; auth_data: string; get_client: string; signature: string; auth_expect: int; }',
         'WaPywReg :: struct { name: string; attestation: string; client: string; challenge: string; origin: string; rp: string; expect: int; }',
         'WaPywGet :: struct { name: string; key: string; auth_data: string; client: string; signature: string; challenge: string; stored: u32; uv: bool; be: bool; expect: int; }',
         'WaRsa :: struct { id: int; key: int; msg: string; sig: string; valid: bool; comment: string; }',
         '// The W3C test vectors (RP ID example.org, origin https://example.org): %d pairs.' % len(spec),
         'WA_SPEC :: [%d] WaSpec.[' % len(spec)]
    o += ['  WaSpec.{name=%s,reg_challenge="%s",reg_client="%s",attestation="%s",reg_expect=%s,key="%s",be=%s,get_challenge="%s",auth_data="%s",get_client="%s",signature="%s",auth_expect=%s},'
          % (coax(c['name']), c['reg_challenge'], c['reg_client'], c['attestation'], c['reg_expect'], c['key'], c['be'],
             c['get_challenge'], c['auth_data'], c['get_client'], c['signature'], c['auth_expect']) for c in spec]
    o += ['];', '// py_webauthn: real registrations.', 'WA_PYW_REG :: [%d] WaPywReg.[' % len(regs)]
    o += ['  WaPywReg.{name=%s,attestation="%s",client="%s",challenge="%s",origin="%s",rp="%s",expect=%s},'
          % (coax(c['name']), c['attestation'], c['client'], c['challenge'], c['origin'], c['rp'], c['expect']) for c in regs]
    o += ['];', '// py_webauthn: real assertions.', 'WA_PYW_GET :: [%d] WaPywGet.[' % len(gets)]
    o += ['  WaPywGet.{name=%s,key="%s",auth_data="%s",client="%s",signature="%s",challenge="%s",stored=%d,uv=%s,be=%s,expect=%s},'
          % (coax(c['name']), c['key'], c['auth_data'], c['client'], c['signature'], c['challenge'], c['stored'], c['uv'], c['be'], c['expect']) for c in gets]
    o += ['];', '// Wycheproof rsa_signature_2048_sha256 (e = 65537): %d cases; %d with e = 3 left out.' % (len(rsa), skipped),
          'WA_RSA_KEYS :: [%d] string.[' % len(keys)] + ['  "%s",' % k for k in keys] + ['];', 'WA_RSA_CASES :: [%d] WaRsa.[' % len(rsa)]
    o += ['  WaRsa.{id=%d,key=%d,msg="%s",sig="%s",valid=%s,comment=%s},' % (c['id'], c['key'], c['msg'], c['sig'], c['valid'], coax(c['comment'])) for c in rsa]
    o += ['];', '']
    open(OUT, 'w').write('\n'.join(o))
    print('%d spec pairs, %d registrations, %d assertions, %d RSA cases (%d left out)' % (len(spec), len(regs), len(gets), len(rsa), skipped))

main()

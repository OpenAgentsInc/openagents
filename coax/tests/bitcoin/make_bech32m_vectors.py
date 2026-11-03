#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published vectors outside Coax)
"""Writes coax/tests/bitcoin/bech32m_vectors.coax for Bitcoin chapter 28 from
BIP-350, BIP-173 and Bitcoin Core's key_io files, all pinned in
coax/tests/vectors/upstream/:

  python3 -I coax/tests/bitcoin/make_bech32m_vectors.py

- bip-0350.mediawiki: the 7 valid and 14 invalid Bech32m strings, and the
  8 valid and 15 invalid segwit addresses with their scriptPubKeys or
  reasons;
- bip-0173.mediawiki: the 6 valid and 10 invalid segwit addresses;
- key_io_valid.json: the 32 Bech32 and Bech32m addresses, with their chain
  and scriptPubKey;
- key_io_invalid.json: every string, all of which must refuse.

Each file's SHA-256 must equal its pin below, so a changed file refuses
instead of silently changing the checks. Nothing here computes a checksum
or regroups bits: every string, script and reason is copied. A valid
address's witness version and program are read from its scriptPubKey (the
version opcode, a push length, the program), and each invalid address's
expected refusal is the SW_* code for the reason its BIP gives."""
import hashlib, json, re
from pathlib import Path

UPSTREAM = Path(__file__).resolve().parents[1] / 'vectors/upstream'
PINS = {
    'bip350/bip-0350.mediawiki': '63634b06aa8bae88b31929674736e74964c4598684269ac2b0b140c43a7a0dec',
    'bip173/bip-0173.mediawiki': 'b19eab06b264bbd15ad7f7d640fa34de8f4a8809fba8a6ecc3e165545b27a543',
    'bitcoin-core/key_io_valid.json': '90bd1d35d12763e0d00c5400b2c9fe551e532a821e1e466c20cad3aced70a7fe',
    'bitcoin-core/key_io_invalid.json': 'c3ca74ddad7c01faaca7c26063537feb73ff2be03eb7fc83a42a655d2979261d',
}
# The refusal for each reason the BIPs give.
REASONS = {
    'Invalid human-readable part': 'SW_UNKNOWN_HRP',
    'Invalid checksum': 'SW_BAD_CHECKSUM',
    'Invalid checksum (Bech32 instead of Bech32m)': 'SW_WRONG_VARIANT',
    'Invalid checksum (Bech32m instead of Bech32)': 'SW_WRONG_VARIANT',
    'Invalid character in checksum': 'SW_BAD_FORMAT',
    'Invalid witness version': 'SW_BAD_VERSION',
    'Invalid program length': 'SW_BAD_LENGTH',
    'Invalid program length (1 byte)': 'SW_BAD_LENGTH',
    'Invalid program length (41 bytes)': 'SW_BAD_LENGTH',
    'Invalid program length for witness version 0 (per BIP141)': 'SW_BAD_V0_LENGTH',
    'Mixed case': 'SW_MIXED_CASE',
    'zero padding of more than 4 bits': 'SW_BAD_PADDING',
    'Non-zero padding in 8-to-5 conversion': 'SW_BAD_PADDING',
    'Empty data section': 'SW_NO_VERSION',
}
# BIP-173 predates BIP-350, and its addresses for witness versions 1 to 16
# have Bech32 checksums. Under BIP-350 they refuse as the wrong variant
# before any later rule: the three valid ones (versions 1, 16 and 2) and
# three invalid ones (versions 3, 15 and 2, whose BIP-173 reasons are
# program length, program length and padding).
BIP173_WRONG_VARIANT = {
    'bc1pw508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7k7grplx',
    'BC1SW50QA3JX3S',
    'bc1zw508d6qejxtdg4y5r3zarvaryvg6kdaj',
    'bc1rw5uspcuh',
    'bc10w508d6qejxtdg4y5r3zarvary0c5xw7kw508d6qejxtdg4y5r3zarvary0c5xw7kw5rljs90',
    'bc1zw508d6qejxtdg4y5r3zarvaryvqyzf3du',
}
NETWORKS = {'bc': 'SW_MAINNET', 'tb': 'SW_TESTNET', 'bcrt': 'SW_REGTEST'}
CHAINS = {'main': 'SW_MAINNET', 'testnet4': 'SW_TESTNET', 'signet': 'SW_TESTNET', 'regtest': 'SW_REGTEST'}


def read(name):
    data = (UPSTREAM / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data


def section(text, title):
    """The lines of one test-vector section, up to the next heading."""
    start = text.index(title)
    end = text.find('\n==', start + len(title))
    return text[start:end if end > 0 else len(text)]


def script(hex_script):
    """The witness version and program a segwit scriptPubKey holds."""
    s = bytes.fromhex(hex_script)
    assert s[0] == 0 or 0x51 <= s[0] <= 0x60, hex_script
    assert s[1] == len(s) - 2, hex_script
    return (0 if s[0] == 0 else s[0] - 0x50), s[2:].hex()


def network(text):
    return NETWORKS[text[:text.rindex('1')].lower()]


def segwit(text, bip):
    valid = re.findall(r'^\* <tt>([^<]+)</tt>: <tt>([0-9a-f]+)</tt>$', section(text, 'The following list gives valid segwit'), re.M)
    invalid = re.findall(r'^\* <tt>([^<]+)</tt>: (.+)$', section(text, 'The following list gives invalid segwit'), re.M)
    rows = []
    for t, spk in valid:
        version, program = script(spk)
        code = 'SW_WRONG_VARIANT' if bip == 173 and t in BIP173_WRONG_VARIANT else '0'
        rows.append((t, code, network(t), version, program, f'BIP-{bip} valid'))
    for t, reason in invalid:
        code = 'SW_WRONG_VARIANT' if bip == 173 and t in BIP173_WRONG_VARIANT else REASONS[reason.strip()]
        rows.append((t, code, '0', 0, '', f'BIP-{bip} invalid: {reason.strip()}'))
    return rows, len(valid), len(invalid)


def tables():
    b350 = read('bip350/bip-0350.mediawiki').decode()
    bm = section(b350, '===Test vectors for Bech32m===')
    good = re.findall(r'^\* <tt>([^<]+)</tt>$', bm.split('The following string are not valid')[0], re.M)
    bad_part = bm.split('The following string are not valid')[1]
    bad = re.findall(r'^\* <tt>([^<]+)</tt>: ', bad_part, re.M)
    lead = re.findall(r'^\* 0x([0-9A-F]{2}) \+ <tt>([^<]+)</tt>: ', bad_part, re.M)
    assert (len(good), len(bad), len(lead)) == (7, 11, 3)
    rows350, v350, i350 = segwit(section(b350, '===Test vectors for v0-v16'), 350)
    rows173, v173, i173 = segwit(section(read('bip173/bip-0173.mediawiki').decode(), '===Test vectors==='), 173)
    assert (v350, i350, v173, i173) == (8, 15, 6, 10)
    assert sum(r[0] in BIP173_WRONG_VARIANT for r in rows173) == len(BIP173_WRONG_VARIANT)
    keyio = []
    for t, spk, meta in json.loads(read('bitcoin-core/key_io_valid.json')):
        if t.lower().startswith(('bc1', 'tb1', 'bcrt1')):
            assert not meta['isPrivkey']
            version, program = script(spk)
            keyio.append((t, '0', CHAINS[meta['chain']], version, program, f'key_io_valid {meta["chain"]}'))
    assert len(keyio) == 32
    invalid = [row[0] for row in json.loads(read('bitcoin-core/key_io_invalid.json'))]
    assert len(invalid) == 70 and all('"' not in s and '\\' not in s for s in invalid)
    return good, bad, lead, rows350 + rows173 + keyio, invalid


def render(good, bad, lead, rows, invalid):
    def strings(name, items):
        return '%s :: [%d] string.[\n%s];' % (name, len(items), ',\n'.join('  "%s"' % s for s in items))
    out = ['// Generated by make_bech32m_vectors.py from BIP-350, BIP-173 and Bitcoin',
           '// Core\'s key_io_valid.json and key_io_invalid.json, pinned in',
           '// coax/tests/vectors/upstream/ (see SOURCES.md there). Published values,',
           '// copied, not computed. Test data only.',
           strings('SW_BECH32M_VALID', good),
           '// BIP-350\'s invalid Bech32m strings. Three start with a byte outside',
           '// printable ASCII; SW_BECH32M_LEAD holds it and SW_BECH32M_TAIL the rest.',
           strings('SW_BECH32M_INVALID', bad),
           'SW_BECH32M_LEAD :: [3] u8.[%s];' % ', '.join('0x' + b.lower() for b, _ in lead),
           strings('SW_BECH32M_TAIL', [t for _, t in lead]),
           '// Segwit addresses: code is 0 for a valid address, with its network,',
           '// witness version and program (hex), or the SW_* refusal its reason names.',
           'SwCase :: struct {text:string;code:int;network:int;version:int;program:string;}',
           'SW_CASES :: [%d] SwCase.[' % len(rows)]
    for i, (t, code, net, version, program, note) in enumerate(rows):
        out.append('  SwCase.{text="%s",code=%s,network=%s,version=%d,program="%s"}%s  // %s'
                   % (t, code, net, version, program, ',' if i + 1 < len(rows) else '];', note))
    out += [
           strings('SW_KEY_IO_INVALID', invalid)]
    return '\n'.join(out) + '\n'


if __name__ == '__main__':
    Path(__file__).with_name('bech32m_vectors.coax').write_text(render(*tables()))

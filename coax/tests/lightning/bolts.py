#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: reads the BOLTs' published test vectors outside Coax)
"""The pinned BOLT texts for Bitcoin chapters 104 to 107, read and checked.

The BOLTs at lightning/bolts revision 19146282 are pinned in
coax/tests/vectors/upstream/bolts/. Every read checks the file's SHA-256
against its pin, so a changed text cannot change a vector silently.
"""
import hashlib
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
UP = HERE.parent / 'vectors/upstream/bolts'
PINS = {
    '01-messaging.md': '5801219aabcf854cf0a6dd4e98759b4351cbca238588061c680574912f6ef918',
    '03-transactions.md': '27ffbf1ae436f257bff3f255b4d1e4d4bbf8597fb1e2533d7ff9b66f4fe38d43',
    '08-transport.md': '85a1d6402adb653416c5a48f3b4c8eac2d8964621c41cb1bb6e09b8e06fbde46',
    '09-features.md': '39ead9fa5743104a3e66e0f6c342e34d64966dc943dcbcc337969e543e3e3cbe',
}


def text(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], f'{name} differs from its pin'
    return data.decode()


def section(body, start, end):
    """The text from the heading line `start` up to the heading line `end`."""
    a = body.index(start)
    b = body.index(end, a + len(start)) if end else len(body)
    return body[a:b]


# ---- BOLT 1

def bigsize():
    """Appendix A: the decoding rows, each marked if it is also an encoding row."""
    body = text('01-messaging.md')
    blocks = re.findall(r'```json\n(.*?)\n```', section(body, '## Appendix A', '## Appendix B'), re.S)
    decode, encode = json.loads(blocks[0]), json.loads(blocks[1])
    encoded = {(r['value'], r['bytes']) for r in encode}
    for r in decode:
        r['encode'] = (r['value'], r['bytes']) in encoded and 'exp_error' not in r
    assert all((r['value'], r['bytes']) in {(d['value'], d['bytes']) for d in decode} for r in encode)
    return decode


def _streams(chunk):
    out = []
    for m in re.finditer(r'1\. (Valid|Invalid) stream: 0x([0-9a-f ]*)\n2\. (?:Reason|Explanation|Values): ([^\n]*)', chunk):
        out.append((m.group(1) == 'Valid', m.group(2).replace(' ', ''), m.group(3).strip()))
    return out


def _values(text):
    """`tlv1` `amount_msat`=0 and the like, as a dict."""
    fields = dict(re.findall(r'`(\w+)`=(\S+)', text))
    record = int(re.match(r'`tlv(\d)`', text).group(1))
    return record, fields


def tlv():
    """Appendix B: (stream, namespace, ok, reason, record, fields). Namespace
    0 is every namespace."""
    body = section(text('01-messaging.md'), '## Appendix B', '## Appendix C')
    rows = []
    groups = re.split(r'\n(?=[^\n]*following TLV stream)', body)
    for g in groups:
        head = ' '.join(g.split('\n')[:2])
        if 'namespace `n1`' in head or '`n1` namespace' in head:
            ns = 1
        elif 'namespace `n2`' in head:
            ns = 2
        elif 'any namespace' in head or 'either namespace' in head:
            ns = 0
        else:
            continue
        for ok, stream, why in _streams(g.split('\n### ')[0]):
            record, fields = (_values(why) if ok and ns == 1 else (0, {}))
            rows.append((stream, ns, ok, why, record, fields))
    return rows


def init_extension():
    """Appendix C: (message, ok, why)."""
    body = section(text('01-messaging.md'), '## Appendix C', '## Appendix D')
    valid, invalid = body.split('The following `init` messages are invalid:')
    rows = []
    for part, ok in ((valid, True), (invalid, False)):
        for m in re.finditer(r'- `0x([0-9a-f]+)`: ([^\n]*)', part):
            rows.append((m.group(1), ok, m.group(2)))
    return rows


def features():
    """BOLT 9's table: bit pair -> name."""
    names = {}
    for m in re.finditer(r'^\| (\d+)/(\d+) +\| `(\w+)`', text('09-features.md'), re.M):
        names[int(m.group(1)) // 2] = m.group(3)
    return names


def scid(text):
    block, tx, out = (int(x) for x in text.split('x'))
    return (block << 40) | (tx << 16) | out

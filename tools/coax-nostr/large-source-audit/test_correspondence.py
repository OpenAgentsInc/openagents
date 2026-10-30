#!/usr/bin/env python3
"""Check scoped correspondence with the checked-operation-corrected v2 profile,
plus this profile's checked-operation range rules (large-delta.json)."""
import hashlib
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OLD = ROOT/'tools/coax-nostr/public-view-check'
NEW = Path(__file__).resolve().parent
PINS = {
    'aggregate.coax':'55af5286b8b8c4ee57a9ea60ebc6a85e5e54ae9eeaaeff80b4e6380429db9d08',
    'bridge-aggregate.cpp':'ac56e8d614612532900a62462a6089e4c0de6f2857c57d8c52a98cd8cfb5754c',
    'check.py':'4a478426bcf468e5e4d319185bffce2b503ac62e3fc2ef4659cbb13d77656e6b',
    'aggregate.py':'6e3babd692fdb5a3ea351b6abf50ed9d33aef6856a9857d868d37c716ac9078d',
}

def replace_exact(text, old, new, count=1):
    if text.count(old) != count:
        raise ValueError('unexpected correspondence anchor: '+old)
    return text.replace(old, new)

def expected(name):
    raw = (OLD/name).read_bytes()
    if hashlib.sha256(raw).hexdigest() != PINS[name]:
        raise ValueError('reviewed profile changed: '+name)
    text = raw.decode()
    if name == 'aggregate.coax':
        text = replace_exact(text, '#load "../../../coax/lib/crypto/secret-flow.coax";', '#load "inventory.coax";')
        text = replace_exact(text, 'if !SC_shape(nodes,roots,types) || values.count!=nodes.count || memory.count!=1',
                             'if !LA_shape(nodes,roots,types) || values.count!=nodes.count || memory.count!=1')
    elif name == 'bridge-aggregate.cpp':
        for old, new in [('SC_units(count)','LA_units(count)'),
                         ('SC_inputs({lengths.data(),(int64_t)lengths.size()},0)','LA_inputs({lengths.data(),(int64_t)lengths.size()},0)'),
                         ('SC_node_limit()','LA_node_limit()')]:
            text = replace_exact(text, old, new)
    elif name == 'check.py':
        for old, new in [('self.lib.SC_units','self.lib.LA_units'),
                         ('self.lib.SC_inputs','self.lib.LA_inputs'),
                         ('self.lib.SC_closure','self.lib.LA_closure')]:
            count = text.count(old)
            if count < 2: raise ValueError('missing driver identity anchor: '+old)
            text = replace_exact(text, old, new, count)
    elif name == 'aggregate.py':
        text = replace_exact(text, 'aggregate-source-v1', 'large-aggregate-source-v3')
        old = 'coax.secret-classification-views.v2'
        count = text.count(old)
        if count == 0: raise ValueError('missing classification schema')
        text = replace_exact(text, old, 'coax.secret-large-classification-views.v3', count)
        text = replace_exact(text, "if set(manifest)!=required or (not views and manifest.get('schema')!='coax.secret-classification.v1')",
                             "if set(manifest)!=required or not views")
    # This profile's own rules (contained casts, unsigned ranges, loop
    # invariants, public fields) are an exact, ordered list of block
    # replacements per file.
    for old, new in json.loads((NEW/'large-delta.json').read_text()).get(name, []):
        text = replace_exact(text, old, new)
    return text.encode()


class Correspondence(unittest.TestCase):
    def test_complete_profile_bytes_and_seeded_change(self):
        for name in PINS:
            with self.subTest(name=name):
                actual = (NEW/name).read_bytes()
                self.assertEqual(actual, expected(name))
                self.assertNotEqual(actual+b'\n# seeded unexpected edit\n', expected(name))


if __name__ == '__main__':
    unittest.main()

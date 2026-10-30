#!/usr/bin/env python3
"""Check scoped correspondence with the checked-operation-corrected v2 profile,
plus this profile's range-contained checked-cast rule."""
import hashlib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OLD = ROOT/'tools/coax-nostr/public-view-check'
NEW = Path(__file__).resolve().parent
PINS = {
    'aggregate.coax':'391dfdbbc101f1f0ecff76b0e0c4cc850763b596bc2e335b8838df5dfe54895b',
    'bridge-aggregate.cpp':'ac56e8d614612532900a62462a6089e4c0de6f2857c57d8c52a98cd8cfb5754c',
    'check.py':'4a478426bcf468e5e4d319185bffce2b503ac62e3fc2ef4659cbb13d77656e6b',
    'aggregate.py':'6e3babd692fdb5a3ea351b6abf50ed9d33aef6856a9857d868d37c716ac9078d',
}

CONTAINED = "// A checked cast whose source type's whole range lies inside the target's\n// cannot reach its range trap, so the emitted check's branch never depends\n// on the value. Only unsigned strict widening qualifies: u8 -> u32/u64 and\n// u32 -> u64. Every other checked cast of a secret refuses: identity,\n// narrowing, any int source or target, bool and others.\nAG_contained :: (from:int,to:int)->bool {\n if from==-5 {return to==-2 || to==-1;}\n if from==-2 {return to==-1;}\n return false;\n}\n// The cast's target as a primitive code, read like AG_type but without\n// failing: anything other than a primitive is 0, which nothing contains.\nAG_cast_primitive :: (nodes:[]ScNode,id:int)->int {\n if id<0 {return ifx id>=-5 then id else 0;}\n if SC_ref(id,nodes.count) && nodes[id].kind==1 {return AG_primitive(nodes[id].name);}\n return 0;\n}\n"
CONTAINED_TEST = '#test "only range-contained checked casts admit a secret operand" {\n #assert !AG_contained(-5,-5) && AG_contained(-5,-2) && AG_contained(-5,-1) && !AG_contained(-5,-4);\n #assert !AG_contained(-2,-2) && AG_contained(-2,-1) && !AG_contained(-2,-4) && !AG_contained(-2,-5);\n #assert !AG_contained(-1,-1) && !AG_contained(-1,-2) && !AG_contained(-1,-5) && !AG_contained(-1,-4);\n #assert !AG_contained(-4,-4) && !AG_contained(-4,-1) && !AG_contained(-4,-2) && !AG_contained(-4,-5);\n #assert !AG_contained(-3,-3) && !AG_contained(-3,-1) && !AG_contained(-5,-3) && !AG_contained(-1,-3);\n #assert !AG_contained(0,-1) && !AG_contained(-5,0) && !AG_contained(0,0) && !AG_contained(-6,-1) && !AG_contained(-5,-6);\n state:[1]ScAudit;roots:[1]int;nodes:=[4]ScNode.[ScNode.{},ScNode.{kind=1,name="u64"},ScNode.{kind=1,name="Point"},ScNode.{kind=2,name="u8"}];\n #assert AG_cast_primitive(nodes[..],1)==-1 && AG_cast_primitive(nodes[..],2)==0 && AG_cast_primitive(nodes[..],3)==0;\n #assert AG_cast_primitive(nodes[..],-5)==-5 && AG_cast_primitive(nodes[..],-1)==-1 && AG_cast_primitive(nodes[..],-6)==0;\n #assert AG_cast_primitive(nodes[..],4)==0 && AG_cast_primitive(nodes[..],0)==0;\n // Node 0 resolves like any other node, as in AG_type.\n nodes[0]=ScNode.{kind=1,name="u32"};#assert AG_cast_primitive(nodes[..],0)==-2 && AG_type(nodes[..],roots[0..<0],0,state[..])==-2;\n}\n'

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
        # v2 of this profile: a secret checked cast is admitted only when its
        # source type's range lies inside the target's (CONTAINED below).
        text = replace_exact(text, 'if r.mask!=0 && !n.flag {SC_fail(state,SC_UNSUPPORTED,id);return AgValue.{};}',
                             'if r.mask!=0 && !n.flag && !AG_contained(r.type,AG_cast_primitive(nodes,n.d)) {SC_fail(state,SC_UNSUPPORTED,id);return AgValue.{};}')
        text = replace_exact(text, 'AG_primitive_name :: (type:int)->string', CONTAINED+'AG_primitive_name :: (type:int)->string')
        anchor = '#test "aggregate source bounds and scalar identities are explicit" {'
        text = replace_exact(text, anchor, CONTAINED_TEST+anchor)
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
        text = replace_exact(text, 'aggregate-source-v1', 'large-aggregate-source-v2')
        old = 'coax.secret-classification-views.v2'
        count = text.count(old)
        if count == 0: raise ValueError('missing classification schema')
        text = replace_exact(text, old, 'coax.secret-large-classification-views.v3', count)
        text = replace_exact(text, "if set(manifest)!=required or (not views and manifest.get('schema')!='coax.secret-classification.v1')",
                             "if set(manifest)!=required or not views")
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

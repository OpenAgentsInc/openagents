"""Tooling checks: authentic dataset bytes and independent, non-training math."""
import importlib.util
import hashlib
import json
import math
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "mnist_reference", Path(__file__).resolve().parents[1] / "mnist_reference.py")
ref = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ref)


class ReferenceChecks(unittest.TestCase):
    def test_published_splitmix_words_and_epoch_reconstruction(self):
        self.assertEqual(ref.random_word(0, 0), 0xe220a8397b1dcdaf)
        self.assertEqual(ref.random_word(1, 0), 0x910a2dec89025cc1)
        a, b = ref.epoch_order(7, 11, 0), ref.epoch_order(7, 11, 1)
        self.assertEqual(sorted(a), list(range(7)))
        self.assertNotEqual(a, b)
        self.assertEqual(ref.epoch_order(7, 11, 1), b)
        self.assertEqual(ref.epoch_order(1, 11, 8), [0])

    def test_pooling_has_exact_binary32_output(self):
        image = bytes([255] * 784)
        self.assertEqual(ref.pool(image), [1.0] * 196)
        image = bytes([0] * 784)
        self.assertEqual(ref.pool(image), [0.0] * 196)
        image = bytearray(784)
        image[0], image[1], image[28], image[29] = 1, 2, 3, 4
        self.assertEqual(ref.pool(image)[0], ref.f32(10 / 1020))
        with self.assertRaises(ValueError):
            ref.pool(image[:-1])

    def test_idx_header_exact_length_and_magic(self):
        header = struct.pack(">IIII", 2051, 2, 28, 28)
        spec = dict(kind="images", records=2, decompressed_bytes=1584)
        raw = header + bytes(1568)
        ref.check_idx(raw, spec)
        for changed in (raw[:-1], raw + b"x", b"BAD!" + raw[4:]):
            with self.assertRaises(ValueError):
                ref.check_idx(changed, spec)
        labels = struct.pack(">II", 2049, 2) + bytes([0, 10])
        with self.assertRaises(ValueError):
            ref.check_idx(labels, dict(kind="labels", records=2, decompressed_bytes=10))

    def test_digest_mismatch_fails_before_use(self):
        ref.check_digest(b"abc", "sha256", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        with self.assertRaises(ValueError):
            ref.check_digest(b"abd", "sha256", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")

    def test_dyadic_gradient_matches_exact_rationals(self):
        got = ref.tiny_oracle()
        self.assertEqual(got["preactivation"], [0.75, -0.40625, 0.125])
        self.assertEqual(got["logits"], [0.34375, -0.140625])
        self.assertEqual(got["fc1_bias_gradient"], [0.9140625, 0.0, -0.11328125])
        self.assertTrue(got["exact_fraction_check"])
        self.assertTrue(all(got["fc1_weight_gradient"][i] == 0 for i in (1, 4, 7, 10)))

    def test_real_profile_layout_and_finite_initial_values(self):
        p = ref.initial_parameters()
        self.assertEqual(len(p), 3322)
        self.assertTrue(all(math.isfinite(x) and x == ref.f32(x) for x in p))
        self.assertTrue(all(abs(x) <= ref.f32(1 / math.sqrt(196)) for x in p[:3152]))
        self.assertTrue(all(abs(x) <= 0.25 for x in p[3152:]))
        self.assertEqual(p[0], ref.initial_value(11801, 206, 196))
        self.assertEqual(p[-1], ref.initial_value(11801, 3527, 16))

    def test_zero_parameter_model_checks_target_and_gradient(self):
        y, loss, g, _ = ref.mlp([0.5] * 4, [0, 1], [0.0] * 23, 4, 3, 2)
        self.assertEqual(y, [0.0, 0.0])
        self.assertEqual(loss, 1.0)
        self.assertEqual(g[:-2], [0.0] * 21)
        self.assertEqual(g[-2:], [0.0, -2.0])

    def test_ir_forward_rounds_products_and_running_sum_to_f32(self):
        # Compensated f64 or math.fsum would give 1, so this distinguishes
        # the actual public autodiff IR's sequential binary32 forward path.
        self.assertEqual(ref.sum_f32([16777216.0, 1.0, -16777216.0]), 0.0)
        y, loss, _, pre = ref.mlp([16777216.0, 1.0, -16777216.0], [1.0],
                                 [1.0, 1.0, 1.0, 0.0, 1.0, 0.0], 3, 1, 1)
        self.assertEqual(pre, [0.0])
        self.assertEqual(y, [0.0])
        self.assertEqual(loss, 1.0)

    def test_capture_reader_refuses_short_extra_and_nonfinite_values(self):
        with tempfile.TemporaryDirectory() as directory:
            p = Path(directory) / "gradient.f64"
            for raw in (b"", struct.pack("<dd",1.0,2.0), struct.pack("<d",math.nan), struct.pack("<d",math.inf)):
                p.write_bytes(raw)
                with self.assertRaises(ValueError):
                    ref.read_floats(p, 1, "d")
            p.write_bytes(struct.pack("<d",1.25))
            self.assertEqual(ref.read_floats(p, 1, "d"), [1.25])

    def test_predeclared_capture_comparison_boundaries(self):
        ref.compare_values("gradient", [2e-6], [0.0], "d")
        with self.assertRaises(ValueError):
            ref.compare_values("gradient", [2.000001e-6], [0.0], "d")
        with self.assertRaises(ValueError):
            ref.compare_values("gradient", [math.nan], [0.0], "d")
        with self.assertRaises(ValueError):
            ref.compare_values("parameters", [1.00000001], [1.0], "d")
        with self.assertRaises(ValueError):
            ref.compare_values("features", [-0.0], [0.0], "f")

    @staticmethod
    def signed(tag,body):
        # Independent hashlib construction, not the reader's digest helper.
        tag_hash=hashlib.sha256(tag.encode()).digest()
        return hashlib.sha256(tag_hash+tag_hash+body).digest()

    def checkpoint_fixture(self,revision=1536):
        identities=tuple(bytes([i])*32 for i in range(1,5))
        header=(b"PSCK\x01\x00\x04\x00"+b"".join(identities)+bytes(32)+
                struct.pack("<7QI",revision,11802,revision//256,(revision%256)*16,
                            0,revision,0,3322))
        body=header+struct.pack("<9966d",*([0.0]*9966))
        d=self.signed("psionic.checkpoint.v1",body)
        fields=struct.pack("<4Qd",0,0,0,0,0.0)
        tail=b"PSTR\x01\x00\x00\x00"+fields+self.signed("psionic.trainer.v1",d+fields)
        pointer=b"PSCP\x01\x00\x00\x00"+struct.pack("<Q",revision)+d
        return body+d+tail,pointer+self.signed("psionic.checkpoint-pointer.v1",pointer),identities

    def test_checkpoint_pointer_and_trainer_tail_layout(self):
        for revision in (1536,1537,3072):
            raw,pointer,ids=self.checkpoint_fixture(revision)
            self.assertEqual(len(raw),80068)
            got=ref.parse_checkpoint(raw,ids,revision)
            ref.check_pointer(pointer,got)
            self.assertEqual(got["parameters"],[0.0]*3322)
            self.assertEqual(got["revision"],revision)

    def test_checkpoint_tampering_wrong_identity_and_stale_pointer_refuse(self):
        raw,pointer,ids=self.checkpoint_fixture()
        for at in (0,224,228,79956,79988,80067):
            bad=bytearray(raw);bad[at]^=1
            with self.assertRaises(ValueError):ref.parse_checkpoint(bad,ids,1536)
        with self.assertRaises(ValueError):ref.parse_checkpoint(raw[:-1],ids,1536)
        with self.assertRaises(ValueError):ref.parse_checkpoint(raw,ids,1537)
        with self.assertRaises(ValueError):ref.parse_checkpoint(raw,(bytes(32),*ids[1:]),1536)
        got=ref.parse_checkpoint(raw,ids,1536)
        stale=bytearray(pointer);struct.pack_into("<Q",stale,8,1537)
        stale[48:]=self.signed("psionic.checkpoint-pointer.v1",stale[:48])
        with self.assertRaises(ValueError):ref.check_pointer(stale,got)

    def test_valid_digests_do_not_admit_impossible_checkpoint_numbers(self):
        raw,_,ids=self.checkpoint_fixture()
        body_end=228+24*3322
        for at,value in ((228,math.nan),(228+2*3322*8,-1.0),(228,17.0)):
            bad=bytearray(raw);struct.pack_into("<d",bad,at,value)
            d=self.signed("psionic.checkpoint.v1",bad[:body_end])
            bad[body_end:body_end+32]=d
            fields=bad[body_end+40:body_end+80]
            bad[-32:]=self.signed("psionic.trainer.v1",d+fields)
            with self.assertRaises(ValueError):ref.parse_checkpoint(bad,ids,1536)

    def export_fixture(self):
        desc={"__metadata__":{"format":"psionic-interchange-v1"}}
        offset=0
        for name,shape,n in (("fc1.weight",[196,16],3136),("fc1.bias",[16],16),
                             ("fc2.weight",[16,10],160),("fc2.bias",[10],10)):
            desc[name]=dict(dtype="F32",shape=shape,data_offsets=[offset,offset+4*n]);offset+=4*n
        header=json.dumps(desc).encode()
        return struct.pack("<Q",len(header))+header+bytes(4*3322),desc

    def test_export_compares_all_named_tensor_words(self):
        raw,desc=self.export_fixture()
        self.assertTrue(ref.check_export(raw,[0.0]*3322)["all_exported_words_match"])
        bad=bytearray(raw);bad[-4:]=struct.pack("<f",1.0)
        with self.assertRaises(ValueError):ref.check_export(bad,[0.0]*3322)
        desc["fc1.weight"]["shape"]=[16,196]
        h=json.dumps(desc).encode()
        with self.assertRaises(ValueError):ref.check_export(struct.pack("<Q",len(h))+h+bytes(4*3322),[0.0]*3322)
        with self.assertRaises(ValueError):json.loads('{"x":1,"x":2}',object_pairs_hook=ref.unique_json)

    def test_metrics_pin_first_tie_and_f32_sum_squared_error(self):
        scores=[0.0]*10+[1.0,1.0]+[0.0]*8
        got=ref.score_metrics(scores,bytes([0,1]),0)
        self.assertEqual(got,dict(correct=1,baseline_correct=1,mean_sse=1.0))
        with self.assertRaises(ValueError):ref.score_metrics(scores[:-1],bytes([0,1]),0)
        scores[0]=math.inf
        with self.assertRaises(ValueError):ref.score_metrics(scores,bytes([0,1]),0)


if __name__ == "__main__":
    unittest.main()

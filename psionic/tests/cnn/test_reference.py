"""Independent CNN formulas and saved-artifact audit regressions; never train."""
import hashlib
import importlib.util
import json
import math
from fractions import Fraction as Q
from pathlib import Path
import struct
import unittest

spec = importlib.util.spec_from_file_location(
    "cnn_reference", Path(__file__).resolve().parents[1] / "cnn_reference.py")
ref = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ref)


def tiny():
    x = [(i-5)/4 for i in range(16)]
    conv = [1/4,0,1/4,-1/4,1/2,0,1/4,-1/4,1/4,
            -1/4,1/4,0,1/4,-1/4,1/2,-1/4,1/4,0]
    return x, [0.0,1.0], conv+[1/2,1]+[((i*5)%7-3)/4 for i in range(16)]+[1/4,-1/2]


def rational_loss(x, target, p):
    """An exact expression for the tiny 4x4, two-filter, two-class model."""
    h = [max(Q(0), sum(Q(x[(r+kr)*4+c+kc])*Q(p[f*9+kr*3+kc])
         for kr in range(3) for kc in range(3))+Q(p[18+f]))
         for f in range(2) for r in range(2) for c in range(2)]
    y = [sum(h[j]*Q(p[20+j*2+k]) for j in range(8))+Q(p[36+k]) for k in range(2)]
    return y, sum((y[k]-Q(target[k]))**2 for k in range(2))


class CnnReferenceChecks(unittest.TestCase):
    def test_dyadic_forward_and_all_derivatives_against_exact_central_difference(self):
        x,t,p = tiny()
        y,loss,g,pre = ref.cnn(x,t,p,side=4,classes=2)
        exact_y,exact_loss = rational_loss(x,t,p)
        self.assertEqual(y,list(map(float,exact_y)))
        self.assertEqual(loss,float(exact_loss))
        self.assertTrue(all(v>0 for v in pre))
        delta=Q(1,1024)
        for i in range(len(p)):
            a=list(map(Q,p));b=a.copy();a[i]+=delta;b[i]-=delta
            numeric=(rational_loss(x,t,a)[1]-rational_loss(x,t,b)[1])/(2*delta)
            self.assertEqual(g[i],float(numeric),i)
        self.assertTrue(all(v!=0 for v in g[:18]))
        self.assertNotEqual(g[1],g[3])

    def test_channel_major_flatten_and_kernel_asymmetry_are_observable(self):
        x,t,p=tiny();y,_,_,_=ref.cnn(x,t,p,side=4,classes=2)
        transposed=[x[c*4+r] for r in range(4) for c in range(4)]
        self.assertNotEqual(y,ref.cnn(transposed,t,p,side=4,classes=2)[0])
        flipped=p.copy();flipped[:9]=list(reversed(p[:9]))
        self.assertNotEqual(y,ref.cnn(x,t,flipped,side=4,classes=2)[0])
        mixed=p.copy()
        # Interpret the same fc payload in spatial-major channel order.
        for spatial in range(4):
            for channel in range(2):
                for k in range(2):
                    mixed[20+(channel*4+spatial)*2+k]=p[20+(spatial*2+channel)*2+k]
        self.assertNotEqual(y,ref.cnn(x,t,mixed,side=4,classes=2)[0])

    def test_relu_removes_one_filters_derivatives(self):
        x,t,p=tiny();p[18]=-100
        _,_,g,pre=ref.cnn(x,t,p,side=4,classes=2)
        self.assertTrue(all(v<0 for v in pre[:4]))
        self.assertEqual(g[:9],[0.0]*9)
        self.assertEqual(g[18],0.0)
        self.assertEqual(g[20:28],[0.0]*8)
        self.assertTrue(any(v!=0 for v in g[9:18]))

    def test_zero_model_and_ordered_f32_channel_accumulation(self):
        _,loss,g,_=ref.cnn([0.5]*196,[0.0]*9+[1.0],[0.0]*2910)
        self.assertEqual(loss,1.0);self.assertEqual(g[:-1],[0.0]*2909);self.assertEqual(g[-1],-2)
        # f64 accumulation would retain the middle 1, whereas this profile loses it.
        y,loss,_,pre=ref.cnn([16777216.0,1.0,-16777216.0],[1.0],
            [1.0,1.0,1.0,0.0,1.0,0.0],side=1,channels=3,filters=1,kernel=1,classes=1)
        self.assertEqual(pre,[0.0]);self.assertEqual(y,[0.0]);self.assertEqual(loss,1.0)

    def test_real_initializer_offsets_and_shape_refusal(self):
        p=ref.initial_parameters();self.assertEqual(len(p),2910)
        self.assertEqual(p[0],ref.initial_value(11811,206,9))
        self.assertEqual(p[19],ref.initial_value(11811,225,9))
        self.assertEqual(p[20],ref.initial_value(11811,226,288))
        self.assertEqual(p[-1],ref.initial_value(11811,3115,288))
        self.assertTrue(all(math.isfinite(v) and ref.f32(v)==v for v in p))
        for x,t,w in (([0.0]*195,[0.0]*10,p),([0.0]*196,[0.0]*9,p),([0.0]*196,[0.0]*10,p[:-1])):
            with self.assertRaises(ValueError):ref.cnn(x,t,w)

    @staticmethod
    def tagged(tag,payload):
        t=hashlib.sha256(tag.encode()).digest();return hashlib.sha256(t+t+payload).digest()

    def checkpoint(self,revision=1536):
        ids=tuple(bytes([i])*32 for i in range(1,5))
        header=(b"PSCK\x01\x00\x04\x00"+b"".join(ids)+bytes(32)+
                struct.pack("<7QI",revision,11812,revision//256,(revision%256)*16,0,revision,0,2910))
        body=header+bytes(2910*24);d=self.tagged("psionic.checkpoint.v1",body)
        fields=struct.pack("<4Qd",0,0,0,0,0.0)
        tail=b"PSTR\x01\x00\x00\x00"+fields+self.tagged("psionic.trainer.v1",d+fields)
        return body+d+tail,ids

    def test_checkpoint_counts_domains_and_tampering(self):
        for revision in (1536,1537,3072):
            raw,ids=self.checkpoint(revision);self.assertEqual(len(raw),70180)
            q=ref.parse_checkpoint(raw,ids,revision);self.assertEqual(q["parameters"],[0.0]*2910)
            for at in (0,224,228,len(raw)-1):
                bad=bytearray(raw);bad[at]^=1
                with self.assertRaises(ValueError):ref.parse_checkpoint(bad,ids,revision)
            with self.assertRaises(ValueError):ref.parse_checkpoint(raw,ids,revision+1)
            with self.assertRaises(ValueError):ref.parse_checkpoint(raw,(bytes(32),*ids[1:]),revision)

    def test_cnn_export_exact_names_shapes_offsets_and_words(self):
        p=ref.initial_parameters();desc={"__metadata__":{"format":"psionic-interchange-v1"}};payload=b""
        for name,(shape,first,n) in ref.EXPORT_SHAPES.items():
            part=struct.pack(f"<{n}f",*p[first:first+n]);lo=len(payload);payload+=part
            desc[name]={"dtype":"F32","shape":shape,"data_offsets":[lo,len(payload)]}
        head=json.dumps(desc,separators=(",",":")).encode();raw=struct.pack("<Q",len(head))+head+payload
        self.assertTrue(ref.check_export(raw,p)["all_exported_words_match"])
        changed=bytearray(raw);changed[-1]^=1
        with self.assertRaises(ValueError):ref.check_export(changed,p)
        desc["conv.bias"]["shape"]=[2]
        head=json.dumps(desc,separators=(",",":")).encode()
        with self.assertRaises(ValueError):ref.check_export(struct.pack("<Q",len(head))+head+payload,p)

    def test_declared_comparison_tolerance_is_not_widened(self):
        ref.compare_values("gradient",[2e-6],[0.0],"d")
        with self.assertRaises(ValueError):ref.compare_values("gradient",[2.000001e-6],[0.0],"d")
        with self.assertRaises(ValueError):ref.compare_values("parameters",[1.00000001],[1.0],"d")


if __name__=="__main__":unittest.main()

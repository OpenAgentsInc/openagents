"""The painterly texture rules of pack_realm.py (docs/verse/art-styles.md, 10).
Tooling only, never shipped.

    python3 verse/tools/test_pack_style.py
"""
import copy, io, json, os, random, struct, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pack_realm as P
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = json.loads((ROOT / "verse/packs/fixture.json").read_text())
HAVE_KIT = P.home(FIXTURE["scene"]).exists()


def flat(rgb, size=64):
    return Image.new("RGB", (size, size), rgb)


def grainy(size=64, seed=7):
    """A mid-green layer with texel-scale grain: what "fine detail" means."""
    r = random.Random(seed)
    im = Image.new("RGB", (size, size))
    im.putdata([(60 + d, 120 + d, 40 + d) for d in (r.randint(-40, 40) for _ in range(size * size))])
    return im


def catalog(pack):
    n, = struct.unpack_from("<I", pack, 4)
    off, size = struct.unpack_from("<II", pack, P.HEADER + 4)
    return pack[off:off + size].decode()


class Measures(unittest.TestCase):
    def test_flat_layer_has_no_fine_detail(self):
        e, s, v = P.layer_measures(flat((120, 160, 60)))
        self.assertEqual(e, 0.0)
        self.assertAlmostEqual(s, 1 - 60 / 160, places=2)
        self.assertAlmostEqual(v, 160 / 255, places=2)

    def test_grainy_layer_fails_with_its_energy(self):
        why = P.layer_refusal("grain.jpg", grainy())
        self.assertIsNotNone(why)
        self.assertIn("fine-detail energy", why)
        e = P.layer_measures(grainy())[0]
        self.assertGreater(e, P.FINE_MAX)
        self.assertIn(f"{e:.3f}", why)

    def test_cut_out_texels_dont_count(self):
        im = grainy().convert("RGBA")
        im.putalpha(0)
        im.paste(Image.new("RGBA", (16, 16), (120, 160, 60, 255)), (0, 0))
        self.assertEqual(P.layer_measures(im)[0], 0.0)

    def test_saturation_and_value_outside_the_albedo_rules_fail(self):
        self.assertIn("saturation", P.layer_refusal("grey", flat((128, 128, 128))))
        self.assertIn("saturation", P.layer_refusal("neon", flat((0, 255, 0))))
        self.assertIn("value", P.layer_refusal("dark", flat((20, 30, 10))))
        self.assertIsNone(P.layer_refusal("ok", flat((120, 160, 60))))


class PaintPass(unittest.TestCase):
    def test_paint_makes_a_grainy_layer_pass(self):
        before = grainy(128)
        self.assertIsNotNone(P.layer_refusal("grain", before))
        after = P.paint(before)
        self.assertIsNone(P.layer_refusal("grain (painted)", after))
        self.assertLessEqual(len(set(after.convert("RGB").tobytes()[i:i + 3] for i in range(0, 3 * 128 * 128, 3))), P.PAINT_COLOURS)

    def test_paint_pulls_saturation_and_value_in(self):
        self.assertIsNone(P.layer_refusal("neon (painted)", P.paint(flat((0, 255, 0)))))
        self.assertIsNone(P.layer_refusal("grey (painted)", P.paint(flat((128, 128, 128)))))

    def test_paint_keeps_cutouts(self):
        im = grainy().convert("RGBA")
        im.putalpha(Image.new("L", im.size, 0))
        im.paste(Image.new("RGBA", (16, 16), (120, 160, 60, 255)), (8, 8))
        out = P.paint(im)
        self.assertEqual(out.getchannel("A").tobytes(), im.getchannel("A").tobytes())

    def test_kuwahara_keeps_an_edge_and_flattens_grain(self):
        im = Image.new("RGB", (32, 32), (200, 60, 40))
        im.paste((40, 60, 200), (16, 0, 32, 32))
        out = P.kuwahara(im, 2)
        self.assertEqual(out.getpixel((15, 10)), (200, 60, 40))
        self.assertEqual(out.getpixel((16, 10)), (40, 60, 200))
        self.assertLess(P.layer_measures(P.kuwahara(grainy(), 2))[0], P.layer_measures(grainy())[0])


class Manifest(unittest.TestCase):
    def build(self, change):
        m = copy.deepcopy(FIXTURE)
        change(m)
        return P.build(m)

    def test_unknown_style_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "style is one of"):
            self.build(lambda m: m.update(style="cel"))

    def test_a_painterly_pack_names_no_maps(self):
        with self.assertRaisesRegex(SystemExit, "no normal, roughness or detail maps"):
            self.build(lambda m: m.update(style="painterly", normal_map="bark-normal{n}"))
        with self.assertRaisesRegex(SystemExit, r"Barrel\.roughness"):
            self.build(lambda m: m.update(style="painterly", meshes=[{"name": "Barrel", "roughness": "x"}]))

    def test_a_painterly_layer_is_at_most_512(self):
        with self.assertRaisesRegex(SystemExit, "at most 512"):
            self.build(lambda m: m.update(style="painterly", texture_size=1024))

    @unittest.skipUnless(HAVE_KIT, "the kit's scene.bin and layers aren't on this machine")
    def test_the_fixture_pack_unpainted_fails_its_layers(self):
        with self.assertRaisesRegex(SystemExit, r"painterly layer refused: .*paint\": true"):
            self.build(lambda m: (m.update(style="painterly"), m.pop("texture_size")))

    @unittest.skipUnless(HAVE_KIT, "the kit's scene.bin and layers aren't on this machine")
    def test_the_fixture_pack_tagged_painterly_and_painted_passes(self):
        m, keys, data, *_ = self.build(lambda m: (m.update(style="painterly", paint=True), m.pop("texture_size")))
        self.assertTrue(catalog(data).startswith("style painterly\npack fixture\n"))
        self.assertLessEqual(len(data), m["budget"])

    @unittest.skipUnless(HAVE_KIT, "the kit's scene.bin and layers aren't on this machine")
    def test_a_pack_without_a_style_is_unchanged(self):
        m, keys, data, *_ = P.build(copy.deepcopy(FIXTURE))
        self.assertTrue(catalog(data).startswith("pack fixture\n"))
        shipped = ROOT / "apps/openagents-web/realms/fixture/assets/fixture.pack"
        if shipped.exists():
            self.assertEqual(catalog(data), catalog(shipped.read_bytes()))


class Canopy(unittest.TestCase):
    """Blob canopies (art-styles.md 7): a tree's leaves carry their shape, colour and per-vertex blob."""
    def test_shapes_by_name(self):
        self.assertEqual(P.canopy_shape("CommonTree_3_Leaves"), "sphere")
        self.assertEqual(P.canopy_shape("Pine_5_Leaves"), "ellipsoid")
        self.assertIsNone(P.canopy_shape("CommonTree_3"))

    def test_the_blob_points_out_of_the_centre_with_the_height_share(self):
        v = bytearray()
        for p in ((0.0, 4.0, 0.0), (2.0, 2.0, 0.0), (0.0, 0.0, -1.0)):
            v += struct.pack("<3f", *p) + bytes(36)
        P.canopy_blob(v, 0, [-2.0, 0.0, -1.0], [2.0, 4.0, 1.0], False)
        top = struct.unpack_from("<3bB", v, 36)
        self.assertEqual(top, (0, 127, 0, 255))
        side = struct.unpack_from("<3bB", v, 48 + 36)
        self.assertEqual((side[0], side[1], side[2], side[3]), (127, 0, 0, 128))
        # an ellipsoid leans toward its short axis: z's half size is 1 against y's 2
        self.assertEqual(struct.unpack_from("<3bB", v, 96 + 36)[2], -57)
        P.canopy_blob(v, 0, [-2.0, 0.0, -1.0], [2.0, 4.0, 1.0], True)
        self.assertEqual(struct.unpack_from("<3bB", v, 96 + 36)[2], -114)

    @unittest.skipUnless(HAVE_KIT, "the kit's scene.bin and layers aren't on this machine")
    def test_the_fixture_pack_names_its_canopy(self):
        m, keys, data, *_ = P.build(copy.deepcopy(FIXTURE))
        line = next(x for x in catalog(data).split("\n") if x.startswith("M CommonTree_3_Leaves "))
        words = line.split()
        self.assertEqual(words[11:13], ["canopy", "sphere"])
        r, g, b = map(int, words[13:16])
        self.assertTrue(g > r >= b and g > 50)
        trunk = next(x for x in catalog(data).split("\n") if x.startswith("M CommonTree_3 "))
        self.assertNotIn("canopy", trunk)


if __name__ == "__main__":
    unittest.main()

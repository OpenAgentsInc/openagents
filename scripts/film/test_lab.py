#!/usr/bin/env python3
"""Tests for Video Lab (scripts/film/lab.py): the store, patches, checks and
refusals, on a tiny cut made here with ffmpeg.

    python3 scripts/film/test_lab.py
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

TMP = Path(tempfile.mkdtemp(prefix="video-lab-test-"))
os.environ["VIDEO_LAB_ROOT"] = str(TMP / "lab")
sys.path.insert(0, str(Path(__file__).resolve().parent))
import lab  # noqa: E402

lab.AUTO_RENDER = False


def clip(name, seconds):
    path = TMP / f"{name}.mp4"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"testsrc2=size=320x180:rate=30:duration={seconds}",
                    "-pix_fmt", "yuv420p", str(path)], check=True)
    return str(path)


def tone(seconds):
    path = TMP / "music.wav"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"sine=frequency=220:duration={seconds}",
                    str(path)], check=True)
    return str(path)


class LabTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        a, b = clip("a", 6), clip("card", 8)
        Path(b).with_suffix(".beats.json").write_text(json.dumps([{"from": 3.0, "to": 5.0, "what": "the card"}]))
        cls.cut = {"fps": 30, "size": [320, 180], "bpm": 120, "music": tone(9), "out": str(TMP / "out.mp4"),
                   "shots": [{"name": "open", "src": a, "in": 0.0, "dur": 2.0},
                             {"name": "card", "src": b, "in": 0.0, "dur": 4.0},
                             {"name": "end", "src": a, "in": 2.0, "dur": 3.0}],
                   "captions": [{"text": "Over the card.", "at": 2.5, "dur": 1.0},
                                {"text": "At the end.", "at": 6.5, "dur": 1.0}]}
        lab.create("film", "cut.v1", "Test film", cls.cut, "First cut", "test")

    def edit(self, patch, summary="change", **kw):
        return lab.call("lab_edit", {"document": "film", "patch": json.dumps(patch), "summary": summary,
                                     "render": False, **kw})

    def test_a_cut_ending_inside_an_animation_is_found(self):
        problems, _ = lab.check("cut.v1", lab.body("film", 1))
        self.assertTrue(any("cuts the card off" in p for p in problems), problems)

    def test_an_edit_that_adds_a_problem_is_refused_in_plain_words(self):
        out = self.edit([{"op": "replace", "path": "/shots/@card/dur", "value": 4.25}])
        self.assertTrue(out["refused"])
        self.assertIn("off the 120 BPM beat grid", out["text"])
        out = self.edit([{"op": "replace", "path": "/shots/@card/dur", "value": 4.5},
                         {"op": "replace", "path": "/shots/@end/dur", "value": 2.5}])
        self.assertTrue(out["refused"], "still cutting the animation off, at another time, is refused")
        self.assertNotIn("would have fixed", out["text"])
        self.assertIn("Not saved", out["text"])
        out = self.edit([{"op": "add", "path": "/shots/@card/duration", "value": 5}])
        self.assertTrue(out["refused"])
        self.assertIn("did you mean dur", out["text"])
        out = self.edit([{"op": "replace", "path": "/shots/@card/dur", "value": 6.0}])
        self.assertTrue(out["refused"])
        self.assertIn("music is 9.00s", out["text"])
        with self.assertRaises(lab.Refused):
            self.edit([{"op": "replace", "path": "/shots/@nope/dur", "value": 5.0}])
        with self.assertRaises(lab.Refused):
            self.edit([{"op": "replace", "path": "/shots/@card/dur", "value": 5.0}], summary=" ")

    def test_a_good_edit_is_a_new_version_and_captions_follow_their_shots(self):
        before = len(lab.versions("film"))
        out = self.edit([{"op": "replace", "path": "/shots/@open/dur", "value": 1.0},
                         {"op": "replace", "path": "/shots/@card/dur", "value": 5.0}],
                        summary="The card plays to its end", version=1)
        self.assertNotIn("refused", out, out)
        v = lab.version("film")
        self.assertEqual(v["n"], before + 1)
        self.assertEqual(v["summary"], "The card plays to its end")
        self.assertEqual(v["parent"], 1)
        self.assertTrue(v["digest"].startswith("sha256:"))
        self.assertIn("Fixed: card: cuts the card off", out["text"])
        data = lab.body("film", v["n"])
        self.assertEqual([c["at"] for c in data["captions"]], [1.5, 6.5])
        self.assertEqual(lab.body("film", 1), self.cut, "an edit never changes an older version")

    def test_times_and_versions_are_read_from_what_the_chat_says(self):
        self.assertEqual(lab.seconds("1:26"), 86.0)
        self.assertEqual(lab.seconds("0:01:02.5"), 62.5)
        self.assertEqual(lab.seconds(12), 12.0)
        self.assertEqual(lab.seconds("card", self.cut), 2.0)
        with self.assertRaises(lab.Refused):
            lab.seconds("soon")
        self.assertEqual(lab.resolve("Test film"), "film")
        with self.assertRaises(lab.Refused):
            lab.resolve("episode 1")
        with self.assertRaises(lab.Refused):
            lab.version("film", 99)

    def test_private_assets_are_marked(self):
        self.assertEqual(lab.private("cut.v1", self.cut), [])
        meta = {"shots": [{"src": "~/work/film-out/draconia/metahuman/latest/face.mp4"}],
                "captions": [{"text": "Berkeley is a city"}]}
        why = lab.private("cut.v1", meta)
        self.assertTrue(any("MetaHuman" in w for w in why), why)
        self.assertFalse(any("Berkeley Mono" in w for w in why), "caption words are not assets")
        self.assertTrue(lab.private("cut.v1", {"src": "~/work/film-out/segments/delegation.mp4"}))

    def test_frames_draw_from_the_sources_before_a_render(self):
        out = lab.call("lab_frame", {"document": "film", "at": "0:03", "version": "1"})
        self.assertTrue(Path(out["image"]).exists())
        self.assertIn("drawn from the sources", out["text"])

    def test_every_tool_has_a_declaration(self):
        names = {s["name"] for s in lab.tool_specs()}
        self.assertEqual(names, set(lab.TOOLS))
        for s in lab.tool_specs():
            self.assertEqual(s["parameters"]["type"], "object")


if __name__ == "__main__":
    unittest.main()

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


def talking(name, w=180, h=320, rotate=None):
    """A clip with "speech": three tone bursts with pauses between, 6 s."""
    path = TMP / f"{name}.mp4"
    tone_expr = "0.5*sin(2*PI*300*t)*(lt(t,1)+between(t,1.5,3)+between(t,3.6,5))"
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"testsrc2=size={w}x{h}:rate=30:duration=6",
                    "-f", "lavfi", "-i", f"aevalsrc='{tone_expr}':s=48000:d=6", "-pix_fmt", "yuv420p",
                    "-c:a", "aac", "-shortest", str(path)], check=True)
    if rotate is not None:
        turned = TMP / f"{name}-turned.mp4"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-display_rotation:v:0", str(rotate), "-i", str(path),
                        "-c", "copy", str(turned)], check=True)
        return turned
    return path


# What the transcriber would hear in the talking clip (whisper.cpp isn't needed for the tests).
HEARD = [{"w": "Well,", "from": 0.05, "to": 0.4}, {"w": "folks.", "from": 0.4, "to": 0.95},
         {"w": "This", "from": 1.55, "to": 1.9}, {"w": "is", "from": 1.9, "to": 2.1},
         {"w": "CODR", "from": 2.1, "to": 2.5}, {"w": "from", "from": 2.5, "to": 2.7},
         {"w": "open", "from": 2.7, "to": 2.85}, {"w": "agents.", "from": 2.85, "to": 2.98},
         {"w": "Let's", "from": 3.65, "to": 4.0}, {"w": "try", "from": 4.0, "to": 4.5}, {"w": "it.", "from": 4.5, "to": 4.95}]


class SpliceTest(unittest.TestCase):
    """A clip with its own speech, brought in and spliced into a cut (#11617)."""

    @classmethod
    def setUpClass(cls):
        lab.whisper_words = lambda wav, log=None: [dict(w) for w in HEARD]
        cls.clip = talking("whiteboard")
        a = clip("wide", 12)
        cls.cut = {"fps": 30, "size": [320, 180], "bpm": 120, "music": tone(30), "out": str(TMP / "talk.mp4"),
                   "shots": [{"name": "open", "src": a, "in": 0.0, "dur": 2.0},
                             {"name": "after", "src": a, "in": 2.0, "dur": 3.0}],
                   "captions": [{"text": "Over the after shot.", "at": 2.5, "dur": 1.0}]}
        lab.create("talk", "cut.v1", "Talk film", cls.cut, "First cut", "test")

    def test_import_measures_pauses_and_fixes_names(self):
        info = lab.import_source(str(self.clip))
        self.assertEqual((info["w"], info["h"]), (180, 320))
        self.assertTrue(info["audio"])
        sp = lab.speech(info["id"])
        gaps = [(q["from"], q["to"]) for q in sp["quiet"]]
        for a, b in ((1.0, 1.5), (3.0, 3.6), (5.0, 6.0)):
            self.assertTrue(any(x <= a + 0.1 and y >= b - 0.1 for x, y in gaps), (a, b, gaps))
        said = " ".join(w["w"] for w in sp["words"])
        self.assertIn("This is Coder from OpenAgents.", said)
        self.assertTrue(Path(info["strip"]).exists() and Path(info["poster"]).exists())
        self.assertEqual(lab.resolve_source("whiteboard"), info["id"])
        out = lab.call("lab_sources", {"source": "whiteboard"})
        self.assertIn("This is Coder from OpenAgents.", out["text"])
        self.assertIn("Pauses where a cut can go", out["text"])

    def test_a_turned_phone_clip_reads_as_it_plays(self):
        p = lab.cutmod().probe(talking("turned", 320, 180, rotate=90))
        self.assertEqual((p["w"], p["h"]), (180, 320))
        self.assertEqual(abs(p["rotation"]), 90)

    def test_ranges_snap_to_pauses_on_whole_beats(self):
        sp = {"duration": 6.0, "quiet": [{"from": 1.0, "to": 1.5}, {"from": 3.0, "to": 3.6}, {"from": 5.0, "to": 6.0}]}
        a, b = lab.snap_range(sp, 1.2, 3.2, 0.5)
        self.assertTrue(1.0 < a < 1.5 and 3.0 < b < 3.6, (a, b))
        self.assertAlmostEqual((b - a) / 0.5, round((b - a) / 0.5))
        self.assertIsNone(lab.snap_range({"duration": 6.0, "quiet": []}, 1.2, 3.2, 0.5))

    def test_captions_come_from_what_is_said(self):
        sp = {"words": [dict(w) for w in HEARD]}
        sp["words"] = lab.fix_words(sp["words"], {"CODR": "Coder", "open agents": "OpenAgents"})
        caps = lab.speech_captions(sp, 1.25, 3.25, 10.0, "wb", 12.0)
        self.assertEqual([c["text"] for c in caps], ["This is Coder from OpenAgents."])
        self.assertAlmostEqual(caps[0]["at"], 10.3, places=2)
        self.assertLessEqual(caps[0]["at"] + caps[0]["dur"], 12.0)

    def test_word_times_settle_out_of_pauses_and_belong_to_one_side(self):
        quiet = [{"from": 19.11, "to": 19.49}, {"from": 0.01, "to": 0.31}]
        words = lab.settle([{"w": "this.", "from": 18.97, "to": 19.44}, {"w": "Well,", "from": 0.0, "to": 0.32}],
                           quiet)
        self.assertEqual(words[0]["to"], 19.11, "a word stretched into the pause after it ends where the voice does")
        self.assertEqual(words[1]["from"], 0.31, "a word stretched back over the pause before it starts after it")
        self.assertTrue(lab.in_range(words[0], 0.25, 19.25))
        self.assertFalse(lab.in_range(words[0], 19.35, 42.85), "a word is captioned on one side of a cut only")
        self.assertTrue(lab.in_range(words[1], 0.25, 19.25))

    def test_splice_inserts_with_its_sound_captions_and_framing(self):
        lab.import_source(str(self.clip))
        before = lab.body("talk", lab.version("talk")["n"])
        out = lab.call("lab_splice", {"document": "talk", "source": "whiteboard", "from": "0.9", "to": "3.2",
                                      "after": "open", "name": "wb", "render": False})
        self.assertNotIn("refused", out, out["text"])
        data = lab.body("talk", out["version"])
        shot = data["shots"][1]
        self.assertEqual(shot["name"], "wb")
        self.assertEqual(shot["sound"], {"gain": 1.0})
        self.assertEqual(shot["fit"], "blur", "a portrait clip sits over a blurred copy of itself")
        self.assertTrue(1.0 < shot["in"] < 1.5 and 3.0 < shot["in"] + shot["dur"] < 3.6, shot)
        self.assertEqual(shot["dur"] % 0.5, 0)
        mine = [c for c in data["captions"] if c.get("shot") == "wb"]
        self.assertEqual([c["text"] for c in mine], ["This is Coder from OpenAgents."])
        moved = next(c for c in data["captions"] if c["text"] == "Over the after shot.")
        old = next(c for c in before["captions"] if c["text"] == "Over the after shot.")
        self.assertAlmostEqual(moved["at"], old["at"] + shot["dur"])
        self.assertIn("in pauses", out["text"])

        # A cut in the middle of a word, or a caption that isn't what's said, is refused.
        out2 = lab.call("lab_edit", {"document": "talk", "summary": "start mid-word", "render": False,
                                     "patch": json.dumps([{"op": "replace", "path": "/shots/@wb/in", "value": 1.75}])})
        self.assertTrue(out2.get("refused"), out2)
        self.assertIn("mid-speech", out2["text"])
        i = next(k for k, c in enumerate(data["captions"]) if c.get("shot") == "wb")
        out3 = lab.call("lab_edit", {"document": "talk", "summary": "wrong words", "render": False,
                                     "patch": json.dumps([{"op": "replace", "path": f"/captions/{i}/text",
                                                           "value": "Something nobody said at all here."}])})
        self.assertTrue(out3.get("refused"), out3)
        self.assertIn("doesn't match what's said", out3["text"])

        # Framing stills for both choices.
        fr = lab.call("lab_framing", {"source": "whiteboard", "at": "2"})
        self.assertTrue(Path(fr["image"]).exists())

    def test_removing_a_shot_takes_its_captions_and_music_hit(self):
        lab.create("trim", "cut.v1", "Trim film", {**self.cut, "music": tone(30),
                                                   "score": {"cuts_at": ["after"]}}, "First", "test")
        data = lab.body("trim", 1)
        data.pop("score")
        out = lab.call("lab_edit", {"document": "trim", "summary": "drop after", "render": False,
                                    "patch": json.dumps([{"op": "remove", "path": "/shots/@after"}])})
        self.assertNotIn("refused", out, out.get("text"))
        new = lab.body("trim", out["version"])
        self.assertEqual([s["name"] for s in new["shots"]], ["open"])
        self.assertEqual(new["captions"], [])
        self.assertEqual(new["score"]["cuts_at"], [])
        self.assertIn("captions over the removed shots went with them", out["text"])

    def test_a_spliced_cut_renders_with_the_clips_sound_under_ducked_music(self):
        lab.import_source(str(self.clip))
        cut = {"fps": 30, "size": [320, 180], "bpm": 120, "music": tone(30), "out": str(TMP / "spliced.mp4"),
               "cache": str(TMP / "cache"), "sheets": str(TMP / "sheets"),
               "shots": [{"name": "open", "src": self.cut["shots"][0]["src"], "in": 0.0, "dur": 1.0},
                         {"name": "wb", "src": str(self.clip), "in": 1.25, "dur": 2.0, "sound": {"gain": 1.0},
                          "fit": "blur"},
                         {"name": "crop", "src": str(self.clip), "in": 3.3, "dur": 1.0, "fit": "crop"}]}
        spec = TMP / "spliced.json"
        spec.write_text(json.dumps(cut))
        r = subprocess.run([sys.executable, str(lab.FILM / "cut.py"), "render", str(spec), "--force"],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr[-1500:])
        probe = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=codec_type:format=duration",
                                "-of", "json", str(TMP / "spliced.mp4")], capture_output=True, text=True).stdout
        j = json.loads(probe)
        self.assertEqual(sorted(s["codec_type"] for s in j["streams"]), ["audio", "video"])
        self.assertAlmostEqual(float(j["format"]["duration"]), 4.0, delta=0.1)
        expr = lab.cutmod().duck_expr([(None, 1.0, 2.0)], 0.2)
        self.assertIn("1-0.800*", expr)


if __name__ == "__main__":
    unittest.main()

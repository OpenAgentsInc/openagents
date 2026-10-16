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

TMP = Path(tempfile.mkdtemp(prefix="video-lab-test-")).resolve()
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

    def test_a_silent_source_renders_without_a_music_track(self):
        cut = {"fps": 30, "size": [320, 180], "out": str(TMP / "silent.mp4"),
               "shots": [{"name": "silent", "src": self.cut["shots"][0]["src"], "dur": 2.0}]}
        self.assertEqual(lab.check("cut.v1", cut)[0], [])
        lab.create("silent", "cut.v1", "Silent clip", cut, "First", "test")
        with (TMP / "silent.log").open("w") as log:
            rendered = lab.do_render("silent", 1, "preview", log)
        self.assertTrue(rendered.is_file())
        self.assertAlmostEqual(lab.cutmod().probe(rendered)["dur"], 2.0, delta=0.1)

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

    def test_timeline_saves_links_moves_a_group_and_refuses_stale_saves(self):
        lab.create("editor", "cut.v1", "Editor film", self.cut, "First", "test")
        first = lab.editor_view("editor", 1)
        self.assertEqual(first["version"], 1)
        self.assertTrue(any(c["id"] == "video:card" for c in first["clips"]))
        out = lab.editor_edit({"doc": "editor", "version": 1, "render": False,
                              "command": {"op": "link", "id": "text:0", "shot": "card"}})
        self.assertNotIn("refused", out, out)
        with self.assertRaises(lab.Refused):
            lab.editor_edit({"doc": "editor", "version": 1, "render": False,
                             "command": {"op": "move", "id": "video:card", "at": 0}})
        out = lab.editor_edit({"doc": "editor", "version": 2, "render": False,
                              "command": {"op": "move", "id": "video:card", "at": 0}})
        self.assertNotIn("refused", out, out)
        saved = lab.body("editor", out["version"])
        self.assertEqual(saved["shots"][0]["name"], "card")
        self.assertEqual(saved["captions"][0]["at"], 0.5)
        self.assertEqual(lab.body("editor", 1), self.cut)
        image = lab.editor_asset("editor", out["version"], "video:card")
        self.assertTrue(image.is_file())

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

    def test_scene_digest_tracks_the_event_log_and_excludes_the_output(self):
        log = TMP / "events.jsonl"
        rendered = TMP / "scene-render.mp4"
        log.write_text("first event\n")
        rendered.write_text("first render")
        scene = {"renderer": "verse-film-shot", "shot": {"scene": {"kind": "layer_tree", "events": str(log)}}, "out": str(rendered)}
        before, files = lab.digest("scene.v1", scene)
        self.assertIn(str(log), files)
        self.assertNotIn(str(rendered), files)
        rendered.write_text("another render")
        self.assertEqual(before, lab.digest("scene.v1", scene)[0])
        log.write_text("changed event, different length\n")
        self.assertNotEqual(before, lab.digest("scene.v1", scene)[0])
        relative = {"shot": {"scene": {"events": "crates/layertree-tee-lab/fixtures/events/regtest.jsonl"}}}
        self.assertIn("crates/layertree-tee-lab/fixtures/events/regtest.jsonl", lab.digest("scene.v1", relative)[1])

    def test_a_scene_refuses_to_render_changed_version_inputs(self):
        log = TMP / "changed-version-events.jsonl"
        log.write_text("original events\n")
        scene = {"renderer": "verse-film-shot", "shot": {"scene": {"kind": "layer_tree", "events": str(log)}}}
        lab.create("changed-scene", "scene.v1", "Scene", scene, "Original", "test")
        log.write_text("changed events, different length\n")
        with self.assertRaisesRegex(lab.Refused, "Save a new scene version"):
            lab.do_render("changed-scene", 1, "review", sys.stderr)

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
        # The video kind's manifest (crates/video-lab) declares the same tools.
        kind = json.loads((Path(__file__).resolve().parents[2] / "crates/video-lab/tools.json").read_text())
        self.assertEqual(kind, lab.tool_specs(),
                         "regenerate crates/video-lab/tools.json from lab.tool_specs() after changing a tool")

    def test_a_shot_that_pauses_is_refused_and_ending_the_pause_is_saved(self):
        lab.create("still", "cut.v1", "Still film", self.cut, "First", "test")
        out = lab.call("lab_edit", {"document": "still", "summary": "hold the end", "render": False,
                                    "patch": json.dumps([{"op": "add", "path": "/shots/@end/play", "value": 2.0}])})
        self.assertTrue(out["refused"], out["text"])
        self.assertIn("holds a still frame for 1s", out["text"])
        held = {**self.cut, "shots": [dict(s) for s in self.cut["shots"]]}
        held["shots"][2]["play"] = 2.0
        lab.create("held", "cut.v1", "Held film", held, "First", "test")
        out = lab.call("lab_edit", {"document": "held", "summary": "no hold", "render": False,
                                    "patch": json.dumps([{"op": "remove", "path": "/shots/@end/play"}])})
        self.assertNotIn("refused", out, out.get("text"))
        self.assertIn("Fixed: end: holds a still frame", out["text"])
        ok = {**held, "shots": [dict(s) for s in held["shots"]]}
        ok["shots"][2]["still_ok"] = 1.0
        self.assertFalse([p for p in lab.check("cut.v1", ok)[0] if "still frame" in p], "still_ok allows a hold")

    def test_a_clip_that_freezes_mid_shot_is_found_before_and_after_the_render(self):
        frozen = TMP / "frozen.mp4"  # moves 0-2 s, still 2-3.5 s, moves again 3.5-5 s
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30:duration=2",
                        "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30:duration=1.5",
                        "-filter_complex", "[0:v]tpad=stop_mode=clone:stop_duration=1.5[a];[a][1:v]concat=n=2:v=1",
                        "-pix_fmt", "yuv420p", str(frozen)], check=True)
        cut = {**self.cut, "shots": [{"name": "moves", "src": str(frozen), "in": 0.0, "dur": 2.0},
                                     {"name": "stops", "src": str(frozen), "in": 2.0, "dur": 3.0}],
               "captions": []}
        problems, _ = lab.check("cut.v1", cut)
        self.assertTrue(any(p.startswith("stops: the picture is still from 2.") for p in problems), problems)
        self.assertFalse(any(p.startswith("moves:") for p in problems), problems)
        found = lab.render_pauses(frozen, cut)
        self.assertEqual([p["shot"] for p in found], ["stops"])
        self.assertAlmostEqual(found[0]["from"], 2.0, delta=0.1)
        self.assertAlmostEqual(found[0]["to"], 3.5, delta=0.1)
        self.assertIn("pauses mid-shot 1 times", lab.pauses_text(found))
        cut["shots"][1]["still_ok"] = 2.0
        self.assertEqual(lab.check("cut.v1", cut)[0], [], "The schema accepts the renderer's existing stillness limit")
        self.assertEqual(lab.render_pauses(frozen, cut), [])
        cut["shots"][1]["still_ok"] = -1
        self.assertTrue(lab.check("cut.v1", cut)[0], "A negative allowance is invalid")

    def test_a_preview_an_edit_queued_is_skipped_once_a_later_edit_replaces_it(self):
        lab.create("queue", "cut.v1", "Queue film", self.cut, "First", "test")
        lab.save_version("queue", "cut.v1", self.cut, "Second", "test", parent=1)
        old = {"doc": "queue", "version": 1, "quality": "preview", "edit": True}
        self.assertTrue(lab.superseded(old))
        self.assertFalse(lab.superseded({**old, "version": 2}), "the latest version still renders")
        self.assertFalse(lab.superseded({**old, "edit": False}), "a look at an old version still renders")
        self.assertFalse(lab.superseded({**old, "quality": "review"}), "an asked-for render always runs")

    def test_a_voiceover_clip_goes_over_its_lines_and_their_captions_follow_it(self):
        voice = TMP / "290-test-voice.wav"  # 2.4 s
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=330:duration=2.4",
                        str(voice)], check=True)
        lab.create("voiced", "cut.v1", "Voiced film", {**self.cut, "captions": []}, "First", "test")
        lab.call("lab_lines", {"document": "voiced", "lines": "One two three. Four five six seven eight.",
                               "at": "0.5", "render": False})
        heard = [{"w": "One", "from": 0.1, "to": 0.3}, {"w": "two", "from": 0.3, "to": 0.5},
                 {"w": "three.", "from": 0.5, "to": 0.8}, {"w": "Four", "from": 1.2, "to": 1.4},
                 {"w": "five", "from": 1.4, "to": 1.6}, {"w": "six", "from": 1.6, "to": 1.8},
                 {"w": "seven", "from": 1.8, "to": 2.0}, {"w": "eight.", "from": 2.0, "to": 2.3}]
        real = lab.whisper_words
        lab.whisper_words = lambda wav, log=None: [dict(w) for w in heard]
        try:
            out = lab.call("lab_voice", {"document": "voiced", "clip": str(voice), "at": "0.5", "render": False})
        finally:
            lab.whisper_words = real
        self.assertNotIn("refused", out, out.get("text"))
        new = lab.body("voiced", out["version"])
        self.assertEqual(new["voices"], [{"src": lab.tilde(voice), "at": 0.5}])
        caps = new["captions"]
        self.assertAlmostEqual(caps[0]["at"], 0.6, places=2, msg="the first line when it's said")
        self.assertAlmostEqual(caps[1]["at"], 1.7, places=2, msg="the second line when 'Four' is said")
        self.assertIn("re-timed to when each line is said", out["text"])
        self.assertIn("shorter by", out["text"])
        problems, _ = lab.check("cut.v1", new)
        self.assertFalse([p for p in problems if "voiceover" in p], problems)
        with self.assertRaises(lab.Refused):  # over the voice already there
            lab.call("lab_voice", {"document": "voiced", "clip": str(voice), "at": "1.0", "render": False})
        with self.assertRaises(lab.Refused):  # past the end
            lab.call("lab_voice", {"document": "voiced", "clip": str(voice), "at": "8.0", "render": False})
        talk = {**self.cut, "captions": [], "shots": [dict(s) for s in self.cut["shots"]]}
        talk["shots"][1]["sound"] = {"gain": 1.0}
        lab.create("voiced2", "cut.v1", "Voiced talk", talk, "First", "test")
        with self.assertRaises(lab.Refused) as e:  # into the shot where someone speaks
            lab.call("lab_voice", {"document": "voiced2", "clip": str(voice), "at": "0.5", "render": False})
        self.assertIn("more picture", str(e.exception))
        # spread over the clip when there's no transcript
        self.assertEqual(lab.said_at(["ab", "cd"], None, 2.0)[1][0], 1.0)

    def test_a_cut_with_voiceover_clips_renders_with_them_mixed_in(self):
        voice = TMP / "290-test-voice2.wav"
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=1.0",
                        str(voice)], check=True)
        music = TMP / "voiced-music.wav"  # its own file: tone() rewrites the shared one
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=220:duration=8",
                        str(music)], check=True)
        cut = {"fps": 30, "size": [320, 180], "bpm": 120, "music": str(music), "out": str(TMP / "voiced.mp4"),
               "cache": str(TMP / "cache"), "sheets": str(TMP / "sheets"),
               "shots": [{"name": "open", "src": self.cut["shots"][0]["src"], "in": 0.0, "dur": 2.0},
                         {"name": "end", "src": self.cut["shots"][2]["src"], "in": 2.0, "dur": 2.0}],
               "voices": [{"src": str(voice), "at": 0.5}, {"src": str(voice), "at": 2.5, "gain": 0.8}]}
        spec = TMP / "voiced.json"
        spec.write_text(json.dumps(cut))
        r = subprocess.run([sys.executable, str(lab.FILM / "cut.py"), "render", str(spec), "--force"],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr[-1500:])
        loud = lab.cutmod().mean_db(TMP / "voiced.mp4", 0.6, 0.8)
        quiet = lab.cutmod().mean_db(TMP / "voiced.mp4", 1.8, 0.4)
        self.assertGreater(loud, quiet, "the voice is in the mix over ducked music")

    def test_new_copies_a_version_and_leaves_the_old_document_alone(self):
        before = len(lab.versions("film"))
        out = lab.call("lab_new", {"name": "Film take two", "copy": "film", "version": "1", "render": False})
        self.assertEqual(out["version"], 1)
        self.assertEqual(lab.body("film-take-two", 1), lab.body("film", 1))
        self.assertEqual(lab.doc_meta("film-take-two")["title"], "Film take two")
        self.assertEqual(len(lab.versions("film")), before, "the old document gets no new version")
        with self.assertRaises(lab.Refused):
            lab.call("lab_new", {"name": "film-take-two", "copy": "film", "render": False})
        with self.assertRaises(lab.Refused):
            lab.call("lab_new", {"name": "x", "copy": "nope", "render": False})

    def test_voiceover_lines_become_captions_at_a_speaking_pace(self):
        caps = lab.vo_captions("Coder 1.0 is out today. On its own, a fix cost $4.00, took 284 seconds, and "
                               "solved the same number of the twenty-one issues we measured on the same model.",
                               10.0, 150)
        self.assertEqual(caps[0]["text"], "Coder 1.0 is out today.")
        self.assertEqual(caps[0]["at"], 10.0)
        self.assertTrue(all(c["vo"] for c in caps))
        self.assertTrue(all(len(c["text"]) <= 92 for c in caps), caps)
        self.assertGreater(len(caps), 2, "a long sentence splits at a comma")
        for a, b in zip(caps, caps[1:]):
            self.assertGreater(b["at"], a["at"] + a["dur"] - 1e-6, "in order, never overlapping")
        # Pacing itself is tested in crates/video-lab (edit::spoken_words): numbers take longer to say.
        dur = lambda text: lab.vo_captions(text, 0.0, 30)[0]["dur"]
        self.assertGreater(dur("It cost $4.00 today"), dur("It cost four today"), "numbers take longer to say")
        self.assertAlmostEqual(caps[0]["dur"], (1 + 2.2 + 1 + 1 + 1) * 60 / 150, places=2)  # "1.0" reads as two digits
        lab.create("vo", "cut.v1", "VO film", {**self.cut, "captions": []}, "First", "test")
        out = lab.call("lab_lines", {"document": "vo", "lines": "Some things always wait for you.",
                                     "at": "end", "render": False})
        self.assertNotIn("refused", out, out.get("text"))
        caps = lab.body("vo", out["version"])["captions"]
        self.assertEqual(caps[0]["text"], "Some things always wait for you.")
        self.assertAlmostEqual(caps[0]["at"], 6.4, places=2, msg="a shot's name starts just after its start")
        with self.assertRaises(lab.Refused):  # past the end
            lab.call("lab_lines", {"document": "vo", "lines": "word " * 60, "at": "open", "render": False})
        with self.assertRaises(lab.Refused):  # over the caption just placed
            lab.call("lab_lines", {"document": "vo", "lines": "Again.", "at": "6.5", "render": False})
        talk = {**self.cut, "captions": [], "shots": [dict(s) for s in self.cut["shots"]]}
        talk["shots"][1]["sound"] = {"gain": 1.0}
        lab.create("vo2", "cut.v1", "VO talk film", talk, "First", "test")
        with self.assertRaises(lab.Refused) as e:  # into the shot where someone speaks
            lab.call("lab_lines", {"document": "vo2", "lines": "One two three four five six seven.", "at": "1.0",
                                   "render": False})
        self.assertIn("where someone is speaking", str(e.exception))

    def test_keep_captions_removes_every_caption_but_those_over_the_named_or_speaking_shots(self):
        lab.create("caps", "cut.v1", "Caption film", self.cut, "First", "test")
        out = lab.call("lab_keep_captions", {"document": "caps", "over": "card", "render": False})
        self.assertNotIn("refused", out, out.get("text"))
        new = lab.body("caps", out["version"])
        self.assertEqual([c["text"] for c in new["captions"]], ["Over the card."])
        self.assertEqual(new["shots"], self.cut["shots"], "only captions change")
        self.assertIn("Removed 1 captions", out["text"])
        again = lab.call("lab_keep_captions", {"document": "caps", "over": "card", "render": False})
        self.assertNotIn("version", again, "nothing to remove is no new version")
        with self.assertRaises(lab.Refused):
            lab.call("lab_keep_captions", {"document": "caps", "over": "nope", "render": False})
        with self.assertRaises(lab.Refused):
            lab.call("lab_keep_captions", {"document": "caps", "over": "speech", "render": False})
        talk = {**self.cut, "shots": [dict(s) for s in self.cut["shots"]]}
        talk["shots"][2]["sound"] = {"gain": 1.0}
        lab.create("caps2", "cut.v1", "Speaking film", talk, "First", "test")
        out = lab.call("lab_keep_captions", {"document": "caps2", "over": "speech", "render": False})
        self.assertNotIn("refused", out, out.get("text"))
        self.assertEqual([c["text"] for c in lab.body("caps2", out["version"])["captions"]], ["At the end."])
        self.assertIn("lab_keep_captions", {t["name"] for t in lab.tool_specs()})


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

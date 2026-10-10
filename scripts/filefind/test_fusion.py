"""Tests for the frozen Clef fusion (#11217, #11220).

    python3 -m unittest scripts/filefind/test_fusion.py
"""

import importlib.util
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import filefind as ff  # noqa: E402

CARD = {"kind": ff.FUSION_KIND, "k": 3, "door": "http://127.0.0.1:9/v1/systemone", "model": "clef-flash",
        "question": "Is this file relevant to solving the issue?", "artifact_digest": "sha256:a",
        "head_digest": "sha256:h", "workers": 2}


class FakeDoor:
    def __init__(self, ps, fail=()):
        self.ps, self.fail = ps, set(fail)

    def ask(self, state, timeout):
        path = state.split("FILE: ", 1)[1].split("\n", 1)[0]
        if path in self.fail:
            raise RuntimeError("answered by vertex, not the card's Clef")
        return self.ps[path]


def repo_with(files):
    d = tempfile.mkdtemp()
    subprocess.run(["git", "init", "-q", d], check=True)
    for name, text in files.items():
        (Path(d) / name).write_text(text)
    subprocess.run(["git", "-C", d, "add", "-A"], check=True)
    subprocess.run(["git", "-C", d, "-c", "user.email=t@e", "-c", "user.name=t", "commit", "-qm", "x"], check=True)
    return d, ff.ls_tree(d, "HEAD")


class Fusion(unittest.TestCase):
    def test_state_matches_the_corpus(self):
        spec = importlib.util.spec_from_file_location("frc", HERE.parent / "bench" / "file-relevance-corpus.py")
        mod = importlib.util.module_from_spec(spec)
        argv, sys.argv = sys.argv, ["x"]
        spec.loader.exec_module(mod)
        sys.argv = argv
        body = "b" * 3000
        self.assertEqual(ff.clef_state(7, " T ", body, "a.rs", "fn x() {}"),
                         mod.state_text(7, " T ", body, "a.rs", "fn x() {}"))
        self.assertEqual((ff.CLEF_HEAD_BYTES, ff.CLEF_ISSUE_CHARS), (mod.HEAD_BYTES, mod.ISSUE_CHARS))

    def test_logits_add_inside_the_top_k_only(self):
        repo, tree = repo_with({"a.rs": "a", "b.rs": "b", "c.rs": "c", "d.rs": "d"})
        ranked = [("a.rs", 0.6), ("b.rs", 0.5), ("c.rs", 0.4), ("d.rs", 0.3)]
        door = FakeDoor({"a.rs": 0.1, "b.rs": 0.2, "c.rs": 0.9})
        fused, info = ff.clef_fuse(CARD, ranked, 1, "t", "b", tree, repo, door=door, budget=10)
        self.assertTrue(info["fused"])
        self.assertEqual([p for p, _ in fused], ["c.rs", "b.rs", "a.rs", "d.rs"])
        z = math.log(0.4 / 0.6) + math.log(0.9 / 0.1)
        self.assertAlmostEqual(fused[0][1], 1 / (1 + math.exp(-z)), places=6)
        self.assertEqual(fused[3], ("d.rs", 0.3))

    def test_any_miss_keeps_the_numpy_order(self):
        repo, tree = repo_with({"a.rs": "a", "b.rs": "b", "c.rs": "c"})
        ranked = [("a.rs", 0.6), ("b.rs", 0.5), ("c.rs", 0.4)]
        door = FakeDoor({"a.rs": 0.1, "b.rs": 0.2, "c.rs": 0.9}, fail={"b.rs"})
        fused, info = ff.clef_fuse(CARD, ranked, 1, "t", "b", tree, repo, door=door, budget=10)
        self.assertFalse(info["fused"])
        self.assertIn("vertex", info["why"])
        self.assertEqual(fused, ranked)

    def test_an_unreachable_door_keeps_the_numpy_order(self):
        repo, tree = repo_with({"a.rs": "a"})
        ranked = [("a.rs", 0.6)]
        fused, info = ff.clef_fuse(CARD, ranked, 1, "t", "b", tree, repo, budget=2)
        self.assertFalse(info["fused"])
        self.assertEqual(fused, ranked)


if __name__ == "__main__":
    unittest.main()

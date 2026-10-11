"""Tests for filefind's index-time file cards (#11249): routes are a bounded field,
the card store keys cards by blob and vocabulary, card features are lookups, and the
distilled tower fits outcomes with Clef as a separate teacher field.

    python3 -m unittest scripts/filefind/test_cards.py
"""

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import cards as fc  # noqa: E402
import filefind as ff  # noqa: E402


VOCAB = {
    "schema": fc.VOCAB_SCHEMA, "rev": "0" * 40,
    "areas": [{"id": "billing", "definition": "credits"}, {"id": "chat", "definition": "conversations"}],
    "entities": [{"id": "account", "definition": ""}, {"id": "message", "definition": ""}],
    "layers": [{"id": k, "definition": v} for k, v in fc.LAYERS.items()],
    "clients": [{"id": k, "definition": v} for k, v in fc.CLIENTS.items()],
    "routes": ["/v1/credits", "/v1/chat/messages"],
}
VOCAB["digest"] = fc.vocab_digest(VOCAB)


class Routes(unittest.TestCase):
    def test_quoted_url_paths_only(self):
        text = ('let r = Router::new().route("/v1/credits/{id}", get(h)); // see /usr/lib/x\n'
                'fetch(`/v1/chat/messages?limit=3`); import x from "./lib/types"; "/docs/a.md" "/single"')
        self.assertEqual(fc.file_routes(text), {"/v1/credits/{}", "/v1/chat/messages"})

    def test_ids_normalize(self):
        self.assertEqual(fc.norm_route("/v1/accounts/1234/keys"), "/v1/accounts/{}")
        self.assertIsNone(fc.norm_route("/usr/local/bin"))
        self.assertIsNone(fc.norm_route("/github.com/x"))


class Store(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cache = self.tmp.name
        os.makedirs(os.path.join(self.cache, "cards"))
        with open(os.path.join(self.cache, "cards", "vocab.json"), "w") as f:
            json.dump(VOCAB, f)

    def tearDown(self):
        self.tmp.cleanup()

    def write_cards(self, cards):
        with open(os.path.join(self.cache, "cards", "cards.jsonl"), "w") as f:
            for c in cards:
                f.write(json.dumps(c) + "\n")

    def card(self, blob, path, vocab=None, **tags):
        c = {"v": fc.CARD_SCHEMA, "blob": blob, "path": path, "vocab": vocab or VOCAB["digest"],
             "summary": "", "areas": [], "layers": [], "clients": [], "entities": [], "routes": [], "model": "m"}
        c.update(tags)
        return c

    def test_cards_of_another_vocabulary_are_not_read(self):
        self.write_cards([self.card("a" * 40, "x.rs"), self.card("b" * 40, "y.rs", vocab="sha256:old")])
        st = fc.CardStore(self.cache, key=False)
        self.assertEqual(set(st.load_cards()), {"a" * 40})
        self.assertEqual(set(st.missing({"a" * 40: "x.rs", "b" * 40: "y.rs"})), {"b" * 40})

    def test_features_are_lookups(self):
        a, b, c = "a" * 40, "b" * 40, "c" * 40
        self.write_cards([self.card(a, "bill.rs", areas=["billing"], entities=["account"], layers=["store"],
                                    clients=["worker"], routes=["/v1/credits"]),
                          self.card(b, "chat.rs", areas=["chat"], layers=["ui"], clients=["web"])])
        st = fc.CardStore(self.cache, key=False)
        st.compile_tags()
        st.load_tags()
        st.rows = {a: 0, b: 1}
        st.vecs = np.array([[1, 0], [0, 1]], np.float16)

        class Ix:
            blob_rows = {a.encode(): 0, b.encode(): 1}
            blob_mat = np.zeros((2, 2), np.float16)
        prof = {"areas": [0.9, 0.1], "entities": [0.7, 0.3], "layers": [0.0] * len(fc.LAYERS),
                "clients": [0.0] * len(fc.CLIENTS), "routes": ["/v1/credits"]}
        prof["layers"][list(fc.LAYERS).index("store")] = 0.8
        prof["clients"][list(fc.CLIENTS).index("worker")] = 0.6
        paths = ["bill.rs", "chat.rs", "new.rs"]
        tree = {"bill.rs": a, "chat.rs": b, "new.rs": c}
        cv = fc.CardView(st, Ix, paths, tree, np.array([1.0, 0.0]), prof)
        f0, f1, f2 = cv.feats(0), cv.feats(1), cv.feats(2)
        self.assertAlmostEqual(f0["card_cos"], 1.0, places=3)
        self.assertAlmostEqual(f0["card_area"], 0.9, places=5)
        self.assertAlmostEqual(f0["card_entity"], 0.7, places=5)
        self.assertAlmostEqual(f0["card_layer"], 0.8, places=5)
        self.assertAlmostEqual(f0["card_client"], 0.6, places=5)
        self.assertEqual(f0["card_route"], 1.0)
        self.assertAlmostEqual(f1["card_area"], 0.1, places=5)
        self.assertEqual(f2["card_has"], 0.0)  # a blob carded later is not invented
        self.assertEqual(cv.top("tag", 1), [0])


class Tower(unittest.TestCase):
    def test_fits_outcomes_and_the_teacher_is_not_a_label(self):
        rng = np.random.default_rng(0)
        n, dq, dd = 3000, 6, 5
        Q = rng.normal(size=(n, dq)).astype(np.float32)
        D = rng.normal(size=(n, dd)).astype(np.float32)
        true = (Q[:, 0] * D[:, 0] + Q[:, 1] * D[:, 1]) * 2
        y = (true > 0.5).astype(np.float32)
        teacher = np.where(rng.random(n) < 0.5, true, np.nan).astype(np.float32)
        groups = np.arange(n) // 10
        t = fc.train_tower(Q, D, y, teacher, groups, k=4, epochs=40, log=False)
        sk = np.zeros((n, 3), np.float32)
        s = ((Q @ np.asarray(t["Wq"], np.float32)) * fc.tower_project(t, D)).sum(1) + t["b"]
        self.assertAlmostEqual(float(fc.tower_score(t, Q[0], fc.tower_project(t, D[:1]), sk[:1])[0]), float(s[0]),
                               places=4)
        auc_hits = ((s[:, None] > s[None, :]) & (y[:, None] > y[None, :])).sum() / max(1, (y.sum() * (n - y.sum())))
        self.assertGreater(auc_hits, 0.85)
        self.assertTrue(t["digest"].startswith("sha256:"))
        # the tower's only label input is y: a teacher of NaNs everywhere still trains on outcomes
        t2 = fc.train_tower(Q, D, y, np.full(n, np.nan, np.float32), groups, k=4, epochs=5, log=False)
        self.assertTrue(np.isfinite(t2["val_loss"]))

    def test_model_tower_needs_the_same_vocabulary(self):
        class St:
            vocab = VOCAB
        self.assertIsNone(fc.model_tower({"cards": {"tower": {"k": 1}, "vocab_digest": "sha256:other"}}, St))
        self.assertEqual(fc.model_tower({"cards": {"tower": {"k": 1}, "vocab_digest": VOCAB["digest"]}}, St), {"k": 1})


if __name__ == "__main__":
    unittest.main()

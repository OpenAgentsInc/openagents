"""The query-path speedups (#11269) keep filefind's output identical.

    python3 -m unittest scripts/filefind/test_latency.py

The synthetic tests always run. The bench test runs the fixed query set
(fixtures/latency-queries.json) through this filefind and through the one before
the speedups, on one frozen copy of the real cache, and needs every ranked list,
map, stage set and reason to match byte for byte. It needs the local cache and
takes a few minutes, so it is opt-in:

    FILEFIND_IDENTITY=1 python3 -m unittest scripts/filefind/test_latency.py
    (FILEFIND_IDENTITY_BASE=<rev> to compare against another version; default the
    commit before the speedups; FILEFIND_IDENTITY_N=<n> for the first n queries)
"""

import json
import math
import os
import random
import shutil
import subprocess
import sys
import tempfile
import unittest
from collections import defaultdict
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import filefind as ff  # noqa: E402

BASE_REV = "1d16475293"  # filefind before #11269


def snap(d):
    """A mapping with its key order and each value's iteration order."""
    return [(k, list(v)) for k, v in d.items()]


class PackTest(unittest.TestCase):
    """Packed postings return the same ids, in the same order, as the indexed SQL."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.cache = self.tmp.name
        self.rng = random.Random(7)
        self.vocab = [f"tok_{i}" for i in range(60)] + ["shared-name", "x" * 30]

    def tearDown(self):
        self.tmp.cleanup()

    def add_blobs(self, ix, n, table="tok"):
        """Append blobs the way ensure() does (increasing ids, rows in blob order),
        without touching the pack: what an older filefind on the same cache writes."""
        base = max(ix.ids.values(), default=0) + 1
        rows = []
        for i in range(n):
            bid = base + i
            ix.ids[f"sha{bid}"] = bid
            for t in self.rng.sample(self.vocab, self.rng.randint(0, 25)):
                rows.append((t, bid))
        with ix.db:
            ix.db.executemany("INSERT INTO blob VALUES (?, ?)", [(b, s) for s, b in ix.ids.items() if b >= base])
            ix.db.executemany(f"INSERT INTO {table} VALUES (?, ?)", rows)
            ix.db.execute(f"CREATE INDEX IF NOT EXISTS {table}_t ON {table}(t)")

    def tree_map(self, ix, frac=0.6):
        m = ff._BidMap(list)
        for s, b in sorted(ix.ids.items(), key=lambda kv: kv[1]):
            if self.rng.random() < frac:
                m[b].append(f"p/{b}.rs")
                if self.rng.random() < 0.1:  # one blob at two paths
                    m[b].append(f"q/{b}.rs")
        return m

    def compare_tokens(self, ix):
        bmap = self.tree_map(ix)
        toks = self.vocab + ["absent"]
        self.rng.shuffle(toks)
        ix.pack.current = False
        want = (snap(ix.lookup("tok", toks, bmap)), [ix.postings(t, cap=7) for t in toks])
        self.assertTrue(ix.pack.sync())
        got = (snap(ix.lookup("tok", toks, bmap)), [ix.postings(t, cap=7) for t in toks])
        self.assertEqual(got, want)

    def test_tokens_full_incremental_and_compacted(self):
        ix = ff.TokenIndex(self.cache)
        self.add_blobs(ix, 40)
        self.compare_tokens(ix)                     # first build
        self.add_blobs(ix, 15)                      # an older writer appends
        ix.pack.current = True
        self.assertFalse(ix.pack.sync(limit=0))     # behind: a query falls back to SQL
        self.assertFalse(ix.pack.current)
        self.compare_tokens(ix)                     # catch-up appends a chunk
        old = ff.PACK_COMPACT
        ff.PACK_COMPACT = 1
        try:
            for _ in range(3):
                self.add_blobs(ix, 5)
                self.assertTrue(ix.pack.sync())
            self.assertTrue(ix.pack.sync(compact=True))
            n = ix.db.execute("SELECT max(c) FROM (SELECT count(*) c FROM tokpack GROUP BY t)").fetchone()[0]
            self.assertEqual(n, 1)
        finally:
            ff.PACK_COMPACT = old
        self.compare_tokens(ix)

    def test_chunked_build(self):
        old = ff.PACK_CHUNK_ROWS
        ff.PACK_CHUNK_ROWS = 37
        try:
            ix = ff.TokenIndex(self.cache)
            self.add_blobs(ix, 50)
            self.compare_tokens(ix)
        finally:
            ff.PACK_CHUNK_ROWS = old

    def test_iface_users(self):
        ix = ff.IfaceIndex(self.cache)
        self.vocab = [f"/r/{i}" for i in range(700)] + ["f:name", "a.b"]
        self.add_blobs(ix, 200, table="s")
        strings = list(self.vocab) + ["absent"]
        self.rng.shuffle(strings)
        for cut in (3, 703):  # the SQL walks chunks of 500
            sub = strings[:cut]
            ix.pack.current = False
            want = snap(ix.users(sub))
            self.assertTrue(ix.pack.sync())
            self.assertEqual(snap(ix.users(sub)), want)


class PathTest(unittest.TestCase):
    def test_name_and_prefix_scans(self):
        rng = random.Random(3)
        parts = ["crates", "src", "a", "b.rs", "lib.rs", "main.rs", "docs", "x-y", "README.md", "tests"]
        tree = {}
        for _ in range(400):
            p = "/".join(rng.choice(parts) for _ in range(rng.randint(1, 5)))
            tree[p] = "0" * 40
        q = ff.Query.__new__(ff.Query)
        q.paths = sorted(tree)
        q._by_name = None
        toks = list(tree)[:50] + ["lib.rs", "b.rs", "src/lib.rs", "", "crates/", "crates/src", "nope", "a/b.rs"]
        for tok in toks:
            tok = tok.lstrip("./")
            exact = [p for p in q.paths if p == tok or p.endswith("/" + tok)]
            fast = [p for p in q.by_name().get(tok.rsplit("/", 1)[-1], ()) if p == tok or p.endswith("/" + tok)]
            self.assertEqual(fast, exact, tok)
            pre = tok.rstrip("/") + "/"
            self.assertEqual(q.under(pre), [p for p in q.paths if p.startswith(pre)], tok)


def relative_reference(feats, keys):
    """relative() before #11269."""
    paths = list(feats)
    for k in keys:
        v = np.array([feats[p].get(k, 0.0) for p in paths])
        top = v.max() if len(v) else 0.0
        order = np.argsort(-v, kind="stable")
        r = np.empty(len(v))
        r[order] = np.arange(len(v))
        for i, p in enumerate(paths):
            feats[p][k + "_rel"] = float(v[i] / top) if top > 0 else 0.0
            feats[p][k + "_rk"] = math.log1p(r[i]) if v[i] > 0 else 8.0


class RelativeTest(unittest.TestCase):
    def test_same_values(self):
        rng = random.Random(5)
        for n in (0, 1, 50):
            feats = {f"p{i}": defaultdict(float) for i in range(n)}
            for x in feats.values():
                for k in ("a", "b", "c", "z"):
                    r = rng.random()
                    if k == "z":
                        continue  # never set: all zero
                    x[k] = 0.0 if r < 0.3 else (1.0 if r < 0.5 else rng.random() * 10)
            a = {p: dict(x) for p, x in feats.items()}
            b = {p: dict(x) for p, x in feats.items()}
            relative_reference(a, ["a", "b", "c", "z"])
            ff.relative(b, ["a", "b", "c", "z"])
            self.assertEqual(json.dumps(a), json.dumps(b))


@unittest.skipUnless(os.environ.get("FILEFIND_IDENTITY"), "opt-in: needs the local cache (see the module doc)")
class BenchIdentityTest(unittest.TestCase):
    def test_ranked_output_identical(self):
        repo = subprocess.run(["git", "-C", str(HERE), "rev-parse", "--show-toplevel"], check=True,
                              capture_output=True, text=True).stdout.strip()
        base_rev = os.environ.get("FILEFIND_IDENTITY_BASE", BASE_REV)
        with tempfile.TemporaryDirectory() as tmp:
            base = os.path.join(tmp, "base")
            os.makedirs(base)
            for f in ("filefind.py", "cards.py", "model.json"):
                with open(os.path.join(base, f), "wb") as out:
                    out.write(subprocess.run(["git", "-C", repo, "show", f"{base_rev}:scripts/filefind/{f}"],
                                             check=True, capture_output=True).stdout)
            # one frozen copy of the cache, so both versions read the same history and indexes
            cache = os.path.join(tmp, "cache")
            src = ff.default_cache(repo)
            clone = ["cp", "-cR"] if sys.platform == "darwin" else ["cp", "-R", "--reflink=auto"]
            subprocess.run(clone + [src, cache], check=True)
            for idx in (ff.TokenIndex(cache), ff.IfaceIndex(cache)):
                self.assertTrue(idx.pack.sync())
            rev = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], check=True, capture_output=True,
                                 text=True).stdout.strip()
            n = os.environ.get("FILEFIND_IDENTITY_N")
            out = {}
            for name, ffdir in (("base", base), ("new", str(HERE))):
                out[name] = os.path.join(tmp, name + ".jsonl")
                subprocess.run([sys.executable, str(HERE / "latency_bench.py"), "run", "--ff", ffdir, "--repo", repo,
                                "--rev", rev, "--cache", cache, "--out", out[name]] + (["--n", n] if n else []),
                               check=True, capture_output=True)
            rows = {k: [json.loads(l) for l in open(v)] for k, v in out.items()}
            self.assertGreaterEqual(len(rows["new"]), int(n or 40))
            self.assertEqual([(r["issue"], r["digest"]) for r in rows["new"]],
                             [(r["issue"], r["digest"]) for r in rows["base"]])
            shutil.rmtree(cache, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()

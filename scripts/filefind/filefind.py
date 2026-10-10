#!/usr/bin/env python3
"""filefind: from an issue to the files its fix needs (issue #11210).

One command takes an issue (number or text) and a repository path and prints
a ranked list of the files the change needs, each with the reasons it was
picked. Almost everything is deterministic lookups in indexes precomputed
from the repository's Git history:

  emb    embedding similarity between the issue and each file's path plus
         head (text-embedding-3-small, 256 dims, cached per blob)
  sym    paths, file names, crate names, identifiers and literal strings the
         issue names, resolved with `git grep` (definitions and mentions)
  co     files that historically changed together with the seed files
  sim    the fix files of the most similar past closed issues
  pair   paired-file rules: tests, `mod`/`lib`/`main` lines, Cargo.toml,
         README, the crate's tests/ directory
  dir    the other files in a seed file's directory
  recent files touched in the last commits

Every candidate gets one feature vector, and a small logistic scorer trained
on past issue -> fix pairs (`scripts/bench/file-finding-bench.py train`)
ranks them. The only network call on the query path is one embedding request
for the issue text. The judge (Jev) is called only by the bench's `--judge`
stage, for the files the scorer is unsure about.

Extraction from the issue text reads only bounded fields (paths, file names,
code identifiers, quoted literals); no keyword decides a route. Ranking is
the learned scorer.

Usage
    export OPENROUTER_API_KEY=...          # embeddings (never printed)
    python3 scripts/filefind/filefind.py index --repo . --issues closed.json
    python3 scripts/filefind/filefind.py query --repo . --issue 11210 --k 50
    python3 scripts/filefind/filefind.py query --repo . --text "..." --json

The index lives in ~/.cache/openagents/filefind/<repo name>/ (override with
--cache). It holds vectors, paths and issue numbers/titles, no file contents.
"""
import argparse, bisect, gzip, json, math, os, pickle, re, subprocess, sys, time
import urllib.request, urllib.error
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor

import numpy as np

EMBED_URL = "https://openrouter.ai/api/v1/embeddings"
EMBED_MODEL = "openai/text-embedding-3-small"
DIMS = 256
HEAD_CHARS = 1200
ISSUE_CHARS = 6000
BIG_COMMIT = 40          # commits touching more files are ignored for co-change
LOCK_NAMES = {"Cargo.lock", "package-lock.json", "pnpm-lock.yaml", "yarn.lock", "bun.lock",
              "bun.lockb", "Package.resolved", "go.sum", "flake.lock", "uv.lock", "poetry.lock"}
SOURCES = ["emb", "sym", "co", "sim", "hist", "pair", "dir", "recent", "stage2"]


# ---------------------------------------------------------------- git

def git(repo, *args, check=True, inp=None):
    return subprocess.run(["git", "-C", repo, *args], check=check, capture_output=True, input=inp).stdout


def ls_tree(repo, rev):
    """path -> blob sha for every blob at rev."""
    out = git(repo, "ls-tree", "-r", "-z", rev).decode("utf-8", "replace")
    tree = {}
    for ent in out.split("\0"):
        if not ent:
            continue
        meta, path = ent.split("\t", 1)
        _, typ, sha = meta.split()
        if typ == "blob":
            tree[path] = sha
    return tree


def worktree_tree(repo):
    """path -> blob sha for the working tree's index (HEAD plus staged)."""
    out = git(repo, "ls-files", "-s", "-z").decode("utf-8", "replace")
    tree = {}
    for ent in out.split("\0"):
        if ent:
            meta, path = ent.split("\t", 1)
            tree[path] = meta.split()[1]
    return tree


def cat_heads(repo, shas, n=HEAD_CHARS * 2, cap=HEAD_CHARS):
    """sha -> head text (None for binary) via one `git cat-file --batch`."""
    p = subprocess.Popen(["git", "-C", repo, "cat-file", "--batch"], stdin=subprocess.PIPE,
                         stdout=subprocess.PIPE)
    out = {}
    import threading

    def feed():
        for s in shas:
            p.stdin.write((s + "\n").encode())
        p.stdin.close()
    threading.Thread(target=feed, daemon=True).start()
    for s in shas:
        hdr = p.stdout.readline().split()
        size = int(hdr[2])
        data = p.stdout.read(size)
        p.stdout.read(1)
        head = data[:n]
        out[s] = None if b"\0" in head[:8000] else head.decode("utf-8", "replace")[:cap]
    p.wait()
    return out


# ---------------------------------------------------------------- embeddings

def embed(texts, key, batch=128, workers=6, dims=DIMS):
    """Unit vectors (float32, dims) for texts, in order."""
    def one(chunk):
        body = json.dumps({"model": EMBED_MODEL, "input": chunk, "dimensions": dims}).encode()
        for attempt in range(6):
            r = urllib.request.Request(EMBED_URL, data=body, headers={
                "Authorization": f"Bearer {key}", "Content-Type": "application/json"})
            try:
                with urllib.request.urlopen(r, timeout=120) as x:
                    d = json.load(x)
                if "data" not in d:
                    raise RuntimeError(str(d)[:200])
                return [e["embedding"] for e in sorted(d["data"], key=lambda e: e["index"])]
            except Exception as e:  # retry transient errors
                if attempt == 5:
                    raise
                time.sleep(2 ** attempt)
    chunks = [texts[i:i + batch] for i in range(0, len(texts), batch)]
    with ThreadPoolExecutor(workers) as ex:
        res = list(ex.map(one, chunks))
    m = np.array([v for c in res for v in c], dtype=np.float32).reshape(-1, dims)
    m /= np.linalg.norm(m, axis=1, keepdims=True) + 1e-9
    return m


def embed_key():
    k = os.environ.get("OPENROUTER_API_KEY")
    if not k:
        sys.exit("OPENROUTER_API_KEY is not set (embeddings)")
    return k


def file_text(path, head):
    return f"FILE {path}\n{head or ''}"[:HEAD_CHARS + 200]


def issue_text(title, body):
    return f"{title}\n\n{body or ''}"[:ISSUE_CHARS]


# ---------------------------------------------------------------- index

class Index:
    def __init__(self, cache):
        self.cache = cache
        os.makedirs(cache, exist_ok=True)
        self.hist = None
        self.issues = None
        self.blob_rows = {}
        self.blob_mat = np.zeros((0, DIMS), np.float16)

    # history: commits oldest -> newest
    def load(self, need_blobs=True):
        with open(os.path.join(self.cache, "history.pkl"), "rb") as f:
            self.hist = pickle.load(f)
        p = os.path.join(self.cache, "issues.pkl")
        if os.path.exists(p):
            with open(p, "rb") as f:
                self.issues = pickle.load(f)
        if need_blobs:
            self.load_blobs()
        self.tokens = TokenIndex(self.cache)
        self.load_commit_vecs()
        return self

    def load_blobs(self):
        p = os.path.join(self.cache, "blobs.npz")
        if os.path.exists(p):
            z = np.load(p)
            keys = z["keys"]
            self.blob_mat = z["vecs"]
            self.blob_rows = {k: i for i, k in enumerate(keys.tolist())}

    def save_blobs(self):
        keys = np.array(list(self.blob_rows.keys()), dtype="S40") if self.blob_rows else np.zeros(0, "S40")
        np.savez(os.path.join(self.cache, "blobs.npz"), keys=keys, vecs=self.blob_mat)

    def build_history(self, repo, rev):
        raw = git(repo, "log", "--reverse", "--no-merges", "--format=%x00%H%x09%ct%x09%s",
                  "--name-only", rev).decode("utf-8", "replace")
        paths, pid = [], {}
        commits = []
        for blk in raw.split("\0")[1:]:
            lines = blk.strip("\n").split("\n")
            sha, ts, subj = lines[0].split("\t", 2)
            fids = []
            for l in lines[1:]:
                if l:
                    if l not in pid:
                        pid[l] = len(paths)
                        paths.append(l)
                    fids.append(pid[l])
            commits.append((sha, int(ts), subj, fids))
        by_file = defaultdict(list)
        issue_commits = defaultdict(list)
        for i, (_, _, subj, fids) in enumerate(commits):
            if len(fids) <= BIG_COMMIT:
                for f in fids:
                    by_file[f].append(i)
            for n in set(int(x) for x in re.findall(r"#(\d{3,6})\b", subj)):
                issue_commits[n].append(i)
        self.hist = {"rev": git(repo, "rev-parse", rev).decode().strip(), "commits": commits,
                     "paths": paths, "pid": pid, "by_file": dict(by_file),
                     "issue_commits": dict(issue_commits),
                     "pos": {c[0]: i for i, c in enumerate(commits)}}
        with open(os.path.join(self.cache, "history.pkl"), "wb") as f:
            pickle.dump(self.hist, f, protocol=4)

    def build_commit_vecs(self, key):
        """Embed every commit subject (cached by sha)."""
        p = os.path.join(self.cache, "commits.npz")
        rows = {}
        if os.path.exists(p):
            z = np.load(p)
            rows = {k: v for k, v in zip(z["keys"].tolist(), z["vecs"])}
        todo = [c for c in self.hist["commits"] if c[0].encode() not in rows]
        if todo:
            m = embed([c[2][:500] for c in todo], key, batch=512).astype(np.float16)
            for c, v in zip(todo, m):
                rows[c[0].encode()] = v
        keys = [c[0].encode() for c in self.hist["commits"]]
        np.savez(p, keys=np.array(keys, dtype="S40"), vecs=np.array([rows[k] for k in keys]))
        self.commit_vecs = np.array([rows[k] for k in keys], dtype=np.float32)
        return len(todo)

    def load_commit_vecs(self):
        p = os.path.join(self.cache, "commits.npz")
        self.commit_vecs = None
        if os.path.exists(p):
            z = np.load(p)
            pos = self.hist["pos"]
            keys = z["keys"].tolist()
            m = np.zeros((len(self.hist["commits"]), DIMS), np.float32)
            for k, v in zip(keys, z["vecs"]):
                i = pos.get(k.decode())
                if i is not None:
                    m[i] = v
            self.commit_vecs = m

    def build_issues(self, issues_json, key):
        rows = json.load(open(issues_json))
        old = {}
        if self.issues:
            old = {n: self.issues["vecs"][i] for i, n in enumerate(self.issues["numbers"])}
        todo = [r for r in rows if r["number"] not in old]
        vecs = embed([issue_text(r["title"], r["body"]) for r in todo], key) if todo else np.zeros((0, DIMS))
        for r, v in zip(todo, vecs):
            old[r["number"]] = v
        titles = dict(self.issues["titles"]) if self.issues else {}
        titles.update({r["number"]: r["title"] for r in rows})
        nums = sorted(old)
        self.issues = {"numbers": np.array(nums), "vecs": np.array([old[n] for n in nums], np.float32),
                       "titles": titles}
        with open(os.path.join(self.cache, "issues.pkl"), "wb") as f:
            pickle.dump(self.issues, f, protocol=4)

    def ensure_blobs(self, repo, tree, key, log=True):
        """Embed every blob of `tree` the cache lacks. Returns how many."""
        todo = sorted({s for s in tree.values() if s.encode() not in self.blob_rows})
        if not todo:
            return 0
        by_sha = {}
        for p, s in tree.items():
            by_sha.setdefault(s, p)
        heads = cat_heads(repo, todo)
        texts = [file_text(by_sha[s], heads[s]) for s in todo]
        t0 = time.time()
        m = embed(texts, key).astype(np.float16)
        base = len(self.blob_rows)
        for i, s in enumerate(todo):
            self.blob_rows[s.encode()] = base + i
        self.blob_mat = np.concatenate([self.blob_mat, m]) if len(self.blob_mat) else m
        if log:
            print(f"embedded {len(todo)} blobs in {time.time()-t0:.1f}s", file=sys.stderr)
        return len(todo)


# ---------------------------------------------------------------- token index

TOKEN = re.compile(r"[A-Za-z_][A-Za-z0-9_]{3,}|[a-z0-9]+(?:-[a-z0-9]+)+")
DEFN = re.compile(r"\b(?:fn|struct|enum|trait|type|const|static|mod|union|class|def|interface|func|"
                  r"protocol|macro_rules!)\s+([A-Za-z_][A-Za-z0-9_]*)")
TOKEN_MAX_BYTES = 1 << 20


def skip_for_tokens(path):
    return path.startswith(("bench/terminal-bench/", "assets/", "docs/transcripts/")) or \
        path.endswith((".jsonl", ".lock", ".svg", ".png", ".jpg", ".gif", ".wasm", ".glb", ".ttf"))


class TokenIndex:
    """Inverted index, per blob: identifier and kebab tokens, and defined names.

    Keyed by blob, so one index answers for any revision: a query maps the
    revision's tree (path -> blob) onto the postings.
    """

    def __init__(self, cache):
        import sqlite3
        self.db = sqlite3.connect(os.path.join(cache, "tokens.sqlite"), check_same_thread=False)
        self.db.executescript(
            "CREATE TABLE IF NOT EXISTS blob(id INTEGER PRIMARY KEY, sha TEXT UNIQUE);"
            "CREATE TABLE IF NOT EXISTS tok(t TEXT, b INTEGER);"
            "CREATE TABLE IF NOT EXISTS def(t TEXT, b INTEGER);")
        self.ids = dict(self.db.execute("SELECT sha, id FROM blob"))
        self.db.execute("CREATE INDEX IF NOT EXISTS def_b ON def(b)")

    def defined_in(self, bid):
        return [t for (t,) in self.db.execute("SELECT t FROM def WHERE b = ?", (bid,))]

    def postings(self, t, cap=400):
        return [b for (b,) in self.db.execute("SELECT b FROM tok WHERE t = ? LIMIT ?", (t, cap))]

    def ensure(self, repo, tree, log=True):
        items = tree.items() if isinstance(tree, dict) else tree
        todo = sorted({s for p, s in items if s not in self.ids and not skip_for_tokens(p)})
        if not todo:
            return 0
        t0 = time.time()
        p = subprocess.Popen(["git", "-C", repo, "cat-file", "--batch"], stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE)
        import threading

        def feed():
            for s in todo:
                p.stdin.write((s + "\n").encode())
            p.stdin.close()
        threading.Thread(target=feed, daemon=True).start()
        base = max(self.ids.values(), default=0) + 1
        toks, defs, blobs = [], [], []
        for i, s in enumerate(todo):
            hdr = p.stdout.readline().split()
            data = p.stdout.read(int(hdr[2]))
            p.stdout.read(1)
            bid = base + i
            blobs.append((bid, s))
            self.ids[s] = bid
            if len(data) > TOKEN_MAX_BYTES or b"\0" in data[:8000]:
                continue
            text = data.decode("utf-8", "replace")
            toks.extend((t, bid) for t in set(TOKEN.findall(text)))
            defs.extend((t, bid) for t in set(DEFN.findall(text)))
        p.wait()
        bulk = len(todo) > 5000
        with self.db:
            if bulk:
                self.db.execute("DROP INDEX IF EXISTS tok_t")
                self.db.execute("DROP INDEX IF EXISTS def_t")
            self.db.executemany("INSERT INTO blob VALUES (?, ?)", blobs)
            self.db.executemany("INSERT INTO tok VALUES (?, ?)", toks)
            self.db.executemany("INSERT INTO def VALUES (?, ?)", defs)
            self.db.execute("CREATE INDEX IF NOT EXISTS tok_t ON tok(t)")
            self.db.execute("CREATE INDEX IF NOT EXISTS def_t ON def(t)")
        if log:
            print(f"tokenized {len(todo)} blobs ({len(toks)} postings) in {time.time()-t0:.1f}s",
                  file=sys.stderr)
        return len(todo)

    def lookup(self, table, tokens, bid_to_path):
        """file -> set(token) for tokens present in the tree's blobs."""
        out = defaultdict(set)
        for t in tokens:
            for (b,) in self.db.execute(f"SELECT b FROM {table} WHERE t = ?", (t,)):
                for p in bid_to_path.get(b, ()):
                    out[p].add(t)
        return out


# ---------------------------------------------------------------- issue text fields

CODE = re.compile(r"`([^`\n]{2,160})`")
PATHLIKE = re.compile(r"(?<![\w@:])((?:[\w.-]+/)+[\w.-]+)")
FILENAME = re.compile(r"\b([\w-]+\.(?:rs|md|toml|ts|tsx|js|mjs|py|sh|swift|kt|json|ya?ml|css|html|wgsl|garden|service))\b")
CAMEL = re.compile(r"\b([A-Z][a-z0-9]+(?:[A-Z][a-z0-9]*)+)\b")
SNAKE = re.compile(r"\b([a-z][a-z0-9]*(?:_[a-z0-9]+)+)\b")
SCREAM = re.compile(r"\b([A-Z][A-Z0-9]*(?:_[A-Z0-9]+)+)\b")
KEBAB = re.compile(r"\b([a-z][a-z0-9]*(?:-[a-z0-9]+)+)\b")
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]{3,}")
STOP = {"https", "http", "github", "com", "openagents", "OpenAgentsInc", "issue", "issues", "None",
        "Some", "self", "true", "false", "String", "this", "that", "with", "from", "into", "main"}


def fields(text):
    """Bounded fields named in the issue: paths, file names, identifiers, literals."""
    code = CODE.findall(text)
    paths = set(PATHLIKE.findall(text))
    names = set(FILENAME.findall(text))
    idents, literals, kebabs = set(), set(), set(KEBAB.findall(text))
    for c in code:
        c = c.strip()
        if re.fullmatch(r"[\w:.]+(\(\))?", c):
            idents.update(t for t in re.split(r"[:.()]+", c) if IDENT.fullmatch(t))
        else:
            idents.update(t for t in IDENT.findall(c) if ("_" in t or CAMEL.fullmatch(t)))
            if len(c) >= 6 and not c.startswith("#"):
                literals.add(c)
        kebabs.update(KEBAB.findall(c))
    plain = CODE.sub(" ", text)
    plain = re.sub(r"https?://\S+", " ", plain)
    idents.update(CAMEL.findall(plain))
    idents.update(SNAKE.findall(plain))
    idents.update(SCREAM.findall(plain))
    idents = {i for i in idents if i not in STOP and len(i) >= 4}
    paths = {p.strip(".,)") for p in paths if not p.startswith(("http", "www."))}
    return {"paths": paths, "names": names, "idents": idents, "literals": literals, "kebabs": kebabs}


# ---------------------------------------------------------------- query

class Query:
    """Candidate generation and features for one issue at one revision."""

    def __init__(self, ix, repo, rev, tree, cutoff=None, exclude_issue=None, timing=None):
        self.ix, self.repo, self.rev, self.tree = ix, repo, rev, tree
        h = ix.hist
        self.cutoff = len(h["commits"]) if cutoff is None else cutoff  # commits [0, cutoff) are history
        self.exclude_issue = exclude_issue
        self.t = timing if timing is not None else {}
        self.paths = sorted(tree)
        self.prow = {p: i for i, p in enumerate(self.paths)}
        self.dirs = defaultdict(list)
        for p in self.paths:
            self.dirs[p.rsplit("/", 1)[0] if "/" in p else ""].append(p)

    def tick(self, name, t0):
        self.t[name] = self.t.get(name, 0) + time.perf_counter() - t0

    def run(self, title, body, qvec):
        text = f"{title}\n\n{body}"
        f = self._f = {}  # path -> feature dict
        reason = defaultdict(list)

        def F(p):
            if p not in f:
                f[p] = defaultdict(float)
            return f[p]

        src = defaultdict(set)
        # ---- sym
        t0 = time.perf_counter()
        fl = fields(text)
        crate_dirs = {}
        for p in self.paths:
            parts = p.split("/")
            if len(parts) > 2 and parts[0] in ("crates", "bins", "apps", "packages"):
                crate_dirs.setdefault(parts[1], "/".join(parts[:2]))
        sym_seed = Counter()
        for tok in fl["paths"] | fl["names"]:
            tok = tok.lstrip("./")
            exact = [p for p in self.paths if p == tok or p.endswith("/" + tok)]
            if exact and len(exact) <= 8:
                for p in exact:
                    F(p)["path_exact"] = 1
                    src["sym"].add(p)
                    sym_seed[p] += 3
                    reason[p].append(f"issue names `{tok}`")
                continue
            under = [p for p in self.paths if p.startswith(tok.rstrip("/") + "/")]
            if 0 < len(under) <= 60:
                for p in under:
                    F(p)["path_dir"] = max(F(p)["path_dir"], 1 / math.log2(2 + len(under)))
                    src["sym"].add(p)
                    reason[p].append(f"under `{tok}` named in the issue")
        mentioned_crates = set()
        for k in fl["kebabs"] | {i for i in fl["idents"] if i.islower()}:
            for cand in (k, k.replace("_", "-")):
                if cand in crate_dirs:
                    mentioned_crates.add(crate_dirs[cand])
        for p in self.paths:
            if "/".join(p.split("/")[:2]) in mentioned_crates:
                F(p)["crate_named"] = 1
        for c in mentioned_crates:
            for root in ("src/lib.rs", "src/main.rs", "Cargo.toml"):
                p = f"{c}/{root}"
                if p in self.tree:
                    src["sym"].add(p)
                    reason[p].append(f"crate `{c.split('/')[1]}` named in the issue")
        idents = sorted(fl["idents"] | {k for k in fl["kebabs"] if len(k) >= 6})[:120]
        bid_to_path = defaultdict(list)
        ids = self.ix.tokens.ids
        for p, s in self.tree.items():
            b = ids.get(s)
            if b is not None:
                bid_to_path[b].append(p)
        ment = self.ix.tokens.lookup("tok", idents, bid_to_path)
        defs = self.ix.tokens.lookup("def", [i for i in idents if "-" not in i], bid_to_path)
        lit_toks = {lit: sorted(set(TOKEN.findall(lit))) for lit in sorted(fl["literals"])[:30]}
        lit_toks = {k: v for k, v in lit_toks.items() if len(v) >= 2}
        lt = self.ix.tokens.lookup("tok", sorted({t for v in lit_toks.values() for t in v}), bid_to_path)
        cand = defaultdict(list)
        for p, have in lt.items():
            for lit, need in lit_toks.items():
                if have.issuperset(need):
                    cand[lit].append(p)
        # confirm the literal really occurs (tokens alone over-match); skip undistinctive ones
        check = sorted({p for lit, ps in cand.items() if len(ps) <= 60 for p in ps})
        body = cat_heads(self.repo, [self.tree[p] for p in check], n=1 << 20, cap=1 << 20) if check else {}
        lits = defaultdict(set)
        for lit, ps in cand.items():
            if len(ps) > 60:
                continue
            for p in ps:
                t = body.get(self.tree[p])
                if t and lit in t:
                    lits[p].add(lit)
        df = Counter()
        for p, c in ment.items():
            for k in c:
                df[k] += 1
        for p, c in ment.items():
            w = sum(1 / math.log2(2 + df[k]) for k in c)
            F(p)["sym_ment"] = w
            F(p)["sym_ment_n"] = len(c)
            if w > 0.25:
                src["sym"].add(p)
            rare = [k for k in c if df[k] <= 5]
            if rare:
                reason[p].append("mentions " + ", ".join(f"`{k}`" for k in sorted(rare)[:3]))
            sym_seed[p] += w
        for p, names in defs.items():
            F(p)["sym_def"] = len(names)
            src["sym"].add(p)
            sym_seed[p] += 2 * len(names)
            reason[p].append("defines " + ", ".join(f"`{k}`" for k in sorted(names)[:3]))
        ldf = Counter(k for c in lits.values() for k in c)
        for p, c in lits.items():
            w = sum(1 / math.log2(2 + ldf[k]) for k in c)
            F(p)["lit"] = w
            if ldf and min(ldf[k] for k in c) <= 40:
                src["sym"].add(p)
            sym_seed[p] += w
            reason[p].append("contains " + ", ".join(f"`{k[:40]}`" for k in sorted(c)[:2]))
        self.tick("sym", t0)
        # ---- emb (qvec may be a Future: the embedding call overlaps the token lookups)
        if hasattr(qvec, "result"):
            tw = time.perf_counter()
            qvec = qvec.result()
            self.tick("embed_wait", tw)
        t0 = time.perf_counter()
        rows = [self.ix.blob_rows.get(self.tree[p].encode(), -1) for p in self.paths]
        rows = np.array(rows)
        mat = np.zeros((len(self.paths), DIMS), np.float32)
        ok = rows >= 0
        mat[ok] = self.ix.blob_mat[rows[ok]].astype(np.float32)
        with np.errstate(all="ignore"):
            cos = mat @ qvec
        order = np.argsort(-cos)
        self.cos, self.erank = cos, np.empty_like(order)
        self.erank[order] = np.arange(len(order))
        for r, i in enumerate(order[:300]):
            p = self.paths[i]
            src["emb"].add(p)
            if r < 10:
                reason[p].append(f"embedding rank {r+1} (cos {cos[i]:.2f})")
        self.tick("emb", t0)
        # ---- seeds for graph sources
        seeds = {}
        for r, i in enumerate(order[:10]):
            seeds[self.paths[i]] = 1.0 / (1 + r * 0.3)
        for p, w in sym_seed.most_common(10):
            seeds[p] = max(seeds.get(p, 0), min(1.0, 0.4 + 0.2 * w))
        # ---- co-change
        t0 = time.perf_counter()
        h = self.ix.hist
        co = Counter()
        comax = defaultdict(float)
        for s, ws in seeds.items():
            fid = h["pid"].get(s)
            if fid is None:
                continue
            cs = [c for c in h["by_file"].get(fid, []) if c < self.cutoff][-300:]
            if not cs:
                continue
            cnt = Counter()
            for c in cs:
                for g in h["commits"][c][3]:
                    if g != fid:
                        cnt[g] += 1
            for g, n in cnt.items():
                v = ws * n / (len(cs) + 2)
                co[g] += v
                comax[g] = max(comax[g], n / (len(cs) + 2))
        for g, v in co.most_common(400):
            p = h["paths"][g]
            if p in self.tree:
                F(p)["co"] = v
                F(p)["co_max"] = comax[g]
                src["co"].add(p)
        for g, v in co.most_common(400)[:400]:
            p = h["paths"][g]
            if p in self.tree and v > 0.15:
                reason[p].append(f"changes with the seeds (co {v:.2f})")
        self.tick("co", t0)
        # ---- similar past issues
        t0 = time.perf_counter()
        isx = self.ix.issues
        if isx is not None:
            with np.errstate(all="ignore"):
                sims = isx["vecs"] @ qvec
            cand = []
            for j in np.argsort(-sims)[:80]:
                n = int(isx["numbers"][j])
                if n == self.exclude_issue:
                    continue
                cs = [c for c in h["issue_commits"].get(n, []) if c < self.cutoff]
                if not cs:
                    continue
                cand.append((float(sims[j]), n, cs))
                if len(cand) >= 12:
                    break
            for s, n, cs in cand:
                files = {h["paths"][g] for c in cs for g in h["commits"][c][3]}
                if len(files) > 60:
                    continue
                for p in files:
                    if p in self.tree:
                        F(p)["sim"] += max(0.0, s - 0.3)
                        F(p)["sim_max"] = max(F(p)["sim_max"], s)
                        src["sim"].add(p)
                        if s > 0.5 and len(reason[p]) < 6:
                            reason[p].append(f"fixed with similar #{n} (sim {s:.2f})")
        self.tick("sim", t0)
        # ---- past commits whose subject reads like the issue
        t0 = time.perf_counter()
        cv = self.ix.commit_vecs
        if cv is not None:
            with np.errstate(all="ignore"):
                cs = cv[:self.cutoff] @ qvec
            top = np.argsort(-cs)[:150]
            hist_s, hist_m, hist_best = defaultdict(float), defaultdict(float), {}
            for ci in top:
                sv = float(cs[ci])
                fids = h["commits"][ci][3]
                if len(fids) > BIG_COMMIT or sv < 0.3:
                    continue
                for g in fids:
                    hist_s[g] += sv - 0.3
                    if sv > hist_m[g]:
                        hist_m[g] = sv
                        hist_best[g] = ci
            for g, v in sorted(hist_s.items(), key=lambda kv: -kv[1])[:400]:
                p = h["paths"][g]
                if p in self.tree:
                    F(p)["hist"] = v
                    F(p)["hist_max"] = hist_m[g]
                    src["hist"].add(p)
                    if hist_m[g] > 0.55 and len(reason[p]) < 6:
                        subj = h["commits"][hist_best[g]][2]
                        reason[p].append(f"changed by a similar commit: \"{subj[:70]}\"")
        self.tick("hist", t0)
        # ---- pair + dir rules from the seeds plus top co-change
        t0 = time.perf_counter()
        pseeds = dict(seeds)
        for g, v in co.most_common(8):
            p = h["paths"][g]
            if p in self.tree:
                pseeds.setdefault(p, 0.5)
        for s, ws in pseeds.items():
            for p, why in paired(s, self.tree, self.dirs):
                F(p)["pair"] = max(F(p)["pair"], ws)
                src["pair"].add(p)
                reason[p].append(f"{why} of `{s}`")
            d = s.rsplit("/", 1)[0] if "/" in s else ""
            sib = self.dirs.get(d, []) if d else []
            if len(sib) <= 50:
                for p in sib:
                    F(p)["dir"] = max(F(p)["dir"], ws)
                    src["dir"].add(p)
        self.tick("pair", t0)
        # ---- recency
        t0 = time.perf_counter()
        recent = Counter()
        last = {}
        lo = max(0, self.cutoff - 400)
        for c in range(self.cutoff - 1, lo - 1, -1):
            fids = h["commits"][c][3]
            if len(fids) > BIG_COMMIT:
                continue
            for g in fids:
                recent[g] += 1
                last.setdefault(g, self.cutoff - c)
        for g, n in recent.most_common(150):
            p = h["paths"][g]
            if p in self.tree:
                src["recent"].add(p)
        self.tick("recent", t0)
        # ---- features for the pool
        self.F, self.src, self.reason = F, src, reason
        self.recent, self.last = recent, last
        feats = {p: self.base(p) for p in set().union(*src.values())}
        relative(feats, REL1)
        return feats

    def base(self, p):
        """The full feature dict of path p (source features plus file facts)."""
        h = self.ix.hist
        x = self.F(p)
        i = self.prow[p]
        fid = h["pid"].get(p)
        x["cos"] = float(self.cos[i])
        x["erank"] = math.log1p(int(self.erank[i]))
        x["recent_n"] = math.log1p(self.recent.get(fid, 0)) if fid is not None else 0
        x["recent_age"] = math.log1p(self.last.get(fid, 400)) if fid is not None else math.log1p(400)
        n = bisect.bisect_left(h["by_file"].get(fid, []), self.cutoff) if fid is not None else 0
        x["churn"] = math.log1p(n)
        ext = p.rsplit(".", 1)[-1] if "." in p.rsplit("/", 1)[-1] else ""
        x["is_rs"] = float(ext == "rs")
        x["is_md"] = float(ext == "md")
        x["is_toml"] = float(ext == "toml")
        x["is_test"] = float("/tests" in p or p.endswith(("tests.rs", "_test.rs", "test.py")))
        x["is_docs"] = float(p.startswith("docs/"))
        x["is_bench"] = float(p.startswith(("bench/", "assets/", "knowledge/")))
        x["is_lock"] = float(p.rsplit("/", 1)[-1] in LOCK_NAMES)
        x["depth"] = p.count("/")
        for s_ in SOURCES:
            x["src_" + s_] = float(p in self.src[s_])
        return dict(x)

    def stage2(self, feats, scored):
        """Propagate the first ranking: co-change, pairs and crate/dir mass from its top files."""
        t0 = time.perf_counter()
        h = self.ix.hist
        rank1 = {p: i for i, (p, _) in enumerate(scored)}
        s1 = dict(scored)
        top = scored[:10]
        co2 = Counter()
        for p, sc in top:
            fid = h["pid"].get(p)
            if fid is None:
                continue
            lst = h["by_file"].get(fid, [])
            cs = lst[max(0, bisect.bisect_left(lst, self.cutoff) - 300):bisect.bisect_left(lst, self.cutoff)]
            if not cs:
                continue
            cnt = Counter(g for c in cs for g in h["commits"][c][3] if g != fid)
            for g, n in cnt.items():
                co2[g] += sc * n / (len(cs) + 2)
        crate_mass, dir_mass = Counter(), Counter()
        for p, sc in scored[:30]:
            crate_mass[group(p)] += sc
            dir_mass[p.rsplit("/", 1)[0] if "/" in p else ""] += sc
        tot = sum(sc for _, sc in scored[:30]) or 1.0
        new = set()
        for g, v in co2.most_common(150):
            p = h["paths"][g]
            if p in self.tree:
                new.add(p)
        pair2 = {}
        for p, sc in top:
            for q, why in paired(p, self.tree, self.dirs):
                if sc > pair2.get(q, (0, ""))[0]:
                    pair2[q] = (sc, f"{why} of `{p}`")
                new.add(q)
        # reference graph: files that use names the top files define
        ref, ref_why = Counter(), {}
        ids = self.ix.tokens.ids
        if not hasattr(self, "bid_to_path"):
            self.bid_to_path = defaultdict(list)
            for p, sha in self.tree.items():
                b = ids.get(sha)
                if b is not None:
                    self.bid_to_path[b].append(p)
        for p, sc in scored[:6]:
            b = ids.get(self.tree.get(p, ""))
            if b is None:
                continue
            for name in self.ix.tokens.defined_in(b)[:80]:
                if len(name) < 5:
                    continue
                post = self.ix.tokens.postings(name)
                if len(post) >= 400:
                    continue
                users = {q for x in post for q in self.bid_to_path.get(x, ())} - {p}
                if not users or len(users) > 25:
                    continue
                w = sc / math.log2(2 + len(users))
                for q in users:
                    ref[q] += w
                    if w > ref_why.get(q, (0, ""))[0]:
                        ref_why[q] = (w, f"uses `{name}` defined in `{p}`")
        for q, _ in ref.most_common(120):
            new.add(q)
        for p in new - set(feats):
            self.src["stage2"].add(p)
            feats[p] = self.base(p)
        for p, x in feats.items():
            fid = h["pid"].get(p)
            x["s1"] = s1.get(p, 0.0)
            x["s1_rank"] = math.log1p(rank1.get(p, 3000))
            x["co2"] = co2.get(fid, 0.0) if fid is not None else 0.0
            x["crate_mass"] = crate_mass.get(group(p), 0.0) / tot
            x["dir_mass"] = dir_mass.get(p.rsplit("/", 1)[0] if "/" in p else "", 0.0) / tot
            x["pair2"] = pair2.get(p, (0.0, ""))[0]
            x["ref"] = ref.get(p, 0.0)
            if x["ref"] > 0.1:
                self.reason[p].append(ref_why[p][1])
            x["src_stage2"] = float(p in self.src["stage2"])
            if p in pair2 and pair2[p][0] > 0.3:
                self.reason[p].append(pair2[p][1])
            if x["co2"] > 0.2:
                self.reason[p].append(f"changes with the top-ranked files (co {x['co2']:.2f})")
        relative(feats, REL1 + REL2)
        self.tick("stage2", t0)
        return feats


REL1 = ["cos", "co", "co_max", "hist", "hist_max", "sim", "sim_max", "sym_ment", "lit", "recent_n"]
REL2 = ["co2", "s1", "ref"]


def relative(feats, keys):
    """Per-query features: each signal's share of the query's best, and its rank in the pool."""
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


def tidy(reasons, n=4):
    """Distinct reasons, one per rule (\"crate test of A; crate test of B\" -> the first)."""
    out, seen = [], set()
    for r in reasons:
        key = r.split(" of `")[0] if " of `" in r else r.split("(")[0]
        if key not in seen:
            seen.add(key)
            out.append(r)
    return out[:n]


def group(p):
    parts = p.split("/")
    return "/".join(parts[:2]) if len(parts) > 2 else parts[0]


def paired(s, tree, dirs):
    """Files a change to `s` usually drags in, with the rule's name."""
    out = []
    parts = s.split("/")
    d = "/".join(parts[:-1])
    name = parts[-1]
    stem = name.rsplit(".", 1)[0]
    crate = None
    if len(parts) > 2 and parts[0] in ("crates", "bins", "apps", "packages"):
        crate = "/".join(parts[:2])
    cands = []
    if name.endswith(".rs"):
        cands += [(f"{d}/{stem}/tests.rs", "tests"), (f"{d}/{stem}/mod.rs", "module"),
                  (f"{d}/tests.rs", "tests"), (f"{d}/{stem}_tests.rs", "tests"),
                  (f"{d}/mod.rs", "mod line"), (f"{d}.rs", "mod line")]
        if crate:
            cands += [(f"{crate}/src/lib.rs", "crate root"), (f"{crate}/src/main.rs", "crate root"),
                      (f"{crate}/Cargo.toml", "Cargo.toml")]
            tests = [p for p in dirs.get(f"{crate}/tests", []) if p.endswith(".rs")]
            if len(tests) <= 12:
                cands += [(p, "crate test") for p in tests]
    if name == "Cargo.toml":
        cands += [("Cargo.toml", "workspace Cargo.toml")]
    if crate:
        cands += [(f"{crate}/README.md", "crate README"), (f"{crate}/INVARIANTS.md", "crate INVARIANTS")]
    cands += [(f"{d}/README.md", "directory README")]
    seen = set()
    for p, why in cands:
        if p in tree and p != s and p not in seen:
            seen.add(p)
            out.append((p, why))
    return out


# ---------------------------------------------------------------- scorer
#
# A small gradient-boosted tree ensemble (logistic loss, depth-3 trees over
# quantile bins), trained by scripts/bench/file-finding-bench.py on past
# issue -> fix pairs. numpy only, so nothing heavy is built or installed.

def vectorize(feats, names):
    return np.array([[float(f.get(n, 0.0)) for n in names] for f in feats], dtype=np.float64)


def fit_gbdt(X, y, trees=200, depth=3, lr=0.1, bins=32, min_leaf=30, l2=1.0, pos_weight=3.0,
             sample_weight=None):
    edges = []
    Xb = np.zeros(X.shape, np.uint8)
    for j in range(X.shape[1]):
        e = np.unique(np.quantile(X[:, j], np.linspace(0, 1, bins + 1)[1:-1]))
        edges.append(e.tolist())
        Xb[:, j] = np.searchsorted(e, X[:, j], side="right")
    w = np.where(y > 0, pos_weight, 1.0)
    if sample_weight is not None:
        w = w * sample_weight
    prior = np.clip((w * y).sum() / w.sum(), 1e-4, 1 - 1e-4)
    base = math.log(prior / (1 - prior))
    F = np.full(len(y), base)
    out = []
    for _ in range(trees):
        p = 1 / (1 + np.exp(-F))
        g, hh = (p - y) * w, p * (1 - p) * w
        nodes = []  # (feature, bin, left, right, value)

        def build(idx, d):
            G, H = g[idx].sum(), hh[idx].sum()
            me = len(nodes)
            nodes.append([-1, 0, -1, -1, float(-G / (H + l2))])
            if d == depth or len(idx) < 2 * min_leaf:
                return me
            best = (0.0, None, None)
            parent = G * G / (H + l2)
            xb = Xb[idx]
            for j in range(X.shape[1]):
                gs = np.bincount(xb[:, j], g[idx], minlength=bins + 1)
                hs = np.bincount(xb[:, j], hh[idx], minlength=bins + 1)
                ns = np.bincount(xb[:, j], minlength=bins + 1)
                GL, HL, NL = np.cumsum(gs)[:-1], np.cumsum(hs)[:-1], np.cumsum(ns)[:-1]
                GR, HR, NR = G - GL, H - HL, len(idx) - NL
                gain = GL ** 2 / (HL + l2) + GR ** 2 / (HR + l2) - parent
                gain[(NL < min_leaf) | (NR < min_leaf)] = 0
                k = int(np.argmax(gain))
                if gain[k] > best[0]:
                    best = (float(gain[k]), j, k)
            if best[1] is None:
                return me
            _, j, k = best
            m = xb[:, j] <= k
            nodes[me][0], nodes[me][1] = j, k
            nodes[me][2] = build(idx[m], d + 1)
            nodes[me][3] = build(idx[~m], d + 1)
            return me
        build(np.arange(len(y)), 0)
        out.append(nodes)
        F += lr * predict_tree(nodes, Xb)
    return {"kind": "gbdt", "edges": edges, "trees": out, "base": base, "lr": lr}


_ARRAYS = {}  # id(model) -> per-tree numpy arrays (models live for the whole process)


def tree_arrays(nodes):
    return tuple(np.array([n[i] for n in nodes]) for i in range(5))


def predict_tree(nodes, Xb, arrays=None):
    cur = np.zeros(len(Xb), np.int64)
    feat, thr, left, right, val = arrays or tree_arrays(nodes)
    for _ in range(8):
        f = feat[cur]
        leaf = f < 0
        if leaf.all():
            break
        fx = np.where(leaf, 0, f)
        go_left = Xb[np.arange(len(Xb)), fx] <= thr[cur]
        cur = np.where(leaf, cur, np.where(go_left, left[cur], right[cur]))
    return val[cur]


def score(m, X):
    Xb = np.zeros(X.shape, np.uint8)
    for j, e in enumerate(m["edges"]):
        Xb[:, j] = np.searchsorted(np.array(e), X[:, j], side="right")
    F = np.full(len(X), m["base"])
    arrays = _ARRAYS.get(id(m))
    if arrays is None:
        arrays = _ARRAYS[id(m)] = [tree_arrays(t) for t in m["trees"]]
    for t, arr in zip(m["trees"], arrays):
        F += m["lr"] * predict_tree(t, Xb, arr)
    return 1 / (1 + np.exp(-F))


def rank(m, feats):
    paths = list(feats)
    if not paths:
        return []
    s = score(m, vectorize([feats[p] for p in paths], m["features"]))
    order = np.argsort(-s, kind="stable")
    return [(paths[i], float(s[i])) for i in order]


def sureness(ranked, k=50):
    """The scorer's own estimate of its recall at k: confidence mass in the top k over all."""
    tot = sum(s for _, s in ranked)
    return sum(s for _, s in ranked[:k]) / tot if tot > 0 else 0.0


def rank_two_stage(model, q, feats):
    """Stage 1 ranks the candidates; stage 2 propagates from its top files and re-ranks."""
    first = rank(model["stage1"], feats)
    feats = q.stage2(feats, first)
    return rank(model["stage2"], feats), feats


# ---------------------------------------------------------------- CLI

def default_cache(repo):
    name = os.path.basename(os.path.abspath(repo))
    return os.path.expanduser(f"~/.cache/openagents/filefind/{name}")


MODEL_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "model.json")


def cmd_index(a):
    key = embed_key()
    ix = Index(a.cache or default_cache(a.repo))
    t0 = time.time()
    ix.build_history(a.repo, a.rev)
    print(f"history: {len(ix.hist['commits'])} commits, {len(ix.hist['paths'])} paths "
          f"({time.time()-t0:.1f}s)", file=sys.stderr)
    n = ix.build_commit_vecs(key)
    print(f"commit subjects embedded: {n} new", file=sys.stderr)
    issues = a.issues
    if not issues and not a.no_issues:
        issues = os.path.join(ix.cache, "closed-issues.json")
        with open(issues, "wb") as f:
            f.write(subprocess.run(["gh", "issue", "list", "--state", "closed", "--limit", str(a.issue_limit),
                                    "--json", "number,title,body,closedAt,createdAt"], cwd=a.repo,
                                   check=True, capture_output=True).stdout)
    if issues:
        p = os.path.join(ix.cache, "issues.pkl")
        if os.path.exists(p):
            ix.issues = pickle.load(open(p, "rb"))
        ix.build_issues(issues, key)
        print(f"issues: {len(ix.issues['numbers'])}", file=sys.stderr)
    ix.load_blobs()
    tree = worktree_tree(a.repo) if a.rev == "WORKTREE" else ls_tree(a.repo, a.rev)
    ix.ensure_blobs(a.repo, tree, key)
    ix.save_blobs()
    print(f"blobs: {len(ix.blob_rows)}", file=sys.stderr)
    TokenIndex(ix.cache).ensure(a.repo, tree)


class _First:
    """A Future of a matrix whose first row is wanted."""

    def __init__(self, fut):
        self.fut = fut

    def result(self):
        return self.fut.result()[0]


def issue_from_gh(repo, n):
    out = subprocess.run(["gh", "issue", "view", str(n), "--json", "title,body"], cwd=repo,
                         check=True, capture_output=True).stdout
    d = json.loads(out)
    return d["title"], d["body"] or ""


def cmd_query(a):
    timing = {}
    t0 = time.perf_counter()
    if a.issue:
        title, body = issue_from_gh(a.repo, a.issue)
    else:
        title, body = a.text.split("\n", 1)[0], a.text
    timing["fetch_issue"] = time.perf_counter() - t0
    t_all = time.perf_counter()  # "total" counts from here: the issue text in hand
    key = os.environ.get("OPENROUTER_API_KEY")
    pool = ThreadPoolExecutor(1)
    if key:  # the one network call runs while the indexes load and the token stages run
        qvec = pool.submit(embed, [issue_text(title, body)], key)
        qvec = _First(qvec)
    else:  # no embeddings: the deterministic stages still run; similarity features are zero
        print("OPENROUTER_API_KEY is not set: running without embeddings (lower recall)", file=sys.stderr)
        qvec = np.zeros(DIMS, np.float32)
    t0 = time.perf_counter()
    ix = Index(a.cache or default_cache(a.repo)).load()
    model = json.load(open(a.model))
    timing["load"] = time.perf_counter() - t0
    t0 = time.perf_counter()
    rev = a.rev
    tree = ls_tree(a.repo, rev)
    timing["tree"] = time.perf_counter() - t0
    t0 = time.perf_counter()
    if key:
        if ix.ensure_blobs(a.repo, tree, key, log=False):
            ix.save_blobs()
    ix.tokens.ensure(a.repo, tree, log=False)
    timing["refresh_index"] = time.perf_counter() - t0
    q = Query(ix, a.repo, rev, tree, exclude_issue=a.issue, timing=timing)
    feats = q.run(title, body, qvec)
    t0 = time.perf_counter()
    ranked, feats = rank_two_stage(model, q, feats)
    ranked = ranked[:a.k]
    timing["score"] = time.perf_counter() - t0
    timing["total"] = time.perf_counter() - t_all
    if a.json:
        print(json.dumps({
            "schema": "openagents.filefind.v1", "issue": a.issue, "title": title,
            "rev": git(a.repo, "rev-parse", rev).decode().strip(),
            "model": os.path.basename(a.model), "timing_ms": {k: round(v * 1000) for k, v in timing.items()},
            "files": [{"rank": i, "path": p, "confidence": round(s, 4),
                       "stages": [s_ for s_ in SOURCES if p in q.src[s_]],
                       "reasons": tidy(q.reason.get(p, [])) or ["ranked by the scorer from weak signals"]}
                      for i, (p, s) in enumerate(ranked, 1)]}, indent=1))
        return
    print(f"# {('#' + str(a.issue) + ' ') if a.issue else ''}{title}")
    for i, (p, s) in enumerate(ranked, 1):
        why = "; ".join(tidy(q.reason.get(p, []), 3)) or "scorer: " + ", ".join(
            s_ for s_ in SOURCES if p in q.src[s_])
        print(f"{i:3d}. {s:.3f}  {p}  — {why}")
    print("timing " + ", ".join(f"{k} {v*1000:.0f} ms" for k, v in timing.items()), file=sys.stderr)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    i = sub.add_parser("index", help="build or refresh the history, issue and blob indexes")
    i.add_argument("--repo", default=".")
    i.add_argument("--rev", default="HEAD")
    i.add_argument("--issues", help="saved `gh issue list --state closed --json number,title,body` "
                   "output (default: fetched with gh)")
    i.add_argument("--issue-limit", type=int, default=5000)
    i.add_argument("--no-issues", action="store_true", help="skip the similar-issue index")
    i.add_argument("--cache")
    q = sub.add_parser("query", help="rank the files an issue needs")
    q.add_argument("--repo", default=".")
    q.add_argument("--rev", default="HEAD")
    g = q.add_mutually_exclusive_group(required=True)
    g.add_argument("--issue", type=int)
    g.add_argument("--text")
    q.add_argument("--k", type=int, default=50)
    q.add_argument("--json", action="store_true")
    q.add_argument("--cache")
    q.add_argument("--model", default=MODEL_PATH)
    a = ap.parse_args()
    {"index": cmd_index, "query": cmd_query}[a.cmd](a)


if __name__ == "__main__":
    main()

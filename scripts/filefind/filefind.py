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
# output-only labels for what stage 2 found (not model features)
STAGES_OUT = SOURCES + ["ref", "iface", "rule", "crate", "tmpl", "feedback"]


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
            except urllib.error.HTTPError as e:  # 4xx other than 429 will not get better
                if attempt == 5 or (400 <= e.code < 500 and e.code != 429):
                    raise
                time.sleep(2 ** attempt)
                continue
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


def embed_key(required=True):
    """OPENROUTER_API_KEY, else the OpenAgents key file ~/.openagents/openrouter.json."""
    k = os.environ.get("OPENROUTER_API_KEY")
    if not k:
        try:
            k = json.load(open(os.path.expanduser("~/.openagents/openrouter.json"))).get("api_key")
        except (OSError, ValueError):
            k = None
    if not k and required:
        sys.exit("OPENROUTER_API_KEY is not set (embeddings)")
    return k


def file_text(path, head):
    return f"FILE {path}\n{head or ''}"[:HEAD_CHARS + 200]


def issue_text(title, body):
    return f"{title}\n\n{body or ''}"[:ISSUE_CHARS]


# ---------------------------------------------------------------- index

FEEDBACK_WEIGHT = {"changed": 1.0, "read_outside": 0.5}


def load_feedback(cache):
    """issue -> {path: weight}: files earlier agent runs on the issue changed, or opened
    outside their briefing (`filefind.py feedback`)."""
    out = defaultdict(dict)
    p = os.path.join(cache, "feedback.jsonl")
    if os.path.exists(p):
        for line in open(p):
            try:
                r = json.loads(line)
            except ValueError:
                continue
            w = FEEDBACK_WEIGHT.get(r.get("kind"), 0.5)
            out[int(r["issue"])][r["path"]] = max(w, out[int(r["issue"])].get(r["path"], 0))
    return out


def atomic(path, write):
    """Write a cache file through a temporary file, so a concurrent reader never sees half."""
    tmp = f"{path}.{os.getpid()}.tmp"
    with open(tmp, "wb") as f:
        write(f)
    os.replace(tmp, path)


def dirs_index(h):
    """directory -> sorted commit positions touching any file in it (small commits only)."""
    by_dir = defaultdict(set)
    for fid, cs in h["by_file"].items():
        p = h["paths"][fid]
        by_dir[p.rsplit("/", 1)[0] if "/" in p else ""].update(cs)
    return {d: sorted(v) for d, v in by_dir.items()}


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
        self.iface = IfaceIndex(self.cache)
        self.load_commit_vecs()
        if "by_dir" not in self.hist:
            self.hist["by_dir"] = dirs_index(self.hist)
        self.feedback = load_feedback(self.cache)
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
        atomic(os.path.join(self.cache, "blobs.npz"), lambda f: np.savez(f, keys=keys, vecs=self.blob_mat))

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
        self.hist["by_dir"] = dirs_index(self.hist)
        atomic(os.path.join(self.cache, "history.pkl"), lambda f: pickle.dump(self.hist, f, protocol=4))

    def refresh_history(self, repo, rev):
        """Append the commits between the indexed head and rev; a rewritten history
        rebuilds. Returns the number of new commits (-1 for a rebuild)."""
        h = self.hist
        head = git(repo, "rev-parse", rev).decode().strip()
        if head == h.get("rev") or head in h["pos"]:
            return 0
        last = h["commits"][-1][0] if h["commits"] else None
        if not last or subprocess.run(["git", "-C", repo, "merge-base", "--is-ancestor", last, head],
                                      capture_output=True).returncode != 0:
            self.build_history(repo, rev)
            return -1
        raw = git(repo, "log", "--reverse", "--no-merges", "--format=%x00%H%x09%ct%x09%s",
                  "--name-only", f"{last}..{head}").decode("utf-8", "replace")
        n = 0
        for blk in raw.split("\0")[1:]:
            lines = blk.strip("\n").split("\n")
            sha, ts, subj = lines[0].split("\t", 2)
            if sha in h["pos"]:
                continue
            fids = []
            for l in lines[1:]:
                if l:
                    if l not in h["pid"]:
                        h["pid"][l] = len(h["paths"])
                        h["paths"].append(l)
                    fids.append(h["pid"][l])
            i = len(h["commits"])
            h["commits"].append((sha, int(ts), subj, fids))
            h["pos"][sha] = i
            if len(fids) <= BIG_COMMIT:
                for f in fids:
                    h["by_file"].setdefault(f, []).append(i)
                for d in {h["paths"][f].rsplit("/", 1)[0] if "/" in h["paths"][f] else "" for f in fids}:
                    h["by_dir"].setdefault(d, []).append(i)
            for num in set(int(x) for x in re.findall(r"#(\d{3,6})\b", subj)):
                h["issue_commits"].setdefault(num, []).append(i)
            n += 1
        h["rev"] = head
        atomic(os.path.join(self.cache, "history.pkl"), lambda f: pickle.dump(h, f, protocol=4))
        return n

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
        atomic(p, lambda f: np.savez(f, keys=np.array(keys, dtype="S40"), vecs=np.array([rows[k] for k in keys])))
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
        atomic(os.path.join(self.cache, "issues.pkl"), lambda f: pickle.dump(self.issues, f, protocol=4))

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


# ---------------------------------------------------------------- interface strings

QUOTED = re.compile(r'"([^"\\\n]{3,120})"|\'([^\'\\\n]{3,120})\'')
ROUTE_IN = re.compile(r"(?:/[A-Za-z0-9_\-.]+){2,}")
METHOD = re.compile(r"^[a-z][a-z0-9_]*(?:[.:][a-z][a-z0-9_]*){1,3}$")
IDLIKE = re.compile(r"^[a-z][a-z0-9]*(?:[-_][a-z0-9]+)+$")
CSS_CLASS = re.compile(r"\.([a-z][a-z0-9]*(?:-[a-z0-9]+)+)")
FILE_EXT = {"rs", "md", "json", "jsonl", "toml", "js", "mjs", "ts", "css", "html", "py", "sh", "png", "jpg",
            "svg", "txt", "yaml", "yml", "lock", "wasm", "gz", "swift", "kt", "com", "org", "io", "dev", "ai", "sh"}


SERDE_DERIVE = re.compile(r"#\[derive\([^)]*(?:Serialize|Deserialize)[^)]*\)\]")
RS_FIELD = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:", re.M)
SERDE_RENAME = re.compile(r"rename\s*=\s*\"([A-Za-z_][\w-]*)\"")
JSON_KEY = re.compile(r"[\"']([a-z][a-z0-9_]{3,40})[\"']\s*[:=]>?|\.get\(\s*\"([a-z][a-z0-9_]{3,40})\"|\[\s*\"([a-z][a-z0-9_]{3,40})\"\s*\]")


def wire_fields(path, text):
    """Field names of wire types: fields of serde structs (and their renames) in Rust, and
    JSON keys written or read anywhere ("name": ..., .get("name"), ["name"])."""
    out = set()
    if path.endswith(".rs"):
        for m in SERDE_DERIVE.finditer(text):
            i = text.find("{", m.end())
            j = text.find(";", m.end())
            if i < 0 or (0 <= j < i) or i - m.end() > 400:
                continue
            depth, k = 0, i
            while k < len(text):
                if text[k] == "{":
                    depth += 1
                elif text[k] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                k += 1
            body = text[i + 1:k]
            out.update(f for f in RS_FIELD.findall(body) if len(f) >= 4)
            out.update(SERDE_RENAME.findall(body))
    for a, b, c in JSON_KEY.findall(text):
        out.add(a or b or c)
    return {"f:" + f for f in out if len(f) >= 4}


def iface_strings(path, text):
    """Interface strings a file uses: routes (and their prefixes), method/event names,
    ids, CSS classes and wire-type field names. These tie a server to its clients
    across crates and languages."""
    out = wire_fields(path, text)
    lits = [a or b for a, b in QUOTED.findall(text)]
    if path.endswith(".css"):
        lits += CSS_CLASS.findall(text)
    for s in lits:
        s = s.strip()
        for m in ROUTE_IN.findall(s):
            segs = [x for x in m.split("/") if x]
            if segs and "." in segs[0] and len(segs) > 1:  # host name
                segs = segs[1:]
            clean = []
            for x in segs:
                if "{" in x or ":" in x or x.startswith("<"):
                    break
                clean.append(x)
            if not clean or not any(c.isalpha() for c in clean[0]):
                continue
            if "." in clean[-1] and clean[-1].rsplit(".", 1)[-1] in FILE_EXT and len(clean) > 1:
                pass  # a file path: still a usable string
            for i in range(2, len(clean) + 1):
                out.add("/" + "/".join(clean[:i]))
            if len(clean) == 1 and len(clean[0]) >= 4:
                out.add("/" + clean[0])
        if " " in s:
            for t in s.split():
                if IDLIKE.match(t) and "-" in t and len(t) >= 6:
                    out.add(t)
            continue
        if METHOD.match(s) and s.rsplit(".", 1)[-1] not in FILE_EXT and len(s) >= 6:
            out.add(s)
        elif IDLIKE.match(s) and len(s) >= 6:
            out.add(s)
    return out


class IfaceIndex:
    """Interface strings per blob, with each string's document frequency."""

    def __init__(self, cache):
        import sqlite3
        self.db = sqlite3.connect(os.path.join(cache, "iface.sqlite"), check_same_thread=False)
        self.db.executescript(
            "CREATE TABLE IF NOT EXISTS blob(id INTEGER PRIMARY KEY, sha TEXT UNIQUE);"
            "CREATE TABLE IF NOT EXISTS s(t TEXT, b INTEGER);"
            "CREATE TABLE IF NOT EXISTS df(t TEXT PRIMARY KEY, n INTEGER);")
        self.ids = dict(self.db.execute("SELECT sha, id FROM blob"))

    def ensure(self, repo, tree, log=True):
        items = tree.items() if isinstance(tree, dict) else tree
        todo = {}
        for p, sha in items:
            if sha not in self.ids and sha not in todo and not skip_for_tokens(p):
                todo[sha] = p
        if not todo:
            return 0
        t0 = time.time()
        shas = sorted(todo)
        base = max(self.ids.values(), default=0) + 1
        rows, blobs, df = [], [], Counter()
        for k in range(0, len(shas), 2000):
            chunk = shas[k:k + 2000]
            texts = cat_heads(repo, chunk, n=TOKEN_MAX_BYTES, cap=TOKEN_MAX_BYTES)
            for i, sha in enumerate(chunk, k):
                bid = base + i
                blobs.append((bid, sha))
                self.ids[sha] = bid
                t = texts.get(sha)
                if t:
                    got = iface_strings(todo[sha], t)
                    rows.extend((x, bid) for x in got)
                    df.update(got)
        bulk = len(shas) > 5000
        with self.db:
            if bulk:
                self.db.execute("DROP INDEX IF EXISTS s_t")
                self.db.execute("DROP INDEX IF EXISTS s_b")
            self.db.executemany("INSERT INTO blob VALUES (?, ?)", blobs)
            self.db.executemany("INSERT INTO s VALUES (?, ?)", rows)
            self.db.executemany("INSERT INTO df VALUES (?, ?) ON CONFLICT(t) DO UPDATE SET n = n + excluded.n",
                                list(df.items()))
            self.db.execute("CREATE INDEX IF NOT EXISTS s_t ON s(t)")
            self.db.execute("CREATE INDEX IF NOT EXISTS s_b ON s(b)")
        if log:
            print(f"interface strings: {len(shas)} blobs, {len(rows)} postings in {time.time()-t0:.1f}s",
                  file=sys.stderr)
        return len(shas)

    def strings_of(self, bid):
        return [t for (t,) in self.db.execute("SELECT t FROM s WHERE b = ?", (bid,))]

    def rare(self, strings, max_df):
        out = {}
        strings = list(strings)
        for i in range(0, len(strings), 500):
            chunk = strings[i:i + 500]
            q = ",".join("?" * len(chunk))
            for t, n in self.db.execute(f"SELECT t, n FROM df WHERE t IN ({q})", chunk):
                if n <= max_df:
                    out[t] = n
        return out

    def users(self, strings, current=None):
        """string -> blob ids using it; with `current` (blob ids of one tree) only those."""
        out = defaultdict(list)
        strings = list(strings)
        if current is not None and not getattr(self, "_cur", None) is current:
            self.db.execute("CREATE TEMP TABLE IF NOT EXISTS cur(b INTEGER PRIMARY KEY)")
            self.db.execute("DELETE FROM cur")
            self.db.executemany("INSERT OR IGNORE INTO cur VALUES (?)", ((b,) for b in current))
            self._cur = current
        for i in range(0, len(strings), 500):
            chunk = strings[i:i + 500]
            q = ",".join("?" * len(chunk))
            sql = (f"SELECT s.t, s.b FROM s JOIN cur ON cur.b = s.b WHERE s.t IN ({q})" if current is not None
                   else f"SELECT t, b FROM s WHERE t IN ({q})")
            for t, b in self.db.execute(sql, chunk):
                out[t].append(b)
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
                if not cs and n not in getattr(self.ix, "feedback", {}):
                    continue
                cand.append((float(sims[j]), n, cs))
                if len(cand) >= 12:
                    break
            fb = getattr(self.ix, "feedback", {})
            for s, n, cs in cand:
                files = {h["paths"][g] for c in cs for g in h["commits"][c][3]} | set(fb.get(n, {}))
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

    def interface_users(self, top):
        """Files that use the interface strings (routes, methods, ids, CSS classes) of the top
        files: the clients and other implementations of what the fix changes."""
        ix = self.ix.iface
        if not hasattr(self, "iface_bid"):
            self.iface_bid = defaultdict(list)
            for p, sha in self.tree.items():
                b = ix.ids.get(sha)
                if b is not None:
                    self.iface_bid[b].append(p)

        score, cross, why = Counter(), Counter(), {}
        for p, sc in top:
            b = ix.ids.get(self.tree.get(p, ""))
            if b is None:
                continue
            strings = ix.rare(ix.strings_of(b), 600)
            fs = sorted((n, t) for t, n in strings.items() if t.startswith("f:"))
            for n, t in fs[60:]:
                del strings[t]
            for n, t in fs[:60]:
                if n > 150:
                    del strings[t]
            if not strings:
                continue
            users = ix.users(strings)
            # Rust readers of a rare wire field use it as `.name`: one batched token lookup
            fields = sorted(t[2:] for t, n in strings.items()
                            if t.startswith("f:") and n <= 100 and ("_" in t or len(t) >= 9))[:40]
            readers = defaultdict(set)
            if fields:
                q_ = ",".join("?" * len(fields))
                for t_, b_ in self.ix.tokens.db.execute(f"SELECT t, b FROM tok WHERE t IN ({q_})", fields):
                    readers["f:" + t_].update(self.bid_to_path.get(b_, ()))
            for t, bs in users.items():
                files = {q for x in bs for q in self.iface_bid.get(x, ())} - {p}
                if t in readers and len(readers[t]) <= 30:
                    files |= readers[t] - {p}
                if not files or len(files) > 30:
                    continue
                w = sc / math.log2(2 + len(files))
                for q in files:
                    score[q] += w
                    if group(q) != group(p):
                        cross[q] += w
                    if w > why.get(q, (0, ""))[0]:
                        why[q] = (w, f"uses `{t}` like `{p}`")
        return score, cross, why

    def rules(self, scored):
        """\"X changes, so Y changes\" from history: P(Y | X) for the top files X and
        P(Y | a file in directory D changes) for their directories."""
        h = self.ix.hist
        rule_f, rule_d, why = Counter(), Counter(), {}
        for p, sc in scored[:20]:
            fid = h["pid"].get(p)
            if fid is None:
                continue
            lst = h["by_file"].get(fid, [])
            cs = lst[max(0, bisect.bisect_left(lst, self.cutoff) - 400):bisect.bisect_left(lst, self.cutoff)]
            if len(cs) < 3:
                continue
            cnt = Counter(g for c in cs for g in h["commits"][c][3] if g != fid)
            for g, n in cnt.items():
                if n < 2:
                    continue
                pr = n / len(cs)
                q = h["paths"][g]
                if q in self.tree and pr * min(1.0, 2 * sc) > rule_f[q]:
                    rule_f[q] = pr * min(1.0, 2 * sc)
                    why[q] = f"changes in {n} of {len(cs)} commits that change `{p}`"
        dmass = Counter()
        for p, sc in scored[:30]:
            dmass[p.rsplit("/", 1)[0] if "/" in p else ""] += sc
        for d, m in dmass.most_common(6):
            lst = h["by_dir"].get(d, [])
            cs = lst[max(0, bisect.bisect_left(lst, self.cutoff) - 400):bisect.bisect_left(lst, self.cutoff)]
            if len(cs) < 4:
                continue
            cnt = Counter(g for c in cs for g in h["commits"][c][3])
            for g, n in cnt.items():
                q = h["paths"][g]
                if n < 3 or q not in self.tree or (q.rsplit("/", 1)[0] if "/" in q else "") == d:
                    continue
                pr = n / len(cs) * min(1.0, m)
                if pr > rule_d[q]:
                    rule_d[q] = pr
                    if pr > rule_f.get(q, 0):
                        why[q] = f"changes in {n} of {len(cs)} commits that touch `{d}/`"
        return rule_f, rule_d, why

    def templates(self, scored, window=3000):
        """Cross-client templates: in past commits that touched one of the strongest crates
        and at least two others (a feature landing in several clients), how often each file
        of the *other* crates changed. Proposes the phone screen, desktop pane, bridge or
        client that such features usually also touch."""
        h = self.ix.hist
        mass = group_mass(scored, n=30)
        tot = sum(mass.values()) or 1.0
        active = {g: m / tot for g, m in mass.most_common(4) if m / tot >= 0.1}
        out, why, base = Counter(), {}, Counter()
        lo = max(0, self.cutoff - window)
        for c in range(lo, self.cutoff):
            fids = h["commits"][c][3]
            if len(fids) > BIG_COMMIT or len(fids) < 3:
                continue
            groups = {}
            for g in fids:
                groups.setdefault(group(h["paths"][g]), []).append(g)
            hit = [a for a in active if a in groups]
            if not hit or len(groups) < 3:
                continue
            w = max(active[a] for a in hit)
            for a in hit:
                base[a] += 1
            for gname, gf in groups.items():
                if gname in active:
                    continue
                for g in gf:
                    out[g] += w
        res = Counter()
        n = max(base.values()) if base else 0
        if n:
            for g, v in out.items():
                p = h["paths"][g]
                if p in self.tree:
                    res[p] = v / n
                    why[p] = f"changes when features land in `{max(active, key=active.get)}` and other clients"
        return res, why

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
        t1 = time.perf_counter()
        if not hasattr(self, "bid_to_path"):
            self.bid_to_path = defaultdict(list)
            for p_, sha in self.tree.items():
                b_ = self.ix.tokens.ids.get(sha)
                if b_ is not None:
                    self.bid_to_path[b_].append(p_)
        iface, iface_x, iface_why = self.interface_users(scored[:8])
        for q, _ in iface.most_common(150):
            new.add(q)
        self.tick("stage2_iface", t1)
        t1 = time.perf_counter()
        rule_f, rule_d, rule_why = self.rules(scored)
        for q, v in list(rule_f.items()) + list(rule_d.items()):
            if v >= 0.3:
                new.add(q)
        self.tick("stage2_rules", t1)
        t1 = time.perf_counter()
        tmpl, tmpl_why = self.templates(scored)
        for q, v in tmpl.most_common(120):
            new.add(q)
        self.tick("stage2_templates", t1)
        # the whole of the crates holding the most stage-1 confidence, so the ranker can
        # place every file of them (the map's crate lists)
        members = defaultdict(list)
        for p in self.paths:
            members[group(p)].append(p)
        crate_rank = {}
        for i, (g, _) in enumerate(group_mass(scored).most_common(4)):
            crate_rank[g] = i + 1
            if len(members[g]) <= 500:
                new.update(members[g])
        for p in new - set(feats):
            self.src["stage2"].add(p)
            feats[p] = self.base(p)
        for p, x in feats.items():
            fid = h["pid"].get(p)
            x["s1"] = s1.get(p, 0.0)
            x["s1_rank"] = math.log1p(rank1.get(p, 3000))
            x["co2"] = co2.get(fid, 0.0) if fid is not None else 0.0
            x["crate_mass"] = crate_mass.get(group(p), 0.0) / tot
            x["crate_rank"] = crate_rank.get(group(p), 9)
            x["dir_mass"] = dir_mass.get(p.rsplit("/", 1)[0] if "/" in p else "", 0.0) / tot
            x["pair2"] = pair2.get(p, (0.0, ""))[0]
            x["ref"] = ref.get(p, 0.0)
            x["iface"] = iface.get(p, 0.0)
            x["iface_x"] = iface_x.get(p, 0.0)
            x["rule_f"] = rule_f.get(p, 0.0)
            x["rule_d"] = rule_d.get(p, 0.0)
            x["tmpl"] = tmpl.get(p, 0.0)
            if x["tmpl"] >= 0.15:
                self.reason[p].append(tmpl_why[p])
            if x["iface"] > 0.1:
                self.reason[p].append(iface_why[p][1])
            if max(x["rule_f"], x["rule_d"]) >= 0.4:
                self.reason[p].append(rule_why[p])
            if x["ref"] > 0.1:
                self.reason[p].append(ref_why[p][1])
            x["src_stage2"] = float(p in self.src["stage2"])
            if p in pair2 and pair2[p][0] > 0.3:
                self.reason[p].append(pair2[p][1])
            if x["co2"] > 0.2:
                self.reason[p].append(f"changes with the top-ranked files (co {x['co2']:.2f})")
        for p, x in feats.items():
            if x.get("ref", 0) > 0:
                self.src["ref"].add(p)
            if x.get("iface", 0) > 0:
                self.src["iface"].add(p)
            if max(x.get("rule_f", 0), x.get("rule_d", 0)) >= 0.3:
                self.src["rule"].add(p)
            if x.get("crate_rank", 9) < 9:
                self.src["crate"].add(p)
            if x.get("tmpl", 0) >= 0.15:
                self.src["tmpl"].add(p)
        relative(feats, REL1 + REL2)
        self.tick("stage2", t0)
        return feats


REL1 = ["cos", "co", "co_max", "hist", "hist_max", "sim", "sim_max", "sym_ment", "lit", "recent_n"]
REL2 = ["co2", "s1", "ref", "iface", "iface_x", "rule_f", "rule_d", "tmpl"]


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


def group_mass(ranked, n=50, key=None):
    key = key or group
    mass = Counter()
    for p, s in ranked[:n]:
        mass[key(p)] += s
    return mass


def build_map(ranked, paths, k=100, c=3, key=None):
    """The map: the top k files, then every file of the c groups (crates) holding the
    most confidence, best first. Returns [(group or None, [paths])]."""
    key = key or group
    score = dict(ranked)
    top = [p for p, _ in ranked[:k]]
    seen = set(top)
    out = [(None, top)]
    members = defaultdict(list)
    for p in paths:
        members[key(p)].append(p)
    for g, _ in group_mass(ranked, key=key).most_common(c):
        rest = sorted((p for p in members.get(g, []) if p not in seen), key=lambda p: (-score.get(p, 0.0), p))
        seen.update(rest)
        out.append((g, rest))
    return out


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
    """One cache per repository, shared by all its worktrees (named after the main checkout)."""
    try:
        common = git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir").decode().strip()
        name = os.path.basename(os.path.dirname(common.rstrip("/")))
    except Exception:
        name = os.path.basename(os.path.abspath(repo))
    return os.path.expanduser(f"~/.cache/openagents/filefind/{name}")


MODEL_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "model.json")


def cmd_feedback(a):
    """Collect late files from agent runs into the index's feedback.jsonl: what an
    issue-run (`coder issue-run`) or a briefed-agent A/B trial changed, and the files it
    opened outside its briefing. Similar-issue lookups use them; a query on the same
    issue keeps them in its list."""
    cache = a.cache or default_cache(a.repo)
    path = os.path.join(cache, "feedback.jsonl")
    seen = set()
    if os.path.exists(path):
        for line in open(path):
            try:
                r = json.loads(line)
                seen.add((r["issue"], r["path"], r["kind"], r["source"]))
            except (ValueError, KeyError):
                pass
    rows = []
    import glob
    for d in a.issue_runs or []:
        for f in glob.glob(os.path.join(os.path.expanduser(d), "*", "summary.json")):
            try:
                r = json.load(open(f))
            except ValueError:
                continue
            src = "issue-run:" + os.path.basename(os.path.dirname(f))
            checks = (r.get("checks") or []) + (r.get("agent_checks") or [])
            if checks and all(c.get("ok") for c in checks) and not r.get("error"):
                for p in r.get("changed") or []:  # only a change whose checks passed
                    rows.append((r["issue"], p, "changed", src))
            for p in r.get("opened_outside_briefing") or []:
                rows.append((r["issue"], p, "read_outside", src))
    for d in a.ab or []:
        for f in glob.glob(os.path.join(os.path.expanduser(d), "**", "result.json"), recursive=True):
            try:
                r = json.load(open(f))
            except ValueError:
                continue
            src = "ab:" + os.path.relpath(os.path.dirname(f), d)
            if r.get("tests_pass"):
                for p in r.get("files_changed") or []:
                    rows.append((r["issue"], p, "changed", src))
            for m in r.get("misses") or []:
                p = m.get("file") if isinstance(m, dict) else m
                if p:
                    rows.append((r["issue"], p, "read_outside", src))
    new = [x for x in rows if x not in seen]
    with open(path, "a") as f:
        for issue, p, kind, src in new:
            f.write(json.dumps({"issue": int(issue), "path": p, "kind": kind, "source": src}) + "\n")
    print(f"feedback: {len(new)} new rows ({len(rows)} read) -> {path}")


def cmd_index(a):
    key = embed_key(required=False)
    if not key:
        print("no embeddings key: refreshing history and the token indexes only", file=sys.stderr)
    ix = Index(a.cache or default_cache(a.repo))
    t0 = time.time()
    if os.path.exists(os.path.join(ix.cache, "history.pkl")) and not a.full:
        ix.load(need_blobs=False)
        ix.refresh_history(a.repo, a.rev)
    else:
        ix.build_history(a.repo, a.rev)
    print(f"history: {len(ix.hist['commits'])} commits, {len(ix.hist['paths'])} paths "
          f"({time.time()-t0:.1f}s)", file=sys.stderr)
    try:
        n = ix.build_commit_vecs(key) if key else 0
        print(f"commit subjects embedded: {n} new", file=sys.stderr)
    except Exception as e:  # e.g. the embeddings account is out of credit: keep going
        print(f"commit subjects not embedded: {str(e)[:120]}", file=sys.stderr)
    issues = a.issues
    if not issues and not a.no_issues:
        issues = os.path.join(ix.cache, "closed-issues.json")
        with open(issues, "wb") as f:
            limit = a.issue_limit if not os.path.exists(os.path.join(ix.cache, "issues.pkl")) else 300
            f.write(subprocess.run(["gh", "issue", "list", "--state", "closed", "--limit", str(limit),
                                    "--json", "number,title,body,closedAt,createdAt"], cwd=a.repo,
                                   check=True, capture_output=True).stdout)
    if issues and key:
        p = os.path.join(ix.cache, "issues.pkl")
        if os.path.exists(p):
            ix.issues = pickle.load(open(p, "rb"))
        try:
            ix.build_issues(issues, key)
            print(f"issues: {len(ix.issues['numbers'])}", file=sys.stderr)
        except Exception as e:
            print(f"issues not embedded: {str(e)[:120]}", file=sys.stderr)
    ix.load_blobs()
    tree = worktree_tree(a.repo) if a.rev == "WORKTREE" else ls_tree(a.repo, a.rev)
    if key:
        try:
            ix.ensure_blobs(a.repo, tree, key)
            ix.save_blobs()
        except Exception as e:
            print(f"blobs not embedded: {str(e)[:120]}", file=sys.stderr)
    print(f"blobs: {len(ix.blob_rows)}", file=sys.stderr)
    TokenIndex(ix.cache).ensure(a.repo, tree)
    IfaceIndex(ix.cache).ensure(a.repo, tree)


def map_json(ranked, n, q):
    """The top n files grouped by crate (crates in order of the confidence they hold),
    each crate's files best first; files past the top k carry rank and confidence too."""
    rank = {p: i for i, (p, _) in enumerate(ranked, 1)}
    groups = defaultdict(list)
    for p, s in ranked[:n]:
        groups[group(p)].append((p, s))
    order = sorted(groups, key=lambda g: -sum(s for _, s in groups[g]))
    return {"paths": min(n, len(ranked)), "groups": [
        {"group": g, "confidence_mass": round(sum(s for _, s in groups[g]), 3),
         "files": [{"rank": rank[p], "path": p, "confidence": round(s, 4),
                    "stages": [s_ for s_ in STAGES_OUT if p in q.src[s_]]} for p, s in groups[g]]}
        for g in order]}


class _First:
    """A Future of a matrix whose first row is wanted."""

    def __init__(self, fut):
        self.fut = fut

    def result(self):
        try:
            return self.fut.result()[0]
        except Exception as e:  # no embeddings this time: the deterministic stages still run
            print(f"issue not embedded ({str(e)[:80]}); running without embeddings", file=sys.stderr)
            return np.zeros(DIMS, np.float32)


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
    key = embed_key(required=False)
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
    if ix.refresh_history(a.repo, a.rev) and key:  # new commits since the index: keep it fresh
        try:
            ix.build_commit_vecs(key)
        except Exception as e:
            print(f"new commit subjects not embedded ({str(e)[:80]})", file=sys.stderr)
        ix.hist.setdefault("by_dir", dirs_index(ix.hist))
    timing["refresh_history"] = time.perf_counter() - t0
    t0 = time.perf_counter()
    rev = a.rev
    tree = ls_tree(a.repo, rev)
    timing["tree"] = time.perf_counter() - t0
    t0 = time.perf_counter()
    if key:
        try:
            if ix.ensure_blobs(a.repo, tree, key, log=False):
                ix.save_blobs()
        except Exception as e:
            print(f"new blobs not embedded ({str(e)[:80]})", file=sys.stderr)
    ix.tokens.ensure(a.repo, tree, log=False)
    ix.iface.ensure(a.repo, tree, log=False)
    timing["refresh_index"] = time.perf_counter() - t0
    q = Query(ix, a.repo, rev, tree, exclude_issue=a.issue, timing=timing)
    feats = q.run(title, body, qvec)
    t0 = time.perf_counter()
    ranked_all, feats = rank_two_stage(model, q, feats)
    fb = ix.feedback.get(a.issue, {}) if a.issue else {}
    if fb:  # files earlier runs on this issue changed or had to open: keep them in the list
        conf = dict(ranked_all)
        for p, w in fb.items():
            if p in tree:
                conf[p] = max(conf.get(p, 0.0), 0.5 + 0.4 * w)
                q.src["feedback"].add(p)
                q.reason[p].insert(0, "an earlier agent run on this issue " +
                                   ("changed it" if w >= 1 else "opened it outside its briefing"))
        ranked_all = sorted(conf.items(), key=lambda kv: -kv[1])
    ranked = ranked_all[:a.k]
    timing["score"] = time.perf_counter() - t0
    timing["total"] = time.perf_counter() - t_all
    if a.json:
        print(json.dumps({
            "schema": "openagents.filefind.v1", "issue": a.issue, "title": title,
            "rev": git(a.repo, "rev-parse", rev).decode().strip(),
            "model": os.path.basename(a.model), "timing_ms": {k: round(v * 1000) for k, v in timing.items()},
            "files": [{"rank": i, "path": p, "confidence": round(s, 4),
                       "stages": [s_ for s_ in STAGES_OUT if p in q.src[s_]],
                       "reasons": tidy(q.reason.get(p, [])) or ["ranked by the scorer from weak signals"]}
                      for i, (p, s) in enumerate(ranked, 1)],
            "map": map_json(ranked_all, a.map, q) if a.map else None}, indent=1))
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
    i.add_argument("--full", action="store_true", help="rebuild the history index from scratch")
    i.add_argument("--cache")
    q = sub.add_parser("query", help="rank the files an issue needs")
    q.add_argument("--repo", default=".")
    q.add_argument("--rev", default="HEAD")
    g = q.add_mutually_exclusive_group(required=True)
    g.add_argument("--issue", type=int)
    g.add_argument("--text")
    q.add_argument("--k", type=int, default=100, help="files in the ranked list")
    q.add_argument("--map", type=int, default=400,
                   help="also print the map: the top N files grouped by crate (0: off)")
    q.add_argument("--json", action="store_true")
    q.add_argument("--cache")
    q.add_argument("--model", default=MODEL_PATH)
    fb = sub.add_parser("feedback", help="collect late files from issue-run and A/B runs")
    fb.add_argument("--repo", default=".")
    fb.add_argument("--cache")
    fb.add_argument("--issue-runs", nargs="*", default=["~/.openagents/coder-new/issue-runs"])
    fb.add_argument("--ab", nargs="*", default=[], help="briefed-agent A/B results directories")
    a = ap.parse_args()
    {"index": cmd_index, "query": cmd_query, "feedback": cmd_feedback}[a.cmd](a)


if __name__ == "__main__":
    main()

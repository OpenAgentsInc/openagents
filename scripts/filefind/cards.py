#!/usr/bin/env python3
"""filefind cards: per-file decisions made at index time (issue #11249).

The finder's query used to wait 15-60 s for per-file Clef decisions. This module
moves that model work to index time, so a query does one issue embedding, one
batched tag decision for the issue, and lookups.

At index time, once per file version (keyed by blob sha), incrementally:

  card     a short summary plus typed tags from a closed vocabulary mined from the
           repository: areas (feature area / capability), layers (route/handler,
           store, protocol/wire type, UI, CLI verb, test, doc, registry,
           generated, ...), clients (web, phone, desktop, CLI, worker), entities,
           and routes. One structured call per file on Vertex AI Gemini
           (`gemini-2.5-flash-lite`, Google first for anything paid). Routes are a
           bounded field read from the text and kept when they are in the mined
           route vocabulary. Files that are data or binary get a card without a
           model call (path and routes only).
  vector   the card text plus the file's defined identifiers, embedded with Vertex
           AI `text-embedding-005` (256 dims, RETRIEVAL_DOCUMENT).
  tower    a two-tower projection distilled offline (`train-tower`): the issue side
           is the issue vector plus its tag probabilities, the file side is the
           card vector, the file's path+head vector and its tags. Trained on the
           file-relevance-v1 corpus's training partition (#11215): labels are the
           outcomes (the fix changed the file); Clef's answers on the same items
           are a teacher field the loss also fits, never a label. At query time
           Clef's judgment costs one dot product per file (file sides are
           projected once per blob and cached).

At query time (`profile` + `features`): the issue's tag probabilities come from one
batched Jev decision (TypeSafe direct, TYPESAFE_API_KEY or ~/work/.secrets/typesafe.env;
choices over areas and entities, nouls per layer and client); then card cosine, tag
overlaps, route overlap and the distilled score are computed for every file of the
tree and fed to the ranker as features, and the best files by card cosine and by the
distilled score join the candidate pool (stage `card`).

Everything lives in the per-repository cache (`filefind.default_cache`, namespaced
by repository identity and workspace) under `cards/`:

  vocab.json                       the closed vocabulary, its source revision, digest
  cards.jsonl                      one card per blob (append-only; vocab digest on each)
  vecs<embedder tag>.npz           card vectors per blob
  tags.npz                         compiled per-blob tag matrix (rebuilt from cards.jsonl)
  issue-profiles.jsonl             Jev profiles of replayed issues (bench only)
  tower-<digest>.npz               file-side projections per blob for one tower

    python3 scripts/filefind/cards.py vocab --repo . --rev REV     # mine the vocabulary
    python3 scripts/filefind/cards.py index --repo . [--rev HEAD|--blobs FILE]
    python3 scripts/filefind/cards.py cost --repo .
"""
import argparse, hashlib, json, math, os, pickle, re, subprocess, sys, threading, time, urllib.error
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor

import numpy as np

import warnings

# Accelerate's float32 matmul raises spurious FP warnings on macOS (numpy 2); results are finite.
warnings.filterwarnings("ignore", category=RuntimeWarning, message=".*encountered in matmul")

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import filefind as ff  # noqa: E402

CARD_MODEL = os.environ.get("FILEFIND_CARD_MODEL", "gemini-2.5-flash-lite")
VOCAB_MODEL = os.environ.get("FILEFIND_VOCAB_MODEL", "gemini-2.5-flash")
CARD_SCHEMA = "openagents.filefind.card.v1"
VOCAB_SCHEMA = "openagents.filefind.card-vocab.v1"
CARD_HEAD_CHARS = 3000
# USD per million tokens on Vertex AI (list price, us-central1), for the cost report.
PRICE = {"gemini-2.5-flash-lite": (0.10, 0.40, 0.025), "gemini-2.5-flash": (0.30, 2.50, 0.075),
         "text-embedding-005": (0.025 / 4, 0.0, 0.0)}  # embeddings: $0.025 per 1M chars ~ 4 chars a token
TYPESAFE_URL = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = os.environ.get("FILEFIND_JEV_MODEL", "jev-latest")

LAYERS = {
    "route_handler": "an HTTP/RPC route table or request handler",
    "store": "persistence: database tables, queries, storage, caches",
    "protocol": "protocol or wire types: request/response/event shapes, schemas, serialization contracts",
    "ui": "user interface: views, screens, components, panes, styles",
    "cli_verb": "a command-line command or subcommand",
    "test": "tests",
    "doc": "documentation for people",
    "registry": "wiring that lists other parts: module lists, registries, catalogs, manifests",
    "generated": "generated output (OpenAPI documents, tree.json, codegen)",
    "core_logic": "domain logic or library code",
    "config_build": "build, CI, deploy or runtime configuration",
    "script": "a script or developer tool",
    "fixture": "fixtures, test data, recorded traces",
}
CLIENTS = {
    "web": "the web app (openagents.com pages, browser JS/CSS)",
    "phone": "the phone apps (iOS/Android, mobile bridge)",
    "desktop": "the desktop app",
    "cli": "a command-line client",
    "worker": "the server side: the openagents.com Worker/API, gateway, hosted services",
}
# A route is a quoted string literal that reads as a URL path of two or more segments
# (`"/v1/coder/memory"`, `'/api/v1/systemone'`); a bounded field read from text.
ROUTE = re.compile(r"""["'`](/[A-Za-z0-9_\-{}:]+(?:/[A-Za-z0-9_\-{}:.*]+)+/?)(?:\?[^"'`]*)?["'`]""")
ROUTE_FIRST_SKIP = {"usr", "tmp", "etc", "var", "bin", "dev", "home", "users", "private", "opt", "proc",
                    "lib", "src", "node_modules", "__pycache__", "tests", "test", "crates", "apps", "docs",
                    "scripts", "packages", "target", "assets", "library", "applications", "system", "volumes",
                    "mnt", "root", "run", "sys", "nix", "workspace", "work", "data", "app", "srv"}
TEXT_EXT = {"rs", "ts", "tsx", "js", "mjs", "cjs", "py", "sh", "md", "toml", "swift", "kt", "kts", "css",
            "html", "sql", "yaml", "yml", "php", "wgsl", "go", "java", "c", "h", "m", "mm", "proto",
            "graphql", "vue", "svelte", "jsonc", "garden", "service", "nix", "zsh", "fish", "ps1"}
TEXT_NAMES = {"Dockerfile", "Makefile", "justfile", "Procfile", "package.json", "tsconfig.json",
              "wrangler.json", "app.json", "Cargo.toml"}
SKIP_DIRS = ("node_modules/", "vendor/", "target/", ".git/")


def _sha(b):
    return "sha256:" + hashlib.sha256(b if isinstance(b, bytes) else b.encode()).hexdigest()


def card_dir(cache):
    d = os.path.join(cache, "cards")
    os.makedirs(d, exist_ok=True)
    return d


# ---------------------------------------------------------------- model calls

class Gemini:
    """generateContent on Vertex AI with a JSON response schema. Retries 429/5xx."""

    def __init__(self, model):
        self.model = model
        project = os.environ.get("VERTEX_PROJECT") or "openagentsgemini"
        loc = os.environ.get("FILEFIND_CARD_LOCATION") or "us-central1"
        self.url = (f"https://{loc}-aiplatform.googleapis.com/v1/projects/{project}/locations/{loc}"
                    f"/publishers/google/models/{model}:generateContent")
        self.token = ff.GoogleToken()
        self.usage = Counter()
        self.lock = threading.Lock()

    def __call__(self, prompt, schema, max_tokens=400, temperature=0.0, timeout=120):
        body = {"contents": [{"role": "user", "parts": [{"text": prompt}]}],
                "generationConfig": {"responseMimeType": "application/json", "responseSchema": schema,
                                     "temperature": temperature, "maxOutputTokens": max_tokens}}
        if "2.5" in self.model and "lite" not in self.model:
            body["generationConfig"]["thinkingConfig"] = {"thinkingBudget": 0}
        for attempt in range(8):
            try:
                d = ff._http_json(self.url, body, {"Authorization": f"Bearer {self.token.get()}",
                                                   "Content-Type": "application/json"}, timeout=timeout)
                u = d.get("usageMetadata", {})
                with self.lock:
                    self.usage["calls"] += 1
                    self.usage["in"] += u.get("promptTokenCount", 0)
                    self.usage["cached"] += u.get("cachedContentTokenCount", 0)
                    self.usage["out"] += u.get("candidatesTokenCount", 0) + u.get("thoughtsTokenCount", 0)
                text = d["candidates"][0]["content"]["parts"][0]["text"]
                return json.loads(text)
            except urllib.error.HTTPError as e:
                if e.code not in (429, 500, 502, 503, 504) or attempt == 7:
                    raise
            except (KeyError, IndexError, ValueError, OSError):
                if attempt == 7:
                    raise
            time.sleep(min(60, 2 ** attempt))

    def dollars(self):
        pin, pout, pcache = PRICE.get(self.model, (0.1, 0.4, 0.025))
        u = self.usage
        return ((u["in"] - u["cached"]) * pin + u["cached"] * pcache + u["out"] * pout) / 1e6


def typesafe_key():
    k = os.environ.get("TYPESAFE_API_KEY")
    if k:
        return k.strip()
    for p in ("~/work/.secrets/typesafe.env",):
        p = os.path.expanduser(p)
        if os.path.exists(p):
            for line in open(p):
                if line.startswith("TYPESAFE_API_KEY="):
                    return line.split("=", 1)[1].strip().strip('"')
    return None


# ---------------------------------------------------------------- vocabulary

def _group_descriptions(repo, rev, tree):
    groups = {}
    for p in tree:
        parts = p.split("/")
        if len(parts) > 2 and parts[0] in ("crates", "bins", "apps", "packages", "clients"):
            groups.setdefault("/".join(parts[:2]), [])
    for g in groups:
        desc = ""
        for name in ("Cargo.toml", "package.json", "README.md"):
            sha = tree.get(f"{g}/{name}")
            if not sha:
                continue
            text = ff.git(repo, "cat-file", "-p", sha).decode("utf-8", "replace")
            if name == "Cargo.toml":
                m = re.search(r'^description\s*=\s*"([^"]+)"', text, re.M)
                desc = m.group(1) if m else ""
            elif name == "package.json":
                try:
                    desc = json.loads(text).get("description") or ""
                except ValueError:
                    desc = ""
            else:
                lines = [l.strip() for l in text.splitlines() if l.strip() and not l.startswith(("#", "!", "<", "["))]
                desc = lines[0][:200] if lines else ""
            if desc:
                break
        groups[g] = desc
    return groups


def mine_routes(repo, tree, min_df=2, cap=400):
    """Route prefixes (up to three segments) that occur in at least min_df text files."""
    shas = {s: p for p, s in tree.items() if eligible_text(p)}
    heads = ff.cat_heads(repo, sorted(shas), n=1 << 18, cap=1 << 18)
    df = Counter()
    for s, t in heads.items():
        if t:
            df.update(set(file_routes(t)))
    keep = [r for r, n in df.most_common() if n >= min_df]
    return keep[:cap]


def norm_route(r):
    segs = [x for x in r.strip("/").split("/") if x]
    if len(segs) < 2 or "." in segs[0] or segs[0].lower() in ROUTE_FIRST_SKIP or segs[0].startswith((":", "{")):
        return None
    if any("." in x for x in segs[:3]):  # file paths, not routes
        return None
    segs = [("{}" if (x.startswith(("{", ":")) or re.fullmatch(r"[0-9a-f\-]{8,}|\d+", x)) else x.lower())
            for x in segs]
    return "/" + "/".join(segs[:3])


def file_routes(text):
    out = set()
    for m in ROUTE.findall(text[:200000]):
        if len(m) <= 80:
            r = norm_route(m)
            if r:
                out.add(r)
    return out


def cmd_vocab(a):
    """Mine the closed vocabulary at --rev: areas and entities by one Gemini call over
    the repository's groups (with their descriptions), its docs directories and the
    last 400 commit subjects before --rev; routes from the tree; layers and clients
    fixed. Changing the vocabulary means re-carding every blob."""
    cache = ff.cache_for(a)
    rev = ff.git(a.repo, "rev-parse", a.rev).decode().strip()
    tree = ff.ls_tree(a.repo, rev)
    groups = _group_descriptions(a.repo, rev, tree)
    docs = sorted({"/".join(p.split("/")[:2]) for p in tree if p.startswith("docs/") and p.count("/") >= 2})
    subjects = ff.git(a.repo, "log", "--format=%s", "-n", "400", rev).decode("utf-8", "replace").splitlines()
    prompt = (
        "You are building a closed tag vocabulary for a code search index over one software repository.\n"
        "From the repository's parts, docs directories and recent commit subjects below, produce:\n"
        "1. `areas`: 50-90 feature areas or capabilities of the product (e.g. billing, chat attachments, "
        "account memory, schedules), each an id in snake_case with a one-line definition. Areas are "
        "capabilities a change request is about, not file kinds and not crate names.\n"
        "2. `entities`: 80-150 domain entities the code names (e.g. account, environment, conversation, "
        "api_key, payout), each an id in snake_case with a one-line definition.\n"
        "Cover the whole repository; ids must be distinct.\n\n"
        "PARTS (path: description)\n" + "\n".join(f"{g}: {d}" for g, d in sorted(groups.items())) +
        "\n\nDOCS DIRECTORIES\n" + "\n".join(docs) +
        "\n\nRECENT COMMIT SUBJECTS\n" + "\n".join(subjects))
    item = {"type": "OBJECT", "properties": {"id": {"type": "STRING"}, "definition": {"type": "STRING"}},
            "required": ["id", "definition"]}
    schema = {"type": "OBJECT", "properties": {"areas": {"type": "ARRAY", "items": item},
                                               "entities": {"type": "ARRAY", "items": item}},
              "required": ["areas", "entities"]}
    g = Gemini(VOCAB_MODEL)
    got = g(prompt, schema, max_tokens=12000, temperature=0.0, timeout=900)
    # second pass: what the change history is about that the parts list missed
    hist = ff.git(a.repo, "log", "--format=%s", "-n", "2500", rev).decode("utf-8", "replace").splitlines()
    more = g("A closed tag vocabulary for a code search index over one repository has these ids.\n"
             "AREAS: " + ", ".join(x["id"] for x in got["areas"]) + "\nENTITIES: " +
             ", ".join(x["id"] for x in got["entities"]) + "\n\nFrom the commit subjects below, return only "
             "the feature areas (up to 40) and domain entities (up to 40) that changes keep touching and the "
             "lists miss (e.g. accounts, sign-in, attachments, memory, environments, schedules), each an id in "
             "snake_case with a one-line definition. Do not repeat ids or near-synonyms already listed.\n\n"
             "COMMIT SUBJECTS\n" + "\n".join(hist), schema, max_tokens=8000, temperature=0.0, timeout=900)
    got = {k: got[k] + more.get(k, []) for k in ("areas", "entities")}

    def clean(xs, cap):
        out, seen = [], set()
        for x in xs:
            i = re.sub(r"[^a-z0-9_]", "_", x["id"].strip().lower()).strip("_")
            if i and i not in seen:
                seen.add(i)
                out.append({"id": i, "definition": x["definition"].strip()[:160]})
        return out[:cap]
    vocab = {"schema": VOCAB_SCHEMA, "rev": rev, "model": VOCAB_MODEL,
             "areas": clean(got["areas"], 130), "entities": clean(got["entities"], 220),
             "layers": [{"id": k, "definition": v} for k, v in LAYERS.items()],
             "clients": [{"id": k, "definition": v} for k, v in CLIENTS.items()],
             "routes": mine_routes(a.repo, tree)}
    vocab["digest"] = vocab_digest(vocab)
    path = os.path.join(card_dir(cache), "vocab.json")
    ff.atomic(path, lambda f: f.write(json.dumps(vocab, indent=1).encode()))
    print(f"vocab {vocab['digest'][:19]}: {len(vocab['areas'])} areas, {len(vocab['entities'])} entities, "
          f"{len(vocab['routes'])} routes at {rev[:10]} (${g.dollars():.3f}) -> {path}")


def vocab_digest(v):
    core = {k: v[k] for k in ("areas", "entities", "layers", "clients", "routes")}
    return _sha(json.dumps(core, sort_keys=True))


def load_vocab(cache):
    p = os.path.join(cache, "cards", "vocab.json")
    return json.load(open(p)) if os.path.exists(p) else None


KINDS = ("areas", "entities", "layers", "clients")


def tag_columns(vocab):
    """Column order of the tag matrix: areas, entities, layers, clients."""
    cols = []
    for k in KINDS:
        cols += [(k, x["id"]) for x in vocab[k]]
    return cols


# ---------------------------------------------------------------- cards

def eligible_text(path):
    """Whether a file gets a model-written card (else a card from its path and routes only):
    source, docs and config text, not recorded bench output, vendored code or lockfiles."""
    if path.startswith(SKIP_DIRS) or "/node_modules/" in path or path.startswith(("bench/", "third_party/")):
        return False
    name = path.rsplit("/", 1)[-1]
    if name in ff.LOCK_NAMES:
        return False
    if name in TEXT_NAMES:
        return True
    ext = name.rsplit(".", 1)[-1] if "." in name else ""
    return ext in TEXT_EXT


def card_prompt_prefix(vocab):
    def block(k):  # entity ids name themselves; their definitions would double the prompt
        if k == "entities":
            return ", ".join(x["id"] for x in vocab[k])
        return "\n".join(f"- {x['id']}: {x['definition']}" for x in vocab[k])
    return ("You write index cards for files of one software repository, for a code search system "
            "that maps change requests to the files they need.\n"
            "For the file below return: `summary`, at most 40 words on what the file does and what it "
            "is for (no filler); `areas`, the 1-3 feature areas it serves; `layers`, the 1-2 layers it "
            "is; `clients`, every client it is part of or directly serves (empty when none); "
            "`entities`, up to 6 domain entities it handles. Use only the ids listed.\n\n"
            "AREAS\n" + block("areas") + "\n\nLAYERS\n" + block("layers") + "\n\nCLIENTS\n" + block("clients") +
            "\n\nENTITIES\n" + block("entities") + "\n\n")


def card_schema(vocab):
    def arr(k):  # ids are checked against the vocabulary after the call (enums this large are refused)
        return {"type": "ARRAY", "items": {"type": "STRING"}}
    return {"type": "OBJECT", "properties": {"summary": {"type": "STRING"}, "areas": arr("areas"),
                                             "layers": arr("layers"), "clients": arr("clients"),
                                             "entities": arr("entities")},
            "required": ["summary", "areas", "layers", "clients", "entities"]}


class CardStore:
    """Cards per blob in the per-repository cache. Append-only JSONL plus compiled arrays."""

    def __init__(self, cache, key=None):
        self.cache = cache
        self.dir = card_dir(cache)
        self.vocab = load_vocab(cache)
        self.key = key if key is not None else ff.embed_key(required=False, cache=cache)
        self.tag = ff.emb_tag(self.key)
        self.cards = None
        self.rows, self.vecs = {}, np.zeros((0, ff.DIMS), np.float16)
        self.tags = None  # compiled: {"rows": {sha: i}, "hot": uint8 [n, cols], "routes": [set]}

    # -- raw cards
    def load_cards(self):
        self.cards = {}
        p = os.path.join(self.dir, "cards.jsonl")
        if os.path.exists(p) and self.vocab:
            for line in open(p):
                try:
                    c = json.loads(line)
                except ValueError:
                    continue
                if c.get("vocab") == self.vocab["digest"]:
                    self.cards[c["blob"]] = c
        return self.cards

    def missing(self, blobs):
        if self.cards is None:
            self.load_cards()
        return {s: p for s, p in blobs.items() if s not in self.cards}

    def make(self, repo, blobs, workers=48, log=True, tokens=None):
        """Card every blob of blobs {sha: path} that has no card for the current vocabulary."""
        if not self.vocab:
            raise SystemExit("no card vocabulary in this cache: run `cards.py vocab` first")
        todo = self.missing(blobs)
        if not todo:
            return {"cards": 0, "dollars": 0.0, "seconds": 0.0}
        t0 = time.time()
        g = Gemini(CARD_MODEL)
        prefix, schema = card_prompt_prefix(self.vocab), card_schema(self.vocab)
        routes_v = set(self.vocab["routes"])
        allowed = {k: {x["id"] for x in self.vocab[k]} for k in KINDS}
        out = open(os.path.join(self.dir, "cards.jsonl"), "a")
        lock = threading.Lock()
        shas = sorted(todo)
        done = [0]
        llm = [0]

        def one(chunk):
            heads = ff.cat_heads(repo, chunk, n=1 << 18, cap=1 << 18)
            for s in chunk:
                path = todo[s]
                text = heads.get(s)
                card = {"v": CARD_SCHEMA, "blob": s, "path": path, "vocab": self.vocab["digest"],
                        "summary": "", "areas": [], "layers": [], "clients": [], "entities": [], "routes": [],
                        "model": None}
                if text is not None:
                    card["routes"] = sorted(file_routes(text) & routes_v)
                if text is not None and text.strip() and eligible_text(path):
                    try:
                        r = g(prefix + f"FILE: {path}\n```\n{text[:CARD_HEAD_CHARS]}\n```", schema, max_tokens=300)
                        card["summary"] = str(r.get("summary", ""))[:400]
                        for k in KINDS:
                            card[k] = sorted({x for x in r.get(k, []) if x in allowed[k]})
                        card["model"] = CARD_MODEL
                        with lock:
                            llm[0] += 1
                    except Exception as e:  # noqa: BLE001 - leave it uncarded; the next index retries
                        if log:
                            print(f"card failed for {path}: {str(e)[:100]}", file=sys.stderr)
                        continue
                with lock:
                    out.write(json.dumps(card, sort_keys=True) + "\n")
                    self.cards[s] = card
                    done[0] += 1
                    if log and done[0] % 2000 == 0:
                        el = time.time() - t0
                        print(f"carded {done[0]}/{len(todo)} ({el:.0f}s, ${g.dollars():.2f})", file=sys.stderr,
                              flush=True)
        chunks = [shas[i:i + 8] for i in range(0, len(shas), 8)]
        with ThreadPoolExecutor(workers) as ex:
            list(ex.map(one, chunks))
        out.close()
        el = time.time() - t0
        rep = {"cards": done[0], "model_calls": llm[0], "seconds": round(el, 1), "dollars": round(g.dollars(), 4),
               "usage": dict(g.usage), "model": CARD_MODEL}
        with open(os.path.join(self.dir, "cost.jsonl"), "a") as f:
            f.write(json.dumps(dict(rep, at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), what="cards")) + "\n")
        return rep

    # -- vectors
    def vec_path(self):
        return os.path.join(self.dir, f"vecs{self.tag}.npz")

    def load_vecs(self):
        p = self.vec_path()
        if os.path.exists(p):
            z = np.load(p)
            self.rows = {s: i for i, s in enumerate(z["shas"].tolist())}
            self.vecs = z["vecs"]
        return self

    def save_vecs(self):
        shas = np.array(sorted(self.rows, key=self.rows.get), dtype="S40")
        ff.atomic(self.vec_path(), lambda f: np.savez(f, shas=shas, vecs=self.vecs))

    def card_text(self, c, idents=()):
        parts = [f"FILE {c['path']}", c.get("summary", "")]
        for k in KINDS:
            if c.get(k):
                parts.append(f"{k}: " + ", ".join(c[k]))
        if c.get("routes"):
            parts.append("routes: " + " ".join(c["routes"][:12]))
        if idents:
            parts.append("identifiers: " + " ".join(idents[:40]))
        return "\n".join(p for p in parts if p)[:2400]

    def embed(self, repo, blobs, tokens=None, log=True):
        """Embed the card text of every carded blob of blobs that has no vector."""
        if self.cards is None:
            self.load_cards()
        if not self.rows:
            self.load_vecs()
        todo = [s for s in sorted(blobs) if s in self.cards and s.encode() not in self.rows and s not in self.rows]
        if not todo or not self.key:
            return {"vectors": 0, "seconds": 0.0, "dollars": 0.0}
        t0 = time.time()
        texts = []
        for s in todo:
            idents = []
            if tokens is not None:
                b = tokens.ids.get(s)
                if b is not None:
                    idents = sorted(set(tokens.defined_in(b)))
            texts.append(self.card_text(self.cards[s], idents))
        chars = sum(len(t) for t in texts)
        m = ff.embed(texts, self.key, task="RETRIEVAL_DOCUMENT").astype(np.float16)
        base = len(self.rows)
        self.rows = {(k.decode() if isinstance(k, bytes) else k): v for k, v in self.rows.items()}
        for i, s in enumerate(todo):
            self.rows[s] = base + i
        self.vecs = np.concatenate([self.vecs, m]) if len(self.vecs) else m
        self.save_vecs()
        rep = {"vectors": len(todo), "seconds": round(time.time() - t0, 1),
               "dollars": round(chars / 1e6 * 0.025, 4), "chars": chars}
        with open(os.path.join(self.dir, "cost.jsonl"), "a") as f:
            f.write(json.dumps(dict(rep, at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), what="vectors")) + "\n")
        if log:
            print(f"card vectors: {len(todo)} in {rep['seconds']}s", file=sys.stderr)
        return rep

    # -- compiled tags
    def tags_path(self):
        return os.path.join(self.dir, "tags.npz")

    def compile_tags(self):
        if self.cards is None:
            self.load_cards()
        cols = tag_columns(self.vocab)
        ci = {c: i for i, c in enumerate(cols)}
        rindex = {r: i for i, r in enumerate(self.vocab["routes"])}
        shas = sorted(self.cards)
        hot = np.zeros((len(shas), len(cols)), np.uint8)
        rr, rc = [], []
        has_model = np.zeros(len(shas), np.uint8)
        for i, s in enumerate(shas):
            c = self.cards[s]
            for k in KINDS:
                for x in c.get(k, []):
                    j = ci.get((k, x))
                    if j is not None:
                        hot[i, j] = 1
            for r in c.get("routes", []):
                if r in rindex:
                    rr.append(i)
                    rc.append(rindex[r])
            has_model[i] = 1 if c.get("model") else 0
        arr = dict(shas=np.array(shas, dtype="S40"), hot=hot, route_r=np.array(rr, np.int32),
                   route_c=np.array(rc, np.int32), has_model=has_model,
                   vocab=np.array([self.vocab["digest"]], dtype="S80"))
        ff.atomic(self.tags_path(), lambda f: np.savez(f, **arr))
        return len(shas)

    def load_tags(self):
        p = self.tags_path()
        if not os.path.exists(p):
            return None
        z = np.load(p)
        if self.vocab is None or z["vocab"][0].decode() != self.vocab["digest"]:
            return None
        n = len(z["shas"])
        routes = [None] * n
        rset = defaultdict(set)
        for r, c in zip(z["route_r"].tolist(), z["route_c"].tolist()):
            rset[r].add(c)
        self.tags = {"rows": {s: i for i, s in enumerate(z["shas"].tolist())}, "hot": z["hot"],
                     "routes": rset, "has_model": z["has_model"]}
        return self.tags

    def load(self):
        self.load_vecs()
        self.load_tags()
        return self


# ---------------------------------------------------------------- issue profile (Jev)

def profile_questions(vocab):
    def crit(k):
        return {x["id"]: x["definition"] for x in vocab[k]}
    q = {"area": {"type": "choice",
                  "instructions": "Which feature area of this repository does the change request in `issue` mainly need changed?",
                  "criteria": crit("areas")},
         "entity": {"type": "choice",
                    "instructions": "Which domain entity does the change request in `issue` mainly change?",
                    "criteria": crit("entities")}}
    for x in vocab["layers"]:
        q["layer:" + x["id"]] = {"type": "noul", "instructions":
                                 f"Will a complete fix for the change request in `issue` need to change {x['definition']}?"}
    for x in vocab["clients"]:
        q["client:" + x["id"]] = {"type": "noul", "instructions":
                                  f"Will a complete fix for the change request in `issue` need to change {x['definition']}?"}
    return q


def profile_issue(vocab, title, body, key=None, timeout=10):
    """One batched Jev decision: probabilities over areas and entities (choices), and a
    noul per layer and client. Routes named in the issue are a bounded field read from
    the text. Returns {"areas": [...], "entities": [...], "layers": [...], "clients": [...],
    "routes": [...], "seconds": s} or None when Jev is unreachable."""
    key = key or typesafe_key()
    text = ff.issue_text(title, body)
    routes = sorted(file_routes(text) & set(vocab["routes"]))
    if not key:
        return None
    t0 = time.perf_counter()
    body_ = {"model": JEV_MODEL, "state": {"issue": text[:4000]}, "questions": profile_questions(vocab)}
    for attempt in range(3):
        try:
            d = ff._http_json(TYPESAFE_URL, body_, {"Authorization": f"Bearer {key}",
                                                     "Content-Type": "application/json"}, timeout=timeout)
            break
        except Exception:  # noqa: BLE001
            if attempt == 2:
                return None
            time.sleep(0.5 * (attempt + 1))
    ans = d["answers"]
    out = {"routes": routes, "seconds": round(time.perf_counter() - t0, 3), "model": d.get("model")}
    out["areas"] = [float(ans["area"]["probabilities"].get(x["id"], 0.0)) for x in vocab["areas"]]
    out["entities"] = [float(ans["entity"]["probabilities"].get(x["id"], 0.0)) for x in vocab["entities"]]
    out["layers"] = [float(ans["layer:" + x["id"]]["noul"]) for x in vocab["layers"]]
    out["clients"] = [float(ans["client:" + x["id"]]["noul"]) for x in vocab["clients"]]
    return out


class Profiles:
    """Cached Jev profiles of replayed issues (bench), keyed by vocab digest and issue text."""

    def __init__(self, cache, vocab):
        self.path = os.path.join(card_dir(cache), "issue-profiles.jsonl")
        self.vocab = vocab
        self.by = {}
        self.lock = threading.Lock()
        if os.path.exists(self.path):
            for line in open(self.path):
                try:
                    r = json.loads(line)
                    self.by[r["k"]] = r["profile"]
                except (ValueError, KeyError):
                    pass

    def key(self, title, body):
        return _sha(self.vocab["digest"] + "\n" + ff.issue_text(title, body))

    def get(self, title, body, fetch=True):
        k = self.key(title, body)
        if k in self.by or not fetch:
            return self.by.get(k)
        p = profile_issue(self.vocab, title, body, timeout=30)
        if p is not None:
            with self.lock:
                self.by[k] = p
                with open(self.path, "a") as f:
                    f.write(json.dumps({"k": k, "profile": p}) + "\n")
        return p


def profile_vector(vocab, prof):
    """The issue side's tag part: areas, entities, layers, clients probabilities."""
    n = sum(len(vocab[k]) for k in KINDS)
    if not prof:
        return np.zeros(n, np.float32)
    return np.array(prof["areas"] + prof["entities"] + prof["layers"] + prof["clients"], np.float32)


# ---------------------------------------------------------------- two-tower

def tower_inputs_doc(card_vecs, blob_vecs, hot):
    return np.concatenate([card_vecs.astype(np.float32), blob_vecs.astype(np.float32),
                           hot.astype(np.float32)], axis=1)


def tower_inputs_query(qvec, pvec):
    return np.concatenate([np.asarray(qvec, np.float32), pvec]).astype(np.float32)


def tower_skips(Q, D, e=ff.DIMS):
    """The fixed similarities the tower corrects: issue . card vector, issue . path+head
    vector, issue tag probabilities . file tags (rows of Q and D paired)."""
    return np.stack([(Q[:, :e] * D[:, :e]).sum(1), (Q[:, :e] * D[:, e:2 * e]).sum(1),
                     (Q[:, e:] * D[:, 2 * e:]).sum(1)], axis=1)


def tower_score(tower, qin, dproj, skips):
    """Distilled score (a logit) of one issue against files whose sides are projected
    (dproj [n, k]) with their skip similarities (skips [n, 3]): one dot product each."""
    qp = qin @ np.asarray(tower["Wq"], np.float32)
    return skips @ np.asarray(tower["a"], np.float32) + dproj @ qp + float(tower["b"])


def tower_project(tower, din):
    return din @ np.asarray(tower["Wd"], np.float32)


def train_tower(Q, D, y, teacher, groups, k=32, lam=0.5, epochs=60, lr=3e-3, wd=1e-2, seed=0, log=True):
    """Fit s = a . skips(q, d) + (q Wq).(d Wd) + b: the card, path+head and tag similarities
    with learned weights, plus a learned low-rank correction. BCE on the outcome y, and,
    where `teacher` is finite, squared error to Clef's logit (weight lam). Early stopping
    on 10% of the groups. Pure numpy, Adam."""
    rng = np.random.default_rng(seed)
    ug = np.unique(groups)
    val_g = set(rng.choice(ug, max(1, len(ug) // 10), replace=False).tolist())
    val = np.array([g in val_g for g in groups])
    dq, dd = Q.shape[1], D.shape[1]
    e = ff.DIMS if dq > ff.DIMS else dq // 2
    S = tower_skips(Q, D, e) if dq > ff.DIMS else np.zeros((len(Q), 3), np.float32)
    P = {"Wq": rng.normal(0, 0.01, (dq, k)).astype(np.float32),
         "Wd": rng.normal(0, 0.01, (dd, k)).astype(np.float32),
         "a": np.array([4.0, 4.0, 1.0], np.float32), "b": np.array([-2.0], np.float32)}
    M = {n: np.zeros_like(v) for n, v in P.items()}
    V = {n: np.zeros_like(v) for n, v in P.items()}
    has_t = np.isfinite(teacher)
    tch = np.where(has_t, np.clip(teacher, -8, 8), 0.0).astype(np.float32)
    pos_w = float((y == 0).sum() / max(1, (y == 1).sum()))
    w = np.where(y > 0, min(pos_w, 10.0), 1.0).astype(np.float32)

    def fwd(idx):
        qp, dp = Q[idx] @ P["Wq"], D[idx] @ P["Wd"]
        s = S[idx] @ P["a"] + (qp * dp).sum(1) + P["b"][0]
        p = 1 / (1 + np.exp(-s))
        bce = -(w[idx] * (y[idx] * np.log(p + 1e-7) + (1 - y[idx]) * np.log(1 - p + 1e-7))).sum() / w[idx].sum()
        mse = (has_t[idx] * (s - tch[idx]) ** 2).sum() / max(1, has_t[idx].sum())
        return bce + lam * mse, bce, mse, s, p, qp, dp
    best, best_P, bad, t = 1e9, None, 0, 0
    tri, vai = np.where(~val)[0], np.where(val)[0]
    hist = []
    for ep in range(epochs):
        rng.shuffle(tri)
        for i in range(0, len(tri), 512):
            idx = tri[i:i + 512]
            _, _, _, s, p, qp, dp = fwd(idx)
            g = w[idx] * (p - y[idx]) / w[idx].sum()
            g = (g + lam * 2 * has_t[idx] * (s - tch[idx]) / max(1, has_t[idx].sum())).astype(np.float32)
            grads = {"Wq": Q[idx].T @ (g[:, None] * dp) + wd * P["Wq"],
                     "Wd": D[idx].T @ (g[:, None] * qp) + wd * P["Wd"],
                     "a": S[idx].T @ g, "b": np.array([g.sum()], np.float32)}
            t += 1
            for n in P:
                M[n] = 0.9 * M[n] + 0.1 * grads[n]
                V[n] = 0.999 * V[n] + 0.001 * grads[n] ** 2
                P[n] -= lr * (M[n] / (1 - 0.9 ** t)) / (np.sqrt(V[n] / (1 - 0.999 ** t)) + 1e-8)
        vl, vb, vm, *_ = fwd(vai)
        hist.append(round(float(vl), 4))
        if log:
            print(f"tower epoch {ep}: val loss {vl:.4f} (bce {vb:.4f}, teacher mse {vm:.3f})", file=sys.stderr)
        if vl < best - 1e-4:
            best, best_P, bad = vl, {n: v.copy() for n, v in P.items()}, 0
        else:
            bad += 1
            if bad >= 5:
                break
    out = {"Wq": best_P["Wq"].round(5).tolist(), "Wd": best_P["Wd"].round(5).tolist(),
           "a": best_P["a"].round(5).tolist(), "b": float(best_P["b"][0]), "k": k, "lam": lam, "e": e,
           "val_loss": float(best), "val_series": hist, "epochs_run": len(hist)}
    out["digest"] = _sha(json.dumps([out["Wq"], out["Wd"], out["a"], out["b"]]))
    return out


# ---------------------------------------------------------------- query-time features

CARD_FEATURES = ["card_cos", "card_area", "card_entity", "card_layer", "card_client", "card_route",
                 "card_tag", "card_has", "tt"]
CARD_REL = ["card_cos", "card_tag", "tt"]


class CardView:
    """Card features of every file of one tree, for one issue. Built once per query:
    a handful of matrix products over the tree's rows."""

    def __init__(self, store, ix, paths, tree, qvec, prof, tower=None, tower_cache=None):
        self.paths = paths
        n = len(paths)
        v = store.vocab
        tags = store.tags or {"rows": {}, "hot": np.zeros((0, len(tag_columns(v))), np.uint8), "routes": {}}
        crow = np.array([store.rows.get(tree[p], store.rows.get(tree[p].encode(), -1)) if store.rows else -1
                         for p in paths])
        trow = np.array([tags["rows"].get(tree[p].encode(), -1) for p in paths])
        brow = np.array([ix.blob_rows.get(tree[p].encode(), -1) for p in paths])
        self.cos = np.zeros(n, np.float32)
        ok = crow >= 0
        if ok.any() and qvec is not None:
            self.cos[ok] = store.vecs[crow[ok]].astype(np.float32) @ np.asarray(qvec, np.float32)
        hot = np.zeros((n, tags["hot"].shape[1]), np.float32)
        tok = trow >= 0
        hot[tok] = tags["hot"][trow[tok]]
        self.has = tok.astype(np.float32)
        na, ne, nl = len(v["areas"]), len(v["entities"]), len(v["layers"])
        pv = profile_vector(v, prof)
        self.area = hot[:, :na] @ pv[:na]
        self.entity = hot[:, na:na + ne] @ pv[na:na + ne]
        lay = hot[:, na + ne:na + ne + nl]
        self.layer = (lay @ pv[na + ne:na + ne + nl]) / np.maximum(1, lay.sum(1))
        cl = hot[:, na + ne + nl:]
        self.client = (cl * pv[na + ne + nl:]).max(1) if cl.shape[1] else np.zeros(n)
        self.route = np.zeros(n, np.float32)
        if prof and prof.get("routes"):
            want = {v["routes"].index(r) for r in prof["routes"] if r in v["routes"]}
            for i in np.where(tok)[0]:
                got = tags["routes"].get(int(trow[i]))
                if got:
                    self.route[i] = len(want & got)
        self.tagscore = self.area + self.entity + 0.5 * self.layer + 0.5 * self.client + self.route
        self.tt = np.zeros(n, np.float32)
        if tower is not None and prof is not None and qvec is not None and ok.any():
            qin = tower_inputs_query(qvec, pv)
            have = ok & (brow >= 0)
            if tower_cache is not None:
                dp = tower_cache.rows_for([tree[p] for p in paths])
                have = have & dp[1]
                dproj = dp[0][have]
            else:
                din = tower_inputs_doc(store.vecs[crow[have]], ix.blob_mat[brow[have]], hot[have])
                dproj = tower_project(tower, din)
            bcos = ix.blob_mat[brow[have]].astype(np.float32) @ np.asarray(qvec, np.float32)
            skips = np.stack([self.cos[have], bcos, hot[have] @ pv], axis=1)
            self.tt[have] = tower_score(tower, qin, dproj, skips)
            self.tt[~have] = float(np.min(self.tt[have])) if have.any() else 0.0
        self.ok = ok

    def top(self, which, k):
        a = {"cos": self.cos, "tt": self.tt, "tag": self.tagscore}[which]
        if not self.ok.any():
            return []
        idx = np.argsort(-a)[:k]
        return [int(i) for i in idx if self.ok[i]]

    def feats(self, i):
        return {"card_cos": float(self.cos[i]), "card_area": float(self.area[i]), "card_entity": float(self.entity[i]),
                "card_layer": float(self.layer[i]), "card_client": float(self.client[i]),
                "card_route": float(self.route[i]), "card_tag": float(self.tagscore[i]),
                "card_has": float(self.has[i]), "tt": float(self.tt[i])}


class TowerCache:
    """File-side projections of one tower per blob, cached in the card directory."""

    def __init__(self, store, ix, tower):
        self.store, self.ix, self.tower = store, ix, tower
        self.path = os.path.join(store.dir, f"tower-{tower['digest'].split(':')[1][:12]}.npz")
        self.rows, self.mat = {}, np.zeros((0, tower["k"]), np.float32)
        if os.path.exists(self.path):
            z = np.load(self.path)
            self.rows = {s: i for i, s in enumerate(z["shas"].tolist())}
            self.mat = z["mat"].astype(np.float32)

    def ensure(self, shas):
        st, ix = self.store, self.ix
        tags = st.tags
        todo = [s for s in sorted(set(shas)) if s.encode() not in self.rows
                and (s in st.rows or s.encode() in st.rows) and s.encode() in ix.blob_rows
                and s.encode() in tags["rows"]]
        if not todo:
            return 0
        cv = st.vecs[[st.rows.get(s, st.rows.get(s.encode())) for s in todo]]
        bv = ix.blob_mat[[ix.blob_rows[s.encode()] for s in todo]]
        hot = tags["hot"][[tags["rows"][s.encode()] for s in todo]]
        proj = tower_project(self.tower, tower_inputs_doc(cv, bv, hot)).astype(np.float16)
        base = len(self.rows)
        for i, s in enumerate(todo):
            self.rows[s.encode()] = base + i
        self.mat = np.concatenate([self.mat, proj.astype(np.float32)]) if len(self.mat) else proj.astype(np.float32)
        shas_ = np.array(sorted(self.rows, key=self.rows.get), dtype="S40")
        ff.atomic(self.path, lambda f: np.savez(f, shas=shas_, mat=self.mat.astype(np.float16)))
        return len(todo)

    def rows_for(self, shas):
        idx = np.array([self.rows.get(s.encode(), -1) for s in shas])
        ok = idx >= 0
        out = np.zeros((len(shas), self.mat.shape[1] if len(self.mat) else self.tower["k"]), np.float32)
        out[ok] = self.mat[idx[ok]]
        return out, ok


def cards_on(model):
    """Whether queries and the index run the card stage: when the active model was trained
    with card features (its `cards` block), or FILEFIND_CARDS=1 (experiments, bench)."""
    return bool((model or {}).get("cards")) or os.environ.get("FILEFIND_CARDS") == "1"


def model_tower(model, store):
    """The model card's tower when it was trained on this cache's vocabulary."""
    c = (model or {}).get("cards") or {}
    if not c.get("tower") or not store.vocab or c.get("vocab_digest") != store.vocab["digest"]:
        return None
    return c["tower"]


# ---------------------------------------------------------------- CLI

def refresh(repo, cache, tree, key=None, workers=48, log=True, model=None):
    """Card, embed and compile every blob of tree that lacks it (the post-merge hook and
    `filefind.py index` call this). Returns a cost report."""
    st = CardStore(cache, key)
    if not st.vocab:
        if log:
            print("cards: no vocabulary in this cache (cards.py vocab); skipped", file=sys.stderr)
        return None
    blobs = {}
    for p, s in tree.items():
        blobs.setdefault(s, p)
    t0 = time.time()
    rep = {"cards": st.make(repo, blobs, workers=workers, log=log)}
    tok = ff.TokenIndex(cache)
    rep["vectors"] = st.embed(repo, blobs, tokens=tok, log=log)
    if rep["cards"]["cards"] or not os.path.exists(st.tags_path()):
        st.compile_tags()
    st.load_tags()
    ix = None
    tower = model_tower(model, st)
    if tower is not None:
        ix = ff.Index(cache, st.key)
        ix.load_blobs()
        st.load_vecs()
        rep["tower"] = TowerCache(st, ix, tower).ensure(list(blobs))
    rep["seconds"] = round(time.time() - t0, 1)
    rep["dollars"] = round(rep["cards"].get("dollars", 0) + rep["vectors"].get("dollars", 0), 4)
    if log:
        print(f"cards: +{rep['cards']['cards']} cards, +{rep['vectors']['vectors']} vectors in {rep['seconds']}s "
              f"(${rep['dollars']:.3f})", file=sys.stderr)
    return rep


def cmd_index(a):
    cache = ff.cache_for(a)
    if a.blobs:  # bench replays: every blob of every replayed tree, {sha: path}
        blobs = pickle.load(open(a.blobs, "rb"))
        st = CardStore(cache)
        t0 = time.time()
        print(json.dumps(st.make(a.repo, blobs, workers=a.workers)))
        print(json.dumps(st.embed(a.repo, blobs, tokens=ff.TokenIndex(cache))))
        st.compile_tags()
        print(f"total {time.time()-t0:.0f}s")
        return
    model = json.load(open(a.model)) if a.model and os.path.exists(a.model) else None
    tree = ff.worktree_tree(a.repo) if a.rev == "WORKTREE" else ff.ls_tree(a.repo, a.rev)
    print(json.dumps(refresh(a.repo, cache, tree, workers=a.workers, model=model)))


def cmd_cost(a):
    cache = ff.cache_for(a)
    p = os.path.join(card_dir(cache), "cost.jsonl")
    tot = Counter()
    for line in open(p):
        r = json.loads(line)
        tot[r["what"] + "_dollars"] += r.get("dollars", 0)
        tot[r["what"] + "_seconds"] += r.get("seconds", 0)
        tot[r["what"]] += r.get("cards", r.get("vectors", 0))
    print(json.dumps(dict(tot), indent=1))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("vocab", "index", "cost"):
        p = sub.add_parser(name)
        p.add_argument("--repo", default=".")
        p.add_argument("--cache")
        p.add_argument("--workspace")
        p.add_argument("--rev", default="HEAD")
        p.add_argument("--workers", type=int, default=48)
        p.add_argument("--blobs", help="index: a pickle {sha: path} of blobs to card (bench replays)")
        p.add_argument("--model", default=ff.MODEL_PATH)
    a = ap.parse_args()
    a.repo = os.path.abspath(a.repo)
    {"vocab": cmd_vocab, "index": cmd_index, "cost": cmd_cost}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())

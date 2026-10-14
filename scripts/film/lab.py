#!/usr/bin/env python3
"""Video Lab: versioned cuts, shots and scenes, local renders, and the tools a
chat edits them with (docs/video-lab/README.md, docs/video-lab/mvp.md).

    scripts/film/lab.py serve [--port 8792]          the service the chats call
    scripts/film/lab.py import                       Episode 290 and the portal walk
    scripts/film/lab.py list
    scripts/film/lab.py open DOC
    scripts/film/lab.py show DOC [--from T] [--to T]
    scripts/film/lab.py frame DOC AT [--version N]
    scripts/film/lab.py edit DOC PATCH.json --summary "..." [--author NAME]
    scripts/film/lab.py versions DOC
    scripts/film/lab.py render DOC [--quality preview|review|master] [--version N]
    scripts/film/lab.py compare DOC [--a N] [--b N] [--at T]
    scripts/film/lab.py notes DOC

A document is typed JSON: `cut.v1` (scripts/film/cut.py's cut list), `shot.v1`
(a generated shot: studio.py, a film script, a take) or `scene.v1` (a Verse
scene for the verse film_shot example); schemas in docs/video-lab/schemas/.
Every edit is a new version (the JSON, a one-line summary, who made it, and a
digest of the JSON plus every file it uses), kept under
~/work/film-out/lab/<project>/<doc>/vN/ (VIDEO_LAB_ROOT), never in git.

`edit` is the only write: a JSON Patch (RFC 6902; `/shots/@name/...` picks the
shot by name) checked against the schema and `cut.py check` before it is
saved. A patch that adds a problem is refused with the reason in plain words.

Renders are cached by version digest and quality; one render runs at a time.
A render that uses private assets (MetaHuman, Fab packs, Berkeley Mono, see
lab_private.json) is marked not for training and never leaves this Mac: the
service binds 127.0.0.1 only and has no upload path.

The tools (lab_open, lab_show, lab_frame, lab_edit, lab_render, lab_compare,
lab_notes, lab_versions, lab_list) are POST /api/tool/<name> with JSON
arguments; the web chat, Coder and `openagents lab` all call them there.
Stdlib only, plus Pillow and numpy for frames; ffmpeg on PATH.
"""

from __future__ import annotations

import argparse
import copy
import fcntl
import hashlib
import html
import importlib.util
import json
import mimetypes
import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, quote, urlparse

HERE = Path(__file__).resolve().parent
REPO = Path(os.environ.get("OPENAGENTS_REPO", HERE.parents[1]))
FILM = REPO / "scripts/film"
SCHEMAS = REPO / "docs/video-lab/schemas"
HOME = Path.home()
ROOT = Path(os.path.expanduser(os.environ.get("VIDEO_LAB_ROOT", "~/work/film-out/lab")))
PROJECT = os.environ.get("VIDEO_LAB_PROJECT", "films")
REVIEWS = HOME / "work/film-out/reviews"
PORT = int(os.environ.get("VIDEO_LAB_PORT", "8792"))
KINDS = ("cut.v1", "shot.v1", "scene.v1")
QUALITIES = {
    # name: (what it is, for a cut: the frame size, or None for the cut's own)
    "preview": ("quick look, 960x540", (960, 540)),
    "review": ("1080p", (1920, 1080)),
    "master": ("full size, with a ProRes master", None),
}
LOCK = threading.Lock()


def now() -> float:
    return time.time()


def clock(t: float) -> str:
    m, s = divmod(max(t, 0.0), 60)
    return f"{int(m)}:{s:04.1f}" if s % 1 else f"{int(m)}:{int(s):02d}"


def expand(p: str) -> Path:
    return Path(os.path.expanduser(p))


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


class Refused(Exception):
    """A request the Lab won't do, with the reason in plain words."""


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_CUT = None


def cutmod():
    global _CUT
    if _CUT is None:
        _CUT = load_module("film_cut", FILM / "cut.py")
    return _CUT


# ---- schema ---------------------------------------------------------------------

def schema(kind: str) -> dict:
    return json.loads((SCHEMAS / f"{kind}.json").read_text())


TYPES = {"object": dict, "array": list, "string": str, "boolean": bool}


def where(path: str) -> str:
    return path or "the document"


def validate(value, sch: dict, root: dict, path: str = "") -> list[str]:
    """A small JSON Schema subset, with plain reasons."""
    if "$ref" in sch:
        sch = root["$defs"][sch["$ref"].split("/")[-1]]
    out = []
    t = sch.get("type")
    if t == "number":
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            return [f"{where(path)} must be a number"]
    elif t == "integer":
        if isinstance(value, bool) or not isinstance(value, int):
            return [f"{where(path)} must be a whole number"]
    elif t and not isinstance(value, TYPES[t]):
        return [f"{where(path)} must be {'an' if t[0] in 'aeiou' else 'a'} {t}"]
    if "enum" in sch and value not in sch["enum"]:
        out.append(f"{where(path)} must be one of {', '.join(map(str, sch['enum']))}")
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        if "minimum" in sch and value < sch["minimum"]:
            out.append(f"{where(path)} must be at least {sch['minimum']}")
        if "exclusiveMinimum" in sch and value <= sch["exclusiveMinimum"]:
            out.append(f"{where(path)} must be more than {sch['exclusiveMinimum']}")
        if "maximum" in sch and value > sch["maximum"]:
            out.append(f"{where(path)} must be at most {sch['maximum']}")
    if isinstance(value, str) and len(value) < sch.get("minLength", 0):
        out.append(f"{where(path)} can't be empty")
    if isinstance(value, list):
        if len(value) < sch.get("minItems", 0):
            out.append(f"{where(path)} needs at least {sch['minItems']} item(s)")
        if "maxItems" in sch and len(value) > sch["maxItems"]:
            out.append(f"{where(path)} takes at most {sch['maxItems']} items")
        if "items" in sch:
            for i, item in enumerate(value):
                label = item.get("name") if isinstance(item, dict) and isinstance(item.get("name"), str) else None
                out += validate(item, sch["items"], root, f"{path}[{i}]" + (f" ({label})" if label else ""))
    if isinstance(value, dict):
        for key in sch.get("required", []):
            if key not in value:
                out.append(f"{where(path)} needs '{key}'")
        props = sch.get("properties", {})
        for key, v in value.items():
            if key in props:
                out += validate(v, props[key], root, f"{path}.{key}" if path else key)
            elif sch.get("additionalProperties") is False:
                near = [k for k in props if k.startswith(key[:2]) or key.startswith(k)]
                hint = f" (did you mean {near[0]}?)" if near else ""
                out.append(f"{where(path)} has '{key}', which it doesn't take{hint}")
            elif isinstance(sch.get("additionalProperties"), dict):
                out += validate(v, sch["additionalProperties"], root, f"{path}.{key}")
        if "anyOf" in sch and not any(all(k in value for k in alt.get("required", [])) for alt in sch["anyOf"]):
            keys = " or ".join("'" + "', '".join(alt["required"]) + "'" for alt in sch["anyOf"])
            out.append(f"{where(path)} needs {keys}")
    return out


# ---- private assets ---------------------------------------------------------------

def private_rules() -> list[dict]:
    path = FILM / "lab_private.json"
    return json.loads(path.read_text())["rules"] if path.exists() else []


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for v in value.values():
            yield from strings(v)
    elif isinstance(value, list):
        for v in value:
            yield from strings(v)


def assets(kind: str, body: dict) -> list[str]:
    """Every file the document uses, as written (paths with ~)."""
    found = []
    for s in strings(body):
        if (s.startswith("~/") or s.startswith("/")) and not s.endswith("/"):
            if Path(s).suffix and expand(s).exists():
                found.append(s)
        elif kind == "shot.v1" and s.startswith("scripts/") and (REPO / s).exists():
            found.append(s)
    return sorted(set(found))


def private(kind: str, body: dict) -> list[str]:
    """Why a render of this document must stay on this Mac and out of training."""
    reasons = []
    for s in list(strings(body)):
        if "/" not in s:
            continue  # paths only; a caption that says "Berkeley" is not an asset
        low = s.lower()
        for rule in private_rules():
            if any(m in low for m in rule["match"]) and rule["reason"] not in reasons:
                reasons.append(rule["reason"])
    return reasons


# ---- digests ---------------------------------------------------------------------

def file_digest(path: Path) -> str:
    """sha256 of a file, cached by size and mtime."""
    cache_path = ROOT / ".assets.json"
    st = path.stat()
    key = str(path)
    with LOCK:
        cache = json.loads(cache_path.read_text()) if cache_path.exists() else {}
        hit = cache.get(key)
        if hit and hit["size"] == st.st_size and hit["mtime"] == st.st_mtime:
            return hit["sha256"]
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    with LOCK:
        cache = json.loads(cache_path.read_text()) if cache_path.exists() else {}
        cache[key] = {"size": st.st_size, "mtime": st.st_mtime, "sha256": h.hexdigest()}
        ROOT.mkdir(parents=True, exist_ok=True)
        cache_path.write_text(json.dumps(cache, indent=1))
    return h.hexdigest()


def digest(kind: str, body: dict) -> tuple[str, dict]:
    files = {}
    for a in assets(kind, body):
        p = expand(a) if not a.startswith("scripts/") else REPO / a
        files[a] = file_digest(p)
    blob = json.dumps({"kind": kind, "body": body, "assets": files}, sort_keys=True, separators=(",", ":"))
    return "sha256:" + hashlib.sha256(blob.encode()).hexdigest(), files


# ---- the store -------------------------------------------------------------------

def doc_dir(doc: str) -> Path:
    return ROOT / PROJECT / doc


def documents() -> list[dict]:
    base = ROOT / PROJECT
    out = []
    if base.exists():
        for d in sorted(base.iterdir()):
            if (d / "doc.json").exists():
                meta = json.loads((d / "doc.json").read_text())
                meta["versions"] = len(versions(d.name))
                out.append(meta)
    return out


def resolve(doc) -> str:
    """A document id from what the chat passed: its id or its title."""
    if not isinstance(doc, str) or not doc.strip():
        raise Refused("Say which document: " + ", ".join(d["id"] for d in documents()))
    want = slug(doc)
    for d in documents():
        if want in (d["id"], slug(d["title"])) or want in [slug(a) for a in d.get("aliases", [])]:
            return d["id"]
    raise Refused(f"There's no document called '{doc}'. There are: "
                  + ", ".join(f"{d['id']} ({d['title']})" for d in documents()))


def doc_meta(doc: str) -> dict:
    return json.loads((doc_dir(doc) / "doc.json").read_text())


def versions(doc: str) -> list[dict]:
    d = doc_dir(doc)
    out = []
    for v in sorted(d.glob("v*/version.json"), key=lambda p: int(p.parent.name[1:])):
        out.append(json.loads(v.read_text()))
    return out


def version(doc: str, n=None) -> dict:
    vs = versions(doc)
    if not vs:
        raise Refused(f"{doc} has no versions yet")
    if n in (None, "", "latest"):
        return vs[-1]
    if isinstance(n, str):
        n = n.lstrip("vV")
        if n.lower() == "previous":
            if len(vs) < 2:
                raise Refused(f"{doc} has only one version")
            return vs[-2]
    try:
        n = int(n)
    except (TypeError, ValueError):
        raise Refused(f"'{n}' isn't a version; {doc} has v1 to v{len(vs)}")
    for v in vs:
        if v["n"] == n:
            return v
    raise Refused(f"{doc} has no v{n}; it has v1 to v{len(vs)}")


def body(doc: str, n) -> dict:
    return json.loads((doc_dir(doc) / f"v{n}/doc.json").read_text())


def save_version(doc: str, kind: str, data: dict, summary: str, author: str, parent=None, extra=None) -> dict:
    d = doc_dir(doc)
    n = len(versions(doc)) + 1
    vd = d / f"v{n}"
    vd.mkdir(parents=True, exist_ok=False)
    dg, files = digest(kind, data)
    meta = {"doc": doc, "n": n, "kind": kind, "summary": summary.strip(), "author": author,
            "created": now(), "parent": parent, "digest": dg, "assets": files,
            "private": private(kind, data), **(extra or {})}
    (vd / "doc.json").write_text(json.dumps(data, indent=2) + "\n")
    (vd / "version.json").write_text(json.dumps(meta, indent=2) + "\n")
    return meta


def create(doc: str, kind: str, title: str, data: dict, summary: str, author: str, **extra) -> dict:
    d = doc_dir(doc)
    d.mkdir(parents=True, exist_ok=True)
    meta = {"id": doc, "kind": kind, "title": title, "created": now(), **extra}
    (d / "doc.json").write_text(json.dumps(meta, indent=2) + "\n")
    return save_version(doc, kind, data, summary, author)


# ---- checks ----------------------------------------------------------------------

def timeline(data: dict) -> list[dict]:
    t, out = 0.0, []
    shots = data.get("shots", [])
    for i, s in enumerate(shots):
        out.append({"i": i, "name": s.get("name"), "start": t, "end": t + s.get("dur", 0)})
        t += s.get("dur", 0)
    return out


def score_args(data: dict) -> dict | None:
    score = data.get("score")
    if not score:
        return None
    tl = {s["name"]: s for s in timeline(data)}
    total = tl and max(s["end"] for s in tl.values())
    cuts = [tl[n]["start"] for n in score["cuts_at"] if n in tl]
    end = tl[score["end_at"]]["start"] if score.get("end_at") in tl else total - 8.0
    return {"seconds": round(total, 3), "cuts": [round(c, 3) for c in cuts], "end": round(end, 3)}


def check(kind: str, data: dict) -> tuple[list[str], list[str]]:
    """Problems (which refuse an edit) and notes (which don't)."""
    problems = validate(data, schema(kind), schema(kind))
    if problems:
        return problems, []
    notes = []
    if kind == "cut.v1":
        names = [s["name"] for s in data["shots"]]
        for n in sorted({n for n in names if names.count(n) > 1}):
            problems.append(f"two shots are called {n}; shot names must be different")
        score = data.get("score")
        if score:
            for n in score["cuts_at"] + ([score["end_at"]] if score.get("end_at") else []):
                if n not in names:
                    problems.append(f"the score marks a shot called {n}, which the cut doesn't have")
        for i, s in enumerate(data["shots"]):
            if s.get("xfade", 0) > s["dur"]:
                problems.append(f"{s['name']}: its crossfade ({s['xfade']}s) is longer than the shot ({s['dur']}s)")
        test = copy.deepcopy(data)
        if score:
            test.pop("music", None)  # Lab remakes the score to the cut's length when it renders
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(test, f)
        try:
            r = subprocess.run([sys.executable, str(FILM / "cut.py"), "check", f.name, "--json"],
                               capture_output=True, text=True)
        finally:
            os.unlink(f.name)
        try:
            res = json.loads(r.stdout.strip().splitlines()[-1])
        except (ValueError, IndexError):
            return problems + [f"the cut couldn't be checked: {(r.stderr or r.stdout)[-400:].strip()}"], notes
        problems += res["problems"]
        notes += [n for n in res["notes"] if "upscaled" not in n]
    elif kind == "shot.v1":
        r = data["renderer"]
        if r == "studio" and "spec" not in data:
            problems.append("a studio shot needs its spec")
        if r == "film-script":
            if "script" not in data or "args" not in data:
                problems.append("a film-script shot needs 'script' and 'args'")
            elif not (REPO / data["script"]).exists():
                problems.append(f"{data['script']} isn't in the repository")
            elif "{out}" not in data["args"]:
                problems.append("the script's args need {out}, where the render goes")
        for name, p in data.get("inputs", {}).items():
            if not expand(p).exists():
                problems.append(f"input {name} is missing: {p}")
    elif kind == "scene.v1":
        keys = data["shot"]["rig"]["keys"]
        ts = [k["t"] for k in keys]
        if ts != sorted(ts):
            problems.append("the camera keys must be in time order")
        if ts and ts[-1] > data["shot"]["duration"] + 1e-6:
            problems.append(f"a camera key at {ts[-1]}s is past the scene's end ({data['shot']['duration']}s)")
    return problems, notes


# ---- JSON Patch ------------------------------------------------------------------

def pointer(doc_data, path: str):
    if path == "":
        return []
    if not path.startswith("/"):
        raise Refused(f"'{path}' isn't a JSON Pointer; it starts with /, like /shots/@s4-delegation/dur")
    parts = [p.replace("~1", "/").replace("~0", "~") for p in path[1:].split("/")]
    # `@name` picks the array item with that name.
    out, cur = [], doc_data
    for p in parts:
        if isinstance(cur, list) and p.startswith("@"):
            idx = next((i for i, x in enumerate(cur) if isinstance(x, dict) and x.get("name") == p[1:]), None)
            if idx is None:
                names = [x.get("name") for x in cur if isinstance(x, dict)]
                raise Refused(f"there's no shot called {p[1:]}; the shots are {', '.join(map(str, names))}")
            p = str(idx)
        out.append(p)
        if isinstance(cur, list):
            cur = cur[int(p)] if p.isdigit() and int(p) < len(cur) else None
        elif isinstance(cur, dict):
            cur = cur.get(p)
        else:
            cur = None
    return out


def walk(data, parts):
    cur = data
    for p in parts[:-1]:
        if isinstance(cur, list):
            if not p.isdigit() or int(p) >= len(cur):
                raise Refused(f"there's no item {p} at /{'/'.join(parts)}")
            cur = cur[int(p)]
        elif isinstance(cur, dict):
            if p not in cur:
                raise Refused(f"/{'/'.join(parts)}: '{p}' isn't there")
            cur = cur[p]
        else:
            raise Refused(f"/{'/'.join(parts)} goes inside a value that has no fields")
    return cur


def get(data, parts):
    if not parts:
        return data
    parent = walk(data, parts)
    last = parts[-1]
    if isinstance(parent, list):
        if not last.isdigit() or int(last) >= len(parent):
            raise Refused(f"there's no item {last} at /{'/'.join(parts)}")
        return parent[int(last)]
    if last not in parent:
        raise Refused(f"/{'/'.join(parts)} isn't set")
    return parent[last]


def apply_patch(data: dict, ops) -> dict:
    if isinstance(ops, str):
        try:
            ops = json.loads(ops)
        except ValueError as e:
            raise Refused(f"the patch isn't JSON ({e})")
    if isinstance(ops, dict):
        ops = [ops]
    if not isinstance(ops, list) or not ops:
        raise Refused("the patch is a list of operations, like "
                      '[{"op": "replace", "path": "/shots/@s4-delegation/dur", "value": 12.5}]')
    out = copy.deepcopy(data)
    for op in ops:
        if not isinstance(op, dict) or op.get("op") not in ("add", "remove", "replace", "move", "copy", "test"):
            raise Refused(f"{json.dumps(op)[:120]} isn't a patch operation (add, remove, replace, move, copy or test)")
        parts = pointer(out, op.get("path", ""))
        if op["op"] in ("add", "replace", "test") and "value" not in op:
            raise Refused(f"{op['op']} {op.get('path')} needs a value")
        if op["op"] == "test":
            if get(out, parts) != op["value"]:
                raise Refused(f"{op['path']} is {json.dumps(get(out, parts))}, not {json.dumps(op['value'])}")
            continue
        if op["op"] in ("move", "copy"):
            value = copy.deepcopy(get(out, pointer(out, op.get("from", ""))))
            if op["op"] == "move":
                src = pointer(out, op["from"])
                parent = walk(out, src)
                del parent[int(src[-1]) if isinstance(parent, list) else src[-1]]
                parts = pointer(out, op["path"])
            op = {"op": "add", "path": op["path"], "value": value}
        if not parts:
            raise Refused("a patch can't replace the whole document")
        parent = walk(out, parts)
        last = parts[-1]
        if isinstance(parent, list):
            if op["op"] == "add":
                idx = len(parent) if last == "-" else int(last) if last.isdigit() else None
                if idx is None or idx > len(parent):
                    raise Refused(f"can't add at {op['path']}")
                parent.insert(idx, op["value"])
            else:
                if not last.isdigit() or int(last) >= len(parent):
                    raise Refused(f"there's no item {last} at {op['path']}")
                if op["op"] == "remove":
                    del parent[int(last)]
                else:
                    parent[int(last)] = op["value"]
        elif isinstance(parent, dict):
            if op["op"] == "add":
                parent[last] = op["value"]
            elif last not in parent:
                raise Refused(f"{op['path']} isn't set, so it can't be {'removed' if op['op'] == 'remove' else 'replaced'}"
                              + ("; use add" if op["op"] == "replace" else ""))
            elif op["op"] == "remove":
                del parent[last]
            else:
                parent[last] = op["value"]
        else:
            raise Refused(f"{op['path']} goes inside a value that has no fields")
    return out


def ripple(old: dict, new: dict) -> int:
    """Captions move with the shot they're under when shots change length."""
    before = {s["name"]: s for s in timeline(old)}
    after = {s["name"]: s for s in timeline(new)}
    moved = 0
    for c in new.get("captions", []):
        under = next((s for s in before.values() if s["start"] - 1e-6 <= c["at"] < s["end"] - 1e-6), None)
        if under and under["name"] in after:
            shift = after[under["name"]]["start"] - under["start"]
            if abs(shift) > 1e-6:
                c["at"] = round(c["at"] + shift, 3)
                moved += 1
    return moved


# ---- times -----------------------------------------------------------------------

def seconds(at, data: dict | None = None) -> float:
    """A time from what the chat passed: seconds, m:ss, h:mm:ss, or a shot's name (its start)."""
    if isinstance(at, (int, float)) and not isinstance(at, bool):
        return float(at)
    if not isinstance(at, str) or not at.strip():
        raise Refused("say a time, like 1:26 or 86")
    s = at.strip().rstrip("s")
    m = re.fullmatch(r"(?:(\d+):)?(\d+):(\d+(?:\.\d+)?)", s)
    if m:
        return int(m[1] or 0) * 3600 + int(m[2]) * 60 + float(m[3])
    try:
        return float(s)
    except ValueError:
        pass
    if data and data.get("shots"):
        for sh in timeline(data):
            if sh["name"] == at.strip():
                return sh["start"]
    raise Refused(f"'{at}' isn't a time; say 1:26, 86, or a shot's name")


def length(kind: str, data: dict) -> float:
    if kind == "cut.v1":
        return sum(s["dur"] for s in data["shots"])
    if kind == "scene.v1":
        return float(data["shot"]["duration"])
    return float(data.get("duration", 0) or 0)


# ---- renders ---------------------------------------------------------------------

def render_path(doc: str, v: dict, quality: str) -> Path:
    return doc_dir(doc) / "renders" / f"{v['digest'][7:23]}-{quality}.mp4"


def render_info(doc: str, v: dict, quality: str) -> dict | None:
    p = render_path(doc, v, quality)
    side = p.with_suffix(".json")
    if p.exists() and side.exists():
        return json.loads(side.read_text())
    return None


def best_render(doc: str, v: dict) -> tuple[Path, str] | None:
    for q in ("review", "master", "preview"):
        if render_info(doc, v, q):
            return render_path(doc, v, q), q
    return None


def run(cmd, env=None, cwd=None, log=None):
    r = subprocess.run(cmd, capture_output=True, text=True, env=env, cwd=cwd)
    if log:
        log.write(f"$ {' '.join(map(str, cmd))}\n{r.stdout[-4000:]}{r.stderr[-4000:]}\n")
    if r.returncode:
        tail = (r.stderr or r.stdout).strip().splitlines()[-6:]
        raise RuntimeError(f"{Path(str(cmd[1] if cmd[0] == sys.executable else cmd[0])).name} failed: " + " | ".join(tail))
    return r


def make_score(data: dict, log) -> str:
    args = score_args(data)
    key = hashlib.sha256(json.dumps(args, sort_keys=True).encode()).hexdigest()[:16]
    out = ROOT / "scores" / f"score-{key}.wav"
    if not out.exists():
        out.parent.mkdir(parents=True, exist_ok=True)
        tmp = out.with_suffix(".part.wav")
        run([sys.executable, str(FILM / "music.py"), str(tmp), "--seconds", str(args["seconds"]),
             "--cuts", ",".join(str(c) for c in args["cuts"]), "--end", str(args["end"])], log=log)
        tmp.rename(out)
    return str(out)


def scaled(src: Path, out: Path, size, log):
    run(["ffmpeg", "-v", "error", "-y", "-i", str(src), "-vf",
         f"scale={size[0]}:{size[1]}:force_original_aspect_ratio=decrease:flags=lanczos,pad={size[0]}:{size[1]}:-1:-1",
         "-c:v", "libx264", "-crf", "20", "-preset", "fast", "-c:a", "copy", "-movflags", "+faststart", str(out)],
        log=log)


def do_render(doc: str, n: int, quality: str, log) -> Path:
    v = version(doc, n)
    data = body(doc, n)
    kind = v["kind"]
    out = render_path(doc, v, quality)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmpdir = Path(tempfile.mkdtemp(prefix="video-lab-"))
    started = now()
    try:
        part = tmpdir / f"render{out.suffix}"
        if kind == "cut.v1":
            cut = copy.deepcopy(data)
            if cut.get("score"):
                cut["music"] = make_score(cut, log)
            cut["sheets"] = str(doc_dir(doc) / "sheets" / f"v{n}-{quality}")
            spec = tmpdir / "cut.json"
            spec.write_text(json.dumps(cut))
            cmd = [sys.executable, str(FILM / "cut.py"), "render", str(spec), "--out", str(part), "--force"]
            size = QUALITIES[quality][1]
            if size:
                cmd += ["--size", f"{size[0]}x{size[1]}"]
            if quality == "master":
                cmd.append("--master")
            run(cmd, log=log)
            if quality == "master" and part.with_suffix(".mov").exists():
                shutil.move(part.with_suffix(".mov"), out.with_suffix(".mov"))
        elif kind == "shot.v1" and data["renderer"] == "film-script":
            vals = {k: str(expand(p)) for k, p in data.get("inputs", {}).items()}
            full = tmpdir / "full.mp4"
            vals["out"] = str(full if quality == "preview" else part)
            args = [a.format(**vals) for a in data["args"]]
            run([sys.executable, str(REPO / data["script"]), *args], log=log)
            if quality == "preview":
                scaled(full, part, (960, 540), log)
        elif kind == "shot.v1" and data["renderer"] == "studio":
            spec = tmpdir / "shot.json"
            spec.write_text(json.dumps(data["spec"]))
            blender = os.environ.get("BLENDER", "/Applications/Blender.app/Contents/MacOS/Blender")
            cmd = [blender, "-b", "--factory-startup", "-P", str(FILM / "studio/studio.py"), "--",
                   "--shot", str(spec), "--out", str(part)]
            if data.get("video"):
                cmd += ["--video", str(expand(data["video"]))]
            if quality == "preview":
                cmd.append("--preview")
            run(cmd, log=log)
        elif kind == "scene.v1":
            spec = tmpdir / "scene.json"
            spec.write_text(json.dumps(data["shot"]))
            env = dict(os.environ)
            env.setdefault("CARGO_TARGET_DIR", str(HOME / "work/openagents-target-videolab"))
            if quality == "master":
                env["VERSE_QUALITY"] = "high"
            cmd = ["cargo", "run", "--release", "-p", "verse", "--example", "film_shot", "--", str(spec),
                   "--out", str(part)]
            if quality == "preview":
                cmd += ["--subframes", "1"]
            run(cmd, env=env, cwd=str(REPO), log=log)
        else:
            raise RuntimeError(f"Lab can't render a {data.get('renderer')} shot yet")
        if not part.exists():
            raise RuntimeError("the renderer finished without writing a video")
        shutil.move(part, out)
        poster = out.with_suffix(".jpg")
        dur = probe_duration(out)
        run(["ffmpeg", "-v", "error", "-y", "-ss", f"{min(5.0, dur / 2):.2f}", "-i", str(out), "-frames:v", "1",
             "-vf", "scale=960:-2", str(poster)])
        why = private(kind, data)
        info = {"doc": doc, "version": n, "quality": quality, "digest": v["digest"], "file": str(out),
                "poster": str(poster), "seconds": dur, "rendered": now(), "took": round(now() - started, 1),
                "no_training": bool(why), "private": why}
        out.with_suffix(".json").write_text(json.dumps(info, indent=2) + "\n")
        if why:
            # The mark travels with the file: not for training, stays on this Mac.
            out.with_suffix(".no-training").write_text(
                "Not for training, and stays on this Mac: " + "; ".join(why) + "\n")
        return out
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)


def probe_duration(path: Path) -> float:
    r = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", str(path)],
                       capture_output=True, text=True)
    try:
        return float(r.stdout.strip())
    except ValueError:
        return 0.0


class Renders:
    """One render at a time (a file lock, so the CLI and the service share it)."""

    def __init__(self):
        self.jobs: dict[str, dict] = {}
        self.q: queue.Queue = queue.Queue()
        threading.Thread(target=self.worker, daemon=True).start()

    def job_id(self, doc, n, quality):
        return f"{doc}-v{n}-{quality}"

    def submit(self, doc: str, n: int, quality: str, auto: bool = True) -> dict:
        """Queue a render; `auto` renders (the ones a look asks for) wait for
        `lab render` when no service is running, so a CLI look returns at once."""
        v = version(doc, n)
        jid = self.job_id(doc, n, quality)
        if render_info(doc, v, quality):
            return {"id": jid, "state": "done"}
        if auto and not AUTO_RENDER:
            return self.jobs.get(jid) or {"id": jid, "state": "none"}
        with LOCK:
            job = self.jobs.get(jid)
            if job and job["state"] in ("queued", "running"):
                return job
            est = estimate(doc, quality)
            job = {"id": jid, "doc": doc, "version": n, "quality": quality, "state": "queued",
                   "queued": now(), "estimate": est}
            self.jobs[jid] = job
        self.q.put(jid)
        return job

    def status(self, doc: str, n: int, quality: str) -> dict:
        v = version(doc, n)
        jid = self.job_id(doc, n, quality)
        if render_info(doc, v, quality):
            return {"id": jid, "state": "done"}
        return self.jobs.get(jid) or {"id": jid, "state": "none"}

    def worker(self):
        while True:
            jid = self.q.get()
            job = self.jobs[jid]
            job["state"], job["started"] = "running", now()
            logp = ROOT / "logs" / f"{jid}.log"
            logp.parent.mkdir(parents=True, exist_ok=True)
            try:
                with open(logp, "w") as log, render_lock():
                    do_render(job["doc"], job["version"], job["quality"], log)
                job["state"] = "done"
            except Exception as e:  # a failed render is reported, never retried silently
                job["state"], job["error"] = "failed", str(e)[:600]
            job["finished"] = now()


class render_lock:
    def __enter__(self):
        ROOT.mkdir(parents=True, exist_ok=True)
        self.f = open(ROOT / ".render.lock", "w")
        fcntl.flock(self.f, fcntl.LOCK_EX)
        return self

    def __exit__(self, *a):
        fcntl.flock(self.f, fcntl.LOCK_UN)
        self.f.close()


def estimate(doc: str, quality: str) -> float | None:
    took = [json.loads(p.read_text()).get("took") for p in (doc_dir(doc) / "renders").glob(f"*-{quality}.json")]
    took = [t for t in took if t]
    return round(sorted(took)[len(took) // 2], 0) if took else None


RENDERS: Renders | None = None
AUTO_RENDER = True


def renders() -> Renders:
    global RENDERS
    if RENDERS is None:
        RENDERS = Renders()
    return RENDERS


# ---- frames ----------------------------------------------------------------------

def frame_path(doc: str, v: dict, t: float, tag: str) -> Path:
    return doc_dir(doc) / "frames" / f"{v['digest'][7:19]}-{t:08.2f}-{tag}.jpg"


def frame(doc: str, n, t: float) -> tuple[Path, str]:
    """The frame at t: from the best render of that version, or drawn from the
    sources when there's no render yet (cuts only)."""
    v = version(doc, n)
    data = body(doc, v["n"])
    best = best_render(doc, v)
    if best:
        out = frame_path(doc, v, t, best[1])
        if not out.exists():
            out.parent.mkdir(parents=True, exist_ok=True)
            run(["ffmpeg", "-v", "error", "-y", "-ss", f"{t:.3f}", "-i", str(best[0]), "-frames:v", "1",
                 "-vf", "scale=960:-2", "-q:v", "3", str(out)])
        return out, f"from the {best[1]} render"
    if v["kind"] != "cut.v1":
        raise Refused(f"there's no render of {doc} v{v['n']} yet; render a preview first")
    out = frame_path(doc, v, t, "src")
    if not out.exists():
        source_frame(data, t, out)
    return out, "drawn from the sources (no render of this version yet)"


def source_frame(data: dict, t: float, out: Path):
    from PIL import Image
    import numpy as np
    cm = cutmod()
    w, h = 960, 540
    fps = data["fps"]
    tl = timeline(data)
    total = tl[-1]["end"]
    t = min(max(t, 0.0), total - 1.0 / fps)
    sh = next(s for s in tl if s["start"] <= t < s["end"] or s is tl[-1])
    shot = data["shots"][sh["i"]]
    off = t - sh["start"]
    out.parent.mkdir(parents=True, exist_ok=True)
    if "title" in shot:
        spec = {k: v for k, v in shot["title"].items() if k != "under"}
        img = cm.draw_title({**spec, "grain": False}, w, h, off, shot["dur"], np.random.default_rng(0),
                            cm.backdrop(w, h, 0))
    else:
        if shot.get("play") is not None and off > shot["play"]:
            off = max(shot["play"] - 1.0 / fps, 0.0)
        src_t = shot.get("in", 0.0) + off * shot.get("speed", 1.0)
        src_t = min(src_t, max(probe_duration(expand(shot["src"])) - 2.0 / fps, 0.0))
        tmp = out.with_suffix(".src.png")
        run(["ffmpeg", "-v", "error", "-y", "-ss", f"{src_t:.3f}", "-i", str(expand(shot["src"])), "-frames:v", "1",
             "-vf", f"scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:-1:-1", str(tmp)])
        img = Image.open(tmp).convert("RGB")
        tmp.unlink()
    for c in data.get("captions", []):
        if c["at"] <= t < c["at"] + c["dur"]:
            st = cm.caption_style(data, h)
            png = out.with_suffix(".cap.png")
            x, y = cm.render_caption(c, st, w, h, png)
            cap = Image.open(png)
            img.paste(cap, (x, y), cap)
            png.unlink()
    img.save(out, quality=88)


def side_by_side(a: Path, b: Path, la: str, lb: str, out: Path):
    from PIL import Image, ImageDraw
    ia, ib = Image.open(a).convert("RGB"), Image.open(b).convert("RGB")
    w, h = 960, int(960 * ia.height / ia.width)
    ia, ib = ia.resize((w, h)), ib.resize((w, h))
    sheet = Image.new("RGB", (w * 2 + 12, h + 44), (14, 14, 16))
    sheet.paste(ia, (0, 44))
    sheet.paste(ib, (w + 12, 44))
    d = ImageDraw.Draw(sheet)
    try:
        f = cutmod().font("medium", 22)
    except Exception:
        f = None
    d.text((12, 10), la, fill=(225, 225, 230), font=f)
    d.text((w + 24, 10), lb, fill=(225, 225, 230), font=f)
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out, quality=88)


# ---- notes -----------------------------------------------------------------------

def review_mod():
    return load_module("film_review", FILM / "review.py")


def notes_for(doc: str) -> list[dict]:
    """Every review the owner left on a video of this document: the videos it
    was made as before Lab (`reviewed_as`) and its renders."""
    meta = doc_meta(doc)
    out = []
    stems = {s: None for s in meta.get("reviewed_as", [])}
    renders_dir = doc_dir(doc) / "renders"
    by_file = {}
    for side in renders_dir.glob("*.json") if renders_dir.exists() else []:
        info = json.loads(side.read_text())
        by_file[info["file"]] = info
    if REVIEWS.exists():
        for nj in sorted(REVIEWS.glob("*/notes.json")):
            r = json.loads(nj.read_text())
            video = r.get("video", "")
            stem = Path(video).stem
            info = by_file.get(video)
            if info or stem in stems:
                ver = info["version"] if info else next(
                    (v["n"] for v in versions(doc) if stem in v.get("reviewed_as", [])), None)
                for n in r.get("notes", []):
                    out.append({"video": video, "version": ver, "t": n["t"], "text": n["text"].strip(),
                                "frame": str(nj.parent / n["frame"]) if n.get("frame") else None,
                                "notes_md": str(nj.parent / "notes.md")})
    return out


def add_note(doc: str, n, quality: str, t: float, text: str) -> dict:
    v = version(doc, n)
    if not render_info(doc, v, quality):
        raise Refused(f"there's no {quality} render of v{v['n']} to note on")
    video = render_path(doc, v, quality)
    rv = review_mod()
    import uuid
    with rv.LOCK:
        review = rv.load(video)
        nid = uuid.uuid4().hex[:8]
        frame, strip = rv.frame_names(t, nid)
        review["notes"].append({"id": nid, "t": round(t, 3), "text": text.strip(), "created": now(),
                                "frame": frame, "strip": strip})
        rv.save(video, review)
    threading.Thread(target=rv.grab, args=(video, t, rv.review_dir(video), nid), daemon=True).start()
    return {"saved": True, "notes_md": str(rv.review_dir(video) / "notes.md")}


# ---- the tools -------------------------------------------------------------------

def base_url() -> str:
    return os.environ.get("VIDEO_LAB_URL", f"http://127.0.0.1:{PORT}")


def media(path: Path | str | None) -> str | None:
    if not path:
        return None
    rel = Path(path).resolve().relative_to(ROOT.resolve())
    return "/media/" + quote(str(rel))


def versions_brief(doc: str) -> list[dict]:
    return [{"n": v["n"], "summary": v["summary"], "author": v["author"], "created": v["created"]}
            for v in versions(doc)]


def card(doc: str, v: dict, quality: str = "preview", start=None, end=None, image=None, compare=None,
         job=None) -> dict:
    info = render_info(doc, v, quality)
    meta = doc_meta(doc)
    c = {"card": "video_lab", "lab": base_url(), "doc": doc, "title": meta["title"], "kind": v["kind"],
         "version": v["n"], "summary": v["summary"], "quality": quality,
         "versions": versions_brief(doc),
         "video": media(info["file"]) if info else None,
         "poster": media(info["poster"]) if info else None,
         "from": start, "to": end, "image": media(image) if image else None,
         "private": private(v["kind"], body(doc, v["n"])), "notes": len([n for n in notes_for(doc) if n["version"] == v["n"]]),
         "player": f"{base_url()}/player?doc={quote(doc)}&v={v['n']}" + (f"&t={start:.2f}" if start else "")}
    if not info:
        st = job or renders().status(doc, v["n"], quality)
        c["job"] = {k: st.get(k) for k in ("id", "state", "error", "estimate", "started", "queued")}
    if compare:
        c["compare"] = compare
    return c


def shots_text(data: dict, a: float, b: float) -> list[str]:
    lines = []
    for s in timeline(data):
        if s["end"] > a and s["start"] < b:
            sh = data["shots"][s["i"]]
            what = f"title card" if "title" in sh else f"{Path(sh['src']).name} from {sh.get('in', 0)}s"
            extra = ""
            if sh.get("play") is not None:
                extra += f", plays {sh['play']}s then holds"
            if sh.get("xfade"):
                extra += f", {sh['xfade']}s crossfade in"
            beats = cutmod().beats_for(expand(sh["src"])) if "src" in sh else []
            if beats:
                extra += "; its animations: " + ", ".join(f"{bb['what']} {bb['from']}-{bb['to']}s of the clip"
                                                            for bb in beats)
            lines.append(f"[{s['i']}] {sh['name']}: {clock(s['start'])}-{clock(s['end'])} "
                         f"({sh['dur']}s), {what}{extra}")
    return lines


def caps_text(data: dict, a: float, b: float) -> list[str]:
    return [f"{clock(c['at'])} \"{c['text']}\"" for c in data.get("captions", []) if a <= c["at"] < b]


def tool_list(args) -> dict:
    docs = documents()
    text = "Documents:\n" + "\n".join(
        f"- {d['id']}: {d['title']} ({d['kind']}, {d['versions']} versions)" for d in docs)
    return {"text": text, "documents": docs}


def tool_open(args) -> dict:
    doc = resolve(args.get("document"))
    v = version(doc, args.get("version"))
    data = body(doc, v["n"])
    problems, notes = check(v["kind"], data)
    lines = [f"{doc_meta(doc)['title']} v{v['n']} of {len(versions(doc))}: {v['summary']}",
             f"{v['kind']}, {clock(length(v['kind'], data))} long"]
    if private(v["kind"], data):
        lines.append("Private: " + "; ".join(private(v["kind"], data)) + ". Renders stay on this Mac and are not for training.")
    if v["kind"] == "cut.v1":
        lines.append("Shots:")
        lines += shots_text(data, 0, 1e9)
    else:
        lines.append(json.dumps(data, indent=1)[:2500])
    if problems:
        lines.append("Problems the checks find in this version:")
        lines += [f"- {p}" for p in problems]
    ns = [n for n in notes_for(doc)]
    if ns:
        lines.append(f"{len(ns)} review notes (lab_notes reads them).")
    lines.append("Versions: " + "; ".join(f"v{x['n']} {x['summary']}" for x in versions_brief(doc)))
    job = renders().submit(doc, v["n"], "preview") if args.get("render", True) else None
    return {"text": "\n".join(lines), "card": card(doc, v, "preview", job=job)}


def tool_show(args) -> dict:
    doc = resolve(args.get("document"))
    v = version(doc, args.get("version"))
    data = body(doc, v["n"])
    total = length(v["kind"], data)
    a = seconds(args["from"], data) if args.get("from") not in (None, "") else (
        seconds(args["at"], data) if args.get("at") not in (None, "") else 0.0)
    b = seconds(args["to"], data) if args.get("to") not in (None, "") else min(a + 15.0, total)
    if b <= a:
        raise Refused(f"the end ({clock(b)}) is before the start ({clock(a)})")
    quality = args.get("quality") or "preview"
    job = renders().submit(doc, v["n"], quality)
    lines = [f"{doc_meta(doc)['title']} v{v['n']}, {clock(a)} to {clock(b)}:"]
    strip = None
    if v["kind"] == "cut.v1":
        lines += shots_text(data, a, b)
        caps = caps_text(data, a, b)
        if caps:
            lines.append("Captions: " + " / ".join(caps))
        # A strip of frames across the range, so the chat can see it before the render is done.
        times = [a + (b - a) * k / 4 for k in range(5)]
        frames = [frame(doc, v["n"], t)[0] for t in times]
        strip = doc_dir(doc) / "frames" / f"{v['digest'][7:19]}-strip-{a:.1f}-{b:.1f}.jpg"
        contact(frames, [clock(t) for t in times], strip)
    st = renders().status(doc, v["n"], quality)
    lines.append({"done": "The video is ready in the card.",
                  "queued": "The preview is queued; the card plays it when it's done.",
                  "running": "The preview is rendering; the card plays it when it's done.",
                  "failed": f"The preview failed: {st.get('error')}"}.get(
        st["state"], f"There's no {quality} render of v{v['n']} yet (lab render makes one)."))
    return {"text": "\n".join(lines), "card": card(doc, v, quality, a, b, image=strip, job=job)}


def contact(frames, labels, out: Path):
    from PIL import Image, ImageDraw
    tiles = [Image.open(f).convert("RGB").resize((384, 216)) for f in frames]
    sheet = Image.new("RGB", (384 * len(tiles) + 6 * (len(tiles) - 1), 216 + 30), (14, 14, 16))
    d = ImageDraw.Draw(sheet)
    for i, (tile, label) in enumerate(zip(tiles, labels)):
        sheet.paste(tile, (i * 390, 30))
        d.text((i * 390 + 6, 8), label, fill=(220, 220, 225))
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out, quality=88)


def tool_frame(args) -> dict:
    doc = resolve(args.get("document"))
    v = version(doc, args.get("version"))
    data = body(doc, v["n"])
    ats = args.get("at")
    ats = ats if isinstance(ats, list) else [ats]
    ts = [seconds(x, data) for x in ats]
    paths = [frame(doc, v["n"], t) for t in ts]
    if len(paths) == 1:
        img = paths[0][0]
    else:
        img = doc_dir(doc) / "frames" / f"{v['digest'][7:19]}-set-{'-'.join(f'{t:.1f}' for t in ts)}.jpg"
        contact([p[0] for p in paths], [clock(t) for t in ts], img)
    text = f"{doc_meta(doc)['title']} v{v['n']} at {', '.join(clock(t) for t in ts)} ({paths[0][1]})."
    if v["kind"] == "cut.v1":
        text += "\n" + "\n".join(shots_text(data, min(ts), max(ts) + 0.01))
    return {"text": text, "image": str(img), "card": card(doc, v, "preview", min(ts), None, image=img)}


def tool_edit(args) -> dict:
    doc = resolve(args.get("document"))
    summary = (args.get("summary") or "").strip()
    if not summary:
        raise Refused("an edit needs a one-line summary of what it changes, in plain words")
    parent = version(doc, args.get("version"))
    old = body(doc, parent["n"])
    ops = args.get("patch")
    if isinstance(ops, str):
        try:
            ops = json.loads(ops)
        except ValueError as e:
            raise Refused(f"the patch isn't JSON ({e})")
    ops = [ops] if isinstance(ops, dict) else ops
    new = apply_patch(old, ops)
    if new == old:
        raise Refused("the patch changes nothing")
    moved = 0
    touched_caps = any(isinstance(op, dict) and str(op.get("path", "")).startswith("/captions") for op in ops)
    if parent["kind"] == "cut.v1" and args.get("ripple_captions", True) and not touched_caps:
        moved = ripple(old, new)
    before, _ = check(parent["kind"], old)
    after, notes = check(parent["kind"], new)
    added = [p for p in after if p not in before]
    shape = lambda p: re.sub(r"[\d.]+", "#", p)  # the same problem with other numbers
    if added:
        fixed = [p for p in before if shape(p) not in {shape(q) for q in after}]
        why = "Not saved. " + " ".join(f"{p}." if not p.endswith(".") else p for p in plain(added))
        if fixed:
            why += " (It would have fixed: " + "; ".join(fixed) + ".)"
        return {"text": why, "refused": True, "problems": added}
    author = args.get("author") or "chat"
    v = save_version(doc, parent["kind"], new, summary, author, parent=parent["n"])
    fixed = [p for p in before if shape(p) not in {shape(q) for q in after}]
    lines = [f"Saved v{v['n']}: {summary}"]
    if moved:
        lines.append(f"{moved} captions moved with their shots.")
    if fixed:
        lines.append("Fixed: " + "; ".join(fixed))
    if after:
        lines.append("Still open from before: " + "; ".join(after))
    if new.get("shots") and old.get("shots"):
        lines.append(f"Length {clock(length('cut.v1', old))} -> {clock(length('cut.v1', new))}.")
    job = renders().submit(doc, v["n"], "preview") if args.get("render", True) else None
    if job:
        lines.append("A preview is rendering.")
    return {"text": "\n".join(lines), "version": v["n"], "card": card(doc, v, "preview", job=job)}


def plain(problems: list[str]) -> list[str]:
    """Problems as a person reads them: a run of shots knocked off the beat
    grid by one change is one sentence."""
    off = re.compile(r"^(?P<name>[^:]+): starts at (?P<at>[\d.]+)s, off the (?P<bpm>[\d.]+) BPM beat grid$")
    grid = [m for m in map(off.match, problems) if m]
    rest = [p for p in problems if not off.match(p)]
    if len(grid) > 1:
        first, n, bpm = grid[0], len(grid) - 1, grid[0]["bpm"]
        after = "the shot after it" if n == 1 else f"the {n} shots after it"
        beat = 60.0 / float(bpm)
        rest.insert(0, f"{first['name']} and {after} would start off the {bpm} BPM beat grid ({first['name']} at "
                       f"{first['at']}s); keep shot lengths to whole beats (multiples of {beat:g} s)")
    else:
        rest = [m.string for m in grid] + rest
    return rest


def tool_render(args) -> dict:
    doc = resolve(args.get("document"))
    v = version(doc, args.get("version"))
    quality = args.get("quality") or "preview"
    if quality not in QUALITIES:
        raise Refused(f"quality is preview, review or master, not {quality}")
    job = renders().submit(doc, v["n"], quality, auto=False)
    est = job.get("estimate")
    state = {"done": "is ready", "queued": "is queued", "running": "is rendering"}.get(job["state"], job["state"])
    text = f"The {quality} ({QUALITIES[quality][0]}) of {doc_meta(doc)['title']} v{v['n']} {state}."
    if est and job["state"] != "done":
        text += f" The last one took about {int(est // 60)} min {int(est % 60)} s."
    return {"text": text, "card": card(doc, v, quality, job=job)}


def tool_compare(args) -> dict:
    doc = resolve(args.get("document"))
    b = version(doc, args.get("b"))
    a = version(doc, args.get("a")) if args.get("a") not in (None, "") else (
        version(doc, b["parent"]) if b.get("parent") else version(doc, max(b["n"] - 1, 1)))
    data_b = body(doc, b["n"])
    t = seconds(args.get("at"), data_b) if args.get("at") not in (None, "") else 0.0
    fa, ha = frame(doc, a["n"], t)
    fb, hb = frame(doc, b["n"], t)
    out = doc_dir(doc) / "frames" / f"compare-v{a['n']}-v{b['n']}-{t:08.2f}.jpg"
    side_by_side(fa, fb, f"v{a['n']}  {clock(t)}", f"v{b['n']}  {clock(t)}", out)
    quality = args.get("quality") or "preview"
    for v in (a, b):
        renders().submit(doc, v["n"], quality)
    ia, ib = render_info(doc, a, quality), render_info(doc, b, quality)
    compare = {"at": t,
               "a": {"version": a["n"], "summary": a["summary"], "video": media(ia["file"]) if ia else None},
               "b": {"version": b["n"], "summary": b["summary"], "video": media(ib["file"]) if ib else None}}
    lines = [f"v{a['n']} ({a['summary']}) and v{b['n']} ({b['summary']}) at {clock(t)}:"]
    if b["kind"] == "cut.v1":
        lines.append(f"v{a['n']}: " + "; ".join(shots_text(body(doc, a["n"]), t, t + 0.01)))
        lines.append(f"v{b['n']}: " + "; ".join(shots_text(data_b, t, t + 0.01)))
    return {"text": "\n".join(lines), "image": str(out),
            "card": card(doc, b, quality, t, None, image=out, compare=compare)}


def tool_notes(args) -> dict:
    doc = resolve(args.get("document"))
    ns = notes_for(doc)
    want = args.get("version")
    if want not in (None, ""):
        n = version(doc, want)["n"]
        ns = [x for x in ns if x["version"] == n]
    if not ns:
        return {"text": f"No review notes on {doc_meta(doc)['title']}" + (f" v{want}" if want else "") + " yet."}
    lines = []
    for x in ns:
        lines.append(f"- v{x['version']} {clock(x['t'])} ({Path(x['video']).name}, seconds into that file): {x['text']}")
    lines.append("Times are into the video each note was made on; a re-cut moves things.")
    return {"text": "\n".join(lines), "notes": ns}


def tool_versions(args) -> dict:
    doc = resolve(args.get("document"))
    lines = [f"v{v['n']} by {v['author']}: {v['summary']}"
             + (" (rendered: " + ", ".join(q for q in QUALITIES if render_info(doc, v, q)) + ")"
                if any(render_info(doc, v, q) for q in QUALITIES) else "")
             for v in versions(doc)]
    return {"text": "\n".join(lines), "versions": versions_brief(doc)}


TOOLS = {
    "lab_list": (tool_list, "List the Video Lab documents (cuts, shots, scenes) on this Mac.",
                 {}),
    "lab_open": (tool_open, "Open a document: its latest version (or the one asked for), its shots, any problems, "
                            "the versions, and a preview in the chat (rendered if there isn't one).",
                 {"document": "string", "version": "string?"}),
    "lab_show": (tool_show, "Show a time range of a document in the chat: which shots and captions are there, a "
                            "strip of frames, and the preview video starting there. Times are m:ss or seconds.",
                 {"document": "string", "from": "string?", "to": "string?", "version": "string?"}),
    "lab_frame": (tool_frame, "One still at a time (or several times as a strip), for looking closely.",
                  {"document": "string", "at": "string", "version": "string?"}),
    "lab_edit": (tool_edit, "Change a document by a JSON Patch (RFC 6902) on its JSON, making a new version. "
                            "Address a shot by name: /shots/@s4-delegation/dur. In a cut, captions move with "
                            "their shots when shot lengths change. The edit is checked first (schema, the beat "
                            "grid, sources long enough, no shot ending inside an animation, music length) and "
                            "refused with the reason if it adds a problem; then fix the patch. A preview renders.",
                 {"document": "string", "patch": "string", "summary": "string", "version": "string?"}),
    "lab_render": (tool_render, "Render a version: preview (quick, 960x540), review (1080p) or master (full size).",
                   {"document": "string", "quality": "string?", "version": "string?"}),
    "lab_compare": (tool_compare, "Two versions side by side at a time (default: the latest and the one before it).",
                    {"document": "string", "at": "string?", "a": "string?", "b": "string?"}),
    "lab_notes": (tool_notes, "Read the owner's review notes on a document's videos.",
                  {"document": "string", "version": "string?"}),
    "lab_versions": (tool_versions, "List a document's versions: who made each and what it changed.",
                     {"document": "string"}),
}


def tool_specs() -> list[dict]:
    """The tools as function declarations (JSON Schema), for any chat's model."""
    specs = []
    for name, (_, desc, params) in TOOLS.items():
        props, req = {}, []
        for p, t in params.items():
            props[p] = {"type": "string", "description": PARAM_DOCS.get(p, p)}
            if not t.endswith("?"):
                req.append(p)
        specs.append({"name": name, "description": desc,
                      "parameters": {"type": "object", "properties": props, "required": req}})
    return specs


PARAM_DOCS = {
    "document": "the document's id or title, e.g. episode-290",
    "version": "a version number like 3 (default: the latest)",
    "from": "start time, m:ss or seconds", "to": "end time, m:ss or seconds",
    "at": "a time, m:ss or seconds", "a": "the first version to compare (default: the one before b)",
    "b": "the second version (default: the latest)",
    "patch": 'a JSON Patch as a JSON string, e.g. [{"op":"replace","path":"/shots/@s4-delegation/dur","value":12.5}]',
    "summary": "one line in plain words saying what the edit changes",
    "quality": "preview, review or master",
}


def call(name: str, args: dict) -> dict:
    if name not in TOOLS:
        raise Refused(f"there's no tool {name}")
    return TOOLS[name][0](args or {})


# ---- import ----------------------------------------------------------------------

def take2_score(data: dict) -> dict:
    """The take 2 score: a whoosh at the start of each section (the shots'
    sN- prefixes), the end chord under the end card (build2.py)."""
    cuts, seen = [], set()
    for s in data["shots"]:
        sec = s["name"].split("-")[0]
        if sec not in seen:
            seen.add(sec)
            cuts.append(s["name"])
    return {"cuts_at": cuts[1:-1] if data["shots"][-1]["name"] == "end-card" else cuts[1:],
            "end_at": "end-card"}


def do_import(author: str = "import") -> list[str]:
    done = []
    film = HOME / "work/film-out"
    if not (doc_dir("episode-290") / "doc.json").exists():
        draft = film / "cut/episode-290-draft.json"
        take2 = film / "cut/episode-290-take2.json"
        first = True
        for path, summary, reviewed in ((draft, "The draft (take 1), as reviewed on 2026-10-13", "episode-290-draft"),
                                        (take2, "Take 2: the re-cut after the draft's review", "episode-290-take2")):
            if not path.exists():
                continue
            data = json.loads(path.read_text())
            if path == take2:
                data["score"] = take2_score(data)
            if first:
                create("episode-290", "cut.v1", "Episode 290", data, summary, author,
                       aliases=["ep 290", "episode 290", "coder v1 launch film"],
                       reviewed_as=["episode-290-draft", "episode-290-take2"],
                       source=str(path))
                first = False
            else:
                save_version("episode-290", "cut.v1", data, summary, author, parent=1)
            vd = sorted(doc_dir("episode-290").glob("v*/version.json"))[-1]
            meta = json.loads(vd.read_text())
            meta["reviewed_as"] = [reviewed]
            vd.write_text(json.dumps(meta, indent=2) + "\n")
        done.append("episode-290")
    walk_spec = REPO / "docs/verse/film/verse-shots/portal-walk.json"
    if walk_spec.exists() and not (doc_dir("portal-walk-scene") / "doc.json").exists():
        create("portal-walk-scene", "scene.v1", "Portal walk (Verse scene)",
               {"renderer": "verse-film-shot", "shot": json.loads(walk_spec.read_text()),
                "out": "~/work/film-out/verse/portal-walk-raw.mp4",
                "notes": "The engine's own render of the walk on the Grid into the Everglade portal "
                         "(cargo run -p verse --example film_shot)."},
               "The walk as shot for the launch film", author, aliases=["portal walk scene", "the walk"],
               source=str(walk_spec))
        done.append("portal-walk-scene")
    if not (doc_dir("portal-walk") / "doc.json").exists():
        create("portal-walk", "shot.v1", "Portal walk",
               {"renderer": "film-script", "script": "scripts/film/portal-walk.py",
                "inputs": {"walk": "~/work/film-out/verse/portal-walk-raw.mp4"},
                "args": ["{walk}", "{out}", "--cross", "12.0"], "duration": 12.5,
                "out": "~/Downloads/openagents-portal-walk.mp4",
                "notes": "The film's ending: the Verse walk eased to white at the portal, then the Coming soon "
                         "card. The walk itself is the portal-walk-scene document."},
               "The ending shot as used in Episode 290", author,
               aliases=["portal walk", "openagents portal walk", "the ending"],
               source=str(REPO / "scripts/film/portal-walk.py"))
        done.append("portal-walk")
    return done


# ---- the service -----------------------------------------------------------------

PLAYER = (HERE / "lab_player.html")


class Handler(BaseHTTPRequestHandler):
    server_version = "VideoLab/1"

    def log_message(self, fmt, *args):
        pass

    def local(self) -> bool:
        return self.client_address[0] in ("127.0.0.1", "::1")

    def send_json(self, value, code=200):
        data = json.dumps(value).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST")
        self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.end_headers()

    def do_GET(self):
        if not self.local():
            return self.send_error(403)
        u = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(u.query).items()}
        try:
            if u.path in ("/", "/player"):
                data = PLAYER.read_bytes()
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                return self.wfile.write(data)
            if u.path == "/healthz":
                return self.send_json({"ok": True})
            if u.path == "/api/tools":
                return self.send_json({"tools": tool_specs()})
            if u.path == "/api/docs":
                return self.send_json({"documents": documents()})
            if u.path == "/api/doc":
                doc = resolve(q.get("doc"))
                vs = versions(doc)
                out = []
                for v in vs:
                    rs = {qq: media(render_info(doc, v, qq)["file"]) for qq in QUALITIES if render_info(doc, v, qq)}
                    out.append({**{k: v[k] for k in ("n", "summary", "author", "created", "digest", "private")},
                                "renders": rs, "job": renders().status(doc, v["n"], "preview")})
                return self.send_json({"doc": doc_meta(doc), "versions": out,
                                       "notes": notes_for(doc)})
            if u.path == "/api/card":
                doc = resolve(q.get("doc"))
                v = version(doc, q.get("v"))
                compare = None
                if q.get("a"):
                    a = version(doc, q["a"])
                    qa = q.get("q") or "preview"
                    ia, ib = render_info(doc, a, qa), render_info(doc, v, qa)
                    compare = {"at": float(q.get("at") or 0),
                               "a": {"version": a["n"], "summary": a["summary"],
                                     "video": media(ia["file"]) if ia else None},
                               "b": {"version": v["n"], "summary": v["summary"],
                                     "video": media(ib["file"]) if ib else None}}
                return self.send_json(card(doc, v, q.get("q") or "preview",
                                           float(q["from"]) if q.get("from") else None,
                                           float(q["to"]) if q.get("to") else None, compare=compare))
            if u.path.startswith("/media/"):
                return self.send_media(u.path[len("/media/"):])
            self.send_error(404)
        except Refused as e:
            self.send_json({"error": str(e)}, 400)

    def do_POST(self):
        if not self.local():
            return self.send_error(403)
        u = urlparse(self.path)
        n = int(self.headers.get("Content-Length") or 0)
        try:
            args = json.loads(self.rfile.read(n) or b"{}")
        except ValueError:
            return self.send_json({"error": "the request isn't JSON"}, 400)
        try:
            if u.path.startswith("/api/tool/"):
                name = u.path[len("/api/tool/"):].replace(".", "_")
                return self.send_json(call(name, args))
            if u.path == "/api/note":
                doc = resolve(args.get("doc"))
                return self.send_json(add_note(doc, args.get("v"), args.get("q") or "preview",
                                               float(args["t"]), str(args.get("text") or "")))
            self.send_error(404)
        except Refused as e:
            self.send_json({"error": str(e), "text": str(e), "refused": True}, 200)
        except Exception as e:
            self.send_json({"error": f"the Lab failed: {e}"}, 500)

    def send_media(self, rel: str):
        from urllib.parse import unquote
        path = (ROOT / unquote(rel)).resolve()
        if ROOT.resolve() not in path.parents or not path.is_file():
            return self.send_error(404)
        size = path.stat().st_size
        start, end = 0, size - 1
        rng = self.headers.get("Range")
        if rng:
            m = re.match(r"bytes=(\d*)-(\d*)", rng)
            if m:
                if m[1]:
                    start = int(m[1])
                    end = int(m[2]) if m[2] else size - 1
                else:
                    start = max(0, size - int(m[2]))
            end = min(end, size - 1)
        self.send_response(206 if rng else 200)
        self.send_header("Content-Type", mimetypes.guess_type(str(path))[0] or "application/octet-stream")
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Length", str(end - start + 1))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "private, max-age=3600")
        if path.with_suffix(".no-training").exists() or (
                path.suffix == ".jpg" and any(path.parent.glob("*.no-training"))):
            self.send_header("X-Robots-Tag", "noai, noimageai")
            self.send_header("X-OpenAgents-No-Training", "1")
        if rng:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        with open(path, "rb") as f:
            f.seek(start)
            left = end - start + 1
            while left > 0:
                chunk = f.read(min(1 << 20, left))
                if not chunk:
                    break
                try:
                    self.wfile.write(chunk)
                except (BrokenPipeError, ConnectionResetError):
                    return
                left -= len(chunk)


def serve(port: int):
    global PORT
    PORT = port
    do_import()
    renders()
    srv = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    print(f"Video Lab on http://127.0.0.1:{port} (documents in {ROOT / PROJECT})", flush=True)
    srv.serve_forever()


def service_up() -> bool:
    try:
        with urllib.request.urlopen(f"{base_url()}/healthz", timeout=1) as r:
            return r.status == 200
    except OSError:
        return False


def remote(name: str, args: dict) -> dict:
    req = urllib.request.Request(f"{base_url()}/api/tool/{name}", data=json.dumps(args).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=600) as r:
        return json.loads(r.read())


def main():
    ap = argparse.ArgumentParser(description="Video Lab")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("serve")
    p.add_argument("--port", type=int, default=PORT)
    sub.add_parser("import")
    sub.add_parser("list")
    sub.add_parser("tools")
    for name in ("open", "versions", "notes"):
        p = sub.add_parser(name)
        p.add_argument("document")
        p.add_argument("--version")
    p = sub.add_parser("show")
    p.add_argument("document")
    p.add_argument("--from", dest="start")
    p.add_argument("--to", dest="end")
    p.add_argument("--version")
    p = sub.add_parser("frame")
    p.add_argument("document")
    p.add_argument("at", nargs="+")
    p.add_argument("--version")
    p = sub.add_parser("edit")
    p.add_argument("document")
    p.add_argument("patch", help="a file with the JSON Patch, or - for stdin, or the JSON itself")
    p.add_argument("--summary", required=True)
    p.add_argument("--author", default=os.environ.get("USER", "owner"))
    p.add_argument("--version")
    p = sub.add_parser("render")
    p.add_argument("document")
    p.add_argument("--quality", default="preview")
    p.add_argument("--version")
    p.add_argument("--wait", action="store_true")
    p = sub.add_parser("compare")
    p.add_argument("document")
    p.add_argument("--a")
    p.add_argument("--b")
    p.add_argument("--at")
    a = ap.parse_args()
    if a.cmd == "serve":
        return serve(a.port)
    if a.cmd == "import":
        print("\n".join(do_import()) or "Nothing new to import.")
        return
    if a.cmd == "tools":
        print(json.dumps(tool_specs(), indent=1))
        return
    args = {"document": getattr(a, "document", None), "version": getattr(a, "version", None)}
    name = {"list": "lab_list", "open": "lab_open", "versions": "lab_versions", "notes": "lab_notes",
            "show": "lab_show", "frame": "lab_frame", "edit": "lab_edit", "render": "lab_render",
            "compare": "lab_compare"}[a.cmd]
    if a.cmd == "show":
        args.update({"from": a.start, "to": a.end})
    if a.cmd == "frame":
        args["at"] = a.at if len(a.at) > 1 else a.at[0]
    if a.cmd == "edit":
        raw = sys.stdin.read() if a.patch == "-" else Path(a.patch).read_text() if Path(a.patch).exists() else a.patch
        args.update({"patch": raw, "summary": a.summary, "author": a.author})
    if a.cmd == "render":
        args["quality"] = a.quality
    if a.cmd == "compare":
        args.update({"a": a.a, "b": a.b, "at": a.at})
    try:
        if service_up():
            out = remote(name, args)
        else:
            global AUTO_RENDER
            AUTO_RENDER = False
            do_import()
            out = call(name, args)
            if a.cmd == "render" and RENDERS:
                # No service: render here and wait, so the command's result is real.
                while any(j["state"] in ("queued", "running") for j in RENDERS.jobs.values()):
                    time.sleep(1)
                out = call(name, args)
    except Refused as e:
        print(str(e))
        sys.exit(1)
    print(out.get("text") or out.get("error") or json.dumps(out))
    c = out.get("card")
    if c:
        print(f"Open it: {c['player']}")
    if out.get("refused"):
        sys.exit(1)


if __name__ == "__main__":
    main()

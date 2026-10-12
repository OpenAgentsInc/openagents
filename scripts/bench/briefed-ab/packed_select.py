#!/usr/bin/env python3
"""Packed-repository file selection: SubQ Snap vs Clef vs Jev.

For each bench task: filefind ranks 200 files at the fix's parent; the top
files are packed into a token budget (full text for the first FULL files,
a symbol outline for the rest); one Choice question over the packed files
asks which file must change. Recall@5/@10 against the fix's own files.

  packed_select.py ISSUE [ISSUE ...]
Writes rows to OUT/<arm>/<issue>.json (a rerun skips rows already written).
"""
import json, os, re, subprocess, sys, time, urllib.request, urllib.error
from pathlib import Path

BENCH = Path(__file__).resolve().parent
sys.path.insert(0, str(BENCH))
os.environ.setdefault("AB_REPO", str(Path.home() / "work/openagents"))
from common import LEVERS  # noqa
import briefing  # noqa

REPO = os.environ["AB_REPO"]
OUT = Path(os.environ.get("PACK_OUT", str(BENCH / ".work" / "packed-select")))
CHARS_PER_TOKEN = 3.6
FULL = 8
FULL_CAP = 24000
OUTLINE_CAP = 3000

def secret(file, name):
    for line in Path(file).read_text().splitlines():
        line = line.strip().removeprefix("export ")
        if line.startswith(name + "="):
            return line.split("=", 1)[1].strip().strip("'\"")

DOORS = {
    "subq": ("https://api.subq.ai/v1/systemone", "subq-snap-preview",
             secret(Path.home() / "work/.secrets/subq.env", "SUBQ_API_KEY")),
    "clef": ("http://127.0.0.1:11434/v1/systemone", "clef-flash", None),
    "jev": ("https://api.typesafe.ai/v1/systemone", "jev-latest",
            secret(Path.home() / "work/.secrets/typesafe.env", "TYPESAFE_API_KEY")),
}

ITEM = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?(async\s+|unsafe\s+|const\s+)*(fn|struct|enum|trait|impl|mod|type|const|static|macro_rules!)\b")

def show(rev, path):
    p = subprocess.run(["git", "-C", REPO, "show", f"{rev}:{path}"], capture_output=True, text=True)
    return p.stdout if p.returncode == 0 else ""

def outline(path, text):
    lines = text.splitlines()
    keep = []
    for i, line in enumerate(lines):
        if path.endswith(".rs") and ITEM.match(line):
            keep.append(f"{i+1}: {line.strip()[:160]}")
        elif path.endswith(".md") and line.startswith("#"):
            keep.append(f"{i+1}: {line.strip()[:160]}")
        elif line.startswith("//!"):
            keep.append(f"{i+1}: {line.strip()[:160]}")
    body = "\n".join(keep) or "\n".join(lines[:15])
    return f"[outline of {len(lines)} lines]\n" + body[:OUTLINE_CAP]

def pack(task, ranked, budget_tokens, max_files=255):
    """Outline every ranked file that fits (up to the 255-option limit), then
    upgrade the top-ranked files to full text while the budget allows."""
    budget = int(budget_tokens * CHARS_PER_TOKEN)
    head = f"Issue #{task['issue']}: {task['title']}\n\n{(task.get('body') or '')[:4000]}\n\n# Repository files\n"
    used = len(head)
    entries = []  # [path, text, block]
    for f in ranked:
        text = show(task["parent"], f["path"])
        if not text:
            continue
        block = f"\n=== {f['path']} ===\n{outline(f['path'], text)}\n"
        if used + len(block) > budget:
            break
        entries.append([f["path"], text, block]); used += len(block)
        if len(entries) >= max_files:
            break
    for e in entries:
        full = f"\n=== {e[0]} ===\n{e[1][:FULL_CAP]}\n"
        extra = len(full) - len(e[2])
        if extra > 0 and used + extra <= budget:
            e[2] = full; used += extra
    return head + "".join(e[2] for e in entries), [e[0] for e in entries]

def ask(arm, state, files):
    url, model, key = DOORS[arm]
    q = {"file": {"type": "choice",
                  "instructions": "Which one of these repository files must change to fix the issue? Pick the file where the main change belongs.",
                  "criteria": {f: None for f in files}}}
    body = json.dumps({"model": model, "state": state, "questions": q}).encode()
    h = {"content-type": "application/json"}
    if key:
        h["authorization"] = f"Bearer {key}"
    t0 = time.time()
    try:
        with urllib.request.urlopen(urllib.request.Request(url, data=body, headers=h), timeout=300) as r:
            res = json.loads(r.read())
    except urllib.error.HTTPError as e:
        return {"ok": False, "error": f"HTTP {e.code}: {e.read().decode(errors='replace')[:200]}", "ms": int((time.time()-t0)*1000)}
    except Exception as e:  # noqa
        return {"ok": False, "error": str(e)[:200], "ms": int((time.time()-t0)*1000)}
    a = (res.get("answers") or {}).get("file") or {}
    probs = a.get("probabilities") or {}
    return {"ok": True, "ms": int((time.time()-t0)*1000), "tokens": (res.get("usage") or {}).get("input_tokens"),
            "ranked": sorted(probs, key=lambda k: -probs[k])[:20], "top_p": [round(probs[k], 3) for k in sorted(probs, key=lambda k: -probs[k])[:5]]}

def recall(ranked, labels, k):
    hit = [f for f in ranked[:k] if f in labels]
    return len(hit) / max(1, len(labels))

ARMS = [("subq", 220000, 255), ("subq", 14000, 255), ("clef", 14000, 255), ("jev", 14000, 255),
        ("subq", 12000, 26), ("clef", 12000, 26), ("jev", 12000, 26)]
FULL_CAP = 40000

def main():
    for issue in sys.argv[1:]:
        task = json.load(open(BENCH / "tasks" / issue / "task.json"))
        labels = set(task["source_files"]) | set(task["test_files"])
        cache = OUT / "ranked" / f"{issue}.json"
        if cache.exists():
            ranked = json.loads(cache.read_text())
        else:
            ff, _ = briefing.finder_files(task, dict(LEVERS, briefing_files=200), no_cards=True)
            ranked = [{"path": f["path"]} for f in ff]
            cache.parent.mkdir(parents=True, exist_ok=True); cache.write_text(json.dumps(ranked))
        base = [f["path"] for f in ranked]
        row = {"issue": int(issue), "labels": sorted(labels),
               "filefind": {"r5": recall(base, labels, 5), "r10": recall(base, labels, 10)}}
        print(f"#{issue} labels={len(labels)} filefind r5={row['filefind']['r5']:.2f} r10={row['filefind']['r10']:.2f}", flush=True)
        for arm, budget, cap in ARMS:
            tag = f"{arm}-{budget//1000}k" + ("" if cap == 255 else f"-{cap}")
            path = OUT / tag / f"{issue}.json"
            if path.exists():
                res = json.loads(path.read_text())
            else:
                state, files = pack(task, ranked, budget, cap)
                res = ask(arm, state, files)
                res.update({"files_packed": len(files), "chars": len(state),
                            "labels_in_pack": sorted(set(files) & labels)})
                if res.get("ok"):
                    res["r5"] = recall(res["ranked"], labels, 5)
                    res["r10"] = recall(res["ranked"], labels, 10)
                path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(res, indent=1))
            if res.get("ok"):
                print(f"   {tag:13} files={res['files_packed']:3} tokens={res.get('tokens')} in-pack={len(res['labels_in_pack'])}/{len(labels)} r5={res['r5']:.2f} r10={res['r10']:.2f} top_p={res['top_p'][:3]} {res['ms']}ms", flush=True)
            else:
                print(f"   {tag:13} FAILED {res['error'][:120]}", flush=True)

if __name__ == "__main__":
    main()

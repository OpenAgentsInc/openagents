"""Carries the demo's recorded data over (pt-import-demo-data).

    python3 -I import_demo.py DATA_DIR --db CONNINFO [--psql PATH]

DATA_DIR is a copy of the bucket aethyr-demo-data (import_demo.sh makes one
with a read-only `gcloud storage rsync`; this never writes to it). The rules
of each table are in import_demo.sql, one transaction keyed on the demo's own
ids: run it again and only what the demo recorded since is added.

It prints, for each kind, what the files hold (source), what this run added
or brought up to date (added), what the database now holds from the demo
(held) and, for published worlds, the ones openagents.com had published
itself and keeps (kept); it exits 1 unless source = held + kept everywhere.

The demo's layout (aethyr-cpp tools/serve.py and tools/analytics.py at
demo/kiki-2026-10-28d; docs/verse/playtest-port.md): namespace "" (the Kiki
landing, space 'kiki') at the top, namespace "dev" (main's pages, the Haunted
House among them, space 'dev') under dev/. players.json is shared.
"""
import argparse, glob, hashlib, json, os, re, subprocess, sys

SOURCE = "aethyr-demo-data/"
EVENT_NAME = re.compile(r"^[a-z][a-z0-9-]{0,31}$")
SID = re.compile(r"^[A-Za-z0-9_:-]{1,80}$")


def rows(data):
    """(kind, key, space, doc) for every record in the copy, and the source counts."""
    out, count = [], {}

    def add(kind, key, space, doc):
        out.append((kind, SOURCE + key, space, doc))
        count[kind] = count.get(kind, 0) + 1

    surveys = {}
    for prefix, space in (("", "kiki"), ("dev/", "dev")):
        base = os.path.join(data, prefix)
        for path in sorted(glob.glob(os.path.join(base, "events", "*.jsonl"))):
            rel = prefix + "events/" + os.path.basename(path)
            with open(path, encoding="utf-8") as f:
                for n, line in enumerate(f, 1):
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        e = json.loads(line)
                    except ValueError:
                        count["bad lines"] = count.get("bad lines", 0) + 1
                        continue
                    if (not isinstance(e, dict) or not isinstance(e.get("t"), (int, float))
                            or not SID.match(str(e.get("sid", ""))) or not EVENT_NAME.match(str(e.get("ev", "")))):
                        count["bad lines"] = count.get("bad lines", 0) + 1
                        continue
                    add("event", "%s:%06d" % (rel, n), space, e)
        for folder, kind in (("playtesters", "playtester"), ("survey-answers", "answer"), ("surveys", "survey")):
            for path in sorted(glob.glob(os.path.join(base, folder, "*.json"))):
                with open(path, encoding="utf-8") as f:
                    rec = json.load(f)
                if kind == "survey":
                    surveys.setdefault((space, rec["id"]), (prefix + folder + "/" + os.path.basename(path), rec))
                else:
                    add(kind, prefix + folder + "/" + os.path.basename(path), space, rec)
        # Before the one-file-a-record folders, the forms were a list in survey.json.
        if os.path.exists(os.path.join(base, "survey.json")):
            with open(os.path.join(base, "survey.json"), encoding="utf-8") as f:
                for rec in json.load(f):
                    surveys.setdefault((space, rec["id"]), (prefix + "survey.json#" + rec["id"], rec))
        for path in sorted(glob.glob(os.path.join(base, "*.json"))):
            name = os.path.basename(path)
            with open(path, "rb") as f:
                raw = f.read()
            doc = json.loads(raw)
            add("file", prefix + name, space, {"doc": doc, "md5": hashlib.md5(raw).hexdigest()})
            if name == "players.json" and prefix == "":
                for h, p in sorted(doc.items()):
                    add("player", name + "#" + p["id"], space, {"hash": h, "id": p["id"], "at": p["at"]})
            elif name == "world-owners.json":
                for w, o in sorted(doc.items()):
                    add("world", prefix + name + "#" + w, space, {"id": w, "owner": o.get("owner"), "at": o.get("at")})
            elif name.startswith("world-"):
                add("published", prefix + name, space, dict(doc, world=name[6:-5]))
            elif name.startswith("log-"):
                for e in doc:
                    if e.get("state") in ("waiting", "accepted", "declined") and re.match(r"^[A-Za-z0-9_-]{1,64}$", str(e.get("id", ""))):
                        count["offer"] = count.get("offer", 0) + 1
                    if e.get("state") in ("published", "reset") and isinstance(e.get("version"), int):
                        add("log", "%s%s#%d@%r" % (prefix, name, e["version"], e["at"]), space, dict(e, world=name[4:-5]))
    # Awards, planets and saved copies: what verse.planets_from_demo() keeps.
    users, planets, published, saves = set(), set(), set(), 0
    for prefix in ("", "dev/"):
        base = os.path.join(data, prefix)
        ok = re.compile(r"^[A-Za-z0-9_-]{1,64}$")
        if os.path.exists(os.path.join(base, "awards.json")):
            with open(os.path.join(base, "awards.json"), encoding="utf-8") as f:
                users |= {u for u, p in json.load(f).items() if ok.match(u) and isinstance(p, str) and ok.match(p)}
        if os.path.exists(os.path.join(base, "planets.json")):
            with open(os.path.join(base, "planets.json"), encoding="utf-8") as f:
                planets |= {k for k, v in (json.load(f).get("planets") or {}).items()
                            if ok.match(k) and isinstance(v, dict) and ok.match(str(v.get("id", ""))) and isinstance(v.get("at"), (int, float))}
        for path in glob.glob(os.path.join(base, "world-planet-*.json")):
            with open(path, encoding="utf-8") as f:
                w = json.load(f)
            pl = (w.get("doc") or {}).get("planet") if isinstance(w.get("doc"), dict) else None
            if isinstance(pl, dict) and isinstance(w.get("publishedAt"), (int, float)) and os.path.basename(path) == "world-%s.json" % pl.get("id"):
                published.add(pl["id"])
        saves += len(glob.glob(os.path.join(base, "planet-save-*.json")))
    count["award"], count["planet"], count["planet_save"] = len(users), len(planets | published), saves
    for (space, _), (key, rec) in sorted(surveys.items()):
        add("survey", key, space, rec)
    return out, count


def copy_text(s):
    return s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n").replace("\r", "\\r")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("data")
    ap.add_argument("--db", required=True)
    ap.add_argument("--psql", default="psql")
    a = ap.parse_args()
    out, source = rows(a.data)
    here = os.path.dirname(os.path.abspath(__file__))
    text = "".join("\t".join((kind, copy_text(key), space, copy_text(json.dumps(doc, ensure_ascii=False)))) + "\n"
                   for kind, key, space, doc in out)
    r = subprocess.run([a.psql, a.db, "-X", "-q", "-At", "-F", "\t", "-f", os.path.join(here, "import_demo.sql")],
                       input=text, capture_output=True, text=True)
    if r.returncode:
        sys.stderr.write(r.stderr)
        return 2
    got = {"added": {}, "held": {}, "kept": {}}
    for line in r.stdout.splitlines():
        what, kind, n = line.split("\t")
        got[what][kind] = int(n)
    added, held, kept = got["added"], got["held"], got["kept"]
    kinds = {"event": "events", "player": "players", "world": "worlds", "published": "published", "log": "log",
             "playtester": "playtesters", "answer": "survey_answers", "survey": "surveys", "file": "files", "offer": "offers",
             "award": "awards", "planet": "planets", "planet_save": "planet_saves"}
    want = {kinds[k]: n for k, n in source.items() if k in kinds}
    want["sessions"] = len({doc["sid"] for kind, _, _, doc in out if kind == "event"})
    report = {k: {"source": want.get(k, 0), "added": added.get(k, 0), "held": held.get(k, 0)} for k in held}
    for k, n in kept.items():
        report[k]["kept"] = n
    if source.get("bad lines"):
        report["bad lines"] = {"source": source["bad lines"]}
    print(json.dumps(report, indent=1))
    bad = [k for k, v in report.items() if k != "bad lines" and v["source"] != v["held"] + v.get("kept", 0)]
    if bad:
        sys.stderr.write("counts differ: " + ", ".join(bad) + "\n")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

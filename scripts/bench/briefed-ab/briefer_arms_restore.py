"""Rebuild WORK/briefer-arms/ba1 from the committed judgments.csv (#11389).

Writes the frozen candidate lists (paths and order from the CSV, the
finder's reasons attached where a fresh filefind run lists the same path)
and the clef/hosted rows, so `briefer_arms.py run --arms haiku` can re-ask
Haiku over the identical candidates and `report` can compare. Run from
scripts/bench/briefed-ab with FILEFIND_EMBEDDINGS=vertex.
"""
import csv, json, sys
from pathlib import Path
from collections import defaultdict
sys.path.insert(0, str(Path.cwd()))
import briefer_arms as ba, briefing
plan = json.load(open("plans/briefer-arms.json"))
out = ba.WORK / "briefer-arms" / plan["tag"]
rows = defaultdict(lambda: defaultdict(list))
meta = {}
for r in csv.DictReader(open("results/briefer-arms-ba1/judgments.csv")):
    rows[r["arm"]][int(r["issue"])].append(r)
for issue in plan["issues"]:
    task = ba.task_for(issue)
    order = [r["path"] for r in rows["clef"][issue]]
    for arm in ("hosted", "haiku"):
        o = [r["path"] for r in rows[arm][issue] if r["path"]]
        if o and o != order and len(o) > len(order):
            order = o
    ff, used = briefing.finder_files(task, dict(ba.LEVERS, briefing_files=plan["candidates"]), no_cards=True)
    by = {f["path"]: f for f in ff}
    same = [f["path"] for f in ff] == order
    cands = [by.get(p, {"path": p}) for p in order]
    ba.dump_json(out / "candidates" / f"{issue}.json", {"issue": issue, "finder": used, "files": cands,
                 "restored_from": "judgments.csv", "finder_rerun_identical": same})
    print(issue, "identical" if same else f"DIFF {len(set(order)&set(by))}/{len(order)}", flush=True)
    for arm in ("clef", "hosted"):
        rs = rows[arm][issue]
        js = [{"path": r["path"], "p": float(r["p"]), "decision": r["decision"]} for r in rs if r["p"]]
        ba.dump_json(out / arm / f"{issue}.json", {
            "arm": arm, "ok": bool(js), "kind": None, "judgments": js,
            "keep": sorted(j["path"] for j in js if j["decision"] == "keep"), "extra": [],
            "secs": float(rs[0]["secs_issue"]) if rs else 0, "cost_usd": float(rs[0]["cost_usd_issue"] or 0) if rs else 0,
            "unpriced": 1 if arm == "hosted" else 0, "errors": [r["error"] for r in rs if r["error"]][:1],
            "issue": issue, "candidates": order, "fix": sorted(ba.labels(task)), "restored_from": "judgments.csv"})

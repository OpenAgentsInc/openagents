#!/usr/bin/env python3
"""Recompute retained efficiency measurements without importing project code."""
import argparse
import collections
import datetime
import fnmatch
import hashlib
import json
import math
import pathlib
import random
import statistics as st
import subprocess

COMMIT = "5e22f2af962f7e848df3aee1385e6cf165132123"
BOOTSTRAP_SEED = 20261003
MANIFEST = {}


def read(root, path):
    rel = str(pathlib.Path(path).relative_to(root)) if pathlib.Path(path).is_absolute() else str(path)
    data = subprocess.check_output(["git", "-C", str(root), "show", COMMIT + ":" + rel])
    MANIFEST[rel] = hashlib.sha256(data).hexdigest()
    return data


def tree(root):
    return subprocess.check_output(["git", "-C", str(root), "ls-tree", "-r", "--name-only", COMMIT], text=True).splitlines()

FILES = [
    "bench/efficiency/results/2026-10-03.jsonl",
    "docs/cost/2026-10-02-shadow-baseline/collected.jsonl",
    "docs/cost/2026-10-02-shadow-baseline/collected-10244.jsonl",
]


def wilson(k, n):
    z = 1.959963984540054
    p = k / n
    den = 1 + z*z/n
    center = (p + z*z/(2*n))/den
    half = z*math.sqrt(p*(1-p)/n + z*z/(4*n*n))/den
    return [center-half, center+half]


def quantile(xs, p):
    xs = sorted(xs)
    x = (len(xs)-1)*p
    lo, hi = math.floor(x), math.ceil(x)
    return xs[lo] + (xs[hi]-xs[lo])*(x-lo)


def interval(xs):
    return [quantile(xs, .025), quantile(xs, .975)]


def aggregate(rows):
    passes = [r for r in rows if r.get("passed") is True]
    out = {"n":len(rows), "passed":len(passes), "pass_wilson95":wilson(len(passes),len(rows))}
    keys = ["cost_usd", "wall_s", "engine_usd", "jev_usd", "jev_loop_usd", "jev_recipe_usd", "embedding_usd", "input_tokens", "cache_read", "cache_write", "output_tokens", "uncached_input", "steps", "requests", "turns"]
    for key in keys:
        vs = [r[key] for r in rows if isinstance(r.get(key),(float,int))]
        out[key] = {"present":len(vs), "sum":math.fsum(vs), "mean":st.mean(vs) if vs else None, "median":st.median(vs) if vs else None}
    out["cost_per_checked_pass"] = out["cost_usd"]["sum"]/len(passes) if passes else None
    out["sum_wall_per_pass"] = out["wall_s"]["sum"]/len(passes) if passes else None
    out["passing_wall_median"] = st.median(r["wall_s"] for r in passes) if passes else None
    out["cache_read_fraction"] = out["cache_read"]["sum"]/out["input_tokens"]["sum"]
    out["cache_write_fraction"] = out["cache_write"]["sum"]/out["input_tokens"]["sum"]
    out["tasks"] = {task:len(rs) for task,rs in sorted(group(rows,"task").items())}
    out["metadata"] = {k:dict(collections.Counter(json.dumps(r.get(k),sort_keys=True) for r in rows)) for k in ["engine","engines","models","effort","commit","host","claude","codex","task_class","ending","klass","recipe","projection","exit"]}
    out["cost_balance_max_error"] = max(abs(r["cost_usd"]-r["engine_usd"]-r["jev_usd"]) for r in rows)
    codex = [r for r in rows if "codex" in (r.get("engine") or "") or "codex" in r.get("engines",[]) or "codex" in r["arm"]]
    out["codex_pricing_max_error"] = max((abs(r["engine_usd"] - ((r["input_tokens"]-r.get("cache_read",0))*2 + r.get("cache_read",0)*.2 + r["output_tokens"]*10)/1e6) for r in codex), default=None)
    out["failed_rows"] = [{k:r.get(k) for k in ["task","trial","cost_usd","wall_s","ending","exit"]} for r in rows if r.get("passed") is not True]
    out["per_task"] = {t:{"n":len(rs),"passed":sum(r.get("passed") is True for r in rs),"cost_mean":st.mean(r["cost_usd"] for r in rs),"wall_mean":st.mean(r["wall_s"] for r in rs),"cost_median":st.median(r["cost_usd"] for r in rs),"wall_median":st.median(r["wall_s"] for r in rs)} for t,rs in group(rows,"task").items()}
    return out


def group(rows, key):
    out = collections.defaultdict(list)
    for r in rows: out[r.get(key)].append(r)
    return dict(out)


def compare(a,b,reps):
    ga,gb = group(a,"task"),group(b,"task")
    tasks=sorted(set(ga)&set(gb))
    out={"tasks":tasks,"by_metric":{}}
    for key in ["cost_usd","wall_s"]:
        va={t:[r[key] for r in ga[t]] for t in tasks}
        vb={t:[r[key] for r in gb[t]] for t in tasks}
        ma={t:st.mean(va[t]) for t in tasks}; mb={t:st.mean(vb[t]) for t in tasks}
        point=sum(ma.values())/sum(mb.values())
        rng=random.Random(BOOTSTRAP_SEED)
        stratified=[]; clustered=[]; hierarchical=[]
        for _ in range(reps):
            aa=sum(sum(rng.choices(va[t],k=len(va[t])))/len(va[t]) for t in tasks)
            bb=sum(sum(rng.choices(vb[t],k=len(vb[t])))/len(vb[t]) for t in tasks)
            stratified.append(aa/bb)
            picked=rng.choices(tasks,k=len(tasks))
            clustered.append(sum(ma[t] for t in picked)/sum(mb[t] for t in picked))
            aa=sum(sum(rng.choices(va[t],k=len(va[t])))/len(va[t]) for t in picked)
            bb=sum(sum(rng.choices(vb[t],k=len(vb[t])))/len(vb[t]) for t in picked)
            hierarchical.append(aa/bb)
        out["by_metric"][key]={
            "task_mean_ratio":point,"independent_stratified_bootstrap95":interval(stratified),
            "task_cluster_bootstrap95_sensitivity":interval(clustered),
            "task_and_trial_bootstrap95_sensitivity":interval(hierarchical),
            "ratio_of_all_run_medians":st.median(r[key] for r in a)/st.median(r[key] for r in b),
            "unweighted_mean_task_ratio":st.mean(ma[t]/mb[t] for t in tasks),
            "geometric_mean_task_ratio":math.exp(st.mean(math.log(ma[t]/mb[t]) for t in tasks)),
            "wins_by_task_mean":sum(ma[t]<mb[t] for t in tasks),
            "per_task_ratio":{t:ma[t]/mb[t] for t in tasks},
        }
    out["cost_per_pass_ratio"]=(sum(r["cost_usd"] for r in a)/sum(r["passed"] for r in a))/(sum(r["cost_usd"] for r in b)/sum(r["passed"] for r in b))
    out["same_task_trial_keys"] = sorted((r["task"],r["trial"]) for r in a)==sorted((r["task"],r["trial"]) for r in b)
    aa={(r["task"],r["trial"]):r for r in a};bb={(r["task"],r["trial"]):r for r in b}
    pairs=sorted(set(aa)&set(bb))
    out["descriptive_task_trial_differences"]=[{"task":t,"trial":i,"cost_arm_minus_base":aa[(t,i)]["cost_usd"]-bb[(t,i)]["cost_usd"],"wall_arm_minus_base":aa[(t,i)]["wall_s"]-bb[(t,i)]["wall_s"]} for t,i in pairs]
    savings=sum(bb[k]["cost_usd"]-aa[k]["cost_usd"] for k in pairs)
    delay=sum(aa[k]["wall_s"]-bb[k]["wall_s"] for k in pairs)
    out["batch_cost_saving_usd"]=savings
    out["batch_extra_wall_seconds"]=delay
    out["descriptive_synchronous_wait_breakeven_usd_per_hour"]=savings/delay*3600 if savings>0 and delay>0 else None
    return out


def matched(root):
    p=root/"docs/terminal-bench/2026-09-23-matched-opus-controller.json"
    data=json.loads(read(root,p))
    trials=data["trials"]
    def summarize(rows):
        passes=sum((r["reward"] if r["reward"] is not None else r.get("recovered_grade",{}).get("reward",0))==1 for r in rows)
        cost=sum(r["cost_usd"] for r in rows); wall=sum(r["agent_seconds"] for r in rows)
        native_errors=[]
        for r in rows:
            for session in r["sessions"]:
                q=root/session["path"]
                events=[json.loads(line) for line in read(root,q).splitlines() if line.strip()]
                results=[v for v in events if v.get("type")=="result"]
                total=sum(v.get("total_cost_usd",0) for v in results)
                native_errors.append(abs(total-session["cost_usd"]))
        return {"n":len(rows),"passes":passes,"cost":cost,"agent_seconds":wall,"cost_per_pass":cost/passes,"agent_seconds_per_pass":wall/passes,"controller_cost":sum(r.get("controller_cost_usd") or 0 for r in rows),"native_session_cost_max_error":max(native_errors),"tokens":{k:sum(s["usage"].get(k,0) for r in rows for s in r["sessions"]) for k in ["input_tokens","cache_creation_input_tokens","cache_read_input_tokens","output_tokens"]}}
    return {"path":str(p.relative_to(root)),"sha256":MANIFEST[str(p.relative_to(root))],"arms":{a:summarize(rs) for a,rs in group(trials,"arm").items()},"exclude_recovered_pair":{a:summarize([r for r in rs if not(r["task"]=="batched-eval-parity" and r["repetition"]==1)]) for a,rs in group(trials,"arm").items()}}


def harbor_rows(root, paths, patterns):
    rows=[]
    for path in paths:
        if not path.endswith(".episode/harbor-result.json") or not any(fnmatch.fnmatch(path,p) for p in patterns): continue
        d=json.loads(read(root,path)); a=d.get("agent_result") or {}; ex=d.get("agent_execution") or {}
        reward=(d.get("verifier_result") or {}).get("rewards",{}).get("reward")
        dt=lambda x: datetime.datetime.fromisoformat(x.replace("Z","+00:00"))
        seconds=(dt(ex["finished_at"])-dt(ex["started_at"])).total_seconds() if ex.get("finished_at") and ex.get("started_at") else None
        usage_path=path.removesuffix("harbor-result.json")+"evaluation/usage.json"
        usage=json.loads(read(root,usage_path)) if usage_path in paths else {}
        cost=usage.get("cost") or {}
        rows.append({"path":path,"task":d["task_name"],"reward":reward,"cost_usd":cost.get("amount_usd",a.get("cost_usd")),"harbor_cost_usd":a.get("cost_usd"),"controller_usd":cost.get("decisions_usd"),"agent_seconds":seconds,"input_tokens":a.get("n_input_tokens"),"cache_tokens":a.get("n_cache_tokens"),"output_tokens":a.get("n_output_tokens"),"exception":d.get("exception_info")})
    return rows


def retained_older(root):
    paths=tree(root);out={}
    for family,arms in {
        "targeted_matched":{
            "plain":["bench/terminal-bench/traces/tb4--claude-code-opus-matched--*--matched-v8-9567*/*.episode/harbor-result.json"],
            "coder":["bench/terminal-bench/traces/tb4--coder-one-matched-v8--*--matched-v8-9567*/*.episode/harbor-result.json"]},
        "development_eight":{
            "plain":["bench/terminal-bench/traces/panel--claude-code-opus--*/*.episode/harbor-result.json","bench/terminal-bench/traces/extended--claude-code-opus--*/*.episode/harbor-result.json","bench/terminal-bench/traces/smoke--claude-code-opus--fix-git/*.episode/harbor-result.json","bench/terminal-bench/traces/smoke--claude-code-opus--build-cython-ext/*.episode/harbor-result.json"],
            "coder":["bench/terminal-bench/traces/panel--coder-one-jevprobe2-opus-lean-low-5m--*/*.episode/harbor-result.json","bench/terminal-bench/traces/extended--coder-one-jevprobe2-opus-lean-low-5m--*/*.episode/harbor-result.json"]},
        "tb4_retained_subset_only":{
            "plain":["bench/terminal-bench/traces/tb4--claude-code-opus--*/*.episode/harbor-result.json"],
            "coder":["bench/terminal-bench/traces/tb4--coder-one-tunable-v2--*/*.episode/harbor-result.json"]},
    }.items():
        out[family]={}
        for arm,patterns in arms.items():
            rows=harbor_rows(root,paths,patterns); graded=[r for r in rows if r["reward"] is not None]
            passes=sum(r["reward"]==1 for r in graded);cost=sum(r["cost_usd"] or 0 for r in graded);secs=sum(r["agent_seconds"] or 0 for r in graded)
            out[family][arm]={"retained":len(rows),"graded":len(graded),"passes":passes,"cost":cost,"unpriced":sum(r["cost_usd"] is None for r in graded),"agent_seconds":secs,"cost_per_pass":cost/passes if passes else None,"seconds_per_pass":secs/passes if passes else None,"rows":rows}
    out["targeted_manifest"]=json.loads(read(root,"docs/terminal-bench/2026-09-23-matched-controller-targeted.json"))
    return out


def historical_examples(root):
    """Recompute historical examples from pinned public records only."""
    paths = tree(root)
    tb21_pattern = "bench/terminal-bench/microcoder-runs/coderos-4080-tb21/*/summary.json"
    tb21_rows = []
    for path in sorted(fnmatch.filter(paths, tb21_pattern)):
        raw = json.loads(read(root, path))
        outcome = raw["outcome"]
        record = pathlib.PurePosixPath(path).parent.name
        tb21_rows.append({
            "path": path,
            "record": record,
            "task": raw["task"],
            "reward": raw.get("reward"),
            "cost_usd": outcome.get("usd"),
            "known_usd": outcome.get("known_usd"),
            "usd_upper": outcome.get("usd_upper"),
            "seconds": outcome["seconds"],
            "steps": outcome["steps"],
            "kb": raw.get("kb"),
            "knowledge_assisted": outcome.get("knowledge_assisted"),
        })
    assert len(tb21_rows) == 127, "TB2.1 retained population changed"
    tasks = group(tb21_rows, "task")
    passing = [r for r in tb21_rows if r["reward"] == 1]
    known = [r["cost_usd"] for r in tb21_rows if r["cost_usd"] is not None]
    # Record-directory suffixes are the run-start timestamps. Recompute the
    # first screen outcome independently of the generated report's verdicts.
    screens = [min(rs, key=lambda r: int(r["record"].rsplit("-", 1)[1])) for rs in tasks.values()]
    report_path = "bench/terminal-bench/studies/2026-09-26-out-of-sample/t1-report.json"
    report = json.loads(read(root, report_path))
    reference = report["reference"]
    cost_wins = [r for r in passing if r["cost_usd"] is not None and r["cost_usd"] < reference[r["task"]]["usd_per_trial"]]
    cost_win_counts = collections.Counter(r["task"] for r in cost_wins)
    tb21 = {
        "source_pattern": tb21_pattern,
        "reference_path": report_path,
        "retained": len(tb21_rows),
        "unique_tasks": len(tasks),
        "passes": len(passing),
        "failures": sum(r["reward"] == 0 for r in tb21_rows),
        "unknown_rewards": sum(r["reward"] not in (0, 1) for r in tb21_rows),
        "screen_passes": sum(r["reward"] == 1 for r in screens),
        "screen_tasks": len(screens),
        "tasks_with_at_least_two_passes": sum(sum(r["reward"] == 1 for r in rs) >= 2 for rs in tasks.values()),
        "cost_wins_against_reference_mean_trial": len(cost_wins),
        "tasks_with_at_least_two_cost_wins": sum(n >= 2 for n in cost_win_counts.values()),
        "known_cost_runs": len(known),
        "unknown_cost_runs": len(tb21_rows) - len(known),
        "known_complete_run_cost_sum_usd": math.fsum(known),
        "complete_total_cost_usd": math.fsum(known) if len(known) == len(tb21_rows) else None,
        "known_cost_lower_bound_usd": math.fsum(r["known_usd"] if r["known_usd"] is not None else (r["cost_usd"] or 0) for r in tb21_rows),
        "passing_cost_median_usd": st.median(r["cost_usd"] for r in passing if r["cost_usd"] is not None),
        "passing_seconds_median": st.median(r["seconds"] for r in passing),
        "knowledge_assisted_counts": dict(collections.Counter(str(r["knowledge_assisted"]).lower() for r in tb21_rows)),
        "kb_counts": dict(collections.Counter(str(r["kb"]) for r in tb21_rows)),
        "reference_passes": sum(reference[t]["successes"] for t in tasks),
        "reference_trials": sum(reference[t]["trials"] for t in tasks),
        "per_task": {t: {"n": len(rs), "passes": sum(r["reward"] == 1 for r in rs)} for t, rs in sorted(tasks.items())},
        "rows": tb21_rows,
        "limits": "Confirmation repeats follow successful screens; 83/127 is not a full-suite reliability estimate. KB is off. Jev and loop contributions are not separately randomized. Reference cost is a different model's reported mean per trial, not matched individual winning-run cost. Subscription list price is not a cash invoice.",
    }
    assert tb21["passes"] == report["totals"]["passes"]
    assert tb21["tasks_with_at_least_two_cost_wins"] == report["totals"]["confirmed_wins"]

    v18_base = "bench/terminal-bench/experiments/2026-09-25-microluna-v18-family/"
    v18_path = v18_base + "measurement.json"
    measurement = json.loads(read(root, v18_path))
    rows = measurement["rows"]
    assert len(rows) == 18, "V18 completed population changed"
    card_rewards = []
    for row in rows:
        card = json.loads(read(root, v18_base + row["card"]))
        assert card["identity"]["reward"] == row["reward"], "V18 card and measurement disagree"
        card_rewards.append(card["identity"]["reward"])
    numeric_fields = ["recorded_lower_bound_usd", "counted_cost_usd", "jev_cost_usd", "jev_requests", "luna_cost_lower_bound_usd", "luna_calls", "trial_seconds", "agent_seconds", "unknown_calls", "signal_gap", "full_score_in_any_session", "baseline_entry_found", "finish_refusals", "unverified_sessions", "bound_ended_trial", "missing_program_turns", "missing_program_turns_before_first_edit", "oracle_complete"]
    totals = {key: math.fsum(row[key] for row in rows) for key in numeric_fields}
    v18 = {
        "path": v18_path,
        "retained": len(rows),
        "passes": sum(row["reward"] == 1 for row in rows),
        "failures": sum(row["reward"] == 0 for row in rows),
        "card_rewards_checked": len(card_rewards),
        "totals": totals,
        "reported_totals_max_error": max(abs(totals[key] - measurement["totals"][key]) for key in numeric_fields),
        "completed_cohort_outcome": measurement["completed_cohort_outcome"],
        "strict_protocol_verdict": measurement["strict_protocol_verdict"],
        "cost_per_pass": None,
        "agent_seconds_per_pass": None,
        "rows": rows,
        "limits": "The measured completed cohort passed no task; cost/time per pass is undefined. Counted cost uses a conservative estimate for unknown calls, not an invoice. Incomplete intermediate replay means the whole-cohort selector rescue ceiling is unknown. The formal protocol verdict differs from the completed-cohort numerical loss.",
    }

    packer = {"task": "log-summary-date-ranges", "arms": {}}
    for name, arm in [("old_section_packer", "coder-one-jevprobe3-luna"), ("coverage_packer", "coder-one-pack-luna")]:
        pattern = "bench/terminal-bench/traces/extended--" + arm + "--log-summary-date-ranges*/*.episode/harbor-result.json"
        rows = harbor_rows(root, paths, [pattern])
        assert len(rows) == 3, "Coverage-packer comparison population changed"
        packer["arms"][name] = {
            "source_pattern": pattern,
            "n": len(rows),
            "passes": sum(r["reward"] == 1 for r in rows),
            "cost_mean_usd": st.mean(r["cost_usd"] for r in rows),
            "agent_seconds_mean": st.mean(r["agent_seconds"] for r in rows),
            "rows": rows,
        }
    briefs = {
        "old_section_packer": "bench/terminal-bench/traces/extended--coder-one-jevprobe3-luna--log-summary-date-ranges/log-summary-date-ranges__XWSKgz5.episode/artifacts/delegate-1.briefing.md",
        "coverage_packer": "bench/terminal-bench/traces/extended--coder-one-pack-luna--log-summary-date-ranges/log-summary-date-ranges__Hsswu9g.episode/artifacts/delegate-1.briefing.md",
    }
    packer["representative_briefs"] = {name: {"path": path, "guidance": read(root, path).decode().rsplit("## What to do", 1)[1].strip()} for name, path in briefs.items()}
    packer["rendered_guidance_equal"] = packer["representative_briefs"]["old_section_packer"]["guidance"] == packer["representative_briefs"]["coverage_packer"]["guidance"]
    packer["limits"] = "Three repeats on one development task. Packer treatment includes requirement coverage, delivered records, omission labels and changed rendered guidance; the gain is not an isolation of only byte ranking. It does not establish a general pass-rate gain."
    return {"microcoder_tb21": tb21, "microluna_v18": v18, "coverage_packer": packer}


def main():
    global COMMIT
    ap=argparse.ArgumentParser();ap.add_argument("root");ap.add_argument("--commit",default=COMMIT);ap.add_argument("--reps",type=int,default=10000);ap.add_argument("--output",default="/tmp/openagents-audit-recompute-results.json");args=ap.parse_args();COMMIT=args.commit
    root=pathlib.Path(args.root).resolve();out={"schema":"openagents.audit.measurements.v1","commit":COMMIT,"input_mode":"git show COMMIT:path","bootstrap_replicates":args.reps,"bootstrap_seed":BOOTSTRAP_SEED,"bootstrap_percentiles":"linear interpolation at 2.5% and 97.5%","studies":{}}
    for rel in FILES:
        p=root/rel;rows=[json.loads(l) for l in read(root,p).splitlines() if l.strip()];arms=group(rows,"arm")
        keys=[(r["arm"],r["task"],r["trial"]) for r in rows]
        study={"sha256":MANIFEST[rel],"n":len(rows),"duplicate_arm_task_trial":len(keys)-len(set(keys)),"arms":{a:aggregate(rs) for a,rs in arms.items()},"comparisons":{}}
        starts = [datetime.datetime.fromisoformat(r["started"].replace("Z", "+00:00")) for r in rows if r.get("started")]
        ends = [datetime.datetime.fromisoformat(r["started"].replace("Z", "+00:00")) + datetime.timedelta(seconds=r["wall_s"]) for r in rows if r.get("started")]
        study["observed_execution_span_seconds"] = (max(ends)-min(starts)).total_seconds() if starts else None
        study["start_span_caveat"] = "Elapsed span across retained executions, not full scheduling/setup/check time; cohorts may run on different days."
        pairs=[(a,"raw-claude") for a in arms if a!="raw-claude"]
        if "raw-codex" in arms: pairs.append(("routed-default","raw-codex"))
        for engine in ["claude","codex"]:
            if f"routed-{engine}-off" in arms:pairs.append((f"routed-{engine}-on",f"routed-{engine}-off"))
        for a,b in pairs:study["comparisons"][a+" / "+b]=compare(arms[a],arms[b],args.reps)
        out["studies"][rel]=study
    out["matched_controller"]=matched(root)
    out["older_retained"]=retained_older(root)
    out["historical_examples"]=historical_examples(root)
    out["input_sha256"]=MANIFEST
    pathlib.Path(args.output).write_text(json.dumps(out,indent=2,sort_keys=True)+"\n")
    for name,s in out["studies"].items():
        print(name)
        for a,v in s["arms"].items(): print(a,"n/pass",v["n"],v["passed"],"cost",v["cost_usd"]["sum"],"cost/pass",v["cost_per_checked_pass"],"wall total/median",v["wall_s"]["sum"],v["wall_s"]["median"],"cache",v["cache_read_fraction"])
        for pair,c in s["comparisons"].items(): print(pair, {k:(v["task_mean_ratio"],v["independent_stratified_bootstrap95"],v["task_cluster_bootstrap95_sensitivity"]) for k,v in c["by_metric"].items()})
    print("matched",json.dumps(out["matched_controller"]["arms"]))


if __name__ == "__main__": main()

"""router_set.py URL CALLS.jsonl [N]: replay recorded router requests as the
split router sends them (main, answer, cli_group at once) and time each set."""
import json, sys, time, statistics, urllib.request
from concurrent.futures import ThreadPoolExecutor
url, calls = sys.argv[1], sys.argv[2]
n = int(sys.argv[3]) if len(sys.argv) > 3 else 47
SIDE = ("answer", "cli_group")
rows = [json.loads(l)["request"] for l in open(calls)]
rows = [r for r in rows if "cli_group" in r["questions"]][:n]
def post(body):
    req = urllib.request.Request(url, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    t = time.perf_counter()
    with urllib.request.urlopen(req, timeout=120) as r:
        d = json.loads(r.read())
    return time.perf_counter() - t, d
def split(r):
    main = {"model": "clef-flash", "state": r["state"], "questions": {k: v for k, v in r["questions"].items() if k not in SIDE}}
    sides = [{"model": "clef-flash", "state": r["state"], "questions": {k: r["questions"][k]}} for k in SIDE]
    return [main] + sides
pool = ThreadPoolExecutor(8)
post(split(rows[0])[0])
walls, tops, tokens = [], [], None
for r in rows:
    t = time.perf_counter()
    res = list(pool.map(post, split(r)))
    walls.append(time.perf_counter() - t)
    route = res[0][1]["answers"].get("route", {})
    tops.append(max(route.get("probabilities", {"x": 0}).values()) if isinstance(route, dict) and route.get("probabilities") else None)
    tokens = [d.get("usage", {}).get("input_tokens") for _, d in res]
walls.sort()
print(json.dumps({"sets": len(walls), "p50_s": round(statistics.median(walls), 3), "p90_s": round(walls[int(0.9 * len(walls)) - 1], 3),
                  "tokens_last": tokens, "route_top_p_median": statistics.median([t for t in tops if t is not None]) if any(tops) else None}))

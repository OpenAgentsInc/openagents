"""Decision latency at fixed prompt sizes against any System One server.

  bench_latency.py URL MODEL REQUEST.json [REQUEST.json ...]

Each request body is sent once to warm up and then RUNS (default 5) times,
every time with a fresh nonce line at the start of its state, so no server
can reuse a cached prefix. Prints one JSON line per request file: prompt
tokens, every latency and the median, and the first answer.
"""

import json
import os
import statistics
import sys
import time
import urllib.request
import uuid

url, model, files = sys.argv[1], sys.argv[2], sys.argv[3:]
runs = int(os.environ.get("RUNS", "5"))


def call(body):
    request = urllib.request.Request(
        url, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"}
    )
    began = time.perf_counter()
    with urllib.request.urlopen(request, timeout=600) as response:
        data = json.loads(response.read())
    return time.perf_counter() - began, data


for path in files:
    base = json.load(open(path, encoding="utf-8"))
    base["model"] = model
    times, last = [], None
    for index in range(runs + 1):
        body = dict(base)
        state = body["state"] if isinstance(body["state"], str) else json.dumps(body["state"])
        body["state"] = f"RUN {uuid.uuid4().hex}\n\n{state}"
        seconds, last = call(body)
        if index > 0:
            times.append(seconds)
    usage = last.get("usage", {})
    print(json.dumps({
        "file": os.path.basename(path),
        "input_tokens": usage.get("input_tokens"),
        "seconds": [round(t, 4) for t in times],
        "median_s": round(statistics.median(times), 4),
        "answers": last.get("answers"),
        "psionic": last.get("psionic"),
    }), flush=True)

"""Ask every reachable door the same Score questions and record the answers.

One item is one request with one `score` question, so nothing a door returns
depends on what else was packed beside it. The answers land in
`results/<door>.jsonl`, one row per item, and `analyze.py` reads them.

Items come from two places:

- `ramps.json`, the twelve five-level severity ramps authored here. Each ramp
  holds its subject fixed and walks the true level from 0 to 4.
- the `severity` family of `crates/lev/suites/support-v2.json`, 36 items on
  the three-level rubric this repository already reports Score numbers for.

Usage:

    python3 probe.py --door kev-0.5b
    python3 probe.py --door all
    python3 probe.py --list

The hosted door needs a key. Export `TYPESAFE_API_KEY`, or point
`--env-file` at a `KEY=value` file outside the repository. A key is never
read from a tracked file and never printed.
"""

import argparse
import json
import os
import pathlib
import sys
import time
import urllib.error
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
SUITE = REPO / "crates" / "lev" / "suites" / "support-v2.json"
RESULTS = HERE / "results"

KEV = "http://127.0.0.1:8009/v1/systemone"

#: Every door the probe knows how to reach. `auth` names the environment
#: variable holding a bearer token; the hosted door is the only one that
#: needs one, and it is read from the environment, never from this file.
DOORS = {
    "kev-0.5b": {"url": KEV, "model": "kev-0.5b"},
    "kev-0.6b": {"url": KEV, "model": "kev-0.6b"},
    "kev-4b": {"url": KEV, "model": "kev-4b"},
    "kev-8b": {"url": KEV, "model": "kev-8b"},
    "jev": {
        "url": "https://api.typesafe.ai/v1/systemone",
        "model": "jev-latest",
        "auth": "TYPESAFE_API_KEY",
    },
    "lev-base": {"url": "http://127.0.0.1:11436/v1/systemone", "model": None},
    "lev-v1": {"url": "http://127.0.0.1:11437/v1/systemone", "model": None},
    "lev-band": {"url": "http://127.0.0.1:11438/v1/systemone", "model": None},
    "lev-perm": {"url": "http://127.0.0.1:11439/v1/systemone", "model": None},
}


def load_env(path):
    """Read `KEY=value` lines into the environment, for the hosted door."""
    for line in pathlib.Path(path).expanduser().read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        os.environ.setdefault(key.strip(), value.strip().strip("\"'"))


def items():
    """Every probe item, as `(id, family, truth, state, question)` records."""
    out = []
    ramps = json.loads((HERE / "ramps.json").read_text())
    rubric = ramps["rubric"]
    for ramp in ramps["ramps"]:
        for level, state in enumerate(ramp["states"]):
            out.append(
                {
                    "id": f"ramp/{ramp['id']}/{level}",
                    "family": "ramp",
                    "ramp": ramp["id"],
                    "truth": level,
                    "levels": len(rubric["criteria"]),
                    "state": state,
                    "question": {
                        "type": "score",
                        "instructions": rubric["instructions"],
                        "criteria": rubric["criteria"],
                    },
                }
            )
    suite = json.loads(SUITE.read_text())
    for item in suite["items"]:
        if item.get("family") != "severity":
            continue
        out.append(
            {
                "id": item["id"],
                "family": "severity",
                "ramp": None,
                "truth": int(item["truth"]),
                "levels": len(item["question"]["criteria"]),
                "state": item["state"],
                "question": item["question"],
            }
        )
    return out


def ask(door, item, timeout=180.0):
    """Put one item to one door and return the parsed answer."""
    body = {"state": item["state"], "questions": {"q": item["question"]}}
    if door.get("model"):
        body["model"] = door["model"]
    headers = {"content-type": "application/json"}
    if door.get("auth"):
        key = os.environ.get(door["auth"])
        if not key:
            raise SystemExit(f"set {door['auth']} before probing this door")
        headers["authorization"] = f"Bearer {key}"
    request = urllib.request.Request(
        door["url"], data=json.dumps(body).encode(), headers=headers
    )
    started = time.time()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            payload = json.loads(response.read())
    except urllib.error.HTTPError as error:
        if error.code >= 500:
            raise
        raise SystemExit(
            f"{item['id']}: HTTP {error.code} {error.read().decode()[:400]}"
        ) from error
    elapsed = time.time() - started
    answer = payload["answers"]["q"]
    if answer.get("type") != "score":
        raise SystemExit(f"{item['id']}: door answered {answer.get('type')}")
    return {
        "score": answer["score"],
        "confidence": answer.get("confidence"),
        "probabilities": answer["probabilities"],
        "usage": payload.get("usage", {}),
        "seconds": round(elapsed, 3),
    }


def run(name, retries=3):
    """Probe one door and write `results/<name>.jsonl`."""
    door = DOORS[name]
    RESULTS.mkdir(exist_ok=True)
    out = RESULTS / f"{name}.jsonl"
    rows = []
    work = items()
    for index, item in enumerate(work, start=1):
        for attempt in range(retries):
            try:
                answer = ask(door, item)
                break
            except (urllib.error.URLError, TimeoutError, OSError) as error:
                if attempt == retries - 1:
                    raise SystemExit(f"{name} {item['id']}: {error}")
                time.sleep(2.0 * (attempt + 1))
        row = {k: item[k] for k in ("id", "family", "ramp", "truth", "levels")}
        row["door"] = name
        row.update(answer)
        rows.append(row)
        print(
            f"\r{name}: {index}/{len(work)} {item['id']:<28}"
            f" score={answer['score']:.2f}",
            end="",
            file=sys.stderr,
        )
    print("", file=sys.stderr)
    with out.open("w") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")
    total = sum(r["seconds"] for r in rows)
    print(f"{name}: {len(rows)} items, {total:.0f}s wall, wrote {out}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--door", default="all", help="a door name, or `all`")
    parser.add_argument("--list", action="store_true", help="list the doors")
    parser.add_argument(
        "--env-file", default=None, help="a KEY=value file outside the repository"
    )
    args = parser.parse_args()
    if args.env_file:
        load_env(args.env_file)
    if args.list:
        for name, door in DOORS.items():
            print(f"{name:<10} {door['url']}")
        return
    names = list(DOORS) if args.door == "all" else [args.door]
    for name in names:
        if name not in DOORS:
            raise SystemExit(f"unknown door {name}; --list shows them")
        run(name)


if __name__ == "__main__":
    main()

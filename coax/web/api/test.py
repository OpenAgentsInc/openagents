"""Checks for the API service (test.sh runs them against a throwaway database)."""
import json, os, subprocess, sys, urllib.request

API = os.environ["API"]
PSQL = [os.environ["PSQL"], os.environ["PGURL"], "-Atc"]
failed = 0


def check(name, ok, detail=""):
    global failed
    print(("  ok    " if ok else "  FAIL  ") + name + ("" if ok else "  " + str(detail)))
    failed += 0 if ok else 1


def call(method, path, body=None, headers=None):
    req = urllib.request.Request(API + path, method=method, data=body, headers=headers or {})
    try:
        with urllib.request.urlopen(req) as r:
            return r.status, r.read(), dict(r.headers)
    except urllib.error.HTTPError as e:
        return e.code, e.read(), dict(e.headers)


def sql(q):
    return subprocess.run(PSQL + [q], capture_output=True, text=True, check=True).stdout.strip()


status, body, _ = call("GET", "/api/playtest/health")
h = json.loads(body)
check("health answers with a database round trip", status == 200 and h["ok"] and h["database"] == "oa_analytics", body)
check("every migration applied once", sql("select count(*) from schema_migrations") == str(len([f for f in os.listdir(os.path.join(os.path.dirname(__file__), "migrations")) if f.endswith(".sql")])))
check("an unknown path is a 404", call("GET", "/api/nothing")[0] == 404)

for extra in sorted(f for f in os.listdir(os.path.dirname(os.path.abspath(__file__))) if f.startswith("test_") and f.endswith(".py")):
    exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), extra)).read())

sys.exit(1 if failed else 0)

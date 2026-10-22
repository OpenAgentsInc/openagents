#!/usr/bin/env python3
"""Watch the Grid's build billboard update from live NIP-DEPLOY test events.

Run under `openagents browser run -- python3 check_board.py URL OUT CLI`.
Publishes test events (--test) to the public relay with CLI and captures
the board after each step.
"""
import base64, importlib.util, json, os, subprocess, sys, time, urllib.request
from pathlib import Path

url, out, cli = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
out.mkdir(parents=True, exist_ok=True)
root = Path(Path(__file__).resolve().parents[4])
spec = importlib.util.spec_from_file_location("cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py")
mod = importlib.util.module_from_spec(spec); spec.loader.exec_module(mod)
port = os.environ["OPENAGENTS_CHROME_PORT"]
target = json.load(urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
cdp = mod.Cdp(target["webSocketDebuggerUrl"])
cdp.call("Page.enable"); cdp.call("Runtime.enable")
cdp.call("Emulation.setDeviceMetricsOverride", {"width": 1280, "height": 800, "deviceScaleFactor": 1, "mobile": False})
receipt = {"url": url, "steps": []}

def ev(expr):
    r = cdp.eval(expr)
    if "exceptionDetails" in r: raise RuntimeError(r["exceptionDetails"])
    return r["result"].get("value")

def board():
    try:
        t = ev('import("/everglade_web.js").then(m=>m.verse_build_board())')
    except RuntimeError:
        return None
    return json.loads(t) if t else None

def wait(cond, label, timeout=120):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        b = board()
        if b and cond(b): return b
        time.sleep(0.25)
    raise TimeoutError((label, board()))

def capture(name):
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (out / f"{name}.png").write_bytes(base64.b64decode(png))

RUN = f"check-{int(time.time())}"
START = int(time.time())
def publish(env, phase, run=RUN, commit="2ac6f0c4ce", tier="image"):
    r = subprocess.run([cli, "deploy", "event", "--env", env, "--phase", phase, "--run", run, "--commit", commit,
                        "--tier", tier, "--by", "agent", "--started", str(START), "--test", "--json"],
                       capture_output=True, text=True, cwd=root)
    assert r.returncode == 0, r.stderr
    return json.loads(r.stdout.strip().splitlines()[-1])["id"]

def step(name, b):
    receipt["steps"].append({"step": name, "board": b, "at": time.time()})
    print(name, json.dumps(b), flush=True)

cdp.call("Page.navigate", {"url": url})
b = wait(lambda b: b["subscribed"] and b["staging"]["word"], "first events")
step("loaded", b)
time.sleep(2)
capture("01-overview")
# Join, then face the board from the spawn.
point = {"x": 640, "y": 700}
for kind in ("mousePressed", "mouseReleased"):
    cdp.call("Input.dispatchMouseEvent", {"type": kind, "x": point["x"], "y": point["y"], "button": "left", "clickCount": 1})
time.sleep(4)
ev('import("/everglade_web.js").then(m=>m.verse_face(-26, 30))')
time.sleep(2)

publish("staging", "build_started")
b = wait(lambda b: b["staging"]["word"] == "BUILDING", "building")
step("building", b); time.sleep(1); capture("02-staging-building")
publish("staging", "build_done"); publish("staging", "deploying"); publish("staging", "smoke_running")
b = wait(lambda b: b["staging"]["word"] == "SMOKE TEST", "smoke")
step("smoke", b); capture("03-staging-smoke")
publish("staging", "smoke_passed"); publish("staging", "live")
b = wait(lambda b: b["staging"]["word"] == "LIVE" and b["staging"]["flash"] == "Live", "live")
step("live", b); capture("04-staging-live-flash")
prod = RUN + "-p"
publish("production", "deploying", run=prod); publish("production", "promoted", run=prod)
publish("production", "smoke_running", run=prod); publish("production", "smoke_failed", run=prod)
b = wait(lambda b: b["production"]["word"] == "SMOKE FAILED" and b["production"]["flash"] == "Failed", "failed")
step("failed", b); capture("05-production-failed-flash")
time.sleep(8)
b = board(); step("settled", b); capture("06-settled")
(out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")

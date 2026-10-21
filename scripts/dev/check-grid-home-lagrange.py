#!/usr/bin/env python3
"""Walk the Grid homepage through the Lagrange 2 arch and back (#12438).

Run under `openagents browser run`. Joins the Grid on `/`, walks into the
Lagrange 2 arch, and checks three things: the load screen is up with its
bar moving, the zone uses its own controls (the zone panel, A turns rather
than strafes, Space climbs, a panel button acts), and the Pylon overlay is
hidden. Then it presses the panel's The Grid button and checks the load
screen again and that the overlay is back. Captures and `capture.json`
(the `verse-zone` and `verse-zone-load` events, the load screen's samples,
and each check) go to --out.
"""
import argparse, base64, importlib.util, json, math, os, time, urllib.request
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--url", required=True)
p.add_argument("--out", required=True, type=Path)
p.add_argument("--width", type=int, default=1440)
p.add_argument("--height", type=int, default=900)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
port = os.environ["OPENAGENTS_CHROME_PORT"]
t = json.load(urllib.request.urlopen(urllib.request.Request(
    f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
cdp = m.Cdp(t["webSocketDebuggerUrl"])
cdp.call("Page.enable"); cdp.call("Runtime.enable")
# Record the page's zone events and sample the load screen every frame.
cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
globalThis.__events=[];globalThis.__load=[];
for(const n of ["verse-zone","verse-zone-load"])addEventListener(n,e=>__events.push({name:n,t:Math.round(performance.now()),detail:e.detail}));
const sample=()=>{const c=document.getElementById("verse-load");
if(c&&c.style.display!=="none"){const f=c.querySelector("[role=progressbar]");
const last=__load[__load.length-1];const s={t:Math.round(performance.now()),opacity:c.style.opacity,
title:c.querySelector("p").textContent,value:f&&f.getAttribute("aria-valuenow"),detail:c.querySelectorAll("p")[1].textContent};
if(!last||last.value!==s.value||last.opacity!==s.opacity||last.detail!==s.detail)__load.push(s)}
requestAnimationFrame(sample)};requestAnimationFrame(sample);"""})
cdp.call("Emulation.setDeviceMetricsOverride", {
    "width": a.width, "height": a.height, "deviceScaleFactor": 1, "mobile": False})

def ev(x):
    r = cdp.eval(x)
    if "exceptionDetails" in r:
        raise RuntimeError(r["exceptionDetails"])
    return r["result"].get("value")

def where():
    return ev('import("/everglade/everglade_web.js").then(m=>JSON.parse(m.verse_where()))')

def shot(name):
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (a.out / f"{name}.png").write_bytes(base64.b64decode(png))

def key(code, down):
    cdp.call("Input.dispatchKeyEvent", {
        "type": "keyDown" if down else "keyUp", "code": code,
        "key": " " if code == "Space" else code[-1].lower()})

def click(x, y):
    for kind in ("mousePressed", "mouseReleased"):
        cdp.call("Input.dispatchMouseEvent", {
            "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1})

def overlay():
    return ev("""({field:getComputedStyle(document.getElementById('pylons-field')).visibility,
      hint:(e=>e&&getComputedStyle(e).visibility)(document.getElementById('grid-controls')),
      place:document.documentElement.dataset.versePlace,
      pylons:document.querySelectorAll('#pylons-field .pylon').length})""")

def wait(cond, seconds, what):
    dl = time.monotonic() + seconds
    while True:
        value = cond()
        if value:
            return value
        assert time.monotonic() < dl, what
        time.sleep(.1)

checks = {}
cdp.call("Page.navigate", {"url": a.url})
wait(lambda: ev('performance.getEntriesByName("verse-visible").length > 0'), 120, "no verse")
wait(lambda: ev("document.querySelectorAll('#pylons-field .pylon').length") > 0, 60, "no pylons")
key("KeyW", True); key("KeyW", False)
wait(lambda: ev('performance.getEntriesByName("home-third-person").length > 0'), 30, "not joined")
time.sleep(2)
checks["grid_before"] = {"where": where(), "overlay": overlay()}
shot("1-grid-before")

# Walk to the arch's approach, then through it.
def walk_to(x, z, stop):
    for _ in range(4):
        ev(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({x},{z}))')
        time.sleep(.15)
        key("KeyW", True)
        dl = time.monotonic() + 20
        while time.monotonic() < dl:
            w = where()
            if w["place"] != "grid":
                key("KeyW", False); return True
            px, _, pz, _ = w["player"]
            if math.hypot(px - x, pz - z) < stop:
                break
            time.sleep(.05)
        key("KeyW", False)
        if math.hypot(px - x, pz - z) < stop:
            return False
    return False

approach = (-17.46, 6.59)
yaw = -0.8106
walk_to(*approach, 0.6)
ahead = (approach[0] + math.sin(yaw) * 12, approach[1] + math.cos(yaw) * 12)
ev(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({ahead[0]},{ahead[1]}))')
time.sleep(.2)
key("KeyW", True)
wait(lambda: ev("(e=>e&&e.style.display!=='none')(document.getElementById('verse-load'))"), 20, "no load screen")
key("KeyW", False)
shot("2-load-screen-in")
checks["loading_in"] = {"where": where(), "overlay": overlay()}
wait(lambda: where()["place"] == "zone", 60, "zone never ready")
time.sleep(1.5)
w = where()
checks["zone"] = {"where": w, "overlay": overlay()}
shot("3-lagrange2")

# The zone's own controls.
before = where()["player"]
key("KeyA", True); time.sleep(.6); key("KeyA", False); time.sleep(.2)
after = where()["player"]
checks["a_turns"] = {"before": before, "after": after,
                     "yaw_change": after[3] - before[3],
                     "moved": math.dist(before[:3], after[:3])}
before = where()["player"]
for _ in range(10):
    key("Space", True); time.sleep(.03); key("Space", False); time.sleep(.03)
time.sleep(.2)
after = where()["player"]
checks["space_climbs"] = {"before": before, "after": after, "dy": after[1] - before[1]}
panel = {b["label"]: b["frame"] for b in where()["panel"] or []}
fx, fy, fw, fh = panel["Forces"]
click(fx + fw / 2, fy + fh / 2); time.sleep(.5)
checks["forces_button"] = [b["label"] for b in where()["panel"]]
shot("4-lagrange2-forces")

# Back to the Grid through the panel's button.
gx, gy, gw, gh = {b["label"]: b["frame"] for b in where()["panel"]}["The Grid"]
click(gx + gw / 2, gy + gh / 2)
# The way back is quick; the frame samples show the screen went up.
wait(lambda: ev("__load.some(s=>s.title.startsWith('Back'))"), 10, "no load screen back")
shot("5-load-screen-back")
checks["loading_back"] = {"where": where(), "overlay": overlay()}
wait(lambda: where()["place"] == "grid", 60, "grid never ready")
time.sleep(2)
checks["grid_after"] = {"where": where(), "overlay": overlay()}
shot("6-grid-after")
checks["events"] = ev("__events")
checks["load_samples"] = ev("__load")
(a.out / "capture.json").write_text(json.dumps(checks, indent=1))

ok = (checks["grid_before"]["overlay"]["field"] == "visible"
      and checks["loading_in"]["overlay"]["field"] == "hidden"
      and checks["zone"]["where"]["scheme"] == "Station"
      and checks["zone"]["overlay"]["field"] == "hidden"
      and checks["zone"]["overlay"]["hint"] in ("hidden", None)
      and abs(checks["a_turns"]["yaw_change"]) > 0.3 and checks["a_turns"]["moved"] < 0.2
      and checks["space_climbs"]["dy"] > 0.1
      and "Hide forces" in checks["forces_button"]
      and checks["grid_after"]["overlay"]["field"] == "visible"
      and [e["detail"]["phase"] for e in checks["events"] if e["name"] == "verse-zone-load"]
          == ["start", "ready", "start", "ready"])
print(json.dumps({k: v for k, v in checks.items() if k not in ("events", "load_samples")}, indent=1)[:4000])
print("PASS" if ok else "FAIL")
raise SystemExit(0 if ok else 1)

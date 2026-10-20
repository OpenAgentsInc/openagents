#!/usr/bin/env python3
"""Check the real /home world under `openagents browser run -- python3 ...`."""

import argparse
import atexit
import base64
import importlib.util
import json
import os
from pathlib import Path
import time
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--url", default="http://localhost:4331/home")
parser.add_argument("--out", required=True, type=Path)
parser.add_argument("--timeout", type=float, default=120)
args = parser.parse_args()
args.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "browser_cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py"
)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
port = os.environ["OPENAGENTS_CHROME_PORT"]
request = urllib.request.Request(
    f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT"
)
target = json.load(urllib.request.urlopen(request))
cdp = module.Cdp(target["webSocketDebuggerUrl"])
cdp.call("Page.enable")
cdp.call("Runtime.enable")
cdp.call("Log.enable")
cdp.call("Network.enable")
atexit.register(lambda: (args.out / "browser-events.json").write_text(json.dumps(cdp.events)))
cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
let camera;
Object.defineProperty(globalThis,"pylonsCamera",{
    configurable:true,
    get(){return camera},
    set(draw){camera=(matrix,width,height)=>{
        draw(matrix,width,height);
        globalThis.__pylonsFrame={matrix:Array.from(matrix),width,height};
    }}
});
"""})
cdp.call("Emulation.setDeviceMetricsOverride", {
    "width": 1440, "height": 900, "deviceScaleFactor": 1, "mobile": False,
})


def evaluate(expression):
    result = cdp.eval(expression)
    if "exceptionDetails" in result:
        raise RuntimeError(result["exceptionDetails"])
    return result["result"].get("value")


def sample_anchors():
    assert evaluate('document.querySelector("#everglade-canvas.ready") !== null'), "Verse stopped."
    sample = evaluate("""(()=>{
        const frame=globalThis.__pylonsFrame;
        if(!frame)return null;
        const pylons=Array.from(document.querySelectorAll("#pylons-field [data-world-anchor]"))
            .map(p=>({id:p.dataset.pylon,anchor:p.dataset.worldAnchor.split(",").map(Number),
                hidden:p.classList.contains("hid"),x:parseFloat(p.style.getPropertyValue("--x")),
                y:parseFloat(p.style.getPropertyValue("--y")),transition:getComputedStyle(p).transitionProperty}));
        return {frame,pylons};
    })()""")
    assert sample is not None, "No world camera handoff: rebuild both the pylons layer and Verse."
    frame = sample["frame"]
    for pylon in sample["pylons"]:
        if pylon["hidden"]:
            continue
        point = pylon["anchor"] + [1]
        matrix = frame["matrix"]
        clip = [sum(matrix[column * 4 + row] * point[column] for column in range(4))
                for row in range(4)]
        assert clip[3] > 0 and 0 <= clip[2] <= clip[3], pylon
        x = (clip[0] / clip[3] + 1) * frame["width"] / 2
        y = (1 - clip[1] / clip[3]) * frame["height"] / 2
        assert abs(x - pylon["x"]) < .11 and abs(y - pylon["y"]) < .11, pylon
        assert "transform" not in pylon["transition"], pylon
    return sample


cdp.call("Page.navigate", {"url": args.url})
deadline = time.monotonic() + args.timeout
while not evaluate('performance.getEntriesByName("verse-visible").length > 0'):
    if time.monotonic() >= deadline:
        raise TimeoutError(evaluate(
            'JSON.stringify({marks:performance.getEntriesByType("mark")'
            '.map(x=>x.name),status:document.getElementById("everglade-status")?.textContent})'
        ))
    time.sleep(.25)

# Read the real renderer's state. A page shell or a handoff fixture can't pass.
state = evaluate(
    'import("/everglade/everglade_web.js").then(m=>JSON.parse(m.verse_where()))'
)
assert state["draw"] == "grid" and state["stopped"] is False, state
assert evaluate('document.querySelector("#everglade-canvas.ready") !== null'), evaluate(
    'document.getElementById("everglade-status").textContent'
)
assert evaluate('Number(getComputedStyle(document.getElementById("everglade-canvas")).opacity)') == 1
assert evaluate('document.querySelector("h1").textContent') == "OpenAgents"
assert evaluate(
    'Array.from(document.querySelectorAll("input,textarea"))'
    '.every(x=>getComputedStyle(x).display==="none")'
)
marks = evaluate('performance.getEntriesByType("mark").map(x=>({name:x.name,ms:x.startTime}))')
times = {mark["name"]: mark["ms"] for mark in marks}
assert times["lander5-presented"] < times["pylons-layer"] < times["verse-start"]
assert times["verse-start"] <= times["verse-first-frame"] <= times["verse-visible"]
assert not [e for e in cdp.events if e.get("method") == "Network.webSocketCreated"]
samples = []
for name in ("home-verse.png", "home-orbit.png"):
    samples.append(sample_anchors())
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (args.out / name).write_bytes(base64.b64decode(png))
    if name == "home-verse.png":
        time.sleep(2)
assert samples[0]["frame"]["matrix"] != samples[1]["frame"]["matrix"], "Camera did not orbit."
cdp.call("Emulation.setDeviceMetricsOverride", {
    "width": 720, "height": 900, "deviceScaleFactor": 1, "mobile": False,
})
time.sleep(.5)
samples.append(sample_anchors())
anchors = {}
for sample in samples:
    for pylon in sample["pylons"]:
        assert anchors.setdefault(pylon["id"], pylon["anchor"]) == pylon["anchor"], pylon
result = {"url": args.url, "state": state, "marks": marks, "world_anchors": samples}
(args.out / "home-check.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))

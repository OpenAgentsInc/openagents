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
parser.add_argument("--chat", action="store_true", help="Submit a test chat; use only a local fixture or dedicated test account.")
parser.add_argument("--allow-context-loss", action="store_true", help="Check input/API behavior despite a software GPU failure; does not verify rendering.")
parser.add_argument("--movement", action="store_true", help="Walk to the shared terminal and check typing (fresh spawn).")
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
    set(draw){camera=(matrix,width,height,player)=>{
        draw(matrix,width,height,player);
        globalThis.__pylonsFrame={matrix:Array.from(matrix),width,height,player:Array.from(player)};
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
    '.every(x=>x.disabled||getComputedStyle(x).display==="none")'
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
if args.movement:
    assert evaluate('document.querySelector("#everglade-canvas").dataset.pylonTerminal') == "world"
    assert evaluate('document.querySelector("svg#pylons-terminal") === null'), "Computer still overlays the world."
    assert evaluate('document.querySelector("#pylon-composer").hasAttribute("inert")')

    def position():
        return evaluate('globalThis.__pylonsFrame.player')

    def key(code, down, text=None, repeat=False):
        name = {"Escape": "Escape", "Enter": "Enter", "Space": " "}.get(code, code[-1].lower())
        event = {"type": "keyDown" if down else "keyUp", "code": code, "key": name, "autoRepeat": repeat}
        if text is not None:
            event["text"] = text
        cdp.call("Input.dispatchKeyEvent", event)

    def wait_for(condition, label, timeout=60):
        deadline = time.monotonic() + timeout
        while not condition():
            if time.monotonic() > deadline:
                raise TimeoutError((label, position(), evaluate('document.querySelector("#home-chat-status").textContent')))
            time.sleep(.05)

    def walk_until(code, condition):
        key(code, True)
        try:
            wait_for(lambda: condition(position()), code)
        finally:
            key(code, False)

    def draft_chars():
        return evaluate('Number(document.querySelector("#pylon-composer").dataset.draftChars)')

    start = position()
    x = start[0]
    walk_until("KeyA", lambda p: abs(p[0] - x) >= .8)
    walk_until("KeyD", lambda p: abs(p[0] - x) <= .2)
    key("KeyW", True)
    try:
        wait_for(lambda: evaluate('document.activeElement?.id === "pylon-message"'), "GPUI autofocus")
        key("KeyW", True, "w", repeat=True)
        time.sleep(.2)
        assert draft_chars() == 0, "Held walking key leaked into the composer."
    finally:
        key("KeyW", False)
    time.sleep(.4)
    assert evaluate('document.querySelector("#home-chat-mount canvas") !== null'), "No GPUI canvas."
    assert evaluate('getComputedStyle(document.querySelector("#pylon-composer")).opacity') == "1"
    assert evaluate('document.querySelector("#pylon-composer").getAttribute("aria-hidden")') == "false"
    typing_at = position()
    draft = "hello pylons wasd "
    for char in draft:
        code = "Space" if char == " " else "Key" + char.upper()
        key(code, True, char)
        key(code, False)
    wait_for(lambda: draft_chars() == len(draft), "GPUI typed draft")
    time.sleep(.3)
    assert all(abs(a-b) < .02 for a,b in zip(typing_at, position())), (typing_at, position())
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (args.out / "home-terminal.png").write_bytes(base64.b64decode(png))
    key("Escape", True)
    key("Escape", False)
    assert evaluate('document.querySelector("#pylon-composer").hasAttribute("inert")')
    walk_until("KeyS", lambda p: p[2] <= -.5)
    walk_until("KeyW", lambda _: evaluate('document.activeElement?.id === "pylon-message"'))
    wait_for(lambda: draft_chars() == len(draft), "GPUI draft recovery")
    samples.append(sample_anchors())
    result["terminal"] = {"start": start, "typing_at": typing_at, "returned": position(), "draft_chars": draft_chars()}
    if args.chat:
        wait_for(lambda: evaluate('document.querySelector("#pylon-composer").dataset.chatState === "ready"'), "Chat connection")
        key("Enter", True)
        key("Enter", False)
        wait_for(lambda: evaluate('Number(document.querySelector("#pylon-composer").dataset.messageCount)>=2 && document.querySelector("#pylon-composer").dataset.chatState === "ready"'), "GPUI streamed reply")
        posts = [e["params"]["request"] for e in cdp.events if e.get("method") == "Network.requestWillBeSent" and e["params"]["request"]["method"] == "POST" and "/v1/threads" in e["params"]["request"]["url"]]
        assert len(posts) == 1, posts
        assert json.loads(posts[0]["postData"])["text"] == draft.strip(), posts
        assert evaluate('location.pathname') == "/home"
        assert evaluate('new URLSearchParams(location.search).has("chat")')
        result["chat"] = {"submitted": json.loads(posts[0]["postData"]), "messages": evaluate('Number(document.querySelector("#pylon-composer").dataset.messageCount)')}
        png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
        (args.out / "home-chat.png").write_bytes(base64.b64decode(png))
    evaluate('window.dispatchEvent(new Event("verse-stopped"))')
    assert evaluate('document.querySelector("#pylon-composer").hasAttribute("inert")')
losses = [e["params"]["entry"]["text"] for e in cdp.events
          if e.get("method") == "Log.entryAdded"
          and "context lost" in e["params"]["entry"]["text"].lower()]
result["graphics_context_losses"] = losses
(args.out / "home-check.json").write_text(json.dumps(result, indent=2) + "\n")
assert args.allow_context_loss or not losses, losses
print(json.dumps(result))

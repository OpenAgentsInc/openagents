#!/usr/bin/env python3
"""Capture the Grid homepage's help desk (#12443) under `openagents browser run`.

Joins the Grid, captures the desk from the spawn, walks up to the counter and
captures the prompt, presses E, asks --question as a signed-out visitor, and
captures the answer. It records the visit's events as the page queued them
(names and details only; the page never sends what anyone typed) and checks
that the desk's events are among them. Use a local server only.
"""
import argparse, base64, importlib.util, json, math, os, time, urllib.request
from pathlib import Path
from urllib.parse import urlsplit

p = argparse.ArgumentParser()
p.add_argument("--url", required=True)
p.add_argument("--out", required=True, type=Path)
p.add_argument("--question", default="What is OpenAgents?")
p.add_argument("--followup", default="How do I connect my phone to Coder on my Mac?")
p.add_argument("--width", type=int, default=1440)
p.add_argument("--height", type=int, default=900)
p.add_argument("--answer-wait", type=float, default=150.0)
a = p.parse_args()
assert urlsplit(a.url).hostname in ("localhost", "127.0.0.1", "::1"), "Use a local server."
a.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
port = os.environ["OPENAGENTS_CHROME_PORT"]
t = json.load(urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
cdp = m.Cdp(t["webSocketDebuggerUrl"])
cdp.call("Page.enable"); cdp.call("Runtime.enable")
# The player's position each frame, and the visit's events as queued.
cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
let camera;Object.defineProperty(globalThis,"pylonsCamera",{configurable:true,get(){return camera},
set(draw){camera=(m,w,h,p)=>{draw(m,w,h,p);globalThis.__pylonsFrame={player:Array.from(p)};}}});
globalThis.__visit=[];const beacon=navigator.sendBeacon.bind(navigator);
navigator.sendBeacon=(url,body)=>{try{if(String(url).endsWith("/a/v"))globalThis.__visit.push(...JSON.parse(body).e)}catch(e){}return beacon(url,body)};"""})
cdp.call("Emulation.setDeviceMetricsOverride", {"width": a.width, "height": a.height, "deviceScaleFactor": 1, "mobile": False})


def ev(x):
    r = cdp.eval(x)
    if "exceptionDetails" in r:
        raise RuntimeError(r["exceptionDetails"])
    return r["result"].get("value")


def shot(name):
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (a.out / f"{name}.png").write_bytes(base64.b64decode(png))


def key(code, down, k=None):
    cdp.call("Input.dispatchKeyEvent", {"type": "keyDown" if down else "keyUp", "code": code, "key": k or code[-1].lower()})


def wait(expr, seconds, what):
    dl = time.monotonic() + seconds
    while not ev(expr):
        assert time.monotonic() < dl, what
        time.sleep(.25)


VERSE = 'import("/everglade/everglade_web.js")'
cdp.call("Page.navigate", {"url": a.url})
wait('performance.getEntriesByName("verse-visible").length > 0', 120, "no verse")
time.sleep(3)
info = {"marks": ev('performance.getEntriesByType("mark").map(x=>[x.name,Math.round(x.startTime)])')}
key("KeyW", True); key("KeyW", False)
wait('performance.getEntriesByName("home-third-person").length > 0', 30, "no third person")
time.sleep(3)
shot("spawn")
info["spawn"] = ev("globalThis.__pylonsFrame && globalThis.__pylonsFrame.player")

# The desk's place, from crates/grid-help-desk: the counter at (-6, -5)
# facing (0, -7); the visitor asks 1.3 m in front of it.
desk = (-6.0, -5.0)
yaw = math.atan2(0.0 - desk[0], -7.0 - desk[1])
stand = (desk[0] + 1.3 * math.sin(yaw), desk[1] + 1.3 * math.cos(yaw))


def local(x, z):
    """Desk coordinates (x across, z out of the front) on the floor."""
    s_, c_ = math.sin(yaw), math.cos(yaw)
    return (desk[0] + x * c_ + z * s_, desk[1] - x * s_ + z * c_)


def walk_to(point, stop=0.5):
    for _ in range(3):
        ev(f'{VERSE}.then(m=>m.verse_face({point[0]},{point[1]}))')
        time.sleep(.2)
        key("KeyW", True)
        dl = time.monotonic() + 20
        while time.monotonic() < dl:
            pp = ev("globalThis.__pylonsFrame.player")
            if math.hypot(pp[0] - point[0], pp[2] - point[1]) < stop:
                break
            time.sleep(.03)
        key("KeyW", False)
        time.sleep(.3)


# A close look from the walk in, outside the desk's reach.
walk_to(local(-2.2, 4.6))
ev(f'{VERSE}.then(m=>m.verse_face({desk[0]},{desk[1]}))')
time.sleep(2.5)
shot("approach")
# Zoom in for the devices, then back out.
key("Equal", True, "="); time.sleep(float(os.environ.get("DESK_ZOOM", "1.4"))); key("Equal", False, "=")
time.sleep(1.0)
shot("closeup")
key("Minus", True, "-"); time.sleep(1.4); key("Minus", False, "-")
time.sleep(.5)
info["closeup_prompt"] = ev("!!globalThis.openagentsChatDesk")
for _ in range(3):
    ev(f'{VERSE}.then(m=>m.verse_face({stand[0]},{stand[1]}))')
    time.sleep(.2)
    key("KeyW", True)
    dl = time.monotonic() + 20
    while time.monotonic() < dl:
        pp = ev("globalThis.__pylonsFrame.player")
        if math.hypot(pp[0] - stand[0], pp[2] - stand[1]) < 0.5:
            break
        time.sleep(.03)
    key("KeyW", False)
    time.sleep(.3)
ev(f'{VERSE}.then(m=>m.verse_face({desk[0]},{desk[1]}))')
time.sleep(2.5)
info["at_desk"] = ev("globalThis.__pylonsFrame.player")
shot("prompt")

key("KeyE", True, "e"); key("KeyE", False, "e")
wait('document.getElementById("pylon-composer").classList.contains("open")', 60, "the chat didn't open")
wait('document.getElementById("pylon-composer").dataset.chatState === "ready"', 60, "the chat isn't ready")
time.sleep(1.5)
info["opened"] = ev("({desk: globalThis.openagentsChatDesk, label: document.getElementById('pylon-composer').getAttribute('aria-label'), focus: document.activeElement && document.activeElement.id})")
shot("open")
# Typed as keys, as a person types: the chat's field reads key events.
for char in a.question:
    code = "Space" if char == " " else ("Key" + char.upper() if char.isalpha() else "Unidentified")
    cdp.call("Input.dispatchKeyEvent", {"type": "keyDown", "code": code, "key": char, "text": char})
    cdp.call("Input.dispatchKeyEvent", {"type": "keyUp", "code": code, "key": char})
time.sleep(.5)
info["draft_chars"] = ev('document.getElementById("pylon-composer").dataset.draftChars')
shot("typed")
cdp.call("Input.dispatchKeyEvent", {"type": "keyDown", "code": "Enter", "key": "Enter"})
cdp.call("Input.dispatchKeyEvent", {"type": "keyUp", "code": "Enter", "key": "Enter"})
started = time.monotonic()
STATE = 'document.getElementById("pylon-composer").dataset'
dl = time.monotonic() + 30
while not ev(f'["sending","streaming","notice"].includes({STATE}.chatState) || +{STATE}.messageCount > 0'):
    if time.monotonic() > dl:
        info["stuck"] = ev(f"({{state: {STATE}.chatState, draft: {STATE}.draftChars, focus: document.activeElement && document.activeElement.id}})")
        shot("stuck")
        raise SystemExit(json.dumps(info["stuck"]))
    time.sleep(.25)
wait('document.getElementById("pylon-composer").dataset.chatState !== "sending" && document.getElementById("pylon-composer").dataset.chatState !== "streaming"', a.answer_wait, "no answer")
info["answer_seconds"] = round(time.monotonic() - started, 1)
time.sleep(2)
info["chat"] = ev("({state: document.getElementById('pylon-composer').dataset.chatState, messages: document.getElementById('pylon-composer').dataset.messageCount, pick: document.getElementById('pylon-composer').dataset.pylonPick})")
shot("answer")
OPEN = "document.getElementById('pylon-composer').classList.contains('open')"
# A question the answer bank doesn't hold goes to the model.
if a.followup:
    for char in a.followup:
        code = "Space" if char == " " else ("Key" + char.upper() if char.isalpha() else "Unidentified")
        cdp.call("Input.dispatchKeyEvent", {"type": "keyDown", "code": code, "key": char, "text": char})
        cdp.call("Input.dispatchKeyEvent", {"type": "keyUp", "code": code, "key": char})
    time.sleep(.3)
    cdp.call("Input.dispatchKeyEvent", {"type": "keyDown", "code": "Enter", "key": "Enter"})
    cdp.call("Input.dispatchKeyEvent", {"type": "keyUp", "code": "Enter", "key": "Enter"})
    started = time.monotonic()
    wait(f'+{STATE}.messageCount >= 3', 30, "the follow-up wasn't sent")
    wait(f'!["sending","streaming"].includes({STATE}.chatState) && +{STATE}.messageCount >= 4', a.answer_wait, "no follow-up answer")
    info["followup_seconds"] = round(time.monotonic() - started, 1)
    time.sleep(2)
    shot("followup")
# Escape closes the chat and the prompt comes back; E opens it again.
key("Escape", True, "Escape"); key("Escape", False, "Escape")
time.sleep(1.5)
info["after_escape"] = ev(f"({{open: {OPEN}, desk: !!globalThis.openagentsChatDesk}})")
assert not info["after_escape"]["open"], "Escape left the chat open"
shot("closed")
key("KeyE", True, "e"); key("KeyE", False, "e")
wait(OPEN, 20, "E didn't open the chat again")
time.sleep(1)
# A click on the world hands the keys back; walking away closes the chat.
cdp.call("Input.dispatchMouseEvent", {"type": "mousePressed", "x": 160, "y": 90, "button": "left", "clickCount": 1})
cdp.call("Input.dispatchMouseEvent", {"type": "mouseReleased", "x": 160, "y": 90, "button": "left", "clickCount": 1})
time.sleep(.5)
walk_to(local(0.0, 8.0))
time.sleep(1.5)
info["after_leaving"] = ev(f"({{open: {OPEN}, desk: !!globalThis.openagentsChatDesk}})")
assert not info["after_leaving"]["open"] and not info["after_leaving"]["desk"], "walking away left the chat open"
ev('dispatchEvent(new Event("pagehide"))')
time.sleep(.5)
info["visit"] = ev("globalThis.__visit")
names = [e[0] for e in info["visit"]]
for need in ("help_desk_reached", "help_desk_opened", "help_desk_message"):
    assert need in names, f"{need} missing from the visit"
assert "terminal_opened" not in names and "message_sent" not in names, "the desk counted as the terminal"
assert a.question not in json.dumps(info["visit"]), "the visit carries the question"
(a.out / "capture.json").write_text(json.dumps(info, indent=1))
print(json.dumps({k: v for k, v in info.items() if k != "marks"}, indent=1)[:3000])

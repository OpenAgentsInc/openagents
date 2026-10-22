#!/usr/bin/env python3
"""Capture the Grid homepage with no HTML over the world (#12459).

Run under `openagents browser run` against a local server only, for example
the synthetic-account preview in
`crates/openagents-web/src/game_lab/hosted/tests.rs` (`browser_preview`):

    openagents browser run -- python3 scripts/dev/check-grid-home-overlays.py \
        --url http://127.0.0.1:PORT/ --out DIR [--phone] [--signed-in]

It joins the Grid and checks that, once Verse and the GPUI chat are up,
nothing visible over the world is HTML: the header is clipped away, the
machines' HTML labels are hidden, and the chat's canvas is clipped to what
it draws. It captures the world, a pylon selected with its GPUI card, the
prompt under the GAME LAB arch, Game Lab's GPUI panel after E (signed out:
the sign-in line; signed in: the game, then an edit through the chat, with
the transcript floating over the world and New chat in the composer's row),
and a game shown full screen. With --signed-in it sets the preview's
synthetic session cookie; never use a real account.
"""
import argparse
import base64
import importlib.util
import json
import math
import os
import time
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit

p = argparse.ArgumentParser()
p.add_argument("--url", required=True)
p.add_argument("--out", required=True, type=Path)
p.add_argument("--phone", action="store_true", help="390 x 844, touch, as a phone.")
p.add_argument("--signed-in", action="store_true", help="The preview's synthetic account.")
p.add_argument("--edit", default="Make the man slower")
p.add_argument("--answer-wait", type=float, default=120.0)
a = p.parse_args()
host = urlsplit(a.url).hostname
assert host in ("localhost", "127.0.0.1", "::1"), "Use a local server."
a.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py"
)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
port = os.environ["OPENAGENTS_CHROME_PORT"]
t = json.load(urllib.request.urlopen(urllib.request.Request(
    f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
cdp = m.Cdp(t["webSocketDebuggerUrl"])
cdp.call("Page.enable")
cdp.call("Runtime.enable")
cdp.call("Network.enable")
# The player's position each frame, what the chat last told the page about
# its overlay, and the visit's events as queued.
cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
let camera;Object.defineProperty(globalThis,"pylonsCamera",{configurable:true,get(){return camera},
set(draw){camera=(m,w,h,p)=>{draw(m,w,h,p);globalThis.__pylonsFrame={player:Array.from(p)};}}});
let overlay;Object.defineProperty(globalThis,"homeOverlay",{configurable:true,get(){return overlay},
set(f){overlay=json=>{globalThis.__overlay=JSON.parse(json);f(json)}}});
globalThis.__visit=[];const beacon=navigator.sendBeacon.bind(navigator);
navigator.sendBeacon=(url,body)=>{try{if(String(url).endsWith("/a/v"))globalThis.__visit.push(...JSON.parse(body).e)}catch(e){}return beacon(url,body)};"""})
W, H = (390, 844) if a.phone else (1440, 900)
cdp.call("Emulation.setDeviceMetricsOverride", {
    "width": W, "height": H, "deviceScaleFactor": 2 if a.phone else 1, "mobile": a.phone,
})
if a.phone:
    cdp.call("Emulation.setTouchEmulationEnabled", {"enabled": True, "maxTouchPoints": 5})
if a.signed_in:
    cdp.call("Network.setCookie", {
        "name": "oa_cloud_session", "value": "sess_" + "a" * 64,
        "url": f"http://{urlsplit(a.url).netloc}/",
    })
info = {"width": W, "height": H, "signed_in": a.signed_in}


def ev(x):
    r = cdp.eval(x)
    if "exceptionDetails" in r:
        raise RuntimeError(r["exceptionDetails"])
    return r["result"].get("value")


def shot(name):
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (a.out / f"{name}.png").write_bytes(base64.b64decode(png))


def key(code, down, k=None):
    cdp.call("Input.dispatchKeyEvent", {
        "type": "keyDown" if down else "keyUp", "code": code, "key": k or code[-1].lower()})


def click(x, y):
    for kind in ("mousePressed", "mouseReleased"):
        cdp.call("Input.dispatchMouseEvent", {
            "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1})


def type_text(text):
    for char in text:
        code = "Space" if char == " " else ("Key" + char.upper() if char.isalpha() else "Unidentified")
        cdp.call("Input.dispatchKeyEvent", {"type": "keyDown", "code": code, "key": char, "text": char})
        cdp.call("Input.dispatchKeyEvent", {"type": "keyUp", "code": code, "key": char})
    time.sleep(.3)
    cdp.call("Input.dispatchKeyEvent", {"type": "keyDown", "code": "Enter", "key": "Enter"})
    cdp.call("Input.dispatchKeyEvent", {"type": "keyUp", "code": "Enter", "key": "Enter"})


def wait(expr, seconds, what):
    dl = time.monotonic() + seconds
    while not ev(expr):
        if time.monotonic() > dl:
            shot("stuck")
            raise SystemExit(what)
        time.sleep(.25)


VERSE = 'import("/everglade/everglade_web.js")'


def walk_to(point, stop=0.5):
    for _ in range(3):
        ev(f'{VERSE}.then(m=>m.verse_face({point[0]},{point[1]}))')
        time.sleep(.2)
        key("KeyW", True)
        dl = time.monotonic() + 25
        while time.monotonic() < dl:
            # Passing the terminal opens its chat, which takes the keys;
            # hand them back to the world, as a click on it does.
            if ev("document.activeElement && document.activeElement.id === 'pylon-message'"):
                key("KeyW", False)
                ev("document.getElementById('everglade-canvas').focus()")
                key("KeyW", True)
            pp = ev("globalThis.__pylonsFrame.player")
            if math.hypot(pp[0] - point[0], pp[2] - point[1]) < stop:
                break
            time.sleep(.03)
        key("KeyW", False)
        time.sleep(.3)


def face(point):
    ev(f'{VERSE}.then(m=>m.verse_face({point[0]},{point[1]}))')
    time.sleep(1.5)


# What a person sees that is HTML: every element with a painted box or
# visible text over the world that isn't a canvas, hidden, clipped away, or
# a page the chat frames.
HTML_SEEN = """(()=>{const seen=[];
for(const el of document.body.querySelectorAll('*')){
  if(['CANVAS','SCRIPT','STYLE','INPUT','TEXTAREA'].includes(el.tagName))continue;
  const r=el.getBoundingClientRect();if(r.width<2||r.height<2)continue;
  if(r.right<=0||r.bottom<=0||r.left>=innerWidth||r.top>=innerHeight)continue;
  let hidden=false;
  for(let e=el;e&&e!==document.documentElement;e=e.parentElement){const s=getComputedStyle(e);
    if(s.display==='none'||s.visibility==='hidden'||+s.opacity===0||s.clipPath==='inset(50%)'||(s.clip&&s.clip!=='auto'&&s.position==='absolute'&&r.width<=1)){hidden=true;break}
    if(e.classList.contains('sr')){hidden=true;break}
    const b=e.getBoundingClientRect();if((b.width<=1||b.height<=1)&&s.overflow==='hidden'){hidden=true;break}}
  if(hidden)continue;
  const s=getComputedStyle(el);
  // A page the chat frames (a game, Kiln) is content, not chrome.
  if(el.closest('.home-frame'))continue;
  const paints=(s.backgroundColor!=='rgba(0, 0, 0, 0)'&&!el.closest('#pylon-composer'))||s.borderTopWidth!=='0px'&&s.borderTopStyle!=='none'
    ||(s.color!=='rgba(0, 0, 0, 0)'&&[...el.childNodes].some(n=>n.nodeType===3&&n.textContent.trim()));
  if(paints)seen.push({tag:el.tagName,id:el.id,cls:String(el.className),text:(el.textContent||'').trim().slice(0,40)});
}
return seen})()"""

cdp.call("Page.navigate", {"url": a.url})
wait('performance.getEntriesByName("verse-visible").length > 0', 180, "no verse")
key("KeyW", True)
key("KeyW", False)
wait('document.documentElement.classList.contains("gpui-ready")', 120, "the GPUI chat didn't draw")
time.sleep(4)
info["marks"] = ev('performance.getEntriesByType("mark").map(x=>[x.name,Math.round(x.startTime)])')
info["html_seen"] = ev(HTML_SEEN)
info["overlay_at_spawn"] = ev("globalThis.__overlay")
shot("world")

# A pylon: walk toward the Bay, point at a machine, and open its card.
bay = (17.0 + 6.0, -2.0 + 10.0)
walk_to((8.0, 3.0))
face(bay)
info["pylons"] = ev("document.querySelectorAll('#pylons-field .pylon[data-world-anchor]').length")
shot("pylon-bay")
target = ev("""(()=>{const p=[...document.querySelectorAll('#pylons-field .pylon:not(.hid):not(.group-part)')]
  .map(e=>[e,e.getBoundingClientRect()]).filter(([e,r])=>r.width>4&&r.top>60&&r.bottom<innerHeight-90&&r.left>10&&r.right<innerWidth-10);
  if(!p.length)return null;const [e,r]=p[0];return {id:e.dataset.pylon,x:r.left+r.width/2,y:r.top+r.height*0.6}})()""")
info["pylon_target"] = target
if target:
    cdp.call("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": target["x"], "y": target["y"]})
    time.sleep(1)
    click(target["x"], target["y"])
    wait('document.querySelector("#pylon-card.open.ready") !== null', 60, "the pylon card didn't open")
    time.sleep(1.5)
    info["html_seen_pylon"] = ev(HTML_SEEN)
    shot("pylon-selected")
    key("Escape", True, "Escape")
    key("Escape", False, "Escape")
    time.sleep(1)

# Game Lab: under its arch, the prompt; E opens the panel.
arch = (-8.0, 3.0)
walk_to((arch[0], arch[1] - 2.0))
face(arch)
info["game_prompt_near"] = ev("!globalThis.openagentsGameLabOpen")
shot("game-lab-prompt")
key("KeyE", True, "e")
key("KeyE", False, "e")
wait("globalThis.openagentsGameLabOpen === true", 20, "E didn't open Game Lab")
if a.signed_in:
    wait("globalThis.openagentsLab === 'game' && !!(globalThis.__overlay||{}).frames && globalThis.__overlay.frames.length > 0", 60, "Game Lab's card didn't show")
else:
    wait("!document.getElementById('game-lab-signin').hidden", 60, "Game Lab didn't offer sign-in")
time.sleep(3)
info["game_status"] = ev("document.getElementById('game-lab-status').textContent")
info["html_seen_game"] = ev(HTML_SEEN)
info["overlay_game"] = ev("globalThis.__overlay")
shot("game-lab-panel")
if a.signed_in:
    STATE = 'document.getElementById("pylon-composer").dataset'
    wait('document.getElementById("pylon-composer").classList.contains("open")', 30, "Game Lab's chat didn't open")
    type_text(a.edit)
    wait(f'+{STATE}.messageCount > 0', 30, "the edit wasn't sent")
    wait(f'!["sending","streaming"].includes({STATE}.chatState) && +{STATE}.messageCount >= 2', a.answer_wait, "no answer")
    time.sleep(6)
    info["overlay_edited"] = ev("globalThis.__overlay")
    shot("game-lab-edited")
    # Play, under the preview: the button row starts 10 px below it.
    frame = info["overlay_edited"]["frames"][0]
    click(frame["x"] + 30, frame["y"] + frame["h"] + 10 + 16)
    wait("globalThis.openagentsGamePlaying === true", 20, "Play didn't show the game")
    time.sleep(6)
    info["overlay_play"] = ev("globalThis.__overlay")
    shot("game-lab-play")
    back = info["overlay_play"]["regions"][0]
    click(back[0] + back[2] / 2, back[1] + back[3] / 2)
    wait("globalThis.openagentsGamePlaying === false", 20, "Back to Game Lab didn't")
    time.sleep(2)
key("Escape", True, "Escape")
key("Escape", False, "Escape")
wait("globalThis.openagentsGameLabOpen === false", 20, "Escape didn't close Game Lab")
time.sleep(1.5)
ev('dispatchEvent(new Event("pagehide"))')
time.sleep(.5)
info["visit"] = ev("globalThis.__visit")
names = [e[0] for e in info["visit"]]
for need in ["game_lab_opened", "game_lab_closed"] + (["game_lab_played"] if a.signed_in else []):
    assert need in names, f"{need} missing from the visit"
(a.out / "capture.json").write_text(json.dumps(info, indent=2))
print(json.dumps({k: info[k] for k in ("html_seen", "pylons", "game_status")}, indent=2))

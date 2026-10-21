#!/usr/bin/env python3
"""Capture the Grid homepage's Pylon Bay (#12423) under `openagents browser run`.

Takes the overview, the joined view, and, with --walk, a view after walking
to a floor point (and --look turning to face another). --hover names the end
of a pylon's ID to hover and click, which captures its label and card and
records where focus went. --simulate-busy rewrites the Bay handed to Verse so
idle live data shows the busy look (heads answering, GPUs full, pulses); use
it only for captures, never as evidence of real load.
"""
import argparse, base64, importlib.util, json, os, time, urllib.request
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument("--url", required=True)
p.add_argument("--out", required=True, type=Path)
p.add_argument("--prefix", default="")
p.add_argument("--walk", default="", help="ground point x,z to walk to after joining")
p.add_argument("--look", default="")
p.add_argument("--stop", type=float, default=7.0)
p.add_argument("--hover", default="", help="pylon id to hover at the end")
p.add_argument("--simulate-busy", action="store_true")
p.add_argument("--width", type=int, default=1440)
p.add_argument("--height", type=int, default=900)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py")
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
port = os.environ["OPENAGENTS_CHROME_PORT"]
t = json.load(urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
cdp = m.Cdp(t["webSocketDebuggerUrl"])
cdp.call("Page.enable"); cdp.call("Runtime.enable")
cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
let camera;Object.defineProperty(globalThis,"pylonsCamera",{configurable:true,get(){return camera},
set(draw){camera=(m,w,h,p)=>{draw(m,w,h,p);globalThis.__pylonsFrame={player:Array.from(p)};}}});"""})
if a.simulate_busy:
    # Check only: show the busy look on idle live data (heads answering,
    # GPUs full, a receipt pulse on each ordinary pylon).
    cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
let raw;Object.defineProperty(globalThis,'pylonsBay',{configurable:true,
get(){if(!raw)return raw;const a=Float64Array.from(raw);const n=a[1],m=a[2];const now=performance.now();
for(let i=0;i<n;i++){const o=3+i*6;const shape=a[o+2];
if(shape===1)a[o+3]=2;if(shape===4||shape===5||shape===0)a[o+3]=1;
if(shape<4)a[o+4]=now-((now+i*500)%2400);}
for(let j=0;j<m;j++)a[3+n*6+j*4+3]=1;return a;},set(v){raw=v}});"""})
cdp.call("Emulation.setDeviceMetricsOverride", {"width": a.width, "height": a.height, "deviceScaleFactor": 1, "mobile": False})
def ev(x):
    r = cdp.eval(x)
    if "exceptionDetails" in r: raise RuntimeError(r["exceptionDetails"])
    return r["result"].get("value")
def shot(name):
    png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
    (a.out / f"{a.prefix}{name}.png").write_bytes(base64.b64decode(png))
def key(code, down):
    cdp.call("Input.dispatchKeyEvent", {"type": "keyDown" if down else "keyUp", "code": code, "key": code[-1].lower()})
cdp.call("Page.navigate", {"url": a.url})
dl = time.monotonic() + 120
while not ev('performance.getEntriesByName("verse-visible").length > 0'):
    assert time.monotonic() < dl, "no verse"
    time.sleep(.25)
dl = time.monotonic() + 60
while ev('document.querySelectorAll("#pylons-list li").length') < 3 and time.monotonic() < dl:
    time.sleep(.5)
time.sleep(4)
info = {"marks": ev('performance.getEntriesByType("mark").map(x=>[x.name,Math.round(x.startTime)])'),
        "pylons": ev('Array.from(document.querySelectorAll("#pylons-list li")).map(l=>l.textContent)'),
        "where": ev('import("/everglade/everglade_web.js").then(m=>JSON.parse(m.verse_where()))')}
shot("overview")
time.sleep(6); shot("overview-later")
key("KeyW", True); key("KeyW", False)
dl = time.monotonic() + 30
while not ev('performance.getEntriesByName("home-third-person").length > 0') and time.monotonic() < dl:
    time.sleep(.2)
time.sleep(2.5)
shot("joined")
info["spawn"] = ev("globalThis.__pylonsFrame && globalThis.__pylonsFrame.player")
if a.walk:
    x, z = map(float, a.walk.split(","))
    for step in range(3):
        ev(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({x},{z}))')
        time.sleep(.2)
        key("KeyW", True)
        dl = time.monotonic() + 20
        while time.monotonic() < dl:
            pp = ev("globalThis.__pylonsFrame.player")
            if ((pp[0]-x)**2 + (pp[2]-z)**2) ** .5 < a.stop: break
            time.sleep(.05)
        key("KeyW", False)
    if a.look:
        lx, lz = map(float, a.look.split(","))
        ev(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({lx},{lz}))')
        time.sleep(2.5)
    time.sleep(1.5)
    info["walked"] = ev("globalThis.__pylonsFrame.player")
    shot("near")
info["tags"] = ev("""Array.from(document.querySelectorAll('#pylons-field .pylon')).map(p=>({id:p.dataset.pylon,cls:p.className,tag:p.querySelector('.tag')?.innerText}))""")
if a.hover:
    r = ev(f"""(()=>{{const p=document.querySelector('#pylons-field .pylon[data-pylon$="{a.hover}"]');if(!p)return null;const b=p.getBoundingClientRect();return [b.x+b.width/2,b.y+b.height/2]}})()""")
    if r:
        cdp.call("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": r[0], "y": r[1]})
        time.sleep(1.0)
        shot("hover")
        cdp.call("Input.dispatchMouseEvent", {"type": "mousePressed", "x": r[0], "y": r[1], "button": "left", "clickCount": 1})
        cdp.call("Input.dispatchMouseEvent", {"type": "mouseReleased", "x": r[0], "y": r[1], "button": "left", "clickCount": 1})
        time.sleep(2.0)
        shot("card")
        info["after_click"] = ev("({active: document.activeElement && (document.activeElement.id || document.activeElement.tagName), card: document.getElementById('pylon-card').className})")
(a.out / f"{a.prefix}capture.json").write_text(json.dumps(info, indent=1))
print(json.dumps(info, indent=1)[:2000])

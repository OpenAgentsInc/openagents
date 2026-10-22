#!/usr/bin/env python3
"""Capture a model group on the Grid homepage's Pylon Bay (#12403) under
`openagents browser run`.

Joins the Grid, walks to the group's lane, faces it, and takes one capture
for each `--shot`:

  idle       the group as the live data shows it
  live       the same, meant to be taken while a real request runs
  prefill    simulated: the head took a job just now (reading the prompt)
  decode     simulated: the head is answering
  selected   the pointer over the group's head
  down       simulated: the group left the Grid a moment ago
  mac        simulated: the last part runs on a Mac's GPU (Metal)

The simulated shots rewrite what the pylons layer hands Verse
(`pylonsGroups` and `pylonsBay`) in this page only: they show the look of a
phase on the live group, never evidence of real load. `--ask CMD` runs a
command (such as `pylon ask ...`) in the background before the `live` shot,
and `--live-after S` waits that long before taking it.
"""
import argparse, base64, importlib.util, json, os, shlex, subprocess, time, urllib.request
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("--url", required=True)
p.add_argument("--out", required=True, type=Path)
p.add_argument("--prefix", default="")
p.add_argument("--walk", default="20,3.5", help="ground point x,z to walk to")
p.add_argument("--look", default="28,3.5", help="ground point x,z to face")
p.add_argument("--stop", type=float, default=1.5)
p.add_argument("--shot", action="append", default=[])
p.add_argument("--ask", default="", help="a command to run in the background before the live shot")
p.add_argument("--live-after", type=float, default=4.0)
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
set(draw){camera=(m,w,h,p)=>{draw(m,w,h,p);globalThis.__pylonsFrame={player:Array.from(p)};}}});
// Capture only: globalThis.__sim = {phase, since} rewrites the handoff.
let groups;Object.defineProperty(globalThis,"pylonsGroups",{configurable:true,
get(){const s=globalThis.__sim;if(!s||!groups)return groups;const v=JSON.parse(groups);
for(const g of v.groups){
 if(s.phase==="prefill"||s.phase==="decode"){g.phase=s.phase;g.busy_since=s.since;}
 if(s.phase==="down"){g.phase="down";g.busy_since=-1;g.gone=true;}
 if(s.phase==="mac"&&g.stages.length){g.stages[g.stages.length-1].device="metal-m5-max";}
 globalThis.__simGroup=g;}
return JSON.stringify(v);},set(v){groups=v}});
let bay;Object.defineProperty(globalThis,"pylonsBay",{configurable:true,
get(){const s=globalThis.__sim,g=globalThis.__simGroup;if(!s||!bay||!g)return bay;const a=Float64Array.from(bay);
const n=a[1],np=a[2];const near=(x,z,p)=>Math.abs(x-p[0])<0.01&&Math.abs(z-p[2])<0.01;
const members=[g.at].concat(g.stages.map(st=>st.at));
const out=[];
for(let i=0;i<n;i++){const o=3+i*6;const at=[a[o],0,a[o+1]];const mine=members.some(p=>near(at[0],at[2],p));
 if(mine&&s.phase==="down")continue;
 if(mine&&(s.phase==="prefill"||s.phase==="decode"))a[o+3]=(a[o+2]===4)?2:1;
 if(s.phase==="mac"&&g.stages.length){const last=g.stages[g.stages.length-1].at;if(near(at[0],at[2],last))a[o+2]=0;}
 out.push(...a.slice(o,o+6));}
const pipes=Array.from(a.slice(3+n*6));
if(s.phase==="prefill"||s.phase==="decode")for(let j=0;j<np;j++)pipes[j*4+3]=1;
if(s.phase==="down")pipes.length=0;
return Float64Array.from([a[0],out.length/6,pipes.length/4,...out,...pipes]);},set(v){bay=v}});
"""})
cdp.call("Emulation.setDeviceMetricsOverride", {"width": a.width, "height": a.height, "deviceScaleFactor": 1, "mobile": False})


def ev(x):
    r = cdp.eval(x)
    if "exceptionDetails" in r:
        raise RuntimeError(r["exceptionDetails"])
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
while not ev('!!globalThis.pylonsGroups && JSON.parse(globalThis.pylonsGroups).groups.length > 0') and time.monotonic() < dl:
    time.sleep(.5)
info = {"groups": json.loads(ev("globalThis.pylonsGroups") or "{}"),
        "pylons": ev('Array.from(document.querySelectorAll("#pylons-list li")).map(l=>l.textContent)')}
key("KeyW", True); key("KeyW", False)
dl = time.monotonic() + 30
while not ev('performance.getEntriesByName("home-third-person").length > 0') and time.monotonic() < dl:
    time.sleep(.2)
time.sleep(2.0)
x, z = map(float, a.walk.split(","))
for _ in range(3):
    ev(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({x},{z}))')
    time.sleep(.2)
    key("KeyW", True)
    dl = time.monotonic() + 20
    while time.monotonic() < dl:
        pp = ev("globalThis.__pylonsFrame.player")
        if ((pp[0] - x) ** 2 + (pp[2] - z) ** 2) ** .5 < a.stop:
            break
        time.sleep(.05)
    key("KeyW", False)
lx, lz = map(float, a.look.split(","))
ev(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({lx},{lz}))')
time.sleep(2.5)
info["player"] = ev("globalThis.__pylonsFrame.player")
asked = None
for name in a.shot or ["idle"]:
    ev("globalThis.__sim = null")
    if name == "idle":
        time.sleep(1.0)
    elif name == "live":
        if a.ask:
            asked = subprocess.Popen(shlex.split(a.ask), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        time.sleep(a.live_after)
        info["live_group"] = json.loads(ev("globalThis.pylonsGroups") or "{}")
    elif name == "prefill":
        ev("globalThis.__sim = {phase: 'prefill', since: performance.now() - 300}")
        time.sleep(0.7)
    elif name == "decode":
        ev("globalThis.__sim = {phase: 'decode', since: performance.now() - 60000}")
        time.sleep(1.13)
    elif name in ("down", "mac"):
        ev(f"globalThis.__sim = {{phase: '{name}'}}")
        time.sleep(1.0)
    elif name == "selected":
        head = info["groups"]["groups"][0]["id"] if info["groups"].get("groups") else None
        r = ev("""(()=>{const p=[...document.querySelectorAll('#pylons-field .pylon.group-head')][0];
            if(!p)return null;const b=p.getBoundingClientRect();return [b.x+b.width/2,b.y+b.height/2]})()""")
        info["selected_at"] = r
        if r:
            cdp.call("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": r[0], "y": r[1]})
            time.sleep(1.2)
    shot(name)
    info.setdefault("shots", []).append(name)
if asked:
    try:
        out, _ = asked.communicate(timeout=600)
    except subprocess.TimeoutExpired:
        asked.kill()
        out, _ = asked.communicate()
    info["ask"] = out
(a.out / f"{a.prefix}capture.json").write_text(json.dumps(info, indent=1))
print(json.dumps({k: v for k, v in info.items() if k != "groups"}, indent=1)[:3000])

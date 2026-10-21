#!/usr/bin/env python3
"""Send one signed-out Grid terminal turn to Flint on a local site (#12422).

Run under `openagents browser run -- python3 scripts/dev/check-grid-flint.py
--out DIR --click-pick [--pin sha256:ARTIFACT]`. It joins the world, walks to
the shared terminal, clicks Flint beside the composer, sends one message,
waits for the reply, and records the request. The site's Pylon receipts name
the Pylon that answered. Signed out, so no account keeps the chat.
"""
import argparse, base64, importlib.util, json, os, time, urllib.request
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("--url", default="http://127.0.0.1:4341/home")
p.add_argument("--out", required=True, type=Path)
p.add_argument("--click-pick", action="store_true", help="Click the Flint segment (a bundle with #12425).")
p.add_argument("--pin", help="Pin one Flint artifact: rewrite the turn's model header to flint@ARTIFACT, the door's exact-artifact form, which the page's 64-character header cap can't carry.")
p.add_argument("--text", default="In two sentences explain why lighthouses use a rotating beam")
args = p.parse_args()
args.out.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("browser_cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py")
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
port = os.environ["OPENAGENTS_CHROME_PORT"]
target = json.load(urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
cdp = module.Cdp(target["webSocketDebuggerUrl"])
for m in ("Page.enable", "Runtime.enable", "Network.enable"):
    cdp.call(m)
cdp.call("Page.addScriptToEvaluateOnNewDocument", {"source": """
let camera;
Object.defineProperty(globalThis,"pylonsCamera",{configurable:true,get(){return camera},
  set(draw){camera=(m,w,h,p)=>{draw(m,w,h,p);globalThis.__pylonsFrame={player:Array.from(p)};}}});
"""})
cdp.call("Emulation.setDeviceMetricsOverride", {"width": 1440, "height": 900, "deviceScaleFactor": 1, "mobile": False})

def ev(e):
    r = cdp.eval(e)
    if "exceptionDetails" in r:
        raise RuntimeError(r["exceptionDetails"])
    return r["result"].get("value")

pinned = []

def service():
    """Answer paused requests, pinning the turn's model when asked."""
    for event in list(cdp.events):
        if event.get("method") != "Fetch.requestPaused" or event.get("handled"):
            continue
        event["handled"] = True
        request = event["params"]["request"]
        headers = [{"name": k, "value": v} for k, v in request["headers"].items()]
        if request["method"] == "POST" and any(h["name"].lower() == "x-openagents-pylon-model" for h in headers):
            for h in headers:
                if h["name"].lower() == "x-openagents-pylon-model":
                    pinned.append({"from": h["value"], "to": "flint@" + args.pin})
                    h["value"] = "flint@" + args.pin
        cdp.call("Fetch.continueRequest", {"requestId": event["params"]["requestId"], "headers": headers})

def wait_for(cond, label, timeout=120):
    end = time.monotonic() + timeout
    while not cond():
        if args.pin:
            service()
        if time.monotonic() > end:
            raise TimeoutError(label)
        time.sleep(.1)

def key(code, down, text=None):
    name = {"Enter": "Enter", "Space": " "}.get(code, code[-1].lower())
    e = {"type": "keyDown" if down else "keyUp", "code": code, "key": name}
    if text is not None:
        e["text"] = text
    cdp.call("Input.dispatchKeyEvent", e)

def shot(name):
    (args.out / name).write_bytes(base64.b64decode(cdp.call("Page.captureScreenshot", {"format": "png"})["data"]))

panel = 'document.querySelector("#pylon-composer")'
cdp.call("Page.navigate", {"url": args.url})
wait_for(lambda: ev('performance.getEntriesByName("verse-visible").length > 0'), "world visible")
wait_for(lambda: ev("globalThis.openagentsPylonFlintReady === true"), "a Flint pylon online")
key("KeyW", True); key("KeyW", False)
wait_for(lambda: ev('performance.getEntriesByName("home-third-person").length > 0'), "joined")
wait_for(lambda: ev('performance.getEntriesByName("home-chat-ready").length > 0'), "chat ready")
key("KeyW", True)
try:
    wait_for(lambda: ev('document.activeElement?.id === "pylon-message"'), "reached the terminal")
finally:
    key("KeyW", False)
time.sleep(.5)
wait_for(lambda: ev(panel + '.dataset.chatState === "ready"'), "chat connection")
shot("terminal.png")
# A bundle with the Flint pick (#12425) shows it beside the composer; click it.
# An older bundle lacks it but still sends the page's model global, which the
# pick sets, so set that instead and record which way the turn was routed.
pick_mode = "page-global"
if args.click_pick:
    for x in range(1050, 820, -8):
        for kind in ("mousePressed", "mouseReleased"):
            cdp.call("Input.dispatchMouseEvent", {"type": kind, "x": x, "y": 843, "button": "left",
                                                   "buttons": 1 if kind == "mousePressed" else 0, "clickCount": 1})
        time.sleep(.08)
        if ev(panel + '.dataset.pylonPick') == "flint":
            pick_mode = "clicked"
            break
for ch in args.text:
    code = "Space" if ch == " " else ("Key" + ch.upper() if ch.isalpha() else None)
    if code:
        key(code, True, ch); key(code, False)
    else:
        cdp.call("Input.insertText", {"text": ch})
wait_for(lambda: int(ev(panel + '.dataset.draftChars')) == len(args.text), "typed draft")
shot("flint-pick.png")
if pick_mode == "page-global":
    ev('globalThis.openagentsPylonModel = "flint"')
if args.pin:
    cdp.call("Fetch.enable", {"patterns": [{"urlPattern": "*/v1/threads*", "requestStage": "Request"}]})
assert ev("globalThis.openagentsPylonModel") == "flint"
sent = time.time()
key("Enter", True); key("Enter", False)
wait_for(lambda: int(ev(panel + '.dataset.messageCount')) >= 2 and ev(panel + '.dataset.chatState') == "ready", "Flint reply", timeout=900)
seconds = time.time() - sent
time.sleep(1)
shot("flint-reply.png")
posts = [e["params"]["request"] for e in cdp.events if e.get("method") == "Network.requestWillBeSent"
         and e["params"]["request"]["method"] == "POST" and "/v1/threads" in e["params"]["request"]["url"]]
headers = [{k.lower(): v for k, v in post["headers"].items()} for post in posts]
chat = ev('new URLSearchParams(location.search).get("chat")')
result = {"url": args.url, "chat": chat, "seconds": seconds, "pick_mode": pick_mode, "pinned": pinned, "pick": ev(panel + '.dataset.pylonPick'),
          "messages": int(ev(panel + '.dataset.messageCount')),
          "posts": [{"url": post["url"], "body": json.loads(post.get("postData") or "{}"),
                     "pylon_model": h.get("x-openagents-pylon-model"), "surface": h.get("x-openagents-chat-surface")}
                    for post, h in zip(posts, headers)]}
assert any(p["pylon_model"] == "flint" and p["surface"] == "home" for p in result["posts"]), result
assert not args.pin or pinned, "the pin was not applied"
assert not args.click_pick or pick_mode == "clicked", "the Flint segment was not found"
(args.out / "flint-turn.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))

"""Open the Water Lab in headless Chrome on WebGPU and with ?gl, and save a
screenshot and the console of each.

Usage: openagents browser run -- python3 -I webcheck.py SITE_DIR OUT_DIR CDP_DIR

SITE_DIR is what `scripts/build-everglade-web.sh --with-pack SITE_DIR` wrote;
CDP_DIR holds `bench/verse/2026-10-05/platform-clients/browser-cdp.py`
copied as `cdp.py`."""
import base64
import functools
import http.server
import json
import os
import sys
import threading
import time
import urllib.request

sys.path.insert(0, sys.argv[3])
from cdp import Cdp  # noqa: E402

site, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=site)
handler.log_message = lambda *a: None
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
origin = f"http://127.0.0.1:{server.server_address[1]}/"
port = os.environ["OPENAGENTS_CHROME_PORT"]
results = {}
for name, query in [("webgpu", "?zone=water&frames"), ("webgl2", "?gl&zone=water&frames")]:
    req = urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")
    target = json.load(urllib.request.urlopen(req))
    page = Cdp(target["webSocketDebuggerUrl"])
    page.call("Runtime.enable")
    page.call("Log.enable")
    page.call("Emulation.setDeviceMetricsOverride", {"width": 960, "height": 540, "deviceScaleFactor": 1, "mobile": False})
    page.call("Page.navigate", {"url": origin + query})
    time.sleep(40)
    probe = page.eval(
        "(async () => { const c = document.getElementById('everglade-canvas');"
        " const gl = c ? !!c.getContext('webgl2') : null;"
        " let adapter = null; if (navigator.gpu) { const a = await navigator.gpu.requestAdapter(); adapter = !!a; }"
        " const status = document.getElementById('status');"
        " return {canvas_has_webgl2: gl, navigator_gpu: !!navigator.gpu, webgpu_adapter: adapter,"
        " status: status ? status.textContent : null, title: document.title}; })()"
    )
    shot = page.call("Page.captureScreenshot", {"format": "png"})
    path = os.path.join(out, f"web-{name}.png")
    open(path, "wb").write(base64.b64decode(shot["data"]))
    logs = []
    for event in page.events:
        method = event.get("method")
        if method == "Runtime.consoleAPICalled":
            args = event["params"]["args"]
            logs.append({"level": event["params"]["type"], "text": " ".join(str(a.get("value", a.get("description", ""))) for a in args)[:400]})
        elif method == "Log.entryAdded":
            entry = event["params"]["entry"]
            logs.append({"level": entry["level"], "text": entry["text"][:400]})
    results[name] = {"url": query, "probe": probe.get("result", {}).get("value"), "console": logs[-40:]}
    page.call("Page.close")
json.dump(results, open(os.path.join(out, "web.json"), "w"), indent=2)
print(json.dumps({k: v["probe"] for k, v in results.items()}, indent=2))
for k, v in results.items():
    print(k, [l for l in v["console"] if l["level"] in ("error", "warning")][:8])
server.shutdown()

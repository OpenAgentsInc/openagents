#!/usr/bin/env python3
"""Cold first frame of the Verse Grid at a throttled link (#12319).

    first_frame.py BUILD_DIR [--mbit 20] [--runs 3]

Serves BUILD_DIR (the output of scripts/build-everglade-web.sh, with .br
and .gz copies) the way the site does: `/verse` is a page that loads the
module as /lander11 does, and `/everglade/*` answers with the brotli copy
when the request accepts it. Under `openagents browser run`, it drives the
browser over DevTools with the cache off and the network throttled, and
prints JSON: bytes over the wire, the times of the page's marks
(compiled, instantiated, the Grid's start steps, first frame) from
navigation, and the longest main-thread task while the module starts.
"""
import argparse, http.server, json, os, sys, threading, time

PAGE = """<!doctype html><html><head><meta charset=utf-8>
<style>html,body{margin:0;height:100%;background:#000}#everglade-canvas{display:block;width:100vw;height:100vh}</style>
</head><body><canvas id="everglade-canvas"></canvas><p id="everglade-status"></p>
<script type=module>
window.__long=[];
new PerformanceObserver(l=>{for(const e of l.getEntries())window.__long.push([e.startTime,e.duration])}).observe({type:"longtask",buffered:true});
const status=document.getElementById("everglade-status");
const drawn=new Promise(r=>new MutationObserver((list,o)=>{if(status.style.display==="none"){o.disconnect();r()}}).observe(status,{attributes:true,attributeFilter:["style"]}));
const [module,glue]=await Promise.all([WebAssembly.compileStreaming(fetch("/everglade/everglade_web_bg.wasm")),import("/everglade/everglade_web.js")]);
performance.mark("verse-compiled");
try{await glue.default({module_or_path:module})}catch(e){window.__err=String(e)}
performance.mark("verse-instantiated");
await drawn;
await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
performance.mark("verse-first-frame");
window.__done=true;
</script></body></html>"""

TYPES = {".wasm": "application/wasm", ".js": "text/javascript"}


def serve(root):
    class H(http.server.BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def do_GET(self):
            if self.path.split("?")[0] == "/verse":
                body, kind, coding = PAGE.encode(), "text/html", None
            elif self.path.startswith("/everglade/"):
                name = os.path.basename(self.path.split("?")[0])
                path = os.path.join(root, name)
                kind, coding = TYPES.get(os.path.splitext(name)[1], "application/octet-stream"), None
                accept = self.headers.get("Accept-Encoding", "")
                for ext, enc in ((".br", "br"), (".gz", "gzip")):
                    if enc in accept and os.path.exists(path + ext):
                        path, coding = path + ext, enc
                        break
                if not os.path.exists(path):
                    self.send_error(404)
                    return
                body = open(path, "rb").read()
            else:
                self.send_error(404)
                return
            self.send_response(200)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            if coding:
                self.send_header("Content-Encoding", coding)
            self.end_headers()
            self.wfile.write(body)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def run(port, mbit):
    import urllib.request
    import websocket
    target = json.load(urllib.request.urlopen(
        urllib.request.Request(f"http://127.0.0.1:{os.environ['OPENAGENTS_CHROME_PORT']}/json/new?about:blank",
                               method="PUT")))
    ws = websocket.create_connection(target["webSocketDebuggerUrl"], timeout=120, suppress_origin=True)
    seq = [0]
    wire = [0]

    def call(method, params=None):
        seq[0] += 1
        ws.send(json.dumps({"id": seq[0], "method": method, "params": params or {}}))
        while True:
            message = json.loads(ws.recv())
            if message.get("method") == "Network.loadingFinished":
                wire[0] += int(message["params"].get("encodedDataLength", 0))
            if message.get("id") == seq[0]:
                return message.get("result", {})

    call("Network.enable")
    call("Network.setCacheDisabled", {"cacheDisabled": True})
    call("Network.emulateNetworkConditions", {"offline": False, "latency": 40,
         "downloadThroughput": mbit * 1e6 / 8, "uploadThroughput": 5e6 / 8})
    call("Page.enable")
    call("Page.navigate", {"url": f"http://127.0.0.1:{port}/verse"})
    start = time.time()
    while time.time() - start < 90:
        done = call("Runtime.evaluate", {"expression": "window.__done===true", "returnByValue": True})
        if done.get("result", {}).get("value"):
            break
        time.sleep(0.25)
    result = call("Runtime.evaluate", {"returnByValue": True, "expression": """JSON.stringify({
        marks:Object.fromEntries(performance.getEntriesByType('mark').map(m=>[m.name,Math.round(m.startTime)])),
        tasks:window.__long.map(([s,d])=>[Math.round(s),Math.round(d)]),
        long:window.__long.filter(([s,d])=>s>=(performance.getEntriesByName('verse-compiled')[0]||{startTime:0}).startTime)
             .reduce((m,[s,d])=>Math.max(m,Math.round(d)),0),
        status:document.getElementById('everglade-status').textContent,done:window.__done===true,error:window.__err||null})"""})
    call("Network.disable")
    ws.close()
    out = json.loads(result["result"]["value"])
    out["wire_bytes"] = wire[0]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("build")
    ap.add_argument("--mbit", type=float, default=20)
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    server = serve(os.path.abspath(a.build))
    runs = [run(server.server_address[1], a.mbit) for _ in range(a.runs)]
    print(json.dumps({"build": a.build, "mbit": a.mbit, "runs": runs}, indent=1))


if __name__ == "__main__":
    sys.exit(main())

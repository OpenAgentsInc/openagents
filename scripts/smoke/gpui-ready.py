#!/usr/bin/env python3
"""Check signed-out GPUI startup and keep browser diagnostics, including on failure.

Run directly or through `openagents browser run`. Requires websocket-client.
This check only reads the page; it never signs in or sends a chat message.
"""

import argparse
import base64
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.parse
import urllib.request

import websocket


class Browser:
    def __init__(self, port):
        request = urllib.request.Request(
            f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")
        with urllib.request.urlopen(request, timeout=10) as response:
            self.page = json.load(response)
        self.connection = websocket.create_connection(
            self.page["webSocketDebuggerUrl"], suppress_origin=True, timeout=15)
        self.sequence = 0
        self.events = []
        self.requests = {}

    def call(self, method, params=None):
        self.sequence += 1
        self.connection.send(json.dumps({
            "id": self.sequence, "method": method, "params": params or {},
        }))
        while True:
            result = json.loads(self.connection.recv())
            event = result.get("method")
            values = result.get("params", {})
            # Keep startup errors and asset responses, without cookies,
            # authorization headers, or request bodies.
            if event == "Network.requestWillBeSent":
                self.requests[values["requestId"]] = values["request"]["url"]
            elif event == "Network.responseReceived":
                response = values["response"]
                headers = {name.lower(): value for name, value in response.get("headers", {}).items()}
                self.events.append({"event": event, **{
                    name: response.get(name) for name in ("url", "status", "mimeType")},
                    "headers": {name: headers[name] for name in (
                        "content-security-policy", "content-type", "content-encoding",
                        "cache-control", "x-content-type-options") if name in headers}})
            elif event in ("Runtime.exceptionThrown", "Runtime.consoleAPICalled",
                           "Log.entryAdded", "Network.loadingFailed"):
                self.events.append({"event": event, "details": values,
                                    "url": self.requests.get(values.get("requestId"))})
            if result.get("id") == self.sequence:
                if "error" in result:
                    raise RuntimeError(f"{method}: {result['error']}")
                return result.get("result", {})

    def state(self):
        result = self.call("Runtime.evaluate", {"returnByValue": True, "expression": """({
            url: location.href, gpui: document.documentElement.dataset.gpui,
            canvas: !!document.querySelector('canvas'), gpu: !!navigator.gpu,
            readyMs: window.__gpuiReadyMs,
            loaded: document.readyState === 'complete',
            shell: !!document.querySelector('#gpui-shell'),
            features: document.querySelector('meta[name="openagents-features"]')?.content,
            status: document.querySelector('#gpui-status')?.textContent,
            body: document.body?.innerText.slice(0, 500)
        })"""})
        return result.get("result", {}).get("value", {})


def ready(state):
    return state.get("gpui") == "ready" and state.get("canvas") is True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="https://staging.openagents.com")
    parser.add_argument("--timeout", type=float, default=90)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--if-enabled", action="store_true",
                        help="skip when the page explicitly has gpui_default turned off")
    args = parser.parse_args()
    if not os.environ.get("OPENAGENTS_CHROME_PORT"):
        return subprocess.run([
            "openagents", "browser", "run", "--", sys.executable,
            str(Path(__file__).resolve()), *sys.argv[1:],
        ], env={**os.environ, "BROWSER": "/usr/bin/true"}, check=False).returncode

    args.out.mkdir(parents=True, exist_ok=True)
    browser = Browser(os.environ["OPENAGENTS_CHROME_PORT"])
    state = {}
    failure = None
    skipped = False
    try:
        for domain in ("Page", "Runtime", "Network", "Log"):
            browser.call(domain + ".enable")
        browser.call("Network.setCacheDisabled", {"cacheDisabled": True})
        browser.call("Network.clearBrowserCookies")
        browser.call("Network.setExtraHTTPHeaders", {"headers": {"X-OpenAgents-Test": "1"}})
        browser.call("Emulation.setDeviceMetricsOverride", {
            "width": 1440, "height": 1000, "deviceScaleFactor": 1, "mobile": False,
        })
        # The new chat, at /chat since / became the Grid homepage (#12413).
        browser.call("Page.navigate", {"url": args.base.rstrip("/") + "/chat"})
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            state = browser.state()
            if ready(state):
                break
            if args.if_enabled and state.get("loaded") and state.get("features") is not None:
                if "gpui_default" not in state["features"].split(",") and not state.get("shell"):
                    skipped = True
                    break
            for event in browser.events:
                path = urllib.parse.urlsplit(event.get("url") or "").path
                if path.startswith(("/gpui/build/", "/gpui-fonts/")) and (
                    event.get("status", 0) >= 400 or event["event"] == "Network.loadingFailed"
                ):
                    raise RuntimeError(f"Startup asset failed: {event['url']} ({event.get('status', 'network error')})")
            if any(event["event"] == "Runtime.exceptionThrown" for event in browser.events):
                raise RuntimeError("The page raised an uncaught exception")
            time.sleep(0.25)
        if not skipped and not ready(state):
            raise RuntimeError("The signed-out page did not draw its GPUI canvas")
    except Exception as error:
        failure = str(error)
    finally:
        try:
            picture = browser.call("Page.captureScreenshot", {"format": "png"})
            (args.out / "page.png").write_bytes(base64.b64decode(picture["data"]))
        except Exception as error:
            browser.events.append({"event": "captureFailed", "details": str(error)})
        (args.out / "state.json").write_text(json.dumps(state, indent=2) + "\n")
        (args.out / "events.json").write_text(json.dumps(browser.events, indent=2) + "\n")
        browser.connection.close()
    print(json.dumps({"result": "FAIL" if failure else "SKIP" if skipped else "PASS",
                      "check": "signed-out GPUI startup", "state": state,
                      "error": failure, "diagnostics": str(args.out)}), flush=True)
    return 1 if failure else 0


if __name__ == "__main__":
    sys.exit(main())

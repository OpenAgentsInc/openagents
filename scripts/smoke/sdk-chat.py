#!/usr/bin/env python3
"""Continue one disposable cloud chat on desktop, iOS, and the web; remove it afterwards.

Run through openagents browser run with an explicit dedicated-account credential.
Native binaries must already be built. No owner account or computer is used.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import websocket

ROOT = Path(__file__).resolve().parents[2]

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None

class Browser:
    def __init__(self, website, token, chat):
        port = os.environ["OPENAGENTS_CHROME_PORT"]
        request = urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")
        with urllib.request.urlopen(request, timeout=10) as response:
            page = json.load(response)
        self.connection = websocket.create_connection(page["webSocketDebuggerUrl"], suppress_origin=True, timeout=10)
        self.sequence = 0
        self.call("Network.enable", {})
        assert self.call("Network.setCookie", {"name": "oa_cloud_session", "value": token, "url": website + "/", "secure": True, "httpOnly": True})["success"]
        self.call("Emulation.setDeviceMetricsOverride", {"width": 1120, "height": 900, "deviceScaleFactor": 1, "mobile": False})
        self.call("Page.navigate", {"url": website + "/chat/" + chat})
        self.wait("document.querySelector('#chat-form') !== null")
        self.evaluate("window.__sdkChatProbe = 'one-page'; true")

    def call(self, method, params):
        self.sequence += 1
        self.connection.send(json.dumps({"id": self.sequence, "method": method, "params": params}))
        while True:
            response = json.loads(self.connection.recv())
            if response.get("id") == self.sequence:
                assert "error" not in response, "Browser command failed"
                return response["result"]

    def evaluate(self, expression):
        result = self.call("Runtime.evaluate", {"expression": expression, "returnByValue": True})
        assert "exceptionDetails" not in result, "Browser expression failed"
        return result.get("result", {}).get("value")

    def wait(self, expression, timeout=100):
        deadline = time.monotonic() + timeout
        while not self.evaluate(expression):
            assert time.monotonic() < deadline, "Browser did not receive the chat update"
            time.sleep(0.2)

    def reply(self, text):
        self.evaluate("""(() => {
            const form = document.querySelector('#chat-form');
            const input = form.querySelector('textarea');
            input.value = %s;
            input.dispatchEvent(new Event('input', {bubbles: true}));
            form.requestSubmit(); return true;
        })()""" % json.dumps(text))

    def sees(self, index, word):
        self.wait("document.querySelector('#chat-message-%d')?.textContent.includes(%s) === true" % (index, json.dumps(word)))
        assert self.evaluate("window.__sdkChatProbe === 'one-page'"), "The web page reloaded"

    def capture(self, path):
        result = self.call("Page.captureScreenshot", {"format": "png"})
        path.write_bytes(base64.b64decode(result["data"]))

    def close(self):
        self.connection.close()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dedicated-account", action="store_true", required=True)
    parser.add_argument("--api", default="https://api.openagents.com")
    parser.add_argument("--website", default="https://openagents.com")
    parser.add_argument("--desktop-binary", type=Path, required=True)
    parser.add_argument("--ios-target-dir", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    args = parser.parse_args()
    token = os.environ["OPENAGENTS_TOKEN"]
    expected_account = os.environ["OPENAGENTS_TEST_ACCOUNT"]
    assert token.startswith("oa_sk_")
    assert args.api.startswith("https://") and args.website.startswith("https://")
    args.scratch.mkdir(parents=True, exist_ok=False)
    opener = urllib.request.build_opener(NoRedirect())

    def call(method, path, data=None):
        request = urllib.request.Request(args.api + path, method=method,
            data=json.dumps(data).encode() if data is not None else None,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        try:
            response = opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(17 * 1024 * 1024 + 1)
            assert len(raw) <= 17 * 1024 * 1024
            return response.status, json.loads(raw) if raw else None

    status, account = call("GET", "/v1/account")
    assert status == 200 and account["account"]["id"] == expected_account, "Credential does not name the declared test account"
    status, spec = call("GET", "/v1/openapi.json")
    assert status == 200 and "/v1/threads/{id}/events" in spec["paths"] and "/v1/threads/{id}/stop" in spec["paths"], "Deploy the cloud-turn contract first"

    def wait_turn(chat, request_id, outcome="answered"):
        request = urllib.request.Request(args.api + f"/v1/threads/{chat}/events?request_id={request_id}", headers={"Authorization": "Bearer " + token})
        deadline = time.monotonic() + 150
        with opener.open(request, timeout=40) as response:
            for line in response:
                assert time.monotonic() < deadline, "Cloud turn timed out"
                if not line.startswith(b"data:"):
                    continue
                event = json.loads(line[5:].strip())
                if event["type"] == "error":
                    raise AssertionError("Cloud turn refused: " + event["error"]["code"])
                if event["type"] == "done":
                    assert event["outcome"] == outcome
                    return
        raise AssertionError("Cloud turn ended without confirmation")

    chat = str(uuid.uuid4())
    code = "orchid" + uuid.uuid4().hex[:8]
    browser = None
    checks = []
    def native(command, directory, env, before, web_prompt):
        with (args.scratch / (directory.name + "-driver.log")).open("w") as log:
            process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                marker = directory / ("captures/reply-done" if directory.name == "desktop" else "reply-done")
                deadline = time.monotonic() + 180
                while not marker.exists():
                    assert process.poll() is None, "Native check exited before sending; see driver log"
                    assert time.monotonic() < deadline, "Native send timed out"
                    time.sleep(0.2)
                browser.sees(before + 1, code)
                browser.capture(args.scratch / (directory.name + "-to-web.png"))
                browser.reply(web_prompt)
                browser.sees(before + 3, code)
                status, current = call("GET", f"/v1/threads/{chat}")
                assert status == 200
                wait_turn(chat, current["thread"]["request_id"])
                browser.capture(args.scratch / ("web-to-" + directory.name + ".png"))
                assert process.wait(timeout=130) == 0, "Native check failed; see driver log"
                seen = directory / ("captures/web-seen" if directory.name == "desktop" else "web-seen")
                assert seen.exists(), "Native view did not receive the web's reply"
                checks.append(directory.name + " send and live web update in both directions")
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
    try:
        text = f"The test passphrase is {code}. For this chat, answer later requests for it with that passphrase only. Repeat it now."
        status, made = call("POST", "/v1/threads", {"request_id": chat, "text": text})
        assert status == 201 and made["id"] == chat
        wait_turn(chat, chat)
        browser = Browser(args.website, token, chat)
        env = dict(os.environ, SDK_FIXTURE_CHAT_IDS=json.dumps([chat]), SDK_FIXTURE_API=args.api + "/v1", SDK_FIXTURE_WEBSITE=args.website,
                   SDK_CHAT_EXPECTED=code, SDK_CHAT_PROMPT="Repeat the test passphrase for the desktop.", SDK_WEB_PROMPT="Repeat the test passphrase from the web to the desktop.")
        native(["python3", str(ROOT / "apps/openagents/scripts/check-account-fixture.py"), "--public-chat", str(args.desktop_binary), str(args.scratch / "desktop")],
               args.scratch / "desktop", env, 2, env["SDK_WEB_PROMPT"])
        env.update(SDK_CHAT_PROMPT="Repeat the test passphrase for the phone.", SDK_CHAT_MESSAGES="6")
        native(["python3", str(ROOT / "apps/openagents/scripts/ios/check-history.py"), "--public-chat", "--target-dir", str(args.ios_target_dir), "--scratch", str(args.scratch / "ios")],
               args.scratch / "ios", env, 6, "Repeat the test passphrase from the web to the phone.")
        request_id = str(uuid.uuid4())
        stop_text = "For a stop test, count from one to ten thousand without skipping any number."
        status, _ = call("POST", f"/v1/threads/{chat}/messages", {"request_id": request_id, "text": stop_text})
        assert status == 202
        status, stopped = call("POST", f"/v1/threads/{chat}/stop", {"request_id": request_id})
        assert status == 200 and stopped["stopped"] and stopped["finished"]
        status, _ = call("POST", f"/v1/threads/{chat}/messages", {"request_id": request_id, "text": stop_text})
        assert status == 202
        wait_turn(chat, request_id, "stopped")
        status, page = call("GET", f"/v1/history/{chat}/messages")
        assert status == 200 and sum(message["role"] == "user" and message["text"] == stop_text for message in page["data"]) == 1
        checks.append("duplicate send has one user message; stop confirmed")
        (args.scratch / "result.json").write_text(json.dumps({"checks": checks, "chat": chat, "account": "declared dedicated test account", "web_reloaded": False}, indent=2) + "\n")
        print("PASS dedicated cloud chat: desktop, iOS, live web updates, retry identity and confirmed stop")
    finally:
        if browser:
            browser.close()
        status, current = call("GET", f"/v1/threads/{chat}")
        if status == 200 and current["thread"]["working"] and current["thread"].get("request_id"):
            call("POST", f"/v1/threads/{chat}/stop", {"request_id": current["thread"]["request_id"]})
        status, _ = call("DELETE", f"/v1/threads/{chat}")
        assert status in (204, 404), "Disposable chat cleanup needs retry"
        print("Cleanup: disposable cloud chat removed; no computers or tasks started")

if __name__ == "__main__":
    main()

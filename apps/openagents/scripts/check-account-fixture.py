#!/usr/bin/env python3
"""Run the native auth fixture under `openagents browser run`."""
import argparse
import base64
import json
import os
import signal
from pathlib import Path
import plistlib
import shutil
import subprocess
import time
import urllib.parse
import urllib.request

import websocket


def main():
    def interrupted(_signal, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--continue-chat", action="store_true")
    parser.add_argument("--public-chat", action="store_true")
    parser.add_argument("--public-history", action="store_true")
    parser.add_argument("--history", action="store_true")
    parser.add_argument("--reader", action="store_true")
    parser.add_argument("binary", type=Path)
    parser.add_argument("scratch", type=Path)
    args = parser.parse_args()
    port = os.environ["OPENAGENTS_CHROME_PORT"] if not (args.public_history or args.public_chat or args.continue_chat) else None
    root = args.scratch.resolve()
    root.mkdir(parents=True, exist_ok=False)
    app = root / "Coder Account Fixture.app"
    contents = app / "Contents"
    executable = contents / "MacOS" / "account-fixture"
    executable.parent.mkdir(parents=True)
    shutil.copyfile(args.binary, executable)
    executable.chmod(0o755)
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleIdentifier": "com.openagents.desktop.dev.account-fixture",
            "CFBundleName": "Coder Account Fixture",
            "CFBundleExecutable": "account-fixture",
            "CFBundlePackageType": "APPL",
            "CFBundleVersion": "1",
            "NSHighResolutionCapable": True,
        }, output)
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
    output = root / "captures"
    with (root / "native.log").open("w") as log:
        native = subprocess.Popen([str(executable), str(output)] + (["--continue-chat"] if args.continue_chat else ["--public-chat"] if args.public_chat else ["--public-history"] if args.public_history else ["--reader"] if args.reader else ["--history"] if args.history else []), stdout=log, stderr=log)
        try:
            if args.public_history or args.public_chat or args.continue_chat:
                status = native.wait(timeout=450 if args.continue_chat else 230 if args.public_chat else 110)
                if status != 0:
                    raise RuntimeError(f"public native fixture exited {status}; see native.log")
                names = ["signed-in", "public-sidebar", "continued-on-computer", "signed-out"] if args.continue_chat else ["signed-in", "public-sidebar", "public-cloud-reply", "public-cloud-from-web", "signed-out"] if args.public_chat else ["signed-in", "public-sidebar", "public-reader-0", "public-reader-1", "public-reader-2", "signed-out"]
                for name in names:
                    if not (output / f"{name}.png").is_file():
                        raise RuntimeError(f"missing capture: {name}")
                print("Native continuation and earlier context passed." if args.continue_chat else "Native cloud send and web-to-desktop update passed." if args.public_chat else "Native deployed history: three chats, 615 messages, restore, and local sign-out passed.")
                return
            deadline = time.monotonic() + 100
            while not (output / "waiting.png").exists():
                if native.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError("native fixture did not reach browser approval")
                time.sleep(0.1)
            url = (output / "browser-url").read_text().strip()
            request = urllib.request.Request(
                f"http://127.0.0.1:{port}/json/new?{urllib.parse.quote(url, safe='')}",
                method="PUT",
            )
            with urllib.request.urlopen(request) as response:
                page = json.load(response)
            connection = websocket.create_connection(page["webSocketDebuggerUrl"], suppress_origin=True)
            sequence = 0

            def call(method, params):
                nonlocal sequence
                sequence += 1
                connection.send(json.dumps({"id": sequence, "method": method, "params": params}))
                while True:
                    answer = json.loads(connection.recv())
                    if answer.get("id") == sequence:
                        if "error" in answer:
                            raise RuntimeError(answer["error"])
                        return answer["result"]

            deadline = time.monotonic() + 10
            while True:
                ready = call("Runtime.evaluate", {"expression": "document.body?.innerText", "returnByValue": True})
                if "BCDF-GHJK" in ready.get("result", {}).get("value", ""):
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError("browser did not display the approval code")
                time.sleep(0.1)
            screenshot = call("Page.captureScreenshot", {"format": "png"})
            (root / "browser-approval.png").write_bytes(base64.b64decode(screenshot["data"]))
            call("Runtime.evaluate", {"expression": "document.querySelector('button').click()"})
            status = native.wait(timeout=30)
            connection.close()
            if status != 0:
                raise RuntimeError(f"native fixture exited {status}")
            captures = ["waiting", "signed-in", "signed-out"]
            if args.history or args.reader:
                captures += ["sidebar-200", "sidebar-205-pinned", "selected-existing", "archived", "shared-project", "offline-retry"]
            if args.reader:
                captures += ["reader-200", "reader-405", "reader-file", "reader-changed", "reader-reloaded"]
            for name in captures:
                if not (output / f"{name}.png").is_file():
                    raise RuntimeError(f"missing native capture: {name}")
        finally:
            if native.poll() is None:
                native.terminate()
                native.wait(timeout=10)
            if args.public_chat or args.continue_chat:
                shutil.rmtree(app)
    print("Native sidebar authentication and isolated browser approval passed.")


if __name__ == "__main__":
    main()

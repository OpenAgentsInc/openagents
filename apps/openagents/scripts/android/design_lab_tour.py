"""Drive the Android debug app's offline demo through every screen for Design Lab.

Run by scripts/design-lab-capture-android.sh. Each screen is a fresh launch of
the demo (`--ez openagents.demo true`) with an optional `openagents.route`,
like the iOS DesignLabTourTests, so one failed screen never affects the next.
Every shot saves `<area>--<screen>[@dark].png` and its element tree as JSON in
the shape Design Lab reads (`ElementTree`: frames in dp, `scale` = pixels per
dp). Nothing signs in and nothing reaches the network.
"""

import argparse
import json
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET

PACKAGE = "com.openagents.app.debug"
ACTIVITY = PACKAGE + "/com.openagents.app.MainActivity"
# The iOS tour's demo sessions, in the same order (DesignLabTourTests.testSessions).
SESSIONS = [
    "veil", "picker", "tabs", "errored", "deploy", "ios-scroll", "ios-keyboard",
    "home", "visuals", "cjk", "blog", "side", "oklch", "presence",
]
KINDS = {
    "Button": "button", "ImageButton": "button", "RadioButton": "radio-button",
    "CheckBox": "check-box", "Switch": "switch", "EditText": "text-field",
    "TextView": "text", "ImageView": "image", "ListView": "table",
    "RecyclerView": "table", "ScrollView": "scroll-view",
    "HorizontalScrollView": "scroll-view", "ProgressBar": "progress",
}


class Device:
    def __init__(self, adb, serial):
        self.base = [adb] + (["-s", serial] if serial else [])

    def run(self, *args, check=True, binary=False):
        out = subprocess.run(self.base + list(args), capture_output=True, check=check)
        return out.stdout if binary else out.stdout.decode(errors="replace")

    def shell(self, *args):
        return self.run("shell", *args)

    def density(self):
        match = re.search(r"(\d+)", self.shell("wm", "density").split(":")[-1])
        return int(match.group(1)) if match else 420

    def launch(self, route=None):
        self.shell("am", "force-stop", PACKAGE)
        args = ["am", "start", "-W", "-n", ACTIVITY, "--ez", "openagents.demo", "true"]
        if route:
            args += ["--es", "openagents.route", route]
        self.shell(*args)
        time.sleep(4)

    def nodes(self):
        self.shell("uiautomator", "dump", "/sdcard/design-lab.xml")
        xml = self.run("exec-out", "cat", "/sdcard/design-lab.xml")
        try:
            root = ET.fromstring(xml[xml.index("<"):])
        except (ValueError, ET.ParseError):
            return []
        found = []
        for node in root.iter("node"):
            bounds = re.findall(r"\d+", node.get("bounds", ""))
            if len(bounds) != 4:
                continue
            left, top, right, bottom = map(int, bounds)
            found.append({
                "class": node.get("class", "").split(".")[-1],
                "text": node.get("text", ""),
                "desc": node.get("content-desc", ""),
                "id": node.get("resource-id", "").split("/")[-1],
                "bounds": (left, top, right, bottom),
                "clickable": node.get("clickable") == "true",
            })
        return found

    def find(self, *labels):
        wanted = [label.lower() for label in labels]
        for node in self.nodes():
            for value in (node["desc"], node["text"]):
                if value and value.strip().lower() in wanted:
                    return node
        return None

    def tap(self, node):
        left, top, right, bottom = node["bounds"]
        self.shell("input", "tap", str((left + right) // 2), str((top + bottom) // 2))
        time.sleep(1.5)


def element_tree(device, nodes, scale):
    width, height = 0, 0
    elements = []
    for node in nodes:
        left, top, right, bottom = node["bounds"]
        width, height = max(width, right), max(height, bottom)
        if right <= left or bottom <= top:
            continue
        element = {
            "type": KINDS.get(node["class"], "other"),
            "x": round(left / scale, 2), "y": round(top / scale, 2),
            "width": round((right - left) / scale, 2), "height": round((bottom - top) / scale, 2),
        }
        if node["id"]:
            element["identifier"] = node["id"]
        label = node["desc"] or (node["text"] if element["type"] != "text-field" else "")
        if label:
            element["label"] = label[:500]
        if element["type"] == "text-field" and node["text"]:
            element["value"] = node["text"][:500]
        elements.append(element)
    return {"scale": scale, "width": round(width / scale, 2), "height": round(height / scale, 2),
            "elements": elements}


def shot(device, out, area, screen, dark, scale):
    name = f"{area}--{screen}{'@dark' if dark else ''}"
    png = device.run("exec-out", "screencap", "-p", binary=True)
    if not png.startswith(b"\x89PNG"):
        raise RuntimeError(f"no picture for {name}")
    with open(f"{out}/{name}.png", "wb") as f:
        f.write(png)
    with open(f"{out}/{name}.json", "w") as f:
        json.dump(element_tree(device, device.nodes(), scale), f)
    print(f"design-lab-tour: {name}", flush=True)


def tour(device, out, dark, scale):
    failures = []

    def attempt(label, action):
        try:
            action()
        except Exception as error:  # Keep the screens already taken.
            failures.append(f"{label}: {error}")
            print(f"design-lab-tour: {label} failed: {error}", file=sys.stderr, flush=True)

    def cloud():
        device.launch()
        shot(device, out, "chat", "cloud-chat", dark, scale)
        composer = device.find("Message", "Ask OpenAgents")
        if composer:
            device.tap(composer)
            shot(device, out, "chat", "composer-keyboard", dark, scale)

    def drawer():
        device.launch("drawer")
        shot(device, out, "drawer", "sessions", dark, scale)
        size = device.shell("wm", "size").split(":")[-1].strip().split("x")
        w, h = int(size[0]), int(size[1])
        device.shell("input", "swipe", str(w // 2), str(h * 3 // 4), str(w // 2), str(h // 4), "400")
        time.sleep(1.5)
        shot(device, out, "drawer", "sessions-scrolled", dark, scale)

    def search():
        device.launch("drawer")
        field = device.find("Search chats", "Search")
        if field is None:
            raise RuntimeError("no search field")
        device.tap(field)
        device.shell("input", "text", "veil")
        time.sleep(1.5)
        shot(device, out, "drawer", "search", dark, scale)

    def route(name, area, screen):
        def run():
            device.launch(name)
            shot(device, out, area, screen, dark, scale)
        return run

    attempt("cloud chat", cloud)
    attempt("drawer", drawer)
    attempt("search", search)
    for session in SESSIONS:
        attempt(f"session {session}", route(f"chat:chat-{session}", "session", session))
    return failures


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--adb", default="adb")
    parser.add_argument("--serial")
    parser.add_argument("--apk")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    device = Device(args.adb, args.serial)
    if args.apk:
        device.run("install", "-r", args.apk)
    scale = device.density() / 160
    failures = []
    try:
        for dark in (False, True):
            device.shell("cmd", "uimode", "night", "yes" if dark else "no")
            time.sleep(1)
            failures += tour(device, args.out, dark, scale)
    finally:
        device.shell("cmd", "uimode", "night", "no")
        device.shell("am", "force-stop", PACKAGE)
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()

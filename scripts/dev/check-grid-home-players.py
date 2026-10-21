#!/usr/bin/env python3
"""Check the Grid homepage (`/`, formerly /home) with several signed-out players on one local relay (#12406).

    check-grid-home-players.py --url http://127.0.0.1:PORT/ \\
        --relay ws://127.0.0.1:RELAY_PORT --out DIR [--players 3]

Starts one `openagents browser run` per player, each with its own Chrome
profile, so each gets its own browser-local key. Players 1..N-1 join
first; player 0 stays on the overview until it sees them all, then joins.
Then it checks, from every page:

- every other player is shown, tagged with the npub prefix of its key;
- no two players spawned overlapping;
- player 0, walking straight at player 1, stops against them: never
  inside, never through, and steady once stopped.

Captures and a JSON receipt go to DIR. Use only a local server and relay;
nothing signs in.
"""

import argparse
import base64
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request
from urllib.parse import urlsplit

RADIUS = 0.45
SPAWN_SPACING = RADIUS * 2 + 1.2

parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
parser.add_argument("--url", default="http://127.0.0.1:4331/")
parser.add_argument("--relay", required=True)
parser.add_argument("--out", required=True, type=Path)
parser.add_argument("--players", type=int, default=3)
parser.add_argument("--player", type=int, help=argparse.SUPPRESS)
parser.add_argument("--timeout", type=float, default=150)
args = parser.parse_args()
assert urlsplit(args.url).hostname in ("localhost", "127.0.0.1", "::1"), "Use a local server."
assert urlsplit(args.relay).hostname in ("localhost", "127.0.0.1", "::1"), "Use a local relay."
assert 2 <= args.players <= 3
args.out.mkdir(parents=True, exist_ok=True)


def note(name):
    return args.out / f"{name}.json"


def wait_note(name, timeout):
    deadline = time.monotonic() + timeout
    while not note(name).exists():
        if time.monotonic() > deadline:
            raise TimeoutError(name)
        time.sleep(0.1)
    return json.loads(note(name).read_text())


def write_note(name, value):
    tmp = note(name).with_suffix(".tmp")
    tmp.write_text(json.dumps(value, indent=2) + "\n")
    tmp.replace(note(name))


def coordinate():
    for stale in args.out.glob("*.json"):
        stale.unlink()
    me = Path(__file__).resolve()
    runs = [
        subprocess.Popen([
            "openagents", "browser", "run", "--", sys.executable, str(me),
            "--url", args.url, "--relay", args.relay, "--out", str(args.out),
            "--players", str(args.players), "--player", str(i), "--timeout", str(args.timeout),
        ])
        for i in range(args.players)
    ]
    codes = [run.wait() for run in runs]
    results = [json.loads(note(f"result-{i}").read_text()) for i in range(args.players) if note(f"result-{i}").exists()]
    receipt = {"url": args.url, "relay": args.relay, "exit": codes, "players": results}
    (args.out / "players-check.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt))
    sys.exit(0 if all(code == 0 for code in codes) and len(results) == args.players else 1)


def play(i):
    root = Path(__file__).resolve().parents[2]
    import importlib.util
    spec = importlib.util.spec_from_file_location("browser_cdp", root / "bench/verse/2026-10-05/platform-clients/browser-cdp.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    port = os.environ["OPENAGENTS_CHROME_PORT"]
    target = json.load(urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}/json/new?about:blank", method="PUT")))
    cdp = module.Cdp(target["webSocketDebuggerUrl"])
    cdp.call("Page.enable")
    cdp.call("Runtime.enable")
    # The local relay isn't in /home's policy; production's relay is.
    cdp.call("Page.setBypassCSP", {"enabled": True})
    cdp.call("Emulation.setDeviceMetricsOverride", {"width": 1280, "height": 800, "deviceScaleFactor": 1, "mobile": False})

    def evaluate(expression):
        result = cdp.eval(expression)
        if "exceptionDetails" in result:
            raise RuntimeError(result["exceptionDetails"])
        return result["result"].get("value")

    def players():
        text = evaluate('import("/everglade/everglade_web.js").then(m=>m.verse_players())')
        return json.loads(text) if text else None

    def wait_for(condition, label, timeout=args.timeout):
        deadline = time.monotonic() + timeout
        while True:
            value = condition()
            if value:
                return value
            if time.monotonic() > deadline:
                raise TimeoutError((i, label, players()))
            time.sleep(0.1)

    def capture(name):
        png = cdp.call("Page.captureScreenshot", {"format": "png"})["data"]
        (args.out / name).write_bytes(base64.b64decode(png))

    def key(code, down):
        name = code[-1].lower()
        cdp.call("Input.dispatchKeyEvent", {"type": "keyDown" if down else "keyUp", "code": code, "key": name})

    def mark(name):
        return evaluate(f'performance.getEntriesByName("{name}").length > 0')

    def join():
        # A click on the overview joins; the glide ends in third person.
        point = evaluate("""(()=>{for(const y of [700,600,500,400])for(const x of [80,640,1200])
            if(document.elementFromPoint(x,y)?.id==="everglade-canvas")return {x,y};return null})()""")
        assert point, "No world area takes the click."
        for kind in ("mousePressed", "mouseReleased"):
            cdp.call("Input.dispatchMouseEvent", {"type": kind, **point, "button": "left", "buttons": 1 if kind == "mousePressed" else 0, "clickCount": 1})
        wait_for(lambda: mark("home-third-person"), "third person")

    relay = urllib.request.quote(args.relay, safe="")
    cdp.call("Page.navigate", {"url": f"{args.url}?relay={relay}"})
    wait_for(lambda: mark("verse-visible"), "first frame")
    wait_for(lambda: players() is not None, "presence")
    result = {"player": i}
    if i != 0:
        join()
        me = players()["me"]
        write_note(f"joined-{i}", me)
    else:
        # The overview shows the players already in the Grid.
        others = [wait_note(f"joined-{n}", args.timeout) for n in range(1, args.players)]
        keys = {o["pubkey"] for o in others}
        seen = wait_for(lambda: (lambda p: p if {o["pubkey"] for o in p["others"]} >= keys else None)(players()), "overview players")
        assert not seen["joined"], seen
        time.sleep(1.0)
        capture("overview-player-0.png")
        result["overview_saw"] = len(seen["others"])
        join()
        write_note("joined-0", players()["me"])
    everyone = [wait_note(f"joined-{n}", args.timeout) for n in range(args.players)]
    keys = {p["pubkey"] for p in everyone} - {players()["me"]["pubkey"]}
    seen = wait_for(lambda: (lambda p: p if {o["pubkey"] for o in p["others"]} >= keys else None)(players()), "everyone")
    me = seen["me"]
    # Tags: the npub prefix of each key, and no name for signed-out visitors.
    for other in seen["others"]:
        prefix, name = other["tag"]
        assert prefix.startswith("npub1") and prefix.endswith("…") and len(prefix) == 12, other
        assert name is None, other
    assert me["tag"][0].startswith("npub1"), me
    # Spawns: nobody overlaps.
    spots = [p["pos"] for p in everyone]
    gaps = [math.dist(a[::2], b[::2]) for n, a in enumerate(spots) for b in spots[n + 1:]]
    assert min(gaps) >= RADIUS * 2, gaps
    result.update({"pubkey": me["pubkey"], "tag": me["tag"], "spawn": me["pos"], "saw": [o["tag"] for o in seen["others"]], "spawn_gaps": gaps})
    time.sleep(1.0)
    capture(f"players-{i}.png")
    write_note(f"seen-{i}", result)
    if i == 0:
        # Walk straight at player 1 for three seconds.
        for n in range(1, args.players):
            wait_note(f"seen-{n}", args.timeout)
        target = everyone[1]["pos"]
        evaluate(f'import("/everglade/everglade_web.js").then(m=>m.verse_face({target[0]},{target[2]}))')
        time.sleep(0.2)
        path = []
        key("KeyW", True)
        try:
            end = time.monotonic() + 3.0
            while time.monotonic() < end:
                p = players()
                other = next(o for o in p["others"] if o["pubkey"] == everyone[1]["pubkey"])
                path.append({"me": p["me"]["pos"], "other": other["pos"]})
                time.sleep(0.05)
        finally:
            key("KeyW", False)
        capture("collision-player-0.png")
        gaps = [math.dist(s["me"][::2], s["other"][::2]) for s in path]
        start = math.dist(path[0]["me"][::2], target[::2])
        assert start > 1.0, start
        assert min(gaps) >= RADIUS * 2 - 0.02, min(gaps)
        # Stopped against them, on this side: closer than when it set out.
        last = path[-1]
        assert gaps[-1] < 1.4, gaps[-1]
        ahead = [t - m for t, m in zip(target[::2], path[0]["me"][::2])]
        along = sum(a * (l - t) for a, l, t in zip(ahead, last["me"][::2], target[::2]))
        assert along < 0, ("passed through", last)
        # Steady once stopped: the last second moves under 5 cm.
        tail = [s["me"] for s in path[-20:]]
        drift = max(math.dist(a[::2], tail[-1][::2]) for a in tail)
        assert drift < 0.05, drift
        result["collision"] = {"start_gap": start, "closest": min(gaps), "final_gap": gaps[-1], "final_drift": drift, "samples": len(path)}
        write_note("done", {})
    else:
        wait_note("done", args.timeout)
        # The walker as this page saw them: never inside this player.
        p = players()
        result["final_others"] = [o["pos"] for o in p["others"]]
    write_note(f"result-{i}", result)


if args.player is None:
    coordinate()
else:
    play(args.player)

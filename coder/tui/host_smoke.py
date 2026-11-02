#!/usr/bin/env python3
"""coder host against a local Coax relay (coax/relay), in a fresh CODER_HOME:
a stand-in browser key (coder host call) asks to pair, is refused a thread
before it is allowed, is allowed with coder host allow, reads its grant from
browser.status, runs a thread on the echo engine, reads its steps and stops
it. An unpaired key is refused, a forgotten key is refused revoked, and
browser.status keeps answering for the pairing (allowed, then denied).

    python3 coder/tui/host_smoke.py            # builds the relay into a temporary folder

A host to point a browser or another harness at, by hand (a relay URL names
a host, never an IP literal: relay_uri.coax refuses ws://127.0.0.1):

    coax/relay/build.sh /tmp/relay && /tmp/relay --listen 127.0.0.1 --port 7777 --url ws://localhost:7777
    OPENAGENTS_NIP_HOST_RELAY=ws://localhost:7777 CODER_HOME=/tmp/host-home coder/tui/coder.sh host --engine echo
    CODER_HOME=/tmp/host-home coder/tui/coder.sh host allow ID    # when the browser's request shows

Test-only harness; the programs under test (coder/tui/host.coax, the relay)
are Coax.
"""
import json, os, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
CODER = os.path.join(HERE, "coder.sh")


def free_port():
    s = socket.socket(); s.bind(("127.0.0.1", 0)); p = s.getsockname()[1]; s.close(); return p


def main():
    tmp = tempfile.mkdtemp(prefix="coder-host-smoke.")
    relay_bin = os.path.join(tmp, "relay")
    subprocess.run([os.path.join(REPO, "coax/relay/build.sh"), relay_bin], check=True, stdout=subprocess.DEVNULL)
    port = free_port(); url = f"ws://localhost:{port}"   # relay URLs name a host, never an IP literal
    relay = subprocess.Popen([relay_bin, "--listen", "127.0.0.1", "--port", str(port), "--url", url],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    env = dict(os.environ, CODER_HOME=os.path.join(tmp, "home"), BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1",
               OPENAGENTS_NIP_HOST_RELAY=url, CODER_REALM_TOOLS="0", CODER_USAGE_KEYCHAIN="0")
    os.makedirs(env["CODER_HOME"])
    failures = []

    def check(ok, what):
        print(("ok   " if ok else "FAIL ") + what)
        if not ok: failures.append(what)

    def host(*args, timeout=300):
        return subprocess.run([CODER, "host", *args], cwd=REPO, env=env, capture_output=True, text=True, timeout=timeout)

    serve = None
    try:
        printed = host("key").stdout.split()   # builds the host tool on first use
        key = printed[0] if printed else ""
        check(len(key) == 64 and len(printed) == 2 and printed[1].startswith("npub1"), "coder host key prints the host key as 64 hex and an npub")
        mode = os.stat(os.path.join(env["CODER_HOME"], ".openagents/coder/host/key")).st_mode & 0o777
        check(mode == 0o600, "the host key file is mode 600")
        serve = subprocess.Popen([CODER, "host", "--engine", "echo", "--for", "240"], cwd=REPO, env=env,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        line = serve.stdout.readline()
        check(key in line and printed[1] in line, "coder host serves and prints its npub and hex key")
        shown = line
        while "Listening" not in line:
            line = serve.stdout.readline(); shown += line
            if not line: break
        check("EDIT > COMPUTER" in shown, "it says where to paste the key")
        bk = os.path.join(tmp, "browser.key"); other = os.path.join(tmp, "other.key")

        def call(op, *args, key_file=bk, label=None):
            r = host("call", op, "--host", key, "--browser-key", key_file, *args, timeout=60)
            if r.returncode != 0:
                return {"exit": r.returncode, "err": r.stderr.strip()}
            res = json.loads(r.stdout)["result"]   # the reply body: result.status ok or refused
            return dict(res, ok=res["status"] == "ok")

        r = call("browser.request", "--label", "Smoke browser")
        check(r.get("ok") and r["outcome"]["kind"] == "browser_request" and r["outcome"]["state"] == "asked", "a browser asks to pair")
        pairing = r["outcome"]["request"]
        r = call("browser.status", "--request", pairing)
        check(r.get("ok") and r["outcome"]["state"] == "asked", "it waits: browser.status says asked")
        fake_grant = "ab" * 32
        r = call("thread.run", "--text", "Add a well", "--realm", "garden", "--grant", fake_grant, "--epoch", "0")
        check(r.get("ok") is False and r["code"] == "forbidden", "a thread before it is allowed is refused (" + str(r.get("code")) + ")")
        listed = json.loads(host("list", "--json").stdout)
        check(len(listed["waiting"]) == 1 and listed["waiting"][0]["label"] == "Smoke browser", "coder host list shows it waiting")
        a = host("allow", pairing[:8])
        check(a.returncode == 0 and "Allowed Smoke browser" in a.stdout, "coder host allow allows it")
        r = call("browser.status", "--request", pairing)
        ok = r.get("ok") and r["outcome"]["state"] == "allowed"
        check(ok and r["outcome"]["grant"]["rights"] == ["observe", "operate"], "browser.status hands it the grant")
        grant = r["outcome"]["grant"]["grant"]; epoch = str(r["outcome"]["grant"]["epoch"])
        g = ["--grant", grant, "--epoch", epoch]
        r = call("thread.run", "--text", "Add a well by the oak", "--realm", "garden", *g)
        check(r.get("ok") and r["outcome"]["kind"] == "dispatched" and r["outcome"]["operation"] == "thread.run", "thread.run starts a job")
        thread = r["outcome"]["reference"]
        steps = None
        for _ in range(30):
            r = call("thread.read", "--thread", thread, "--after", "0", *g)
            if r.get("ok") and r["outcome"]["state"] != "running": steps = r; break
            time.sleep(1)
        check(steps is not None and steps["outcome"]["state"] == "finished", "thread.read follows it to finished")
        kinds = [s["kind"] for s in (steps or {"outcome": {"steps": []}})["outcome"]["steps"]]
        check(kinds[:1] == ["user"] and "reply" in kinds and kinds[-1:] == ["end"], "its steps: " + ",".join(kinds))
        user = steps["outcome"]["steps"][0]["text"] if steps else ""
        check("garden" in user and "Add a well by the oak" in user, "the job was told the realm and the words")
        nxt = str(steps["outcome"]["next"]) if steps else "0"
        r = call("thread.read", "--thread", thread, "--after", nxt, *g)
        check(r.get("ok") and r["outcome"]["steps"] == [], "thread.read after next returns no steps")
        r = call("thread.stop", "--thread", thread, *g)
        check(r.get("ok") and r["outcome"]["kind"] == "dispatched" and r["outcome"]["operation"] == "thread.stop", "thread.stop answers dispatched")
        # the job's session ran under the read ceiling with the realm tools carve-out
        sessions = os.path.join(env["CODER_HOME"], ".openagents/coder/sessions")
        head = ""
        for f in os.listdir(sessions):
            if f.endswith(".jsonl"):
                with open(os.path.join(sessions, f)) as fh: head = fh.readline()
        check('"engine":"echo"' in head, "the job ran on the host's engine")
        r = call("thread.read", "--thread", thread, "--after", "0", *g, key_file=other)
        check(r.get("ok") is False and r["code"] == "forbidden", "an unpaired key is refused (" + str(r.get("code")) + ")")
        r = call("browser.status", "--request", pairing)
        check(r.get("ok") and r["outcome"]["state"] == "allowed" and r["outcome"]["grant"]["grant"] == grant, "browser.status keeps answering allowed for the pairing while the grant lasts")
        f = host("forget", grant[:8])
        check(f.returncode == 0 and "Revoked" in f.stdout, "coder host forget revokes it")
        r = call("thread.read", "--thread", thread, "--after", "0", *g)
        check(r.get("ok") is False and r["code"] == "revoked", "a forgotten key is refused (" + str(r.get("code")) + ")")
        r = call("browser.status", "--request", pairing)
        check(r.get("ok") and r["outcome"]["state"] == "denied" and r["outcome"]["grant"] is None, "browser.status says denied once forgotten, with no grant")
        r = call("browser.request", "--label", "Smoke again")
        check(r.get("ok") and r["outcome"]["state"] == "asked", "the forgotten browser can ask again")
        r = call("browser.forget")
        check(r.get("ok") and r["outcome"]["kind"] == "forgotten", "browser.forget answers forgotten")
        listed = json.loads(host("list", "--json").stdout)
        check(listed["waiting"] == [] and listed["browsers"] == [], "nothing is waiting or allowed after browser.forget")
        st = os.stat(os.path.join(env["CODER_HOME"], ".openagents/coder/host/state.json")).st_mode & 0o777
        check(st == 0o600, "state.json is mode 600")
    finally:
        if serve is not None:
            serve.terminate()
            try: serve.wait(10)
            except subprocess.TimeoutExpired: serve.kill()
        relay.terminate()
        try: relay.wait(10)
        except subprocess.TimeoutExpired: relay.kill()
    print(f"{'FAILED' if failures else 'passed'}: {len(failures)} failures")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

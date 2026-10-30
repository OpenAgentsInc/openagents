#!/usr/bin/env python3
"""The GPU switch: one card, two Psionic servers, one at a time.

coderos-4080's RTX 4080 has 16 GiB. elephant-1 under psionic-serve
--elephant-gpu (/v1/systemone, port 8790) holds about 13.3 GiB resident plus
1.5 GiB pinned; gpt-oss-20b behind the Psionic Responses API (port 8080)
holds about 13 GiB. They cannot both be resident, so this small loopback
service owns the card and swaps on demand for the OpenAgents Mac app
(docs/desktop/coax-desktop.md):

    POST /use/elephant   POST /use/gptoss   POST /stop   GET /status

Each answers one line of text, "STATE MODEL SECONDS [REASON]":

    ready gptoss 41.3            loaded (SECONDS: how long the load took)
    loading elephant 12.0        loading (SECONDS: so far)
    failed gptoss 3.1 REASON     the load failed, and why
    stopped - 0                  nothing is loaded

A /use for the model already loaded or loading changes nothing (it answers
"ready" at once when loaded). A /use for
the other one stops the running server (SIGTERM to its process group, then
SIGKILL after 30 s), waits for its port to close, starts the requested one
and reports ready once its port accepts connections (both servers bind only
after their weights are resident). The switch only ever stops servers it
started itself: if a port is held by anything else, the load fails and says
which process holds it. The last requested model is kept in
~/.config/psionic-gpu-switch.state and loaded again when the switch starts
(after a reboot or a crash: it runs as the systemd user unit
psionic-gpu-switch.service, Restart=always, linger on). Listens on
127.0.0.1:8791 only; the Mac reaches it through an SSH tunnel.

POST /stop then /use/gptoss restarts gpt-oss-20b (to serve a new build),
and every app user's next Psionic turn waits the whole load, which is what
cost a Lab capture 103 s on 2026-10-29. Build first
(psionic/serve-gptoss.sh --build-only in ~/psionic-responses, so the load
is only the weights) and restart when no one is mid-turn. Restarting this
unit restarts the loaded server too.
"""
import http.server
import os
import signal
import socket
import subprocess
import threading
import time

HOME = os.path.expanduser("~")
PORT = int(os.environ.get("GPU_SWITCH_PORT", "8791"))
STATE_FILE = os.path.join(HOME, ".config", "psionic-gpu-switch.state")
LOAD_LIMIT_S = 900

MODELS = {
    "elephant": {
        "port": 8790,
        "cwd": os.path.join(HOME, "oa-lab-elephant"),
        "cmd": ["psionic/platform/serve.sh", "--elephant-gpu", os.path.join(HOME, "models", "elephant-1-b3f8a934"),
                "--port", "8790", "--capacity", "512"],
        "env": {"PSIONIC_SERVE_BUILD": os.path.join(HOME, "oa-lab-elephant", ".serve-build")},
        "log": os.path.join(HOME, "elephant-systemone.log"),
    },
    "gptoss": {
        "port": 8080,
        "cwd": os.path.join(HOME, "psionic-responses"),
        "cmd": [os.path.join(HOME, "psionic-responses", "serve.sh")],   # reads its key from ~/.config, tees its own log
        "env": {},
        "log": os.path.join(HOME, "psionic-responses-switch.log"),
    },
}

lock = threading.Condition()
want = None          # the model asked for, or None
have = None          # the model whose server runs (loading or ready), or None
proc = None          # its process (the group leader)
state = "stopped"    # stopped | loading | ready | failed
since = time.time()  # when the current state's clock started
took = 0.0           # the last load's seconds
reason = ""


def log(msg):
    print(time.strftime("%Y-%m-%dT%H:%M:%S"), msg, flush=True)


def port_open(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=1):
            return True
    except OSError:
        return False


def holder(port):
    """Who listens on the port (ss -ltnp), for a refusal."""
    try:
        out = subprocess.run(["ss", "-ltnpH", f"sport = :{port}"], capture_output=True, text=True, timeout=5).stdout
        return out.split("users:", 1)[1].strip() if "users:" in out else "an unknown process"
    except Exception:
        return "an unknown process"


def tail(path, n=3):
    try:
        with open(path, "rb") as f:
            f.seek(0, 2)
            f.seek(max(0, f.tell() - 4096))
            lines = [l.strip() for l in f.read().decode("utf-8", "replace").splitlines() if l.strip()]
        return " | ".join(lines[-n:])
    except OSError:
        return ""


def stop_ours():
    """Stops the server this switch started, and waits for its port to close."""
    global proc, have
    if proc is None:
        have = None
        return
    port = MODELS[have]["port"]
    log(f"stopping {have} (pid {proc.pid})")
    try:
        os.killpg(proc.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        proc.wait(timeout=30)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        proc.wait()
    try:
        os.killpg(proc.pid, signal.SIGKILL)   # stragglers in its group
    except ProcessLookupError:
        pass
    for _ in range(100):
        if not port_open(port):
            break
        time.sleep(0.1)
    proc = None
    have = None


def set_state(s, why=""):
    global state, since, reason
    state, since, reason = s, time.time(), why
    log(f"{s} {have or want or '-'} {why}".rstrip())


def start(target):
    """Starts a server (lock held): loading, or failed when its port is someone else's."""
    global have, proc
    m = MODELS[target]
    have = target
    if port_open(m["port"]):
        set_state("failed", f"port {m['port']} is held by {holder(m['port'])}, not by the switch")
        return
    env = dict(os.environ)
    env.update(m["env"])
    with open(m["log"], "ab") as logf:
        logf.write(f"== {time.strftime('%Y-%m-%dT%H:%M:%S')} started by the GPU switch\n".encode())
        logf.flush()
        proc = subprocess.Popen(m["cmd"], cwd=m["cwd"], env=env, stdin=subprocess.DEVNULL, stdout=logf,
                                stderr=subprocess.STDOUT, start_new_session=True)
    set_state("loading")


def worker():
    """Brings the card to what was asked for, and watches the server."""
    global proc, took
    while True:
        with lock:
            if want != have:
                stop_ours()
                if want is None:
                    set_state("stopped")
                else:
                    start(want)
            elif have is not None and proc is not None:
                code = proc.poll()
                if code is not None:
                    proc = None
                    set_state("failed", f"the server exited ({code}): {tail(MODELS[have]['log']) or 'no log'}")
                elif state == "loading" and port_open(MODELS[have]["port"]):
                    took = time.time() - since
                    set_state("ready", f"in {took:.1f} s")
                elif state == "loading" and time.time() - since > LOAD_LIMIT_S:
                    stop_ours()
                    globals()["have"] = want
                    set_state("failed", f"not ready after {LOAD_LIMIT_S} s")
            lock.wait(timeout=0.5)


def status_line():
    with lock:
        model = have or want or "-"
        if state == "ready":
            return f"ready {model} {took:.1f}"
        secs = time.time() - since
        if state == "loading":
            return f"loading {model} {secs:.1f}"
        if state == "failed":
            return f"failed {model} {secs:.1f} {reason}"
        return "stopped - 0"


def ask(model):
    """A /use or /stop. Every one that changes something is logged first, as
    "asked MODEL (was STATE MODEL)", so the log says why a model was stopped
    or loaded; a /use for the model already ready logs "asked MODEL: ready",
    and repeated asks while it loads (the app polls) log nothing."""
    global want
    with lock:
        name = model or "stop"
        if model != want or state == "failed":
            log(f"asked {name} (was {state} {have or want or '-'})")
            want = model
            if state == "failed" and have == model:
                globals()["have"] = None   # a retry of the same model starts over
            try:
                os.makedirs(os.path.dirname(STATE_FILE), exist_ok=True)
                with open(STATE_FILE, "w") as f:
                    f.write((model or "") + "\n")
            except OSError:
                pass
            lock.notify_all()
        elif state == "ready" and model is not None:
            log(f"asked {name}: ready")
    time.sleep(0.05)
    return status_line()


class Handler(http.server.BaseHTTPRequestHandler):
    def reply(self, code, text):
        b = (text + "\n").encode()
        self.send_response(code)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        if self.path == "/status":
            return self.reply(200, status_line())
        self.reply(404, "not found: GET /status, POST /use/elephant, POST /use/gptoss, POST /stop")

    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        if n:
            self.rfile.read(min(n, 4096))
        if self.path.startswith("/use/") and self.path[5:] in MODELS:
            return self.reply(200, ask(self.path[5:]))
        if self.path == "/stop":
            return self.reply(200, ask(None))
        self.reply(404, "not found: GET /status, POST /use/elephant, POST /use/gptoss, POST /stop")

    def log_message(self, fmt, *args):
        pass


def main():
    global want
    try:
        with open(STATE_FILE) as f:
            last = f.read().strip()
        if last in MODELS:
            want = last
    except OSError:
        pass

    def quit(*_):
        with lock:
            stop_ours()
        os._exit(0)

    signal.signal(signal.SIGTERM, quit)
    signal.signal(signal.SIGINT, quit)
    threading.Thread(target=worker, daemon=True).start()
    with lock:
        lock.notify_all()
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    log(f"GPU switch on 127.0.0.1:{PORT}, last model {want or '-'}")
    srv.serve_forever()


if __name__ == "__main__":
    main()

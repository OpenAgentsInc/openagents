#!/usr/bin/env python3
"""The GPU switch (psionic/platform/gpu-switch.py) against two stand-in
servers: a /use for the model already loaded answers "ready" at once and
starts nothing, a swap stops one and loads the other, polls while loading
restart nothing, and the log names each request that changed the card.

    python3 -I psionic/tests/gpu_switch_test.py
"""
import importlib.util
import io
import os
import socket
import sys
import tempfile
import threading
import time
import contextlib
import http.server

HERE = os.path.dirname(os.path.abspath(__file__))


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def load_switch(tmp):
    spec = importlib.util.spec_from_file_location("gpu_switch", os.path.join(HERE, "..", "platform", "gpu-switch.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    m.STATE_FILE = os.path.join(tmp, "state")
    # a stand-in server: binds its port after a short "load", as both real ones do
    for name in ("elephant", "gptoss"):
        port = free_port()
        log = os.path.join(tmp, name + ".log")
        m.MODELS[name] = {
            "port": port, "cwd": tmp, "env": {}, "log": log,
            "cmd": [sys.executable, "-I", "-c",
                    "import socket,time,sys\n"
                    "open(sys.argv[2],'a').write('stand-in up\\n')\n"
                    "time.sleep(0.6)\n"
                    "s=socket.socket();s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)\n"
                    "s.bind(('127.0.0.1',int(sys.argv[1])));s.listen(8)\n"
                    "while True: time.sleep(1)\n", str(port), log],
        }
    return m


def starts(m, name):
    try:
        with open(m.MODELS[name]["log"]) as f:
            return f.read().count("stand-in up")
    except OSError:
        return 0


def wait_ready(m, name, limit=20.0):
    end = time.time() + limit
    while time.time() < end:
        line = m.status_line()
        if line.startswith(f"ready {name}") or line.startswith("failed"):
            return line
        time.sleep(0.05)
    return m.status_line()


def main():
    tmp = tempfile.mkdtemp(prefix="gpu-switch-test.")
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        m = load_switch(tmp)
        threading.Thread(target=m.worker, daemon=True).start()
        try:
            # a cold card loads what is asked for
            first = m.ask("gptoss")
            assert first.startswith("loading gptoss"), first
            # the app polls /use while it loads: nothing restarts
            for _ in range(5):
                assert m.ask("gptoss").split()[1] == "gptoss"
            assert wait_ready(m, "gptoss").startswith("ready gptoss"), m.status_line()
            assert starts(m, "gptoss") == 1, starts(m, "gptoss")

            # already loaded: "ready" at once, no second start, no stop
            t0 = time.time()
            again = m.ask("gptoss")
            assert again.startswith("ready gptoss"), again
            assert time.time() - t0 < 0.5
            time.sleep(0.8)
            assert starts(m, "gptoss") == 1 and m.status_line().startswith("ready gptoss")

            # a swap: gpt-oss stops, Elephant loads; back again
            assert m.ask("elephant").split()[0] in ("loading", "ready")
            assert wait_ready(m, "elephant").startswith("ready elephant"), m.status_line()
            assert m.ask("gptoss").split()[0] == "loading"
            assert wait_ready(m, "gptoss").startswith("ready gptoss")
            assert starts(m, "gptoss") == 2 and starts(m, "elephant") == 1

            # a restart (/stop, then /use) is a full load, and the log says who asked
            m.ask(None)
            time.sleep(0.3)
            m.ask("gptoss")
            assert wait_ready(m, "gptoss").startswith("ready gptoss")
            assert starts(m, "gptoss") == 3
        finally:
            with m.lock:
                m.stop_ours()
    log = out.getvalue()
    for want in ("asked gptoss (was stopped -)", "asked gptoss: ready", "asked elephant (was ready gptoss)",
                 "asked stop (was ready gptoss)", "stopping gptoss", "stopped -"):
        assert want in log, (want, log)
    # polls while loading log nothing
    assert log.count("asked gptoss: ready") == 1, log
    print("ok   the GPU switch: already loaded is ready at once; swaps, polls and restarts as logged")


if __name__ == "__main__":
    main()

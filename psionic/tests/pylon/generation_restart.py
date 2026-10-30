"""Real local Coax relay/Pylon/fleet restart qualification; no public relay.

Use existing qualified toy/fleet executables; only the changed Pylon needs a
new build. Example: python3 -I generation_restart.py PYLON RELAY SERVE FLEET OUT
"""
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
IDENTITY = "psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573"


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_port(port, process):
    end = time.monotonic() + 20
    while time.monotonic() < end:
        assert process.poll() is None, "service exited before listening"
        try:
            with socket.create_connection(("127.0.0.1", port), .2):
                return
        except OSError:
            time.sleep(.05)
    raise AssertionError("service did not listen")


def wait_online(path, process):
    end = time.monotonic() + 40
    while time.monotonic() < end:
        text = path.read_text()
        if "online, waiting for jobs" in text:
            return text
        assert process.poll() is None, text
        time.sleep(.05)
    raise AssertionError(path.read_text())


def stop(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def main():
    pylon, relay, serve, fleet, output = map(Path, sys.argv[1:])
    output.mkdir(parents=True, exist_ok=False)
    state = output / "state"
    state.mkdir(mode=0o700)
    label = output / "demo-label"
    label.write_text(os.urandom(32).hex())
    label.chmod(0o600)
    counter = state / "identity.generation"
    logs, processes = [], []

    def spawn(name, arguments):
        log = output / (name + ".log")
        handle = log.open("w")
        logs.append(handle)
        process = subprocess.Popen(list(map(str, arguments)), stdout=handle, stderr=subprocess.STDOUT)
        processes.append(process)
        return process, log

    def decision(port):
        body = {"model": "psionic-toy", "state": "Water is wet.", "questions": {
            "wet": {"type": "choice", "instructions": "Is the water wet?", "criteria": {"yes": None, "no": None}}}}
        request = urllib.request.Request("http://127.0.0.1:%d/v1/systemone" % port,
            data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=10) as response:
            result = json.load(response)
        assert result["service"]["identity"] == IDENTITY, result
        assert result["answers"]["wet"]["choice"] == "no", result
        return result

    try:
        rport, eport, fport = free_port(), free_port(), free_port()
        url = "ws://localhost:%d" % rport
        r, _ = spawn("relay", [relay, "--listen", "127.0.0.1", "--port", rport, "--url", url, "--store", output / "relay.store"])
        wait_port(rport, r)
        engine, _ = spawn("engine", [serve, "--toy", "--port", eport])
        wait_port(eport, engine)
        args = [pylon, "--relay", url, "--engine", "http://localhost:%d" % eport,
            "--model", "psionic-toy", "--slug", "generation-local", "--test-key-label-file", label,
            "--references", ROOT / "psionic/fixtures/pylon/psionic-toy.selfcheck", "--identity-state", state / "identity"]
        first, first_log = spawn("first", args)
        text = wait_online(first_log, first)
        assert "beacon online generation 2 " in text, text
        provider = re.search(r"pubkey ([0-9a-f]{64})", text).group(1)
        generation = counter.read_text().splitlines()
        assert generation == ["psionic.pylon-generation.v1", provider, "2"], generation
        client, _ = spawn("fleet", [fleet, "--identity", IDENTITY, "--relay", url, "--port", fport,
            "--timeout-ms", 6000, "--attempt-ms", 3000, "--discover-ms", 750])
        wait_port(fport, client)
        before = decision(fport)
        stop(first)
        assert first.returncode == 0, first_log.read_text()
        second, second_log = spawn("second", args)
        text = wait_online(second_log, second)
        assert "beacon online generation 3 " in text, text
        assert re.search(r"pubkey ([0-9a-f]{64})", text).group(1) == provider
        after = decision(fport)  # same live fleet retains the old provider epoch
        duplicate, duplicate_log = spawn("duplicate", args)
        duplicate.wait(15)
        assert duplicate.returncode == 6 and "generation state already in use" in duplicate_log.read_text()
        assert "connection 1 open" not in duplicate_log.read_text()
        assert counter.read_text().splitlines()[-1] == "3"
        stop(second)
        saved = counter.read_text()
        counter.write_text("broken\n")
        corrupt, corrupt_log = spawn("corrupt", args)
        corrupt.wait(15)
        assert corrupt.returncode == 6 and "generation state refused" in corrupt_log.read_text()
        assert "connection 1 open" not in corrupt_log.read_text()
        counter.write_text(saved)
        state.chmod(0o500)
        try:
            refused, refused_log = spawn("unwritable", args)
            refused.wait(15)
            assert refused.returncode == 6 and "not durably committed" in refused_log.read_text()
            assert "connection 1 open" not in refused_log.read_text()
        finally:
            state.chmod(0o700)
        assert counter.read_text() == saved
        (output / "result.json").write_text(json.dumps({"provider": provider, "epochs": [2, 3],
            "same_fleet_process": True, "before": before, "after": after,
            "duplicate_refused": True, "corrupt_refused": True, "unwritable_refused": True}, indent=2) + "\n")
        print("local encrypted task succeeds before/after restart with one retained native fleet")
        print("same provider advances 2→3; duplicate, corrupt and uncommitted generations never advertise")
    finally:
        state.chmod(0o700)
        for process in reversed(processes):
            stop(process)
        for handle in logs:
            handle.close()
        for path in output.glob("*.log"):
            text = path.read_text(errors="replace")
            assert "ERROR: AddressSanitizer" not in text and "runtime error:" not in text, text


if __name__ == "__main__":
    main()

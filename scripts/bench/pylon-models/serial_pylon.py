"""Serial Pylon CLI qualification using the existing private relay fixture."""

from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import urllib.error
import urllib.request


@contextmanager
def recording_engine(api):
    """Forward real HTTP requests and retain exact model input/output."""
    records = []
    state = {"active": 0, "maximum_active": 0}
    lock = threading.Lock()

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_GET(self):
            self.forward()

        def do_POST(self):
            self.forward()

        def forward(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
            generation = self.path == "/v1/chat/completions"
            row = {"path": self.path, "request": json.loads(body) if body else None}
            started = time.perf_counter()
            if generation:
                with lock:
                    state["active"] += 1
                    state["maximum_active"] = max(state["maximum_active"], state["active"])
            try:
                request = urllib.request.Request(api.rstrip("/") + self.path,
                    data=body if self.command == "POST" else None,
                    headers={"Content-Type": "application/json"})
                try:
                    with urllib.request.urlopen(request, timeout=120) as response:
                        status, data = response.status, response.read()
                except urllib.error.HTTPError as error:
                    status, data = error.code, error.read()
                row.update(status=status, response=json.loads(data))
            except (OSError, ValueError) as error:
                status = 502
                data = json.dumps({"error": {"message": "qualification backend unavailable"}}).encode()
                row.update(status=status, error=str(error))
            finally:
                if generation:
                    with lock:
                        state["active"] -= 1
                        row["wall_ms"] = round((time.perf_counter() - started) * 1000, 3)
                        records.append(row)
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            try:
                self.wfile.write(data)
            except (BrokenPipeError, ConnectionResetError):
                pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}", records, state
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


def qualify(args, row, folder, cli, stop_process, boxed_check):
    folder.mkdir(parents=True, exist_ok=True)
    result = {"scope": "serial_pylon_transport", "qualified": False, "checks": [],
              "sampling": {"temperature": 0.6, "top_p": 0.95, "seed": 1},
              "provider_slots": 1, "provider_deadline_seconds": 90,
              "model_http_timeout_seconds": 120, "model": row}
    provider = relay = None
    provider_logs = []
    home = folder / "pylon-home"
    report = folder / "results.json"

    def save():
        report.write_text(json.dumps(result, indent=2))

    def record(name, detail, passed):
        result["checks"].append({"name": name, "passed": bool(passed), **detail})
        save()
        print(json.dumps({"event": "serial-pylon-check", "name": name, "passed": bool(passed)}), flush=True)
        if not passed:
            raise RuntimeError("Serial Pylon check failed: " + name)

    try:
        relay = subprocess.Popen([str(args.fixture_relay_bin), "1800"], stdout=subprocess.PIPE,
                                 stderr=(folder / "relay.log").open("w"), text=True, start_new_session=True)
        relay_url = relay.stdout.readline().strip()
        if not relay_url.startswith("ws://127.0.0.1:"):
            raise RuntimeError("The fixture relay did not return a private loopback URL")
        result["relay"] = relay_url
        identity = cli(args.pylon_bin, home, ["whoami"])["response"]["data"]
        with recording_engine(args.serial_pylon_api) as (proxy, exchanges, state):
            result["http_exchanges"] = exchanges
            result["http_concurrency"] = state

            def launch(max_tokens=2048, stop=None):
                nonlocal provider
                stop_process(provider)
                # Give the next beacon a later timestamp than the offline one.
                time.sleep(1.1)
                command = [str(args.pylon_bin), "serve", "--engine", proxy, "--model", row["id"],
                           "--pylon", "ds0-serial", "--allow", identity["buyer"], "--slots", "1",
                           "--rate", "600", "--max-tokens", str(max_tokens), "--dedicated",
                           "--temperature", "0.6", "--top-p", "0.95", "--seed", "1",
                           "--relay", relay_url, "--json"]
                if stop:
                    command += ["--stop", stop]
                log = (folder / f"provider-{len(provider_logs)}.log").open("w")
                provider_logs.append(log)
                provider = subprocess.Popen(command, env=dict(os.environ, OPENAGENTS_PYLON_HOME=str(home)),
                                            stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                result.setdefault("provider_commands", []).append(command)
                time.sleep(2)
                if provider.poll() is not None:
                    raise RuntimeError("The serial provider exited; see its log")

            instruction = "Please reason step by step, and put your final answer within \\boxed{}."
            prompt = "What is 1 + 1? " + instruction

            def ask(text=prompt, history=None, wait=118):
                command = ["ask", text, "--pylon", identity["provider"], "--wait", str(wait), "--relay", relay_url]
                if history:
                    command += ["--history", str(history)]
                return cli(args.pylon_bin, home, command, timeout=wait + 15)

            def correct(answer, expected):
                data = answer["response"].get("data", {})
                text = data.get("text")
                matched = any(boxed_check(exchange.get("response", {}), expected)
                    and exchange["response"]["choices"][0]["message"]["content"] == text
                    for exchange in exchanges)
                return (answer["exit_code"] == 0 and matched
                        and data.get("receipt") and not data.get("receipt_error"))

            launch()
            cold = ask()
            record("cold_answer", {"cli": cold}, correct(cold, "2"))
            first_text = cold["response"]["data"]["text"]
            first_exchange = exchanges[-1]
            record("answer_bytes", {}, first_text == first_exchange["response"]["choices"][0]["message"]["content"])
            history = folder / "history.json"
            history.write_text(json.dumps([{"role": "user", "content": prompt},
                                           {"role": "assistant", "content": first_text}]))
            continuation = ask("Add one to your previous answer. " + instruction, history)
            record("continuation", {"cli": continuation}, correct(continuation, "3")
                   and exchanges[-1]["request"]["messages"][1]["content"] == first_text)
            for index in range(9):
                answer = ask()
                record(f"warm_answer_{index + 2}", {"cli": answer}, correct(answer, "2"))

            with ThreadPoolExecutor(max_workers=2) as pool:
                answers = list(pool.map(lambda _: ask(), range(2)))
            record("overlap", {"cli": answers}, state["maximum_active"] == 1
                   and any(correct(answer, "2") for answer in answers)
                   and all(correct(answer, "2") or answer["response"].get("data", {}).get("error")
                           == "rate_limited: no free slot" for answer in answers))

            # Expiring the buyer's wait does not cancel the provider's model job.
            # Start with a fresh free-slot beacon: busy beacons refresh at a
            # slower cadence than this deliberately short buyer deadline.
            launch()
            long_prompt = "List the integers from 1 to 300 in your reasoning before computing their sum. " + instruction
            expired = ask(long_prompt, wait=1)
            record("buyer_wait_expired", {"cli": expired, "http_active_when_buyer_left": state["active"]},
                   expired["exit_code"] != 0 and state["active"] == 1)
            after_expiry = ask()
            record("request_while_previous_job_runs", {"cli": after_expiry},
                   correct(after_expiry, "2") or after_expiry["response"].get("data", {}).get("error")
                   == "rate_limited: no free slot")
            deadline = time.monotonic() + 90
            while state["active"] and time.monotonic() < deadline:
                time.sleep(0.05)
            record("timed_out_buyers_job_finished", {}, state["active"] == 0)
            time.sleep(0.2)
            after_expiry = ask()
            record("after_buyer_timeout", {"cli": after_expiry}, correct(after_expiry, "2")
                   and state["maximum_active"] == 1)

            launch(max_tokens=1)
            truncated = ask()
            record("truncation_refused", {"cli": truncated}, truncated["exit_code"] != 0
                   and exchanges[-1].get("response", {}).get("choices", [{}])[0].get("finish_reason") == "length")
            launch(stop="</think>")
            reasoning_only = ask()
            record("reasoning_only_refused", {"cli": reasoning_only}, reasoning_only["exit_code"] != 0
                   and not exchanges[-1]["response"]["choices"][0]["message"].get("content"))

            if args.serial_engine_control:
                launch()
                with ThreadPoolExecutor(max_workers=1) as pool:
                    pending = pool.submit(ask, long_prompt)
                    deadline = time.monotonic() + 10
                    while not state["active"] and not pending.done() and time.monotonic() < deadline:
                        time.sleep(0.01)
                    record("backend_request_in_flight", {}, state["active"] == 1)
                    stopped = subprocess.run([str(args.serial_engine_control), "crash"], capture_output=True, text=True, timeout=30)
                    record("backend_crash", {"control": stopped.stdout, "stderr": stopped.stderr}, stopped.returncode == 0)
                    interrupted = pending.result(timeout=30)
                record("interrupted_job_failed", {"cli": interrupted}, interrupted["exit_code"] != 0
                       and exchanges[-1]["status"] == 502)
                unavailable = ask(wait=5)
                record("backend_unavailable", {"cli": unavailable}, unavailable["exit_code"] != 0)
                started = subprocess.run([str(args.serial_engine_control), "start"], capture_output=True, text=True, timeout=120)
                record("backend_restart", {"control": started.stdout, "stderr": started.stderr}, started.returncode == 0)
                launch()
                recovered = ask()
                record("after_backend_restart", {"cli": recovered}, correct(recovered, "2"))

            requests = [exchange["request"] for exchange in exchanges]
            record("explicit_sampling", {}, bool(requests) and all(
                all(request.get(key) == value for key, value in result["sampling"].items())
                and request.get("stream") is False and request.get("n") == 1 for request in requests))
            record("serial_http_execution", {}, state["maximum_active"] == 1 and state["active"] == 0)
            result["qualified"] = True
    except Exception as error:
        result["error"] = str(error)
    finally:
        stop_process(provider)
        stop_process(relay)
        for log in provider_logs:
            log.close()
        save()
    return result

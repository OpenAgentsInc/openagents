"""Serial Pylon CLI qualification using the existing private relay fixture."""

from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import signal
import re
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


def qualify_recovery(args, row, folder, cli, stop_process):
    """Run real buyer jobs while Pylon automatically replaces its GPU child."""
    folder.mkdir(parents=True, exist_ok=True)
    home = folder / "pylon-home"
    result = {"scope": "automatic_serial_pylon_recovery", "model": row, "qualified": False, "checks": []}
    via_up = args.serial_pylon_up
    if via_up:
        result["scope"] = "managed_deepseek_pylon_up"
    report = folder / "results.json"
    relay = provider = None
    logs = []

    def record(name, passed, **detail):
        result["checks"].append({"name": name, "passed": bool(passed), **detail})
        report.write_text(json.dumps(result, indent=2))
        print(json.dumps({"event": "automatic-recovery-check", "name": name, "passed": bool(passed)}), flush=True)
        if not passed:
            raise RuntimeError("Automatic recovery check failed: " + name)

    def live():
        try:
            return json.loads((home / "live.json").read_text())
        except (OSError, ValueError):
            return {}

    def wait(predicate, seconds=120):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            state = live()
            if predicate(state):
                return state
            if provider.poll() is not None:
                raise RuntimeError("The managed provider stopped; see provider.log")
            time.sleep(0.05)
        raise RuntimeError("The managed worker did not reach the expected state")

    def ready(old=None):
        return wait(lambda s: s.get("state") == "serving" and any(
            lane.get("up") and lane.get("pid") and lane["pid"] != old
            and not lane.get("generation_busy") for lane in s.get("lanes", [])))

    def owned(state):
        lane = state["lanes"][0]
        pid = lane["pid"]
        status = Path(f"/proc/{pid}/status").read_text()
        parent = int(re.search(r"^PPid:\s*(\d+)", status, re.MULTILINE)[1])
        if parent != provider.pid or os.getpgid(pid) != pid:
            raise RuntimeError("Refusing to signal a worker not owned by this provider")
        gpu = subprocess.check_output(["nvidia-smi", "--query-compute-apps=pid,used_memory",
                                       "--format=csv,noheader,nounits"], text=True)
        memory = next((int(line.split(",")[1]) for line in gpu.splitlines()
                       if line.split(",")[0].strip() == str(pid)), 0)
        return {"pid": pid, "parent": parent, "process_group": os.getpgid(pid),
                "start_ticks": Path(f"/proc/{pid}/stat").read_text().split()[21],
                "command": Path(f"/proc/{pid}/cmdline").read_bytes().decode().split("\0")[:-1],
                "gpu_mib": memory}

    def correct(answer, number):
        data = answer["response"].get("data", {})
        text = data.get("text") or ""
        return (answer["exit_code"] == 0 and re.findall(r"\\boxed\{([+-]?\d+)\}", text) == [number]
                and not any(marker in text for marker in ("<think>", "</think>", "<｜end▁of▁sentence｜>"))
                and data.get("receipt") and not data.get("receipt_error"))

    try:
        if os.name != "posix" or not Path("/proc").is_dir():
            raise RuntimeError("The GPU recovery probe requires a Linux host")
        relay_log = (folder / "relay.log").open("w")
        logs.append(relay_log)
        relay = subprocess.Popen([str(args.fixture_relay_bin), "900"], stdout=subprocess.PIPE,
                                 stderr=relay_log, text=True, start_new_session=True)
        relay_url = relay.stdout.readline().strip()
        if not relay_url.startswith("ws://127.0.0.1:"):
            raise RuntimeError("The qualification relay is not on loopback")
        identity = cli(args.pylon_bin, home, ["whoami"])["response"]["data"]
        command = [str(args.pylon_bin), "serve", "--engine", args.serial_pylon_api,
                   "--managed-llama", str(args.serial_managed_llama_bin),
                   "--model-file", str(args.model_dir / row["id"] / row["file"]),
                   "--model", row["id"], "--slots", "1", "--max-tokens", "2048", "--dedicated",
                   "--temperature", "0.6", "--top-p", "0.95", "--seed", "1", "--generation-seconds", "10",
                   "--startup-seconds", "120", "--restart-limit", "3", "--allow", identity["buyer"],
                   "--rate", "600", "--relay", relay_url, "--json"]
        if via_up:
            from urllib.parse import urlsplit
            command = [str(args.pylon_bin), "up", "--foreground", "--text-profile", row["id"],
                       "--managed-llama", str(args.serial_managed_llama_bin),
                       "--text-model", str(args.model_dir / row["id"] / row["file"]),
                       "--text-port", str(urlsplit(args.serial_pylon_api).port), "--dedicated",
                       "--generation-seconds", "10", "--startup-seconds", "120", "--restart-limit", "3",
                       "--allow", identity["buyer"], "--rate", "600", "--relay", relay_url, "--json"]
        result["provider_command"] = command
        log = (folder / "provider.log").open("w")
        logs.append(log)
        provider = subprocess.Popen(command, env=dict(os.environ, OPENAGENTS_PYLON_HOME=str(home)),
                                    stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        started = time.perf_counter()
        state = ready()
        process = owned(state)
        record("owned_gpu_ready", process["gpu_mib"] > 0, process=process, state=state,
               startup_ms=round((time.perf_counter() - started) * 1000, 3))
        if via_up:
            saved = json.loads((home / "share.json").read_text())
            record("saved_text_only_profile", saved["decide"] is None and saved["text"]["profile"] == row["id"]
                   and saved["slots"] == 1 and saved["max_tokens"] == 2048
                   and saved["text"]["serial"]["sampling"] == {"temperature": 0.6, "top_p": 0.95, "seed": 1}, settings=saved)
            status = cli(args.pylon_bin, home, ["status", "--relay", relay_url])
            data = status["response"].get("data", {})
            record("status_serving", status["exit_code"] == 0 and data.get("state") == "serving"
                   and data.get("model") == row["id"] and data.get("device") == "cuda"
                   and len(data.get("engines", [])) == 1, cli=status)
        instruction = "Please reason step by step, and put your final answer within \\boxed{}."
        prompt = "What is 1 + 1? " + instruction

        def ask(text=prompt, history=None):
            command = ["ask", text, "--pylon", identity["provider"], "--wait", "118", "--relay", relay_url]
            if history:
                command += ["--history", str(history)]
            return cli(args.pylon_bin, home, command, timeout=133)

        first = ask()
        record("cold_answer", correct(first, "2"), cli=first)
        history = folder / "history.json"
        history.write_text(json.dumps([{"role": "user", "content": prompt},
                                       {"role": "assistant", "content": first["response"]["data"]["text"]}]))
        continued = ask("Add one to your previous answer. " + instruction, history)
        record("continuation", correct(continued, "3"), cli=continued)
        if via_up:
            warm = [ask() for _ in range(10)]
            record("ten_warm_answers", all(correct(answer, "2") for answer in warm), cli=warm)

        for name, sig in [("crash", signal.SIGKILL), ("stopped_worker", signal.SIGSTOP)]:
            # Only the test's fault is injected; every replacement is started
            # by the production supervisor, with no external restart helper.
            state = ready()
            original = owned(state)
            with ThreadPoolExecutor(max_workers=1) as pool:
                pending = pool.submit(ask, "List the integers from 1 to 300 in your reasoning before computing their sum. " + instruction)
                active = wait(lambda s: any(lane.get("pid") == original["pid"]
                    and lane.get("generation_busy") for lane in s.get("lanes", [])), seconds=15)
                checked = owned(active)
                if checked["start_ticks"] != original["start_ticks"]:
                    raise RuntimeError("The worker changed before fault injection")
                record(name + "_request_active", True, state=active, process=checked)
                os.kill(checked["pid"], sig)
                failed = pending.result(timeout=25)
            record(name + "_job_failed", failed["exit_code"] != 0
                   and failed["response"].get("data", {}).get("text") is None
                   and failed["response"].get("data", {}).get("outcome") != "timeout", cli=failed)
            started = time.perf_counter()
            restarted = ready(original["pid"])
            current = owned(restarted)
            record(name + "_automatic_restart", not Path(f"/proc/{original['pid']}").exists()
                   and current["gpu_mib"] > 0, state=restarted, process=current,
                   ready_after_failed_job_ms=round((time.perf_counter() - started) * 1000, 3))
            answered = ask()
            record(name + "_next_job", correct(answered, "2"), cli=answered)

        # This runtime suppresses layer-loading details at its default log
        # level. Verify GPU allocation for each owned process through NVML.
        workers = [check["process"] for check in result["checks"]
                   if check["name"] in ("owned_gpu_ready", "crash_automatic_restart",
                                        "stopped_worker_automatic_restart")]
        record("all_replacements_use_gpu", len({worker["pid"] for worker in workers}) == 3
               and all(worker["gpu_mib"] > 0 for worker in workers), processes=workers)
        if via_up:
            down = cli(args.pylon_bin, home, ["down", "--relay", relay_url])
            provider.wait(timeout=15)
            record("down", down["exit_code"] == 0 and not Path(f"/proc/{current['pid']}").exists()
                   and not json.loads((home / "share.json").read_text())["on"], cli=down, state=live())
            command = [str(args.pylon_bin), "up", "--foreground", "--relay", relay_url, "--json"]
            result["resume_command"] = command
            provider = subprocess.Popen(command, env=dict(os.environ, OPENAGENTS_PYLON_HOME=str(home)),
                                        stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            state = ready(current["pid"])
            current = owned(state)
            record("saved_profile_resumed", current["gpu_mib"] > 0
                   and json.loads((home / "share.json").read_text())["decide"] is None, state=state, process=current)
            resumed = ask()
            record("answer_after_up", correct(resumed, "2"), cli=resumed)
            down = cli(args.pylon_bin, home, ["down", "--relay", relay_url])
            provider.wait(timeout=15)
            record("final_down", down["exit_code"] == 0, cli=down)
        else:
            stop_process(provider)
        record("shutdown", not Path(f"/proc/{current['pid']}").exists()
               and live().get("state") == "off" and not json.loads((home / "share.json").read_text())["on"], state=live())
        result["qualified"] = True
    except Exception as error:
        result["error"] = str(error)
    finally:
        stop_process(provider)
        stop_process(relay)
        for log in logs:
            log.close()
        report.write_text(json.dumps(result, indent=2))
    return result

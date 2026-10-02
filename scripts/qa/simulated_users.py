#!/usr/bin/env python3
"""The simulated-user QA run: pretend people use every OpenAgents surface.

docs/qa/simulated-users.md is the runbook; scripts/qa/simulated-users.sh
runs this with the Python and packages it needs.

Each persona in qa/personas/*.toml drives one surface through the real
product (the website over HTTP, `openagents chat`, `openagents terminal` in
a pseudo-terminal, the installer into a temporary HOME, and the desktop
window and the phone's chat through the release gate's driver). A model
plays the person: each next message reacts to what the product just said.
A second model call judges the transcript against qa/rubric.md and
qa/facts.md. Findings land in OUT/findings.json and OUT/report.md, and
`--file` turns the ones you confirmed into GitHub issues labeled `qa`,
after checking open issues for duplicates.

Everything runs under temporary homes and scratch identities. Nothing
reads or writes the real home's stores, chats, wallet, or paired devices,
and no key is printed.
"""

from __future__ import annotations

import argparse
import difflib
import fcntl
import hashlib
import http.cookiejar
import json
import os
import pty
import re
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
QA = ROOT / "qa"
SITE = os.environ.get("OPENAGENTS_QA_SITE", "https://openagents.com")
INSTALL_URL = (
    "https://storage.googleapis.com/openagentsgemini-cli-releases/openagents/install.sh"
)
OPENROUTER = "https://openrouter.ai/api/v1/chat/completions"
DEFAULT_MODEL = "stealth/space-bunny-alpha"
REPO = "OpenAgentsInc/openagents"


# Accounting -----------------------------------------------------------------


class Budget:
    """Live chat-worker jobs this run sent, and what the models cost."""

    def __init__(self, max_jobs: int):
        self.max_jobs = max_jobs
        self.jobs = 0
        self.model_usd = 0.0
        self.model_calls = 0

    def take(self) -> bool:
        if self.jobs >= self.max_jobs:
            return False
        self.jobs += 1
        return True


BUDGET = Budget(300)


def log(*parts: object) -> None:
    print("qa:", *parts, file=sys.stderr, flush=True)


# Models ----------------------------------------------------------------------


def ask_model(system: str, user: str, model: str, temperature: float) -> str:
    """One completion: OpenRouter for `stealth/…`-style ids, the Claude CLI for
    `claude:MODEL`. Raises on failure."""
    BUDGET.model_calls += 1
    if model.startswith("claude:"):
        claude = shutil.which("claude")
        if not claude:
            raise RuntimeError("the claude CLI is not installed")
        done = subprocess.run(
            [
                claude,
                "-p",
                "--model",
                model.split(":", 1)[1],
                "--no-session-persistence",
                "--output-format",
                "json",
                "--append-system-prompt",
                system,
            ],
            input=user,
            capture_output=True,
            text=True,
            timeout=300,
        )
        data = json.loads(done.stdout or "{}")
        BUDGET.model_usd += float(data.get("total_cost_usd") or 0)
        if data.get("is_error") or "result" not in data:
            raise RuntimeError(f"claude: {done.stderr[-300:] or data}")
        return data["result"]
    key = os.environ.get("OPENROUTER_API_KEY", "").strip()
    if not key:
        raise RuntimeError("OPENROUTER_API_KEY is not set")
    body = {
        "model": model,
        "temperature": temperature,
        "max_tokens": 16000,
        "reasoning": {"effort": "low"},
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user},
        ],
    }
    last = None
    for attempt in range(3):
        request = urllib.request.Request(
            OPENROUTER,
            data=json.dumps(body).encode(),
            headers={
                "Authorization": f"Bearer {key}",
                "Content-Type": "application/json",
                "X-Title": "OpenAgents simulated-user QA",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=180) as response:
                data = json.load(response)
            BUDGET.model_usd += float((data.get("usage") or {}).get("cost") or 0)
            text = data["choices"][0]["message"].get("content") or ""
            if text.strip():
                return text
            last = "an empty completion"
        except (urllib.error.URLError, KeyError, TimeoutError, json.JSONDecodeError) as e:
            last = str(e)
        time.sleep(2 + attempt * 3)
    raise RuntimeError(f"the model did not answer: {last}")


def json_in(text: str) -> dict:
    """The first JSON object in a model's text."""
    text = re.sub(r"^```(?:json)?|```$", "", text.strip(), flags=re.M)
    start = text.find("{")
    while start != -1:
        depth = 0
        in_string = False
        escaped = False
        for i in range(start, len(text)):
            c = text[i]
            if in_string:
                if escaped:
                    escaped = False
                elif c == "\\":
                    escaped = True
                elif c == '"':
                    in_string = False
            elif c == '"':
                in_string = True
            elif c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    try:
                        return json.loads(text[start : i + 1])
                    except json.JSONDecodeError:
                        break
        start = text.find("{", start + 1)
    raise ValueError(f"no JSON object in: {text[:200]!r}")


def model_json(system: str, user: str, model: str, temperature: float) -> dict:
    """A JSON answer from `model`, or from the fallback model
    (OPENAGENTS_QA_FALLBACK_MODEL, default the Claude CLI's Haiku) when it
    gives none."""
    last = None
    fallback = os.environ.get("OPENAGENTS_QA_FALLBACK_MODEL", "claude:haiku")
    for attempt in (model, model, fallback):
        try:
            began = time.monotonic()
            answer = json_in(ask_model(system, user, attempt, temperature))
            log(f"model {attempt}: {time.monotonic() - began:.1f}s")
            return answer
        except (ValueError, RuntimeError) as e:
            last = e
            log(f"model {attempt} failed: {str(e)[:200]}")
    raise RuntimeError(f"no usable JSON from {model} or {fallback}: {last}")


# Transcript -------------------------------------------------------------------


class Transcript:
    def __init__(self, persona: dict, out: Path):
        self.persona = persona
        self.out = out
        self.rows: list[dict] = []
        out.mkdir(parents=True, exist_ok=True)

    def add(self, kind: str, text: str, **extra) -> dict:
        row = {"n": len(self.rows), "kind": kind, "text": text, **extra}
        self.rows.append(row)
        with open(self.out / "transcript.jsonl", "a") as f:
            f.write(json.dumps(row) + "\n")
        shown = text if len(text) < 300 else text[:300] + "…"
        log(f"[{self.persona['id']}] {kind}: {shown!r}" + (f" ({extra.get('ms')} ms)" if "ms" in extra else ""))
        return row

    def as_text(self, limit: int = 3500) -> str:
        lines = []
        for row in self.rows:
            text = row["text"]
            if len(text) > limit:
                text = text[:limit] + " […]"
            meta = []
            if "ms" in row:
                meta.append(f"{row['ms']} ms")
            if row.get("evidence"):
                meta.append(f"evidence {row['evidence']}")
            for key in ("route", "status", "note"):
                if row.get(key) not in (None, "", {}):
                    meta.append(f"{key}={json.dumps(row[key])[:400]}")
            note = f" (harness notes, not shown to the person: {'; '.join(meta)})" if meta else ""
            lines.append(f"#{row['n']} [{row['kind']}]{note}\n{text}")
        return "\n\n".join(lines)


def next_message(persona: dict, transcript: Transcript, model: str) -> dict:
    system = (
        "You are role-playing a person who is using a software product, so its "
        "makers can see how real people experience it. Stay in character.\n\n"
        f"Who you are:\n{persona['persona'].strip()}\n\nYour goals:\n"
        + "\n".join(f"- {g}" for g in persona["goals"])
        + "\n\nWrite this person's next message to the product, reacting to what "
        "the product last said: follow up on anything unclear, wrong, or "
        "unhelpful the way this person would, and move toward the goals you "
        "have not reached. Type it the way this person types: short and "
        "natural, no quotation marks. Set done to true when every goal is met "
        "or this person would give up. Answer with JSON only: "
        '{"message": "...", "done": false, "goals_met": ["..."], "why": "one short sentence"}'
    )
    user = "The conversation so far (newest last):\n\n" + transcript.as_text(1500)
    try:
        answer = model_json(system, user, model, 0.8)
    except RuntimeError as e:
        log(f"persona model failed: {e}")
        return {"message": "", "done": True, "why": str(e)}
    answer["message"] = str(answer.get("message") or "").strip().strip('"')
    return answer


# Drivers ------------------------------------------------------------------------


def strip_html(html: str) -> str:
    html = re.sub(r"(?is)<(script|style)[^>]*>.*?</\1>", " ", html)
    text = re.sub(r"(?s)<[^>]+>", " ", html)
    for a, b in (("&amp;", "&"), ("&lt;", "<"), ("&gt;", ">"), ("&#39;", "'"), ("&quot;", '"'), ("&nbsp;", " ")):
        text = text.replace(a, b)
    return re.sub(r"\s+", " ", text).strip()


class Website:
    """openagents.com over HTTP: its pages and links, then the Ask box."""

    def __init__(self, persona: dict, t: Transcript, opts):
        self.t = t
        jar = http.cookiejar.CookieJar()
        self.http = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))
        self.turns: list[dict] = []

    def get(self, path: str) -> tuple[int, str, int]:
        began = time.monotonic()
        try:
            with self.http.open(SITE + path, timeout=30) as r:
                body = r.read().decode("utf-8", "replace")
                status = r.status
        except urllib.error.HTTPError as e:
            body, status = e.read().decode("utf-8", "replace"), e.code
        except urllib.error.URLError as e:
            body, status = str(e), 0
        return status, body, int((time.monotonic() - began) * 1000)

    def start(self) -> None:
        links: set[str] = set()
        for path in ("/", "/download", "/docs"):
            status, body, ms = self.get(path)
            (self.t.out / f"page{path.replace('/', '_') or '_root'}.html").write_text(body)
            self.t.add("page", f"GET {path} -> {status}\n{strip_html(body)[:2500]}", ms=ms, status=status,
                       evidence=f"page{path.replace('/', '_')}.html")
            for href in re.findall(r'href="(/[^"#?]*)', body):
                links.add(href)
        checked = []
        bad = []
        for href in sorted(links)[:60]:
            if href.startswith("/static/"):
                continue
            status, _, ms = self.get(href)
            checked.append(f"{status} {ms}ms {href}")
            if status != 200 or ms > 3000:
                bad.append(f"{status} {ms}ms {href}")
        self.t.add("links", f"{len(checked)} internal links checked; not OK or slow: {bad or 'none'}",
                   status="ok" if not bad else "bad")
        (self.t.out / "links.txt").write_text("\n".join(checked) + "\n")

    def send(self, text: str) -> dict:
        self.turns.append({"role": "user", "text": text})
        began = time.monotonic()
        first = None
        done = None
        error = None
        request = urllib.request.Request(
            SITE + "/ask",
            data=json.dumps({"turns": self.turns}).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        raw = []
        try:
            with self.http.open(request, timeout=180) as r:
                for line in r:
                    if first is None:
                        first = int((time.monotonic() - began) * 1000)
                    raw.append(line.decode("utf-8", "replace"))
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if event.get("done"):
                        done = event.get("text", "")
                    if event.get("error"):
                        error = event["error"]
        except urllib.error.HTTPError as e:
            error = f"HTTP {e.code}: {e.read().decode('utf-8', 'replace')[:300]}"
        except (urllib.error.URLError, TimeoutError) as e:
            error = str(e)
        ms = int((time.monotonic() - began) * 1000)
        reply = done if done is not None else f"[error] {error or 'no reply'}"
        self.turns.append({"role": "assistant", "text": done or ""})
        return {"reply": reply, "ms": ms, "first_ms": first, "status": "ok" if done is not None else "error"}

    def close(self) -> None:
        pass


def temp_home(prefix: str) -> Path:
    # Short, for control-socket path limits.
    return Path(tempfile.mkdtemp(prefix=prefix, dir="/tmp"))


def scrubbed_env(home: Path) -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("OPENROUTER", "ANTHROPIC", "OPENAI", "XAI"))}
    env.update({"HOME": str(home), "TMPDIR": str(home / "tmp"), "OPENAGENTS_NO_LAUNCH": "1"})
    (home / "tmp").mkdir(exist_ok=True)
    return env


class Chat:
    """`openagents chat send --scratch --json --no-run` in a temporary HOME."""

    def __init__(self, persona: dict, t: Transcript, opts):
        self.t = t
        self.oa = opts.openagents
        self.home = temp_home("oaqa-chat.")
        self.env = scrubbed_env(self.home)
        self.thread = None

    def start(self) -> None:
        pass

    def send(self, text: str) -> dict:
        command = [self.oa, "chat", "send", "--scratch", "--json", "--no-run", "--timeout", "150"]
        if self.thread:
            command += ["--thread", self.thread]
        command.append(text)
        began = time.monotonic()
        first = None
        events = []
        process = subprocess.Popen(command, cwd=self.home, env=self.env, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, text=True)
        for line in process.stdout:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                events.append({"event": "unparsed", "line": line.strip()})
                continue
            if event.get("event") == "partial" and first is None:
                first = int((time.monotonic() - began) * 1000)
            events.append(event)
        stderr = process.stderr.read()
        process.wait()
        ms = int((time.monotonic() - began) * 1000)
        for event in events:
            if event.get("thread"):
                self.thread = event["thread"]
        result = next((e for e in events if e.get("event") == "result"), None)
        route = next((e for e in events if e.get("event") == "route"), {})
        offers = [e for e in events if e.get("event") == "offer"]
        failures = [e for e in events if e.get("event") == "failure"]
        reply = result["text"] if result else f"[no result] {failures or stderr.strip()[-400:]}"
        extras = []
        for key in ("followups", "cards"):
            if route.get(key):
                extras.append(f"{key}: {json.dumps(route[key])[:600]}")
        if offers:
            extras.append(f"offer: {json.dumps(offers)[:600]}")
        shown = reply + ("\n\n[shown with the reply] " + "\n".join(extras) if extras else "")
        with open(self.t.out / "events.jsonl", "a") as f:
            for event in events:
                f.write(json.dumps(event) + "\n")
        return {
            "reply": shown,
            "ms": ms,
            "first_ms": first,
            "route": {k: route.get(k) for k in ("route", "tier", "served_answer") if route.get(k) is not None}
            | {"model": (result or {}).get("model")},
            "status": "ok" if result else "error",
        }

    def close(self) -> None:
        shutil.rmtree(self.home, ignore_errors=True)


class Terminal:
    """`openagents terminal --scratch` in a pseudo-terminal, read through a VT
    emulator; with `install`, the public installer first."""

    ROWS, COLS = 34, 110

    def __init__(self, persona: dict, t: Transcript, opts):
        self.persona = persona
        self.t = t
        self.opts = opts
        self.home = temp_home("oaqa-term.")
        self.env = scrubbed_env(self.home)
        self.env.update({"TERM": "xterm-256color", "LANG": "en_US.UTF-8"})
        self.oa = opts.openagents
        self.pid = None
        self.fd = None
        self.lock = threading.Lock()
        self.changed = time.monotonic()
        self.captures = 0

    def install(self) -> None:
        began = time.monotonic()
        done = subprocess.run(
            ["sh", "-c", f"curl -fsSL {INSTALL_URL} | sh"],
            cwd=self.home, env=self.env, capture_output=True, text=True, timeout=600,
        )
        ms = int((time.monotonic() - began) * 1000)
        output = (done.stdout + done.stderr).strip()
        self.t.add("install", f"$ curl -fsSL …/install.sh | sh  (exit {done.returncode})\n{output[-2500:]}",
                   ms=ms, status="ok" if done.returncode == 0 else "error")
        installed = self.home / ".openagents" / "bin" / "openagents"
        if installed.exists():
            self.oa = str(installed)
            for program in ("openagents", "microcoder"):
                path = self.home / ".openagents" / "bin" / program
                check = subprocess.run([str(path), "--version"], env=self.env, capture_output=True,
                                       text=True, timeout=60)
                self.t.add("check", f"$ {program} --version (exit {check.returncode})\n"
                           f"{(check.stdout + check.stderr).strip()[:500]}",
                           status="ok" if check.returncode == 0 else "error")
            check = subprocess.run([self.oa, "--help"], env=self.env, capture_output=True, text=True, timeout=60)
            self.t.add("check", f"$ openagents --help (exit {check.returncode})\n"
                       f"{(check.stdout + check.stderr).strip()[:2500]}")

    def start(self) -> None:
        import pyte

        if self.persona.get("install"):
            self.install()
        self.screen = pyte.Screen(self.COLS, self.ROWS)
        self.stream = pyte.ByteStream(self.screen)
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(self.home)
            os.execve(self.oa, [self.oa, "terminal", "--scratch"], self.env)
        self.pid, self.fd = pid, fd
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", self.ROWS, self.COLS, 0, 0))
        os.kill(pid, signal.SIGWINCH)
        self.raw = open(self.t.out / "terminal.raw", "wb")
        threading.Thread(target=self.read, daemon=True).start()
        began = time.monotonic()
        self.settle(min_wait=2, stable=2.5, limit=40)
        self.t.add("screen", f"(opened `openagents terminal --scratch`)\n{self.text()}",
                   ms=int((time.monotonic() - began) * 1000), evidence=self.capture())

    def read(self) -> None:
        while True:
            try:
                data = os.read(self.fd, 65536)
            except OSError:
                return
            if not data:
                return
            self.raw.write(data)
            with self.lock:
                self.stream.feed(data)
                # Answer cursor-position and device-attribute queries, as a
                # terminal does.
                if b"\x1b[6n" in data:
                    y, x = self.screen.cursor.y + 1, self.screen.cursor.x + 1
                    os.write(self.fd, f"\x1b[{y};{x}R".encode())
                if b"\x1b[c" in data or b"\x1b[0c" in data:
                    os.write(self.fd, b"\x1b[?62;22c")
                self.changed = time.monotonic()

    def text(self) -> str:
        with self.lock:
            lines = [line.rstrip() for line in self.screen.display]
        while lines and not lines[-1]:
            lines.pop()
        return "\n".join(lines)

    def capture(self) -> str:
        self.captures += 1
        name = f"screen-{self.captures:02}.txt"
        (self.t.out / name).write_text(self.text() + "\n")
        return name

    def settle(self, min_wait: float, stable: float, limit: float) -> None:
        """Waits until the screen's words (not its bytes: a spinner or a
        clock redraws without changing them) held still for `stable`
        seconds, or `limit` passed."""
        began = time.monotonic()
        time.sleep(min_wait)
        last = self.words()
        since = time.monotonic()
        while time.monotonic() - began < limit:
            time.sleep(0.3)
            now = self.words()
            if now != last:
                last, since = now, time.monotonic()
            elif time.monotonic() - since >= stable:
                return

    def words(self) -> str:
        # Spinner glyphs and elapsed-time counters change while nothing
        # else does; leave them out of the comparison.
        text = re.sub(r"[\u2800-\u28ff\u25d0-\u25d3\u2580-\u259f|/\\-]", "", self.text())
        return re.sub(r"\b\d+(\.\d+)?\s?(ms|s|m)\b", "", text)

    def alive(self) -> bool:
        try:
            done, _ = os.waitpid(self.pid, os.WNOHANG)
            return done == 0
        except ChildProcessError:
            return False

    def type(self, text: str) -> None:
        for chunk in (text[i : i + 32] for i in range(0, len(text), 32)):
            os.write(self.fd, chunk.encode())
            time.sleep(0.03)

    def key(self, name: str) -> dict:
        codes = {"{esc}": b"\x1b", "{ctrl-t}": b"\x14", "{ctrl-c}": b"\x03", "{enter}": b"\r"}
        began = time.monotonic()
        os.write(self.fd, codes[name])
        self.settle(min_wait=1, stable=2, limit=20)
        return {"reply": self.text(), "ms": int((time.monotonic() - began) * 1000),
                "evidence": self.capture(), "status": "ok" if self.alive() else "exited"}

    def send(self, text: str, interrupt: bool = False) -> dict:
        began = time.monotonic()
        self.type(text)
        time.sleep(0.2)
        os.write(self.fd, b"\r")
        if interrupt:
            time.sleep(2.0)
            os.write(self.fd, b"\x1b")
            self.settle(min_wait=1, stable=3, limit=30)
        else:
            # A reply has arrived once the screen held still for a few
            # seconds after it started changing.
            self.settle(min_wait=3, stable=6, limit=180)
        return {"reply": self.text(), "ms": int((time.monotonic() - began) * 1000),
                "evidence": self.capture(), "status": "ok" if self.alive() else "exited"}

    def close(self) -> None:
        if self.pid and self.alive():
            for _ in range(2):
                try:
                    os.write(self.fd, b"\x03")
                except OSError:
                    break
                time.sleep(0.5)
            time.sleep(1)
            if self.alive():
                os.kill(self.pid, signal.SIGKILL)
        shutil.rmtree(self.home, ignore_errors=True)


class Gate:
    """The desktop window or the phone's chat, through the release gate's
    driver (`scripts/release/acceptance.sh --only persona`), which runs a
    scratch host in a temporary HOME."""

    def __init__(self, persona: dict, t: Transcript, opts):
        self.persona = persona
        self.t = t
        self.opts = opts
        self.evidence = t.out / "gate"
        self.dir = self.evidence / "persona"
        self.process = None

    def start(self) -> None:
        self.dir.mkdir(parents=True, exist_ok=True)
        (self.dir / "in.jsonl").write_text("")
        env = dict(os.environ)
        for k in list(env):
            if k.startswith(("OPENROUTER",)):
                del env[k]
        env["OPENAGENTS_ACCEPTANCE_PERSONA_SURFACE"] = self.persona["surface"]
        if self.persona.get("seed"):
            env["OPENAGENTS_ACCEPTANCE_SEED"] = str(QA / "seeds" / self.persona["seed"])
        command = [str(ROOT / "scripts/release/acceptance.sh"), "--bin-dir", self.opts.bin_dir,
                   "--evidence", str(self.evidence), "--only", "persona"]
        command += ["--allow-missing-engine"] if self.persona.get("engines") else ["--no-engines"]
        self.log = open(self.t.out / "gate.log", "w")
        self.process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=self.log, stderr=subprocess.STDOUT)
        self.t.add("setup", f"scratch host and the {self.persona['surface']} driver starting "
                   f"(engines: {'yes' if self.persona.get('engines') else 'no'}; seed: {self.persona.get('seed')})")

    def outputs(self) -> list[dict]:
        path = self.dir / "out.jsonl"
        if not path.exists():
            return []
        return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]

    def send(self, text: str) -> dict:
        have = len(self.outputs())
        with open(self.dir / "in.jsonl", "a") as f:
            f.write(json.dumps({"text": text}) + "\n")
        deadline = time.monotonic() + 1500
        while time.monotonic() < deadline:
            out = self.outputs()
            if len(out) > have:
                row = out[have]
                break
            if self.process.poll() is not None:
                return {"reply": f"[the driver exited {self.process.returncode}; see gate.log]",
                        "ms": 0, "status": "error"}
            time.sleep(1)
        else:
            return {"reply": "[no reply within 25 minutes]", "ms": 0, "status": "error"}
        reply = row.get("reply") or f"[error] {row.get('error')}"
        coder = row.get("coder")
        if coder:
            if coder.get("summary"):
                outcome = f"finished in {coder.get('ms', 0) // 1000} s, {coder.get('files_changed')} files changed; Coder's reply: {coder['summary']}"
            else:
                outcome = f"did not finish: {coder.get('failure') or coder.get('asked')}"
            reply += f"\n\n[the screen then showed a Coder run that {outcome}]"
        if row.get("autostart"):
            events = ", ".join(str(e.get("event")) for e in row["autostart"])
            reply += f"\n\n[harness: the computer's Coder journal recorded {events}]"
        if row.get("run_coder_offered"):
            reply += "\n\n[the chat shows a Run Coder button]"
        if row.get("coder_started"):
            reply += "\n\n[the chat shows Coder started, with Stop]"
        return {"reply": reply, "ms": row.get("ms", 0), "status": "error" if row.get("error") else "ok",
                "evidence": "gate/persona/" + str(row.get("capture")), "coder": coder or {}}

    def close(self) -> None:
        if not self.process:
            return
        with open(self.dir / "in.jsonl", "a") as f:
            f.write(json.dumps({"end": True}) + "\n")
        try:
            self.process.wait(timeout=900)
        except subprocess.TimeoutExpired:
            self.process.terminate()
        results = self.evidence / "results.jsonl"
        if results.exists():
            for line in results.read_text().splitlines():
                row = json.loads(line)
                if row.get("status") != "PASS" or row.get("scenario") != "persona":
                    self.t.add("gate", f"{row.get('scenario')}: {row.get('status')} {row.get('detail')}",
                               status=row.get("status"))


DRIVERS = {"website": Website, "chat": Chat, "terminal": Terminal, "desktop": Gate, "phone": Gate}


# A persona's run ----------------------------------------------------------------


def run_persona(persona: dict, out: Path, opts) -> dict:
    t = Transcript(persona, out / persona["id"])
    driver = DRIVERS[persona["surface"]](persona, t, opts)
    sends_jobs = persona["surface"] in ("website", "chat", "desktop", "phone", "terminal")
    try:
        driver.start()
        steps = persona.get("steps") or ["{persona}"] * int(persona.get("max_turns", 4))
        turns = 0
        first = True
        for step in steps:
            if step in ("{persona}", "{persona-esc}"):
                if turns >= int(persona.get("max_turns", 4)):
                    break
                if first and persona.get("opening"):
                    message, first = persona["opening"], False
                else:
                    first = False
                    answer = next_message(persona, t, opts.persona_model)
                    if answer.get("done") or not answer["message"]:
                        t.add("persona", f"(stops: {answer.get('why', '')})")
                        if not persona.get("steps"):
                            break
                        continue
                    message = answer["message"]
                if sends_jobs and not BUDGET.take():
                    t.add("budget", f"stopped: the run's cap of {BUDGET.max_jobs} chat jobs is spent")
                    break
                t.add("user", message)
                if step == "{persona-esc}":
                    result = driver.send(message, interrupt=True)
                    result["note"] = "the person pressed Esc 2 s after sending, on purpose, to stop the reply"
                else:
                    result = driver.send(message)
                turns += 1
            elif step.startswith("{"):
                t.add("user", f"(presses {step.strip('{}')})")
                result = driver.key(step)
            else:
                if sends_jobs and not BUDGET.take():
                    break
                t.add("user", step)
                result = driver.send(step)
            t.add("product", result.pop("reply"), **{k: v for k, v in result.items() if v not in (None, "", {})})
    except Exception as e:  # The run goes on to the next persona.
        t.add("driver-error", f"{type(e).__name__}: {e}")
    finally:
        try:
            driver.close()
        except Exception as e:
            t.add("driver-error", f"close: {e}")
    return judge(persona, t, opts)


def judge(persona: dict, t: Transcript, opts) -> dict:
    system = (
        "You are a strict QA reviewer for OpenAgents. You read a transcript of a "
        "simulated person using one product surface and list real problems a "
        "user would hit. Product turns are what the product showed; user turns "
        "were written by a simulator, so never report problems in them. Report "
        "only problems you can quote from a product turn (or a timing, status, "
        "or a missing reply), each checked against the rubric and the facts. "
        "Lines in square brackets inside a product turn are the harness describing what "
        "the screen showed (buttons, a Coder run), not the product's words; harness notes "
        "are never shown to the person. A Coder run may take minutes; the 20-second rule is "
        "for chat replies. A reply the person stopped with Esc on purpose is not a failure. "
        "Two problems with one cause are one finding. Prefer few, solid "
        "findings to many weak ones.\n\n# Rubric\n" + (QA / "rubric.md").read_text()
        + "\n\n# Facts\n" + (QA / "facts.md").read_text()
        + "\n\nAnswer with JSON only: {\"summary\": \"two sentences on how it went\", "
        "\"goals_met\": [\"...\"], \"goals_missed\": [\"...\"], \"findings\": [{\"title\": "
        "\"short issue title naming the surface and the problem\", \"category\": \"facts|bloat|limits|raw|"
        "routing|latency|tone|stale|broken\", \"severity\": \"high|medium|low\", \"turn\": N, "
        "\"excerpt\": \"exact quote from the product turn\", \"expected\": \"...\", \"actual\": \"...\"}]}"
    )
    user = (
        f"Persona: {persona['name']} on the {persona['surface']} surface.\n"
        f"{persona['persona'].strip()}\nGoals: {persona['goals']}\n\n# Transcript\n\n" + t.as_text()
    )
    try:
        verdict = model_json(system, user, opts.judge_model, 0.0)
    except RuntimeError as e:
        verdict = {"summary": f"the judge failed: {e}", "findings": []}
    findings = []
    for i, finding in enumerate(verdict.get("findings") or []):
        if not isinstance(finding, dict) or not finding.get("title"):
            continue
        finding["id"] = f"{persona['id']}-{i + 1}"
        finding["persona"] = persona["id"]
        finding["persona_name"] = persona["name"]
        finding["surface"] = persona["surface"]
        findings.append(finding)
    verdict["findings"] = findings
    verdict["persona"] = persona["id"]
    verdict["timings_ms"] = [row["ms"] for row in t.rows if row["kind"] == "product" and "ms" in row]
    (t.out / "verdict.json").write_text(json.dumps(verdict, indent=2))
    return verdict


# Issues ---------------------------------------------------------------------------


def qa_key(finding: dict) -> str:
    words = re.sub(r"[^a-z0-9 ]", "", finding["title"].lower()).split()
    return hashlib.sha256(f"{finding['surface']}|{finding.get('category')}|{' '.join(sorted(words))}".encode()).hexdigest()[:12]


def open_issues() -> list[dict]:
    done = subprocess.run(["gh", "issue", "list", "-R", REPO, "--state", "open", "--limit", "400",
                           "--json", "number,title,body,labels"], capture_output=True, text=True, check=True)
    return json.loads(done.stdout)


def duplicate_of(finding: dict, issues: list[dict]) -> int | None:
    key = qa_key(finding)
    title = finding["title"].lower()
    for issue in issues:
        if f"qa-key: {key}" in (issue.get("body") or ""):
            return issue["number"]
        if difflib.SequenceMatcher(None, title, issue["title"].lower()).ratio() > 0.75:
            return issue["number"]
    return None


def issue_body(finding: dict, run_dir: Path) -> str:
    persona_dir = run_dir / finding["persona"]
    rows = [json.loads(line) for line in (persona_dir / "transcript.jsonl").read_text().splitlines()]
    turn = finding.get("turn")
    near = [r for r in rows if isinstance(turn, int) and turn - 3 <= r["n"] <= turn] or rows[-4:]
    steps = "\n".join(f"{i + 1}. {r['text'][:300]}" for i, r in enumerate(r for r in rows if r["kind"] == "user"))
    excerpt = "\n\n".join(f"[{r['kind']}{', ' + str(r['ms']) + ' ms' if 'ms' in r else ''}] {r['text'][:1200]}" for r in near)
    evidence = [r.get("evidence") for r in near if r.get("evidence")]
    return f"""Found by the simulated-user QA run (`scripts/qa/simulated-users.sh`, docs/qa/simulated-users.md).

**Persona:** {finding['persona_name']} (`qa/personas/{finding['persona']}.toml`)
**Surface:** {finding['surface']}
**Category:** {finding.get('category')} · **Severity:** {finding.get('severity')}

## Steps

{steps or '(the surface opened)'}

## Expected

{finding.get('expected', '')}

## Actual

{finding.get('actual', '')}

> {str(finding.get('excerpt', '')).replace(chr(10), chr(10) + '> ')}

## Evidence

```
{excerpt[:5000]}
```

Run: `{run_dir.name}`; evidence files: {', '.join(map(str, evidence)) or 'transcript.jsonl'}. Re-run with
`scripts/qa/simulated-users.sh --personas {finding['persona']}`.

<!-- qa-key: {qa_key(finding)} -->
"""


def file_issues(run_dir: Path, ids: list[str], extra_note: dict[str, str]) -> None:
    findings = json.loads((run_dir / "findings.json").read_text())
    chosen = [f for f in findings if f["id"] in ids]
    issues = open_issues()
    filed = {}
    for finding in chosen:
        dup = duplicate_of(finding, issues)
        if dup:
            log(f"{finding['id']}: duplicate of #{dup}, not filed")
            filed[finding["id"]] = {"duplicate": dup}
            continue
        body = issue_body(finding, run_dir)
        if finding["id"] in extra_note:
            body = extra_note[finding["id"]] + "\n\n" + body
        title = finding["title"]
        done = subprocess.run(["gh", "issue", "create", "-R", REPO, "--label", "qa", "--title", title,
                               "--body-file", "-"], input=body, capture_output=True, text=True)
        if done.returncode != 0:
            log(f"{finding['id']}: gh failed: {done.stderr.strip()}")
            continue
        url = done.stdout.strip()
        log(f"{finding['id']}: filed {url}")
        filed[finding["id"]] = {"url": url}
        issues.append({"number": int(url.rsplit("/", 1)[-1]), "title": title, "body": body})
    with open(run_dir / "issues.json", "a") as f:
        f.write(json.dumps(filed) + "\n")


# Main -------------------------------------------------------------------------------


def load_personas(names: str | None) -> list[dict]:
    personas = []
    for path in sorted((QA / "personas").glob("*.toml")):
        persona = tomllib.loads(path.read_text())
        if names and persona["id"] not in names.split(","):
            continue
        personas.append(persona)
    return personas


def report(run_dir: Path, verdicts: list[dict]) -> None:
    findings = [f for v in verdicts for f in v.get("findings", [])]
    (run_dir / "findings.json").write_text(json.dumps(findings, indent=2))
    lines = [f"# Simulated-user QA run {run_dir.name}", "",
             f"Chat-worker jobs: {BUDGET.jobs} (cap {BUDGET.max_jobs}); model calls: {BUDGET.model_calls}, "
             f"${BUDGET.model_usd:.4f}.", ""]
    for v in verdicts:
        ms = v.get("timings_ms") or []
        lines += [f"## {v['persona']}", "", v.get("summary", ""), "",
                  f"Goals met: {v.get('goals_met')}; missed: {v.get('goals_missed')}; reply times (ms): {ms}", ""]
        for f in v.get("findings", []):
            lines.append(f"- **{f['id']}** [{f.get('severity')}/{f.get('category')}] {f['title']} "
                         f"(turn {f.get('turn')}): \"{str(f.get('excerpt', ''))[:200]}\"")
        lines.append("")
    (run_dir / "report.md").write_text("\n".join(lines))


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--personas", help="comma-separated persona ids (default: all)")
    p.add_argument("--surfaces", help="comma-separated surfaces to run (default: all)")
    p.add_argument("--out", help="the run directory (default: qa-runs/<time> under $TMPDIR)")
    p.add_argument("--bin-dir", default=os.path.join(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target")), "debug"),
                   help="where openagents, openagents-desktop, coder, and microcoder are")
    p.add_argument("--max-jobs", type=int, default=300, help="cap on live chat-worker jobs per run")
    p.add_argument("--persona-model", default=os.environ.get("OPENAGENTS_QA_PERSONA_MODEL", DEFAULT_MODEL))
    p.add_argument("--judge-model", default=os.environ.get("OPENAGENTS_QA_JUDGE_MODEL", DEFAULT_MODEL),
                   help="an OpenRouter model id, or claude:MODEL for the Claude CLI")
    p.add_argument("--file", metavar="IDS", help="file these finding ids of --out's run as issues, after dedupe")
    p.add_argument("--list", action="store_true", help="list the personas")
    opts = p.parse_args()
    if opts.list:
        for persona in load_personas(None):
            print(f"{persona['id']:18} {persona['surface']:9} {persona['name']}")
        return 0
    if opts.file:
        if not opts.out:
            p.error("--file needs --out, the run directory")
        file_issues(Path(opts.out), opts.file.split(","), {})
        return 0
    BUDGET.max_jobs = opts.max_jobs
    opts.openagents = os.path.join(opts.bin_dir, "openagents")
    run_dir = Path(opts.out or Path(tempfile.gettempdir()) / "qa-runs" / time.strftime("%Y%m%dT%H%M%S"))
    run_dir.mkdir(parents=True, exist_ok=True)
    personas = load_personas(opts.personas)
    if opts.surfaces:
        personas = [x for x in personas if x["surface"] in opts.surfaces.split(",")]
    log(f"run {run_dir}: {', '.join(x['id'] for x in personas)}")
    verdicts = []
    for persona in personas:
        verdicts.append(run_persona(persona, run_dir, opts))
        report(run_dir, verdicts)
    report(run_dir, verdicts)
    print((run_dir / "report.md").read_text())
    print(f"run directory: {run_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""The Coder harness acceptance gate (#11549): a real chat on every harness.

scripts/coder/harness-acceptance.sh runs this; docs/coder/harness-acceptance.md
is the short runbook. For each harness installed on this computer it opens the
real Coder TUI headlessly (tmux, its own socket), in an isolated HOME and Coder
state, starts a new chat pinned to that harness with Shift+Tab, and sends
three tiny turns. It reads what the screen showed, Coder's own records (the
routing records, the saved chat, the /export ATIF), and the harness's own
session logs where it keeps them, and checks:

  answer          a real, non-empty answer from the model; the turn ends
  rail            the rail reads `Harness (Model display name)`, no raw slug
  default-model   the model that ran is the harness's intended default
  thinking        reasoning the model reported streamed into the transcript
  clean           no Coder notices, "New chat" lines, raw HTTP or log paths
  models-new      /models in the new chat before any message: own catalog
  models-turn     /models after a turn: own catalog, current model marked
  one-lane        every turn ran on the same harness, model and payer, even
                  after another model is picked in /models mid-chat
  shift-tab       Shift+Tab after a turn doesn't switch the chat's harness
  export          the ATIF export matches what the screen showed
  ctrl-n          Ctrl+N writes nothing into the new chat's transcript
  click-model     clicking a reply shows its model for ~3 s, then it's gone,
                  and it never enters the transcript or the export
  usage           Codex and Claude: the rail's plan windows after a turn are
                  the provider's values, labelled by window (5h, week), no 0d
  model-calls     the export's model_calls name a model and a duration
  prompt-tokens   the Coder loop's first-turn prompt is under the budget
  billing         a Claude model picked in the Coder loop, with a Claude
                  login here, runs on the Claude plan, not gateway credit

Nothing here writes to the real ~/.openagents or to a harness's own config
directory: each harness gets a private copy of only the login it needs
(account.json and sync.json are never copied), and Claude Code reads the
real login Keychain through a link, as scripts/release/acceptance.sh does.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import datetime as dt
import json
import os
import re
import shlex
import shutil
import sqlite3
import urllib.request
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REAL_HOME = Path(os.environ.get("HACC_REAL_HOME", str(Path.home())))
SOCKET = f"hacc-{os.getpid()}"
SEED_FACTS = True
PROMPTS = [
    "who are you? answer in one line",
    "what's 2+2?",
    "Is 391 prime? Think it through briefly, then answer in one line.",
]
CHECKS = [
    "answer",
    "rail",
    "default-model",
    "thinking",
    "clean",
    "models-new",
    "models-turn",
    "one-lane",
    "shift-tab",
    "export",
    "ctrl-n",
    "click-model",
    "usage",
    "model-calls",
    "prompt-tokens",
    "billing",
]
# Text that must never be inside a reply or the transcript.
FORBIDDEN = [
    (re.compile(r"doesn.t ask before every step", re.I), "Coder's permissions notice"),
    (re.compile(r"New chat\. The last one", re.I), "the Ctrl+N notice"),
    (re.compile(r"\bHTTP[ /]?\d\.?\d? ?[45]\d\d\b|\b(status|error)[: ]+[45]\d\d\b|\b[45]\d\d (Bad Request|Unauthorized|Forbidden|Not Found)", re.I), "a raw HTTP error"),
    (re.compile(r"(~|/(private|tmp|Users|var))[\w./-]*\.(log|jsonl)\b|raw-http-request=|(~|/(private|tmp|Users|var))[\w./-]*/(logs?|http-\d{3}-requests)/[\w./-]+"), "a log path"),
]

# The harnesses, in Shift+Tab's order. `rail` is the name the rail shows;
# `bins` decide whether it's installed (as acp_discovery does).
HARNESSES = [
    {"key": "auto", "rail": ["Auto"], "bins": []},
    {"key": "coder-loop", "rail": ["Coder", "Coder loop"], "bins": [], "gateway": True},
    {"key": "claude-code", "rail": ["Claude", "Claude Code"], "bins": ["claude"]},
    {"key": "codex", "rail": ["Codex"], "bins": ["codex"]},
    {"key": "opencode", "rail": ["OpenCode"], "bins": ["opencode"]},
    {"key": "grok-build", "rail": ["Grok Build", "Grok"], "bins": ["grok"]},
    {"key": "devin-cli", "rail": ["Devin", "Devin CLI"], "bins": ["devin"]},
    {"key": "cursor", "rail": ["Cursor"], "bins": ["cursor-agent"]},
    {"key": "oh-my-pi", "rail": ["Oh My Pi"], "bins": ["omp"]},
    {"key": "goose", "rail": ["Goose"], "bins": ["goose"]},
    {"key": "kimi", "rail": ["Kimi Code", "Kimi"], "bins": ["kimi"]},
    {"key": "amp", "rail": ["Amp"], "bins": ["amp", "amp-acp"]},
    {"key": "hermes", "rail": ["Hermes Agent", "Hermes"], "bins": ["hermes-acp"]},
]
BY_RAIL = {name: h for h in HARNESSES for name in h["rail"]}
ALL_LOGINS = [h["key"] for h in HARNESSES] + ["extensions"]


# ---------------------------------------------------------------- helpers


def say(msg: str) -> None:
    print(f"==> {msg}", file=sys.stderr, flush=True)


def run(cmd, env=None, timeout=90, input=None) -> tuple[int, str]:
    env = dict(env if env is not None else os.environ)
    env.update(BROWSER="true", NO_OPEN_BROWSER="1")
    try:
        p = subprocess.run(
            cmd, env=env, input=input, capture_output=True, text=True, timeout=timeout
        )
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except (subprocess.TimeoutExpired, OSError) as e:
        return 124, str(e)


def norm(s: str) -> str:
    return re.sub(r"[^a-z0-9]", "", (s or "").lower())


def strip_params(model: str) -> str:
    return re.sub(r"\[.*?\]", "", model or "").strip()


def words(s: str) -> str:
    return re.sub(r"\s+", " ", re.sub(r"[│┌┐└┘─╭╮╰╯●❯]", " ", s or "")).strip()


def unmark(s: str) -> str:
    """Text as Markdown draws it: no emphasis, code or heading marks."""
    return re.sub(r"\s+", " ", re.sub(r"(\*\*|__|[*_`]|^#+\s)", "", s or "")).strip()


def owner_path() -> str:
    keep = []
    for part in os.environ.get("PATH", "").split(":"):
        if not part or ".openagents/bin" in part:
            continue
        keep.append(part)
    return ":".join(keep)


def which(name: str, path: str) -> str | None:
    return shutil.which(name, path=path)


# ---------------------------------------------------------------- isolation


def make_home(root: Path, key: str, keys: list[str]) -> Path:
    """A private HOME holding copies of just the logins `keys` need."""
    home = root / key
    for sub in ("Library", "proj", ".config", ".local/share", ".cache", "state"):
        (home / sub).mkdir(parents=True, exist_ok=True)
    os.chmod(home, 0o700)
    # The gate's chats stay in its own home: chat sync is chosen off, so
    # nothing it says reaches the owner's account (and Coder doesn't ask).
    sync = home / "state" / "sync.json"
    if not sync.exists():
        sync.write_text(json.dumps({"on": False, "chosen": True, "folder": str(home / "state")}))

    def copy(src: Path, dst_rel: str) -> None:
        if src.is_file():
            dst = home / dst_rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, dst)
            os.chmod(dst, 0o600)

    def sqlite_copy(src: Path, dst_rel: str) -> None:
        if src.is_file():
            dst = home / dst_rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            out = sqlite3.connect(dst)
            try:
                con = sqlite3.connect(f"file:{src}?mode=ro", uri=True)
                con.backup(out)
            except sqlite3.OperationalError:
                # A WAL database whose -shm is gone (nothing has it open)
                # can't open read-only; read it as an unchanging file.
                con = sqlite3.connect(f"file:{src}?mode=ro&immutable=1", uri=True)
                con.backup(out)
            out.close()
            con.close()
            os.chmod(dst, 0o600)

    for k in keys:
        if k == "claude-code":
            # The login lives in the Keychain: link it, as acceptance.sh does.
            kc = home / "Library" / "Keychains"
            if not kc.exists():
                kc.symlink_to(REAL_HOME / "Library" / "Keychains")
            try:
                state = json.loads((REAL_HOME / ".claude.json").read_text())
                keep = {
                    k2: state[k2]
                    for k2 in ("oauthAccount", "userID", "hasCompletedOnboarding")
                    if k2 in state
                }
                (home / ".claude.json").write_text(json.dumps(keep))
            except (OSError, ValueError):
                pass
        elif k == "codex":
            copy(REAL_HOME / ".codex" / "auth.json", ".codex/auth.json")
            copy(REAL_HOME / ".codex" / "config.toml", ".codex/config.toml")
        elif k == "grok-build":
            copy(REAL_HOME / ".grok" / "auth.json", ".grok/auth.json")
            copy(REAL_HOME / ".grok" / "config.toml", ".grok/config.toml")
            # Its model list, as the owner's Grok Build has it cached.
            copy(REAL_HOME / ".grok" / "models_cache.json", ".grok/models_cache.json")
        elif k == "devin-cli":
            copy(REAL_HOME / ".local/share/devin/credentials.toml", ".local/share/devin/credentials.toml")
            copy(REAL_HOME / ".config/devin/config.json", ".config/devin/config.json")
        elif k == "cursor":
            copy(REAL_HOME / ".cursor" / "cli-config.json", ".cursor/cli-config.json")
            kc = home / "Library" / "Keychains"
            if not kc.exists():
                kc.symlink_to(REAL_HOME / "Library" / "Keychains")
        elif k == "opencode":
            copy(REAL_HOME / ".local/share/opencode/auth.json", ".local/share/opencode/auth.json")
            copy(REAL_HOME / ".config/opencode/opencode.json", ".config/opencode/opencode.json")
        elif k == "oh-my-pi":
            for f in ("agent.db", "models.db"):
                sqlite_copy(REAL_HOME / ".omp/agent" / f, f".omp/agent/{f}")
            copy(REAL_HOME / ".omp/agent/config.yml", ".omp/agent/config.yml")
        elif k == "extensions":
            # Installed extension plugins (Disk cleanup...): packages, no keys.
            src = REAL_HOME / ".openagents" / "extensions"
            if src.is_dir() and not (home / ".openagents" / "extensions").exists():
                (home / ".openagents").mkdir(parents=True, exist_ok=True)
                shutil.copytree(src, home / ".openagents" / "extensions", symlinks=True)
        elif k == "amp":
            copy(REAL_HOME / ".local/share/amp/secrets.json", ".local/share/amp/secrets.json")
    proj = home / "proj"
    if not (proj / ".git").exists():
        (proj / "README.md").write_text("# scratch\n\nA scratch project for the harness gate.\n")
        g = ["git", "-C", str(proj), "-c", "user.email=gate@example.com", "-c", "user.name=gate"]
        run(["git", "init", "-q", "-b", "main", str(proj)])
        run(g + ["add", "."])
        run(g + ["commit", "-qm", "init"])
    return home


# The dedicated test account first (#11601): no gate run may leave chats,
# computers, or projects on the owner's account.
SIGNIN_KEY_FILES = [
    REAL_HOME / "work/.secrets/openagents-test-account.env",
    REAL_HOME / "work/.secrets/coder-gate-signin.env",
    REAL_HOME / "work/.secrets/coder-acceptance.env",
]


def harness_env(home: Path, path: str, extra: dict | None = None) -> dict:
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith(("OPENAGENTS_", "CODER_", "CODEX_", "CLAUDE_", "GROK_", "XDG_", "TMUX"))
        and k not in ("ANTHROPIC_API_KEY", "OPENAI_API_KEY", "OPENROUTER_API_KEY", "XAI_API_KEY")
    }
    env.update(
        HOME=str(home),
        PATH=path,
        TERM="xterm-256color",
        COLORTERM="truecolor",
        LANG=env.get("LANG", "en_US.UTF-8"),
        CODER_UPDATE="off",
        # Nothing the gate starts may open a browser or an interactive
        # sign-in on the owner's screen (owner, 2026-10-13).
        BROWSER="true",
        NO_OPEN_BROWSER="1",
        XDG_CONFIG_HOME=str(home / ".config"),
        XDG_DATA_HOME=str(home / ".local/share"),
        XDG_CACHE_HOME=str(home / ".cache"),
        # Signed in with the gate's key, Coder must not pose as the owner's
        # Mac: no check-in under its name, no replies taken from the web.
        OPENAGENTS_COMPUTER_NAME="harness-acceptance-gate",
        # A test run (#11601): every call Coder and openagents make carries
        # X-OpenAgents-Test: 1, so the site lists none of the chats it syncs,
        # deletes them within an hour, and records no computer.
        OPENAGENTS_TEST_RUN="1",
    )
    if extra:
        env.update(extra)
    return env


# ---------------------------------------------------------------- the TUI


class Tui:
    """One Coder TUI in a tmux pane on the gate's own tmux server."""

    def __init__(self, name: str, coder: str, home: Path, state: Path, env: dict, cols=150, rows=56, unset=()):
        self.name = name
        self.home = home
        launcher = home / f"run-{name}.sh"
        lines = ["#!/bin/sh"] + [f"unset {v}" for v in unset]
        lines += [f"export {k}={shlex.quote(v)}" for k, v in env.items() if re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", k)]
        lines.append(
            f"exec {shlex.quote(coder)} --in {shlex.quote(str(home / 'proj'))} --state {shlex.quote(str(state))} 2>>{shlex.quote(str(home / ('stderr-' + name + '.log')))}"
        )
        launcher.write_text("\n".join(lines) + "\n")
        os.chmod(launcher, 0o700)
        subprocess.run(
            ["tmux", "-L", SOCKET, "new-session", "-d", "-s", name, "-x", str(cols), "-y", str(rows), str(launcher)],
            check=True,
            env={k: v for k, v in os.environ.items() if not k.startswith("TMUX")},
        )
        # A copy a run makes (an export's path) never reaches a clipboard.
        subprocess.run(["tmux", "-L", SOCKET, "set-option", "-g", "set-clipboard", "off"], capture_output=True)

    def screen(self) -> str:
        p = subprocess.run(["tmux", "-L", SOCKET, "capture-pane", "-p", "-t", self.name], capture_output=True, text=True)
        return p.stdout

    def alive(self) -> bool:
        return subprocess.run(["tmux", "-L", SOCKET, "has-session", "-t", self.name], capture_output=True).returncode == 0

    def keys(self, *keys: str) -> None:
        subprocess.run(["tmux", "-L", SOCKET, "send-keys", "-t", self.name, *keys], capture_output=True)

    def text(self, s: str) -> None:
        subprocess.run(["tmux", "-L", SOCKET, "send-keys", "-t", self.name, "-l", s], capture_output=True)

    def raw(self, data: bytes) -> None:
        subprocess.run(["tmux", "-L", SOCKET, "send-keys", "-t", self.name, "-H", *[f"{b:02x}" for b in data]], capture_output=True)

    def click(self, col: int, row: int) -> None:
        # An SGR mouse press and release (1-based cells), as a terminal sends.
        self.raw(f"\x1b[<0;{col};{row}M".encode())
        time.sleep(0.05)
        self.raw(f"\x1b[<0;{col};{row}m".encode())

    def wait(self, pred, timeout: float, every=0.3):
        end = time.time() + timeout
        while time.time() < end:
            s = self.screen()
            if pred(s):
                return s
            time.sleep(every)
        return None

    def stop(self) -> None:
        self.keys("Escape")
        time.sleep(0.2)
        self.keys("C-c")
        time.sleep(0.5)
        self.keys("C-c")
        time.sleep(0.5)
        subprocess.run(["tmux", "-L", SOCKET, "kill-session", "-t", self.name], capture_output=True)


RAIL_RE = re.compile(r"^┌─.* ─┐\s*$")


def rail(screen: str) -> dict | None:
    for line in reversed(screen.splitlines()):
        m = RAIL_RE.match(line.strip())
        if m:
            # A running turn writes "⠦ Working (14s)" on the left; the
            # label is the last segment.
            label = re.split(r"─{2,}", line.strip().rstrip("┐").rstrip("─ "))[-1].strip(" ─┌")
            head = label.split(" · ")[0].strip()
            m2 = re.match(r"^(.*?) \((.*)\)$", head)
            name, display = (m2.group(1), m2.group(2)) if m2 else (head, "")
            return {"label": label, "head": head, "name": name.strip(), "display": display.strip()}
    return None


def transcript(screen: str) -> list[str]:
    """The conversation area: below the header, above the input box."""
    lines = screen.splitlines()
    end = len(lines)
    for i in range(len(lines) - 1, -1, -1):
        if RAIL_RE.match(lines[i].strip()):
            end = i
            break
    body = lines[:end]
    # The two-line header (name and version) isn't conversation.
    out = []
    for i, line in enumerate(body):
        if i < 3 and re.match(r"^\s*(OpenAgents Coder|v\d)", line):
            continue
        out.append(line.rstrip())
    return out


def answers(screen: str) -> list[str]:
    """The agent replies on screen: blocks that start with '●'."""
    out: list[str] = []
    cur: list[str] | None = None
    for line in transcript(screen):
        s = line.strip()
        if s.startswith("● "):
            if cur:
                out.append(" ".join(cur))
            cur = [s[2:]]
        elif s.startswith(("❯ ", "●")) or not s:
            if cur:
                out.append(" ".join(cur))
            cur = None
        elif cur is not None:
            cur.append(s)
    if cur:
        out.append(" ".join(cur))
    return [words(a) for a in out if words(a)]


def picker_open(screen: str) -> bool:
    # Any boxed picker: Coder's own ("Pick model", "Pick reasoning level")
    # or a harness's model list, all with the same footer.
    # A long list can push the footer off screen; the title still shows.
    return (
        "╭ Pick" in screen
        or "Pick model" in screen
        or "╭ Select model" in screen
        or "╭ What runs here" in screen
        or ("╭" in screen and "Enter Select" in screen)
    )


def read_picker(tui: Tui, limit=60) -> dict:
    """Opens /models and walks it with ↓, reading each row's name and ID."""
    tui.text("/models")
    time.sleep(0.4)
    tui.keys("Enter")
    s = tui.wait(picker_open, 4)
    if not s:
        time.sleep(0.5)
        body = [l for l in transcript(tui.screen()) if l.strip() and not l.strip().startswith(("❯", "●"))]
        return {"opened": False, "notice": words(" ".join(body[-2:])), "items": [], "screen": tui.screen()}
    items: list[dict] = []
    seen = set()
    first_rows = picker_rows(s)
    for i in range(limit):
        s = tui.screen()
        sel = selected_row(s)
        mid = picker_field(s, "ID")
        prov = picker_field(s, "Provider")
        key = (sel, mid)
        if key in seen:
            break
        seen.add(key)
        items.append({"name": sel, "id": mid, "provider": prov, "current": "(current)" in (sel or "")})
        tui.keys("Down")
        time.sleep(0.25)
    return {"opened": True, "items": items, "rows": first_rows, "screen": s}


def picker_lines(screen: str) -> list[str]:
    out = []
    for line in screen.splitlines():
        m = re.search(r"│(.*)│", line)
        if m:
            out.append(m.group(1))
    return out


def picker_rows(screen: str) -> list[str]:
    rows, on = [], False
    for line in picker_lines(screen):
        if line.strip().startswith("Search:"):
            on = True
            continue
        if on:
            if not line.strip():
                break
            rows.append(line.strip())
    return rows


def selected_row(screen: str) -> str | None:
    for row in picker_rows(screen):
        if row.startswith("❯"):
            return re.split(r"\s{2,}", row[1:].strip())[0]
    return None


def picker_field(screen: str, field: str) -> str | None:
    for line in picker_lines(screen):
        m = re.match(rf"^\s*{field}\s+(.+?)\s*$", line)
        if m:
            return m.group(1)
    return None


# ---------------------------------------------------------------- records


def routing_records(state: Path) -> list[dict]:
    out = []
    for f in sorted((state / "routing").glob("*.jsonl")):
        for line in f.read_text(errors="replace").splitlines():
            try:
                out.append(json.loads(line))
            except ValueError:
                pass
    return out


def turns_of(records: list[dict]) -> list[dict]:
    """Each turn's choice and end, in order."""
    chose = [r for r in records if r.get("type") == "chose"]
    ended = {r.get("id"): r for r in records if r.get("type") == "ended"}
    out = []
    for c in chose:
        e = ended.get(c.get("id"), {})
        out.append({"chose": c, "ended": e})
    return out


def newest_session(state: Path) -> Path | None:
    files = sorted((state / "sessions").glob("*.atif.json"), key=lambda p: p.stat().st_mtime)
    return files[-1] if files else None


def atif_steps(doc: dict) -> list[dict]:
    return doc.get("steps", []) if isinstance(doc, dict) else []


def devin_refused(home: Path, since: float) -> str:
    """Devin's own log line, since `since`, saying the model Coder asked
    for (`devin acp --model`) isn't one its server allows here."""
    for f in sorted((home / ".local/share/devin/cli/logs").glob("*.log")):
        try:
            if f.stat().st_mtime < since:
                continue
            for line in f.read_text(errors="replace").splitlines():
                if "`devin acp --model` value did not resolve" in line:
                    return "`devin acp --model` value did not resolve to an available model"
        except OSError:
            pass
    return ""


def native_reasoning(key: str, home: Path, since: float) -> dict:
    """What the harness's own logs say it reasoned: tokens and any text."""
    tokens, texts = 0, []
    if key in ("codex", "auto", "coder-loop"):
        for f in (home / ".codex" / "sessions").rglob("*.jsonl"):
            if f.stat().st_mtime < since:
                continue
            for line in f.read_text(errors="replace").splitlines():
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                p = d.get("payload", {}) or {}
                if p.get("type") == "token_count":
                    info = (p.get("info") or {}).get("total_token_usage") or {}
                    tokens = max(tokens, int(info.get("reasoning_output_tokens") or 0))
                elif p.get("type") == "reasoning":
                    for part in p.get("summary") or []:
                        if part.get("text"):
                            texts.append(part["text"])
                elif p.get("type") == "agent_reasoning" and p.get("text"):
                    texts.append(p["text"])
    if key in ("claude-code", "auto", "coder-loop"):
        for f in (home / ".claude" / "projects").rglob("*.jsonl"):
            if f.stat().st_mtime < since:
                continue
            for line in f.read_text(errors="replace").splitlines():
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                msg = d.get("message") or {}
                for part in msg.get("content") or [] if isinstance(msg.get("content"), list) else []:
                    if isinstance(part, dict) and part.get("type") == "thinking" and part.get("thinking"):
                        texts.append(part["thinking"])
    return {"tokens": tokens, "texts": texts}


# ---------------------------------------------------------------- expectations


def cursor_headless(path: str, root: Path) -> bool:
    """Whether Cursor is signed in inside an isolated home, read with
    `cursor-agent status --format json` and nothing that can open a
    browser. Without that, the gate never starts Cursor: a signed-out
    Cursor opens a browser sign-in on the owner's screen."""
    home = make_home(root, "cursor-check", ["cursor"])
    code, out = run(["cursor-agent", "status", "--format", "json"], env=harness_env(home, path), timeout=20)
    try:
        return json.loads(out[out.index("{"):]).get("isAuthenticated") is True
    except (ValueError, AttributeError):
        return False


def expectations(path: str, homes_root: Path) -> dict:
    """Each harness's intended default model and its own catalog, read from
    the harness's own CLI or config where it has one."""
    exp: dict[str, dict] = {}
    tmp = make_home(homes_root, "probe", ["grok-build", "devin-cli", "cursor", "opencode", "oh-my-pi"])
    env = harness_env(tmp, path)
    # Grok Build: `grok models` names its default.
    if which("grok", path):
        code, out = run(["grok", "models"], env=env, timeout=90)
        default = re.search(r"Default model:\s*(\S+)", out)
        ids = re.findall(r"^\s*[*-]\s+(\S+)", out, re.M)
        exp["grok-build"] = {
            "default": default.group(1) if default else "grok-4.7",
            "rule": f"`grok models` default ({default.group(1) if default else 'unreadable; grok-4.7'})",
            "catalog": ids,
        }
    # Devin: its configured model, else SWE-2.
    if which("devin", path):
        code, out = run(["devin", "models", "list"], env=env, timeout=90)
        ids, names = [], {}
        for line in out.splitlines():
            m = re.match(r"^  (\S+)\s{2,}(.+?)(\s{2,}\[.*)?$", line)
            if m and not m.group(1).startswith("aliases"):
                ids.append(m.group(1))
                names[m.group(1)] = m.group(2).strip()
        conf = {}
        try:
            conf = json.loads((REAL_HOME / ".config/devin/config.json").read_text())
        except (OSError, ValueError):
            pass
        configured = ((conf.get("agent") or {}).get("model")) or ""
        # Coder runs the model the person configured for Devin, else its
        # own SWE-2 Medium (owner, 2026-10-13).
        exp["devin-cli"] = {
            "default": configured or "swe-2-medium",
            "rule": f"configured `{configured}` (devin config)" if configured else "Coder's SWE-2 Medium (nothing configured)",
            "catalog": ids,
            "names": names,
        }
    # Cursor: the CLI's selected model, only when its login works headless
    # (the status check never opens a browser).
    if which("cursor-agent", path) and cursor_headless(path, root=homes_root):
        code, out = run(["cursor-agent", "models"], env=env, timeout=90)
        ids = re.findall(r"^(\S+) - ", out, re.M)
        sel = ""
        try:
            conf = json.loads((REAL_HOME / ".cursor/cli-config.json").read_text())
            sel = ((conf.get("selectedModel") or {}).get("modelId")) or ((conf.get("model") or {}).get("modelId")) or ""
            disp = ((conf.get("model") or {}).get("displayName")) or ""
        except (OSError, ValueError):
            disp = ""
        exp["cursor"] = {
            "default": sel or "auto",
            "display": disp,
            "rule": f"cursor-agent's selected model `{sel or 'auto'}`" + (f" ({disp})" if disp else ""),
            "catalog": ids,
        }
    # OpenCode: opencode.json's model, else its own default.
    if which("opencode", path):
        # The first run in a fresh home lists OpenCode's cached catalog and
        # refreshes it; the second reads the fresh one, as Coder later does.
        run(["opencode", "models"], env=env, timeout=120)
        code, out = run(["opencode", "models", "--verbose"], env=env, timeout=120)
        ids = [l.strip() for l in out.splitlines() if re.match(r"^[\w.-]+/\S+$", l.strip())]
        configured, display = "", ""
        try:
            conf = json.loads((REAL_HOME / ".config/opencode/opencode.json").read_text())
            configured = conf.get("model", "")
            if "/" in configured:
                prov, mid = configured.split("/", 1)
                display = (((conf.get("provider") or {}).get(prov) or {}).get("models") or {}).get(mid, {}).get("name", "")
        except (OSError, ValueError, AttributeError):
            pass
        exp["opencode"] = {
            "default": configured,
            # Its name in the person's opencode.json, as OpenCode shows it.
            "display": display,
            "rule": f"configured `{configured}` (opencode.json)" if configured else "OpenCode's own default (a free model)",
            "catalog": ids,
        }
    # Oh My Pi: any chat-capable model, never a clef* local model.
    if which("omp", path):
        code, out = run(["omp", "models"], env=env, timeout=90)
        ids, provider = [], ""
        for line in out.splitlines():
            m = re.match(r"^(\S+) \(\d+\)", line)
            if m:
                provider = m.group(1)
            m = re.match(r"^│ (\S+)\s+│", line)
            if m and m.group(1) != "model":
                ids.append(f"{provider}/{m.group(1)}" if provider else m.group(1))
        exp["oh-my-pi"] = {
            "default": None,
            "never": r"clef",
            "rule": "a chat-capable model, never clef*",
            "catalog": ids,
            # Signed in to nothing that chats: only the Clef decision models.
            "no_chat": code == 0 and all(re.search(r"clef", i, re.I) for i in ids),
        }
    exp["claude-code"] = {
        "default": os.environ.get("HACC_CLAUDE_DEFAULT", "claude-haiku-5-5"),
        "rule": "Coder's Claude default (Haiku 5.5) on a fresh state",
        "prefix": "claude-",
    }
    exp["codex"] = {
        "default": os.environ.get("HACC_CODEX_DEFAULT", "gpt-6.1-sol"),
        "rule": "Coder's Codex default (GPT-6.1-Sol) on a fresh state",
        "pattern": r"^(gpt-|o\d|codex)",
    }
    return exp


# ---------------------------------------------------------------- one harness


class Run:
    def __init__(self, spec, coder, root, path, exp, signin, budget, turn_timeout, evidence):
        self.spec = spec
        self.key = spec["key"]
        self.coder = coder
        self.path = path
        self.exp = exp.get(self.key, {})
        self.signin = signin
        self.budget = budget
        self.turn_timeout = turn_timeout
        self.ev = evidence / self.key
        self.ev.mkdir(parents=True, exist_ok=True)
        keys = ALL_LOGINS if self.key in ("auto", "coder-loop") else [self.key]
        self.home = make_home(root, self.key, keys)
        self.state = self.home / "state"
        self.results: dict[str, tuple[str, str]] = {}
        self.facts: dict = {"harness": self.key}

    def seed_state(self) -> None:
        """Start from the owner's harness facts (each harness's last-known
        model, sign-in and plan windows; no keys), as the owner's Coder
        does, unless --fresh-state."""
        src = REAL_HOME / ".openagents" / "coder" / "harness-facts.json"
        if SEED_FACTS and src.is_file() and not (self.state / "harness-facts.json").exists():
            self.state.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(src, self.state / "harness-facts.json")

    def ok(self, check, evidence=""):
        self.results[check] = ("pass", evidence)

    def fail(self, check, evidence):
        self.results[check] = ("fail", evidence)

    def na(self, check, evidence):
        self.results[check] = ("n/a", evidence)

    def skip(self, check, evidence):
        self.results[check] = ("skipped", evidence)

    def save(self, name, text):
        (self.ev / name).write_text(text if isinstance(text, str) else json.dumps(text, indent=2))

    def login(self, env) -> bool:
        if not self.signin:
            return False
        code, out = run([self.coder, "login", "--key", "--state", str(self.state)], env=env, input=self.signin, timeout=60)
        self.save("login.txt", re.sub(r"oa_sk_\w+", "oa_sk_…", out))
        return code == 0

    def pick(self, tui: Tui) -> bool:
        want = self.spec["rail"]
        if self.key == "auto":
            return (rail(tui.screen()) or {}).get("name") == "Auto"
        names = []
        for _ in range(20):
            tui.keys("BTab")
            time.sleep(0.9)
            r = rail(tui.screen()) or {}
            names.append(r.get("name"))
            if r.get("name") in want:
                return True
            if r.get("name") == "Auto" and len(names) > 1:
                break
        self.facts["offered"] = names
        return False

    def send(self, tui: Tui, prompt: str, n: int) -> dict:
        before = len(turns_of(routing_records(self.state)))
        tui.text(prompt)
        time.sleep(0.3)
        tui.keys("Enter")
        t0 = time.time()
        frames = []
        ended = None
        while time.time() - t0 < self.turn_timeout:
            frames.append(tui.screen())
            ts = turns_of(routing_records(self.state))
            if len(ts) > before and ts[-1]["ended"]:
                ended = ts[-1]
                break
            time.sleep(0.5)
        time.sleep(1.0)
        final = tui.screen()
        frames.append(final)
        secs = round(time.time() - t0, 1)
        self.save(f"turn{n}-final.txt", final)
        self.save(f"turn{n}-frames.txt", "\n\f\n".join(frames))
        return {"prompt": prompt, "secs": secs, "ended": ended, "final": final, "frames": frames}

    def go(self) -> dict:
        env = harness_env(self.home, self.path)
        self.seed_state()
        signed_in = self.login(env)
        if signed_in:
            # The isolated run sees the key as OPENAGENTS_TOKEN too, as the
            # tools Coder starts (openagents …) read it.
            env["OPENAGENTS_TOKEN"] = self.signin
        self.facts["signed_in"] = signed_in
        if self.spec.get("gateway") and not signed_in:
            for c in CHECKS:
                self.na(c, "not signed in: no OpenAgents sign-in key for the gate (see the runbook)")
            return self.report()
        tui = Tui(f"h-{self.key}", self.coder, self.home, self.state, env)
        try:
            return self._go(tui, signed_in)
        finally:
            tui.stop()
            self.save("stderr.txt", (self.home / f"stderr-h-{self.key}.log").read_text(errors="replace") if (self.home / f"stderr-h-{self.key}.log").exists() else "")

    def _go(self, tui: Tui, signed_in: bool) -> dict:
        start = time.time()
        if not tui.wait(lambda s: rail(s) is not None, 40):
            for c in CHECKS:
                self.fail(c, "Coder's screen never came up")
            self.save("startup.txt", tui.screen())
            return self.report()
        if self.spec.get("gateway"):
            # The Coder loop: its own Shift+Tab stop when it has one, else
            # Auto with a gateway model picked in /models.
            if not self.pick(tui):
                pass
        elif not self.pick(tui):
            for c in CHECKS:
                self.na(c, f"not offered by Shift+Tab (offered: {', '.join(n for n in self.facts.get('offered', []) if n)})")
            self.results["answer"] = ("fail", f"{self.key} is installed but Shift+Tab never offered it")
            return self.report()
        r0 = rail(tui.screen())
        self.facts["rail_new"] = r0
        self.save("new-chat.txt", tui.screen())

        # /models in the new chat, before any message.
        p0 = read_picker(tui)
        tui.keys("Escape")
        time.sleep(0.5)
        self.save("models-new.json", {k: v for k, v in p0.items() if k != "screen"})
        if p0.get("screen"):
            self.save("models-new.txt", p0["screen"])
        if not tui.wait(lambda s: not picker_open(s), 3):
            tui.keys("Escape")
            time.sleep(0.5)
        # A notice /models left is not a turn's output.
        self.models_check("models-new", p0, r0)

        if self.spec.get("gateway") and (rail(tui.screen()) or {}).get("name") == "Auto":
            self.choose_gateway(tui)

        turns = []
        for n, prompt in enumerate(PROMPTS, 1):
            if n == 3:
                # /models after a turn, then another model picked mid-chat:
                # the chat must keep its lane (one-lane).
                p1 = read_picker(tui)
                self.save("models-turn.json", {k: v for k, v in p1.items() if k != "screen"})
                if p1.get("screen"):
                    self.save("models-turn.txt", p1["screen"])
                self.models_check("models-turn", p1, rail(tui.screen()))
                other = [i for i in p1.get("items", []) if not i["current"]]
                if p1.get("opened") and other:
                    # The walk left the highlight on the last row; go back
                    # to the first other model (a chat model, not the last
                    # oddity in a long catalog).
                    idx = p1["items"].index(other[0])
                    for _ in range(len(p1["items"]) - 1 - idx):
                        tui.keys("Up")
                        time.sleep(0.1)
                    self.facts["picked_mid_chat"] = other[0]
                    tui.keys("Enter")
                    # A second step (the reasoning level) keeps its default.
                    for _ in range(3):
                        time.sleep(1.0)
                        if not picker_open(tui.screen()):
                            break
                        tui.keys("Enter")
                else:
                    tui.keys("Escape")
                time.sleep(1.0)
                for _ in range(3):
                    if not picker_open(tui.screen()):
                        break
                    tui.keys("Escape")
                    time.sleep(0.5)
            t = self.send(tui, prompt, n)
            turns.append(t)
            if not t["ended"]:
                break
            # The harness's own account plan can't run the model Coder asks
            # for (Coder's fixed refusal, e.g. "Devin CLI can't run SWE-2
            # Medium on this Devin account"): Coder behaved correctly, the
            # machine's account can't take the test, so the harness is
            # skipped with the reason, as a harness without a headless login is.
            record = (t["ended"] or {}).get("ended") or {}
            diagnostic = str(record.get("diagnostic") or "")
            if n == 1 and record.get("ended") == "error" and re.search(r"can't run .+ on this .+ account", diagnostic):
                self.facts["account_plan"] = diagnostic[:300]
                for check in CHECKS:
                    self.results[check] = ("skipped", f"this computer's {self.key} account can't run the model Coder asks for: {diagnostic[:160]}")
                self.turns = turns
                return self.report()
        self.turns = turns
        rail_after = rail(tui.screen())
        self.facts["rail_after"] = rail_after

        # The export, as /export writes it.
        export_path = self.home / "export.json"
        tui.text(f"/export {export_path}")
        time.sleep(0.3)
        tui.keys("Enter")
        for _ in range(20):
            if export_path.exists():
                break
            time.sleep(0.3)
        time.sleep(0.5)
        export = {}
        try:
            export = json.loads(export_path.read_text())
            self.save("export.json", export)
        except (OSError, ValueError):
            pass

        # Clicking a reply: its model, right-aligned, for about 3 s.
        self.click_check(tui, export_path)

        # Shift+Tab after a turn.
        before = tui.screen()
        tui.keys("BTab")
        time.sleep(1.5)
        after = tui.screen()
        self.save("shift-tab.txt", after)
        ra, rb = rail(before) or {}, rail(after) or {}
        kept_text = any(l.strip().startswith("❯ ") for l in transcript(after))
        if rb.get("name") == ra.get("name"):
            self.ok("shift-tab", f"rail stayed `{rb.get('label')}`")
        elif not kept_text:
            self.ok("shift-tab", f"Shift+Tab started a new chat on `{rb.get('label')}`; the old one kept its harness")
        else:
            self.fail("shift-tab", f"switched this chat from `{ra.get('head')}` to `{rb.get('head')}` after a turn")
        tui.keys("Escape")

        # Ctrl+N writes nothing into the new chat.
        tui.keys("C-n")
        time.sleep(1.5)
        s = tui.screen()
        self.save("ctrl-n.txt", s)
        body = [l for l in transcript(s) if l.strip()]
        if any("New chat" in l for l in body):
            self.fail("ctrl-n", "the new chat shows: " + words(" ".join(l for l in body if "New chat" in l))[:120])
        elif body:
            self.fail("ctrl-n", "the new chat isn't empty: " + words(" ".join(body))[:120])
        else:
            self.ok("ctrl-n", "the new chat's transcript is empty")

        self.analyse(turns, export, r0, rail_after, start)
        return self.report()

    def choose_gateway(self, tui: Tui) -> None:
        """Auto with a gateway model picked in /models (the Coder loop)."""
        p = read_picker(tui)
        tui.keys("Escape")
        self.facts["gateway_models"] = p.get("items", [])

    def click_check(self, tui: Tui, export_path: Path) -> None:
        s = tui.screen()
        lines = s.splitlines()
        row = next((i for i, l in enumerate(lines) if l.strip().startswith("● ")), None)
        model = ((self.turns[0]["ended"] or {}).get("ended") or {}).get("model") if self.turns else None
        if row is None:
            self.fail("click-model", "no reply on screen to click")
            return
        base = lines[row]
        tui.click(4, row + 1)
        shown = None
        for _ in range(8):
            time.sleep(0.25)
            now = tui.screen().splitlines()
            near = now[max(0, row - 1) : row + 2]
            for l in near:
                if l not in lines and re.search(r"\S\s*$", l):
                    tail = l.rstrip().split("   ")[-1].strip()
                    if tail and (not model or norm(strip_params(model))[:4] in norm(tail) or len(tail) < 40):
                        shown = l
                        break
            if shown:
                break
        time.sleep(4.0)
        later = tui.screen()
        self.save("click.txt", f"before:\n{s}\n\nshown:\n{shown}\n\nafter 4s:\n{later}")
        still = shown is not None and shown in later.splitlines()
        # Export again: the hint never enters the export.
        export_path2 = self.home / "export-after-click.json"
        tui.text(f"/export {export_path2}")
        time.sleep(0.3)
        tui.keys("Enter")
        time.sleep(1.5)
        leaked = False
        try:
            doc = json.loads(export_path2.read_text())
            msgs = " ".join(str(st.get("message", "")) for st in atif_steps(doc))
            if shown and words(shown.split("   ")[-1]) and words(shown.split("   ")[-1]) in msgs:
                leaked = True
        except (OSError, ValueError):
            pass
        if shown is None:
            self.fail("click-model", "clicking the reply showed no model (mouse sent as SGR press/release)")
        elif still:
            self.fail("click-model", f"model hint still on screen after 4 s: `{words(shown)[-60:]}`")
        elif leaked:
            self.fail("click-model", "the model hint entered the export")
        else:
            self.ok("click-model", f"showed `{words(shown)[-50:]}`, gone after 4 s, not in the export")

    def models_check(self, check: str, p: dict, r: dict | None) -> None:
        self.facts[f"{check}-screen"] = words(p.get("screen") or "")
        if not p.get("opened"):
            self.fail(check, f"/models didn't open a picker: “{p.get('notice', '')[:140]}”")
            return
        items = p.get("items", [])
        if not items:
            self.fail(check, "the picker listed nothing")
            return
        ids = [strip_params(i.get("id") or "") for i in items]
        stray = []
        cat = self.exp.get("catalog")
        if self.key in ("auto", "coder-loop"):
            pass
        elif cat:
            norm_cat = {strip_params(c) for c in cat}
            tails = {c.split("/")[-1] for c in norm_cat}
            stray = [i for i in ids if i and i not in norm_cat and i.split("/")[-1] not in tails]
        elif self.exp.get("prefix"):
            stray = [i for i in ids if i and not i.startswith(self.exp["prefix"])]
        elif self.exp.get("pattern"):
            stray = [i for i in ids if i and not re.match(self.exp["pattern"], i)]
        names = ", ".join((i.get("name") or "?").replace(" (current)", "") for i in items[:6])
        if stray:
            self.fail(check, f"not {self.key}'s catalog: {', '.join(stray[:5])} (listed: {names}…)")
            return
        current = [i for i in items if i["current"]]
        if not current:
            self.fail(check, f"no model marked current (listed: {names})")
            return
        if self.key == "opencode":
            free = [("free" in (i.get("id") or "").lower() or "free" in (i.get("name") or "").lower() or "big-pickle" in (i.get("id") or "")) for i in items]
            if any(free) and not all(free[: free.count(True)]):
                self.fail(check, f"free models aren't first (listed: {names})")
                return
        self.ok(check, f"{len(items)} of its own; current `{current[0]['name'].replace(' (current)', '')}`")

    def analyse(self, turns, export, r0, rail_after, start) -> None:
        ended = [t["ended"] for t in turns]
        # answer
        bad = []
        for n, t in enumerate(turns, 1):
            e = (t["ended"] or {}).get("ended") or {}
            ans = answers(t["final"])
            if not t["ended"]:
                bad.append(f"turn {n} didn't end in {self.turn_timeout}s")
            elif e.get("ended") not in ("done", None):
                bad.append(f"turn {n} ended `{e.get('ended')}`" + (f": {str(e.get('diagnostic'))[:100]}" if e.get("diagnostic") else ""))
            elif len(ans) < n:
                bad.append(f"turn {n}: no reply on screen")
        steps = atif_steps(export)
        agent_msgs = [st for st in steps if st.get("source") == "agent"]
        times = ", ".join(f"{t['secs']}s" for t in turns)
        if len(turns) < len(PROMPTS) and not bad:
            bad.append("stopped early")
        # OpenCode on the owner's configured model, refused by that model's
        # provider on every turn ("Update Required" from a third-party
        # Cursor proxy, 2026-10-13): the owner's provider to fix, not
        # Coder's. Coder must show the provider's words plainly and add
        # nothing to the chat.
        diags = [str(((t["ended"] or {}).get("ended") or {}).get("diagnostic") or "") for t in turns]
        ends = [((t["ended"] or {}).get("ended") or {}).get("ended") for t in turns]
        self.refused = ""
        if (
            self.key == "opencode"
            and turns
            and len(turns) == len(PROMPTS)
            and all(e == "error" for e in ends)
            and diags[0]
            and all(d == diags[0] for d in diags)
        ):
            said = re.sub(r"^OpenCode:\s*", "", diags[0])
            screen = words(" ".join(transcript(turns[-1]["final"])))
            plain = '{"' not in screen and words(said)[:30].replace("[path]", "") in screen.replace("[path]", "")
            if plain or words(said)[:30] in screen:
                self.refused = said
        if self.refused:
            self.skip(
                "answer",
                f"OpenCode's configured model `{self.exp.get('default')}` is refused by its provider on every turn: “{self.refused[:140]}”. Coder shows it plainly. Owner: update or replace that provider in OpenCode (NEEDS_OWNER.md)",
            )
        elif bad:
            self.fail("answer", "; ".join(bad) + f" (times {times})")
        else:
            first = answers(turns[0]["final"])
            self.ok("answer", f"“{first[0][:70] if first else ''}” ({times})")
        # models that ran
        ran = [((t["ended"] or {}).get("ended") or {}).get("model") or "" for t in turns]
        ran = [m or "" for m in ran]
        # The replies' models: a thinking step ahead of a reply isn't one.
        export_models = [st.get("model_name") or "" for st in agent_msgs if str(st.get("message") or "").strip()]
        model = ran[0] or (export_models[0] if export_models else "") or (export.get("agent") or {}).get("model_name", "")
        self.facts["models_ran"] = ran
        self.facts["export_models"] = export_models
        # rail
        rr = rail_after or {}
        disp = rr.get("display", "")
        problems = []
        if not disp:
            problems.append(f"no model in `{rr.get('label')}`")
        else:
            if re.search(r"[\[\]=]", disp):
                problems.append(f"raw slug `{disp}`")
            if re.search(r"\b\d+ \d+\b", disp):
                problems.append(f"slug-cased `{disp}`")
            if re.fullmatch(r"[a-z0-9./:_-]+", disp) and ("-" in disp or "/" in disp):
                problems.append(f"raw id `{disp}`")
            if model and not (norm(strip_params(model).split("/")[-1]) in norm(disp) or norm(disp) in norm(strip_params(model))):
                if not (self.exp.get("display") and norm(self.exp["display"]) == norm(disp)):
                    problems.append(f"`{disp}` but `{model}` ran")
        # The rail in the new chat, before the first message, named the
        # same model that then ran (a stale cached model is a lie).
        dn = (r0 or {}).get("display", "")
        if dn and model and not (norm(strip_params(model).split("/")[-1]) in norm(dn) or norm(dn) in norm(strip_params(model))):
            if not (self.exp.get("display") and norm(self.exp["display"]) == norm(dn)):
                problems.append(f"the new chat's rail said `{dn}`, then `{model}` ran")
        if problems:
            self.fail("rail", "; ".join(problems))
        else:
            self.ok("rail", f"`{rr.get('head')}`")
        # default model
        e = self.exp
        if self.key in ("auto", "coder-loop"):
            self.na("default-model", f"no single default; ran `{model}`")
        elif not model:
            self.fail("default-model", "no model recorded for the turn")
        elif self.key == "devin-cli" and norm(strip_params(model)) != norm(e.get("default", "")) and (refused := devin_refused(self.home, start)):
            # Coder asked for the model (`devin acp --model`); Devin's own
            # server refused it for this account. Not Coder's to fix.
            self.skip(
                "default-model",
                f"Devin's server here won't run `{e.get('default')}` for Coder (its log: “{refused}”), so `{model}` ran and the rail says so. Owner: allow it for the Devin account (NEEDS_OWNER.md)",
            )
        elif e.get("never") and re.search(e["never"], model, re.I):
            self.fail("default-model", f"ran `{model}` ({e['rule']})")
        elif e.get("family") and not (norm(model).startswith(norm(e["family"])) or norm(strip_params(model)) == norm(e.get("default", ""))):
            self.fail("default-model", f"ran `{model}`; want {e['rule']}")
        elif e.get("default") and not e.get("family") and norm(strip_params(model).split("/")[-1]) != norm(strip_params(e["default"]).split("/")[-1]):
            self.fail("default-model", f"ran `{model}`; want `{e['default']}` ({e['rule']})")
        elif not e:
            self.na("default-model", f"no rule for {self.key}; ran `{model}`")
        else:
            self.ok("default-model", f"`{model}`")
        # thinking
        nat = native_reasoning(self.key, self.home, start)
        reasoning_tokens = nat["tokens"]
        texts = list(nat["texts"])
        for st in agent_msgs:
            if st.get("reasoning_content"):
                texts.append(str(st["reasoning_content"]))
            for call in ((st.get("extra") or {}).get("model_calls") or []):
                reasoning_tokens = max(reasoning_tokens, int(call.get("reasoning_tokens") or 0))
            reasoning_tokens = max(reasoning_tokens, int(((st.get("metrics") or {}).get("extra") or {}).get("reasoning_tokens") or 0))
        # Coder draws the thinking as Markdown: `**Bold**` shows as `Bold`.
        frames = unmark(" ".join(words(" ".join(transcript(f))) for t in turns for f in t["frames"]))
        self.facts["reasoning_tokens"] = reasoning_tokens
        if not texts and reasoning_tokens == 0:
            self.na("thinking", "the model reported no reasoning")
        elif not texts:
            self.fail("thinking", f"the model reasoned ({reasoning_tokens} tokens) but no thinking text reached Coder")
        else:
            found = False
            for text in texts:
                w = unmark(words(text)).split(" ")
                for i in range(0, max(1, len(w) - 4)):
                    frag = " ".join(w[i : i + 4])
                    if len(frag) > 12 and frag in frames:
                        found = True
                        break
                if found:
                    break
            if found:
                self.ok("thinking", f"thinking streamed ({reasoning_tokens} reasoning tokens)")
            else:
                self.fail("thinking", f"the model reasoned (“{words(texts[0])[:60]}…”) but none of it showed in the transcript")
        # clean transcript
        dirty = []
        for n, t in enumerate(turns, 1):
            body = "\n".join(transcript(t["final"]))
            for rx, what in FORBIDDEN:
                m = rx.search(body)
                if m:
                    dirty.append(f"turn {n}: {what} (“{m.group(0)[:60]}”)")
        for st in steps:
            for rx, what in FORBIDDEN:
                m = rx.search(str(st.get("message", "")))
                if m:
                    dirty.append(f"export step {st.get('step_id')}: {what}")
        if dirty:
            self.fail("clean", "; ".join(dict.fromkeys(dirty))[:300])
        else:
            self.ok("clean", "replies and transcript hold only the conversation")
        # one lane: harness, model and payer per turn
        lanes = []
        for t in turns:
            c = (t["ended"] or {}).get("chose") or {}
            en = (t["ended"] or {}).get("ended") or {}
            # Who actually paid, as the turn's end records it.
            lanes.append((c.get("harness") or en.get("harness"), strip_params(en.get("model") or ""), en.get("payer") or c.get("payer")))
        self.facts["lanes"] = lanes
        if len(set(lanes)) > 1:
            picked = self.facts.get("picked_mid_chat", {})
            self.fail(
                "one-lane",
                " → ".join(f"{h}/{m}/{p}" for h, m, p in lanes)
                + (f" (after picking `{picked.get('id')}` in /models)" if picked else ""),
            )
        elif lanes:
            self.ok("one-lane", f"{lanes[0][0]}/{lanes[0][1]}/{lanes[0][2]} on every turn")
        # export matches the screen
        mism = []
        users = [st.get("message") for st in steps if st.get("source") == "user"]
        if not export:
            mism.append("/export wrote nothing")
        else:
            if users[: len(turns)] != [t["prompt"] for t in turns]:
                mism.append(f"user steps {users[:3]} ≠ prompts")
            final_screen = turns[-1]["final"] if turns else ""
            on_screen = " ".join(answers(final_screen))
            for st in agent_msgs:
                msg = words(str(st.get("message", "")))
                if msg and msg[:40] not in on_screen and words(msg)[:25] not in on_screen:
                    mism.append(f"step {st.get('step_id')} “{msg[:40]}” not on screen")
            for n, (a, b) in enumerate(zip(ran, export_models), 1):
                if a and b and strip_params(a) != strip_params(b):
                    mism.append(f"turn {n}: ran `{a}`, export says `{b}`")
            if len(agent_msgs) < len(turns) and not self.refused:
                mism.append(f"{len(agent_msgs)} agent steps for {len(turns)} turns")
        if mism:
            self.fail("export", "; ".join(mism)[:300])
        else:
            self.ok("export", f"{len(steps)} steps match the screen")
        # usage windows on the rail (plan-backed harnesses)
        self.usage_check(rail_after)
        # model_calls: a model and a duration on every call
        calls = [c for st in agent_msgs for c in ((st.get("extra") or {}).get("model_calls") or [])]
        badc = [c for c in calls if not c.get("model") or not c.get("duration_ms")]
        if not calls and self.key not in ("codex", "claude-code", "auto", "coder-loop"):
            self.na("model-calls", "an ACP agent reports no model calls")
        elif not calls:
            self.fail("model-calls", "no model_calls in the export")
        elif badc:
            self.fail("model-calls", f"{len(badc)} of {len(calls)} calls with model `{badc[0].get('model')}` and duration_ms {badc[0].get('duration_ms')}")
        else:
            self.ok("model-calls", f"{len(calls)} calls, each with a model and duration")
        # prompt tokens
        # The first turn's first model call: a thinking step ahead of the
        # reply carries no metrics of its own.
        first_prompt = None
        first_turn = []
        for st in steps[1:]:
            if st.get("source") == "user":
                break
            first_turn.append(st)
        for st in first_turn:
            calls_here = (st.get("extra") or {}).get("model_calls") or []
            if calls_here and calls_here[0].get("prompt_tokens") is not None:
                first_prompt = calls_here[0]["prompt_tokens"]
                break
            if (st.get("metrics") or {}).get("prompt_tokens") is not None:
                first_prompt = st["metrics"]["prompt_tokens"]
                break
        if first_prompt == 0:
            first_prompt = None
        self.facts["first_prompt_tokens"] = first_prompt
        lane = lanes[0][0] if lanes else None
        if self.key == "coder-loop" or (self.key == "auto" and lane == "coder-loop"):
            if first_prompt is None:
                self.fail("prompt-tokens", "no prompt tokens recorded")
            elif first_prompt > self.budget:
                self.fail("prompt-tokens", f"{first_prompt} prompt tokens for a one-line question (budget {self.budget})")
            else:
                self.ok("prompt-tokens", f"{first_prompt} ≤ {self.budget}")
        else:
            self.na("prompt-tokens", f"not the Coder loop ({first_prompt} prompt tokens, for the record)")
        # billing
        if self.key == "coder-loop":
            claude = [i for i in self.facts.get("gateway_models", []) if "claude" in (i.get("id") or "").lower()]
            if not claude:
                self.na("billing", "no Claude model in the Coder loop's list")
            else:
                payer = lanes[0][2] if lanes else None
                self.na("billing", f"payer `{payer}`; the Claude-model billing turn needs the signed-in run")
        else:
            self.na("billing", "Coder loop only")
        if self.exp.get("no_chat"):
            self.no_chat_checks(turns, r0, rail_after)
        if self.refused:
            self.skip("click-model", "no reply to click: the provider refused every turn (see answer)")

    def no_chat_checks(self, turns, r0, rail_after) -> None:
        """Oh My Pi with no chat-capable model signed in (`omp models` lists
        only clef*): nothing can answer, which is the owner's login to add,
        not Coder's to fix. Coder must say so plainly, on screen and on the
        rail, and never name a Clef model."""
        why = (
            "Oh My Pi has no chat-capable model signed in on this Mac (`omp models` lists only "
            + ", ".join(self.exp.get("catalog") or ["nothing"])
            + "), so it can't answer. Owner: sign in to a provider in Oh My Pi (NEEDS_OWNER.md)"
        )
        plain = "no model here that can chat"
        screens = " ".join(words(t["final"]) for t in turns)
        if turns and plain in screens and not re.search(r"\bclef", " ".join(transcript(turns[-1]["final"])), re.I):
            self.skip("answer", why + "; every turn said so plainly")
        else:
            self.fail("answer", "no chat model here, and the screen didn't say so plainly")
        rails = [r for r in (r0, rail_after) if r]
        heads = [r.get("head") or r.get("label") or "" for r in rails]
        # A new chat's rail may name no model while Oh My Pi's list is read;
        # after a turn it must say there's none, and never name a Clef one.
        if (
            heads
            and "no chat model" in heads[-1]
            and all(not re.search(r"clef", h, re.I) and (h.endswith("(no chat model)") or "(" not in h) for h in heads)
        ):
            self.ok("rail", f"`{heads[-1]}`")
        else:
            self.fail("rail", "the rail should say there's no chat model, never a Clef model: " + "; ".join(f"`{h}`" for h in heads))
        for check in ("default-model", "thinking", "export", "click-model"):
            self.skip(check, why)
        for check in ("models-new", "models-turn"):
            notice = self.facts.get(f"{check}-screen", "")
            if plain in notice:
                self.ok(check, "/models says plainly that Oh My Pi has no model here that can chat")
            elif self.results.get(check, ("",))[0] != "pass":
                self.fail(check, "/models on Oh My Pi with no chat model didn't say so plainly")

    def usage_check(self, r: dict | None) -> None:
        if self.key not in ("codex", "claude-code"):
            self.na("usage", "not a plan-backed harness")
            return
        label = (r or {}).get("label", "")
        parts = [p.strip() for p in label.split(" · ")[1:]]
        windows = []
        for p in parts:
            if p.startswith("Context"):
                continue
            m = re.fullmatch(r"(?:Codex |Claude )?(\S+) (<1|\d+)%", p)
            if m:
                windows.append((m.group(1), 0 if m.group(2) == "<1" else int(m.group(2))))
        if not windows:
            self.fail("usage", f"no plan windows on the rail (`{label}`)")
            return
        bad = [f"`{w} {v}%`" for w, v in windows if w not in ("5h", "week", "day", "month") or re.fullmatch(r"0\w", w)]
        truth = self.provider_windows()
        if truth:
            have = dict(windows)
            for w, v in truth.items():
                if w not in have:
                    bad.append(f"provider reports {w} {v}% but the rail doesn't show it")
                elif abs(have[w] - v) > 1:
                    bad.append(f"rail {w} {have[w]}% but the provider says {v}%")
        if bad:
            self.fail("usage", "; ".join(bad) + f" (rail `{label}`)")
        else:
            self.ok("usage", " · ".join(f"{w} {v}%" for w, v in windows) + (" (matches the provider)" if truth else ""))

    def provider_windows(self) -> dict:
        """Codex's own rate-limit windows from its session log."""
        if self.key != "codex":
            return {}
        last = None
        for f in (self.home / ".codex" / "sessions").rglob("*.jsonl"):
            for line in f.read_text(errors="replace").splitlines():
                if '"rate_limits"' in line:
                    try:
                        p = json.loads(line).get("payload", {})
                        last = (p.get("rate_limits") or (p.get("info") or {}).get("rate_limits")) or last
                    except ValueError:
                        pass
        out = {}
        for key in ("primary", "secondary"):
            w = (last or {}).get(key)
            if not w or w.get("window_minutes") is None:
                continue
            mins = int(w["window_minutes"])
            name = {300: "5h", 1440: "day", 10080: "week"}.get(mins, f"{mins}m")
            out[name] = round(float(w.get("used_percent") or 0))
        return out

    def report(self) -> dict:
        for c in CHECKS:
            self.results.setdefault(c, ("n/a", "not reached"))
        return {"harness": self.key, "results": {k: list(v) for k, v in self.results.items()}, "facts": self.facts}


def billing_run(coder, root, path, signin, evidence, turn_timeout) -> tuple[str, str]:
    """A Claude model picked in the Coder loop on a computer with a Claude
    login: the turn's payer is the Claude plan, not the gateway credit."""
    if not signin:
        return ("n/a", "not signed in: no OpenAgents sign-in key for the gate")
    if not which("claude", path):
        return ("n/a", "no Claude login on this computer")
    spec = {"key": "billing", "rail": ["Auto"], "bins": []}
    r = Run(spec, coder, root, path, {}, signin, 0, turn_timeout, evidence)
    r.home = make_home(root, "billing", [h["key"] for h in HARNESSES])
    r.state = r.home / "state"
    env = harness_env(r.home, path)
    if not r.login(env):
        return ("fail", "coder login --key failed")
    tui = Tui("h-billing", coder, r.home, r.state, env)
    try:
        if not tui.wait(lambda s: rail(s) is not None, 40):
            return ("fail", "Coder's screen never came up")
        tui.text("/models")
        tui.keys("Enter")
        if not tui.wait(picker_open, 6):
            return ("fail", "/models didn't open on Auto while signed in")
        tui.text("claude")
        time.sleep(1.0)
        s = tui.screen()
        mid = picker_field(s, "ID") or ""
        if "claude" not in mid.lower():
            tui.keys("Escape")
            return ("n/a", "the Coder loop lists no Claude model")
        tui.keys("Enter")
        time.sleep(1.0)
        t = r.send(tui, "what's 2+2?", 1)
        c = (t["ended"] or {}).get("chose") or {}
        payer = c.get("payer") or ""
        if not t["ended"]:
            return ("fail", f"the turn on `{mid}` didn't end")
        if "claude" in payer.lower() or "anthropic" in payer.lower():
            return ("pass", f"`{mid}` ran on `{payer}` via {c.get('harness')}")
        return ("fail", f"`{mid}` ran on payer `{payer}` via {c.get('harness')}, not the Claude plan")
    finally:
        tui.stop()


def plugins_run(coder, root, path, evidence) -> dict:
    """/plugins: every row toggles off and back on from the screen, and the
    details pane shows no raw publisher id or extensions path."""
    ev = evidence / "plugins"
    ev.mkdir(parents=True, exist_ok=True)
    home = make_home(root, "plugins", ALL_LOGINS)
    env = harness_env(home, path)
    tui = Tui("h-plugins", coder, home, home / "state", env)
    out = {}
    try:
        if not tui.wait(lambda s: rail(s) is not None, 40):
            return {"plugins-toggle": ["fail", "Coder's screen never came up"], "plugins-details": ["n/a", ""]}
        tui.text("/plugins")
        time.sleep(0.3)
        tui.keys("Enter")
        s = tui.wait(lambda s: "Up/Down Select" in s, 8)
        if not s:
            return {"plugins-toggle": ["fail", "/plugins didn't open"], "plugins-details": ["n/a", ""]}

        def rows(screen):
            out, on = [], False
            for line in screen.splitlines():
                if re.match(r"^\s*Plugin\s+Enabled\s+Status", line):
                    on = True
                    continue
                if on:
                    m = re.match(r"^\s*(❯)?\s*(.+?)\s{2,}\[\s*(on|off)\s*\]\s*(.*)$", line)
                    if not m:
                        break
                    out.append({"sel": bool(m.group(1)), "name": m.group(2).strip(), "on": m.group(3) == "on", "status": m.group(4).strip()})
            return out

        def details(screen):
            lines = screen.splitlines()
            idx = next((i for i, l in enumerate(lines) if "Up/Down Select" in l), None)
            return "\n".join(lines[idx + 1 :]) if idx is not None else ""

        time.sleep(2.0)
        first = rows(tui.screen())
        (ev / "plugins.txt").write_text(tui.screen())
        stuck, leaks, log = [], [], []
        for i, row in enumerate(first):
            cur = rows(tui.screen())
            sel = next((j for j, r in enumerate(cur) if r["sel"]), 0)
            for _ in range(i - sel):
                tui.keys("Down")
                time.sleep(0.15)
            time.sleep(0.4)
            scr = tui.screen()
            d = details(scr)
            if re.search(r"\b0{64}\b", d) or re.search(r"/\.openagents/extensions|/extensions/[0-9a-f]{16,}", d):
                leaks.append(row["name"])
            before = next((r for r in rows(scr) if r["sel"]), None)
            if not before:
                stuck.append(f"{row['name']} (couldn't select it)")
                continue
            states = [before["on"]]
            for _ in range(2):
                tui.keys("Space")
                time.sleep(1.2)
                scr2 = tui.screen()
                if "Up/Down Select" not in scr2:
                    log.append(f"{row['name']}: Space opened another screen")
                    (ev / f"plugin-{i}-other.txt").write_text(scr2)
                    tui.keys("Escape")
                    time.sleep(0.6)
                    scr2 = tui.screen()
                now = next((r for r in rows(scr2) if r["sel"]), None)
                states.append(now["on"] if now else None)
            (ev / f"plugin-{i}.txt").write_text(tui.screen())
            if not (states[1] is not None and states[1] != states[0] and states[2] == states[0]):
                why = before["status"]
                stuck.append(f"{row['name']} ({'on' if states[0] else 'off'} → {states[1:]}; {why})")
        (ev / "log.txt").write_text("\n".join(log))
        out["plugins-toggle"] = ["fail", "didn't toggle off and back on: " + "; ".join(stuck)] if stuck else ["pass", f"all {len(first)} rows toggled off and on"]
        out["plugins-details"] = ["fail", "raw publisher id or extensions path in: " + ", ".join(leaks)] if leaks else ["pass", f"{len(first)} details panes clean"]
        names = [r["name"] for r in first]
        if (REAL_HOME / ".openagents" / "extensions").is_dir() and not any("disk" in n.lower() for n in names):
            out["plugins-toggle"] = ["fail", "installed extension plugin Disk cleanup isn't listed; " + out["plugins-toggle"][1]]
        return out
    finally:
        tui.stop()




def claims_repo_required(answer: str) -> bool | None:
    """Whether a reply says a new project needs a repository, judged by the
    Decisions API (Clef first) rather than by matching words: a correct
    answer such as "a name is required, plus any repos you want" names both
    words without claiming it (rc.11 gate run 2026-10-14). None when the
    judge can't be reached, so the check is skipped rather than guessed."""
    token = os.environ.get("OPENAGENTS_TOKEN") or os.environ.get("GATE_OPENAGENTS_TOKEN")
    body = {
        "model": "openagents/decide",
        "input": answer,
        "questions": [{
            "type": "predicate",
            "name": "repo_required",
            "instructions": "Does this reply say that creating a new project requires a GitHub "
                            "repository (a project can't be made without one)?",
            "true": "It says a repository is needed or required to make a project.",
            "false": "It says only a name is needed, or that repositories are optional or can be added.",
        }],
    }
    headers = {"Content-Type": "application/json", "X-OpenAgents-Test": "1"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(
        os.environ.get("OPENAGENTS_ORIGIN", "https://openagents.com") + "/api/v1/decisions",
        data=json.dumps(body).encode(), headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            answers = json.loads(resp.read()).get("answers") or []
    except Exception:
        return None
    probability = next((a.get("probability") for a in answers if a.get("type") == "predicate"), None)
    return None if probability is None else probability >= 0.5


def no_home_run(coder, root, path, signin, evidence, turn_timeout) -> dict:
    """Windows' shape: HOME unset, only USERPROFILE set. A first message
    must get an answer without "HOME is not set" (Windows rc.7)."""
    spec = {"key": "no-home", "rail": ["Auto"], "bins": []}
    r = Run(spec, coder, root, path, {}, signin, 0, turn_timeout, evidence)
    env = harness_env(r.home, path)
    r.login(env)
    env.pop("HOME", None)
    env["USERPROFILE"] = str(r.home)
    tui = Tui("h-no-home", coder, r.home, r.state, env, unset=("HOME",))
    try:
        if not tui.wait(lambda s: rail(s) is not None, 40):
            screen = tui.screen()
            (r.ev / "startup.txt").write_text(screen)
            return {"no-home": ["fail", "Coder's screen never came up: " + words(screen)[-160:]]}
        t = r.send(tui, "what's 2+2?", 1)
        err = (r.home / "stderr-h-no-home.log").read_text(errors="replace") if (r.home / "stderr-h-no-home.log").exists() else ""
        text = "\n".join(t["frames"]) + err
        ans = answers(t["final"])
        if re.search(r"HOME is not set|\$HOME", text):
            return {"no-home": ["fail", "“HOME is not set” with only USERPROFILE set"]}
        if not t["ended"] or not ans:
            return {"no-home": ["fail", f"no answer in {t['secs']}s with only USERPROFILE set: " + words(" ".join(transcript(t["final"])))[-160:]]}
        lane = ((t["ended"] or {}).get("chose") or {}).get("harness")
        return {"no-home": ["pass", f"“{ans[-1][:60]}” on {lane}" + ("" if signin else " (not signed in, so not the gateway)")]}
    finally:
        tui.stop()


def projects_run(coder, root, path, signin, evidence, turn_timeout) -> dict:
    """Projects: Coder's answer to "can you make a new project, what do you
    need" claims no repository is required; the live account reads
    read-only; a local or test server takes the full create flow."""
    out = {}
    ev = evidence / "projects"
    ev.mkdir(parents=True, exist_ok=True)
    spec = {"key": "projects", "rail": ["Auto"], "bins": []}
    r = Run(spec, coder, root, path, {}, signin, 0, turn_timeout, evidence)
    env = harness_env(r.home, path)
    r.login(env)
    tui = Tui("h-projects", coder, r.home, r.state, env)
    try:
        if not tui.wait(lambda s: rail(s) is not None, 40):
            out["projects-chat"] = ["fail", "Coder's screen never came up"]
        else:
            t = r.send(tui, "can you make a new project, what do you need", 1)
            ans = " ".join(a for a in answers(t["final"]) if not a.startswith("mcp__"))
            if not t["ended"] or not ans:
                out["projects-chat"] = ["fail", f"no answer in {t['secs']}s"]
            elif (verdict := claims_repo_required(ans)) is None:
                out["projects-chat"] = ["skipped", "the Decisions judge couldn't be reached"]
            elif verdict:
                out["projects-chat"] = ["fail", f"claims a repository is required: “{ans[:160]}”"]
            else:
                out["projects-chat"] = ["pass", f"“{ans[:120]}”"]
    finally:
        tui.stop()
    # The live account, read-only.
    oa = Path(coder).parent / "openagents"
    if not signin:
        out["projects-live"] = ["n/a", "not signed in: no gate sign-in key"]
    elif not os.access(oa, os.X_OK):
        out["projects-live"] = ["n/a", f"no openagents next to {coder}"]
    else:
        out["projects-live"] = projects_live(oa, harness_env(r.home, path, {"OPENAGENTS_TOKEN": signin}), ev)
    # A local or test server: the full flow.
    origin, token = os.environ.get("HACC_PROJECTS_ORIGIN"), os.environ.get("HACC_PROJECTS_TOKEN")
    if not (origin and token):
        out["projects-local"] = ["n/a", "no local server (set HACC_PROJECTS_ORIGIN and HACC_PROJECTS_TOKEN, e.g. scripts/dev/full-local.sh)"]
    else:
        out["projects-local"] = projects_local(origin.rstrip("/"), token, ev)
    return out


def projects_live(oa: Path, env: dict, ev: Path) -> list:
    """The live account: list, then make a `zz-gate-*` project with just a
    name, see it listed with no chats, and remove it. The gate touches only
    `zz-gate-*` projects, never a chat."""
    log = []

    def call(*args):
        code, text = run([str(oa), "--json", "projects", *args], env=env, timeout=60)
        shown = re.sub(r"oa_sk_\w+", "oa_sk_…", text)
        log.append(f"$ openagents projects {' '.join(args)}\n{code}\n{shown}")
        try:
            return code, json.JSONDecoder().raw_decode(text[text.index("{"):])[0]
        except ValueError:
            return code, text

    def listed(doc):
        return doc.get("data", []) if isinstance(doc, dict) else []

    try:
        code, doc = call("list")
        if code != 0:
            return ["fail", f"projects list exited {code}: {str(doc).strip()[:120]}"]
        # Leftovers of an earlier, interrupted gate run.
        for p in listed(doc):
            if str(p.get("name", "")).startswith("zz-gate-") and p.get("id"):
                call("remove", p["id"])
        name = f"zz-gate-{time.strftime('%Y%m%d-%H%M%S')}"
        code, made = call("create", name)
        pid = made.get("id") if isinstance(made, dict) else None
        if code != 0 or not pid:
            return ["fail", f"creating {name} with just a name: {code} {str(made)[:120]}"]
        bad = []
        code, doc = call("list")
        row = next((p for p in listed(doc) if p.get("id") == pid), None)
        if not row:
            bad.append(f"{name} isn't in the list")
        elif (row.get("threads") or row.get("thread_count") or row.get("chats") or 0) not in (0, [], None):
            bad.append(f"{name} has chats: {row.get('threads') or row.get('thread_count') or row.get('chats')}")
        code, gone = call("remove", pid)
        if code != 0:
            bad.append(f"removing {name}: {code} {str(gone)[:100]}")
        code, doc = call("list")
        if any(p.get("id") == pid for p in listed(doc)):
            bad.append(f"{name} is still listed after remove")
        return ["fail", "; ".join(bad)] if bad else ["pass", f"listed, made `{name}` with just a name, removed it"]
    finally:
        (ev / "live.txt").write_text("\n\n".join(log))


def projects_local(origin: str, token: str, ev: Path) -> list:
    import urllib.request

    def call(method, url, body=None):
        req = urllib.request.Request(origin + url, method=method, data=json.dumps(body).encode() if body is not None else None)
        req.add_header("Authorization", f"Bearer {token}")
        req.add_header("Content-Type", "application/json")
        req.add_header("X-OpenAgents-Test", "1")
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return resp.status, json.loads(resp.read() or b"{}")
        except urllib.error.HTTPError as e:
            return e.code, (e.read() or b"")[:300].decode(errors="replace")
        except OSError as e:
            return 0, str(e)

    stamp = time.strftime("%H%M%S")
    made, steps, bad = [], [], []
    code, p1 = call("POST", "/v1/projects", {"name": f"gate {stamp}"})
    steps.append(f"name only: {code}")
    if code in (200, 201) and isinstance(p1, dict):
        made.append(p1.get("id"))
    else:
        bad.append(f"a project with just a name: {code} {str(p1)[:100]}")
    code, lst = call("GET", "/v1/projects")
    repos = [p.get("repository") for p in (lst.get("data") or []) if isinstance(lst, dict) and p.get("repository")] if code == 200 else []
    if repos:
        code, p2 = call("POST", "/v1/projects", {"name": f"gate {stamp} again", "repository": repos[0]})
        steps.append(f"shared repository: {code}")
        if code in (200, 201) and isinstance(p2, dict):
            made.append(p2.get("id"))
        else:
            bad.append(f"a second project on {repos[0]}: {code} {str(p2)[:100]}")
    else:
        bad.append("no project with a repository on the server to share")
    if made and made[0] and len(set(repos)) >= 1:
        two = list(dict.fromkeys(repos))[:2]
        code, p3 = call("PATCH", f"/v1/projects/{made[0]}", {"repositories": two, "primary_repository": two[0]})
        steps.append(f"two repos + primary: {code}")
        if code != 200:
            bad.append(f"adding repositories and a primary: {code} {str(p3)[:100]}")
    for pid in made:
        if pid:
            call("DELETE", f"/v1/projects/{pid}")
    (ev / "local.txt").write_text("\n".join(steps + bad))
    return ["fail", "; ".join(bad)] if bad else ["pass", "; ".join(steps)]


def web_run(evidence) -> dict:
    """A web chat on a local site (scripts/dev/full-local.sh): the answer
    has no "Run in Coder on <computer>" button, one message makes one
    thread, and the composer has no Chat/Code mode switch."""
    import html as htmlmod
    import http.cookiejar
    import urllib.parse
    import urllib.request
    import uuid

    base = os.environ.get("HACC_WEB_ORIGIN", "http://127.0.0.1:4301").rstrip("/")
    gateway = os.environ.get("HACC_GATEWAY_ORIGIN", "http://127.0.0.1:8791").rstrip("/")
    ev = evidence / "web"
    ev.mkdir(parents=True, exist_ok=True)

    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a, **k):
            return None

    jar = http.cookiejar.CookieJar()
    opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar), NoRedirect)

    def get(path):
        try:
            with opener.open(base + path, timeout=30) as r:
                return r.status, r.read().decode(errors="replace")
        except urllib.error.HTTPError as e:
            return e.code, (e.read() or b"").decode(errors="replace")

    try:
        code, home = get("/")
    except OSError as e:
        return {"web-chat": ["n/a", f"no local site at {base} (start scripts/dev/full-local.sh; {e.__class__.__name__})"]}
    # A test account from the account service's own sign-up, when it runs.
    signed = False
    try:
        req = urllib.request.Request(gateway + "/v1/accounts", data=b'{"label":"Harness gate"}', method="POST", headers={"content-type": "application/json"})
        with urllib.request.urlopen(req, timeout=15) as r:
            body = json.loads(r.read())
        token = body.get("session") or body.get("token") or next((v for v in body.values() if isinstance(v, str) and v.startswith("sess_")), None)
        if token:
            jar.set_cookie(http.cookiejar.Cookie(0, "oa_cloud_session", token, None, False, urllib.parse.urlparse(base).hostname, False, False, "/", True, False, None, False, None, None, {}))
            signed = True
            code, home = get("/")
    except (OSError, ValueError):
        pass
    fields = {}
    for tag in home.split("<input")[1:]:
        tag = tag.split(">")[0]
        if 'type="hidden"' in tag:
            n = re.search(r' name="([^"]*)"', tag)
            v = re.search(r' value="([^"]*)"', tag)
            if n and v:
                fields.setdefault(n.group(1), htmlmod.unescape(v.group(1)))
    if "csrf" not in fields:
        return {"web-chat": ["fail", f"GET / has no chat form ({code})"]}
    threads_before = set(re.findall(r'href="/chat/([\w-]+)"', home))
    chat = str(uuid.uuid4())
    form = urllib.parse.urlencode({"q": "what's 2+2?", "request_id": chat, "csrf": fields["csrf"], "selection": fields.get("selection", ""), "project": ""}).encode()
    req = urllib.request.Request(base + "/chat", data=form, method="POST", headers={"Origin": base, "Sec-Fetch-Site": "same-origin", "content-type": "application/x-www-form-urlencoded"})
    try:
        opener.open(req, timeout=30)
    except urllib.error.HTTPError as e:
        if e.code >= 400:
            return {"web-chat": ["fail", f"POST /chat: {e.code}"]}
    t0 = time.time()
    page = ""
    while time.time() - t0 < 150:
        code, page = get(f"/chat/{chat}/transcript")
        at = page.find('id="chat-status"')
        busy = at >= 0 and "oa-busy" in page[at : page.find("</div>", at) if page.find("</div>", at) > 0 else None]
        if "data-oa-reply=" in page and not busy:
            break
        time.sleep(0.5)
    code, full = get(f"/chat/{chat}")
    code2, home2 = get("/")
    (ev / "transcript.html").write_text(page)
    (ev / "chat.html").write_text(full)
    bad = []
    if "data-oa-reply=" not in page:
        bad.append("no reply in 150 s")
    if re.search(r"Run in Coder on", page + full):
        bad.append("the answer offers “Run in Coder on …”")
    new_threads = set(re.findall(r'href="/chat/([\w-]+)"', home2)) - threads_before
    if len(new_threads) > 1:
        bad.append(f"{len(new_threads)} threads for one message: {sorted(new_threads)[:3]}")
    switch = re.findall(r"<(button|label|option|a|span)[^>]*>\s*(Chat|Code|Work)\s*</\1>", full)
    if {"Chat", "Code"} <= {w for _, w in switch}:
        bad.append("the composer has a Chat/Code mode switch")
    who = "a test account" if signed else "a visitor"
    return {"web-chat": ["fail", "; ".join(bad) + f" (as {who})"] if bad else ["pass", f"one thread, a plain answer, no mode switch (as {who})"]}


# ---------------------------------------------------------------- report


def write_report(out: Path, build: str, coder: str, rows: list[dict], exp: dict, extra: dict) -> tuple[int, int]:
    fails = sum(1 for r in rows for v in r["results"].values() if v[0] == "fail")
    fails += sum(1 for v in extra.get("scenarios", {}).values() if v[0] == "fail")
    passes = sum(1 for r in rows for v in r["results"].values() if v[0] == "pass")
    doc = {
        "build": build,
        "coder": coder,
        "at": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "prompts": PROMPTS,
        "checks": CHECKS,
        "expectations": {k: {kk: vv for kk, vv in v.items() if kk not in ("catalog", "names")} for k, v in exp.items()},
        "harnesses": rows,
        "missing_beside_coder": [n for n in ("openagents", "microcoder") if not os.access(Path(coder).parent / n, os.X_OK)],
        **extra,
        "passed": fails == 0,
    }
    (out / "report.json").write_text(json.dumps(doc, indent=2))
    mark = {"pass": "pass", "fail": "**FAIL**", "n/a": "n/a", "not installed": "not installed", "skipped": "skipped"}
    lines = [
        f"# Coder harness acceptance: {build}",
        "",
        f"`{coder}`, {doc['at']}. {'PASSED' if fails == 0 else f'FAILED: {fails} failing checks'} ({passes} passing)."
        + (f" Not beside it: {', '.join(doc['missing_beside_coder'])}." if doc["missing_beside_coder"] else ""),
        "",
        "| check | " + " | ".join(r["harness"] for r in rows) + " |",
        "|---|" + "---|" * len(rows),
    ]
    for c in CHECKS:
        lines.append(f"| {c} | " + " | ".join(mark.get(r["results"][c][0], r["results"][c][0]) for r in rows) + " |")
    scen = extra.get("scenarios", {})
    if scen:
        lines += ["", "| scenario | result | evidence |", "|---|---|---|"]
        for k, (st, ev) in scen.items():
            lines.append(f"| {k} | {mark.get(st, st)} | {ev.replace('|', '/')[:220]} |")
    lines += ["", "## Evidence", ""]
    for r in rows:
        lines.append(f"### {r['harness']}")
        lines.append("")
        for c in CHECKS:
            st, ev = r["results"][c]
            lines.append(f"- {c}: {mark.get(st, st)}. {ev}")
        lines.append("")
    lines += ["## Expected defaults", ""]
    for k, v in exp.items():
        lines.append(f"- {k}: {v.get('rule')}")
    lines += [
        "",
        "## Manual",
        "",
        "- Read the turn captures in `evidence/<harness>/` once per release: the gate reads text, not colour or layout.",
    ]
    (out / "report.md").write_text("\n".join(lines) + "\n")
    return fails, passes


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--coder", default=str(REAL_HOME / ".openagents/bin/coder"))
    ap.add_argument("--out", default=str(REAL_HOME / "work/harness-acceptance"))
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--keep", action="store_true")
    ap.add_argument("--list", action="store_true")
    # The gate's sign-in key, never committed: the first of these files
    # that exists, the dedicated test account's first (#11601).
    ap.add_argument("--signin-key-file", default="")
    ap.add_argument("--prompt-budget", type=int, default=int(os.environ.get("HACC_PROMPT_BUDGET", "5000")))
    ap.add_argument("--turn-timeout", type=int, default=240)
    ap.add_argument("--fresh-state", action="store_true", help="don't start from the owner's harness facts")
    a = ap.parse_args()
    global SEED_FACTS
    SEED_FACTS = not a.fresh_state
    if a.list:
        for h in HARNESSES:
            print(h["key"])
        print("billing\nplugins\nprojects\nno-home\nweb")
        return 0
    for tool in ("tmux", "git"):
        if not shutil.which(tool):
            print(f"harness-acceptance: {tool} is required", file=sys.stderr)
            return 2
    coder = str(Path(a.coder).expanduser().resolve())
    if not os.access(coder, os.X_OK):
        print(f"harness-acceptance: no executable {coder}", file=sys.stderr)
        return 2
    code, ver = run([coder, "--version"], timeout=20)
    m = re.search(r"coder (\S+) \((\w{10})\w* (\w+)\)", ver)
    build = f"{m.group(1)}-{m.group(2)}" + ("" if m.group(3) == "clean" else f"-{m.group(3)}") if m else re.sub(r"\W+", "-", ver.strip())[:40]
    out = Path(a.out).expanduser() / build
    out.mkdir(parents=True, exist_ok=True)
    stamp = dt.datetime.now().strftime("%Y%m%dT%H%M%S")
    evidence = out / "evidence"
    if evidence.exists():
        shutil.rmtree(evidence)
    evidence.mkdir()
    path = owner_path()
    root = Path(tempfile.mkdtemp(prefix="hacc.", dir="/tmp"))
    say(f"build {build}; isolated homes under {root}; report in {out}")
    # Coder's turns call the openagents and microcoder beside it, as an
    # install has them; a lone binary tests less than ships.
    missing = [n for n in ("openagents", "microcoder") if not os.access(Path(coder).parent / n, os.X_OK)]
    if missing:
        say(f"warning: no {' or '.join(missing)} beside {coder}; tools that call them fail as on a broken install")

    signin = ""
    kf = Path(a.signin_key_file).expanduser() if a.signin_key_file else next(
        (f for f in SIGNIN_KEY_FILES if f.is_file()), SIGNIN_KEY_FILES[0]
    )
    if kf.is_file():
        for line in kf.read_text().splitlines():
            mm = re.match(r"^\s*(?:export\s+)?OPENAGENTS_TOKEN=['\"]?(oa_sk_[^'\"\s]+)", line)
            if mm:
                signin = mm.group(1)
    say("OpenAgents sign-in: " + ("a gate sign-in key" if signin else "none (Coder-loop checks are skipped)"))

    only = {x for x in a.only.split(",") if x}
    exp = expectations(path, root)
    specs = []
    rows = []
    for h in HARNESSES:
        if only and h["key"] not in only:
            continue
        if h["bins"] and not any(which(b, path) for b in h["bins"]):
            if h["key"] in ("goose", "kimi", "amp", "hermes"):
                continue
            rows.append({"harness": h["key"], "results": {c: ["not installed", ""] for c in CHECKS}, "facts": {}})
            continue
        if h["key"] == "cursor" and not cursor_headless(path, root):
            rows.append({"harness": "cursor", "results": {c: ["skipped", "no login available headless (cursor-agent status says signed out in an isolated home); never started, so it can't open a sign-in"] for c in CHECKS}, "facts": {}})
            continue
        if h["key"] == "amp" and not which("amp-acp", path):
            rows.append({"harness": "amp", "results": {c: ["not installed", "amp is installed without amp-acp, so Coder can't run it"] for c in CHECKS}, "facts": {}})
            continue
        specs.append(h)

    def one(h):
        say(f"{h['key']}: start")
        t0 = time.time()
        try:
            r = Run(h, coder, root, path, exp, signin, a.prompt_budget, a.turn_timeout, evidence).go()
        except Exception as e:  # a gate bug is a failed row, never a pass
            r = {"harness": h["key"], "results": {c: ["fail", f"gate error: {e!r}"] for c in CHECKS}, "facts": {}}
        bad = [c for c, v in r["results"].items() if v[0] == "fail"]
        say(f"{h['key']}: {'pass' if not bad else 'FAIL ' + ','.join(bad)} ({int(time.time() - t0)}s)")
        return r

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, a.jobs)) as pool:
        done = list(pool.map(one, specs))
    order = {h["key"]: i for i, h in enumerate(HARNESSES)}
    rows = sorted(rows + done, key=lambda r: order.get(r["harness"], 99))
    scenarios: dict = {}
    if not only or "billing" in only:
        say("billing: start")
        st, ev = billing_run(coder, root, path, signin, evidence, a.turn_timeout)
        scenarios["billing"] = [st, ev]
        for r in rows:
            if r["harness"] == "coder-loop":
                r["results"]["billing"] = [st, ev]
    if not only or "plugins" in only:
        say("plugins: start")
        try:
            scenarios.update(plugins_run(coder, root, path, evidence))
        except Exception as e:
            scenarios["plugins-toggle"] = ["fail", f"gate error: {e!r}"]
    if not only or "projects" in only:
        say("projects: start")
        try:
            scenarios.update(projects_run(coder, root, path, signin, evidence, a.turn_timeout))
        except Exception as e:
            scenarios["projects-chat"] = ["fail", f"gate error: {e!r}"]
    if not only or "no-home" in only:
        say("no-home: start")
        try:
            scenarios.update(no_home_run(coder, root, path, signin, evidence, a.turn_timeout))
        except Exception as e:
            scenarios["no-home"] = ["fail", f"gate error: {e!r}"]
    if not only or "web" in only:
        say("web: start")
        try:
            scenarios.update(web_run(evidence))
        except Exception as e:
            scenarios["web-chat"] = ["fail", f"gate error: {e!r}"]
    scenarios["click-model-manual"] = ["n/a", "also look once: click a reply, its model shows right-aligned in gray for ~3 s"]
    extra = {"scenarios": scenarios}
    fails, passes = write_report(out, build, coder, rows, exp, extra)
    (out / f"report-{stamp}.md").write_text((out / "report.md").read_text())
    subprocess.run(["tmux", "-L", SOCKET, "kill-server"], capture_output=True)
    if not a.keep:
        shutil.rmtree(root, ignore_errors=True)
    else:
        say(f"kept {root}")
    print((out / "report.md").read_text().split("## Evidence")[0])
    print(f"report: {out / 'report.md'}")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())

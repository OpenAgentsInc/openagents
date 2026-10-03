#!/usr/bin/env python3
"""Shadow-baseline study for #10209 on coderos-4080.

Arms:
  raw-claude       raw Claude Code CLI, default settings, in a copy of the repo
  routed-claude-on   `openagents chat send` routed to Claude Code, recipe on
  routed-codex-on    routed to Codex, recipe on
  routed-claude-off  routed to Claude Code, recipe off (controller shim)
  routed-codex-off   routed to Codex, recipe off
  routed-claude-lean routed to Claude Code as one lean session (#10246):
                     coder.claude = session, recipe on

Usage:
  study.py prepare
  study.py run ARMS TASKS TRIALS PARALLEL      (comma lists; TRIALS like 1,2,3)
  study.py one ARM TASK TRIAL
"""
import concurrent.futures as cf
import glob
import hashlib
import json
import os
import random
import shutil
import socket
import subprocess
import sys
import tempfile
import time

HOME = os.path.expanduser("~")
BASE = os.path.join(HOME, "shadow-10209")
TMPL = os.path.join(BASE, "templates")
RUNS = os.path.join(BASE, "runs")
RESULTS = os.path.join(BASE, "results.jsonl")
TB = os.path.join(HOME, ".openagents/terminal-bench/upstream/terminal-bench-2.1/tasks")
OA = os.path.join(HOME, "coder-runner/openagents")
OFF_SHIM = os.path.join(BASE, "bin/microcoder-recipe-off")
SRC = os.path.join(BASE, "src")
PY = shutil.which("python3")
TIMEOUT = 3600

GIT = ["git", "-c", "user.email=study@openagents.invalid", "-c", "user.name=Study"]


def sh(cmd, cwd=None, env=None, check=True, timeout=None):
    r = subprocess.run(cmd, cwd=cwd, env=env, shell=isinstance(cmd, str),
                       capture_output=True, text=True, timeout=timeout)
    if check and r.returncode != 0:
        raise RuntimeError(f"{cmd}: {r.returncode}\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")
    return r


def nixos_shim(venv):
    """Make manylinux wheels load on NixOS, as on an ordinary Linux system Python."""
    gcc = sh("gcc -print-file-name=libgcc_s.so.1").stdout.strip()
    cxx = os.path.realpath(sh("gcc -print-file-name=libstdc++.so.6").stdout.strip())
    zmod = sh([PY, "-c", "import zlib;print(zlib.__file__)"]).stdout.strip()
    z = [l.split()[2] for l in sh(["ldd", zmod]).stdout.splitlines() if "libz" in l][0]
    site = glob.glob(os.path.join(venv, "lib/python3*/site-packages"))[0]
    line = "import ctypes; " + "; ".join(
        f'ctypes.CDLL("{p}", mode=ctypes.RTLD_GLOBAL)' for p in (gcc, cxx, z))
    open(os.path.join(site, "00-nixos-libs.pth"), "w").write(line + "\n")


def make_venv(path, packages=()):
    sh([PY, "-m", "venv", path])
    nixos_shim(path)
    sh([os.path.join(path, "bin/pip"), "-q", "install", "--upgrade", "pip"])
    if packages:
        sh([os.path.join(path, "bin/pip"), "-q", "install", *packages])


def commit_all(repo, msg):
    sh(GIT + ["add", "-A"], cwd=repo)
    sh(GIT + ["commit", "-qm", msg], cwd=repo)


def truncated_repo(src, rev, dest):
    """A repository holding only `rev` and its ancestry: no later commits."""
    os.makedirs(dest)
    sh(["git", "init", "-q", "-b", "main"], cwd=dest)
    sha = sh(["git", "rev-parse", rev], cwd=src).stdout.strip()
    sh(["git", "fetch", "-q", src, f"{sha}:refs/heads/snapshot"], cwd=dest)
    sh(["git", "reset", "-q", "--hard", "snapshot"], cwd=dest)
    sh(["git", "branch", "-q", "-D", "snapshot"], cwd=dest)


# ---------------------------------------------------------------- tasks

def prep_fix_git(dest):
    # The upstream personal-site repository is gone; the task's prebuilt image
    # (alexgshaw/fix-git:20260403) holds the prepared checkout, copied out once.
    os.makedirs(dest)
    shutil.copytree(os.path.join(SRC, "fixgit/personal-site"), os.path.join(dest, "personal-site"), symlinks=True)
    return "personal-site"


def prep_fix_code_vulnerability(dest):
    sh(["git", "clone", "-q", "https://github.com/bottlepy/bottle.git", dest])
    sh("git reset -q --hard 0207a34f0c5716cd292dd4480253ad35d3da49f3 && git remote remove origin "
       "&& git tag -d $(git tag -l) >/dev/null; git reflog expire --expire=now --all && git gc -q --prune=now",
       cwd=dest)
    sh("sed -i -e '1562,1563d' bottle.py && sed -i -e '1567,1568d' bottle.py", cwd=dest)
    commit_all(dest, "Snapshot")
    return "."


def prep_headless_terminal(dest):
    os.makedirs(dest)
    sh(["git", "init", "-q", "-b", "main"], cwd=dest)
    shutil.copy(os.path.join(TB, "headless-terminal/environment/base_terminal.py"), dest)
    commit_all(dest, "Base terminal interface")
    return "."


def prep_build_cython_ext(dest):
    os.makedirs(dest)
    sh(["git", "init", "-q", "-b", "main"], cwd=dest)
    open(os.path.join(dest, "README.md"), "w").write("Workspace for building pyknotid.\n")
    commit_all(dest, "Workspace")
    return "."


def prep_repo(name, rev):
    def prep(dest):
        truncated_repo(os.path.join(SRC, name), rev, dest)
        return "."
    return prep


TB_NOTE = ""

TASKS = {
    "fix-git": dict(
        prep=prep_fix_git, venv=None,
        prompt=lambda v: open(os.path.join(TB, "fix-git/instruction.md")).read().strip()),
    "fix-code-vulnerability": dict(
        prep=prep_fix_code_vulnerability, venv=("pytest==8.4.1",),
        prompt=lambda v: (
            open(os.path.join(TB, "fix-code-vulnerability/instruction.md")).read()
            .split("# Terminal-Bench Canary")[0].strip()
            .replace("located in the /app folder", "in the current directory (the repository root)")
            .replace("in the /app/bottle.py file", "in the bottle.py file")
            .replace("create a /app/report.jsonl file in /app folder", "create a report.jsonl file in the repository root")
            .replace('{"file_path": "/app/example.cpp", "cwe_id": ["cwe-123"]}', '{"file_path": "example.cpp", "cwe_id": ["cwe-123"]}')
            .replace("The root path of the repo is /app", "The root path of the repo is the current directory; give file_path relative to it")
            .replace("in the /app/report.jsonl file", "in the report.jsonl file")
            .replace("you can run: `pytest -rA`", f"you can run: `{v}/bin/python -m pytest -rA` (the Python environment at {v} has pytest)"))),
    "headless-terminal": dict(
        prep=prep_headless_terminal, venv=(),
        prompt=lambda v: (
            open(os.path.join(TB, "headless-terminal/instruction.md")).read().strip()
            .replace("put it in a file called `/app/headless_terminal.py`", "put it in a file called `headless_terminal.py` in the repository root (the current directory)")
            .replace("Install dependencies into the system python.",
                     f"The system Python is the environment at {v}: install any dependencies into it ({v}/bin/pip) and use {v}/bin/python; the tests run with it."))),
    "build-cython-ext": dict(
        prep=prep_build_cython_ext, venv=("numpy==2.3.0",),
        prompt=lambda v: (
            open(os.path.join(TB, "build-cython-ext/instruction.md")).read()
            .split("# Terminal-Bench Canary")[0].strip()
            .replace("to `/app/pyknotid`", "to `pyknotid` in the repository root (the current directory)")
            .replace("/app/pyknotid/tests/", "pyknotid/tests/")
            .replace("system's global Python environment", f"system's global Python environment (here: the environment at {v}; use {v}/bin/python and {v}/bin/pip)")
            .replace("to system's global python environment", f"to the system's global python environment ({v})"))),
    "mi-seekable": dict(
        prep=prep_repo("more-itertools", "6b1907d^"), venv=None,
        prompt=lambda v: (
            "Bug: `more_itertools.seekable` with `maxlen=0` loses items. With "
            "`s = seekable([10, 20, 30], maxlen=0)`, `s.peek()` returns 10, but `list(s)` "
            "afterwards is missing 10; `bool(s)` drops the first item the same way. Fix it so "
            "`peek()` and `bool()` never drop items when `maxlen` is 0. Keep the existing tests "
            "passing (`python3 -m unittest tests.test_more`).")),
    "mi-one": dict(
        prep=prep_repo("more-itertools", "def2dab^"), venv=None,
        prompt=lambda v: (
            "Bug: `more_itertools.one()` and `only()` ignore a user-supplied `too_short` or "
            "`too_long` exception whose instances are falsy (for example an Exception subclass "
            "whose `__bool__` returns False): the default `ValueError` is raised instead. Also, "
            "when a custom `too_long` is supplied, they should not build the default error "
            "message, which calls `repr()` on the items and can fail or be slow. Fix both "
            "functions and keep the existing tests passing (`python3 -m unittest tests.test_more`).")),
    "bottle-etag": dict(
        prep=prep_repo("bottle", "457a8fa^"), venv=None,
        prompt=lambda v: (
            "Bug in `static_file()`'s ETag handling in bottle.py: the ETag header it generates "
            "is not a quoted string as HTTP requires, and an If-None-Match request header can "
            "list several comma-separated ETags, but `static_file()` returns 304 only when the "
            "header equals the ETag exactly. Make generated ETags quoted strings, and return 304 "
            "when the file's ETag is among those listed; an unquoted value must not match. Keep "
            "the existing tests passing (`python3 -m unittest discover -s test`).")),
}


def prepare():
    os.makedirs(TMPL, exist_ok=True)
    for name, t in TASKS.items():
        dest = os.path.join(TMPL, name)
        if os.path.exists(dest):
            continue
        print("prepare", name, flush=True)
        sub = t["prep"](dest)
        open(os.path.join(TMPL, name + ".sub"), "w").write(sub)


# ---------------------------------------------------------------- checks

def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def run_pytest(test_src, venv, cwd, env_extra=None, pre=None):
    td = tempfile.mkdtemp(prefix="check-", dir=os.path.join(BASE, "checks"))
    tf = os.path.join(td, "test_outputs.py")
    open(tf, "w").write(test_src)
    env = dict(os.environ)
    env["PATH"] = os.path.join(venv, "bin") + ":" + env["PATH"]
    env.update(env_extra or {})
    r = subprocess.run([os.path.join(venv, "bin/python"), "-m", "pytest", "-q", "-p", "no:cacheprovider", tf, "-rA"],
                       cwd=cwd, env=env, capture_output=True, text=True, timeout=1200)
    return r.returncode == 0, (r.stdout[-3000:] + r.stderr[-1000:])


def check(task, work, venv):
    """Independent check of the work directory. Returns (passed, detail)."""
    tests = os.path.join(TB, task, "tests/test_outputs.py") if task in (
        "fix-git", "fix-code-vulnerability", "headless-terminal", "build-cython-ext") else None
    if task == "fix-git":
        want = {"_includes/about.md": "0273104059c6bf524e767b8847b22946",
                "_layouts/default.html": "0f879389f66640f45316e393a71c5f2f"}
        got = {}
        for p in want:
            try:
                got[p] = hashlib.md5(open(os.path.join(work, p), "rb").read().strip()).hexdigest()
            except OSError:
                got[p] = None
        if got == want:
            return True, json.dumps(got)
        # "Merge them into master": master's own content counts wherever the
        # work happened (a routed run's worktree is detached, master lives in
        # the checkout).
        master = {}
        for p in want:
            r = subprocess.run(["git", "show", f"master:{p}"], cwd=work, capture_output=True)
            master[p] = hashlib.md5(r.stdout.strip()).hexdigest() if r.returncode == 0 else None
        return master == want, json.dumps({"worktree": got, "master": master})
    if task == "fix-code-vulnerability":
        src = open(tests).read().replace('Path("/app/report.jsonl")', f'Path("{work}/report.jsonl")')
        src = src.replace('"/app/bottle.py" in file_paths', 'any(p.endswith("bottle.py") for p in file_paths)')
        src = src.replace('report["/app/bottle.py"]', '[c for k, v in report.items() if k.endswith("bottle.py") for c in v]')
        env = {"PYTHONPATH": work}
        ok1, out1 = run_pytest(src, venv, work, env)
        r = subprocess.run([os.path.join(venv, "bin/python"), "-m", "pytest", "-q", "-p", "no:cacheprovider", "-rA"],
                           cwd=work, env={**os.environ, **env}, capture_output=True, text=True, timeout=1200)
        return ok1 and r.returncode == 0, out1[-1500:] + "\nREPO:" + r.stdout[-1500:]
    if task == "headless-terminal":
        home = tempfile.mkdtemp(prefix="home-", dir=os.path.join(BASE, "checks"))
        # The task image's HOME (/root on Debian) has a .profile that sources .bashrc.
        open(os.path.join(home, ".profile"), "w").write('[ -n "$BASH_VERSION" ] && [ -f "$HOME/.bashrc" ] && . "$HOME/.bashrc"\n')
        srv = tempfile.mkdtemp(prefix="server-", dir=os.path.join(BASE, "checks"))
        os.rmdir(srv)
        port = str(free_port())
        src = open(tests).read().replace('sys.path.append("/app")', f'sys.path.insert(0, "{work}")')
        src = src.replace('"/server"', f'"{srv}"').replace("/server/index.html", f"{srv}/index.html")
        src = src.replace("--directory /server", f"--directory {srv}").replace("8000", port)
        src = src.replace("/app/", work + "/")
        sh([os.path.join(venv, "bin/pip"), "-q", "install", "pytest==8.4.1", "requests==2.32.5"])
        # HeadlessTerminal launches `python` and `bash` itself: put the venv first.
        # A fresh tmux server: one already running for this user would start
        # shells with its own HOME and environment, unlike the task's container.
        tmux = tempfile.mkdtemp(prefix="tmux-", dir="/tmp")
        try:
            return run_pytest(src, venv, work, {"HOME": home, "TMUX_TMPDIR": tmux, "TMUX": ""})
        finally:
            subprocess.run(["tmux", "-S", os.path.join(tmux, f"tmux-{os.getuid()}", "default"), "kill-server"],
                           capture_output=True)
    if task == "build-cython-ext":
        src = open(tests).read().replace("/app/pyknotid", f"{work}/pyknotid")
        sh([os.path.join(venv, "bin/pip"), "-q", "install", "pytest==8.4.1", "packaging"])
        return run_pytest(src, venv, home_dir())
    if task in ("mi-seekable", "mi-one", "bottle-etag"):
        rev = {"mi-seekable": ("more-itertools", "6b1907d", ["tests/test_more.py"], ["-m", "unittest", "tests.test_more"]),
               "mi-one": ("more-itertools", "def2dab", ["tests/test_more.py"], ["-m", "unittest", "tests.test_more"]),
               "bottle-etag": ("bottle", "457a8fa", ["test/test_sendfile.py"], ["-m", "unittest", "test.test_sendfile"])}[task]
        name, fix, files, cmd = rev
        td = tempfile.mkdtemp(prefix="check-", dir=os.path.join(BASE, "checks"))
        # Copy the work tree (no .git) and drop in the fix commit's own tests.
        shutil.copytree(work, os.path.join(td, "w"), ignore=shutil.ignore_patterns(".git"))
        for f in files:
            blob = sh(["git", "show", f"{fix}:{f}"], cwd=os.path.join(SRC, name)).stdout
            open(os.path.join(td, "w", f), "w").write(blob)
            # bottle's test_ims compares a whole-second If-Modified-Since with the
            # test file's mtime: a file written this second reads as modified.
            past = time.time() - 3600
            os.utime(os.path.join(td, "w", f), (past, past))
        if task == "bottle-etag":
            # Upstream's test_sendfile leaks request state between tests under one
            # process (test_ims fails at the fix commit itself), so run each test alone.
            ids = sh([PY, "-c", "import unittest;s=unittest.defaultTestLoader.loadTestsFromName('test.test_sendfile');"
                      "f=lambda t:[x for y in t for x in f(y)] if isinstance(t,unittest.TestSuite) else [t.id()];print('\\n'.join(f(s)))"],
                     cwd=os.path.join(td, "w")).stdout.split()
            bad = [i for i in ids if subprocess.run([PY, "-m", "unittest", i], cwd=os.path.join(td, "w"),
                                                      capture_output=True, timeout=120).returncode != 0]
            return (not bad and len(ids) > 10), f"{len(ids)} tests, failing: {bad}"
        r = subprocess.run([PY, *cmd], cwd=os.path.join(td, "w"), capture_output=True, text=True, timeout=600)
        return r.returncode == 0, r.stderr[-2000:]
    raise KeyError(task)


def home_dir():
    return HOME


# ---------------------------------------------------------------- arms

def settings_file(engine):
    path = os.path.join(BASE, f"settings-{engine}.json")
    if engine == "claude-lean" and not os.path.exists(path):
        # #10246: Claude Code only, run as one lean session.
        settings = json.load(open(settings_file("claude")))
        settings.setdefault("coder", {})["claude"] = "session"
        json.dump(settings, open(path, "w"), indent=1)
    return path


def run_raw_claude(prompt, cwd, d):
    start = time.time()
    try:
        r = subprocess.run(["claude", "-p", prompt, "--output-format", "json", "--dangerously-skip-permissions"],
                           cwd=cwd, capture_output=True, text=True, timeout=TIMEOUT, stdin=subprocess.DEVNULL)
        out, err, code = r.stdout, r.stderr, r.returncode
    except subprocess.TimeoutExpired as e:
        out, err, code = (e.stdout or b"").decode() if isinstance(e.stdout, bytes) else (e.stdout or ""), "timeout", -1
    wall = time.time() - start
    open(os.path.join(d, "out.json"), "w").write(out)
    open(os.path.join(d, "err.txt"), "w").write(err)
    rec = dict(wall_s=wall, exit=code, work=cwd)
    try:
        j = json.loads(out)
        u = j.get("usage", {})
        rec.update(cost_usd=j.get("total_cost_usd"), engine_usd=j.get("total_cost_usd"), jev_usd=0.0,
                   turns=j.get("num_turns"), duration_api_ms=j.get("duration_api_ms"),
                   input_tokens=u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0),
                   uncached_input=u.get("input_tokens", 0), cache_write=u.get("cache_creation_input_tokens", 0),
                   cache_read=u.get("cache_read_input_tokens", 0), output_tokens=u.get("output_tokens", 0),
                   models=list((j.get("modelUsage") or {}).keys()), is_error=j.get("is_error"),
                   ending=j.get("subtype"))
    except Exception as e:  # noqa: BLE001
        rec.update(parse_error=str(e))
    return rec


def atif_summary(task_dir):
    s = dict(engine_usd=0.0, engine_priced=True, jev_loop_usd=0.0, jev_recipe_usd=0.0, embedding_usd=0.0,
             input_tokens=0, cache_read=0, cache_write=0, output_tokens=0, steps=0, requests=0, ending=None,
             recipe=None, checks_frozen=None, klass=None, knowledge_kept=None)
    for f in sorted(glob.glob(os.path.join(task_dir, "*.atif.jsonl"))):
        for line in open(f):
            try:
                d = json.loads(line)
            except ValueError:
                continue
            e = (d.get("step") or {}).get("extensions") or {}
            if "delegate_recipe" in e:
                r = e["delegate_recipe"]["run"]
                s["recipe"] = r.get("version")
                s["klass"] = (r.get("class") or {}).get("class")
                s["jev_recipe_usd"] += ((r.get("jev") or {}).get("cost_usd") or 0)
                s["embedding_usd"] += (((r.get("knowledge") or {}).get("search") or {}).get("embedding_usd") or 0)
                s["knowledge_kept"] = len(((r.get("knowledge") or {}).get("selection") or {}).get("kept") or [])
                s["checks_frozen"] = len((r.get("checks") or {}).get("kept") or [])
            er = e.get("effect_result")
            if er and er.get("kind") == "codex_request":
                u = (er.get("result") or {}).get("usage") or {}
                inp, cached, out = u.get("input", 0), u.get("cached", 0), u.get("output", 0)
                s["input_tokens"] += inp
                s["cache_read"] += cached
                s["output_tokens"] += out
                s["requests"] += 1
                # gpt-6.1-sol: $2 / $0.20 cached / $10 per million (docs/research/ppq.md lists
                # OpenAI's $2/$10; cached at the same 10% as gpt-6-sol). The engine records it
                # as unknown (no list price in codex_transport::price).
                s["engine_usd"] += ((inp - cached) * 2.0 + cached * 0.20 + out * 10.0) / 1e6
            if er and er.get("kind") == "claude_request":
                for ln in ((er.get("result") or {}).get("stdout") or "").splitlines():
                    try:
                        ev = json.loads(ln)
                    except ValueError:
                        continue
                    if ev.get("type") == "result":
                        u = ev.get("usage") or {}
                        s["input_tokens"] += u.get("input_tokens", 0) + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0)
                        s["cache_read"] += u.get("cache_read_input_tokens", 0)
                        s["cache_write"] += u.get("cache_creation_input_tokens", 0)
                        s["output_tokens"] += u.get("output_tokens", 0)
                        s["requests"] += 1
            if er and er.get("kind") == "claude_session":
                # #10246: one lean Claude Code session; its own list-price cost.
                r = er.get("result") or {}
                s["engine_usd"] += r.get("cost_usd") or 0
                s["input_tokens"] += (r.get("input_tokens") or 0) + (r.get("cache_read_input_tokens") or 0) + (r.get("cache_creation_input_tokens") or 0)
                s["cache_read"] += r.get("cache_read_input_tokens") or 0
                s["cache_write"] += r.get("cache_creation_input_tokens") or 0
                s["output_tokens"] += ((r.get("usage") or {}).get("output_tokens") or 0)
                s["requests"] += r.get("api_calls") or 0
                s["steps"] += r.get("num_turns") or 0
                s["ending"] = r.get("status")
            ei = e.get("effect_intent") or e.get("effect")
            if isinstance(ei, dict) and ei.get("kind") == "claude_session":
                s["effort"] = (ei.get("arguments") or {}).get("effort")
            m = (e.get("microcoder") or {}).get("event") or {}
            if m.get("event") == "ended":
                o = m.get("outcome") or {}
                s["steps"] += o.get("steps") or 0
                s["jev_loop_usd"] += o.get("jev_usd") or 0
                s["ending"] = (o.get("ending") or {}).get("reason")
                if o.get("model_usd") is not None:
                    s["claude_model_usd"] = s.get("claude_model_usd", 0) + o["model_usd"]
    if "claude_model_usd" in s:
        s["engine_usd"] = s.pop("claude_model_usd")
    return s


def run_routed(engine, recipe_on, prompt, cwd, d):
    env = dict(os.environ)
    env.update(OPENAGENTS_SETTINGS=settings_file(engine), OPENAGENTS_TASKS=os.path.join(d, "tasks"),
               OPENAGENTS_CHAT_HOME=os.path.join(d, "chat"))
    if not recipe_on:
        env["OPENAGENTS_CODER_CONTROLLER"] = OFF_SHIM
    start = time.time()
    try:
        r = subprocess.run([OA, "chat", "send", "--local", "--json", "--timeout", str(TIMEOUT), prompt],
                           cwd=cwd, env=env, capture_output=True, text=True, timeout=TIMEOUT + 60, stdin=subprocess.DEVNULL)
        out, err, code = r.stdout, r.stderr, r.returncode
    except subprocess.TimeoutExpired as e:
        out, err, code = (e.stdout.decode() if isinstance(e.stdout, bytes) else (e.stdout or "")), "timeout", -1
    open(os.path.join(d, "out.ndjson"), "w").write(out)
    open(os.path.join(d, "err.txt"), "w").write(err)
    events = []
    for ln in out.splitlines():
        try:
            events.append(json.loads(ln))
        except ValueError:
            pass
    thread = next((e.get("thread") for e in events if e.get("thread")), None)
    started = any(e.get("event") == "coder" and e.get("accepted") for e in events)
    if not started and thread:
        # The router only offered: accept the offer, as a person would.
        r2 = subprocess.run([OA, "chat", "run-coder", "--local", "--json", "--thread", thread], cwd=cwd, env=env,
                            capture_output=True, text=True, timeout=TIMEOUT + 60, stdin=subprocess.DEVNULL)
        open(os.path.join(d, "out-run-coder.ndjson"), "w").write(r2.stdout)
        for ln in r2.stdout.splitlines():
            try:
                events.append(json.loads(ln))
            except ValueError:
                pass
    # Wait for the route record to settle (the CLI may stop following first).
    rec = None
    deadline = start + TIMEOUT + 120
    while time.time() < deadline:
        files = glob.glob(os.path.join(d, "routes", "*.jsonl"))
        if files:
            lines = [json.loads(l) for l in open(files[0]) if l.strip()]
            rec = lines[-1]
            if rec.get("settled_ms"):
                break
        time.sleep(5)
    wall = time.time() - start
    work = None
    for e in events:
        t = e.get("task") if isinstance(e.get("task"), dict) else None
        if t and t.get("worktree"):
            work = t["worktree"]
        if e.get("event") == "coder_started" and e.get("worktree"):
            work = e["worktree"]
    result = dict(wall_s=wall, exit=code, work=work, thread=thread, routed_started=started,
                  route_state=(rec or {}).get("state"))
    if rec:
        runs = rec.get("runs") or []
        result.update(route_family=(rec.get("result") or {}).get("route"),
                      route_cost_usd=(sum(r.get("cost_microusd") or 0 for r in runs) / 1e6) if runs and all(r.get("cost_microusd") is not None for r in runs) else None,
                      route_wall_s=sum((r.get("wall_ms") or 0) for r in runs) / 1000 if runs else None,
                      engines=[r.get("engine") for r in runs],
                      projection=[r.get("projection") for r in runs],
                      route_e2e_s=((rec.get("settled_ms") or 0) - rec.get("received_ms", 0)) / 1000 if rec.get("settled_ms") else None)
    s = atif_summary(os.path.join(d, "tasks"))
    result.update(s)
    result["jev_usd"] = s["jev_loop_usd"] + s["jev_recipe_usd"] + s["embedding_usd"]
    result["cost_usd"] = s["engine_usd"] + result["jev_usd"]
    return result


ARMS = {
    "raw-claude": lambda p, c, d: run_raw_claude(p, c, d),
    "routed-claude-on": lambda p, c, d: run_routed("claude", True, p, c, d),
    "routed-codex-on": lambda p, c, d: run_routed("codex", True, p, c, d),
    "routed-claude-off": lambda p, c, d: run_routed("claude", False, p, c, d),
    "routed-codex-off": lambda p, c, d: run_routed("codex", False, p, c, d),
    "routed-claude-lean": lambda p, c, d: run_routed("claude-lean", True, p, c, d),
}


def one(arm, task, trial):
    d = os.path.join(RUNS, task, arm, str(trial))
    if os.path.exists(os.path.join(d, "result.json")):
        return json.load(open(os.path.join(d, "result.json")))
    if os.path.exists(d):
        shutil.rmtree(d)
    os.makedirs(d)
    os.makedirs(os.path.join(BASE, "checks"), exist_ok=True)
    t = TASKS[task]
    sub = open(os.path.join(TMPL, task + ".sub")).read().strip()
    shutil.copytree(os.path.join(TMPL, task), os.path.join(d, "repo"), symlinks=True)
    venv = None
    if t["venv"] is not None:
        venv = os.path.join(d, "venv")
        make_venv(venv, t["venv"])
    prompt = t["prompt"](venv)
    cwd = os.path.normpath(os.path.join(d, "repo", sub))
    res = dict(arm=arm, task=task, trial=trial, started=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()))
    res.update(ARMS[arm](prompt, cwd, d))
    work = res.get("work")
    try:
        ok, detail = check(task, work, venv) if work and os.path.isdir(work) else (False, "no work directory")
    except Exception as e:  # noqa: BLE001
        ok, detail = False, f"check error: {e}"
    res.update(passed=ok, check_detail=detail[-1500:])
    json.dump(res, open(os.path.join(d, "result.json"), "w"), indent=1)
    with open(RESULTS, "a") as f:
        f.write(json.dumps({k: v for k, v in res.items() if k != "check_detail"}) + "\n")
    print(f"{task} {arm} {trial}: pass={ok} cost={res.get('cost_usd')} wall={res.get('wall_s'):.0f}s", flush=True)
    return res


def main():
    cmd = sys.argv[1]
    if cmd == "prepare":
        prepare()
    elif cmd == "recheck":
        # Re-run the check of every finished run of TASK (the agents' work is kept).
        for f in sorted(glob.glob(os.path.join(RUNS, sys.argv[2], "*", "*", "result.json"))):
            r = json.load(open(f))
            d = os.path.dirname(f)
            venv = os.path.join(d, "venv") if TASKS[r["task"]]["venv"] is not None else None
            work = r.get("work")
            ok, detail = check(r["task"], work, venv) if work and os.path.isdir(work) else (False, "no work directory")
            if ok != r.get("passed"):
                print("changed", f, r.get("passed"), "->", ok)
            r.update(passed=ok, check_detail=detail[-1500:])
            json.dump(r, open(f, "w"), indent=1)
    elif cmd == "collect":
        rows = [json.load(open(f)) for f in sorted(glob.glob(os.path.join(RUNS, "*", "*", "*", "result.json")))]
        with open(os.path.join(BASE, "collected.jsonl"), "w") as out:
            for r in rows:
                out.write(json.dumps({k: v for k, v in r.items() if k != "check_detail"}) + "\n")
        print(len(rows), "runs")
    elif cmd == "one":
        one(sys.argv[2], sys.argv[3], int(sys.argv[4]))
    elif cmd == "run":
        arms, tasks, trials, par = sys.argv[2].split(","), sys.argv[3].split(","), [int(x) for x in sys.argv[4].split(",")], int(sys.argv[5])
        jobs = [(a, t, n) for n in trials for t in tasks for a in arms]
        random.Random(10209).shuffle(jobs)
        with cf.ThreadPoolExecutor(par) as ex:
            futs = {ex.submit(one, *j): j for j in jobs}
            for f in cf.as_completed(futs):
                try:
                    f.result()
                except Exception as e:  # noqa: BLE001
                    print("FAILED", futs[f], e, flush=True)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""The check contract per package kind (#11310).

The briefed agent was tuned to one task shape: a Rust fix in one or two
crates, checked with `cargo test -p` and `cargo fmt`. This module says what
checks a change needs for every kind of file in this repository, so the
briefing, the agent's `verify` and the driver's replayed checks all run
the same thing:

| Kind | Package | Checks |
|---|---|---|
| `rust` | the Cargo package (root workspace) | `cargo test -p`, `cargo fmt -p` (verify's own Rust path) |
| `rust-nested` | a package of a nested workspace (`crates/openagents-mobile`, `crates/psionic`) | `cargo test --manifest-path`, `cargo fmt --manifest-path` |
| `python` | the script's directory | `py_compile` on the files; the tests that name the script (`pytest`, else `unittest`); `ruff` only when the repo configures it |
| `shell` | the script's directory | `bash -n`; `shellcheck -S error` when installed; the tests that name the script |
| `docs` | the document's directory | relative links and repo paths resolve (new breakage only, `doccheck`); a document inside a Rust package is checked by that package's tests too (embedded copy, `oa-copy` guard) |
| `web` | the nearest `package.json` | `node --check` for JavaScript; TypeScript only where its compiler is installed |
| `data` | the file's directory | JSON parses |
| `swift`, `kotlin` | the app | `swiftc -parse` where installed; Kotlin has no compiler here: reported, never passed |

A file inside a Rust package that is not Rust (CSS, JSON, a generated
`tree.json`, an embedded Markdown page) belongs to that package: its tests
are what check it (AGENTS.md: a stale generated file shows as a failing
test; regenerate it).

    kinds.py plan --root DIR [--base REV] FILE...        # checks for changed files (JSON)
    kinds.py run --root DIR [--base REV] FILE...         # plan, run, and report (JSON)
    kinds.py doccheck --root DIR [--base REV] FILE...    # links and paths in Markdown

`plan` is what `verify` (crates/briefed-agent) calls with the working
copy's changed files; `run` is what the driver's replay
(scripts/work/work_issue.py) and the bench's grader call.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Workspaces of their own inside the repository (root Cargo.toml's
# `exclude`): `cargo -p` from the root cannot see their packages.
NESTED = ("apps/openagents", "crates/openagents-mobile", "crates/psionic", "vendor")
CHECK_SECS = int(os.environ.get("OA_KIND_CHECK_SECS", "300"))
PY_EXT = (".py",)
SH_EXT = (".sh", ".bash")
DOC_EXT = (".md", ".mdx")
WEB_EXT = (".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx")
SCRIPT_DIRS = ("scripts", "os", "bin", "tools")


# ------------------------------------------------------------------ reading

class Tree:
    """Reads files from a working copy (`root`) or, when `rev` is given,
    from git objects at that commit (the briefing reads the base commit)."""

    def __init__(self, root: Path, rev: str | None = None):
        self.root = Path(root)
        self.rev = rev
        self._paths: list[str] | None = None
        self._dirs: set[str] | None = None

    def read(self, path: str) -> str | None:
        if self.rev:
            proc = subprocess.run(["git", "show", f"{self.rev}:{path}"], cwd=self.root,
                                  capture_output=True, text=True)
            return proc.stdout if proc.returncode == 0 else None
        p = self.root / path
        try:
            return p.read_text(errors="replace") if p.is_file() else None
        except OSError:
            return None

    def exists(self, path: str) -> bool:
        path = path.rstrip("/")
        if self.rev:
            proc = subprocess.run(["git", "cat-file", "-e", f"{self.rev}:{path}"], cwd=self.root,
                                  capture_output=True)
            return proc.returncode == 0
        if (self.root / path).exists():
            return True
        # A sparse checkout leaves files out of the working copy that the
        # index still holds (the bench's worktrees omit assets/).
        if self._dirs is None:
            self._dirs = set()
            for p in self.paths():
                parts = p.split("/")
                for i in range(1, len(parts) + 1):
                    self._dirs.add("/".join(parts[:i]))
        return path in self._dirs

    def paths(self) -> list[str]:
        if self._paths is None:
            if self.rev:
                out = subprocess.run(["git", "ls-tree", "-r", "--name-only", self.rev], cwd=self.root,
                                     capture_output=True, text=True).stdout
            else:
                out = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard"],
                                     cwd=self.root, capture_output=True, text=True).stdout
                gone = set(subprocess.run(["git", "ls-files", "--deleted"], cwd=self.root,
                                          capture_output=True, text=True).stdout.splitlines())
                out = "\n".join(p for p in out.splitlines() if p not in gone)
            self._paths = out.splitlines()
        return self._paths

    def grep_files(self, needle: str, under: tuple[str, ...]) -> list[str]:
        """Files under `under` that contain `needle` (fixed string)."""
        args = ["git", "grep", "-l", "-I", "-F", "-e", needle]
        args += [self.rev, "--"] if self.rev else ["--untracked", "--"]
        args += list(under)
        proc = subprocess.run(args, cwd=self.root, capture_output=True, text=True)
        rows = proc.stdout.splitlines()
        if self.rev:
            rows = [r.split(":", 1)[1] if r.startswith(self.rev + ":") else r for r in rows]
        return rows


def manifest_package(text: str) -> str | None:
    if "[package]" not in text:
        return None
    m = re.search(r'^\s*name\s*=\s*"([^"]+)"', text.split("[package]", 1)[1], re.M)
    return m.group(1) if m else None


def cargo_package(tree: Tree, path: str) -> tuple[str, str] | None:
    """(package name, manifest dir) of the nearest Cargo package above `path`."""
    parts = path.split("/")
    for i in range(len(parts) - 1, 0, -1):
        d = "/".join(parts[:i])
        text = tree.read(f"{d}/Cargo.toml")
        if text:
            name = manifest_package(text)
            if name:
                return name, d
    return None


def nested_root(path: str) -> str | None:
    for root in NESTED:
        if path == root or path.startswith(root + "/"):
            return root
    return None


# ------------------------------------------------------------------- kinds

def kind_of(path: str) -> str:
    low = path.lower()
    if low.endswith(".rs") or low.endswith("/cargo.toml") or low == "cargo.toml":
        return "rust"
    if low.endswith(PY_EXT):
        return "python"
    if low.endswith(SH_EXT):
        return "shell"
    if low.endswith(DOC_EXT):
        return "docs"
    if low.endswith(WEB_EXT):
        return "web"
    if low.endswith(".swift"):
        return "swift"
    if low.endswith(".kt") or low.endswith(".kts"):
        return "kotlin"
    if low.endswith(".json"):
        return "data"
    return "other"


def shebang_kind(tree: Tree, path: str) -> str | None:
    """An extensionless script (`os/bin/screen-record`) by its first line."""
    if "." in Path(path).name:
        return None
    head = (tree.read(path) or "")[:120]
    if head.startswith("#!"):
        if "python" in head.split("\n", 1)[0]:
            return "python"
        if re.search(r"\b(ba|z|)sh\b", head.split("\n", 1)[0]):
            return "shell"
    return None


def group(paths: list[str], tree: Tree) -> list[dict]:
    """The packages these files belong to, in first-seen order:
    `{kind, id, files, manifest?}`. A non-Rust file inside a Rust package
    belongs to that package unless it is a script or a README."""
    out: dict[tuple[str, str], dict] = {}
    for path in paths:
        if path.endswith("Cargo.lock"):
            continue
        kind = kind_of(path)
        if kind == "other":
            kind = shebang_kind(tree, path) or "other"
        pkg = None
        name = Path(path).name.lower()
        inside = cargo_package(tree, path) if path.startswith(("crates/", "apps/", "bins/")) else None
        nested = nested_root(path)
        if kind == "rust" or (inside and kind in ("other", "data", "web")
                              or inside and kind == "docs" and name != "readme.md" and "/docs/" not in path):
            if inside:
                if nested:
                    pkg = {"kind": "rust-nested", "id": inside[0], "manifest": f"{nested}/Cargo.toml",
                           "dir": inside[1]}
                else:
                    pkg = {"kind": "rust", "id": inside[0], "dir": inside[1]}
        if pkg is None:
            if kind in ("python", "shell", "docs", "data"):
                pkg = {"kind": kind, "id": str(Path(path).parent) or "."}
            elif kind == "web":
                pkg = {"kind": "web", "id": web_root(tree, path)}
            elif kind in ("swift", "kotlin"):
                pkg = {"kind": kind, "id": "/".join(path.split("/")[:2])}
            else:
                pkg = {"kind": "other", "id": str(Path(path).parent) or "."}
        key = (pkg["kind"], pkg["id"])
        if key not in out:
            out[key] = dict(pkg, files=[])
        out[key]["files"].append(path)
    return list(out.values())


def web_root(tree: Tree, path: str) -> str:
    parts = path.split("/")
    for i in range(len(parts) - 1, 0, -1):
        d = "/".join(parts[:i])
        if tree.exists(f"{d}/package.json"):
            return d
    return str(Path(path).parent)


# ---------------------------------------------------------------- checks

def have(tool: str) -> bool:
    return shutil.which(tool) is not None


def python_has(module: str) -> bool:
    proc = subprocess.run([sys.executable, "-c", f"import {module}"], capture_output=True)
    return proc.returncode == 0


def tests_naming(tree: Tree, path: str) -> list[str]:
    """Test files that exercise a script: by name next to it or under a
    tests directory, or test files that name the script's file."""
    p = Path(path)
    stem = p.stem if p.suffix else p.name
    snake = stem.replace("-", "_")
    cands = [
        p.parent / f"test_{snake}.py", p.parent / f"{snake}_test.py", p.parent / "tests" / f"test_{snake}.py",
        p.parent / f"{stem}.test.sh", p.parent / f"test-{stem}.sh", p.parent / f"test_{snake}.sh",
        p.parent / "tests" / f"{stem}.sh",
        p.parent.parent / "tests" / f"{stem}.sh", p.parent.parent / "tests" / f"test_{snake}.py",
        Path("scripts/tests") / f"test_{snake}.py",
    ]
    found = [str(c) for c in cands if tree.exists(str(c))]
    dirs = tuple(d for d in {str(p.parent), str(p.parent / "tests"), str(p.parent.parent / "tests"), "scripts/tests"}
                 if d and d != "." and tree.exists(d))
    if dirs:
        for f in tree.grep_files(p.name, dirs):
            if is_script_test(f) and f not in found and f != path:
                found.append(f)
    return found[:6]


def is_script_test(path: str) -> bool:
    name = Path(path).name
    return (name.startswith("test_") and name.endswith(".py")) or name.endswith("_test.py") \
        or name.endswith(".test.sh") or (name.startswith(("test-", "test_")) and name.endswith(".sh")) \
        or "/tests/" in f"/{path}" and name.endswith((".sh", ".py"))


def test_argv(tests: list[str]) -> list[list[str]]:
    py = [t for t in tests if t.endswith(".py")]
    sh = [t for t in tests if not t.endswith(".py")]
    out = []
    if py:
        out.append(["python3", "-m", "pytest", "-q", "-p", "no:cacheprovider", *py] if python_has("pytest")
                   else ["python3", "-m", "unittest", *[t[:-3].replace("/", ".") for t in py]])
    for t in sh:
        out.append(["bash", t])
    return out


def ruff_configured(tree: Tree) -> bool:
    for f in ("ruff.toml", ".ruff.toml"):
        if tree.exists(f):
            return True
    return "[tool.ruff" in (tree.read("pyproject.toml") or "")


def consumers(tree: Tree, path: str) -> list[dict]:
    """Rust packages that embed a document (`include_str!` of its name)."""
    name = Path(path).name
    if name.lower() == "readme.md":
        return []
    out = []
    for f in tree.grep_files(f"{name}\"", ("crates",))[:20]:
        if not f.endswith(".rs"):
            continue
        text = tree.read(f) or ""
        if re.search(r"include_(str|bytes)!\([^)]*" + re.escape(name), text):
            pkg = cargo_package(tree, f)
            if pkg and not nested_root(f) and pkg[0] not in [c["id"] for c in out]:
                out.append({"kind": "rust", "id": pkg[0], "dir": pkg[1], "files": [], "why": f"embeds {path}"})
    return out[:2]


def checks_for(pkg: dict, tree: Tree, base: str | None = None) -> list[dict]:
    """The checks one package needs. Each check: `{id, kind, package, argv,
    what, filter, required, runner, available}`; `runner` is `exec` for a
    build command (the build lock and target) and `direct` otherwise; an
    unavailable check is listed and never counted as a pass."""
    kind, pid, files = pkg["kind"], pkg["id"], pkg.get("files") or []
    c = []

    def add(step, argv, what, *, filter=False, runner="direct", available=True, required=True):
        c.append({"id": f"{step}:{pid}", "kind": kind, "package": pid, "argv": argv, "what": what,
                  "filter": filter, "required": required, "runner": runner, "available": available})

    if kind == "rust":
        add("check", ["cargo", "check", "-p", pid, "--tests", "--message-format", "short"],
            f"compile {pid} and its tests", runner="exec")
        add("test", ["cargo", "test", "-p", pid], f"run {pid}'s tests (pass a test-name filter to run fewer)",
            filter=True, runner="exec")
        add("fmt", ["cargo", "fmt", "-p", pid], f"format {pid}", runner="exec")
    elif kind == "rust-nested":
        m = pkg["manifest"]
        add("check", ["cargo", "check", "--manifest-path", m, "-p", pid, "--tests", "--message-format", "short"],
            f"compile {pid} (its own workspace, {m})", runner="exec")
        add("test", ["cargo", "test", "--manifest-path", m, "-p", pid],
            f"run {pid}'s tests in its own workspace", filter=True, runner="exec")
        add("fmt", ["cargo", "fmt", "--manifest-path", m, "-p", pid], f"format {pid}", runner="exec")
    elif kind == "python":
        add("py_compile", ["python3", "-m", "py_compile", *files], "the scripts parse")
        tests = []
        for f in files:
            if is_script_test(f):
                tests.append(f)
            for t in tests_naming(tree, f):
                if t not in tests:
                    tests.append(t)
        for i, argv in enumerate(test_argv(tests)):
            add("tests" if i == 0 else f"tests{i + 1}", argv,
                "run the tests that exercise these scripts: " + ", ".join(tests))
        if ruff_configured(tree):
            add("ruff", ["ruff", "check", *files], "lint (ruff)", available=have("ruff"))
    elif kind == "shell":
        for f in files:
            add(f"bash_n[{Path(f).name}]", ["bash", "-n", f], f"{f} parses")
        add("shellcheck", ["shellcheck", "-S", "error", *files], "shellcheck, errors only",
            available=have("shellcheck"))
        tests = []
        for f in files:
            if is_script_test(f):
                tests.append(f)
            for t in tests_naming(tree, f):
                if t not in tests:
                    tests.append(t)
        for i, argv in enumerate(test_argv(tests)):
            add("tests" if i == 0 else f"tests{i + 1}", argv,
                "run the tests that exercise these scripts: " + ", ".join(tests))
    elif kind == "docs":
        argv = ["python3", str(HERE / "kinds.py"), "doccheck", "--root", "."]
        if base:
            argv += ["--base", base]
        add("doccheck", argv + files, "relative links and repo paths in these documents resolve (new breakage only)")
    elif kind == "web":
        js = [f for f in files if f.endswith((".js", ".mjs", ".cjs"))]
        ts = [f for f in files if f.endswith((".ts", ".tsx"))]
        for f in js:
            add(f"node_check[{Path(f).name}]", ["node", "--check", f], f"{f} parses", available=have("node"))
        if ts:
            tsc = f"{pid}/node_modules/.bin/tsc"
            add("tsc", [tsc, "--noEmit", "-p", pid], "type-check the package",
                available=Path(tree.root, tsc).exists() and not tree.rev)
    elif kind == "data":
        add("json", [sys.executable, "-c", "import json,sys\nfor p in sys.argv[1:]: json.load(open(p))", *files],
            "the JSON parses")
    elif kind == "swift":
        add("swiftc_parse", ["swiftc", "-parse", *files], "the Swift parses", available=have("swiftc"))
    elif kind == "kotlin":
        add("kotlinc", ["kotlinc", *files], "no Kotlin compiler on this computer: review the change by reading it",
            available=False)
    return c


def plan(paths: list[str], tree: Tree, base: str | None = None, rust: bool = True) -> list[dict]:
    """Every check the changed files need. `rust=False` leaves out
    root-workspace Rust packages (verify runs those itself)."""
    out, seen = [], set()
    pkgs = group(paths, tree)
    for pkg in list(pkgs):
        if pkg["kind"] == "docs":
            for f in pkg["files"]:
                for cons in consumers(tree, f):
                    if all(p["id"] != cons["id"] for p in pkgs):
                        pkgs.append(cons)
    for pkg in pkgs:
        if pkg["kind"] == "rust" and not rust:
            continue
        if pkg["kind"] == "other":
            continue
        for check in checks_for(pkg, tree, base):
            if check["id"] not in seen:
                seen.add(check["id"])
                out.append(check)
    return out


# ------------------------------------------------------------------ running

def run_check(check: dict, root: Path, exec_path: str | None = None, filter_words: list[str] | None = None,
              timeout: int = CHECK_SECS, env: dict | None = None) -> dict:
    """Run one check in `root`. Exec checks go through `exec_path` (the
    build lock) when given."""
    t0 = time.time()
    if not check.get("available", True):
        return {"id": check["id"], "ok": None, "available": False, "secs": 0.0,
                "tail": check.get("what", "not checkable here")}
    argv = list(check["argv"])
    if check.get("filter") and filter_words:
        argv += ["--", *filter_words]
    if check.get("runner") == "exec" and exec_path:
        argv = [exec_path, *argv]
    try:
        proc = subprocess.run(argv, cwd=root, capture_output=True, text=True, timeout=timeout,
                              env=dict(env or os.environ, PYTHONDONTWRITEBYTECODE="1"))
        code, text = proc.returncode, proc.stdout + proc.stderr
    except subprocess.TimeoutExpired as e:
        code, text = 124, f"timed out after {timeout}s\n{(e.stdout or b'')[-2000:]!r}"
    except OSError as e:
        code, text = None, f"could not run: {e}"
    return {"id": check["id"], "ok": code == 0, "available": True, "exit": code,
            "secs": round(time.time() - t0, 1), "tail": "" if code == 0 else tail(text)}


def tail(text: str, lines: int = 25, chars: int = 1800) -> str:
    rows = [r for r in text.splitlines() if r.strip()]
    return "\n".join(rows[-lines:])[-chars:]


def implicated(text: str) -> list[dict]:
    """File and line pairs a failing check's output names (Python
    tracebacks, pytest, shellcheck, bash -n, doccheck, compilers)."""
    found, out = set(), []
    pats = [r'File "([^"]+)", line (\d+)', r"^([\w./@+-]+\.\w+):(\d+):", r"In ([\w./@+-]+) line (\d+):",
            r"^([\w./@+-]+): line (\d+):"]
    for pat in pats:
        for m in re.finditer(pat, text, re.M):
            key = (m.group(1), int(m.group(2)))
            if key not in found and not m.group(1).startswith(("/usr", "<")):
                found.add(key)
                out.append({"file": m.group(1), "line": key[1]})
    return out[:10]


# ----------------------------------------------------------------- doccheck

LINK = re.compile(r"(?<!!)\[[^\]\n]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
CODE_PATH = re.compile(r"`((?:crates|scripts|docs|apps|bins|os|packages|plugins)/[\w./@+-]+)`")


def doc_problems(text: str, path: str, exists) -> set[str]:
    """Relative links and backticked repository paths in one document that
    do not resolve."""
    problems = set()
    here = Path(path).parent
    for target in LINK.findall(text):
        if re.match(r"^[a-z][a-z0-9+.-]*:", target, re.I) or target.startswith(("#", "<")):
            continue
        rel = target.split("#", 1)[0].split("?", 1)[0]
        if not rel:
            continue
        resolved = os.path.normpath(rel.lstrip("/") if rel.startswith("/") else str(here / rel))
        if resolved.startswith("..") or not exists(resolved):
            problems.add(f"link to {target} (resolves to {resolved}) does not exist")
    for ref in CODE_PATH.findall(text):
        ref = ref.rstrip(".,:;")
        if any(ch in ref for ch in "*{}<>$") or "..." in ref:
            continue
        if "." not in Path(ref).name and not ref.endswith("/"):
            continue
        if not exists(ref.split(":", 1)[0].rstrip("/")):
            problems.add(f"path `{ref}` does not exist")
    return problems


def doccheck(root: Path, base: str | None, files: list[str]) -> int:
    work = Tree(root)
    then = Tree(root, base) if base else None
    bad = 0
    for f in files:
        text = work.read(f)
        if text is None:
            continue  # deleted
        now = doc_problems(text, f, work.exists)
        before = set()
        if then is not None:
            old = then.read(f)
            if old is not None:
                before = doc_problems(old, f, then.exists)
        for p in sorted(now - before):
            print(f"{f}:1: {p}")
            bad += 1
    if not bad:
        print(f"doccheck: {len(files)} document(s), no new broken link or path")
    return 1 if bad else 0


# --------------------------------------------------------------------- main

def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("plan", "run", "doccheck"):
        p = sub.add_parser(name)
        p.add_argument("--root", default=".")
        p.add_argument("--base")
        p.add_argument("--exec", dest="exec_path")
        p.add_argument("--no-rust", action="store_true", help="leave root-workspace Rust packages to verify")
        p.add_argument("files", nargs="*")
    args = ap.parse_args()
    root = Path(args.root).resolve()
    if args.cmd == "doccheck":
        return doccheck(root, args.base, args.files)
    tree = Tree(root)
    checks = plan(args.files, tree, args.base, rust=not args.no_rust)
    if args.cmd == "plan":
        print(json.dumps({"checks": checks}))
        return 0
    results = [run_check(c, root, args.exec_path) for c in checks]
    print(json.dumps({"checks": checks, "results": results}))
    return 0 if all(r["ok"] is not False for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())

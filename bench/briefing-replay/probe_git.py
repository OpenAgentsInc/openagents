#!/usr/bin/env python3
"""Show Git's directory collapsing in a temporary repository; never remove worktrees."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    started = time.monotonic()
    env = os.environ.copy()
    for key in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR"):
        env.pop(key, None)
    env.update(GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")
    with tempfile.TemporaryDirectory(prefix="briefing-git-probe-") as directory:
        root = Path(directory)
        def git(*args):
            return subprocess.check_output(["git", "-C", directory, *args], env=env)
        git("init", "--quiet")
        (root / ".gitignore").write_text("/web/node_modules/\n")
        (root / "tracked.txt").write_text("source\n")
        git("add", ".gitignore", "tracked.txt")
        cache = root / "web/node_modules/pkg"
        cache.mkdir(parents=True)
        (cache / "index.js").write_text("reinstallable fixture\n")
        commands = {
            "ls_files_directory": ["ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--directory", "--no-empty-directory"],
            "status_ignored_matching": ["status", "--porcelain", "-z", "--ignored=matching", "--untracked-files=all"],
        }
        outputs = {name: [s.decode() for s in git(*argv).split(b"\0") if s] for name, argv in commands.items()}
        outputs["status_ignored_matching"] = [line for line in outputs["status_ignored_matching"] if line.startswith("!! ")]
        print(json.dumps({"schema": "openagents.briefing.git-probe.v1", "git_version": git("--version").decode().strip(), "commands": commands, "outputs": outputs, "elapsed_seconds": time.monotonic() - started}, indent=2))
        if outputs["ls_files_directory"] != ["web/"] or outputs["status_ignored_matching"] != ["!! web/node_modules/"]:
            raise SystemExit("Observed Git behavior differs; inspect the output before using this briefing fact.")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Republish the existing Coax book from the exact, clean commit on origin/main.

Standard-library maintenance tooling; never part of a compiler or app build.
Tokens are read from the environment or an explicitly supplied local env file.
"""
import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import shlex
import subprocess
import sys
import time
import urllib.error
import urllib.request
import os

ROOT = Path(__file__).resolve().parents[1]
SPACE = "sZhQ0yptSOKMneBDWMti"
PUBLIC = "https://openagents.gitbook.io/the-coax-programming-language"
REPOSITORY = "https://github.com/OpenAgentsInc/openagents.git"

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None

def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()

def token_from_file(path):
    for line in path.read_text().splitlines():
        line = line.strip().removeprefix("export ")
        if line.startswith("GITBOOK_TOKEN="):
            words = shlex.split(line.split("=", 1)[1], comments=True)
            if len(words) == 1:
                return words[0]
    raise SystemExit("The token file must define GITBOOK_TOKEN")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--token-file", type=Path, help="local env file defining GITBOOK_TOKEN")
    parser.add_argument("--timeout", type=int, default=600, help="seconds to wait for the imported revision")
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("--timeout must be positive")
    token = token_from_file(args.token_file) if args.token_file else os.environ.get("GITBOOK_TOKEN")
    if not token:
        parser.error("set GITBOOK_TOKEN or supply --token-file")
    if git("remote", "get-url", "origin") != REPOSITORY:
        raise SystemExit("Refusing to publish from an unexpected origin")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise SystemExit("Commit the worktree before publishing")
    commit = git("rev-parse", "HEAD")
    remote = git("ls-remote", "origin", "refs/heads/main").split()[0]
    if commit != remote:
        raise SystemExit("HEAD must equal the pushed origin/main")
    opener = urllib.request.build_opener(NoRedirect())
    def api(path, body=None):
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request("https://api.gitbook.com/v1" + path, data=data,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        try:
            with opener.open(request, timeout=30) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as exc:
            # Error bodies can reflect request details. Never print credentials.
            raise SystemExit(f"GitBook API returned HTTP {exc.code}; content was not verified") from None
        except (OSError, ValueError):
            raise SystemExit("GitBook request failed; inspect the space before retrying an import") from None
    space = api(f"/spaces/{SPACE}")
    if space["title"] != "The Coax Programming Language" or space["urls"].get("published", "").rstrip("/") != PUBLIC:
        raise SystemExit("The existing book identity no longer matches; no import requested")
    before = api(f"/spaces/{SPACE}/content")
    if before.get("git", {}).get("oid") != commit:
        print(f"Importing pushed commit {commit[:12]} into the existing book", flush=True)
        api(f"/spaces/{SPACE}/git/import", {
            "url": REPOSITORY, "ref": "refs/heads/main", "repoProjectDirectory": "coax/book",
            "repoTreeURL": "https://github.com/OpenAgentsInc/openagents/tree/main",
            "repoCommitURL": "https://github.com/OpenAgentsInc/openagents/commit/",
            "standalone": False, "force": True, "timestamp": datetime.now(timezone.utc).isoformat()
        })
    deadline = time.monotonic() + args.timeout
    while True:
        content = api(f"/spaces/{SPACE}/content")
        if content.get("git", {}).get("oid") == commit:
            def pages(tree):
                for page in tree:
                    yield page
                    yield from pages(page.get("pages", []))
            published_paths = {p.get("git", {}).get("path") for p in pages(content["pages"])}
            import re
            expected_paths = {"coax/book/" + p for p in re.findall(r"\]\(([^)]+\.md)\)", (ROOT / "coax/book/SUMMARY.md").read_text())}
            if not expected_paths.issubset(published_paths):
                raise SystemExit("The imported revision is missing book pages; inspect before retrying")
            print(f"Verified published revision {content['id']} at commit {commit}")
            print(PUBLIC)
            return
        if time.monotonic() >= deadline:
            raise SystemExit("Import verification timed out; inspect the existing space before retrying")
        time.sleep(10)

if __name__ == "__main__":
    main()

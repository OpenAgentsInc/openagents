#!/usr/bin/env python3
"""Publish the clean, pushed Coax book through the existing space's draft API.

Standard-library maintenance tooling, never part of a compiler or app build.
Existing pages keep their IDs/URLs. New SUMMARY sections become overview pages.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import urllib.error
import urllib.request
from urllib.parse import unquote, urlsplit

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

def pages(tree):
    for page in tree:
        yield page
        yield from pages(page.get("pages", []))

def slug(title):
    return re.sub(r"[^a-z0-9._-]+", "-", title.lower()).strip("-")

def inventory():
    result, group = [], None
    for line in (ROOT / "coax/book/SUMMARY.md").read_text().splitlines():
        if line.startswith("## "):
            group = line[3:]
        match = re.fullmatch(r"\* \[([^]]+)\]\(([^)]+\.md)\)", line)
        if match:
            title, filename = match.groups()
            result.append((filename, title, group))
    if not result or len({r[0] for r in result}) != len(result):
        raise SystemExit("SUMMARY.md must list unique book files")
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--token-file", type=Path, help="local env file defining GITBOOK_TOKEN")
    parser.add_argument("--change-request", help="resume a previously printed draft ID")
    args = parser.parse_args()
    # Fail locally before token reads or draft creation if source/book/CLI drift.
    subprocess.run([str(ROOT / "scripts/coax-release.sh"), "--check", "--binary",
                    str(ROOT / "coax/bin/coax")], cwd=ROOT, check=True)
    subprocess.run([sys.executable, str(ROOT / "coax/book/check-structure.py")],
                   cwd=ROOT, check=True)
    token = token_from_file(args.token_file) if args.token_file else os.environ.get("GITBOOK_TOKEN")
    if not token:
        parser.error("set GITBOOK_TOKEN or supply --token-file")
    if git("remote", "get-url", "origin") != REPOSITORY:
        raise SystemExit("Refusing to publish from an unexpected origin")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise SystemExit("Commit the worktree before publishing")
    commit = git("rev-parse", "HEAD")
    if commit != git("ls-remote", "origin", "refs/heads/main").split()[0]:
        raise SystemExit("HEAD must equal the pushed origin/main")
    entries = inventory()
    # Read all source before making any remote mutations.
    sources = {name: (ROOT / "coax/book" / name).read_text() for name, _, _ in entries}
    opener = urllib.request.build_opener(NoRedirect())
    def api(path, body=None):
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request("https://api.gitbook.com/v1" + path, data=data,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        try:
            with opener.open(req, timeout=45) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as exc:
            raise SystemExit(f"GitBook HTTP {exc.code}; inspect the printed draft before retrying") from None
        except (OSError, ValueError):
            raise SystemExit("GitBook request failed; inspect the printed draft before retrying") from None
    base = f"/spaces/{SPACE}"
    space = api(base)
    if space["title"] != "The Coax Programming Language" or space["urls"].get("published", "").rstrip("/") != PUBLIC:
        raise SystemExit("The existing book identity no longer matches; no edits requested")
    live = api(base + "/content")
    original_ids = {p["id"] for p in pages(live["pages"])}
    subject = f"Publish Coax book from {commit}"
    if args.change_request:
        draft = api(base + f"/change-requests/{args.change_request}")
        if draft.get("subject") != subject:
            raise SystemExit("Draft subject does not match this commit")
    else:
        draft = api(base + "/change-requests", {"subject": subject})
    draft_id = draft["id"]
    print(f"Draft {draft_id}; source commit {commit}", flush=True)
    draft_base = base + f"/change-requests/{draft_id}"
    tree = api(draft_base + "/content")
    sections = list(dict.fromkeys(group for _, _, group in entries if group))
    for index, group in enumerate(sections):
        if not any(p["title"] == group for p in tree["pages"]):
            # The authoring API creates document pages, not Git Sync-only groups.
            # Use a useful overview with the same section title and child URLs.
            body = f"Practical chapters in this section are listed below.\n\n[Source revision](https://github.com/OpenAgentsInc/openagents/commit/{commit})."
            current = api(draft_base)
            api(draft_base + "/content?compat=false", {"expectedRevision": current["revision"], "changes": [{
                "operation": "insert_page", "title": group, "slug": slug(group),
                "at": 3 + index, "document": {"markdown": body}
            }]})
            tree = api(draft_base + "/content")
    all_pages = list(pages(tree["pages"]))
    by_source = {p.get("git", {}).get("path", "").removeprefix("coax/book/"): p for p in all_pages if p.get("git", {}).get("path")}
    by_slug = {p.get("slug"): p for p in all_pages}
    resolved = {name: by_source.get(name) or by_slug.get(Path(name).stem.lower()) for name, _, _ in entries}
    refs = {name: ("/pages/" + page["id"] if page else "./book-" + Path(name).stem + ".md") for name, page in resolved.items()}
    def markdown(name):
        # GitBook stores the page title separately; keep SUMMARY's reviewed title.
        text = re.sub(r"\A# [^\n]+\n", "", sources[name], count=1).lstrip("\n")
        def link(match):
            url = urlsplit(match.group(1))
            path = unquote(url.path).removeprefix("./")
            if not url.scheme and not url.netloc and path in refs:
                return "](" + refs[path] + ("#" + url.fragment if url.fragment else "") + ")"
            return match.group(0)
        return re.sub(r"\]\(([^)]+)\)", link, text)
    changes = []
    for name, title, group in entries:
        page = resolved[name]
        change = {"operation": "update_page" if page else "insert_page", "title": title, "document": {"markdown": markdown(name)}}
        if page:
            change["page"] = page["id"]
        else:
            change.update({"slug": Path(name).stem.lower(), "ref": "book-" + Path(name).stem})
            if group:
                change["into"] = next(p["id"] for p in tree["pages"] if p["title"] == group)
        changes.append(change)
    if len(changes) > 50:
        raise SystemExit("Book exceeds one atomic content batch; extend the publisher before proceeding")
    current = api(draft_base)
    api(draft_base + "/content?compat=false", {"expectedRevision": current["revision"], "changes": changes})
    tree = api(draft_base + "/content")
    updated = list(pages(tree["pages"]))
    if not original_ids.issubset({p["id"] for p in updated}):
        raise SystemExit("Draft lost an existing page identity; not merged")
    for name, title, _ in entries:
        matches = [p for p in updated if p["title"] == title]
        if len(matches) != 1:
            raise SystemExit("Draft page inventory is incomplete or ambiguous; not merged")
    merged = api(draft_base + "/merge", {})
    if merged["result"] != "merge":
        raise SystemExit("GitBook reported merge conflicts; inspect the space")
    content = api(base + "/content")
    if content["id"] != merged["revision"]:
        raise SystemExit("Published revision does not match the merge; inspect the space")
    print(f"Verified published revision {content['id']} from commit {commit}")
    print(PUBLIC)

if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""File this computer's synced Coder chats under their projects (#11541).

A one-time backfill for chats synced before Coder named each chat's
repository. For every chat on your openagents.com account that came from
this computer and is in no project, it reads the chat's folder from the
saved chat here (`extra.repository`), that folder's GitHub remote
(`owner/name`), and files the chat under the project holding that
`owner/name`: the one project holding it, else the one where it is the
primary; when that is still not one project, the chat stays where it is.
It matches only on that typed repository, never on names. Chats already in a project are left alone, so running it again
changes nothing.

    scripts/coder/file-chats-under-projects.py           # say what it would move
    scripts/coder/file-chats-under-projects.py --apply   # move them

It signs in as `coder login` saved (`~/.openagents/coder/account.json`, or
`--account-dir DIR`) and never prints the sign-in. It moves a chat with
`PATCH /v1/threads/{id}`; a website from before that route moves it the way
the chat's menu on the website does.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


def slug(url):
    """`owner/name` of a GitHub remote URL, or None."""
    url = url.strip()
    match = re.match(r"^(?:[a-z+]+://)?(?:[^@/]+@)?(?:www\.)?github\.com[:/]([^/]+)/([^/]+?)(?:\.git)?/?$", url, re.I)
    return f"{match.group(1)}/{match.group(2)}" if match else None


def repository(folder):
    """The GitHub repository of the checkout at `folder`: origin first."""
    if not folder or not os.path.isdir(folder):
        return None
    try:
        out = subprocess.run(
            ["git", "config", "--get-regexp", r"^remote\..*\.url$"],
            cwd=folder, capture_output=True, text=True, timeout=10,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return None
    remotes = []
    for line in out.splitlines():
        key, _, url = line.partition(" ")
        name = key[len("remote."):-len(".url")]
        found = slug(url)
        if found:
            remotes.append((name, found))
    for name, found in remotes:
        if name == "origin":
            return found
    return remotes[0][1] if remotes else None


class Site:
    def __init__(self, account_dir):
        with open(os.path.join(account_dir, "account.json")) as f:
            saved = json.load(f)
        self.origin = saved["origin"].rstrip("/")
        self._token = saved["token"]

    def call(self, method, path, body=None):
        request = urllib.request.Request(
            self.origin + path, method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Authorization": "Bearer " + self._token,
                     "Content-Type": "application/json", "User-Agent": "openagents-cli"},
        )
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.status, json.loads(response.read() or b"null")
        except urllib.error.HTTPError as error:
            return error.code, None

    def move_on_page(self, chat, project):
        """The website's own Move to project, for a site without the route."""
        class Stay(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, *args, **kwargs):
                return None
        opener = urllib.request.build_opener(Stay)
        cookie = {"Cookie": "oa_cloud_session=" + self._token, "User-Agent": "openagents-cli"}
        page = opener.open(urllib.request.Request(f"{self.origin}/chat/{chat}", headers=cookie), timeout=60)
        found = re.search(r'name="csrf"\s+value="([^"]+)"', page.read().decode())
        if not found:
            return False
        form = urllib.parse.urlencode({"csrf": found.group(1), "project": project}).encode()
        request = urllib.request.Request(
            f"{self.origin}/chat/{chat}/project", data=form, method="POST",
            headers={**cookie, "Content-Type": "application/x-www-form-urlencoded"})
        try:
            status = opener.open(request, timeout=60).status
        except urllib.error.HTTPError as error:
            status = error.code
        return status in (200, 303)


def repositories_of(project):
    """A project's repositories as (owner/name, primary); an older site
    names only `repository`."""
    if "repositories" in project:
        return [(r["repository"], bool(r.get("primary"))) for r in project["repositories"]]
    return [(project["repository"], True)] if project.get("repository") else []


def filing_project(projects, repo):
    """The project a chat on `repo` files under, as the website does: the
    one project holding it, else the one where it is the primary, else
    none (never a guess)."""
    if not repo:
        return None
    repo = repo.lower()
    holding = [p for p in projects if any(r.lower() == repo for r, _ in repositories_of(p))]
    if len(holding) == 1:
        return holding[0]
    primary = [p for p in holding if any(r.lower() == repo and first for r, first in repositories_of(p))]
    return primary[0] if len(primary) == 1 else None


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--apply", action="store_true", help="move the chats (otherwise only list them)")
    parser.add_argument("--account-dir", default=os.path.expanduser("~/.openagents/coder"))
    args = parser.parse_args()
    site = Site(args.account_dir)
    status, body = site.call("GET", "/v1/projects?limit=200")
    if status != 200:
        sys.exit(f"Couldn't read your projects (the website answered {status}).")
    listed = body["data"]
    status, body = site.call("GET", "/v1/threads?limit=200")
    if status != 200:
        sys.exit(f"Couldn't read your chats (the website answered {status}).")
    threads = list(body["threads"])
    while body.get("more"):
        status, body = site.call("GET", f"/v1/threads?offset={len(threads)}&limit=200")
        if status != 200:
            break
        threads.extend(body["threads"])
    # The chats in no project, as the projects API lists them.
    loose, after = set(), None
    while True:
        path = "/v1/threads?project=none&limit=200" + (f"&after={after}" if after else "")
        status, page = site.call("GET", path)
        if status != 200:
            sys.exit(f"Couldn't read your chats outside projects (the website answered {status}).")
        loose.update(row["id"] for row in page["data"])
        if not page.get("next") or page["next"] == after:
            break
        after = page["next"]
    sessions = os.path.join(args.account_dir, "sessions")
    moved = 0
    for thread in threads:
        session = thread.get("session") or ""
        if thread.get("surface") != "terminal" or thread["id"] not in loose or not session:
            continue
        path = os.path.join(sessions, session + ".atif.json")
        if not os.path.exists(path):
            continue
        try:
            with open(path) as f:
                folder = (json.load(f).get("extra") or {}).get("repository")
        except (OSError, ValueError):
            continue
        repo = repository(folder)
        project = filing_project(listed, repo)
        if not project:
            continue
        title = thread["title"].splitlines()[0][:70] if thread["title"] else "New chat"
        print(f"{thread['id']}  {project['name']}  {title}")
        if not args.apply:
            continue
        status, _ = site.call("PATCH", f"/v1/threads/{thread['id']}", {"project": project["id"]})
        done = status == 200 or (status == 405 and site.move_on_page(thread["id"], project["id"]))
        if done:
            moved += 1
        else:
            print(f"  not moved (the website answered {status})")
    print(f"{'Moved' if args.apply else 'Would move'} {moved if args.apply else 'the chats above'}.")


if __name__ == "__main__":
    main()

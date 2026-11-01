#!/usr/bin/env python3
"""Publish a clean, pushed book written in Coax (`--book coax|psionic|aethyr|nostr`) through its GitBook space's draft API.

Standard-library maintenance tooling, never part of a compiler or app build.
Existing pages keep their IDs/URLs. New SUMMARY sections become overview pages.
Pages that SUMMARY no longer lists are deleted, and their old URLs redirect to
the current page with the same name, or to the book's first page.
After the merge, the live page inventory must equal SUMMARY (`--check-live`
runs only that comparison).
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
REPOSITORY = "https://github.com/OpenAgentsInc/openagents.git"
BLOB = "https://github.com/OpenAgentsInc/openagents/blob/"
BATCH = 50  # changes per atomic content request


def coax_gate():
    # Fail locally before token reads or draft creation if source/book/CLI drift.
    subprocess.run([str(ROOT / "scripts/coax-release.sh"), "--check", "--binary",
                    str(ROOT / "coax/bin/coax")], cwd=ROOT, check=True)
    subprocess.run([sys.executable, str(ROOT / "coax/book/check-structure.py")],
                   cwd=ROOT, check=True)


def book_gate(name):
    def gate():
        subprocess.run([sys.executable, str(ROOT / "psionic/tests/book.py"), "--book", name],
                       cwd=ROOT, check=True)
    return gate


# book: (directory, space ID, space title, public URL, gate, prune pages missing from SUMMARY)
BOOKS = {
    "coax": ("coax/book", "sZhQ0yptSOKMneBDWMti", "The Coax Programming Language",
             "https://openagents.gitbook.io/the-coax-programming-language", coax_gate, True),
    "psionic": ("psionic/book", "Sha9csfX5lAIHvjiIheN", "Psionic: ML From Scratch",
                "https://openagents.gitbook.io/psionic-ml-from-scratch", book_gate("psionic"), True),
    "aethyr": ("aethyr/book", "QxVdkZGgQYnLsXWZlkTh", "Building Aethyr in Coax",
               "https://openagents.gitbook.io/building-aethyr-in-coax", book_gate("aethyr"), True),
    "nostr": ("nostr/book", "FJJZf8YunJx2R1toVsuv", "Nostr From Scratch",
              "https://openagents.gitbook.io/nostr-from-scratch", book_gate("nostr"), True),
}
# Coax keeps its original constants for callers and tests.
SPACE = BOOKS["coax"][1]
PUBLIC = BOOKS["coax"][3]


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


def inventory(directory="coax/book"):
    """SUMMARY entries as (file, title, group); groups are `## X` or `**X**` lines."""
    result, group = [], None
    for line in (ROOT / directory / "SUMMARY.md").read_text().splitlines():
        heading = re.fullmatch(r"## (.+)|\*\*([^*]+)\*\*", line.strip())
        if heading:
            group = heading.group(1) or heading.group(2)
        # A note may follow the link (`- [Title](file.md) — note`).
        match = re.fullmatch(r"[*-] \[([^]]+)\]\(([^)]+\.md)\)(?:\s.*)?", line)
        if match:
            title, filename = match.groups()
            result.append((filename, title, group))
    if not result or len({r[0] for r in result}) != len(result):
        raise SystemExit("SUMMARY.md must list unique book files")
    return result


def expected_tree(entries):
    """The page tree SUMMARY implies: [(title, [child titles])] in order."""
    tree, groups = [], {}
    for _, title, group in entries:
        if not group:
            tree.append((title, []))
        else:
            if group not in groups:
                groups[group] = []
                tree.append((group, groups[group]))
            groups[group].append(title)
    return tree


def live_problems(live_pages, entries):
    """Differences between a live GitBook page tree and SUMMARY: extra, missing or retitled pages."""
    want = expected_tree(entries)
    have = [(p["title"], [c["title"] for c in p.get("pages", [])],
             [g["title"] for c in p.get("pages", []) for g in c.get("pages", [])]) for p in live_pages]
    errors = []
    want_top, have_top = [t for t, _ in want], [t for t, _, _ in have]
    for title in have_top:
        if title not in want_top:
            errors.append(f"extra page: {title}")
    for title in want_top:
        if title not in have_top:
            errors.append(f"missing page: {title}")
    if not errors and want_top != have_top:
        errors.append("top-level pages are out of SUMMARY order")
    for title, children in want:
        found = [h for h in have if h[0] == title]
        if len(found) != 1:
            if found:
                errors.append(f"duplicate page: {title}")
            continue
        _, live_children, grandchildren = found[0]
        for child in live_children:
            if child not in children:
                errors.append(f"extra page under {title}: {child}")
        for child in children:
            if child not in live_children:
                errors.append(f"missing page under {title}: {child}")
        if len(set(live_children)) != len(live_children):
            errors.append(f"duplicate pages under {title}")
        elif set(live_children) == set(children) and live_children != children:
            errors.append(f"pages under {title} are out of SUMMARY order")
        errors += [f"extra nested page under {title}: {g}" for g in grandchildren]
    return errors


def redirect_target(old, current):
    """The current page an old page's URL should reach: same file name without its number, else None."""
    def name(path):
        last = (path or "").rstrip("/").rsplit("/", 1)[-1]
        return re.sub(r"^(ch\d+|part-[ivxl]+\.)-", "", last)
    matches = [p for p in current if p.get("path") and name(p["path"]) == name(old)]
    return matches[0]["path"] if len(matches) == 1 else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--book", choices=sorted(BOOKS), default="coax")
    parser.add_argument("--token-file", type=Path, help="local env file defining GITBOOK_TOKEN")
    parser.add_argument("--change-request", help="resume a previously printed draft ID")
    parser.add_argument("--check-live", action="store_true",
                        help="only compare the live page inventory with SUMMARY; change nothing")
    args = parser.parse_args()
    directory, space_id, space_title, public, gate, prune = BOOKS[args.book]
    if not args.check_live:
        gate()
    token = token_from_file(args.token_file) if args.token_file else os.environ.get("GITBOOK_TOKEN")
    if not token:
        parser.error("set GITBOOK_TOKEN or supply --token-file")
    if args.check_live:
        check_live(args.book, token)
        return
    if git("remote", "get-url", "origin") != REPOSITORY:
        raise SystemExit("Refusing to publish from an unexpected origin")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise SystemExit("Commit the worktree before publishing")
    commit = git("rev-parse", "HEAD")
    if commit != git("ls-remote", "origin", "refs/heads/main").split()[0]:
        raise SystemExit("HEAD must equal the pushed origin/main")
    entries = inventory(directory)
    # Read all source before making any remote mutations.
    sources = {name: (ROOT / directory / name).read_text() for name, _, _ in entries}
    readmes = {BOOKS[b][0] + "/README.md": BOOKS[b][3] for b in BOOKS}
    opener = urllib.request.build_opener(NoRedirect())

    def api(path, body=None, method=None):
        if method == "PUT":
            body = {"redirects": body}
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request("https://api.gitbook.com/v1" + path, data=data, method=method,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        try:
            with opener.open(req, timeout=45) as response:
                raw = response.read()
                return json.loads(raw) if raw else None
        except urllib.error.HTTPError as exc:
            raise SystemExit(f"GitBook HTTP {exc.code}; inspect the printed draft before retrying") from None
        except (OSError, ValueError):
            raise SystemExit("GitBook request failed; inspect the printed draft before retrying") from None

    base = f"/spaces/{space_id}"
    space = api(base)
    if space["title"] != space_title or space["urls"].get("published", "").rstrip("/") != public:
        raise SystemExit("The existing book identity no longer matches; no edits requested")
    live = api(base + "/content")
    original_ids = {p["id"] for p in pages(live["pages"])}
    subject = f"Publish {args.book.capitalize()} book from {commit}"
    if args.change_request:
        draft = api(base + f"/change-requests/{args.change_request}")
        if draft.get("subject") != subject:
            raise SystemExit("Draft subject does not match this commit")
    else:
        draft = api(base + "/change-requests", {"subject": subject})
    draft_id = draft["id"]
    print(f"Draft {draft_id}; source commit {commit}", flush=True)
    draft_base = base + f"/change-requests/{draft_id}"

    def apply(changes):
        for start in range(0, len(changes), BATCH):
            current = api(draft_base)
            api(draft_base + "/content?compat=false", {"expectedRevision": current["revision"],
                                                       "changes": changes[start:start + BATCH]})
        return api(draft_base + "/content")

    tree = api(draft_base + "/content")
    sections = list(dict.fromkeys(group for _, _, group in entries if group))
    leading = sum(1 for _, _, group in entries if not group)
    def same_group(page, group):
        # A section renamed only by dropping " (planned)" keeps its page and URL.
        return page["title"] == group or page["title"].removesuffix(" (planned)") == group
    for index, group in enumerate(sections):
        if not any(same_group(p, group) for p in tree["pages"]):
            # The authoring API creates document pages, not Git Sync-only groups.
            # Use a useful overview with the same section title and child URLs.
            body = f"Practical chapters in this section are listed below.\n\n[Source revision](https://github.com/OpenAgentsInc/openagents/commit/{commit})."
            tree = apply([{"operation": "insert_page", "title": group, "slug": slug(group),
                           "at": min(leading + index, len(tree["pages"])),
                           "document": {"markdown": body}}])
    group_ids = {group: next(p["id"] for p in tree["pages"] if p["title"] == group)
                 if any(p["title"] == group for p in tree["pages"])
                 else next(p["id"] for p in tree["pages"] if same_group(p, group)) for group in sections}

    def resolve(tree):
        all_pages = list(pages(tree["pages"]))
        prefix = directory + "/"
        by_source = {p.get("git", {}).get("path", "").removeprefix(prefix): p for p in all_pages if p.get("git", {}).get("path")}
        by_slug = {p.get("slug"): p for p in all_pages}
        return {name: by_source.get(name) or by_slug.get(Path(name).stem.lower())
                for name, _, _ in entries}

    # Insert missing pages first so every page, old or new, links by stable page ID.
    resolved = resolve(tree)
    missing = [{"operation": "insert_page", "title": title, "slug": Path(name).stem.lower(),
                "document": {"markdown": "Publishing."}, **({"into": group_ids[group]} if group else {})}
               for name, title, group in entries if not resolved[name]]
    if missing:
        tree = apply(missing)
        resolved = resolve(tree)
    if not all(resolved.values()):
        raise SystemExit("Draft is missing inserted pages; not merged")
    refs = {name: "/pages/" + page["id"] for name, page in resolved.items()}

    def markdown(name):
        # GitBook stores the page title separately; keep SUMMARY's reviewed title.
        text = re.sub(r"\A# [^\n]+\n", "", sources[name], count=1).lstrip("\n")

        def link(match):
            url = urlsplit(match.group(1))
            path = unquote(url.path).removeprefix("./")
            fragment = "#" + url.fragment if url.fragment else ""
            if not url.scheme and not url.netloc and path in refs:
                return "](" + refs[path] + fragment + ")"
            if prune and not url.scheme and not url.netloc and path:
                # Links out of this book go to the other book or to the source revision.
                target = os.path.normpath(os.path.join(directory, path))
                if target in readmes:
                    return "](" + readmes[target] + ")"
                if not target.startswith("..") and (ROOT / target).exists():
                    return "](" + BLOB + commit + "/" + target + fragment + ")"
            return match.group(0)
        return re.sub(r"\]\(([^)]+)\)", link, text)

    # Move every page into SUMMARY order: each section's overview precedes its chapters.
    changes, position, placed = [], {}, set()
    def place(page, parent, **extra):
        at = position.get(parent, 0)
        position[parent] = at + 1
        changes.append({"operation": "update_page", "page": page, "into": parent, "at": at, **extra})
    for name, title, group in entries:
        if group and group not in placed:
            placed.add(group)
            place(group_ids[group], None, title=group)
        place(resolved[name]["id"], group_ids[group] if group else None,
              title=title, document={"markdown": markdown(name)})
    keep = {page["id"] for page in resolved.values()} | set(group_ids.values())
    tree = apply(changes)
    # Every kept page is now in place; delete the topmost pages SUMMARY doesn't list.
    orphans, deleted = [], set()
    def collect(tree_pages, inside_orphan=False):
        for p in tree_pages:
            if p["id"] in keep and inside_orphan:
                raise SystemExit("A listed page is still inside an unlisted one; not merged")
            orphan = p["id"] not in keep
            if orphan and not inside_orphan:
                orphans.append(p["id"])
            if orphan or inside_orphan:
                deleted.add(p["id"])
            collect(p.get("pages", []), inside_orphan or orphan)
    collect(tree["pages"])
    if orphans and prune:
        tree = apply([{"operation": "delete_page", "page": i} for i in orphans])
    updated = list(pages(tree["pages"]))
    if not prune and not original_ids.issubset({p["id"] for p in updated}):
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
    # Old URLs of deleted pages redirect to the current page with the same name.
    current = list(pages(content["pages"]))
    first = next((p["path"] for p in current if p.get("path") is not None), "")
    old_paths = [p["path"] for p in pages(live["pages"]) if p["id"] in deleted and p.get("path")]
    if prune and old_paths:
        site = find_site(api, space["organization"], public)
        redirects = [{"source": "/" + old, "intent": "live", "permanent": True,
                      "destination": {"kind": "path", "spaceId": space_id,
                                      "path": redirect_target(old, current) or first}}
                     for old in old_paths if old not in {p.get("path") for p in current}]
        for start in range(0, len(redirects), 500):
            api(f"/orgs/{space['organization']}/sites/{site}/redirects", redirects[start:start + 500],
                method="PUT")
        print(f"Redirected {len(redirects)} removed page URLs")
    problems = live_problems(content["pages"], entries)
    if problems:
        print("\n".join(problems), file=sys.stderr)
        raise SystemExit("The live page inventory differs from SUMMARY.md")
    print(f"Live page inventory matches SUMMARY.md: {sum(1 for _ in pages(content['pages']))} pages")
    print(public)


def find_site(api, organization, public):
    for site in api(f"/orgs/{organization}/sites?limit=100")["items"]:
        if (site.get("urls", {}).get("published") or "").rstrip("/") == public:
            return site["id"]
    raise SystemExit("No published site matches the book; pages merged, redirects not created")


def check_live(book, token):
    """Compare a book's live page inventory with SUMMARY; read-only."""
    directory, space_id, _, public, _, _ = BOOKS[book]
    req = urllib.request.Request(f"https://api.gitbook.com/v1/spaces/{space_id}/content",
                                 headers={"Authorization": "Bearer " + token})
    try:
        with urllib.request.urlopen(req, timeout=45) as response:
            content = json.loads(response.read())
    except (OSError, ValueError):
        raise SystemExit("GitBook request failed") from None
    problems = live_problems(content["pages"], inventory(directory))
    if problems:
        print("\n".join(problems), file=sys.stderr)
        raise SystemExit(f"{book}: the live page inventory differs from SUMMARY.md")
    print(f"{book}: live page inventory matches SUMMARY.md "
          f"({sum(1 for _ in pages(content['pages']))} pages, revision {content['id']}) at {public}")


if __name__ == "__main__":
    main()

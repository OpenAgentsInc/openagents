#!/usr/bin/env python3
"""Publish a clean, pushed book written in Coax (`--book coax|psionic|aethyr|nostr|coder|bitcoin`) through its GitBook space's draft API.

Standard-library maintenance tooling, never part of a compiler or app build.
Existing pages keep their IDs/URLs. New SUMMARY sections become overview pages.
Pages that SUMMARY no longer lists are deleted, and their old URLs redirect to
the current page with the same name, the same page in the book a chapter moved
to (MOVED_TO), or the book's first page. A kept page whose URL changed (its
section was renamed) redirects from its old URL.
After the merge, the live page inventory must equal SUMMARY (`--check-live`
runs that read-only comparison). A successful publication writes an atomic
source/revision receipt. `--check-receipt` validates its local source provenance
without credentials or network; `--check-live --require-receipt` also requires
the recorded live revision and page identities. Live document bodies are not
independently fetched or hashed by these checks.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/OpenAgentsInc/openagents.git"
BLOB = "https://github.com/OpenAgentsInc/openagents/blob/"
BATCH = 50  # changes per atomic content request
RECEIPT_SCHEMA = 1
VERIFICATION_SCOPE = ("Source and submitted-markdown hashes record publication provenance; "
                      "live revision and page inventory were checked, not live document-body parity.")


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
    "aethyr": ("aethyr/book", "QxVdkZGgQYnLsXWZlkTh", "Building Aethyr, World Engine",
               "https://openagents.gitbook.io/building-aethyr-world-engine", book_gate("aethyr"), True),
    "nostr": ("nostr/book", "FJJZf8YunJx2R1toVsuv", "Nostr From Scratch",
              "https://openagents.gitbook.io/nostr-from-scratch", book_gate("nostr"), True),
    "coder": ("coder/book", "lS8V6TGnz1ifNhKVV3hN", "Coder: Coding Agents From Scratch",
              "https://openagents.gitbook.io/coder-coding-agents-from-scratch", book_gate("coder"), True),
    "bitcoin": ("bitcoin/book", "wmf6C6tD4ovtQQqoeMmP", "Bitcoin From Scratch",
                "https://openagents.gitbook.io/bitcoin-from-scratch", book_gate("bitcoin"), True),
}
# Chapters that moved to another book: a deleted page whose file name matches a
# page there redirects to that book's page (Nostr chapters 1-13, 15 and 18
# became Bitcoin chapters).
MOVED_TO = {"nostr": "bitcoin"}
# Coax keeps its original constants for callers and tests.
SPACE = BOOKS["coax"][1]
PUBLIC = BOOKS["coax"][3]


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def git_bytes(*args):
    """Read exact Git blob bytes, including trailing newlines."""
    try:
        return subprocess.check_output(["git", *args], cwd=ROOT, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        raise SystemExit("Publication source is missing from the recorded Git commit") from None


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False,
                                    separators=(",", ":")).encode()).hexdigest()


def file_identity(path, raw):
    return {"path": path, "bytes": len(raw), "sha256": hashlib.sha256(raw).hexdigest()}


def source_snapshot(book, commit):
    """Bind SUMMARY and every listed file to exact bytes in an immutable commit.

    This is local source provenance, not a claim about server document bytes.
    Checking an older receipt permits unrelated later repository commits.
    """
    if not isinstance(commit, str) or not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit):
        raise SystemExit("Publication source requires a full Git commit ID")
    if git_bytes("rev-parse", "--verify", commit + "^{commit}").decode().strip() != commit:
        raise SystemExit("Publication source commit is not an exact commit identity")
    directory = BOOKS[book][0]
    result = {"commit": commit, "directory": directory}
    seen = set()

    def source_file(name):
        candidate = Path(os.path.normpath(ROOT.resolve() / directory / name))
        path = candidate.resolve()
        try:
            relative = path.relative_to(ROOT.resolve()).as_posix()
        except ValueError:
            raise SystemExit("Publication source escapes the repository") from None
        if candidate != path:
            raise SystemExit("Publication source must not traverse symlinks")
        if relative in seen:
            raise SystemExit("Publication source contains duplicate file paths")
        seen.add(relative)
        try:
            raw = path.read_bytes()
        except OSError:
            raise SystemExit("Publication source file cannot be read") from None
        if git_bytes("show", commit + ":" + relative) != raw:
            raise SystemExit("Publication source bytes differ from the recorded commit: " + relative)
        return file_identity(relative, raw)

    result["summary"] = source_file("SUMMARY.md")
    result["pages"] = [dict(source_file(name), file=name, title=title, group=group)
                       for name, title, group in inventory(directory)]
    result["sha256"] = digest(result)
    return result


def live_inventory(tree):
    """Retain observed tree order, page identities, titles and public paths only."""
    seen = set()

    def page(item):
        page_id, title, path = item["id"], item["title"], item.get("path")
        if (not isinstance(page_id, str) or not page_id or page_id in seen or
                not isinstance(title, str) or not title or
                (path is not None and not isinstance(path, str))):
            raise SystemExit("Invalid live page identity in publication evidence")
        seen.add(page_id)
        return {"id": page_id, "title": title, "path": path,
                "pages": [page(child) for child in item.get("pages", [])]}
    return [page(item) for item in tree]


def publication_receipt(book, source, draft_id, content, submitted_markdown):
    entries = [(p["file"], p["title"], p["group"]) for p in source["pages"]]
    if live_problems(content["pages"], entries):
        raise SystemExit("Cannot record a publication whose live inventory differs from source")
    observed = live_inventory(content["pages"])
    return {"schema": RECEIPT_SCHEMA, "repository": REPOSITORY, "book": book,
            "space_id": BOOKS[book][1], "public_url": BOOKS[book][3],
            "published_at": datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z"),
            "source": source, "change_request": draft_id,
            "submitted_markdown": submitted_markdown,
            "submitted_markdown_sha256": digest(submitted_markdown),
            "verified_live": {"revision": content["id"], "inventory": observed,
                              "sha256": digest(observed)},
            "verification_scope": VERIFICATION_SCOPE}


def write_receipt(path, receipt):
    """Replace a success receipt atomically; a failed write preserves its predecessor."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix="." + path.name + ".", delete=False) as out:
            temporary = Path(out.name)
            json.dump(receipt, out, indent=2, ensure_ascii=False)
            out.write("\n")
            out.flush()
            os.fsync(out.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def check_receipt(book, path, content=None):
    """Check local source identity and optionally the observed live revision/tree.

    This never authenticates or accesses the network. A receipt is retained
    maintenance evidence, not a server signature or proof of live body parity.
    """
    try:
        receipt = json.loads(Path(path).read_text())
        if (receipt["schema"] != RECEIPT_SCHEMA or receipt["repository"] != REPOSITORY or
                receipt["book"] != book or receipt["space_id"] != BOOKS[book][1] or
                receipt["public_url"] != BOOKS[book][3] or
                receipt["verification_scope"] != VERIFICATION_SCOPE):
            raise ValueError("identity")
        when = datetime.fromisoformat(receipt["published_at"].replace("Z", "+00:00"))
        if when.utcoffset() != timezone.utc.utcoffset(when):
            raise ValueError("timestamp")
        if not isinstance(receipt["change_request"], str) or not receipt["change_request"]:
            raise ValueError("change request")
        source = receipt["source"]
        if source_snapshot(book, source["commit"]) != source:
            raise ValueError("source digest")
        observed = receipt["verified_live"]
        if not isinstance(observed["revision"], str) or not observed["revision"]:
            raise ValueError("revision")
        normalized = live_inventory(observed["inventory"])
        if normalized != observed["inventory"] or digest(normalized) != observed["sha256"]:
            raise ValueError("live inventory digest")
        entries = [(p["file"], p["title"], p["group"]) for p in source["pages"]]
        if live_problems(normalized, entries):
            raise ValueError("live inventory")
        submitted = receipt["submitted_markdown"]
        if (not isinstance(submitted, list) or len(submitted) != len(source["pages"]) or
                digest(submitted) != receipt["submitted_markdown_sha256"]):
            raise ValueError("submitted markdown digest")
        live_pages = list(pages(normalized))
        for document, original in zip(submitted, source["pages"]):
            matching = [p for p in live_pages if p["id"] == document["page_id"]]
            if (document["file"] != original["file"] or document["path"] != original["file"] or
                    document["title"] != original["title"] or len(matching) != 1 or
                    matching[0]["title"] != original["title"] or
                    type(document["bytes"]) is not int or document["bytes"] < 0 or
                    not re.fullmatch(r"[0-9a-f]{64}", document["sha256"])):
                raise ValueError("submitted markdown identity")
        if content is not None and (content["id"] != observed["revision"] or
                                   live_inventory(content["pages"]) != normalized):
            raise ValueError("live revision or page identities differ from receipt")
    except (OSError, ValueError, TypeError, KeyError, AttributeError):
        raise SystemExit("Invalid or stale publication receipt: " + str(path)) from None
    return receipt


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
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--check-live", action="store_true",
                       help="compare live inventory and any existing receipt; change nothing")
    modes.add_argument("--check-receipt", action="store_true",
                       help="validate local publication source receipt; no credentials or network")
    parser.add_argument("--receipt", type=Path,
                        help="receipt file (default: docs/books/publications/BOOK.json)")
    parser.add_argument("--require-receipt", action="store_true",
                        help="require a valid source/revision receipt with --check-live")
    args = parser.parse_args()
    directory, space_id, space_title, public, gate, prune = BOOKS[args.book]
    receipt_path = args.receipt or ROOT / "docs/books/publications" / (args.book + ".json")
    if args.require_receipt and not args.check_live:
        parser.error("--require-receipt requires --check-live")
    if args.check_receipt:
        receipt = check_receipt(args.book, receipt_path)
        print(f"{args.book}: local publication source matches receipt at {receipt_path}; "
              f"commit {receipt['source']['commit']}, recorded revision {receipt['verified_live']['revision']}")
        print("No network check performed; live document-body parity is not verified.")
        return
    if args.check_live:
        if args.require_receipt or args.receipt is not None or receipt_path.exists():
            check_receipt(args.book, receipt_path)  # fail locally before credentials/network
        else:
            receipt_path = None
    else:
        gate()
        if git("remote", "get-url", "origin") != REPOSITORY:
            raise SystemExit("Refusing to publish from an unexpected origin")
        if git("status", "--porcelain", "--untracked-files=all"):
            raise SystemExit("Commit the worktree before publishing")
        commit = git("rev-parse", "HEAD")
        if commit != git("ls-remote", "origin", "refs/heads/main").split()[0]:
            raise SystemExit("HEAD must equal the pushed origin/main")
        source = source_snapshot(args.book, commit)
        entries = [(p["file"], p["title"], p["group"]) for p in source["pages"]]
        # Upload bytes from the immutable recorded commit, not a later worktree edit.
        sources = {p["file"]: git_bytes("show", commit + ":" + p["path"]).decode("utf-8")
                   for p in source["pages"]}
    token = token_from_file(args.token_file) if args.token_file else os.environ.get("GITBOOK_TOKEN")
    if not token:
        parser.error("set GITBOOK_TOKEN or supply --token-file")
    if args.check_live:
        check_live(args.book, token, receipt_path)
        return
    readmes = {BOOKS[b][0] + "/README.md": BOOKS[b][3] for b in BOOKS}
    opener = urllib.request.build_opener(NoRedirect())

    def api(path, body=None, method=None):
        if method == "PUT":
            body = {"redirects": body}
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request("https://api.gitbook.com/v1" + path, data=data, method=method,
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"})
        try:
            # Creating hundreds of redirects in one PUT can take over a minute.
            with opener.open(req, timeout=240 if method == "PUT" else 45) as response:
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
    changes, position, placed, submitted_markdown = [], {}, set(), []
    def place(page, parent, **extra):
        at = position.get(parent, 0)
        position[parent] = at + 1
        changes.append({"operation": "update_page", "page": page, "into": parent, "at": at, **extra})
    for name, title, group in entries:
        if group and group not in placed:
            placed.add(group)
            place(group_ids[group], None, title=group)
        body = markdown(name)
        submitted_markdown.append(dict(file_identity(name, body.encode("utf-8")),
                                       file=name, page_id=resolved[name]["id"], title=title))
        place(resolved[name]["id"], group_ids[group] if group else None,
              title=title, document={"markdown": body})
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
    # Old URLs of deleted pages redirect to the current page with the same name,
    # or to the same page in the book it moved to. A kept page whose section
    # was renamed or moved keeps working at its old URL too.
    current = list(pages(content["pages"]))
    first = next((p["path"] for p in current if p.get("path") is not None), "")
    now = {p.get("path") for p in current}
    by_id = {p["id"]: p.get("path") for p in current}
    elsewhere = []
    if args.book in MOVED_TO:
        other = BOOKS[MOVED_TO[args.book]]
        elsewhere = list(pages(api(f"/spaces/{other[1]}/content")["pages"]))

    def destination(page):
        if page["id"] in by_id and by_id[page["id"]]:
            return {"kind": "path", "spaceId": space_id, "path": by_id[page["id"]]}
        target = redirect_target(page["path"], current)
        moved = None if target else redirect_target(page["path"], elsewhere)
        if moved:
            return {"kind": "external", "url": other[3] + "/" + moved}
        return {"kind": "path", "spaceId": space_id, "path": target or first}
    old_pages = [p for p in pages(live["pages"]) if p.get("path") and p["path"] not in now
                 and (p["id"] in by_id or (prune and p["id"] in deleted))]
    if old_pages:
        site = find_site(api, space["organization"], public)
        redirects = [{"source": "/" + p["path"], "intent": "live", "permanent": True,
                      "destination": destination(p)} for p in old_pages]
        for start in range(0, len(redirects), 500):
            api(f"/orgs/{space['organization']}/sites/{site}/redirects", redirects[start:start + 500],
                method="PUT")
        print(f"Redirected {len(redirects)} removed page URLs")
    problems = live_problems(content["pages"], entries)
    if problems:
        print("\n".join(problems), file=sys.stderr)
        raise SystemExit("The live page inventory differs from SUMMARY.md")
    # Never replace prior success evidence after a failed merge, redirect or check.
    # Refuse a concurrent local source edit even though the upload used Git blobs.
    if source_snapshot(args.book, commit) != source:
        raise SystemExit("Publication source changed while publishing; no receipt written")
    receipt = publication_receipt(args.book, source, draft_id, content, submitted_markdown)
    write_receipt(receipt_path, receipt)
    print(f"Live page inventory matches SUMMARY.md: {sum(1 for _ in pages(content['pages']))} pages")
    print(f"Publication receipt: {receipt_path}; live document-body parity is not independently verified")
    print(public)


def find_site(api, organization, public):
    for site in api(f"/orgs/{organization}/sites?limit=100")["items"]:
        if (site.get("urls", {}).get("published") or "").rstrip("/") == public:
            return site["id"]
    raise SystemExit("No published site matches the book; pages merged, redirects not created")


def check_live(book, token, receipt_path=None):
    """Compare live inventory and optionally a prior receipt; never writes evidence."""
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
    if receipt_path is not None:
        check_receipt(book, receipt_path, content)
        scope = "source receipt and live revision/page identities match; live body parity not verified"
    else:
        scope = "inventory only; no source receipt or live body parity verified"
    print(f"{book}: live page inventory matches SUMMARY.md "
          f"({sum(1 for _ in pages(content['pages']))} pages, revision {content['id']}) at {public}; {scope}")


if __name__ == "__main__":
    main()

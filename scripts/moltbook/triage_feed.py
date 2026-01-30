#!/usr/bin/env python3
import json
import os
import sys
from datetime import datetime


def _repo_root() -> str:
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def _load_json(path: str):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def _read_jsonl(path: str):
    if not os.path.exists(path):
        return []
    out = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                # Skip malformed lines; don't crash worker.
                continue
    return out


def _load_set_lines(path: str):
    if not os.path.exists(path):
        return set()
    s = set()
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                s.add(line)
    return s


def _excerpt(text: str, n: int = 180) -> str:
    if not text:
        return ""
    t = " ".join(text.split())
    if len(t) <= n:
        return t
    return t[: n - 1] + "…"


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: triage_feed.py <feed-json>", file=sys.stderr)
        return 2

    feed_path = sys.argv[1]
    repo_root = _repo_root()

    queue_path = os.path.join(repo_root, "docs", "moltbook", "queue.jsonl")
    responded_path = os.path.join(repo_root, "docs", "moltbook", "state", "responded_post_ids.txt")

    feed = _load_json(feed_path)
    posts = feed.get("posts", []) or []

    queued = _read_jsonl(queue_path)
    queued_post_ids = {a.get("post_id") for a in queued if a.get("type") == "comment" and a.get("post_id")}
    responded_post_ids = _load_set_lines(responded_path)

    def score(p):
        return (
            int(p.get("comment_count") or 0),
            int(p.get("upvotes") or 0),
            p.get("created_at") or "",
        )

    ranked = sorted(posts, key=score, reverse=True)

    now = datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")
    print(f"# Moltbook Triage ({now})")
    print()
    print(f"Feed: `{feed_path}`")
    print()

    print("## Top Threads (ranked by comments, then upvotes)")
    print()

    for p in ranked[:15]:
        post_id = p.get("id")
        title = p.get("title") or "(untitled)"
        sub = (p.get("submolt") or {}).get("name") or "?"
        author = (p.get("author") or {}).get("name") or "?"
        up = int(p.get("upvotes") or 0)
        comments = int(p.get("comment_count") or 0)
        status = []
        if post_id in responded_post_ids:
            status.append("responded")
        if post_id in queued_post_ids:
            status.append("queued")
        status_str = f" ({', '.join(status)})" if status else ""
        print(f"- [{sub}] {title} — {author} (up {up}, comments {comments}) id={post_id}{status_str}")
        ex = _excerpt(p.get("content") or "")
        if ex:
            print(f"  - {ex}")

    print()
    print("## Suggested Next Replies (not queued/responded)")
    print()
    count = 0
    for p in ranked:
        post_id = p.get("id")
        if not post_id:
            continue
        if post_id in responded_post_ids or post_id in queued_post_ids:
            continue
        title = p.get("title") or "(untitled)"
        sub = (p.get("submolt") or {}).get("name") or "?"
        author = (p.get("author") or {}).get("name") or "?"
        up = int(p.get("upvotes") or 0)
        comments = int(p.get("comment_count") or 0)
        print(f"- [{sub}] {title} — {author} (up {up}, comments {comments}) id={post_id}")
        count += 1
        if count >= 10:
            break

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

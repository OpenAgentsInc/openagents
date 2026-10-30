#!/usr/bin/env python3
"""Which files in a repository go with an open GitHub issue?

An example client that chains our two local model servers, passing each
answer to the next step:

  1. gh reads the issue.
  2. gpt-oss-20b (/v1/responses) summarizes it and names search terms.
  3. git grep turns the terms into candidate files.
  4. Elephant (/v1/systemone) decides, for each candidate, whether it goes
     with the issue, with a probability.
  5. gpt-oss-20b turns the ranked files into a short report.

Tooling only: standard library, plus the gh and git commands. Usage:

  python3 psionic/examples/clients/issue_files.py 12608

The gpt-oss server's key (psionic/docs/serving/responses.md) comes from
PSIONIC_API_KEY, or, for a server on this machine run by this user, from that
server's own environment. It is never printed.
--no-gptoss skips both gpt-oss steps: the issue's own words become the search
terms and the report is the ranked list.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request

STOP = set("""a an and are as at be but by can do for from has have how if in into is it its
not of on or our so that the their then there these this to up was we what when where which
while who why will with without you your should would could make makes made use uses using
issue add adds added new also only more most than""".split())


def say(step, text):
    print(f"\n== {step}\n{text}", flush=True)


def local_key(base):
    """PSIONIC_API_KEY of the process listening on base's port, if base is this
    machine and the process is this user's (Linux: ss and /proc)."""
    match = re.match(r"https?://(127\.0\.0\.1|localhost|\[::1\]):(\d+)", base)
    if not match:
        return None
    out = subprocess.run(["ss", "-ltnpH", f"sport = :{match.group(2)}"], capture_output=True, text=True).stdout
    for pid in re.findall(r"pid=(\d+)", out):
        try:
            if os.stat(f"/proc/{pid}").st_uid != os.getuid():
                continue
            with open(f"/proc/{pid}/environ", "rb") as f:
                for entry in f.read().split(b"\0"):
                    if entry.startswith(b"PSIONIC_API_KEY="):
                        return entry.split(b"=", 1)[1].decode()
        except OSError:
            continue
    return None


def post(url, body, key=None, timeout=300):
    headers = {"Content-Type": "application/json"}
    if key:
        headers["Authorization"] = f"Bearer {key}"
    request = urllib.request.Request(url, json.dumps(body).encode(), headers)
    started = time.monotonic()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            data = json.load(response)
    except urllib.error.HTTPError as e:
        sys.exit(f"{url}: HTTP {e.code}: {e.read()[:300].decode(errors='replace')}")
    except urllib.error.URLError as e:
        sys.exit(f"{url}: {e.reason}")
    return data, time.monotonic() - started


def gptoss(base, key, prompt, max_tokens):
    data, seconds = post(f"{base}/v1/responses", {"model": "gpt-oss-20b", "input": prompt, "max_output_tokens": max_tokens, "reasoning": {"effort": "low"}}, key)
    text = "".join(part.get("text", "") for item in data.get("output", []) if item.get("type") == "message"
                   for part in item.get("content", []) if part.get("type") == "output_text")
    if not text:
        sys.exit(f"gpt-oss returned no text (status {data.get('status')}); try a larger --max-tokens")
    return text.strip(), seconds


def first_json(text):
    match = re.search(r"\{.*\}", text, re.S)
    if not match:
        return None
    try:
        return json.loads(match.group(0))
    except json.JSONDecodeError:
        return None


def words(text):
    seen = []
    for w in re.findall(r"[A-Za-z_][A-Za-z0-9_.-]{3,}", text):
        w = w.strip(".-").lower()
        if w not in STOP and w not in seen:
            seen.append(w)
    return seen


def candidates(repo, terms, top):
    """Files by how many distinct terms they contain; path matches count twice."""
    scores, lines = {}, {}
    files = subprocess.run(["git", "-C", repo, "ls-files"], capture_output=True, text=True, check=True).stdout.split("\n")
    for term in terms:
        out = subprocess.run(["git", "-C", repo, "grep", "-I", "-i", "-n", "-F", "-m", "1", "-e", term, "--"],
                             capture_output=True, text=True).stdout
        for line in out.splitlines():
            path, _, rest = line.partition(":")
            scores[path] = scores.get(path, 0) + 1
            lines.setdefault(path, []).append(rest.partition(":")[2].strip()[:120])
        for path in files:
            if term in path.lower():
                scores[path] = scores.get(path, 0) + 2
    ranked = sorted(scores, key=lambda p: (-scores[p], p))[:top]
    return [(p, scores[p], lines.get(p, [])[:2]) for p in ranked]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("issue", type=int)
    ap.add_argument("--repo", default=".", help="local checkout to search (default: .)")
    ap.add_argument("--gh-repo", help="owner/name for gh (default: the checkout's)")
    ap.add_argument("--elephant", default="http://127.0.0.1:8797")
    ap.add_argument("--gptoss", default="http://127.0.0.1:8080")
    ap.add_argument("--top", type=int, default=10, help="candidate files Elephant judges")
    ap.add_argument("--max-tokens", type=int, default=512, help="per gpt-oss call; the server allows at most 512")
    ap.add_argument("--no-gptoss", action="store_true")
    args = ap.parse_args()
    key = os.environ.get("PSIONIC_API_KEY") or (None if args.no_gptoss else local_key(args.gptoss))
    if not args.no_gptoss and not key:
        sys.exit(f"no key for {args.gptoss}: set PSIONIC_API_KEY, or pass --no-gptoss")

    # 1. The issue.
    view = ["gh", "issue", "view", str(args.issue), "--json", "number,title,body,url,state"]
    if args.gh_repo:
        view += ["--repo", args.gh_repo]
    issue = json.loads(subprocess.run(view, cwd=args.repo, capture_output=True, text=True, check=True).stdout)
    body = (issue.get("body") or "")[:6000]
    say("1. issue", f"#{issue['number']} [{issue['state']}] {issue['title']}\n{issue['url']}")

    # 2. gpt-oss: summary and search terms.
    if args.no_gptoss:
        summary, terms = issue["title"], words(issue["title"] + " " + body)[:12]
    else:
        prompt = ("Read this GitHub issue. Reply with only a JSON object: "
                  '{"summary": "<two sentences on what must change>", "terms": ["<8 to 12 code identifiers, file or '
                  'directory names, or distinctive words likely to appear in the files involved>"]}\n\n'
                  f"Title: {issue['title']}\n\n{body}")
        text, seconds = gptoss(args.gptoss, key, prompt, args.max_tokens)
        parsed = first_json(text) or {}
        summary = str(parsed.get("summary") or issue["title"])
        terms = [str(t).lower() for t in parsed.get("terms", []) if str(t).strip()][:12] or words(issue["title"] + " " + body)[:12]
        say(f"2. gpt-oss summary and terms ({seconds:.1f} s)", f"{summary}\nterms: {', '.join(terms)}")

    # 3. Candidate files.
    found = candidates(args.repo, terms, args.top)
    if not found:
        sys.exit("no file contains any of the terms")
    say("3. candidate files (git grep)", "\n".join(f"{s:3d}  {p}" for p, s, _ in found))

    # 4. Elephant: one choice question per file, in one request.
    state = f"GitHub issue: {issue['title']}\nWhat must change: {summary}"[:900]
    questions = {}
    for i, (path, _, snippets) in enumerate(found):
        seen = "; ".join(snippets)[:200]
        questions[f"f{i}"] = {"type": "choice", "criteria": {"yes": None, "no": None},
                              "instructions": f"Is the file {path} one that must be read or changed to resolve this issue? It contains: {seen}"}
    data, seconds = post(f"{args.elephant}/v1/systemone", {"model": "elephant-1", "state": state, "questions": questions})
    answers = data.get("answers", {})
    ranked = sorted(((answers.get(f"f{i}", {}).get("probabilities", {}).get("yes", 0.0), path, snippets)
                     for i, (path, _, snippets) in enumerate(found)), reverse=True)
    say(f"4. Elephant decisions ({seconds:.1f} s for {len(found)} files)",
        "\n".join(f"p(yes) {p:.3f}  {path}" for p, path, _ in ranked))

    # 5. gpt-oss: the report, from Elephant's ranking.
    listing = "\n".join(f"- {path} (p={p:.2f}): {'; '.join(s)[:160]}" for p, path, s in ranked)
    if args.no_gptoss:
        say("5. report", listing)
        return
    prompt = (f"GitHub issue #{issue['number']}: {issue['title']}\nSummary: {summary}\n\n"
              "A classifier rated how likely each file is to be involved (p near 1 means likely):\n"
              f"{listing}\n\nIn at most 8 lines, say which files to start with and why, and which ones look like "
              "false matches. Use only the files listed.")
    text, seconds = gptoss(args.gptoss, key, prompt, args.max_tokens)
    say(f"5. gpt-oss report ({seconds:.1f} s)", text)


if __name__ == "__main__":
    main()

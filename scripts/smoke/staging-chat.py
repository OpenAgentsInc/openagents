#!/usr/bin/env python3
"""A signed-in chat check on staging as its test account (#12235).

    scripts/smoke/staging-chat.py [--base URL] [--timeout SECONDS] [--delete ID...]

Staging keeps its own account store (`oa-accounts-nfs-staging`), so the
production test account's key (`~/work/.secrets/openagents-test-account.env`)
is refused there with 401. Staging's test account is the fixed agent-work
account `acct_dc7a799879686fc5`. Its API key is SMOKE_AGENT_KEY or, when
unset, the Secret Manager secret `openagents-web-1-staging-agent-smoke-key`
read with the automation account (`CLOUDSDK_CONFIG`, default
`~/work/.secrets/gcloud-sa-config`). The key is exchanged for an eight-hour
session (`POST /api/v1/sessions`), and nothing secret is printed.

The check makes a chat, waits for its answer, sends a second message, waits
again, deletes the chat, and confirms it is gone. Every request carries
`X-OpenAgents-Test: 1`. The chat is deleted even when a step fails. `--delete`
only deletes the named chats. Exit 0 when every step passed.
"""

import argparse
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

SECRET = "openagents-web-1-staging-agent-smoke-key"
PROJECT = "openagentsgemini"


def staging_key():
    key = os.environ.get("SMOKE_AGENT_KEY", "").strip()
    if key:
        return key
    config = os.environ.get("CLOUDSDK_CONFIG") or str(Path.home() / "work/.secrets/gcloud-sa-config")
    done = subprocess.run(
        ["gcloud", "secrets", "versions", "access", "latest", "--secret", SECRET, "--project", PROJECT],
        capture_output=True, text=True, env={**os.environ, "CLOUDSDK_CONFIG": config}, check=False)
    return done.stdout.strip()


class Site:
    def __init__(self, base):
        self.base = base.rstrip("/")

    def call(self, method, path, token, body=None):
        data = json.dumps(body).encode() if body is not None else None
        request = urllib.request.Request(self.base + path, data=data, method=method, headers={
            "Authorization": "Bearer " + token,
            "X-OpenAgents-Test": "1",
            "Content-Type": "application/json",
            "Accept": "application/json",
        })
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                text = response.read().decode()
                status = response.status
        except urllib.error.HTTPError as error:
            text = error.read().decode(errors="replace")
            status = error.code
        try:
            return status, json.loads(text) if text else {}
        except ValueError:
            return status, {}


def record(name, ok, detail=""):
    print(f"{'PASS' if ok else 'FAIL'} {name}" + (f" ({detail})" if detail else ""), flush=True)
    return ok


def wait_answer(site, session, chat, replies, deadline):
    """Wait until the chat is idle with more than `replies` assistant messages."""
    while time.monotonic() < deadline:
        status, body = site.call("GET", f"/v1/threads/{chat}", session)
        if status == 200:
            answers = [m for m in body.get("messages", []) if m.get("role") == "assistant"]
            if not body.get("thread", {}).get("working") and len(answers) > replies:
                return answers[-1].get("text", ""), body["thread"].get("outcome")
        time.sleep(3)
    return None, "timed out"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--base", default="https://staging.openagents.com")
    parser.add_argument("--timeout", type=int, default=240)
    parser.add_argument("--delete", nargs="+", metavar="ID")
    args = parser.parse_args()
    site = Site(args.base)
    key = staging_key()
    if not key:
        record("staging test account key", False, f"set SMOKE_AGENT_KEY or grant access to {SECRET}")
        return 1
    status, body = site.call("POST", "/api/v1/sessions", key)
    session = body.get("token") if status in (200, 201) else None
    if not record("the staging test account signs in", bool(session), str(status)):
        return 1
    status, body = site.call("GET", "/v1/account", session)
    record("the session reads its account", status == 200, str(status))
    if args.delete:
        ok = True
        for chat in args.delete:
            status, _ = site.call("DELETE", f"/v1/threads/{chat}", session)
            ok &= record(f"delete {chat}", status in (200, 204, 404), str(status))
        return 0 if ok else 1

    chat = str(uuid.uuid4())
    ok = True
    deadline = time.monotonic() + args.timeout
    try:
        status, body = site.call("POST", "/v1/threads", session,
                                 {"request_id": chat, "text": "Reply with the word OK."})
        ok &= record("a chat is made", status in (200, 201) and body.get("id") == chat, str(status))
        if ok:
            text, outcome = wait_answer(site, session, chat, 0, deadline)
            ok &= record("the chat answers", text is not None, str(outcome))
        if ok:
            status, body = site.call("POST", f"/v1/threads/{chat}/messages", session,
                                     {"request_id": str(uuid.uuid4()), "text": "Reply with the word YES."})
            ok &= record("a second message is sent", status in (200, 201, 202) and body.get("answering") is True,
                         str(status))
        if ok:
            text, outcome = wait_answer(site, session, chat, 1, deadline)
            ok &= record("the second message answers", text is not None, str(outcome))
    finally:
        status, _ = site.call("DELETE", f"/v1/threads/{chat}", session)
        ok &= record("the chat is deleted", status in (200, 204), str(status))
        status, _ = site.call("GET", f"/v1/threads/{chat}", session)
        ok &= record("the deleted chat is gone", status == 404, str(status))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Check public saved-history reads; optionally create and remove dedicated-account fixtures."""
import argparse
import json
import os
import subprocess
import urllib.error
import urllib.request
import uuid


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--api", default="https://api.openagents.com")
    parser.add_argument("--seed-dedicated-account", action="store_true")
    parser.add_argument("--client-manifest", help="Run a prebuilt independent Cargo client while the synthetic chats exist")
    parser.add_argument("--client-arg", action="append", default=[], help="Argument passed to the independent example")
    parser.add_argument("--native-command", nargs=argparse.REMAINDER, help="Isolated native fixture command; receives only synthetic chat IDs in SDK_FIXTURE_CHAT_IDS")
    args = parser.parse_args()
    assert not (args.client_manifest or args.native_command) or args.seed_dedicated_account, "The external-client check requires disposable fixtures"
    opener = urllib.request.build_opener(NoRedirect())
    token = os.environ.get("OPENAGENTS_TOKEN")

    def call(method, path, data=None, authenticated=True):
        headers = {"Content-Type": "application/json"}
        if authenticated and token:
            headers["Authorization"] = "Bearer " + token
        request = urllib.request.Request(args.api.rstrip("/") + path,
            data=json.dumps(data).encode() if data is not None else None,
            headers=headers, method=method)
        try:
            result = opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            result = error
        with result:
            raw = result.read(17 * 1024 * 1024 + 1)
            assert len(raw) <= 17 * 1024 * 1024
            return result.status, json.loads(raw) if raw else None

    status, spec = call("GET", "/v1/openapi.json", authenticated=False)
    assert status == 200
    for path in ("/v1/history", "/v1/history/projects", "/v1/history/{id}/messages",
                 "/v1/history/{id}/files/{file}", "/v1/history/{id}/captures/{capture}"):
        assert path in spec["paths"], "Missing history schema"
    assert call("GET", "/v1/history", authenticated=False)[0] == 401
    print("PASS deployed history schemas and signed-out refusal")
    if not token:
        assert not args.seed_dedicated_account, "Explicit dedicated-account credential required"
        return
    assert call("GET", "/v1/account")[0] == 200, "Test credential did not authenticate"
    sessions = []
    ids = []
    try:
        if args.seed_dedicated_account:
            for n in range(3):
                session = "sdk-history-" + uuid.uuid4().hex
                sessions.append(session)
                messages = [{"role": "user", "text": "Synthetic saved question"}]
                messages += [{"role": "assistant", "text": "Synthetic saved answer " + str(i)} for i in range(204)]
                messages[1] = {"role": "tool", "text": "Read fixture", "step": {
                    "name": "read", "summary": "Read fixture", "status": "done",
                    "input": "fixture.txt", "output": "Saved fixture output"}}
                messages[2] = {"role": "thinking", "text": "Synthetic saved thinking"}
                status, made = call("PUT", "/v1/threads/synced/" + session,
                    {"computer": "SDK history fixture", "title": "SDK history fixture", "messages": messages})
                assert status == 200, "Fixture upload failed"
                ids.append(made["chat"])
        seen = set()
        after = None
        while True:
            status, page = call("GET", "/v1/history?limit=1" + ("&after=" + after if after else ""))
            assert status == 200 and len(page["data"]) <= 1, "History page failed"
            for row in page["data"]:
                assert row["id"] not in seen and "owner" not in row and "session" not in row
                seen.add(row["id"])
            following = page["next"]
            if following is None:
                break
            assert following != after, "Cursor did not advance"
            after = following
        assert set(ids) <= seen, "Saved fixtures were not discoverable"
        status, projects = call("GET", "/v1/history/projects")
        assert status == 200 and isinstance(projects["data"], list)
        for chat in ids:
            status, first = call("GET", "/v1/history/" + chat + "/messages?limit=200")
            assert status == 200 and len(first["data"]) == 200
            assert first["data"][1]["tool"]["output"] == "Saved fixture output"
            assert first["data"][2]["role"] == "thinking"
            status, last = call("GET", "/v1/history/" + chat + "/messages?after=" + first["next"] + "&revision=" + str(first["revision"]))
            assert status == 200 and len(last["data"]) == 5 and last["next"] is None
            status, _ = call("GET", "/v1/history/" + chat + "/messages?after=msg_199&revision=999999")
            assert status == 409
        if args.client_manifest:
            subprocess.run(["cargo", "run", "--locked", "--manifest-path", args.client_manifest, "--", *args.client_arg], check=True)
        if args.native_command:
            env = dict(os.environ, SDK_FIXTURE_CHAT_IDS=json.dumps(ids))
            subprocess.run(args.native_command, check=True, env=env)
        print("PASS dedicated-account ID pages, project directory, saved tools/thinking, and complete 205-message reads")
    finally:
        failed_cleanup = 0
        for session in sessions:
            try:
                status, _ = call("DELETE", "/v1/threads/synced/" + session)
                failed_cleanup += status not in (200, 204, 404)
            except (OSError, AssertionError):
                failed_cleanup += 1
        assert not failed_cleanup, "Synthetic history cleanup needs retry"
        if sessions:
            print("Cleanup: all synthetic saved chats removed; no computers or tasks started")


if __name__ == "__main__":
    main()

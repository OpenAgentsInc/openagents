# Firm reason: 2 (independent HTTP oracle for the unchanged native auth/OTA services).
"""Serve a signed fixture package plus fake mail/auth; never logs credentials."""
import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse, parse_qs

p = argparse.ArgumentParser()
p.add_argument("directory", type=Path)
p.add_argument("--port", type=int, default=13491)
p.add_argument("--environment-transcript", action="store_true")
a = p.parse_args()
manifest = json.loads((a.directory / "manifest.json").read_text())
objects = {f["sha256"]: a.directory / "Starter" / f["path"] for f in manifest["files"]}
threads = {}
if a.environment_transcript:
    threads["m_00000000000000000001"] = dict(schema="openagents.chat.thread.v1", id="m_00000000000000000001", revision=1, model="google/gemini-4-hydrogen", messages=[dict(role="user", content="Account fixture"), dict(role="assistant", content="Saved account answer.")])
provider_accounts = {}
provider_flows = {}
provider_receipts = {}
provider_reads = {}
def provider_view(host, body):
    accounts = provider_accounts.setdefault(host, [dict(account_id="system",provider="claude-code",label="System Claude Code",email="ada@example.com",plan="max",state="connected",selected=True,windows=[dict(window_id="seven_day",used_basis_points=3500,resets_at="2026-11-20T12:00:00Z"),dict(window_id="five_hour",used_basis_points=625,resets_at="2026-11-14T18:00:00Z")],observed_at_ms=1794672000000,stale=False,error="")])
    method=body["method"]; command=body.get("command_id",""); login={"login_id":"","account_id":"","state":"","authorization_url":"","expires_at_ms":0}
    if command and (host,command) in provider_receipts:
        return provider_receipts[host,command]
    if method=="BeginAccountLogin":
        login=dict(login_id=command,account_id=body.get("account_id") or command,state="awaiting_code",authorization_url="https://claude.com/cai/oauth/authorize?fixture=not-a-real-login",expires_at_ms=1999999999999)
        provider_flows[host,command]=login
    if method in ("ReadAccountLogin","CompleteAccountLogin","CancelAccountLogin"):
        login=provider_flows[host,body["login_id"]]
        if method=="CompleteAccountLogin":
            if body.get("code")!="fixture-code":raise ValueError("Unexpected fixture code")
            login["state"]="completed";login["authorization_url"]=""
            added=dict(accounts[0],account_id=login["account_id"],label="Work",email="work@example.com",selected=False)
            accounts.append(added)
        if method=="CancelAccountLogin":login["state"]="cancelled";login["authorization_url"]=""
    if method=="SelectAccount":
        for account in accounts:account["selected"]=account["account_id"]==body["account_id"]
    shown=accounts
    if method=="ListAccounts":
        provider_reads[host]=provider_reads.get(host,0)+1
        if provider_reads[host]<2:shown=[dict(x,state="checking",windows=[],observed_at_ms=0) for x in accounts]
    value=dict(schema="openagents.environment.accounts.view.v1",host_pubkey=host,method=method,accounts=shown,login=login,command_id=command,error="",retry_after_ms=0)
    if command:provider_receipts[host,command]=json.loads(json.dumps(value))
    return value

counts = dict(sent=0, wrong=0, verified=0, authorized=0, logout=0, device_bound=0)

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def answer(self, status, value):
        body = value if isinstance(value, bytes) else json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if a.environment_transcript and self.path.startswith("/api/chat/environment"):
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            host = "a" * 64
            if urlparse(self.path).path == "/api/chat/environments/v1":
                return self.answer(200, dict(schema="openagents.environment.catalog.v1alpha1", environments=[dict(host_pubkey=host, display_name="Mac"), dict(host_pubkey="b" * 64, display_name="CoderOS")]))
            query = parse_qs(urlparse(self.path).query)
            host = query.get("host", [host])[0]
            name = "CoderOS" if host == "b" * 64 else "Mac"
            if urlparse(self.path).path=="/api/chat/environment/accounts/v1":
                return self.answer(200,provider_view(host,dict(method="ListAccounts")))
            method = query["method"][0]
            summary = dict(session_id="fixture-session", workspace_id="default", title="CoderOS task" if name == "CoderOS" else "Transcript fixture", origin="managed", control="available", generation=1, revision=1, busy=False, interaction_id="", updated_at_ms=1)
            items = [] if method == "ListSessions" else [
                dict(item_id="user-1", role="user", text="Fix the transcript", tool_name="", complete=True),
                dict(item_id="tool-1", role="tool", text="## Tool output is literal\n" + "42  let code = **not Markdown**;\n" * 120, tool_name="Read", complete=True),
                dict(item_id="tool-2", role="tool", text="Updated file successfully.", tool_name="Edit", complete=True),
                dict(item_id="assistant-1", role="assistant", text="**Done.** The transcript is compact.", tool_name="", complete=True)]
            return self.answer(200, dict(schema="openagents.environment.runtime.v1alpha1", host_pubkey=host, display_name=name, method=method, sessions=[summary], items=items, error="", complete=False, next_cursor="1000001", command_id="", command_status="", source_epoch="fixture", models=[dict(model_id="default", display_name="Claude default", efforts=["low", "high"])], efforts=[dict(effort_id="low", display_name="Low"), dict(effort_id="high", display_name="High")], default_effort="high"))
        if self.path == "/latest.cnu":
            return self.answer(200, (a.directory / "latest.cnu").read_bytes())
        if self.path.startswith("/objects/"):
            obj = objects.get(self.path.removeprefix("/objects/"))
            return self.answer(200, obj.read_bytes()) if obj else self.answer(404, {})
        if self.path.startswith("/api/chat/thread/v1?id="):
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            record = threads.get(self.path.split("=", 1)[1])
            return self.answer(200, record) if record else self.answer(404, {})
        if self.path == "/api/chat/history/v1":
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {"error": "Log in first."})
            counts["authorized"] += 1
            return self.answer(200, {"schema": "openagents.chat.history.v1", "chats": [dict(id=k, title=v["messages"][0]["content"][:80], model="google/gemini-4-hydrogen", folder="", updated_ms=3, busy=False) for k,v in threads.items()] + [
                dict(id="42", title="My first saved chat", model="m", folder="openagents", updated_ms=2, busy=False),
                dict(id="41", title="Another saved chat", model="m", folder="openagents", updated_ms=1, busy=False)]})
        if self.path == "/report":
            return self.answer(200, counts)
        return self.answer(404, {})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if urlparse(self.path).path=="/api/chat/environment/accounts/v1":
            if self.headers.get("Authorization")!="Bearer oa_at_fixture":return self.answer(401,{})
            return self.answer(200,provider_view(parse_qs(urlparse(self.path).query)["host"][0],body))
        if self.path in ("/api/chat/thread/v1", "/api/chat/complete/v1"):
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            if self.path.endswith("thread/v1"):
                key = body.get("id") or f"m_{len(threads)+1:020x}"
                current = threads.get(key, {}).get("revision", 0)
                if body["revision"] != current:
                    return self.answer(409, {})
                body.update(id=key, revision=current+1)
                threads[key] = body
                return self.answer(200, body)
            return self.answer(200, {"choices":[{"message":{"role":"assistant","content":"Hello from the account agent.\n\n## Shared Markdown\n\n**Bold**, *italic*, ~~strike~~ and `inline code`. [Docs](https://openagents.com).\n\n- [x] done\n  - nested item\n\n| Left | Right |\n| :--- | ---: |\n| one | two |\n\n> A quote\n\n```coax\nx := 1;\n```"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":6}})
        if self.path == "/api/auth/email/start":
            if body != dict(email="ada@example.com", client="app"):
                return self.answer(400, {"error": "Unexpected email request"})
            counts["sent"] += 1
            return self.answer(200, {"pending": "fixture-challenge", "state": "sent"})
        if self.path == "/api/auth/email/verify":
            if body.get("pending") != "fixture-challenge":
                return self.answer(409, {"error": "Ask for a new code."})
            if body.get("code") != "415926":
                counts["wrong"] += 1
                return self.answer(401, {"error": "That code isn't right. 4 tries left."})
            if not self.headers.get("OA-Device-Key"):
                return self.answer(400, {"error": "Device key missing"})
            counts["verified"] += 1
            counts["device_bound"] += 1
            return self.answer(200, dict(access_token="oa_at_fixture", refresh_token="oa_rt_fixture", account="acct_fixture", expires_in=3600, device_bound=True))
        if self.path == "/api/auth/refresh":
            if body.get("refresh_token")!="oa_rt_fixture":return self.answer(401,{})
            return self.answer(200,dict(access_token="oa_at_fixture",refresh_token="oa_rt_fixture",account="acct_fixture",expires_in=3600,device_bound=True))
        if self.path == "/api/auth/logout":
            provider_accounts.clear();provider_flows.clear();provider_receipts.clear();provider_reads.clear()
            counts["logout"] += 1
            return self.answer(200, {"state": "logged-out"})
        return self.answer(404, {})

ThreadingHTTPServer(("127.0.0.1", a.port), Handler).serve_forever()

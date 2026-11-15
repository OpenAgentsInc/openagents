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
p.add_argument("--projects", action="store_true")
a = p.parse_args()
manifest = json.loads((a.directory / "manifest.json").read_text())
objects = {f["sha256"]: a.directory / "Starter" / f["path"] for f in manifest["files"]}
project_published=False
project_asks=[]
project_links={}
project_draft={"status":"ready","draft":{"card":{"name":"Moon Villa","slug":"moon-villa","line":"A place to grow","owner":"Ada","art_direction":"Soft light"},"mechanics":["Walk"],"design":{"markdown":"A complete design document.","open_questions":["What next?"]},"roadmap":{"initiative":{"slug":"first","title":"First steps","body":"Start small"},"milestones":[{"slug":"garden","title":"Garden","body":"Plant things","tasks":[{"slug":"walk","title":"Walk","body":"Walk around","size":"S","acceptance":["Can walk"]}]}]},"engine_needs":[],"assets":[],"to_make":[]},"edits":0,"approved":False}
project_calls=[]
threads = {}
routes = []
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

account_settings=dict(schema=json.loads((Path(__file__).resolve().parents[3] / "proto/verse/account-settings-v1.json").read_text())["schema"],revision=0,profile=dict(display_name="Ada",name="ada",nip05="",about="",picture="",banner="",website=""),relays=["wss://relay.openagents.com"],privacy=dict(remote_images=True,private_clipboard=True),notifications=dict(security=True))
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
        if self.path in ("/api/auth/settings/v1","/api/auth/identity/v1","/api/auth/account","/api/auth/ways","/api/auth/sessions"):
            if self.headers.get("Authorization")!="Bearer oa_at_fixture":return self.answer(401,{"error":"Log in first"})
            values={"/api/auth/settings/v1":account_settings,"/api/auth/identity/v1":dict(pubkey="79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",source="account"),"/api/auth/account":dict(account="acct_fixture",primary="ada@example.com"),"/api/auth/ways":dict(level="L1",keys=[],passkeys=[]),"/api/auth/sessions":dict(sessions=[dict(id="fixture-session",name="iPhone",this=True)])}
            return self.answer(200,values[self.path])
        if a.projects and (self.path.startswith("/projects/") or self.path.startswith("/api/drops") or self.path.startswith("/api/chat/project")):
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":return self.answer(401,{})
            if self.path=="/projects/mobile-kit/api/state":return self.answer(200,b"Y|Ada|member\nP|Mobile Kit|Native project fixture|1|now|Ada\nG|g1|proposed||Build the garden\nD|2026-11-14|Today\nM|2026-11-14|m1|todo|Ada|Plant flowers|\n")
            if self.path=="/projects/mobile-kit/api/talk":
                rows=["Z|0|10|0|1","G|g1|Ada"]
                for i,prompt in enumerate(project_asks,1):rows += [f"U|{i}|ask|done|now|Ada|{prompt}|**Done.** Your project is updated.||abc1234|1|1||||",f"E|{i}|{i}|agent.edit|Updated the milestone."]
                return self.answer(200,"\n".join(rows).encode()+b"\n")
            if self.path=="/projects/moon-villa/api/state":return self.answer(404,{"error":"not found"})
            if self.path=="/projects/moon-villa/index.md":return self.answer(200,("# "+project_draft["draft"]["card"]["name"]+"\n\nA native project. Its tasks are on Nostr: relay, kind 30621 by "+"a"*64+", d moon-villa.\n\n## Design\n\nA place to grow.").encode())
            if self.path=="/api/drops/me":return self.answer(200,dict(may=True,pubkey="a"*64,limits=dict(file=67108864,files=40)))
            if self.path=="/api/drops":return self.answer(200,dict(drops=[dict(id="d_fixture",status="open",files=1,project=None)]))
            if self.path=="/api/drops/d_fixture":return self.answer(200,dict(id="d_fixture",status="open",files=[dict(id="f_fixture",name="notes.md",status="scanned",bytes=120,kind="text",note="")]))
            if self.path.endswith("/draft"):return self.answer(200,project_draft)
            if self.path.startswith("/api/chat/project/v1?"):
                chats=[dict(id=k,title=threads[k]["messages"][0]["content"][:80],model="google/gemini-4-hydrogen",folder="moon-villa",updated_ms=1,busy=False) for k,v in project_links.items() if v=="moon-villa"]
                return self.answer(200,dict(schema="openagents.chat.history.v1",chats=chats))
            return self.answer(404,{"error":"not found"})
        if self.path == "/api/projects/v1":return self.answer(200,dict(schema="openagents.projects.api.v1alpha1",data=[],revision=None,next_cursor=None))
        if self.path == "/api/projects":
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":return self.answer(401,{})
            return self.answer(200,b"project mobile-kit Mobile Kit\n"+(b"project moon-villa Moon Villa\n" if project_published else b""))
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
            return self.answer(200, dict(counts,project_calls=project_calls))
        return self.answer(404, {})

    def do_POST(self):
        global project_published,account_settings
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if self.path=="/api/auth/settings/v1":
            if self.headers.get("Authorization")!="Bearer oa_at_fixture":return self.answer(401,{})
            if body["revision"]!=account_settings["revision"]:return self.answer(409,{"error":"Settings changed. Reload before saving."})
            account_settings=body;account_settings["revision"]+=1;return self.answer(200,account_settings)
        if self.path=="/api/auth/identity/backup/v1":return self.answer(401,{"error":"Verify with a passkey or login key first."})
        if self.path.startswith("/api/auth/sessions/") or self.path=="/api/auth/devices/push/off":return self.answer(200,{"ok":True})
        if a.projects and (self.path.startswith("/projects/") or self.path.startswith("/api/drops") or self.path.startswith("/api/chat/project")):
            if self.headers.get("Authorization")!="Bearer oa_at_fixture":return self.answer(401,{})
            project_calls.append(self.path)
            if self.path=="/projects/mobile-kit/api/ask":project_asks.append(body["text"]);return self.answer(200,dict(turn=len(project_asks)))
            if self.path.startswith("/projects/mobile-kit/api/"):return self.answer(200,dict(ok=True))
            if self.path.endswith("/draft/edit"):
                target=project_draft["draft"];path=body["path"]
                for part in path[:-1]:target=target[int(part)] if isinstance(target,list) else target[part]
                if isinstance(target,list):target[int(path[-1])]=body["value"]
                else:target[path[-1]]=body["value"]
                project_draft["edits"]+=1;return self.answer(200,project_draft)
            if self.path.endswith("/publish"):project_published=True;return self.answer(200,dict(published=dict(page="/projects/moon-villa")))
            if self.path=="/api/chat/project/v1":
                assert body["schema"]=="openagents.chat.project-link.v1" and body["thread"] in threads
                project_links[body["thread"]]=body["project"];return self.answer(200,dict(linked=True))
            if self.path=="/api/chat/project/complete/v1":
                assert project_links[body["thread"]["id"]]==body["project"]
                return self.answer(200,dict(choices=[dict(message=dict(role="assistant",content="The project chat stays in the app."),finish_reason="stop")]))
            return self.answer(200,project_draft)
        if urlparse(self.path).path=="/api/chat/environment/accounts/v1":
            if self.headers.get("Authorization")!="Bearer oa_at_fixture":return self.answer(401,{})
            return self.answer(200,provider_view(parse_qs(urlparse(self.path).query)["host"][0],body))
        # Where a message would run (docs/router/README.md): a fixed decision for the Mac, and the
        # completion that was told it. The request must be a RouteRequest naming the fixture's computers.
        if self.path == "/api/chat/route/v1":
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            assert body["schema"] == "openagents.router.request.v1alpha1" and body["text"]
            assert [s["station_id"] for s in body["stations"]] == ["mac", "coderos"]
            assert all(s["agents"] == [dict(engine="claude", installed=True, logged_in=True)] for s in body["stations"])
            routes.append(body["text"])
            return self.answer(200, dict(schema="openagents.router.decision.v1alpha1", ok=True, station="mac", engine="claude", model="opus", effort="high",
                difficulty=3, weight=1, door="gemini-decision-1", reasons=["named_station"], eligible_stations=["mac", "coderos"], eligible_engines=["claude"], input_tokens=900, latency_ms=300))
        if self.path == "/api/chat/delegated/complete/v1":
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            assert body["schema"] == "openagents.chat.delegated.v1" and body["thread"]["schema"] == "openagents.chat.thread.v1"
            assert body["note"].startswith("Would run: Claude Code · Opus · high · on Mac\nWhy: You asked for this computer.\n"), body["note"]
            return self.answer(200, dict(choices=[dict(message=dict(role="assistant", content="I'm about to hand this to Claude Code on Mac."), finish_reason="stop")]))
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

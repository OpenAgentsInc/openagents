# Firm reason: 2 (independent HTTP oracle for the unchanged native auth/OTA services).
"""Serve fake account HTTP and an optional signed package; never logs credentials."""
import argparse
import json
import copy
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse, parse_qs

p = argparse.ArgumentParser()
p.add_argument("directory", type=Path)
p.add_argument("--port", type=int, default=13491)
p.add_argument("--environment-transcript", action="store_true")
p.add_argument("--projects", action="store_true")
p.add_argument("--api-only",action="store_true")
p.add_argument("--feed",action="store_true",help="Isolated browser account oracle with two real read-only relays")
a = p.parse_args()
manifest = None if a.api_only else json.loads((a.directory / "manifest.json").read_text())
objects = {} if manifest is None else {f["sha256"]: a.directory / "Starter" / f["path"] for f in manifest["files"]}
project_published=False
project_asks=[]
project_links={}
private_projects={}
private_knowledge={}
private_links={}
PROJECT_SCHEMA="openagents.projects.api.v1alpha1"
def private_reply(value):return dict(schema=PROJECT_SCHEMA,data=value)
def private_row(project):
    return dict(project,counts=dict(knowledge=len(private_knowledge[project["id"]]),conversations=sum(p==project["id"] for p in private_links.values())+sum(c["project_id"]==project["id"] for c in rich_conversations.values())))

# Independent HTTP/SDK oracle: these timers own work even while the app is
# backgrounded or terminated. Reads never advance a turn or start another one.
rich_lock=threading.RLock()
rich_conversations={}
rich_turns={}
rich_operations={}
ACCOUNT_FIXTURE_ANSWER="Hello from the account agent.\n\n## Shared Markdown\n\n**Bold**, *italic*, ~~strike~~ and `inline code`. [Docs](https://openagents.com).\n\n- [x] done\n  - nested item\n\n| Left | Right |\n| :--- | ---: |\n| one | two |\n\n> A quote\n\n```coax\nx := 1;\n```"
def fixture_account(header):
    return {"Bearer oa_at_fixture":"acct_fixture","Bearer oa_at_fixture_other":"acct_other"}.get(header)
def fixture_tokens(account):
    other=account=="acct_other"
    return dict(access_token="oa_at_fixture_other" if other else "oa_at_fixture",refresh_token="oa_rt_fixture_other" if other else "oa_rt_fixture",account=account,expires_in=3600,device_bound=True)
def rich_usage():
    return dict(input_tokens=None,output_tokens=None,cache_read_tokens=None,cache_write_tokens=None,reasoning_tokens=None)
def rich_control(t):
    c=rich_conversations[t["conversation_id"]]
    return dict(schema="openagents.chat.control.v2alpha1",conversation_id=c["id"],turn_id=t["id"],conversation_revision=c["revision"],state=t["state"],stop_requested=t["stop_requested"],event_cursor=len(t["events"]))
def rich_event(t,kind,delta=None):
    t["updated"]=max(t["updated"],int(time.time()*1000))
    rich_conversations[t["conversation_id"]]["updated"]=t["updated"]
    t["events"].append(dict(schema="openagents.chat.event.v2alpha1",conversation_id=t["conversation_id"],turn_id=t["id"],sequence=len(t["events"])+1,type=kind,state=t["state"],stop_requested=t["stop_requested"],block_index=0 if delta is not None else None,delta=delta,usage=t["usage"].copy() if kind=="usage" else None,stop_reason=t["stop_reason"] if kind in ("completed","cancelled") else None,error=None))
def rich_tick(tid,phase):
    with rich_lock:
        t=rich_turns[tid]
        if t["state"] in ("completed","cancelled"):return
        if phase=="cancel":
            t.update(state="cancelled",stop_reason="cancelled");rich_event(t,"cancelled");t["done"].set();return
        if phase=="start":
            t["dispatches"]+=1;t["state"]="running";rich_event(t,"started")
            t["text"]=t["partial"];rich_event(t,"text_delta",t["partial"])
        else:
            delta=t["answer"][len(t["text"]):];t["text"]=t["answer"]
            if delta:rich_event(t,"text_delta",delta)
            t["usage"].update(input_tokens=12,output_tokens=7);rich_event(t,"usage")
            t.update(state="completed",stop_reason="end_turn");rich_event(t,"completed");t["done"].set()
def rich_schedule(tid,phase,delay):
    timer=threading.Timer(delay,rich_tick,args=(tid,phase));timer.daemon=True;timer.start()
def rich_turn(t):
    c=rich_conversations[t["conversation_id"]]
    return dict(schema="openagents.chat.turn.v2alpha1",id=t["id"],conversation_id=c["id"],operation_id=t["operation"],state=t["state"],stop_requested=t["stop_requested"],stop_reason=t["stop_reason"],usage=t["usage"],selection=dict(model_id="fixture-model",provider_id="fixture-provider",effort="low",route_decision_id=None,reason="Independent HTTP lifecycle oracle"),content=[dict(type="text",text=t["text"])] if t["text"] else [],event_cursor=len(t["events"]),conversation_revision=c["revision"],context_snapshot_id="ctx_"+t["id"][4:] if c["project_id"] else None,error=None,created_at_ms=t["created"],updated_at_ms=t["updated"])
def rich_messages(c):
    rows=[]
    for tid in c["turns"]:
        t=rich_turns[tid]
        rows += [dict(id=tid+".user",turn_id=tid,role="user",content=t["content"],created_at_ms=t["created"]),dict(id=tid+".assistant",turn_id=tid,role="assistant",content=[dict(type="text",text=t["text"])] if t["text"] else [],created_at_ms=t["updated"])]
    return rows
project_draft={"status":"ready","draft":{"card":{"name":"Moon Villa","slug":"moon-villa","line":"A place to grow","owner":"Ada","art_direction":"Soft light"},"mechanics":["Walk"],"design":{"markdown":"A complete design document.","open_questions":["What next?"]},"roadmap":{"initiative":{"slug":"first","title":"First steps","body":"Start small"},"milestones":[{"slug":"garden","title":"Garden","body":"Plant things","tasks":[{"slug":"walk","title":"Walk","body":"Walk around","size":"S","acceptance":["Can walk"]}]}]},"engine_needs":[],"assets":[],"to_make":[]},"edits":0,"approved":False}
project_calls=[]
threads = {}
routes = []
stations = []
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
        route=urlparse(self.path);query=parse_qs(route.query)
        if a.feed and route.path in ("/api/auth/account","/api/auth/settings/v1"):
            if route.path=="/api/auth/account":return self.answer(200,dict(account="feed_oracle",primary="fixture@example.test"))
            settings=copy.deepcopy(account_settings)
            settings["relays"]=["wss://relay.primal.net","wss://nos.lol"]
            settings["privacy"]["remote_images"]=False
            return self.answer(200,settings)
        if route.path=="/report/durable/wait":
            with rich_lock:
                found=[t for t in rich_turns.values() if t["prompt"]==query.get("question",[""])[0]]
                t=found[-1] if found else None
            if not t:return self.answer(404,{})
            t["done"].wait(15)
            with rich_lock:return self.answer(200,dict(state=t["state"],dispatches=t["dispatches"],stop_requests=t["stop_requests"]))
        if route.path in ("/api/chat/conversations/v2alpha1","/api/chat/conversation/v2alpha1","/api/chat/turn/v2alpha1","/api/chat/events/v2alpha1"):
            account=fixture_account(self.headers.get("Authorization"))
            if not account:return self.answer(401,{})
            with rich_lock:
                if route.path=="/api/chat/conversations/v2alpha1":
                    rows=[];project=query.get("project_id",[""])[0];cursor=query.get("cursor",[""])[0]
                    for c in sorted(rich_conversations.values(),key=lambda c:c["id"]):
                        if c["account"]!=account or c["id"]<=cursor or (project and c["project_id"]!=project):continue
                        active=next((i for i in c["turns"] if rich_turns[i]["state"] in ("queued","running")),None)
                        rows.append(dict(id=c["id"],title=c["title"],revision=c["revision"],project_id=c["project_id"],active_turn_id=active,updated_at_ms=c["updated"]))
                    return self.answer(200,dict(schema="openagents.chat.index.v2alpha1",items=rows[:60],next_cursor=rows[59]["id"] if len(rows)>60 else None))
                identity=query.get("id",[""])[0];after=int(query.get("after",["0"])[0])
                if route.path=="/api/chat/conversation/v2alpha1":
                    c=rich_conversations.get(identity)
                    if not c or c["account"]!=account:return self.answer(404,{})
                    rows=rich_messages(c);active=next((i for i in c["turns"] if rich_turns[i]["state"] in ("queued","running")),None)
                    return self.answer(200,dict(schema="openagents.chat.conversation.v2alpha1",id=c["id"],revision=c["revision"],project_id=c["project_id"],legacy_thread_id=None,legacy_thread_revision=None,messages=rows[after:after+32],active_turn_id=active,created_at_ms=c["created"],updated_at_ms=c["updated"],next_cursor=str(after+32) if len(rows)>after+32 else None))
                t=rich_turns.get(identity)
                if not t or t["account"]!=account:return self.answer(404,{})
                if route.path=="/api/chat/turn/v2alpha1":return self.answer(200,rich_turn(t))
                if after>len(t["events"]):return self.answer(409,{})
                rows=t["events"][after:after+32]
                return self.answer(200,dict(schema="openagents.chat.events.v2alpha1",conversation_id=t["conversation_id"],turn_id=t["id"],after=after,events=rows,next_cursor=after+len(rows),has_more=len(t["events"])>after+len(rows),state=t["state"]))
        if route.path=="/api/chat/environments/v1" and not a.environment_transcript:
            return self.answer(200,dict(schema="openagents.environment.catalog.v1alpha1",environments=[]))
        if self.path in ("/api/auth/settings/v1","/api/auth/identity/v1","/api/auth/account","/api/auth/ways","/api/auth/sessions"):
            account=fixture_account(self.headers.get("Authorization"))
            if not account:return self.answer(401,{"error":"Log in first"})
            settings=copy.deepcopy(account_settings)
            if account=="acct_other":settings["profile"].update(display_name="Grace",name="grace")
            values={"/api/auth/settings/v1":settings,"/api/auth/identity/v1":dict(pubkey="79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",source="account"),"/api/auth/account":dict(account=account,primary="grace@example.com" if account=="acct_other" else "ada@example.com"),"/api/auth/ways":dict(level="L1",keys=[],passkeys=[]),"/api/auth/sessions":dict(sessions=[dict(id="fixture-session",name="iPhone",this=True)])}
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
        if self.path.startswith("/api/projects/v1"):
            account=fixture_account(self.headers.get("Authorization"))
            if not account:return self.answer(401,{})
            parts=urlparse(self.path).path.split("/")[4:]
            if not parts:return self.answer(200,dict(private_reply([private_row(p) for p in private_projects.values() if p["owner_account_id"]==account]),revision=None,next_cursor=None))
            pid=parts[0];project=private_projects.get(pid)
            if not project or project["owner_account_id"]!=account:return self.answer(404,{})
            if len(parts)==1:return self.answer(200,private_reply(private_row(project)))
            if parts[1:]==["instructions"]:return self.answer(200,private_reply(project["instructions"]))
            if parts[1:]==["knowledge"]:return self.answer(200,dict(private_reply(list(private_knowledge[pid].values())),revision=project["revision"]))
            if len(parts)==4 and parts[1]=="knowledge" and parts[3]=="content":return self.answer(200,private_reply(private_knowledge[pid][parts[2]]))
            if parts[1:]==["chats"]:
                chats=[dict(id=t,title=threads[t]["messages"][0]["content"][:80],model="google/gemini-4-hydrogen",folder=pid,updated_ms=1,busy=False) for t,p in private_links.items() if p==pid]
                return self.answer(200,dict(schema="openagents.chat.history.v1",chats=chats))
            return self.answer(404,{})
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
            # a computer's own report (coder/tui/host-station.coax): the Mac answers; CoderOS runs an
            # older service and does not, so the route falls back to what the app knows of it
            if urlparse(self.path).path=="/api/chat/environment/station/v1":
                stations.append(name)
                if name=="CoderOS":return self.answer(503,{"error":"That computer did not report."})
                return self.answer(200,dict(schema="openagents.router.station.v1alpha1",host_pubkey=host,station=dict(station_id="adas-macbook",display_name="Adas-MacBook",
                    reported_at_ms=1700000000000,os="macos",cpu_cores=12,memory_total_mb=36864,memory_free_mb=12000,disk_free_mb=200000,disk_floor_mb=20480,
                    load_1m_milli=2100,load_15m_milli=1800,apple_builds=True,agents=[dict(engine="claude",installed=True,logged_in=True),dict(engine="codex",installed=True)],repos=["openagents"])))
            method = query["method"][0]
            summary = dict(session_id="fixture-session", workspace_id="default", title="CoderOS task" if name == "CoderOS" else "Transcript fixture", origin="managed", control="available", generation=1, revision=1, busy=False, interaction_id="", updated_at_ms=1)
            items = [] if method == "ListSessions" else [
                dict(item_id="user-1", role="user", text="Fix the transcript", tool_name="", complete=True),
                dict(item_id="tool-1", role="tool", text="## Tool output is literal\n" + "42  let code = **not Markdown**;\n" * 120, tool_name="Read", complete=True),
                dict(item_id="tool-2", role="tool", text="Updated file successfully.", tool_name="Edit", complete=True),
                dict(item_id="assistant-1", role="assistant", text="**Done.** The transcript is compact.", tool_name="", complete=True)]
            return self.answer(200, dict(schema="openagents.environment.runtime.v1alpha1", host_pubkey=host, display_name=name, method=method, sessions=[summary], items=items, error="", complete=False, next_cursor="1000001", command_id="", command_status="", source_epoch="fixture", models=[dict(model_id="default", display_name="Claude default", efforts=["low", "high"])], efforts=[dict(effort_id="low", display_name="Low"), dict(effort_id="high", display_name="High")], default_effort="high"))
        if self.path == "/latest.cnu":
            if a.api_only:return self.answer(404,{})
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
            if self.headers.get("Authorization")=="Bearer oa_at_fixture_other":return self.answer(200,dict(schema="openagents.chat.history.v1",chats=[]))
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
        if self.path in ("/api/chat/submit/v2alpha1","/api/chat/stop/v2alpha1"):
            account=fixture_account(self.headers.get("Authorization"))
            if not account:return self.answer(401,{})
            with rich_lock:
                operation=(account,body["operation_id"]);canonical=json.dumps(body,sort_keys=True)
                if operation in rich_operations:
                    path,prior,tid=rich_operations[operation]
                    if path!=self.path or prior!=canonical:return self.answer(409,{})
                    return self.answer(202 if "submit" in self.path else 200,rich_control(rich_turns[tid]))
                if "stop" in self.path:
                    assert body["schema"]=="openagents.chat.stop.v2alpha1"
                    t=rich_turns.get(body["turn_id"])
                    if not t or t["account"]!=account:return self.answer(404,{})
                    t["stop_requests"]+=1;t["stop_requested"]=True
                    if t["state"]=="queued":rich_tick(t["id"],"cancel")
                    elif t["state"]=="running":rich_event(t,"stop_requested");rich_schedule(t["id"],"cancel",0.15)
                    rich_operations[operation]=(self.path,canonical,t["id"])
                    return self.answer(200,rich_control(t))
                assert body["schema"]=="openagents.chat.submit.v2alpha1"
                assert body["legacy_thread_id"] is None and body["legacy_thread_revision"] is None
                project=body["project_id"]
                if project:
                    record=private_projects.get(project)
                    if not record or record["owner_account_id"]!=account:return self.answer(404,{})
                    assert record["revision"]==body["expected_project_revision"]
                    assert "Cite" in record["instructions"]["text"] and any("violet" in k["text"] for k in private_knowledge[project].values())
                cid=body["conversation_id"]
                if cid:
                    c=rich_conversations.get(cid)
                    if not c or c["account"]!=account:return self.answer(404,{})
                    if c["revision"]!=body["expected_revision"] or any(rich_turns[i]["state"] in ("queued","running") for i in c["turns"]):return self.answer(409,{})
                    assert c["project_id"]==project
                else:
                    assert body["expected_revision"]==0
                    cid=f"cnv_{len(rich_conversations)+1:032x}";now=int(time.time()*1000)
                    c=dict(id=cid,account=account,project_id=project,revision=0,title=body["content"][0]["text"][:160],turns=[],created=now,updated=now);rich_conversations[cid]=c
                tid=f"trn_{len(rich_turns)+1:032x}";now=int(time.time()*1000);prompt="\n".join(b["text"] for b in body["content"])
                partial="The code is" if project else "Hello from";answer="The code is violet." if project else ACCOUNT_FIXTURE_ANSWER;delay=2.0
                if prompt.startswith("Background durable reply"):
                    partial="A partial answer.";answer=partial+"\n\nSaved after background.";delay=8.0
                elif prompt.startswith("Stop this answer"):
                    partial="Stop fixture partial.";answer=partial+"\n\nThis text must never appear after Stop.";delay=30.0
                t=dict(id=tid,account=account,conversation_id=cid,operation=body["operation_id"],prompt=prompt,content=copy.deepcopy(body["content"]),state="queued",stop_requested=False,stop_reason=None,events=[],usage=rich_usage(),text="",partial=partial,answer=answer,created=now,updated=now,done=threading.Event(),dispatches=0,stop_requests=0)
                rich_turns[tid]=t;c["turns"].append(tid);c["revision"]+=1;c["updated"]=now;rich_operations[operation]=(self.path,canonical,tid)
                rich_schedule(tid,"start",0.3);rich_schedule(tid,"complete",delay)
                return self.answer(202,rich_control(t))
        if self.path.startswith("/api/projects/v1"):
            account=fixture_account(self.headers.get("Authorization"))
            if not account:return self.answer(401,{})
            assert body.get("operation_id") and not set(body)-{"operation_id","expected_revision","value"}
            parts=self.path.split("/")[4:];value=body.get("value",{})
            if not parts:
                pid=f"prj_{len(private_projects)+1:032x}"
                project=dict(id=pid,revision=1,owner_account_id=account,name=value["name"],description=value.get("description",""),instructions=dict(text=value.get("instructions",""),revision=1,updated_at_ms=1),created_at_ms=1,updated_at_ms=1)
                private_projects[pid]=project;private_knowledge[pid]={}
                return self.answer(201,private_reply(private_row(project)))
            pid=parts[0];project=private_projects[pid]
            if project["owner_account_id"]!=account:return self.answer(404,{})
            assert body["expected_revision"]==project["revision"]
            if parts[1:]==["complete"]:
                tid=value["thread_id"];record=threads[tid]
                assert private_links[tid]==pid and record["revision"]==value["thread_revision"]
                assert "Cite" in project["instructions"]["text"] and any("violet" in k["text"] for k in private_knowledge[pid].values())
                record["messages"].append(dict(role="assistant",content="The code is violet."));record["revision"]+=1
                return self.answer(200,dict(choices=[dict(message=dict(role="assistant",content="The code is violet."))]))
            project["revision"]+=1
            if parts[1:]==["instructions"]:
                project["instructions"]=dict(text=value["text"],revision=project["revision"],updated_at_ms=1)
                return self.answer(200,private_reply(project["instructions"]))
            if parts[1:]==["knowledge"]:
                kid=f"knw_{len(private_knowledge[pid])+1:032x}";source=value["source"];assert source["kind"]=="text"
                knowledge=dict(id=kid,title=value["title"],text=source["text"],selected=value.get("selected",True),revision=project["revision"])
                private_knowledge[pid][kid]=knowledge;return self.answer(201,private_reply(knowledge))
            if parts[1:]==["chats"]:
                assert value["thread_id"] in threads;private_links[value["thread_id"]]=pid
                return self.answer(200,private_reply(private_row(project)))
            return self.answer(404,{})
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
            mac, coderos = body["stations"]
            # the Mac's facts are its own report, named as you connected it; CoderOS did not report
            assert mac["display_name"] == "Mac" and mac["os"] == "macos" and mac["disk_free_mb"] == 200000 and mac["apple_builds"] and mac["reported_at_ms"] == 1000, mac
            assert mac["agents"] == [dict(engine="claude", installed=True, logged_in=True), dict(engine="codex", installed=True)] and mac["repos"] == ["openagents"], mac
            assert coderos["agents"] == [dict(engine="claude", installed=True, logged_in=True)] and "os" not in coderos, coderos
            assert stations[-2:] == ["Mac", "CoderOS"], stations
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
            return self.answer(200, {"choices":[{"message":{"role":"assistant","content":ACCOUNT_FIXTURE_ANSWER},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":6}})
        if self.path == "/api/auth/email/start":
            if body not in (dict(email="ada@example.com",client="app"),dict(email="grace@example.com",client="app")):
                return self.answer(400, {"error": "Unexpected email request"})
            counts["sent"] += 1
            return self.answer(200, {"pending": "fixture-other-challenge" if body["email"]=="grace@example.com" else "fixture-challenge", "state": "sent"})
        if self.path == "/api/auth/email/verify":
            if body.get("pending") not in ("fixture-challenge","fixture-other-challenge"):
                return self.answer(409, {"error": "Ask for a new code."})
            if body.get("code") != "415926":
                counts["wrong"] += 1
                return self.answer(401, {"error": "That code isn't right. 4 tries left."})
            if not self.headers.get("OA-Device-Key"):
                return self.answer(400, {"error": "Device key missing"})
            counts["verified"] += 1
            counts["device_bound"] += 1
            return self.answer(200,fixture_tokens("acct_other" if body["pending"]=="fixture-other-challenge" else "acct_fixture"))
        if self.path == "/api/auth/refresh":
            if body.get("refresh_token") not in ("oa_rt_fixture","oa_rt_fixture_other"):return self.answer(401,{})
            return self.answer(200,fixture_tokens("acct_other" if body["refresh_token"]=="oa_rt_fixture_other" else "acct_fixture"))
        if self.path == "/api/auth/logout":
            provider_accounts.clear();provider_flows.clear();provider_receipts.clear();provider_reads.clear()
            counts["logout"] += 1
            return self.answer(200, {"state": "logged-out"})
        return self.answer(404, {})

ThreadingHTTPServer(("127.0.0.1", a.port), Handler).serve_forever()

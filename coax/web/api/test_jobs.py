# Scheduled jobs (jobs.c, svc-schedules): only the scheduler's Google ID token
# (checked by a fake tokeninfo that test.sh points the service at) or an
# admin runs a job; a job runs once per Chicago day; 7:00 Chicago is 7:00 on
# both sides of the November 1 daylight-saving change. test.py runs this file.
import http.server
import threading
import urllib.parse

tokens = {}   # token -> claims the fake tokeninfo answers


class FakeTokeninfo(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        t = urllib.parse.parse_qs(urllib.parse.urlparse(self.path).query).get("id_token", [""])[0]
        status, out = (200, tokens[t]) if t in tokens else (400, {"error": "invalid_token"})
        data = json.dumps(out).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *a):
        pass


jobs_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["TOKENINFO_PORT"])), FakeTokeninfo)
threading.Thread(target=jobs_fake.serve_forever, daemon=True).start()

import time as _time
good = {"iss": "https://accounts.google.com", "aud": API + "/api/jobs", "email": "jobs@example.iam.gserviceaccount.com",
        "email_verified": "true", "exp": str(int(_time.time()) + 600)}
tokens["good.token"] = good
tokens["other.aud"] = dict(good, aud="https://elsewhere.example/api/jobs")
tokens["other.caller"] = dict(good, email="someone@example.iam.gserviceaccount.com")
tokens["expired.token"] = dict(good, exp=str(int(_time.time()) - 5))


def job(path, token=None, admin=False):
    headers = {}
    if token:
        headers["Authorization"] = "Bearer " + token
    if admin:
        headers["Authorization"] = "Nostr " + subprocess.run([os.environ["SIGN"], "--key", os.environ["ADMIN_KEY"], "POST", API + path],
                                                             capture_output=True, text=True, check=True).stdout.strip()
    s, b, _ = call("POST", path, b"", headers)
    if "ops_seen" in globals():
        ops_seen.append({"kind": "job-run" if b.startswith(b'{"job"') else "error", "route": "POST " + path.split("?")[0], "status": s, "body": b.decode()})
    return s, (json.loads(b) if b.startswith(b"{") else b)


check("jobs: an unauthenticated call is refused", job("/api/jobs/heartbeat")[0] == 401)
check("jobs: a token Google refuses is refused", job("/api/jobs/heartbeat", "not.a.token")[0] == 401)
check("jobs: a token for another audience is refused", job("/api/jobs/heartbeat", "other.aud")[0] == 401)
check("jobs: another service account is refused", job("/api/jobs/heartbeat", "other.caller")[0] == 401)
check("jobs: an expired token is refused", job("/api/jobs/heartbeat", "expired.token")[0] == 401)
check("jobs: a GET is a 405", call("GET", "/api/jobs/heartbeat")[0] == 405)
today = sql("SELECT (now() AT TIME ZONE 'America/Chicago')::date")
s, b = job("/api/jobs/heartbeat", "good.token")
check("jobs: the scheduler runs today's (Chicago) job", s == 200 and b.get("state") == "ran" and b.get("day") == today, (s, b))
s, b = job("/api/jobs/heartbeat", "good.token")
check("jobs: the same day again is already ran", s == 200 and b.get("state") == "already ran", (s, b))
check("jobs: the scheduler cannot pick a day", job("/api/jobs/heartbeat?day=2026-10-01", "good.token")[0] == 403)
check("jobs: no such job is a 404", job("/api/jobs/nothing", "good.token")[0] == 404)
s, b = job("/api/jobs/heartbeat?day=2026-10-30", admin=True)
check("jobs: an admin runs a missed day by hand", s == 200 and b.get("state") == "ran" and b.get("day") == "2026-10-30", (s, b))
s, b = job("/api/jobs/heartbeat?day=2026-10-30&again=1", admin=True)
check("jobs: and runs it again on purpose", s == 200 and b.get("state") == "ran", (s, b))
check("jobs: a bad day is a 400", job("/api/jobs/heartbeat?day=2026-13-40", admin=True)[0] == 400)
check("jobs: the runs are counted", sql("SELECT runs FROM jobs.runs WHERE job = 'heartbeat' AND day = '2026-10-30'") == "2")
# Dry run of the schedule (schedules.conf: heartbeat at 0 7 * * * in America/Chicago) across 2026-11-01.
utc = sql("SELECT string_agg(to_char(((d::date + time '07:00') AT TIME ZONE 'America/Chicago') AT TIME ZONE 'UTC', 'MM-DD HH24:MI'), ' ' ORDER BY d::date) "
          "FROM generate_series(date '2026-10-31', date '2026-11-02', interval '1 day') d")
check("jobs: 7:00 Chicago is 12:00 UTC before the change and 13:00 after", utc == "10-31 12:00 11-01 13:00 11-02 13:00", utc)
local = sql("SELECT string_agg(to_char(t AT TIME ZONE 'America/Chicago', 'MM-DD HH24:MI'), ' ' ORDER BY t) FROM unnest(ARRAY["
            "timestamptz '2026-10-31 12:00Z', '2026-11-01 13:00Z', '2026-11-02 13:00Z']) t")
check("jobs: and those runs are each 7:00 in Chicago", local == "10-31 07:00 11-01 07:00 11-02 07:00", local)

# The service watch (service.c, service-policy.coax, docs/deployment/service-levels.md, #13297): it runs
# on every call (no claim on the day), opens an alert for what holds, resolves what stopped, and mails
# each change to OA_ALERT_TO (test.sh: owner@example.com, through test_email.py's fake Resend).
check("service-watch: an unauthenticated call is refused", job("/api/jobs/service-watch")[0] == 401)
now_ms = int(sql("SELECT verse.room_now()"))
sql("DELETE FROM verse.room_shards WHERE shard = 97; DELETE FROM verse.room_stats WHERE shard = 97")
sql("INSERT INTO verse.room_shards (shard, rooms_cap, at, seen_at, endpoint) VALUES (97, 0, %d, %d, 'wss://room.openagents.com:4497/api/room/s97/')"
    % (now_ms - 60000, now_ms - 60000))


def stats_report(gen, p95, failures, fenced, unsaved):
    body = ("t %d\nprocess generation=%d\nshard=97\nplayers=2\ntick p95 ms=%.2f\ntick p99 ms=%.2f\ncheckpoint failures=%d\n"
            "checkpoint stale owner refusals=%d\ncheckpoint unsaved age ms most=%d\n") % (now_ms, gen, p95, p95, failures, fenced, unsaved)
    sql("INSERT INTO verse.room_stats (shard, body) VALUES (97, $b$%s$b$)" % body)


stats_report(4, 12.5, 2, 0, 1000)    # the first report of a process counts nothing yet
stats_report(4, 12.5, 3, 3, 31000)   # one more failure, three fenced, unsaved past the loss bound
s, b = job("/api/jobs/service-watch", "good.token")
opened = sorted(a["key"] for a in b.get("result", {}).get("sync", {}).get("opened", [])) if s == 200 else []
check("service-watch: a shard silent for a minute, its slow ticks, its failed, fenced and unsaved checkpoints open alerts",
      s == 200 and b.get("state") == "ran" and all(k in opened for k in ["silent:97", "tick:97", "checkpoint-failures:97", "fenced:97", "unsaved:97"]), (s, b))
s, b = job("/api/jobs/service-watch", "good.token")
check("service-watch: runs again at once, and an alert already open opens nothing", s == 200 and b["result"]["sync"]["opened"] == [], (s, b))
if "mail_seen" in globals():
    mail = [m["body"] for m in mail_seen if "shard 97: stopped reporting" in m["body"].get("subject", "")]
    check("service-watch: the opened alert is mailed to the owner with its runbook line",
          len(mail) == 1 and mail[0].get("to") == ["owner@example.com"] and mail[0]["subject"].startswith("[production] ") and "vm-deploy.sh status" in mail[0].get("text", ""), mail)
s, b, _ = call("GET", "/api/playtest/service-health")
h = json.loads(b) if s == 200 else {}
# Every body service.c sends is service-wire.coax's (Protocol E11, #13250): the route's real health body,
# the job's real result and a refusal read through the codec's golden check (service-wire-goldens.coax
# --read), which holds their keys to proto/openagents/ops/service/v1alpha1/service.proto and writes each
# back through the codec for the same bytes.
import tempfile
SV_REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))


def service_wire(kind, raw):
    f = tempfile.NamedTemporaryFile(suffix=".json", delete=False)
    f.write(raw)
    f.close()
    r = subprocess.run([os.path.join(SV_REPO, "coax", "web", "tool.sh"), "coax/web/api/service-wire-goldens.coax", "--read", kind, f.name],
                       cwd=SV_REPO, capture_output=True, text=True, timeout=1200)
    os.unlink(f.name)
    return r.returncode == 0, (raw[-600:], r.stdout[-800:], r.stderr[-800:])


ok, why = service_wire("health", b)
check("service-health: the body, with shard 97's alerts open and its minute counted, is the codec's bytes and the proto's keys", s == 200 and ok, why)
ws, wb, _ = call("POST", "/api/jobs/service-watch", b"", {"Authorization": "Bearer good.token"})
wpre = b'{"job":"service-watch","state":"ran","result":'
ok, why = service_wire("watch", wb[len(wpre):-1]) if ws == 200 and wb.startswith(wpre) else (False, wb)
check("service-watch: the job's result is the codec's bytes and the proto's keys", ok, why)
es, eb, _ = call("POST", "/api/playtest/service-health", b"")
ok, why = service_wire("error", eb)
check("service-health: a refusal is the codec's ErrorReply", es == 405 and eb == b'{"error":"GET the service health"}' and ok, why)
check("service-health: the three service levels and the open alerts, for anyone",
      s == 200 and [l["name"] for l in h.get("levels", [])] == ["tick time", "join time", "checkpoint age"]
      and h["levels"][0]["target"] == 10000 and h["levels"][1]["objective_ppm"] == 950000
      and any(a["key"] == "silent:97" and a["title"] == "shard 97: stopped reporting for 45 s" for a in h.get("alerts", [])), b)
check("service-health: shard 97's minute counts against tick time and checkpoint age",
      h.get("levels") and h["levels"][0]["total"] >= 1 and h["levels"][0]["worst"] >= 12500 and h["levels"][2]["worst"] >= 31000, h.get("levels"))
now_ms = int(sql("SELECT verse.room_now()"))
sql("UPDATE verse.room_shards SET at = %d, seen_at = %d WHERE shard = 97; DELETE FROM verse.room_stats WHERE shard = 97" % (now_ms, now_ms))
s, b = job("/api/jobs/service-watch", "good.token")
resolved = sorted(a["key"] for a in b.get("result", {}).get("sync", {}).get("resolved", [])) if s == 200 else []
check("service-watch: reporting again, its alerts resolve", s == 200 and all(k in resolved for k in ["silent:97", "tick:97", "unsaved:97"]), (s, b))
sql("UPDATE verse.room_shards SET draining = 5, gen = 5, at = %d, seen_at = %d WHERE shard = 97" % (now_ms - 60000, now_ms - 60000))
s, b = job("/api/jobs/service-watch", "good.token")
check("service-watch: a drained shard's silence is no alert", s == 200 and "silent:97" not in [a["key"] for a in b["result"]["sync"]["opened"]], (s, b))
sql("DELETE FROM verse.room_shards WHERE shard = 97; DELETE FROM verse.room_minutes WHERE shard = 97")
job("/api/jobs/service-watch", "good.token")

# Crash reports (#13298, docs/deployment/crash-reports.md): a shard posts its report signed as its
# `crash` write, the watch folds a page's crash event into one, each lands in a bucket by its
# signature, and the watch raises the alerts (service-policy.coax): one a room bucket, one for the pages.
import hashlib as _hashlib, hmac as _hmac

_master = bytes.fromhex(os.environ.get("OA_ROOM_WRITE_MASTER", "03" * 32))
_kid = _hmac.new(_master, b"oa-room-write-kid-v1", _hashlib.sha256).hexdigest()[:8]


def crash_post(shard, body, purpose="crash", signer=None):
    key = _hmac.new(_master, ("oa-room-shard-key-v1\n%d" % shard).encode(), _hashlib.sha256).digest()
    mac = _hmac.new(key, ("oa-room-write-v1\n%s\n%d\n%s\n" % (purpose, shard, _kid)).encode() + body.encode(), _hashlib.sha256).hexdigest()
    s, b, _ = call("POST", "/api/playtest/room-crash", body.encode(), {"X-Room-Mac": mac, "X-Room-Signer": "%d %s" % (signer or shard, _kid), "Content-Type": "text/plain"})
    return s, (json.loads(b) if b.startswith(b"{") else b)


sql("DELETE FROM verse.crash_buckets")
room_crash = ("crash 1\nt %d\nsource room\nshard 97\nrelease test-release\nbuild 5f0c2a9e\nkind SIGABRT\nat 0\n"
              "trap coax/web/room/room.coax:412: index out of bounds\nf rm_crash_test coax/web/room/room-stats.coax:160\nf main +0x1c0de\n"
              "tick %d 420 2\nsig SIGABRT|rm_crash_test|main\n") % (now_ms, now_ms - 16)
s, b = crash_post(97, room_crash)
room_bucket = _hashlib.md5(b"room SIGABRT|rm_crash_test|main").hexdigest()
check("room-crash: a shard's signed report is kept in the bucket of its signature", s == 200 and b == {"kept": True, "bucket": room_bucket}, (s, b))
check("room-crash: the same report again is kept once", crash_post(97, room_crash)[0] == 200
      and sql("SELECT reports FROM verse.crash_buckets WHERE bucket = '%s'" % room_bucket) == "1")
check("room-crash: signed as another purpose it is refused", crash_post(97, room_crash, purpose="stats")[0] == 401)
check("room-crash: another shard's report is refused", crash_post(96, room_crash)[0] == 403)
check("room-crash: a page's report from a shard is refused", crash_post(97, "crash 1\nt 1\nsource page\npage p\nkind trap\nsig trap|?\n")[0] == 400)
check("room-crash: a body that is no report is refused", crash_post(97, "shard=97\n")[0] == 400)
check("room-crash: unsigned it is refused", call("POST", "/api/playtest/room-crash", room_crash.encode())[0] == 401)
s, b = job("/api/jobs/service-watch", "good.token")
opened = {a["key"]: a for a in b.get("result", {}).get("sync", {}).get("opened", [])} if s == 200 else {}
check("service-watch: a room's crash bucket opens its alert, with the signature, the release and the runbook line",
      opened.get("crash:" + room_bucket[:8], {}).get("title") == "shard 97: a room crashed: SIGABRT|rm_crash_test|main (release test-release)"
      and "/api/playtest/crashes" in opened["crash:" + room_bucket[:8]].get("todo", "") and "crash:page" not in opened, (s, b))
# a page's trap: host.js sends the trap's line and the frames it resolved from the module's symbols
ev = lambda sid, extra: {"sid": sid, "events": [{"ev": "crash", "page": "everglade2", "ct": 1, "p": dict({
    "msg": "THE WORLD STOPPED: unreachable", "rel": "everglade2.9c2aa0c8e19b.wasm", "trap": "apps/openagents-web/everglade2.coax:88: index out of bounds",
    "f0": "abort +0x1f3a2", "f1": "coax_trap +0x1f40c", "f2": "page_crash_test +0x2b7e1", "f3": "frame"}, **extra)}]}
call("POST", "/api/analytics/events", json.dumps(dict(ev("ccrashauto0001", {}), auto="webdriver")).encode(), {"Content-Type": "text/plain"})
call("POST", "/api/analytics/events", json.dumps({"sid": "ccrashplain001", "events": [{"ev": "crash", "page": "everglade2", "ct": 1, "p": {"msg": "RENDERER STOPPED: lost", "trap": None}}]}).encode(), {"Content-Type": "text/plain"})
s, b = job("/api/jobs/service-watch", "good.token")
page_bucket = _hashlib.md5(b"page trap|page_crash_test|frame").hexdigest()
check("service-watch: a page's crash event with frames becomes a report in its bucket; one with none does not",
      sql("SELECT source || ' ' || sig || ' ' || reports FROM verse.crash_buckets WHERE source = 'page'") == "page trap|page_crash_test|frame 1", sql("SELECT sig FROM verse.crash_buckets"))
check("service-watch: an automated browser's page crash opens no alert", s == 200 and "crash:page" not in [a["key"] for a in b["result"]["sync"]["opened"]], (s, b))
call("POST", "/api/analytics/events", json.dumps(ev("ccrashhuman001", {"msg": "THE WORLD STOPPED: unreachable (again)"})).encode(),
     {"Content-Type": "text/plain", "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"})
s, b = job("/api/jobs/service-watch", "good.token")
opened = {a["key"]: a for a in b.get("result", {}).get("sync", {}).get("opened", [])} if s == 200 else {}
check("service-watch: a person's page crash opens the pages' one alert",
      opened.get("crash:page", {}).get("title") == "a page crashed: trap|page_crash_test|frame (1 bucket in the last hour)", (s, b))
s, b, _ = call("GET", "/api/playtest/crashes")
got = {k["bucket"]: k for k in json.loads(b).get("buckets", [])} if s == 200 else {}
check("crashes: the buckets, each with its newest report, for anyone",
      got.get(room_bucket, {}).get("newest") == room_crash and got[room_bucket]["shard"] == 97 and got[room_bucket]["build"] == "5f0c2a9e"
      and got.get(page_bucket, {}).get("reports") == 2 and "f page_crash_test +0x2b7e1\nf frame\n" in got[page_bucket]["newest"]
      and got[page_bucket]["newest"].startswith("crash 2\n"), b)   # v2: each frame with its code offset (#13379)
# a native app's crash (#13379): the dump the app wrote from MetricKit's diagnostic, with no login; the
# codec makes crash report v3 from it, and the apps share one alert as the pages do
app_dump = ("appdump 1\nt %d\napp ios\nos iPhone OS 18.1 (22B83)\nversion 1.4\nbuildno 212\nexe OpenAgents\nsignal 11\nexception 1\n"
            "reason Namespace SIGNAL, Code 11 Segmentation fault: 11\na 5F0C2A9E-7B1D-4C3A-8E6F-001122334455 6948 libswiftCore.dylib\n"
            "a 70B89F27-1634-3580-A695-57CDB41D7743 165304 OpenAgents\na 70B89F27-1634-3580-A695-57CDB41D7743 4096 OpenAgents\n") % now_ms
app_post = lambda body: call("POST", "/api/playtest/app-crash", body, {"Content-Type": "text/plain"})
app_bucket = _hashlib.md5(b"app SIGSEGV|OpenAgents+0x285b8|OpenAgents+0x1000").hexdigest()
s, b, _ = app_post(app_dump.encode())
check("app-crash: an app's dump, with no login, is kept as its report in the bucket of its signature", s == 200 and json.loads(b) == {"kept": True, "bucket": app_bucket}, (s, b))
check("app-crash: the same dump again is kept once", app_post(app_dump.encode())[0] == 200
      and sql("SELECT reports FROM verse.crash_buckets WHERE bucket = '%s'" % app_bucket) == "1")
check("app-crash: a body that is no dump is refused, and so is a report itself", app_post(b"crash 3\nt 1\nsource app\napp ios\nbuild 0\nkind K\nsig K|?\n")[0] == 400)
check("app-crash: more than 16 KB is refused", app_post(b"appdump 1\nt 1\napp ios\n" + b"a 0 1 x\n" * 2100)[0] == 413)
check("app-crash: only a POST", call("GET", "/api/playtest/app-crash")[0] == 405)
s, b = job("/api/jobs/service-watch", "good.token")
opened = {a["key"]: a for a in b.get("result", {}).get("sync", {}).get("opened", [])} if s == 200 else {}
check("service-watch: an app's crash opens the apps' one alert",
      opened.get("crash:app", {}).get("title") == "an app crashed: SIGSEGV|OpenAgents+0x285b8|OpenAgents+0x1000 (1 bucket in the last hour)", (s, b))
s, b, _ = call("GET", "/api/playtest/crashes")
got = {k["bucket"]: k for k in json.loads(b).get("buckets", [])} if s == 200 else {}
check("crashes: an app's bucket with its report: v3, the app binary's UUID as its build, each frame a binary and its offset",
      got.get(app_bucket, {}).get("source") == "app" and got[app_bucket]["build"] == "70b89f2716343580a69557cdb41d7743" and got[app_bucket]["release"] == "1.4 (212)"
      and got[app_bucket]["newest"].startswith("crash 3\n") and "f libswiftCore.dylib +0x1b24\nf OpenAgents +0x285b8\n" in got[app_bucket]["newest"], b)
check("app-crash: one address posts 6 a minute", 429 in [app_post(b"junk %d\n" % i)[0] for i in range(14)])
sql("UPDATE verse.crash_buckets SET last_ms = last_ms - 3600000, live_ms = live_ms - 3600000")
s, b = job("/api/jobs/service-watch", "good.token")
resolved = sorted(a["key"] for a in b.get("result", {}).get("sync", {}).get("resolved", [])) if s == 200 else []
check("service-watch: an hour after the newest report the crash alerts resolve", resolved == sorted(["crash:" + room_bucket[:8], "crash:page", "crash:app"]), (s, b))
sql("DELETE FROM verse.crash_buckets")

# Capacity records (#13296, docs/deployment/soak.md): the soak posts a night's record signed as a
# room's `capacity` write; the API checks it through its codec (soak-record.coax), keeps one a day
# and environment, and answers the newest as it was kept.


def capacity_post(body, purpose="capacity", shard=1):
    key = _hmac.new(_master, ("oa-room-shard-key-v1\n%d" % shard).encode(), _hashlib.sha256).digest()
    mac = _hmac.new(key, ("oa-room-write-v1\n%s\n%d\n%s\n" % (purpose, shard, _kid)).encode() + body.encode(), _hashlib.sha256).hexdigest()
    s, b, _ = call("POST", "/api/playtest/capacity-record", body.encode(), {"X-Room-Mac": mac, "X-Room-Signer": "%d %s" % (shard, _kid), "Content-Type": "text/plain"})
    return s, (json.loads(b) if b.startswith(b"{") else b)


def capacity_record(day, t, seated, verdict="holds", env="staging"):
    return ("capacity 1\nday %s\nt %d\nenv %s\ncommit 1afd85d5a2\nseconds 7200\nplayers 96\nseated %d\ntick_p50_us 310\ntick_p95_us 900\n"
            "tick_p99_us 2400\nslow_windows 0\nrefused_ds 2\ncheckpoint_ms 5100\nticket_p95_ms 41\napi_p95_ms 38\nack_p95_ms 47\nmemory_early_kb 734000\n"
            "memory_late_kb 735200\nbytes_per_player_s 15300\nrejoined 598\nkills 340\nverdict %s\n") % (day, t, env, seated, verdict)


sql("DELETE FROM verse.capacity_records")
check("capacity: with no record kept, 404", call("GET", "/api/playtest/capacity")[0] == 404)
night = capacity_record("2026-11-12", 1762990000000, 92)
s, b = capacity_post(night)
check("capacity-record: a night's signed record is kept", s == 200 and b == {"kept": True}, (s, b))
s, b, _ = call("GET", "/api/playtest/capacity")
check("capacity: the newest record, as it was kept, for anyone", s == 200 and b.decode() == night, (s, b))
later = capacity_record("2026-11-12", 1762990000000 + 3600000, 96)
check("capacity-record: a later run of the same day replaces it, an earlier one does not",
      capacity_post(later) == (200, {"kept": True}) and capacity_post(night) == (200, {"kept": False})
      and sql("SELECT seated || ' ' || holds FROM verse.capacity_records WHERE day = '2026-11-12' AND env = 'staging'") == "96 true")
bad_night = capacity_record("2026-11-13", 1762990000000 + 86400000, 40, "fails slow-tick,leak")
check("capacity-record: a failed night is kept as failed, and is the newest", capacity_post(bad_night) == (200, {"kept": True})
      and call("GET", "/api/playtest/capacity")[1].decode() == bad_night
      and sql("SELECT holds::text FROM verse.capacity_records WHERE day = '2026-11-13'") == "false")
check("capacity: one day's record by its date", call("GET", "/api/playtest/capacity?day=2026-11-12")[1].decode() == later
      and call("GET", "/api/playtest/capacity?day=2026-11-01")[0] == 404)
check("capacity-record: a local room's run is kept but never the one shown",
      capacity_post(capacity_record("2026-11-14", 1762990000000 + 2 * 86400000, 16, env="local")) == (200, {"kept": True})
      and call("GET", "/api/playtest/capacity")[1].decode() == bad_night)
check("capacity-record: signed as another purpose it is refused", capacity_post(night, purpose="stats")[0] == 401)
check("capacity-record: unsigned it is refused", call("POST", "/api/playtest/capacity-record", night.encode())[0] == 401)
check("capacity-record: a body that is no record is refused", capacity_post("shard=1\n")[0] == 400
      and capacity_post(night.replace("verdict holds", "verdict ok"))[0] == 400 and capacity_post(night + "more 1\n")[0] == 400)
check("capacity: a POST is refused", call("POST", "/api/playtest/capacity", b"")[0] == 405)
sql("DELETE FROM verse.capacity_records")

# Every body email.c and jobs.c answered above is its codec's (Protocol E11, #13250): each one's keys
# are its message's (proto/openagents/ops/email/v1alpha1/email.proto, .../jobs/v1alpha1/jobs.proto, and
# the API's ErrorReply) and the codec reads it and writes it back for the same bytes
# (ops-wire-goldens.coax --read-all). OPS_CAPTURE=FILE keeps what the routes answered.
if "ops_seen" in globals():
    # The auth-sweep job runs and sweeps (it answered 500 on every database until #13250: its two
    # deletes were WITH items inside sub-selects, which Postgres refuses). A spent NIP-98 event past
    # its time is one of the rows it takes.
    sql("INSERT INTO auth.nip98_seen (event_id, expires_at) VALUES (repeat('a', 64), now() - interval '1 hour')")
    s, b = job("/api/jobs/auth-sweep", "good.token")
    check("jobs: auth-sweep runs, and a spent signature past its time is gone", s == 200 and b.get("state") == "ran"
          and b.get("result", {}).get("signatures", 0) >= 1 and sql("SELECT count(*) FROM auth.nip98_seen WHERE event_id = repeat('a', 64)") == "0",
          (s, b, [l for l in open(os.environ["API_LOG"]) if "auth-sweep" in l][-3:]))
    s, b = job("/api/jobs/auth-sweep", "good.token")
    check("jobs: the same day again is already ran, with the day's result", s == 200 and b.get("state") == "already ran" and b.get("result", {}).get("signatures", 0) >= 1, (s, b))
    if os.environ.get("OPS_CAPTURE"):
        json.dump(ops_seen, open(os.environ["OPS_CAPTURE"], "w"), indent=1)
    if os.path.exists(os.path.join(SV_REPO, "coax", "web", "api", "ops-wire-goldens.coax")):
        f = tempfile.NamedTemporaryFile(suffix=".json", delete=False, mode="w")
        json.dump(ops_seen, f)
        f.close()
        r = subprocess.run([os.path.join(SV_REPO, "coax", "web", "tool.sh"), "coax/web/api/ops-wire-goldens.coax", "--read-all", f.name],
                           cwd=SV_REPO, capture_output=True, text=True, timeout=1200)
        os.unlink(f.name)
        check("email and jobs: every body the routes answered (%d) is its codec's bytes and its proto's keys" % len(ops_seen),
              r.returncode == 0 and len(ops_seen) >= 20, (r.stdout[-1500:], r.stderr[-500:]))

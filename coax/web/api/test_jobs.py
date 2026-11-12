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

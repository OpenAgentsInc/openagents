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

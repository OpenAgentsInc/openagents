# The rooms' vitals (#12791; migrations/0123_room_stats.sql): a shard's signed report every 10 s, and
# the admins' room view that reads them (GET /api/admin/rooms). test.py runs this file.
if os.environ.get("ROOM_KEY"):
    import hmac as _hm, hashlib as _hl, time as _tm, urllib.parse as _up
    _rk = os.environ["ROOM_KEY"].encode()

    def rs_post(body, mac=None):
        m = mac or _hm.new(_rk, body.encode(), _hl.sha256).hexdigest()
        return call("POST", "/api/playtest/room-stats", body.encode(), {"X-Room-Mac": m, "Content-Type": "text/plain"})

    def rs_body(at, tick, seat="seat|everglade-1|3|512|20|41|36|52|7"):
        return f"t {at}\nshard=2\nplayers=16\ntick p95 ms={tick}\nraiders p95 ms=0.40\nqueue age ms most=20\nout D s players bytes a second=9000\n{seat}\n"

    now = int(_tm.time() * 1000)
    status, raw, _ = rs_post(rs_body(now, "1.20"))
    check("room-stats: a shard's signed report is kept", status == 200 and sql("select count(*) from verse.room_stats where shard = 2") == "1", raw)
    status, raw, _ = rs_post(rs_body(now, "1.20"), mac="0" * 64)
    check("room-stats: an unsigned one is refused", status == 401, raw)
    status, raw, _ = rs_post(rs_body(now - 600000, "1.20"))
    check("room-stats: a replayed one (its t ten minutes old) is refused", status == 400, raw)
    status, raw, _ = rs_post(rs_body(now + 1, "1.80"))
    check("room-stats: the next is kept beside it", status == 200 and sql("select count(*) from verse.room_stats where shard = 2") == "2", raw)

    status, body, _ = call("GET", "/api/admin/rooms")
    check("room view: signed out, a 401 and no numbers", status == 401 and b"tick" not in body, body)
    class _NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a, **k):
            return None
    urllib.request.install_opener(urllib.request.build_opener(_NoRedirect))
    _auth = _up.quote(subprocess.run([os.environ["SIGN"], "--key", os.environ["ADMIN_KEY"], "GET", API + "/api/admin/login"], capture_output=True, text=True, check=True).stdout.strip(), safe="")
    _, _, _h = call("GET", "/api/admin/login?auth=" + _auth)
    _cookie = _h.get("Set-Cookie", "").split(";")[0]
    status, body, _ = call("GET", "/api/admin/rooms", headers={"Cookie": _cookie})
    text = body.decode()
    check("room view: each number now and its series, oldest first, in its group",
          status == 200 and "#SHARD 2 TICK|k|NUMBER|NOW|LAST 10 MINUTES\ntick p95 ms|1.80|1.20,1.80\nraiders p95 ms|0.40|0.40,0.40\n" in text, text)
    check("room view: the queues and the bytes in their own groups",
          "#SHARD 2 PLAYERS|k|" in text and "\nplayers|16|16,16\n" in text and "\nqueue age ms most|20|20,20\n" in text
          and "#SHARD 2 BYTES|k|" in text and "\nout D s players bytes a second|9000|9000,9000\n" in text, text)
    check("room view: the seats from the last report",
          "#SHARD 2 SEATS|w|ROOM|SEAT|QUEUE BYTES|QUEUE MS|RELEVANT|ACK P50 MS|ACK P95 MS|ROUND TRIP MS\neverglade-1|3|512|20|41|36|52|7\n" in text, text)
    sql("delete from verse.room_stats where shard = 2")
    status, body, _ = call("GET", "/api/admin/rooms", headers={"Cookie": _cookie})
    check("room view: no shard reporting is one empty section", status == 200 and body.decode() == "#ROOMS||NUMBER|NOW\n", body)

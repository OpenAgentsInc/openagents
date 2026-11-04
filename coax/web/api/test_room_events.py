# Everglade's world event (#12738; migrations/0118_room_events.sql): the dials an admin sets, and
# what everyone else may do with them. test.py runs this file.
import tempfile


def ev_signed(method, path, body, keyfile):
    raw = json.dumps(body).encode() if body is not None else None
    args = [os.environ["SIGN"], "--key", keyfile, method, API + path]
    if raw is not None:
        f = tempfile.NamedTemporaryFile(delete=False)
        f.write(raw)
        f.close()
        args.append(f.name)
    sig = subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()
    status, out, _ = call(method, path, raw, {"Content-Type": "application/json", "Authorization": "Nostr " + sig})
    return status, json.loads(out)


EV = "/api/playtest/room-event"
status, raw, _ = call("GET", EV)
got = json.loads(raw)
check("room-event: anyone reads the dials, off by default with every default", status == 200 and got["dials"] == {
    "on": False, "every": 0, "waves": 3, "size": 12, "cultists": 6, "fanatics": 1, "warlocks": 3, "warlock_level": 3, "gates": 31,
    "leader": True, "aggression": 2, "town_damage": True, "reward": 100, "minutes": 15, "levels": False, "level_cap": 2} and got["admin"] is False, raw)
_, praw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
visitor = json.loads(praw)["key"]
status, raw, _ = call("POST", EV, json.dumps({"dials": {"on": True}, "start": True}).encode(), {"Content-Type": "application/json", "X-Verse-Player": visitor})
check("room-event: a visitor can't change it", status == 403 and sql("select (dials->>'on') is null and start_n = 0 from verse.room_events where slug = 'everglade'") == "t", raw)
status, got = ev_signed("POST", EV, {"dials": {"on": True, "waves": 99, "size": 2, "reward": 1000, "every": 3, "levels": True, "aggression": -4}}, os.environ["ADMIN_KEY"])
d = got.get("dials", {})
check("room-event: a site admin's dials are kept, each clamped to its range", status == 200 and got["admin"] and d["on"] and d["waves"] == 8
      and d["size"] == 4 and d["reward"] == 300 and d["every"] == 15 and d["levels"] and d["aggression"] == 0 and d["minutes"] == 15, got)
status, got = ev_signed("POST", EV, {"dials": {"waves": 2}, "start": True}, os.environ["ADMIN_KEY"])
check("room-event: dials not named keep their values; START counts a press", status == 200 and got["dials"]["waves"] == 2 and got["dials"]["size"] == 4
      and got["start_n"] == 1 and got["stop_n"] == 0 and got["started_ms"] > 0, got)
status, got = ev_signed("POST", EV, {"stop": True}, os.environ["ADMIN_KEY"])
check("room-event: STOP counts a press", status == 200 and got["stop_n"] == 1 and got["start_n"] == 1, got)
status, raw, _ = call("POST", EV, b"not json", {"Content-Type": "application/json", "X-Verse-Player": visitor})
check("room-event: a body that isn't JSON is a 400", status == 400, raw)
line = sql("select 'event ' || concat_ws(' ', (d->>'on')::boolean::int, d->>'every', d->>'waves', d->>'size', d->>'cultists', d->>'fanatics', d->>'warlocks', "
           "d->>'warlock_level', d->>'gates', (d->>'leader')::boolean::int, d->>'aggression', (d->>'town_damage')::boolean::int, d->>'reward', d->>'minutes', "
           "e.start_n, e.stop_n) from verse.room_events e, lateral (select verse.room_event_dials(e.dials) as d) x where e.slug = 'everglade'")
check("room-event: room-control's line is the room server's 16 numbers", line == "event 1 15 2 4 6 1 3 3 31 1 0 1 300 15 1 1", line)
check("room-event: XP is a column of the vitals, and the migration runs twice",
      sql("select count(*) from information_schema.columns where table_schema = 'verse' and table_name = 'room_vitals' and column_name = 'xp'") == "1")

# oa-room's side, signed with the seat-ticket key (ROOM_KEY when the run has one): room-control's new
# lines, XP in the vitals report (it only rises), and the ticket bringing it back as HPxXP.
if os.environ.get("ROOM_KEY"):
    import hmac, hashlib, time as _t
    rk = os.environ["ROOM_KEY"].encode()
    now = str(int(_t.time() * 1000))
    mac = hmac.new(rk, ("t " + now).encode(), hashlib.sha256).hexdigest()
    status, raw, _ = call("GET", "/api/playtest/room-control", None, {"X-Room-Mac": mac, "X-Room-At": now})
    text = raw.decode()
    check("room-control: the event's line and the levels switch", status == 200 and "\nevent 1 15 2 4 6 1 3 3 31 1 0 1 300 15 1 1\n" in "\n" + text and "\nlevels 1 2\n" in text, text)
    pid = sql(f"select id from verse.players where key_sha256 = encode(sha256(convert_to('{visitor}', 'UTF8')), 'hex')")
    def vitals(body):
        m = hmac.new(rk, body.encode(), hashlib.sha256).hexdigest()
        return call("POST", "/api/playtest/room-vitals", body.encode(), {"X-Room-Mac": m, "Content-Type": "text/plain"})
    t0 = int(_t.time() * 1000)
    vitals(f"t {t0}\n{pid} 5 9 420\n")
    vitals(f"t {t0 + 1}\n{pid} 6 9 100\n")
    check("room-vitals: XP is kept and only rises; an old line (no XP) keeps it", sql(f"select xp || ' ' || hp from verse.room_vitals where who = '{pid}'") == "420 6")
    vitals(f"t {t0 + 2}\n{pid} 7 9\n")
    check("room-vitals: a line without XP keeps the XP", sql(f"select xp || ' ' || hp from verse.room_vitals where who = '{pid}'") == "420 7")
    status, raw, _ = call("POST", "/api/playtest/room-ticket/pair-tk", b"{}", {"Content-Type": "application/json", "X-Verse-Player": visitor})
    tk = json.loads(raw)
    check("room-ticket: its HP field carries the XP (HPxXP), and the answer says it", status == 200 and ".7x420." in tk["ticket"] and tk["xp"] == 420, raw)

# Admission when the rooms are full (#12796; migrations/0126_room_admission.sql, player.c room_ticket).
# test.py runs this file. It needs the seat-ticket key: run test.sh with OA_ROOM_TICKET_KEY (and ROOM_KEY)
# set to the same key.
#   ROOM_KEY=local-development-room-ticket-key-not-a-secret OA_ROOM_TICKET_KEY=$ROOM_KEY \
#   API_TESTS_ONLY=test_room_admission.py coax/web/api/test.sh
# 250 bots ask for 192 seats (6 shards of 2 rooms of 16): 192 are seated, 58 wait. The waiting pages ask
# again at the answer's retry_ms, so once each has asked again their asks a second are sum(1000 / retry_ms),
# and the bound is 5 a second (W / (5 + 0.2 W) for W waiting, under 5 up to 1475 waiting).
import time as _t

if os.environ.get("ROOM_KEY"):
    def ad_player():
        _, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
        key = json.loads(raw)["key"]
        return key, sql(f"select id from verse.players where key_sha256 = encode(sha256(convert_to('{key}', 'UTF8')), 'hex')")

    def ad_ask(key, i, room="evadm"):
        h = {"Content-Type": "application/json", "X-Verse-Player": key, "X-Forwarded-For": f"10.9.{i // 256}.{i % 256}, 130.211.0.1"}
        st, raw, hd = call("POST", f"/api/playtest/room-ticket/{room}?realm=evadm", b"{}", h)
        return st, json.loads(raw) if raw[:1] == b"{" else {}, hd

    caps = sql("select string_agg(shard || ':' || rooms_cap, ',' order by shard) from verse.room_shards")
    sql("update verse.room_shards set rooms_cap = 2, free = null, at = null")
    before = int(sql("select coalesce(sum(n), 0) from verse.room_admission where reason = 'full'"))
    bots = [ad_player() for _ in range(250)]
    answers = [ad_ask(k, i) for i, (k, _) in enumerate(bots)]
    # the seats are holds until a shard reports them; kept for the rest of the check
    sql("update verse.room_holds h set until_ms = until_ms + 600000 from verse.room_instances i where h.code = i.code and i.realm = 'evadm'")
    seated = [i for i, (st, _, _) in enumerate(answers) if st == 200]
    waiting = [i for i, (st, _, _) in enumerate(answers) if st == 503]
    check("room-admission: 250 bots for 192 seats: 192 seated, 58 wait", len(seated) == 192 and len(waiting) == 58, (len(seated), len(waiting)))
    last = answers[waiting[-1]][1] if waiting else {}
    check("room-admission: a full answer says how many wait and when to ask again (and Retry-After)",
          last.get("waiting") == 58 and last.get("retry_ms") == 5000 + 58 * 200
          and answers[waiting[-1]][2].get("Retry-After") == str((5000 + 58 * 200 + 999) // 1000), last)
    # the waiting pages' next asks (each at its retry_ms): their answers set the steady rate
    round2 = [ad_ask(bots[i][0], i) for i in waiting]
    rate = sum(1000.0 / a[1]["retry_ms"] for a in round2 if a[0] == 503)
    check(f"room-admission: once all have asked, the 58 waiting pages ask {rate:.2f} times a second, under the bound of 5",
          all(a[0] == 503 for a in round2) and rate < 5, rate)

    # a reconnect storm: every seated bot asks again at once and keeps its room
    again = [ad_ask(bots[i][0], i) for i in seated]
    check("room-admission: a reconnect storm of the 192 seated keeps every seat", all(st == 200 for st, _, _ in again)
          and all(again[j][1].get("room") == answers[i][1].get("room") for j, i in enumerate(seated)))

    # a seat frees: the next waiting bot is seated, and its wait is the last wait
    gone = answers[seated[0]][1]["room"]
    sql(f"delete from verse.room_holds where pid = '{bots[seated[0]][1]}'")
    w0 = waiting[0]
    st, tk, _ = ad_ask(bots[w0][0], w0)
    check("room-admission: a freed seat goes to a waiting bot", st == 200 and tk.get("room") == gone, (st, tk))
    st, tk, _ = ad_ask(bots[waiting[1]][0], waiting[1])
    check("room-admission: the next full answer names the last wait", st == 503 and tk.get("waiting") == 57 and tk.get("last_wait_ms", -1) >= 0, tk)

    # one player key asking too often: 429 with a retry time
    k, _ = ad_player()
    got = [ad_ask(k, 9000) for _ in range(13)]
    st, body, hd = got[-1]
    check("room-admission: the 13th ask in a minute from one player is 429 with retry_ms and Retry-After",
          all(g[0] != 429 for g in got[:12]) and st == 429 and 0 < body.get("retry_ms", 0) <= 60000 and int(hd.get("Retry-After", "0")) >= 1, (st, body))
    # one address asking for many players: limited at 120 a minute
    many = [ad_player() for _ in range(121)]
    by_addr = [ad_ask(mk, 7777)[0] for mk, _ in many]
    check("room-admission: one address is limited at 120 asks a minute", by_addr.count(429) == 1 and by_addr[-1] == 429, by_addr.count(429))

    # refusals counted by reason
    st, _, _ = ad_ask(bots[waiting[2]][0], waiting[2])   # writes the process's 429s with this 'full'
    counts = dict(l.split("|") for l in sql("select reason || '|' || n from verse.room_admission").splitlines())
    check("room-admission: refusals are counted by reason: full, rate, seated after a wait",
          int(counts.get("full", 0)) - before >= 60 and int(counts.get("rate", 0)) >= 2 and int(counts.get("seated after a wait", 0)) >= 1, counts)

    # back as the other checks found it
    sql("delete from verse.room_holds h using verse.room_instances i where h.code = i.code and i.realm = 'evadm'")
    sql("delete from verse.room_instances where realm = 'evadm'")
    sql("delete from verse.room_waiting")
    for kv in caps.split(","):
        s_, c_ = kv.split(":")
        sql(f"update verse.room_shards set rooms_cap = {c_} where shard = {s_}")

# An INVITE holds a seat in the inviter's room (#12795; migrations/0125_room_invites.sql, player.c
# room_invite, rooms.c). test.py runs this file. It needs the seat-ticket key, to sign the shard's
# directory report and the tickets: run test.sh with ROOM_KEY and OA_ROOM_TICKET_KEY set to the same key.
#   ROOM_KEY=local-development-room-ticket-key-not-a-secret OA_ROOM_TICKET_KEY=$ROOM_KEY \
#   API_TESTS_ONLY=test_room_invites.py coax/web/api/test.sh
import hashlib, hmac, time as _t

if os.environ.get("ROOM_KEY"):
    _ik = os.environ["ROOM_KEY"].encode()

    def ri_player():
        _, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
        key = json.loads(raw)["key"]
        return key, sql(f"select id from verse.players where key_sha256 = encode(sha256(convert_to('{key}', 'UTF8')), 'hex')")

    def ri_report(code, pids):
        body = f"t {int(_t.time() * 1000)}\ns 1 2\nr {code} {len(pids)} {' '.join(pids)}\n"
        m = hmac.new(_ik, body.encode(), hashlib.sha256).hexdigest()
        return call("POST", "/api/playtest/room-directory", body.encode(), {"X-Room-Mac": m, "Content-Type": "text/plain"})

    def ri_ticket(key, room, via=""):
        q = f"/api/playtest/room-ticket/{room}?realm=evinv" + (f"&with={via}" if via else "")
        st, raw, _ = call("POST", q, b"{}", {"Content-Type": "application/json", "X-Verse-Player": key})
        return st, (json.loads(raw) if st == 200 else {"status": st, "raw": raw.decode()})

    def ri_invite(key, room):
        st, raw, _ = call("POST", f"/api/playtest/room-invite/{room}", b"{}", {"Content-Type": "application/json", "X-Verse-Player": key})
        return st, raw

    alice_key, alice = ri_player()
    others = [f"p-evinv{i:010d}" for i in range(14)]
    st, raw, _ = ri_report("evinv-1", [alice] + others)
    check("room-invite: the shard's report puts Alice and 14 others in evinv-1", st == 200, raw)

    stranger_key, stranger = ri_player()
    st, raw = ri_invite(stranger_key, "evinv-1")
    check("room-invite: a player not in the room holds no seat (403)", st == 403, raw)
    st, raw = ri_invite(alice_key, "evinv-1")
    inv = json.loads(raw) if st == 200 else {}
    check("room-invite: Alice's INVITE holds the 16th seat for two minutes",
          st == 200 and inv.get("room") == "evinv-1" and inv.get("until", 0) > _t.time() * 1000 + 110000, raw)

    st, tk = ri_ticket(stranger_key, "evinv")
    check("room-invite: a stranger is placed elsewhere while the seat is held", st == 200 and tk.get("room") not in (None, "evinv-1"), tk)
    st, raw, _ = call("GET", f"/api/playtest/rooms/evinv?with={alice}")
    lst = json.loads(raw)
    check("room-invite: the list sends Alice's friend to her room while her invite holds a seat", st == 200 and lst.get("join") == "evinv-1" and lst.get("with") == "evinv-1", raw)

    friend_key, friend = ri_player()
    st, tk = ri_ticket(friend_key, "evinv-1", alice)
    check("room-invite: the friend's ticket takes the held seat", st == 200 and tk.get("room") == "evinv-1" and not tk.get("full"), tk)
    second_key, _ = ri_player()
    st, tk = ri_ticket(second_key, "evinv-1", alice)
    check("room-invite: claimed once: a second friend hears the room was full, whose it was, and is placed elsewhere",
          st == 200 and tk.get("full") is True and tk.get("asked") == "evinv-1" and tk.get("by") and tk.get("room") != "evinv-1", tk)
    st, raw = ri_invite(alice_key, "evinv-1")
    check("room-invite: no invite into a full room (409)", st == 409, raw)
    st, raw, _ = call("GET", f"/api/playtest/rooms/evinv?with={alice}")
    check("room-invite: the list names Alice's full room as with_full", st == 200 and json.loads(raw).get("with_full") == "evinv-1", raw)

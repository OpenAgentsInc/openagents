# Everglade room chat's moderation (migrations/0121_room_chat_moderation.sql): a kicked player's line is
# refused, blocked words are masked, and checks of one pair mark read each other's lines. test.py runs this file.


def rc_mint():
    status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    return json.loads(raw)


def rc_call(method, path, body=None, key=None, mark=None):
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    if mark:
        headers["X-Verse-Automation"] = mark
    status, raw, _ = call(method, path, json.dumps(body).encode() if body is not None else None, headers)
    return status, json.loads(raw)


RC = "/api/playtest/chat/everglade-9"
rc_a, rc_b = rc_mint(), rc_mint()
status, s = rc_call("POST", RC, {"text": "Shit, a FUCKING mess, and Scunthorpe"}, rc_a["key"])
check("a chat line's blocked words are masked, in any case", status == 200
      and sql(f"select text from verse.room_chat where room = 'everglade-9' and n = {s['n']}") == "****, a ****ING mess, and S****horpe", s)
status, s = rc_call("POST", RC, {"text": "a fine line"}, rc_a["key"])
check("a clean line is kept as said", status == 200 and sql(f"select text from verse.room_chat where room = 'everglade-9' and n = {s['n']}") == "a fine line", s)

sql(f"insert into verse.room_kicks (player, by_who, until_ms, at_ms) values ('{rc_b['id']}', 'test', verse.now_ms() + 60000, verse.now_ms())")
status, s = rc_call("POST", RC, {"text": "still here"}, rc_b["key"])
check("a kicked player can't say a line, and is told until when", status == 403 and s.get("error") == "kicked" and s.get("until", 0) > 0, s)
sql(f"update verse.room_kicks set until_ms = verse.now_ms() - 1 where player = '{rc_b['id']}'")
check("a kick that ended lets them speak again", rc_call("POST", RC, {"text": "back"}, rc_b["key"])[0] == 200)

# Two checks of one pair mark read each other's lines (as they see each other, 0074); another mark and a person don't.
RP = "/api/playtest/presence/pair-chat-t1"
pa, pb, pc, person = rc_mint(), rc_mint(), rc_mint(), rc_mint()
status, s = rc_call("POST", "/api/playtest/chat/pair-chat-t1", {"text": "from check a"}, pa["key"], "pair-chat-t1")
check("a paired check says a line", status == 200, s)
said = s["n"]
status, p = rc_call("POST", RP, {"after": said - 1}, pb["key"], "pair-chat-t1")
check("the other check of the pair reads it with its presence", status == 200 and [m["text"] for m in p["messages"]] == ["from check a"], p)
status, p = rc_call("GET", "/api/playtest/chat/pair-chat-t1?after=%d" % (said - 1), None, pb["key"], "pair-chat-t1")
check("and with a chat read", status == 200 and [m["text"] for m in p["messages"]] == ["from check a"], p)
check("another mark's check doesn't", rc_call("POST", RP, {"after": said - 1}, pc["key"], "pair-chat-other")[1]["messages"] == [])
check("nor does a person", rc_call("POST", RP, {"after": said - 1}, person["key"])[1]["messages"] == [])
check("the speaker still reads their own", [m["text"] for m in rc_call("POST", RP, {"after": said - 1}, pa["key"], "pair-chat-t1")[1]["messages"]] == ["from check a"])

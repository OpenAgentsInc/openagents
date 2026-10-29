# Talking with other players (ff-chat-room, ff-inbox; migrations/0053_verse_social.sql):
# lines say who said them, blocks, an admin removing a line, players who have met,
# and the inbox. test.py runs this file (after test_room.py, whose mint() it uses).


def soc(method, path, body=None, key=None, cookie=None):
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    if cookie:
        headers["Cookie"] = cookie
    status, raw, _ = call(method, path, json.dumps(body).encode() if body is not None else None, headers)
    try:
        return status, json.loads(raw)
    except ValueError:
        return status, raw


FF = "/api/playtest/presence/fairy-forest"
FC = "/api/playtest/chat/fairy-forest"
wren, moss, fern = mint(), mint(), mint()

# Lines say who said them
status, said = soc("POST", FC, {"text": "hello forest"}, moss["key"])
status, p = soc("POST", FF, {"x": 1, "y": 2.5, "z": 3, "after": said["n"] - 1}, wren["key"])
check("social: a line says who said it", status == 200 and p["messages"][0]["from"] == moss["id"] and p["removed"] == [], p)
check("social: a presence keeps how high you fly", sql(f"select y from verse.room_here where room = 'fairy-forest' and player = '{wren['id']}'") == "2.5")

# Met: two players in the forest at once have met; a third who never came has not
soc("POST", FF, {"x": 2, "z": 3, "after": 0}, moss["key"])
check("social: in the room together, they have met",
      sql(f"select verse.have_met('{wren['id']}', '{moss['id']}')") == "t" and sql(f"select verse.have_met('{wren['id']}', '{fern['id']}')") == "f")
status, box = soc("GET", "/api/playtest/inbox", key=wren["key"])
check("social: the inbox lists who you have met, and nothing yet", status == 200 and [m["id"] for m in box["met"]] == [moss["id"]]
      and box["messages"] == [] and box["unread"] == 0 and box["admin"] is False and box["you"]["id"] == wren["id"], box)
check("social: no key, no inbox", soc("GET", "/api/playtest/inbox")[0] == 403)

# A message waits for them, offline or not
status, sent = soc("POST", "/api/playtest/inbox", {"to": moss["id"], "text": "  meet me\nat the pond  "}, wren["key"])
check("social: a message to someone you have met", status == 200 and sent["id"] > 0, sent)
status, mb = soc("GET", "/api/playtest/inbox", key=moss["key"])
check("social: it waits in their inbox, unread, with who sent it", status == 200 and mb["unread"] == 1 and mb["messages"][-1]["text"] == "meet me at the pond"
      and mb["messages"][-1]["from"] == wren["id"] and mb["messages"][-1]["name"] == "Guest " + wren["id"][2:6].upper()
      and mb["messages"][-1]["mine"] is False and mb["messages"][-1]["read"] is False, mb)
check("social: and the sender sees it as theirs", soc("GET", "/api/playtest/inbox", key=wren["key"])[1]["messages"][-1]["mine"] is True)
check("social: reading another's leaves theirs unread", soc("POST", "/api/playtest/inbox/read", {"upto": sent["id"], "from": fern["id"]}, moss["key"])[1]["unread"] == 1)
status, rd = soc("POST", "/api/playtest/inbox/read", {"upto": sent["id"], "from": wren["id"]}, moss["key"])
check("social: read, it is no longer unread", status == 200 and rd["unread"] == 0
      and soc("GET", "/api/playtest/inbox", key=moss["key"])[1]["messages"][-1]["read"] is True, rd)
status, back = soc("POST", "/api/playtest/inbox", {"to": wren["id"], "text": "on my way"}, moss["key"])
check("social: they answer from the inbox", status == 200 and soc("GET", "/api/playtest/inbox", key=wren["key"])[1]["unread"] == 1, back)

# Who can write, and how much
check("social: only to someone you have met", soc("POST", "/api/playtest/inbox", {"to": fern["id"], "text": "hi"}, wren["key"])[0] == 403)
check("social: not to nobody, nor yourself", soc("POST", "/api/playtest/inbox", {"to": "p-nobody", "text": "hi"}, wren["key"])[0] == 404
      and soc("POST", "/api/playtest/inbox", {"to": wren["id"], "text": "hi"}, wren["key"])[0] == 400)
check("social: an empty message is refused", soc("POST", "/api/playtest/inbox", {"to": moss["id"], "text": " \n "}, wren["key"])[0] == 400)
status, long_ = soc("POST", "/api/playtest/inbox", {"to": moss["id"], "text": "y" * 3000}, wren["key"])
check("social: a message is at most 500 characters", status == 200 and sql(f"select length(text) from verse.inbox where id = {long_['id']}") == "500", long_)
for i in range(3):
    soc("POST", "/api/playtest/inbox", {"to": moss["id"], "text": "again %d" % i}, wren["key"])
check("social: five a minute, then a pause", soc("POST", "/api/playtest/inbox", {"to": moss["id"], "text": "too many"}, wren["key"])[0] == 429)
check("social: messages are never in analytics", sql("select count(*) from analytics.events where p::text like '%meet me at the pond%'") == "0")

# Blocks: their lines and messages are hidden, for you only
status, bl = soc("POST", "/api/playtest/block", {"player": wren["id"], "on": True}, moss["key"])
check("social: block a player", status == 200 and bl["blocked"] == [wren["id"]], bl)
mb = soc("GET", "/api/playtest/inbox", key=moss["key"])[1]
check("social: a blocked player's messages are hidden", all(m["from"] != wren["id"] and m["to"] != wren["id"] for m in mb["messages"]) and mb["unread"] == 0
      and mb["met"][0]["blocked"] is True, mb)
status, w = soc("POST", FC, {"text": "can you hear me"}, wren["key"])
lines_moss = soc("POST", FF, {"after": w["n"] - 1}, moss["key"])[1]["messages"]
lines_fern = soc("GET", FC + "?after=%d" % (w["n"] - 1), key=fern["key"])[1]["messages"]
check("social: and their lines, for who blocked them only", lines_moss == [] and [m["text"] for m in lines_fern] == ["can you hear me"], (lines_moss, lines_fern))
check("social: you can't block yourself or nobody", soc("POST", "/api/playtest/block", {"player": moss["id"]}, moss["key"])[0] == 404
      and soc("POST", "/api/playtest/block", {"player": "p-nobody"}, moss["key"])[0] == 404)
status, bl = soc("POST", "/api/playtest/block", {"player": wren["id"], "on": False}, moss["key"])
check("social: unblock, and they are back", status == 200 and bl["blocked"] == [] and soc("GET", "/api/playtest/inbox", key=moss["key"])[1]["unread"] > 0, bl)

# An admin removes a line
scookie = "oa_admin=" + "5" * 64
sql("insert into analytics.admin_sessions (token_sha256, pubkey, created_ms, expires_ms) values (encode(sha256(convert_to('%s', 'UTF8')), 'hex'), '%s', 0, 99999999999999) on conflict do nothing"
    % ("5" * 64, os.environ["OA_ADMIN_PUBKEYS"].split(",")[0]))
R = FC + "/remove"
check("social: a player can't remove a line", soc("POST", R, {"n": w["n"]}, moss["key"])[0] == 403)
check("social: the inbox says who is an admin", soc("GET", "/api/playtest/inbox", key=moss["key"], cookie=scookie)[1]["admin"] is True)
status, rm = soc("POST", R, {"n": w["n"]}, cookie=scookie)
check("social: an admin removes a line", status == 200 and rm["removed"] == w["n"], rm)
p = soc("POST", FF, {"after": w["n"] - 1}, fern["key"])[1]
check("social: it is gone for everyone, and the room says so", p["messages"] == [] and w["n"] in p["removed"], p)
check("social: removing needs a line", soc("POST", R, {"n": 999999}, cookie=scookie)[0] == 404 and soc("POST", R, {}, cookie=scookie)[0] == 400)

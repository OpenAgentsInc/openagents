# Rooms (pt-presence-chat): presence and chat. test.py runs this file.
import time


def mint():
    status, raw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    return json.loads(raw)


def room_call(method, path, body=None, key=None):
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    status, raw, _ = call(method, path, json.dumps(body).encode() if body is not None else None, headers)
    return status, json.loads(raw)


ann, bo = mint(), mint()
R = "/api/playtest/presence/haunted-house"
status, a = room_call("POST", R, {"x": 1.5, "y": 0, "z": -2, "yaw": 0.5, "after": 0}, ann["key"])
check("a player comes into a room and is alone there", status == 200 and a["others"] == [] and a["you"]["id"] == ann["id"]
      and a["you"]["name"] == "Guest " + ann["id"][2:6].upper(), a)
status, b = room_call("POST", R, {"x": 3, "z": 4, "yaw": 1, "after": 0}, bo["key"])
check("the next one sees the first, where they are", status == 200 and len(b["others"]) == 1 and b["others"][0]["id"] == ann["id"]
      and b["others"][0]["x"] == 1.5 and b["others"][0]["z"] == -2 and b["others"][0]["yaw"] == 0.5, b)
status, a = room_call("POST", R, {"x": 1.6, "z": -2, "after": 0}, ann["key"])
check("and the first sees them back", status == 200 and [o["id"] for o in a["others"]] == [bo["id"]], a)
check("another room is another room", room_call("POST", "/api/playtest/presence/w-elsewhere", {}, ann["key"])[1]["others"] == [])
check("no player key, no room", room_call("POST", R, {})[0] == 403 and room_call("POST", R, {}, "x" * 40)[0] == 403)
check("a room's name is a world id", room_call("POST", "/api/playtest/presence/a.b", {}, ann["key"])[0] == 404)
check("silly numbers are zero", room_call("POST", R, {"x": 1e30, "z": "far", "after": 0}, bo["key"])[0] == 200
      and sql(f"select x || ' ' || z from verse.room_here where player = '{bo['id']}' and room = 'haunted-house'") == "0 0")

C = "/api/playtest/chat/haunted-house"
status, s = room_call("POST", C, {"text": "  hello\nthere  ", "sid": "cabc123"}, ann["key"])
check("a line said in the room", status == 200 and s["n"] >= 1, s)
first = s["n"]
status, m = room_call("GET", C + "?after=%d" % (first - 1))
check("everyone reads it, with who said it", status == 200 and m["messages"][0]["text"] == "hello there"
      and m["messages"][0]["name"] == "Guest " + ann["id"][2:6].upper() and m["last"] == first, m)
status, p = room_call("POST", R, {"after": first - 1}, bo["key"])
check("presence brings the lines after the last one seen, marked whose", status == 200 and [x["text"] for x in p["messages"]] == ["hello there"]
      and p["messages"][0]["mine"] is False and p["last"] == first, p)
check("and the speaker's own are marked theirs", room_call("POST", R, {"after": first - 1}, ann["key"])[1]["messages"][0]["mine"] is True)
status, s = room_call("POST", C, {"text": "x" * 2000}, bo["key"])
check("a line is at most 500 characters", status == 200 and sql(f"select length(text) from verse.room_chat where room = 'haunted-house' and n = {s['n']}") == "500", s)
check("an empty line is refused", room_call("POST", C, {"text": "   "}, bo["key"])[0] == 400)
check("a guest without a key can't speak", room_call("POST", C, {"text": "hi"})[0] == 403)
for i in range(4):
    room_call("POST", C, {"text": "again %d" % i}, bo["key"])
check("five lines in ten seconds, then a pause", room_call("POST", C, {"text": "too fast"}, bo["key"])[0] == 429)
check("the sid is kept with the line", sql(f"select sid from verse.room_chat where room = 'haunted-house' and n = {first}") == "cabc123")
check("the demo's paths answer too", room_call("GET", "/api/chat/haunted-house?after=0")[0] == 200
      and room_call("POST", "/api/presence/haunted-house", {}, ann["key"])[0] == 200)
status, m = room_call("GET", C + "?after=0")
check("a read is the last 50 at most", status == 200 and len(m["messages"]) <= 50)

# A room holds 16: the 17th is told it's full; someone already in stays in.
crowd = [mint() for _ in range(14)]
for c in crowd:
    room_call("POST", "/api/playtest/presence/full-room", {}, c["key"])
room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])
room_call("POST", "/api/playtest/presence/full-room", {}, bo["key"])
late = mint()
check("a room holds 16 at once", room_call("POST", "/api/playtest/presence/full-room", {}, late["key"]) == (429, {"error": "the room is full"}))
check("those in it stay", room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])[0] == 200
      and len(room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])[1]["others"]) == 15)
sql("update verse.room_here set seen_ms = seen_ms - 7000 where room = 'full-room' and player = '%s'" % bo["id"])
check("someone gone six seconds isn't there any more, and frees a place",
      room_call("POST", "/api/playtest/presence/full-room", {}, late["key"])[0] == 200
      and bo["id"] not in [o["id"] for o in room_call("POST", "/api/playtest/presence/full-room", {}, ann["key"])[1]["others"]])

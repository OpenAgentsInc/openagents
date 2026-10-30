# Fairy Forest's courses (ff-courses; migrations/0057_fairy_courses.sql): a
# player's hoops kept under the id the page made, shared by link, and the
# fastest times flown on it. test.py runs this file.


def course_call(method, path, body=None, key=None):
    headers = {"Content-Type": "application/json"}
    if key:
        headers["X-Verse-Player"] = key
    status, raw, _ = call(method, path, json.dumps(body).encode() if body is not None else None, headers)
    try:
        return status, json.loads(raw)
    except ValueError:
        return status, raw


def course_player():
    return json.loads(call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})[1])


CO = "/api/playtest/course/"
maker, friend, other = course_player(), course_player(), course_player()
hoops = [[1, 3, 2, 0.5], [10.123, 4, -6, 1], [20, 5, -12, 1.5], [30, 2.5, -20, 2]]

check("course: an unknown course is a 404", course_call("GET", CO + "abcd1234")[0] == 404)
check("course: a bad id is not a route", course_call("GET", CO + "AB")[0] == 404)
check("course: no key, no course", course_call("POST", CO + "abcd1234", {"hoops": hoops})[0] == 403)
check("course: two hoops are not a course", course_call("POST", CO + "abcd1234", {"hoops": hoops[:2]}, maker["key"])[0] == 400)
check("course: a hoop is four numbers", course_call("POST", CO + "abcd1234", {"hoops": hoops[:3] + [[1, 2, "x", 0]]}, maker["key"])[0] == 400)
check("course: hoops stay in reach", course_call("POST", CO + "abcd1234", {"hoops": hoops[:3] + [[1, 2, 99999, 0]]}, maker["key"])[0] == 400)

status, made = course_call("POST", CO + "abcd1234", {"realm": "fairy-forest", "hoops": hoops}, maker["key"])
check("course: a new id is the maker's", status == 200 and made["mine"] is True and made["realm"] == "fairy-forest"
      and len(made["hoops"]) == 4 and made["hoops"][1][0] == 10.12 and made["times"] == [] and made["best"] is None, made)
status, seen = course_call("GET", CO + "abcd1234", key=friend["key"])
check("course: a friend opens it by its link", status == 200 and seen["mine"] is False and seen["by"] == "Guest " + maker["id"][2:6].upper()
      and seen["hoops"] == made["hoops"], seen)
check("course: and can't change it", course_call("POST", CO + "abcd1234", {"hoops": hoops[:3]}, friend["key"])[0] == 403)

status, ran = course_call("POST", CO + "abcd1234/time", {"ms": 15400}, friend["key"])
check("course: a run flown is kept", status == 200 and ran["best"] == 15400 and ran["times"][0]["ms"] == 15400 and ran["times"][0]["mine"] is True, ran)
check("course: one run at a time", course_call("POST", CO + "abcd1234/time", {"ms": 12000}, friend["key"])[0] == 429)
sql(f"update verse.course_times set at_ms = at_ms - 5000 where player = '{friend['id']}'")
status, slower = course_call("POST", CO + "abcd1234/time", {"ms": 19000}, friend["key"])
check("course: a slower run keeps the best", status == 200 and slower["best"] == 15400
      and sql(f"select runs from verse.course_times where player = '{friend['id']}'") == "2", slower)
status, mine = course_call("POST", CO + "abcd1234/time", {"ms": 11000.4}, maker["key"])
check("course: the fastest is first, and each sees which is theirs", status == 200 and [t["ms"] for t in mine["times"]] == [11000, 15400]
      and mine["times"][0]["mine"] is True and mine["times"][1]["mine"] is False, mine)
check("course: a run under a second is refused", course_call("POST", CO + "abcd1234/time", {"ms": 400}, other["key"])[0] == 400)
check("course: no time on no course", course_call("POST", CO + "zzzz9999/time", {"ms": 5000}, other["key"])[0] == 404)

status, again = course_call("POST", CO + "abcd1234", {"hoops": hoops}, maker["key"])
check("course: the same hoops sent again keep the times", status == 200 and len(again["times"]) == 2, again)
status, moved = course_call("POST", CO + "abcd1234", {"hoops": hoops[:3]}, maker["key"])
check("course: new hoops are a new course: the times clear", status == 200 and len(moved["hoops"]) == 3 and moved["times"] == [], moved)
check("course: other routes still answer", course_call("GET", "/api/playtest/player", key=maker["key"])[0] == 200)

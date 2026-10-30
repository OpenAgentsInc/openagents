# The panes edit mode brings up (ff-panes; migrations/0055_realm_panes.sql): a character's lore and
# a quest's form, written by a realm's owner and admins and read by everyone. test.py runs this file
# after test_social.py (soc, mint, scookie: a site admin's session).

RP = "/api/playtest/realms/fairy-forest"
guest = mint()


def panes(key=None):
    status, raw, _ = call("GET", RP + "/panes", headers={"X-Verse-Player": key} if key else {})
    return status, raw.decode()


def as_admin(path, body):
    status, raw, _ = call("POST", RP + path, json.dumps(body).encode(), {"Content-Type": "application/json", "Cookie": scookie, "Origin": API})
    return status, json.loads(raw)


s, text = panes()
check("panes: a realm with none says nothing", s == 200 and text == "", (s, text))
check("panes: no such realm is a 404", call("GET", "/api/playtest/realms/no-such-realm-x/panes")[0] == 404)

s, b = soc("POST", RP + "/lore", {"who": "bramble", "text": "Bramble hoards acorns."}, guest["key"])
check("panes: a visitor can't write lore", s == 403, (s, b))
s, b = soc("POST", RP + "/quest", {"giver": "bramble", "ask": "Find acorns"}, guest["key"])
check("panes: nor make a quest", s == 403, (s, b))

s, b = as_admin("/lore", {"who": "bramble", "text": "Bramble hoards acorns.\nHe | fears owls."})
check("panes: an admin saves a character's lore", s == 200 and b == {"who": "bramble", "saved": True}, (s, b))
s, b = as_admin("/lore", {"who": "Not A Key", "text": "x"})
check("panes: lore names its character by key", s == 400, (s, b))
s, b = as_admin("/lore", {"who": "pip", "text": "y" * 900})
check("panes: lore is capped at 400 characters", s == 200 and sql("select length(text) from verse.realm_lore where slug = 'fairy-forest' and who = 'pip'") == "400", (s, b))
s, b = as_admin("/quest", {"giver": "bramble", "ask": "Bring back the acorns", "steps": ["Find the oak", "", "Fill the basket", "Come home", "too many"], "reward": "A lucky acorn"})
check("panes: saving the form makes the quest", s == 200 and b == {"giver": "bramble", "saved": True}, (s, b))

s, text = panes(guest["key"])
check("panes: everyone reads the lore, one line, its bars and breaks made spaces",
      "lore bramble|Bramble hoards acorns. He   fears owls.\n" in text or "lore bramble|Bramble hoards acorns. He fears owls.\n" in text, text)
check("panes: and the quest, its first three steps in place (a blank one kept blank)",
      "quest bramble|Bring back the acorns|Find the oak||Fill the basket|A lucky acorn\n" in text, text)

s, b = as_admin("/quest", {"giver": "bramble", "ask": "Bring back the acorns", "steps": ["Find the oak"], "reward": ""})
check("panes: saving again changes the giver's one quest", s == 200 and "quest bramble|Bring back the acorns|Find the oak|||\n" in panes()[1], panes()[1])
s, b = as_admin("/quest", {"giver": "bramble", "ask": ""})
check("panes: an empty ask takes the quest away", s == 200 and b["saved"] is False and "quest " not in panes()[1], (s, b))
s, b = as_admin("/lore", {"who": "pip", "text": ""})
check("panes: empty lore forgets it", s == 200 and "lore pip|" not in panes()[1], (s, b))
s, b, _ = call("POST", RP + "/lore", json.dumps({"who": "bramble", "text": "x"}).encode(), {"Content-Type": "application/json", "Cookie": scookie, "Origin": "https://evil.example"})
check("panes: a write from another site is refused", s == 403, (s, b))
check("panes: nothing written reaches analytics",
      sql("select count(*) from analytics.events where p::text like '%acorn%'") == "0")

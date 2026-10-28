# The Health card (pt-analytics-contract): per surface, the last event, the
# last hour and day, and the contract events not seen in a day. test.py runs
# this file after test_dashboard.py, with its helpers.
contract_file = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "analytics-contract.json")))
check("the contract in the database is the one the release walk checks",
      sorted(sql("select surface || '/' || ev from analytics.contract").split("\n")) == sorted(f"{p}/{e}" for p, evs in contract_file.items() for e in evs))
__import__("time").sleep(1.1)
token = login(admin)[2].get("Set-Cookie", "").split(";")[0]
sql("delete from analytics.events where page = 'haunt' and ev = 'quest'")
for ev in [e for e in contract_file["haunt"] if e != "quest"]:
    sql(f"insert into analytics.events (ms, sid, ev, page, space) values (floor(extract(epoch from now()) * 1000)::bigint, 'c-health-1', '{ev}', 'haunt', 'web')")
health = call("GET", reports, headers={"Cookie": token})[1].decode()
section_ = health[health.index("#HEALTH|"):].split("\n#")[0].split("\n")
check("the report has a Health card", section_[0] == "#HEALTH||SURFACE|LAST EVENT|LAST HOUR|LAST DAY|CONTRACT EVENTS MISSING", section_[:2])
haunt = next((r for r in section_[1:] if r.startswith("haunt|")), "")
cells = haunt.split("|")
check("the house: its last event, counts for the hour and the day, and the missing quest named",
      len(cells) == 5 and cells[1] != "never" and int(cells[2]) >= 7 and int(cells[3]) >= 7 and cells[4] == "MISSING quest", haunt)
sql("insert into analytics.events (ms, sid, ev, page, space) values (floor(extract(epoch from now()) * 1000)::bigint, 'c-health-1', 'quest', 'haunt', 'web')")
health = call("GET", reports, headers={"Cookie": token})[1].decode()
check("once it arrives the house is whole", any(r.startswith("haunt|") and r.endswith("|none") for r in health.split("\n")), health[:400])
check("a surface nobody reached says never", any(r.startswith("galaxy|never|0|0|MISSING page-view") for r in health.split("\n"))
      or sql("select count(*) from analytics.events where page = 'galaxy'") != "0", health[:400])

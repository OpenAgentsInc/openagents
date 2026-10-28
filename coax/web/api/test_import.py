# The demo's data carried over (pt-import-demo-data): import_demo.py on a
# small copy laid out like the bucket aethyr-demo-data. test.py runs this file.
import hashlib, shutil, tempfile

demo = tempfile.mkdtemp()
here_api = os.path.dirname(os.path.abspath(__file__))


def put(rel, value, raw=False):
    path = os.path.join(demo, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(value if raw else json.dumps(value))


def run_import():
    r = subprocess.run([sys.executable, "-I", os.path.join(here_api, "import_demo.py"), demo, "--db", os.environ["PGURL"],
                        "--psql", os.environ["PSQL"]], capture_output=True, text=True)
    return r.returncode, (json.loads(r.stdout) if r.stdout.strip().startswith("{") else {}), r.stderr


old_key = "k" * 40
put("events/20261026T143949-fdef15-000001.jsonl",
    json.dumps({"t": 1793025588457, "sid": "kiki-c843929384becaa5-b711f85a", "acct": None, "ev": "session-start", "page": "kiki",
                "p": {"pid": "c843929384becaa5", "agent": "headlesschrome"}, "ct": 1793025588225, "auto": "kiki-e2e"}) + "\n"
    + json.dumps({"t": 1793025589000, "sid": "kiki-c843929384becaa5-b711f85a", "acct": None, "ev": "start", "page": "kiki",
                  "p": {"how": "guest", "to": "haunt"}, "ct": 1793025588900}) + "\n", raw=True)
put("dev/events/20261027T010101-aaaaaa-000001.jsonl",
    json.dumps({"t": 1793100000000, "sid": "smvdemo0001xyz", "acct": None, "ev": "prompt", "page": "haunt",
                "p": {"via": "npc", "text": "where is the key?", "consent": True}, "ct": 5}) + "\n", raw=True)
put("players.json", {hashlib.sha256(old_key.encode()).hexdigest(): {"id": "p-0123456789abcdef", "account": None, "at": 1793132080.5}})
put("dev/world-owners.json", {"planet-demo1": {"owner": "p-0123456789abcdef", "account": None, "at": 1793132162.2}})
put("world-planet-demo1.json", {"doc": {"kind": "verse.world.v1", "id": "planet-demo1"}, "version": 1, "publishedBy": "Kiki", "publishedAt": 1793132163.0})
put("world-taken.json", {"doc": {"kind": "verse.world.v1", "id": "taken", "from": "demo"}, "version": 3, "publishedBy": "Guest", "publishedAt": 1793132163.0})
put("log-planet-demo1.json", [{"version": 1, "who": "Kiki", "summary": "Made a planet", "at": 1793132163.0, "state": "published"}])
put("dev/log-haunt.json", [{"id": "86db41f422cc", "who": "Kiki", "summary": "Added a chest", "at": 1793213841.9, "state": "waiting", "delta": {}}])
put("playtesters/20261026T143951-fdef15-87934adead43ab54.json",
    {"id": "87934adead43ab54", "at": 1793025591.05, "email": "kiki-e2e-test@example.com", "player": "c843929384becaa5",
     "sid": "kiki-c843929384becaa5-b8d0bdc4", "how": "form", "auto": "kiki-e2e"})
put("survey-answers/20261027T010101-aaaaaa-785b94c923a58916.json",
    {"id": "785b94c923a58916", "at": 1793131513.9, "v": 3, "rid": "d5d13808a85f54cc", "q": "disappointed", "a": "very", "skip": False,
     "n": 1, "of": 13, "player": "485c89f4cba0209a", "sid": "kiki-485c89f4cba0209a-4a61f1df", "user": "anon-485c89f4cba0209a", "joinedAs": ""})
form = {"id": "9e8b379b7df36aac", "at": 1793025610.37, "answers": {"v": 2, "easy": "4"}, "auto": "kiki-e2e"}
put("surveys/20261026T144010-fdef15-9e8b379b7df36aac.json", form)
put("survey.json", [form, dict(form, id="05a01ba97cecf20e")])
put("awards.json", {"name-kiki-65hgyv": "planet-mekiki65hgyv6087"})
put("world-planet-mekiki65hgyv6087.json", {"doc": {"kind": "verse.world.v1", "planet": {"id": "planet-mekiki65hgyv6087", "name": "kiki's Village",
    "kind": "village", "art": "Pixel", "cam": "Top down", "cycle": "Always day", "owner": "kiki"}}, "version": 1, "publishedBy": "kiki", "publishedAt": 1793024148.5})
put("dev/awards.json", {"name-kiki-david-svazqc86": "planet-avidsvazqc860682"})
put("dev/planets.json", {"n": 1, "planets": {"planet-avidsvazqc860682": {"id": "planet-avidsvazqc860682", "name": "binulbadreet", "owner": "Kiki David",
    "ownerId": "name-kiki-david-svazqc86", "kind": "cemetery", "at": 1793132162.0, "art": "Low poly", "cam": "Third person", "cycle": "Always night",
    "made": 1793132162.0, "n": 1}}})
put("dev/planet-save-planet-avidsvazqc860682.json", {"saved": {"doc": {"format": "aethyr.record"}, "at": 1793132162.5}, "invited": 1793132170.0})
put("dev/chat-haunt.json", [{"n": 1, "name": "Kiki", "text": "hello", "at": 1793132100.0, "sid": "smvdemo0001xyz"}])
# A world openagents.com published itself keeps its own version.
sql("INSERT INTO verse.published (world, doc, version, published_by, published_at) VALUES ('taken', '{\"mine\":true}', 7, 'site', 1)")

code, rep, err = run_import()
check("import: counts match source and destination", code == 0 and rep["events"]["held"] == 3 and rep["sessions"]["held"] == 2
      and rep["players"]["held"] == 1 and rep["worlds"]["held"] == 1 and rep["log"]["held"] == 1 and rep["playtesters"]["held"] == 1
      and rep["survey_answers"]["held"] == 1 and rep["surveys"]["held"] == 2 and rep["files"]["held"] == 13, (code, rep, err))
check("import: a world openagents.com published is kept", sql("select version || ' ' || coalesce(source, '-') from verse.published where world = 'taken'") == "7 -"
      and rep["published"] == {"source": 3, "added": 2, "held": 2, "kept": 1}, rep.get("published"))
check("import: events keep their space, source file and line, and the demo's words",
      sql("select string_agg(space || ' ' || import_key, ',' order by import_key) from analytics.events where import_key is not null")
      == "dev aethyr-demo-data/dev/events/20261027T010101-aaaaaa-000001.jsonl:000001,"
         "kiki aethyr-demo-data/events/20261026T143949-fdef15-000001.jsonl:000001,kiki aethyr-demo-data/events/20261026T143949-fdef15-000001.jsonl:000002"
      and sql("select p->>'text' from analytics.events where ev = 'prompt' and space = 'dev'") == "where is the key?")
check("import: a session's first and last seen, events and robot agent",
      sql("select first_ms || ' ' || last_ms || ' ' || events || ' ' || agent || ' ' || space from analytics.sessions where sid = 'kiki-c843929384becaa5-b711f85a'")
      == "1793025588457 1793025589000 2 headlesschrome kiki")
check("import: awards, planets (the registry's and the Kiki flow's published ones) and a planet's saved copy",
      rep["awards"] == {"source": 2, "added": 2, "held": 2} and rep["planets"]["held"] == 2 and rep["planet_saves"]["held"] == 1
      and sql("select string_agg(id || ' ' || kind || ' ' || owner_id, ',' order by at) from verse.planets where source like 'aethyr-demo-data/%'")
         == "planet-mekiki65hgyv6087 village name-kiki-65hgyv,planet-avidsvazqc860682 cemetery name-kiki-david-svazqc86"
      and sql("select (invited > 0)::text || ' ' || (saved->>'at') from verse.planet_saves where planet = 'planet-avidsvazqc860682'") == "true 1793132162.5", rep)
check("import: an automated run keeps its mark", sql("select auto from analytics.playtesters where id = '87934adead43ab54'") == "kiki-e2e")
check("import: the rooms' chat has its table, its lines numbered in their room",
      rep["chat"] == {"source": 1, "added": 1, "held": 1}
      and sql("select room || ' ' || n || ' ' || name || ' ' || text || ' ' || space || ' ' || source from verse.room_chat where import_key is not null")
         == "haunt 1 Kiki hello dev aethyr-demo-data/dev/chat-haunt.json", rep.get("chat"))
check("import: offered changes have their table, chat is kept in demo_files too",
      sql("select count(*) from verse.log where world = 'haunt'") == "0" and rep["offers"] == {"source": 1, "added": 1, "held": 1}
      and sql("select state || ' ' || who || ' ' || source from verse.offers where world = 'haunt'") == "waiting Kiki aethyr-demo-data/dev/log-haunt.json"
      and sql("select doc->0->>'text' from verse.demo_files where path = 'aethyr-demo-data/dev/chat-haunt.json'") == "hello")
status, body, _ = call("GET", "/api/playtest/player", headers={"X-Verse-Player": old_key})
check("import: a demo player's old key still works", status == 200 and json.loads(body)["id"] == "p-0123456789abcdef", (status, body))

code, rep2, err = run_import()
check("import: re-running changes nothing", code == 0 and all(v["added"] == 0 for k, v in rep2.items() if k != "bad lines")
      and {k: v["held"] for k, v in rep2.items()} == {k: v["held"] for k, v in rep.items()}, (rep2, err))

with open(os.path.join(demo, "events/20261026T143949-fdef15-000001.jsonl"), "a") as f:
    f.write(json.dumps({"t": 1793025599000, "sid": "kiki-c843929384becaa5-b711f85a", "ev": "session-end", "page": "kiki", "p": {"ms": 11000}}) + "\n")
put("world-planet-demo1.json", {"doc": {"kind": "verse.world.v1", "id": "planet-demo1", "v": 2}, "version": 2, "publishedBy": "Kiki", "publishedAt": 1793132200.0})
code, rep3, err = run_import()
check("import: a later run adds only what the demo recorded since", code == 0 and rep3["events"]["added"] == 1 and rep3["sessions"]["added"] == 1
      and rep3["published"]["added"] == 1 and rep3["files"]["added"] == 1 and rep3["players"]["added"] == 0
      and sql("select version from verse.published where world = 'planet-demo1'") == "2", (rep3, err))
shutil.rmtree(demo)

# Room vitals receipts (#12793) and server-owned WANTED (#12794; migrations/0124_room_vital_receipts.sql).
# test.py runs this file. The room's half needs the seat-ticket key: run test.sh with ROOM_KEY and
# OA_ROOM_TICKET_KEY set to the same key. With ROOM_BIN (a room server built by coax/web/room/build.sh)
# and the stand-in's key (room-dev.coax DEV_KEY), it also runs the fault check: a raid in a room --dev
# whose vitals go to this API through a proxy that answers 500 for 30 s mid-raid and then holds four
# answers past the room's timeout, and the API's XP must end exactly the XP each seat earned.
#   ROOM_KEY=local-development-room-ticket-key-not-a-secret OA_ROOM_TICKET_KEY=$ROOM_KEY \
#   ROOM_BIN=coax/web/room/bin/room API_TESTS_ONLY=test_room_vitals.py coax/web/api/test.sh
import hashlib, hmac, threading, time as _t

if os.environ.get("ROOM_KEY"):
    _rk = os.environ["ROOM_KEY"].encode()

    def rv_post(body, mac=None):
        m = mac or hmac.new(_rk, body.encode(), hashlib.sha256).hexdigest()
        return call("POST", "/api/playtest/room-vitals", body.encode(), {"X-Room-Mac": m, "Content-Type": "text/plain"})

    _, praw, _ = call("POST", "/api/playtest/player", b"{}", {"Content-Type": "application/json"})
    rv_key = json.loads(praw)["key"]
    rv_pid = sql(f"select id from verse.players where key_sha256 = encode(sha256(convert_to('{rv_key}', 'UTF8')), 'hex')")
    now = int(_t.time() * 1000)
    gen = now - 60000

    def rv_body(batch, lines, shard=3, g=None, t=None):
        t = t or int(_t.time() * 1000)
        return f"t {t}\no {shard} {g or gen} {batch}\n" + "".join(l + "\n" for l in lines)

    st, raw, _ = rv_post(rv_body(1, [f"{rv_pid} 7 10 120 3 1 {now - 1000}"]))
    check("room-vitals: a post with operation IDs answers a receipt, a line applied", st == 200 and raw.decode() == f"receipt {gen} 1 a\n", raw)
    check("room-vitals: the line's HP, XP and WANTED are kept, WANTED by the room that wrote it",
          sql(f"select hp || ' ' || xp || ' ' || wanted || ' ' || wanted_by from verse.room_vitals where who = '{rv_pid}'") == f"7 120 3 3.{gen}")
    st, raw, _ = rv_post(rv_body(2, [f"{rv_pid} 7 10 120 3 1 {now - 1000}"]))
    check("room-vitals: the same operation again (a retry) is applied once: d", st == 200 and raw.decode() == f"receipt {gen} 2 d\n"
          and sql(f"select applied || ' ' || dups from verse.room_vital_ops where shard = 3 and gen = {gen} and pid = '{rv_pid}'") == "1 1", raw)
    rv_post(rv_body(3, [f"{rv_pid} 6 10 150 3 2 {now - 500}"]))
    st, raw, _ = rv_post(rv_body(4, [f"{rv_pid} 9 10 130 0 1 {now - 1000}"]))
    check("room-vitals: an older revision from the same process is stale and changes nothing", st == 200 and raw.decode() == f"receipt {gen} 4 s\n"
          and sql(f"select hp || ' ' || xp || ' ' || wanted from verse.room_vitals where who = '{rv_pid}'") == "6 150 3", raw)
    st, raw, _ = rv_post(rv_body(5, ["p-nobody00000000 5 10 1 0 1 " + str(now), "not a line", f"{rv_pid} 6 10 160 4 3 {now - 400}"]))
    check("room-vitals: a receipt names each line: no such player, unreadable, applied", st == 200 and raw.decode() == f"receipt {gen} 5 uxa\n", raw)
    st, raw, _ = rv_post(rv_body(6, [f"{rv_pid} 6 10 200 0 4 {now}"]), mac="0" * 64)
    check("room-vitals: an unsigned post is refused", st == 401 and sql(f"select xp from verse.room_vitals where who = '{rv_pid}'") == "160", raw)
    st, raw, _ = rv_post(rv_body(7, [f"{rv_pid} 6 10 155 4 1 {now - 300}"], g=gen + 5))
    check("room-vitals: another process's revisions are its own; XP still only rises", st == 200 and raw.decode() == f"receipt {gen + 5} 7 a\n"
          and sql(f"select xp from verse.room_vitals where who = '{rv_pid}'") == "160", raw)

    # WANTED is the room's: a page can't clear it, and the next ticket brings the room's level back
    st, raw, _ = call("POST", "/api/playtest/room-state", b'{"wanted":0}', {"Content-Type": "application/json", "X-Verse-Player": rv_key})
    check("room-state: a page's WANTED alone is refused", st == 403 and b"refused" in raw, raw)
    st, raw, _ = call("POST", "/api/playtest/room-state", b'{"wanted":0,"look":"elf.2"}', {"Content-Type": "application/json", "X-Verse-Player": rv_key})
    check("room-state: the look is kept and the WANTED refused", st == 200 and json.loads(raw).get("refused") == "wanted"
          and sql(f"select look || ' ' || wanted from verse.room_vitals where who = '{rv_pid}'") == "elf.2 4", raw)
    st, raw, _ = call("POST", "/api/playtest/room-ticket/pair-rv", b"{}", {"Content-Type": "application/json", "X-Verse-Player": rv_key})
    tk = json.loads(raw)
    check("room-ticket: after the page's 0 and a reload, the room's WANTED is still there (w4)", st == 200 and tk.get("wanted") == 4 and "x160w4." in tk["ticket"], raw)
    sql(f"update verse.room_vitals set wanted_by = null where who = '{rv_pid}'")
    st, raw, _ = call("POST", "/api/playtest/room-ticket/pair-rv", b"{}", {"Content-Type": "application/json", "X-Verse-Player": rv_key})
    tk = json.loads(raw)
    check("room-ticket: a WANTED level no room wrote (a page's, from before 0124) is not brought back", st == 200 and "wanted" not in tk and "w" not in tk["ticket"].split(".")[-2], raw)

# The fault check: a raid in a local room whose vitals reach this API through a bad proxy.
if os.environ.get("ROOM_KEY") and os.environ.get("ROOM_BIN"):
    import http.server, shutil, signal, socket, tempfile, urllib.request as _ur
    root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
    fault = {"mode": "pass", "raid": False, "at": 0, "passed": 0, "failed": 0, "held": 0, "after": 0}
    fault_lock = threading.Lock()

    class Faulty(http.server.BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def do_POST(self):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            # once the raid has started, every post fails for 30 s from the first; then the next two are
            # kept by the API but answered after the room's 5 s timeout, so the room sends them again within
            # its 10 s between notes, the same revisions (the API's d). Then all pass ("after" counts them).
            with fault_lock:
                if fault["raid"] and fault["mode"] == "pass" and not fault["failed"]:
                    fault["mode"] = "500"; fault["at"] = _t.time()
                if fault["mode"] == "500" and _t.time() - fault["at"] > 30:
                    fault["mode"] = "hold"
                if fault["mode"] == "hold" and fault["held"] >= 2:
                    fault["mode"] = "done"
                mode = fault["mode"]
            if mode == "500":
                with fault_lock:
                    fault["failed"] += 1
                self.send_response(500); self.send_header("Content-Length", "2"); self.end_headers(); self.wfile.write(b"{}")
                return
            req = _ur.Request(API + "/api/playtest/room-vitals", data=body, method="POST",
                              headers={"X-Room-Mac": self.headers.get("X-Room-Mac", ""), "Content-Type": "text/plain"})
            try:
                with _ur.urlopen(req) as r:
                    status, out = r.status, r.read()
            except urllib.error.HTTPError as e:
                status, out = e.code, e.read()
            if mode == "hold":   # the API kept it; the answer comes after the room has given up on it
                with fault_lock:
                    fault["held"] += 1
                _t.sleep(7)
            else:
                with fault_lock:
                    fault["passed"] += 1
                    if mode == "done":
                        fault["after"] += 1
            try:
                self.send_response(status); self.send_header("Content-Length", str(len(out))); self.end_headers(); self.wfile.write(out)
            except OSError:
                pass

    proxy = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Faulty)
    threading.Thread(target=proxy.serve_forever, daemon=True).start()
    n = 4
    for i in range(n):
        sql(f"insert into verse.players (id, key_sha256, created_ms) values ('p-raidcheck-{i}', 'rv-fault-{i}', 1) on conflict (id) do nothing")
    work = tempfile.mkdtemp()
    shutil.copy(os.path.join(root, "coax/web/room/dev/world-everglade.json"), work)
    dials = "event 1 0 3 12 6 1 3 3 31 1 2 1 100 15"
    open(os.path.join(work, "room-control.txt"), "w").write(f"{dials} 0 0\nlevels 1 2\n")
    s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
    log = open(os.path.join(work, "room.log"), "w")
    env = dict(os.environ, OA_ROOM_VITALS_URL=f"http://127.0.0.1:{proxy.server_address[1]}/api/playtest/room-vitals")
    room = subprocess.Popen([os.path.abspath(os.path.join(root, os.environ["ROOM_BIN"])) if not os.path.isabs(os.environ["ROOM_BIN"]) else os.environ["ROOM_BIN"],
                             "--port", str(port), "--dev", work], stdout=log, stderr=subprocess.STDOUT, env=env)
    _t.sleep(2)

    def raid_timeline():
        _t.sleep(6)
        open(os.path.join(work, "room-control.txt"), "w").write(f"{dials} 1 0\nlevels 1 2\n")   # START NOW
        _t.sleep(10); fault["raid"] = True    # the faults start with the next post (Faulty)
    threading.Thread(target=raid_timeline, daemon=True).start()
    rc = subprocess.run([os.path.join(root, "coax/web/tool.sh"), os.path.join(root, "coax/web/room/raid-check.coax"), f"ws://127.0.0.1:{port}", str(n), "100", str(int(_t.time()))],
                        capture_output=True, text=True, cwd=root, env=dict(os.environ, OA_ROOM_TICKET_KEY=os.environ["ROOM_KEY"]))
    got = {}
    for line in rc.stdout.splitlines():
        if line.startswith("{"):
            got = json.loads(line)
    seats = got.get("xp_seats", "").split()
    want = {f"p-raidcheck-{i}": sum(int(x) for x in seats[i][1:].split("+")) for i in range(len(seats))}
    # the seats have left: their notes flush; then the room drains on SIGTERM
    deadline = _t.time() + 90
    have = {}
    while _t.time() < deadline:
        have = dict(l.split(" ") for l in sql("select who || ' ' || coalesce(xp, 0) from verse.room_vitals where who like 'p-raidcheck-%'").splitlines() if l)
        # every seat's XP in, and the held posts sent again and answered
        if have and all(int(have.get(p, -1)) == x for p, x in want.items()) and fault["after"] > 0:
            break
        _t.sleep(1)
    room.send_signal(signal.SIGTERM)
    try:
        room.wait(15)
    except subprocess.TimeoutExpired:
        room.kill()
    log.close()
    text = open(os.path.join(work, "room.log")).read()
    proxy.shutdown()
    print(f"  fault check: heard XP {want}, API XP {have}, proxy {fault}")
    check("vitals fault: the raid ran with every seat, and they earned XP", len(want) == n and sum(want.values()) > 0, rc.stdout[-600:])
    check("vitals fault: the API refused and then held posts mid-raid", fault["failed"] > 0 and fault["held"] > 0, fault)
    check("vitals fault: the API's XP is exactly what each seat earned", all(int(have.get(p, -1)) == x for p, x in want.items()), (want, have))
    dups = sql("select coalesce(sum(dups), 0) || ' ' || coalesce(max(applied), 0) from verse.room_vital_ops where pid like 'p-raidcheck-%'")
    check("vitals fault: posts kept while their answers were lost came again and were not applied twice", int(dups.split()[0]) > 0, dups)
    check("vitals fault: the room exits on SIGTERM with every note saved", "0 players' vitals with no receipt" in text,
          " | ".join(l for l in text.splitlines() if "vitals" in l and "tick p95" not in l)[-800:])
    shutil.rmtree(work, ignore_errors=True)

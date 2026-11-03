# Private human packs (verse_packs.c, 0117_verse_pack_sessions.sql): a session a visitor, its
# bytes once under the session's own key, never the stored bytes or the plain pack, no-store, and
# the hourly and daily limits. test.sh makes the pack (VERSE_PACK_DIR) and the manifest
# (OA_VERSE_PACKS); test.py runs this file with its helpers.
import gzip as _vp_gzip, hashlib as _vp_hash, http.server as _vp_http, threading as _vp_threading

_vp_dir = os.environ["VERSE_PACK_DIR"]
_vp_plain = open(os.path.join(_vp_dir, "plain"), "rb").read()
_vp_object = open(os.path.join(_vp_dir, "object"), "rb").read()
_vp_sha = _vp_hash.sha256(_vp_plain).hexdigest()
_vp_asked = []


class _VpBucket(_vp_http.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        _vp_asked.append((self.path, self.headers.get("Authorization")))
        if self.path != f"/storage/v1/b/verse-test/o/packs%2F{_vp_sha}?alt=media" or self.headers.get("Authorization") != "Bearer test-gcs":
            self.send_response(404); self.send_header("Content-Length", "0"); self.end_headers(); return
        self.send_response(200); self.send_header("Content-Length", str(len(_vp_object))); self.end_headers()
        self.wfile.write(_vp_object)


_vp_server = _vp_http.ThreadingHTTPServer(("127.0.0.1", int(os.environ["VERSE_GCS_PORT"])), _VpBucket)
_vp_threading.Thread(target=_vp_server.serve_forever, daemon=True).start()


def _vp_session(name="test-human", ip="203.0.113.7"):
    status, raw, h = call("POST", f"/api/verse/packs/{name}", b"", {"X-Forwarded-For": f"{ip}, 130.211.0.1"})
    return status, (json.loads(raw) if raw.startswith(b"{") else {}), h


def _vp_ctr(key_hex, iv_hex, data):
    return subprocess.run(["openssl", "enc", "-d", "-aes-256-ctr", "-K", key_hex, "-iv", iv_hex], input=data, capture_output=True, check=True).stdout


sql("DELETE FROM verse.pack_sessions")
status, s, h = _vp_session()
check("a pack session answers its key, digest and sizes", status == 200 and len(s.get("key", "")) == 64 and s.get("iv", "").endswith("0" * 16)
      and s.get("sha256") == _vp_sha and s.get("bytes") == len(_vp_plain) and s.get("size") == len(_vp_object) and s.get("expires") == 120, s)
check("a pack session is never cached", "no-store" in h.get("Cache-Control", ""), h)
status, body, h = call("GET", s["url"])
check("the session's bytes arrive once, no-store", status == 200 and "no-store" in h.get("Cache-Control", "") and len(body) == len(_vp_object), (status, h, len(body)))
check("the bytes are neither the stored object nor the pack", body != _vp_object and _vp_plain[:64] not in body and _vp_object[:64] not in body)
plain = _vp_gzip.decompress(_vp_ctr(s["key"], s["iv"], body)) if status == 200 else b""
check("the session's key opens them to the pack, digest and all", _vp_hash.sha256(plain).hexdigest() == _vp_sha and len(plain) == s["bytes"])
check("a session's bytes come once", call("GET", s["url"])[0] == 410)
check("an unknown session is gone", call("GET", "/api/verse/packs/fetch/" + "0" * 32)[0] == 410 and call("GET", "/api/verse/packs/fetch/x/../../y")[0] == 410)
check("an unknown pack is a 404", _vp_session("no-such")[0] == 404 and _vp_session("Bad_Name")[0] == 404)
check("the routes take their own methods", call("GET", "/api/verse/packs/test-human")[0] == 405 and call("POST", s["url"], b"")[0] == 405)
status2, s2, _ = _vp_session()
_, other, _ = _vp_session(ip="203.0.113.8")
check("each session has its own key", status2 == 200 and s2["key"] != s["key"] and other.get("key") not in (None, s["key"], s2["key"]))
sql("UPDATE verse.pack_sessions SET at = now() - interval '121 seconds' WHERE sid = '%s'" % s2["url"].rsplit("/", 1)[1])
check("a session past its two minutes is gone", call("GET", s2["url"])[0] == 410)
check("an address's third session this hour is given", _vp_session()[0] == 200)
status, s3, _ = _vp_session()
check("an address gets no fourth session this hour (OA_VERSE_PACK_PER_HOUR)", status == 429 and "hour" in s3.get("error", ""), (status, s3))
check("the address is kept only as its digest", sql("SELECT count(*) FROM verse.pack_sessions WHERE ip_sha256 LIKE '203.%'") == "0"
      and sql("SELECT count(*) FROM verse.pack_sessions WHERE ip_sha256 = encode(sha256('203.0.113.7'::bytea), 'hex')") == "3")
sql("INSERT INTO verse.pack_sessions (sid, ip_sha256, pack, bytes, fetched_at) VALUES ('budget', 'x', 'test-human', 40000000000, now())")
status, s4, _ = _vp_session(ip="203.0.113.9")
check("the day's budget stops new sessions (OA_VERSE_PACK_DAILY_GB)", status == 429 and "budget" in s4.get("error", ""), (status, s4))
sql("DELETE FROM verse.pack_sessions")
check("the bucket was asked with the service's token", _vp_asked and all(a == "Bearer test-gcs" for _, a in _vp_asked))
_vp_server.shutdown(); _vp_server.server_close()

# Email through Resend (email.c, svc-email-resend): an admin's message reaches
# a fake Resend that test.sh points the service at, with its key, idempotency
# key, attachment and List-Unsubscribe; a repeated key is sent once; failures
# are retried; the allowlist holds; nothing private reaches the logs. test.py
# runs this file.
import base64
import http.server
import threading
import time as _time_mail

mail_seen = []
mail_mode = {"fail": 0, "status": 500}
mail_delivered = {}


class FakeResend(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        mail_seen.append({"auth": self.headers.get("Authorization"), "key": self.headers.get("Idempotency-Key"), "body": body})
        if mail_mode["fail"] > 0:
            mail_mode["fail"] -= 1
            status, out = mail_mode["status"], {"name": "application_error", "message": "try again"}
        else:
            k = self.headers.get("Idempotency-Key")
            mail_delivered.setdefault(k, "msg-%d" % (len(mail_delivered) + 1))   # Resend delivers a key once
            status, out = 200, {"id": mail_delivered[k]}
        data = json.dumps(out).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *a):
        pass


mail_fake = http.server.ThreadingHTTPServer(("127.0.0.1", int(os.environ["RESEND_PORT"])), FakeResend)
threading.Thread(target=mail_fake.serve_forever, daemon=True).start()


mail_last = {}


def mail(message, keyfile=os.environ["ADMIN_KEY"]):
    body = json.dumps(message).encode()
    if keyfile and _time_mail.time() - mail_last.get(body, 0) < 1.1:   # the same request signed in one second is one event, good once
        _time_mail.sleep(1.1)
    mail_last[body] = _time_mail.time()
    headers = {"Content-Type": "application/json"}
    if keyfile:
        path = os.path.join(os.environ.get("TMPDIR", "/tmp"), "oa-mail-body-%d" % os.getpid())
        with open(path, "wb") as f:
            f.write(body)
        signed = subprocess.run([os.environ["SIGN"], "--key", keyfile, "POST", API + "/api/admin/email", path],
                                capture_output=True, text=True, check=True).stdout.strip()
        os.unlink(path)
        headers["Authorization"] = "Nostr " + signed
    return call("POST", "/api/admin/email", body, headers)


pdf = b"%PDF-1.4\n1 0 obj<</Type/Catalog>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n"
letter = {"to": "owner@example.com", "subject": "Mail test, private subject", "text": "A private plain body",
          "html": "<p>A private html body</p>", "idempotency_key": "mail-test-0001",
          "attachments": [{"filename": "standup.pdf", "content": base64.b64encode(pdf).decode()}]}

check("email: unsigned is a 401", mail(letter, keyfile=None)[0] == 401)
check("email: a non-admin is a 403", mail(letter, keyfile=os.environ["OTHER_KEY"])[0] == 403)
check("email: nothing reached Resend for them", mail_seen == [])
s, b, _ = mail(letter)
sent = json.loads(b) if s == 200 else {}
check("email: an admin's message is sent", s == 200 and sent.get("state") == "sent" and sent.get("id") == "msg-1", (s, b))
m = mail_seen[-1] if mail_seen else {"body": {}, "auth": None, "key": None}
check("email: with the key, from no-reply", m["auth"] == "Bearer test-resend" and m["body"].get("from") == "OpenAgents <no-reply@openagents.com>")
check("email: to, subject, text and html as given", m["body"].get("to") == ["owner@example.com"] and m["body"].get("subject") == letter["subject"]
      and m["body"].get("text") == letter["text"] and m["body"].get("html") == letter["html"])
check("email: the PDF attached as sent", [(a["filename"], base64.b64decode(a["content"])) for a in m["body"].get("attachments", [])] == [("standup.pdf", pdf)])
check("email: a List-Unsubscribe header", m["body"].get("headers", {}).get("List-Unsubscribe", "").startswith("<"))
check("email: an idempotency key that is not the caller's", m["key"] and m["key"] != letter["idempotency_key"] and len(m["key"]) == 64)
before = len(mail_seen)
s, b, _ = mail(letter)
check("email: the same key again is already sent, and Resend is not asked", s == 200 and json.loads(b) == {"state": "already sent", "id": "msg-1"}
      and len(mail_seen) == before, (s, b))
mail_mode.update(fail=2, status=503)
s, b, _ = mail(dict(letter, idempotency_key="mail-test-0002"))
check("email: a 503 twice, then sent on the third attempt", s == 200 and json.loads(b).get("state") == "sent" and len(mail_seen) == before + 3, (s, b))
mail_mode.update(fail=1, status=422)
s, b, _ = mail(dict(letter, idempotency_key="mail-test-0003"))
check("email: a 422 is not retried, and is a 502", s == 502 and len(mail_seen) == before + 4, (s, b))
s, b, _ = mail(dict(letter, idempotency_key="mail-test-0003"))
check("email: a failed key may go again", s == 200 and json.loads(b).get("state") == "sent", (s, b))
check("email: a bad address is a 400", mail(dict(letter, to="not an address", idempotency_key="mail-test-0004"))[0] == 400)
check("email: no body is a 400", mail(dict(letter, text="", idempotency_key="mail-test-0005"))[0] == 400)
check("email: a short key is a 400", mail(dict(letter, idempotency_key="short"))[0] == 400)
check("email: not JSON is a 400", call("POST", "/api/admin/email", b"{", {"Authorization": "Nostr " + subprocess.run(
    [os.environ["SIGN"], "--key", os.environ["ADMIN_KEY"], "POST", API + "/api/admin/email"], capture_output=True, text=True, check=True).stdout.strip()})[0] in (400, 401))
s, b, _ = mail(dict(letter, to=["owner@example.com", "stranger@example.org"], idempotency_key="mail-test-0006"))
check("email: an address off the allowlist is refused", s == 400 and "allowlist" in b.decode(), (s, b))
rows = sql("SELECT state || ':' || recipients || ':' || attachments || ':' || attempts FROM email.sends ORDER BY created_ms")
check("email: the send log counts, never says", rows.split() == ["sent:1:1:1", "sent:1:1:3", "sent:1:1:2"], rows)
logs = open(os.environ["API_LOG"]).read() + sql("SELECT row_to_json(s)::text FROM email.sends s")
check("email: no key, address, subject or body in the logs", not any(x in logs for x in
      ["test-resend", "owner@example.com", "private subject", "private plain", "private html", "mail-test-0001", base64.b64encode(pdf).decode()[:20]]))

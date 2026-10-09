"""The web smoke suite: what shipped for 1.0, checked over HTTP against a
running site (staging by default). One PASS, FAIL, or SKIP line per check;
exit status 1 when anything fails. Python standard library only.

    scripts/smoke/staging.sh [BASE_URL] [--no-install] [--only NAME,...] [--restart]

`--restart` forces a new revision of the service (gcloud, the automation
account) and checks the account and its keys survive it.

Signed-in checks use an account made through the account service's own
sign-up (`POST /api/v1/accounts`), never GitHub. Chats it starts are
deleted afterwards. Nothing secret is printed.

`--production` is for openagents.com and its no-traffic tag URL: it asks
one starter question instead of four, makes no account, and skips the
sign-in and account groups (production has no account service yet,
#11127), so it writes nothing beyond that one chat.
"""

import argparse
import concurrent.futures
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

DEFAULT_BASE = "https://staging.openagents.com"
TIMEOUT = 30


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


class Response:
    def __init__(self, status, headers, body):
        self.status = status
        self.headers = headers
        self.body = body

    @property
    def text(self):
        return self.body.decode("utf-8", "replace")

    def json(self):
        return json.loads(self.body)

    def location(self):
        return self.headers.get("Location") or ""

    def cookies(self):
        out = {}
        for value in self.headers.get_all("Set-Cookie") or []:
            pair = value.split(";", 1)[0]
            if "=" in pair:
                name, val = pair.split("=", 1)
                out[name.strip()] = val.strip()
        return out


class Site:
    def __init__(self, base):
        self.base = base.rstrip("/")
        self.cookies = {}

    def request(self, path, method="GET", form=None, body=None, headers=None, cookies=True):
        url = path if path.startswith("http") else self.base + path
        h = {"User-Agent": "openagents-web-smoke/1"}
        data = None
        if form is not None:
            data = urllib.parse.urlencode(form).encode()
            h["Content-Type"] = "application/x-www-form-urlencoded"
        elif body is not None:
            data = json.dumps(body).encode()
            h["Content-Type"] = "application/json"
        if method == "POST" and not path.startswith("http"):
            h["Origin"] = self.base
            h["Sec-Fetch-Site"] = "same-origin"
        if cookies and self.cookies and not path.startswith("http"):
            h["Cookie"] = "; ".join(f"{k}={v}" for k, v in self.cookies.items())
        h.update(headers or {})
        req = urllib.request.Request(url, data=data, headers=h, method=method)
        try:
            r = OPENER.open(req, timeout=TIMEOUT)
            resp = Response(r.status, r.headers, r.read())
        except urllib.error.HTTPError as e:
            resp = Response(e.code, e.headers, e.read())
        if cookies and not path.startswith("http"):
            self.cookies.update(resp.cookies())
        return resp

    def get(self, path, **kw):
        return self.request(path, **kw)

    def follow(self, path, limit=5):
        """GET following same-site redirects, keeping cookies."""
        r = self.get(path)
        for _ in range(limit):
            if r.status not in (301, 302, 303, 307, 308):
                break
            loc = r.location()
            if loc.startswith(self.base):
                loc = loc[len(self.base):]
            if not loc.startswith("/"):
                break
            r = self.get(loc)
        return r


RESULTS = []


def record(name, ok, detail=""):
    word = "SKIP" if ok is None else ("PASS" if ok else "FAIL")
    RESULTS.append((word, name, detail))
    line = f"{word}  {name}"
    if detail:
        line += f"  ({detail})"
    print(line, flush=True)


def check(name):
    def wrap(fn):
        fn.check_name = name
        return fn
    return wrap


def hidden_fields(html, form_id=None):
    fields = {}
    for tag in re.findall(r"<input[^>]*>", html):
        if 'type="hidden"' not in tag:
            continue
        name = re.search(r' name="([^"]*)"', tag)
        value = re.search(r' value="([^"]*)"', tag)
        if name and value:
            fields.setdefault(name.group(1), unescape(value.group(1)))
    return fields


def unescape(text):
    for a, b in (("&lt;", "<"), ("&gt;", ">"), ("&quot;", '"'), ("&#39;", "'"),
                 ("&#x27;", "'"), ("&nbsp;", " "), ("&amp;", "&")):
        text = text.replace(a, b)
    return text


def plain(html):
    text = re.sub(r"<script.*?</script>|<style.*?</style>", " ", html, flags=re.S)
    text = re.sub(r"<[^>]+>", " ", text)
    return " ".join(unescape(text).split())


def replies(html):
    """Each reply's visible text, between the page's reply markers."""
    out = []
    for m in re.finditer(r'data-oa-reply="', html):
        rest = html[m.start():]
        start = rest.find(">") + 1
        end = rest.find("data-oa-reply-end")
        if end < 0:
            end = len(rest)
        out.append(plain(rest[start:end]))
    return out


def working(html):
    at = html.find('id="chat-status"')
    if at < 0:
        return False
    status = html[at:]
    end = status.find("</div>")
    return "oa-busy" in status[: end if end >= 0 else len(status)]


# ---------------------------------------------------------------------------
# Chat


def ask(base, question, stream=False):
    """Ask on a fresh visitor; returns (answer text, streamed?, error)."""
    site = Site(base)
    home = site.get("/")
    if home.status != 200 or "oa_visitor" not in site.cookies:
        return None, False, f"GET / {home.status}, visitor cookie {'oa_visitor' in site.cookies}"
    fields = hidden_fields(home.text)
    chat = str(uuid.uuid4())
    form = {"q": question, "request_id": chat, "csrf": fields.get("csrf", ""),
            "selection": fields.get("selection", ""), "project": ""}
    posted = site.request("/chat", method="POST", form=form)
    if posted.status >= 400:
        return None, False, f"POST /chat {posted.status}: {plain(posted.text)[:120]}"
    streamed = False
    if stream:
        streamed = read_events(site, chat)
    deadline = time.time() + 150
    answer, error = None, None
    while time.time() < deadline:
        t = site.get(f"/chat/{chat}/transcript")
        if t.status != 200:
            error = f"transcript {t.status}"
            break
        found = replies(t.text)
        if found and not working(t.text):
            answer = found[0]
            if not answer:
                error = "empty reply"
            break
        time.sleep(1)
    else:
        error = "no whole reply in 150 s"
    site.request(f"/chat/{chat}/delete", method="POST", form={"csrf": fields.get("csrf", "")})
    return answer, streamed, error


def read_events(site, chat, seconds=60):
    """Whether /chat/{id}/events streams server-sent events."""
    url = f"{site.base}/chat/{chat}/events"
    headers = {"User-Agent": "openagents-web-smoke/1", "Accept": "text/event-stream",
               "Cookie": "; ".join(f"{k}={v}" for k, v in site.cookies.items())}
    try:
        r = OPENER.open(urllib.request.Request(url, headers=headers), timeout=seconds)
    except urllib.error.HTTPError:
        return False
    except Exception:
        return False
    if "text/event-stream" not in (r.headers.get("Content-Type") or ""):
        return False
    end = time.time() + seconds
    try:
        while time.time() < end:
            line = r.readline()
            if not line:
                break
            if line.startswith(b"event:") or line.startswith(b"data:"):
                return True
    except Exception:
        return False
    finally:
        r.close()
    return False


# ---------------------------------------------------------------------------
# Traces (#11109)


def traces(base, session):
    site = Site(base)
    anon = site.get("/api/traces")
    if anon.status == 404:
        record("traces: /api/traces", None, "not on this build (#11109)")
        return
    record("traces: signed out is refused", anon.status == 401, f"{anon.status}")
    if not session:
        record("traces: upload, list, read, share, delete", None, "no test account")
        return
    auth = {"Authorization": f"Bearer {session}"}
    doc = {
        "schema_version": "ATIF-v1.7",
        "session_id": f"smoke-{uuid.uuid4()}",
        "agent": {"name": "openagents-coder", "version": "1", "model_name": "smoke-model"},
        "steps": [
            {"step_id": 1, "source": "user", "message": "Run the tests."},
            {"step_id": 2, "source": "agent", "message": "Done. **All passed.**",
             "model_name": "smoke-model",
             "tool_calls": [{"tool_call_id": "c1", "function_name": "shell",
                             "arguments": {"command": "cargo test"}}],
             "observation": {"results": [{"source_call_id": "c1", "content": "test result: ok"}]}},
        ],
    }
    up = site.request("/api/traces", method="POST", body=doc, headers=auth)
    try:
        trace = up.json()["trace"]
        tid = trace["id"]
    except (ValueError, KeyError, TypeError):
        record("traces: POST /api/traces saves an ATIF trace", False, f"{up.status} {up.text[:120]}")
        return
    record("traces: POST /api/traces saves an ATIF trace", up.status == 201 and not trace.get("shared"),
           f"{up.status} {trace.get('steps')} steps")
    again = site.request("/api/traces", method="POST", body=doc, headers=auth)
    record("traces: the same trace twice is saved once", again.status == 200, f"{again.status}")
    listed = site.get("/api/traces", headers=auth)
    try:
        ids = [t["id"] for t in listed.json()["traces"]]
    except (ValueError, KeyError, TypeError):
        ids = []
    record("traces: GET /api/traces lists it", tid in ids, f"{len(ids)} traces")
    one = site.get(f"/api/traces/{tid}", headers=auth)
    try:
        same = one.json().get("session_id") == doc["session_id"]
    except ValueError:
        same = False
    record("traces: GET /api/traces/{id} returns the document", one.status == 200 and same, f"{one.status}")
    private = Site(base).get(f"/trace/{tid}")
    record("traces: unshared /trace/{id} is 404", private.status == 404, f"{private.status}")
    on = site.request(f"/api/traces/{tid}/share", method="POST", body={"shared": True}, headers=auth)
    public = Site(base).get(f"/trace/{tid}")
    record("traces: shared /trace/{id} is public", on.status == 200 and public.status == 200
           and "All passed" in public.text, f"{on.status}/{public.status}")
    off = site.request(f"/api/traces/{tid}/share", method="POST", body={"shared": False}, headers=auth)
    gone = Site(base).get(f"/trace/{tid}")
    record("traces: stop sharing makes /trace/{id} 404", off.status == 200 and gone.status == 404,
           f"{off.status}/{gone.status}")
    page = Site(base)
    page.cookies["oa_cloud_session"] = session
    settings = page.follow("/settings/traces")
    record("traces: Settings > Traces lists it", settings.status == 200 and tid in settings.text,
           f"{settings.status}")
    deleted = site.request(f"/api/traces/{tid}", method="DELETE", headers=auth)
    after = site.get(f"/api/traces/{tid}", headers=auth)
    record("traces: DELETE removes it", deleted.status == 200 and after.status == 404,
           f"{deleted.status}/{after.status}")


# ---------------------------------------------------------------------------
# Durable accounts across a new revision (#11127)


def gcloud(*args):
    env = dict(os.environ)
    env.setdefault("CLOUDSDK_CONFIG", os.path.expanduser("~/work/.secrets/gcloud-sa-config"))
    return subprocess.run(["gcloud", *args], env=env, capture_output=True, text=True, timeout=900)


def durable(base, token, service, region, project):
    """Make an account, an API key, a saved provider key and a saved
    own-Claude key; force a new revision of `service`; check all four
    still work. Opt-in (--restart): it replaces the running instance."""
    if not token:
        record("durable: test account", None, "SMOKE_SIGNUP_TOKEN unset")
        return
    api = Site(base)
    made = api.request("/api/v1/accounts", method="POST", body={"label": "Smoke durable"},
                       headers={"Authorization": f"Bearer {token}"})
    try:
        d = made.json()
        session, key, ws, account_id = (d["session_token"], d["key_token"],
                                        d["workspace"]["id"], d["account"]["id"])
    except (ValueError, KeyError, TypeError):
        record("durable: test account", False, f"{made.status}")
        return
    bearer = {"Authorization": f"Bearer {session}"}
    provider = "sk-or-v1-smoke-" + uuid.uuid4().hex
    put = api.request(f"/api/v1/workspaces/{ws}/provider-keys/openrouter", method="PUT",
                      body={"key": provider}, headers=bearer)
    try:
        fingerprint = put.json()["fingerprint"]
    except (ValueError, KeyError, TypeError):
        fingerprint = None
    web = Site(base)
    web.cookies["oa_cloud_session"] = session
    page = web.follow("/settings/claude")
    form = re.search(r'<form[^>]*action="/settings/claude"[^>]*>(.*?)</form>', page.text, re.S)
    fields = hidden_fields(form.group(1)) if form else {}
    saved = web.request("/settings/claude", method="POST", form={
        "csrf": fields.get("csrf", ""), "request": fields.get("request", ""),
        "material": "anthropic_api_key", "value": "sk-ant-api03-smoke-durable-not-a-real-key",
        "consent": "custody"})
    claude_saved = saved.status in (302, 303) and "Saved: Anthropic API key" in web.follow(
        "/settings/claude").text
    record("durable: account, API key, provider key and own-Claude key made",
           bool(fingerprint) and claude_saved,
           f"provider key {put.status}, own-Claude key {saved.status}")

    def still(label):
        acct = api.request("/api/v1/account", headers=bearer)
        try:
            same = acct.json()["account"]["id"] == account_id
        except (ValueError, KeyError, TypeError):
            same = False
        record(f"durable: {label}: the session still signs in", acct.status == 200 and same,
               f"{acct.status}")
        # A valid key gets past authentication to the body check (400);
        # an unknown one is 401. No model call is made.
        r = api.request("/api/v1/responses", method="POST", body={},
                        headers={"Authorization": f"Bearer {key}"})
        record(f"durable: {label}: the API key authenticates", r.status == 400, f"{r.status}")
        listed = api.request(f"/api/v1/workspaces/{ws}/provider-keys", headers=bearer)
        try:
            prints = [k["fingerprint"] for k in listed.json()["keys"] if k["provider"] == "openrouter"]
        except (ValueError, KeyError, TypeError):
            prints = []
        record(f"durable: {label}: the saved provider key is kept", prints == [fingerprint],
               f"{listed.status}")
        mine = web.follow("/settings/claude")
        record(f"durable: {label}: the saved own-Claude key is kept",
               mine.status == 200 and "Saved: Anthropic API key" in mine.text, f"{mine.status}")

    still("before")
    where = ["--region", region, "--project", project]
    before = gcloud("run", "services", "describe", service, *where,
                    "--format=value(status.latestReadyRevisionName)").stdout.strip()
    forced = gcloud("run", "services", "update", service, *where, "--quiet",
                    f"--update-labels=smoke-restart={int(time.time())}")
    after = gcloud("run", "services", "describe", service, *where,
                   "--format=value(status.latestReadyRevisionName)").stdout.strip()
    record("durable: a new revision takes the traffic", forced.returncode == 0 and after
           and after != before, f"{before} -> {after}" if forced.returncode == 0
           else (forced.stderr.strip().splitlines() or ["?"])[-1][:160])
    if forced.returncode == 0:
        still("after a new revision")


# Checks


def run(base, only, install, production=False, restart=None):
    site = Site(base)
    want = (lambda name: True) if not only else (lambda name: any(name.startswith(o) for o in only))

    # Home: the composer, the four starter questions, the theme toggle.
    if want("home"):
        home = site.get("/")
        chips = re.findall(r'<form class="oa-suggestion-form"[^>]*>.*?</form>', home.text, re.S)
        questions = [hidden_fields(c).get("q", "") for c in chips]
        record("home: loads", home.status == 200, f"{home.status}")
        record("home: composer", 'data-oa-composer' in home.text and 'name="csrf"' in home.text)
        record("home: four starter questions", len(questions) >= 4, "; ".join(questions[:4]))
        # #11123: every new chat looks like a chat page: the composer docked
        # at the bottom with the starter questions, cards in the middle.
        dock = home.text.find('class="oa-main-composer"')
        record("home: composer docked at the bottom like a chat",
               'data-mode="app"' in home.text and dock >= 0
               and 0 <= dock < home.text.find('id="chat-suggestions"')
               < home.text.find('id="chat-form"'))
        cards = re.findall(r'<a class="oa-link-card" href="([^"]+)"', home.text)
        record("home: four learn-about cards", len(cards) == 4, ", ".join(cards))
        for href in cards:
            href = unescape(href)
            r = site.get(href, cookies=False) if href.startswith("http") else site.follow(href)
            record(f"home: card {href} is live", r.status == 200, f"{r.status}")
        corner = home.text.find('class="oa-sidebar-corner"')
        record("home: theme toggle in the sidebar corner",
               corner >= 0 and "data-oa-theme-toggle" in home.text[corner:corner + 1500])
        if want("home-chat") or not only:
            with concurrent.futures.ThreadPoolExecutor(4) as pool:
                asked = questions[:1] if production else questions[:4]
                futures = {q: pool.submit(ask, base, q, i == 0) for i, q in enumerate(asked)}
                for i, (q, f) in enumerate(futures.items()):
                    answer, streamed, error = f.result()
                    record(f"home: '{q}' gets an answer", error is None and bool(answer),
                           error or f"{len(answer)} chars: {answer[:60]}")
                    if i == 0:
                        record("chat: answer streams over /chat/{id}/events", streamed)

    # Docs.
    if want("docs"):
        docs = site.get("/docs")
        slugs = sorted(set(re.findall(r'href="/docs/([a-z0-9-]+)"', docs.text)) - {"api"})
        record("docs: index", docs.status == 200 and len(slugs) > 0, f"{len(slugs)} pages")
        bad = []
        crumbs = 0
        for slug in slugs:
            page = site.get(f"/docs/{slug}")
            if page.status != 200:
                bad.append(f"{slug} {page.status}")
            elif re.search(r'aria-label="Breadcrumb', page.text, re.I) and 'href="/docs"' in page.text:
                crumbs += 1
        record("docs: every page loads", not bad, ", ".join(bad) or f"{len(slugs)} pages")
        record("docs: pages carry breadcrumbs back to /docs", slugs and crumbs == len(slugs),
               f"{crumbs}/{len(slugs)}")
        api = site.get("/docs/api")
        record("docs: API docs", api.status == 200, f"{api.status}")

    # What works today and the roadmap, from one registry (#11122).
    if want("promises"):
        promises = site.get("/promises")
        proofs = promises.text.count("Proof:")
        record("promises: lists what works with its proof",
               promises.status == 200 and proofs > 0 and 'href="/roadmap"' in promises.text,
               f"{promises.status}, {proofs} items")
        roadmap = site.get("/roadmap")
        issues = len(set(re.findall(r'href="https://github.com/OpenAgentsInc/openagents/issues/(\d+)"',
                                    roadmap.text)))
        record("roadmap: lists what's next, each linked to its issue",
               roadmap.status == 200 and issues > 0 and 'href="/promises"' in roadmap.text,
               f"{roadmap.status}, {issues} issues")
        twins = [p for p in ("/promises.md", "/roadmap.md")
                 if not site.get(p).text.lstrip().startswith("---")]
        record("promises: Markdown twins", not twins, ", ".join(twins))

    # Download: Coder (terminal) one-liners first; desktop downloads hidden.
    if want("download"):
        dl = site.get("/download")
        body = dl.text
        sh_line = "/cli/install.sh" in body and "curl" in body
        ps_line = "/cli/install.ps1" in body
        record("download: loads", dl.status == 200, f"{dl.status}")
        record("download: Coder one-liners for macOS/Linux and Windows", sh_line and ps_line)
        first_install = body.find("/cli/install.sh")
        desktop_links = re.findall(r'href="[^"]*(?:\.dmg|\.msi|\.AppImage|\.deb|desktop/[^"]*)"', body)
        record("download: Coder comes before any other download",
               first_install >= 0 and all(body.find(l) > first_install for l in desktop_links))
        record("download: desktop downloads hidden", not desktop_links,
               ", ".join(desktop_links[:3]))

    # Agent-ready documents.
    if want("agent"):
        sec = site.get("/.well-known/security.txt")
        record("security.txt", sec.status == 200 and "Contact:" in sec.text and "Expires:" in sec.text)
        llms = site.get("/llms.txt")
        record("llms.txt", llms.status == 200 and llms.text.lstrip().startswith("#"))
        robots = site.get("/robots.txt")
        record("robots.txt", robots.status == 200 and "Sitemap:" in robots.text)
        sm = site.get("/sitemap.xml")
        record("sitemap.xml", sm.status == 200 and "<urlset" in sm.text,
               f"{sm.text.count('<loc>')} urls")
        oa = site.get("/openapi.json")
        try:
            ok = oa.status == 200 and "openapi" in oa.json()
        except ValueError:
            ok = False
        record("openapi.json", ok, f"{oa.status}")
        cat = site.get("/.well-known/ai-catalog.json")
        try:
            ok = cat.status == 200 and isinstance(cat.json(), dict)
        except ValueError:
            ok = False
        record("ai-catalog.json", ok, f"{cat.status}")
        init = site.request("/mcp/docs", method="POST", body={
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                       "clientInfo": {"name": "smoke", "version": "1"}}})
        tools = site.request("/mcp/docs", method="POST", body={
            "jsonrpc": "2.0", "id": 2, "method": "tools/list"})
        try:
            ok = init.status == 200 and "result" in init.json() and tools.json()["result"]["tools"]
        except (ValueError, KeyError, TypeError):
            ok = False
        record("mcp/docs: initialize and tools/list", bool(ok), f"{init.status}/{tools.status}")

    if production and (want("github") or want("accounts")):
        record("sign-in and accounts", None, "production: no account service yet (#11127)")

    # GitHub sign-in, as far as a script can go.
    if want("github") and not production:
        login = site.get("/login")
        record("sign-in: /login offers GitHub", login.status == 200 and "/auth/github" in login.text)
        go = site.get("/auth/github")
        loc = go.location()
        q = urllib.parse.parse_qs(urllib.parse.urlparse(loc).query)
        redirect = (q.get("redirect_uri") or [""])[0]
        record("sign-in: /auth/github goes to GitHub with this site's callback",
               go.status in (302, 303) and loc.startswith("https://github.com/login/oauth/authorize")
               and redirect == f"{base}/auth/github/callback",
               f"client {(q.get('client_id') or ['?'])[0]}")
        gh = site.get(loc) if loc.startswith("https://github.com/") else None
        record("sign-in: GitHub answers the authorize request", gh is not None and gh.status in (200, 302),
               "a person approves on github.com; the callback is not scriptable")
        cb = site.get("/auth/github/callback?code=x&state=y")
        record("sign-in: a forged callback is refused", cb.status in (302, 303, 400, 403),
               f"{cb.status}")

    # Signed-out gates.
    if want("gates"):
        dev = site.get("/device")
        record("device: signed out goes to log in", dev.status == 303 and "/login" in dev.location())
        proj = site.get("/projects")
        record("projects: signed out goes to log in", proj.status == 303 and "/login" in proj.location())
        # Environments run on the local address only: a public page never
        # links them, and the path answers plainly instead of erroring.
        links = [p for p in ("/", "/docs", "/download", "/login")
                 if re.search(r'href="/environments[/"]', site.get(p).text)]
        record("environments: no link on the public pages", not links, ", ".join(links))
        env = site.get("/environments")
        record("environments: answers plainly on a public host",
               env.status in (302, 303, 403, 404) and len(env.body) < 2000,
               f"{env.status} {env.location() or plain(env.text)[:40]}")

    # Accounts: open sign-up is refused; GitHub sign-in is the way in. The
    # signed-in checks use one test account made with the staging operator
    # token (SMOKE_SIGNUP_TOKEN), when it is set.
    session = key = None
    account = Site(base)
    if not production and (want("accounts") or want("signed-in") or want("gateway") or want("traces")):
        open_ = Site(base).request("/api/v1/accounts", method="POST", body={"label": "Smoke"})
        try:
            code = open_.json().get("error", {}).get("code", "")
        except (ValueError, AttributeError):
            code = ""
        record("accounts: open sign-up is refused", open_.status == 403 and code == "signup_disabled",
               f"{open_.status} {code}")
        token = os.environ.get("SMOKE_SIGNUP_TOKEN", "")
        if token:
            made = account.request("/api/v1/accounts", method="POST", body={"label": "Smoke"},
                                   headers={"Authorization": f"Bearer {token}"})
            try:
                made_json = made.json()
                session, key = made_json["session_token"], made_json["key_token"]
            except (ValueError, KeyError):
                pass
            record("accounts: the operator test account is made", bool(session), f"{made.status}")
        else:
            record("accounts: the operator test account is made", None, "SMOKE_SIGNUP_TOKEN unset")
    if want("signed-in") or want("gateway") or want("traces"):
        if session and want("signed-in"):
            account.cookies["oa_cloud_session"] = session
            home = account.get("/")
            record("signed in: composer selectors render",
                   'id="composer-row"' in home.text and "/composer/row/project" in home.text)
            keys = account.follow("/settings/api-keys")
            record("signed in: Settings > API keys", keys.status == 200 and "API key" in keys.text,
                   f"{keys.status}")
            settings = account.follow("/settings")
            record("signed in: Settings", settings.status == 200, f"{settings.status}")
            proj = account.follow("/projects")
            record("signed in: Projects", proj.status == 200, f"{proj.status}")
            listing = account.get("/projects?page=2")
            record("signed in: Projects repository list pages",
                   None if "Connect GitHub" in proj.text else listing.status == 200,
                   "needs a GitHub-connected account" if "Connect GitHub" in proj.text else "")
            device = account.get("/device")
            record("signed in: /device code page", device.status == 200 and "code" in device.text.lower())
            env_link = bool(re.search(r'href="/environments[/"]', home.text))
            record("signed in: no Environments link on the public host", not env_link)
        if want("gateway"):
            gw = Site(base)
            models = gw.get("/api/v1/models")
            try:
                ids = [m["id"] for m in models.json()["data"]]
            except (ValueError, KeyError):
                ids = []
            record("gateway: /v1/models", models.status == 200 and ids, f"{len(ids)} models")
            if key:
                resp = gw.request("/api/v1/responses", method="POST",
                                  body={"model": "google/gemini-2.5-flash-lite",
                                        "input": "Reply with the word OK.", "max_output_tokens": 32},
                                  headers={"Authorization": f"Bearer {key}"})
                try:
                    out = resp.json()
                    text = json.dumps(out.get("output", ""))
                    ok = resp.status == 200 and out.get("status") in ("completed", None) and "OK" in text.upper()
                except ValueError:
                    ok, out = False, {}
                record("gateway: one /v1/responses call on the free model", ok,
                       f"{resp.status} {(out.get('error') or {}).get('code', '') if isinstance(out, dict) else ''}")
            else:
                record("gateway: one /v1/responses call on the free model", None, "no test account")
        if want("traces"):
            traces(base, session)

    # The terminal: the hosted installer, into a scratch HOME.
    if want("terminal"):
        script = site.get("/cli/install.sh")
        record("terminal: /cli/install.sh", script.status == 200 and script.text.startswith("#!"),
               f"{script.status}")
        if install and script.status == 200:
            with tempfile.TemporaryDirectory(prefix="oa-smoke-") as home:
                path = os.path.join(home, "install.sh")
                with open(path, "wb") as f:
                    f.write(script.body)
                env = {"HOME": home, "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
                       "CODER_NO_PATH_UPDATE": "1", "CODER_BIN_DIR": os.path.join(home, "bin")}
                run_ = subprocess.run(["sh", path], env=env, capture_output=True, text=True, timeout=600)
                coder = os.path.join(home, "bin", "coder")
                ok = run_.returncode == 0 and os.path.exists(coder)
                record("terminal: installer installs into a scratch HOME", ok,
                       "" if ok else (run_.stderr.strip().splitlines() or ["?"])[-1][:120])
                if ok:
                    v = subprocess.run([coder, "--version"], env=env, capture_output=True, text=True, timeout=60)
                    record("terminal: coder --version", v.returncode == 0,
                           (v.stdout or v.stderr).strip().splitlines()[0][:80] if (v.stdout or v.stderr) else "")
        elif not install:
            record("terminal: installer run", None, "--no-install")

    # Durable accounts: only with --restart, since it forces a new revision.
    if restart:
        durable(base, os.environ.get("SMOKE_SIGNUP_TOKEN", ""), *restart)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("base", nargs="?", default=os.environ.get("SMOKE_BASE", DEFAULT_BASE))
    parser.add_argument("--no-install", action="store_true")
    parser.add_argument("--only", default="")
    parser.add_argument("--production", action="store_true",
                        help="one question, no accounts, no sign-in checks")
    parser.add_argument("--restart", action="store_true",
                        help="also check that an account, its session, an API key, a saved provider "
                        "key and a saved own-Claude key survive a forced new revision of "
                        "--service (it replaces the running instance)")
    parser.add_argument("--service", default="openagents-web-1-staging")
    parser.add_argument("--region", default="us-central1")
    parser.add_argument("--project", default="openagentsgemini")
    args = parser.parse_args()
    only = [o for o in args.only.split(",") if o]
    print(f"Smoke: {args.base}", flush=True)
    started = time.time()
    try:
        restart = (args.service, args.region, args.project) if args.restart else None
        run(args.base.rstrip("/"), only, not args.no_install, args.production, restart)
    except Exception as e:  # a crash is a failure, never a silent pass
        record("suite: ran to the end", False, repr(e)[:200])
    passed = sum(1 for r in RESULTS if r[0] == "PASS")
    failed = sum(1 for r in RESULTS if r[0] == "FAIL")
    skipped = sum(1 for r in RESULTS if r[0] == "SKIP")
    print(f"\n{passed} passed, {failed} failed, {skipped} skipped in {time.time() - started:.0f} s")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""The MCP OAuth sign-in on a running site, end to end (#11084):

    python3 -I scripts/smoke/oauth.py BASE_URL [--session-env NAME] [--mcp]

1. The three metadata documents (RFC 9728 origin and /mcp forms, RFC 8414).
2. Dynamic client registration (RFC 7591) for a loopback public client.
3. /oauth/authorize while signed out sends the browser to sign in.
4. With a session (the environment variable --session-env names, holding an
   `oa_cloud_session` value; staging's operator-made test account): the
   consent page, Approve, the code back at the redirect address with `iss`
   and `state`, a wrong PKCE verifier refused, the right one redeemed for a
   bearer, and that bearer answering GET /api/v1/account.
5. --mcp: /mcp with a made-up `sess_` bearer answers 401 with
   WWW-Authenticate naming the metadata; with the issued bearer (step 4),
   `tools/list` answers.

One PASS/FAIL/SKIP line per check; exit 1 when any check fails. Nothing
secret is printed.
"""

import argparse
import base64
import hashlib
import html
import json
import os
import re
import secrets
import sys
import urllib.error
import urllib.parse
import urllib.request

FAILED = []


def record(name, ok, detail=""):
    word = "SKIP" if ok is None else ("PASS" if ok else "FAIL")
    if ok is False:
        FAILED.append(name)
    print(f"{word} {name}" + (f" ({detail})" if detail else ""))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


def call(url, method="GET", headers=None, body=None):
    request = urllib.request.Request(url, method=method, headers=headers or {}, data=body)
    try:
        with OPENER.open(request, timeout=30) as response:
            return response.status, dict(response.headers), response.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as error:
        return error.code, dict(error.headers), error.read().decode("utf-8", "replace")


def as_json(text):
    try:
        return json.loads(text)
    except ValueError:
        return {}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("base")
    parser.add_argument("--session-env", help="environment variable holding an oa_cloud_session")
    parser.add_argument("--mcp", action="store_true", help="also check /mcp")
    args = parser.parse_args()
    base = args.base.rstrip("/")
    origin = None

    # 1. Metadata.
    status, _, text = call(f"{base}/.well-known/oauth-authorization-server")
    server = as_json(text)
    origin = server.get("issuer")
    record("metadata: authorization server", status == 200 and bool(origin)
           and server.get("code_challenge_methods_supported") == ["S256"]
           and bool(server.get("registration_endpoint")), f"{status} issuer={origin}")
    for path in ("/.well-known/oauth-protected-resource", "/.well-known/oauth-protected-resource/mcp"):
        status, _, text = call(base + path)
        doc = as_json(text)
        record(f"metadata: {path}", status == 200 and origin in (doc.get("authorization_servers") or []),
               f"{status} resource={doc.get('resource')}")

    # 2. Registration.
    redirect = "http://127.0.0.1:33418/callback"
    status, _, text = call(f"{base}/oauth/register", "POST", {"Content-Type": "application/json"},
                           json.dumps({"client_name": "OpenAgents OAuth smoke",
                                       "redirect_uris": [redirect],
                                       "token_endpoint_auth_method": "none",
                                       "grant_types": ["authorization_code"],
                                       "response_types": ["code"]}).encode())
    client_id = as_json(text).get("client_id")
    record("register: a public loopback client", status == 201 and bool(client_id), f"{status}")
    if not client_id:
        return
    status, _, text = call(f"{base}/oauth/register", "POST", {"Content-Type": "application/json"},
                           json.dumps({"redirect_uris": ["http://evil.example/cb"]}).encode())
    record("register: a plain-http non-loopback redirect is refused", status == 400, f"{status}")

    # 3. Authorize, signed out.
    verifier = secrets.token_urlsafe(48)
    challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
    state = secrets.token_urlsafe(12)
    query = urllib.parse.urlencode({
        "response_type": "code", "client_id": client_id, "redirect_uri": redirect,
        "code_challenge": challenge, "code_challenge_method": "S256", "state": state,
        "resource": f"{origin}/mcp", "scope": "account"})
    status, headers, _ = call(f"{base}/oauth/authorize?{query}")
    location = headers.get("Location") or headers.get("location") or ""
    record("authorize: signed out goes to sign in", status in (302, 303, 307) and "/login" in location,
           f"{status} -> {location.split('?')[0]}")

    # 4. Signed in: consent, approve, token.
    session = os.environ.get(args.session_env or "", "")
    token = None
    if not session:
        record("authorize: consent, approve, PKCE token", None, "no session given")
    else:
        cookie = {"Cookie": f"oa_cloud_session={session}"}
        status, headers, page = call(f"{base}/oauth/authorize?{query}", headers=cookie)
        set_cookie = headers.get("Set-Cookie") or headers.get("set-cookie") or ""
        # The consent form's own ticket, not the page's sign-out form's.
        consent = re.search(r'<form[^>]*action="/oauth/authorize".*?</form>', page, re.S)
        consent = consent.group(0) if consent else ""
        csrf = re.search(r'name="csrf" value="([^"]*)"', consent)
        req = re.search(r'name="request" value="([^"]*)"', consent)
        record("authorize: the consent page names the app",
               status == 200 and "OpenAgents OAuth smoke" in page and bool(csrf), f"{status}")
        if csrf and req:
            extra = "; ".join(p.split(";")[0] for p in re.split(r",(?=\s*[A-Za-z_]+=)", set_cookie) if p.strip())
            headers2 = {"Content-Type": "application/x-www-form-urlencoded",
                        "Cookie": cookie["Cookie"] + (f"; {extra}" if extra else ""),
                        "Origin": origin}
            form = urllib.parse.urlencode({"csrf": html.unescape(csrf.group(1)),
                                           "request": html.unescape(req.group(1)),
                                           "decision": "approve"}).encode()
            status, headers, answer = call(f"{base}/oauth/authorize", "POST", headers2, form)
            if status >= 400:
                found = re.search(r"<h1>(.*?)</h1>(.*?)</p>", answer, re.S)
                print("  approve answered:", re.sub("<[^>]+>", " ", found.group(0)) if found else answer[:200])
            back = headers.get("Location") or headers.get("location") or ""
            params = dict(urllib.parse.parse_qsl(urllib.parse.urlparse(back).query))
            code = params.get("code")
            record("authorize: Approve sends the code back with state and iss",
                   back.startswith(redirect) and bool(code) and params.get("state") == state
                   and params.get("iss") == origin, f"{status}")
            if code:
                def exchange(v):
                    return call(f"{base}/oauth/token", "POST",
                                {"Content-Type": "application/x-www-form-urlencoded"},
                                urllib.parse.urlencode({"grant_type": "authorization_code", "code": code,
                                                        "redirect_uri": redirect, "client_id": client_id,
                                                        "code_verifier": v,
                                                        "resource": f"{origin}/mcp"}).encode())
                status, _, text = exchange(secrets.token_urlsafe(48))
                record("token: a wrong PKCE verifier is refused",
                       status == 400 and as_json(text).get("error") == "invalid_grant", f"{status}")
                status, _, text = exchange(verifier)
                body = as_json(text)
                token = body.get("access_token")
                record("token: the PKCE exchange issues a bearer",
                       status == 200 and bool(token) and body.get("token_type") == "Bearer",
                       f"{status} expires_in={body.get('expires_in')}")
                status, _, _ = exchange(verifier)
                record("token: the code is single-use", status == 400, f"{status}")
                if token:
                    status, _, _ = call(f"{base}/api/v1/account",
                                        headers={"Authorization": f"Bearer {token}"})
                    record("token: the bearer is a live account session", status == 200, f"{status}")

    # 5. /mcp.
    if args.mcp:
        rpc = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).encode()
        mcp_headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        status, headers, _ = call(f"{base}/mcp", "POST",
                                  dict(mcp_headers, Authorization="Bearer sess_" + "0" * 64), rpc)
        challenge_header = headers.get("WWW-Authenticate") or headers.get("www-authenticate") or ""
        record("mcp: a bad bearer gets 401 with WWW-Authenticate",
               status == 401 and "oauth-protected-resource" in challenge_header, f"{status}")
        if token:
            status, _, text = call(f"{base}/mcp", "POST", dict(mcp_headers, Authorization=f"Bearer {token}"), rpc)
            record("mcp: tools/list with the issued bearer", status == 200 and '"tools"' in text, f"{status}")
        else:
            record("mcp: tools/list with the issued bearer", None, "no bearer issued")

    print(f"{len(FAILED)} failed")
    sys.exit(1 if FAILED else 0)


if __name__ == "__main__":
    main()

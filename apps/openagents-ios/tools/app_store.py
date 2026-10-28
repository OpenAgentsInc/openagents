#!/usr/bin/env python3
"""OpenAgents App Store Connect operations. Credentials come only from ASC_API_*
environment variables. No credentials/JWTs are printed. This tool targets only
com.openagents.app; beta assignment uses an existing internal group.
"""
import argparse, base64, hashlib, json, os, subprocess, sys, time, urllib.error, urllib.parse, urllib.request


APP_ID = None
BUNDLE = "com.openagents.app"
API = "https://api.appstoreconnect.apple.com"

def b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def token():
    for name in ("ASC_API_KEY_ID", "ASC_API_ISSUER_ID", "ASC_API_PRIVATE_KEY_PATH"):
        if not os.environ.get(name):
            sys.exit(f"asc: {name} is not set")
    now = int(time.time())
    head = {"alg": "ES256", "kid": os.environ["ASC_API_KEY_ID"], "typ": "JWT"}
    claims = {"iss": os.environ["ASC_API_ISSUER_ID"], "iat": now, "exp": now + 1100, "aud": "appstoreconnect-v1"}
    signing = (b64(json.dumps(head).encode()) + "." + b64(json.dumps(claims).encode())).encode()
    der = subprocess.run(["/usr/bin/openssl", "dgst", "-sha256", "-sign", os.environ["ASC_API_PRIVATE_KEY_PATH"]], input=signing, capture_output=True)
    if der.returncode:
        sys.exit("asc: openssl could not sign with the key")
    return signing.decode() + "." + b64(raw(der.stdout))


def raw(der):
    def length(d, at):
        f = d[at]
        if f < 0x80:
            return f, at + 1
        n = f & 0x7F
        return int.from_bytes(d[at + 1:at + 1 + n], "big"), at + 1 + n
    _, at = length(der, 1)
    out = b""
    for _ in range(2):
        n, at = length(der, at + 1)
        out += der[at:at + n].lstrip(b"\0").rjust(32, b"\0")
        at += n
    return out


def call(method, path, body=None, params=None):
    url = API + path + ("?" + urllib.parse.urlencode(params) if params else "")
    req = urllib.request.Request(url, method=method, data=json.dumps(body).encode() if body is not None else None)
    req.add_header("Authorization", "Bearer " + token())
    req.add_header("Content-Type", "application/json")
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            text = r.read()
            return json.loads(text) if text else {}
    except urllib.error.HTTPError as e:
        sys.exit(f"asc: {method} {path} -> {e.code} {e.read().decode()[:800]}")


def try_call(method, path, body):
    req = urllib.request.Request(API + path, method=method, data=json.dumps(body).encode())
    req.add_header("Authorization", "Bearer " + token())
    req.add_header("Content-Type", "application/json")
    try:
        urllib.request.urlopen(req, timeout=60)
        return True
    except urllib.error.HTTPError as e:
        if e.code == 409:
            return False
        sys.exit(f"asc: {method} {path} -> {e.code} {e.read().decode()[:800]}")


def get(path, **params):
    return call("GET", path, params=params or None)


def bundle_id():
    data = get("/v1/bundleIds", **{"filter[identifier]": BUNDLE})["data"]
    match = [b for b in data if b["attributes"]["identifier"] == BUNDLE]
    if not match:
        sys.exit("asc: bundle ID not found")
    return match[0]["id"]


def profile(cert, name, out):
    existing = get("/v1/profiles", **{"filter[name]": name})["data"]
    live = [p for p in existing if p["attributes"]["profileState"] == "ACTIVE"]
    if live:
        content = live[0]["attributes"]["profileContent"]
    else:
        body = {"data": {"type": "profiles", "attributes": {"name": name, "profileType": "IOS_APP_STORE"},
                         "relationships": {"bundleId": {"data": {"type": "bundleIds", "id": bundle_id()}},
                                           "certificates": {"data": [{"type": "certificates", "id": cert}]}}}}
        content = call("POST", "/v1/profiles", body)["data"]["attributes"]["profileContent"]
    with open(out, "wb") as f:
        f.write(base64.b64decode(content))
    print("profile", name, "->", out, "(existing)" if live else "(created)")


def app_id():
    global APP_ID
    if not APP_ID:
        data = get("/v1/apps", **{"filter[bundleId]": BUNDLE})["data"]
        match = [a for a in data if a["attributes"]["bundleId"] == BUNDLE]
        if not match:
            sys.exit("asc: no App Store Connect record for " + BUNDLE)
        APP_ID = match[0]["id"]
    return APP_ID



def find_build(number, version):
    rows = get("/v1/builds", **{"filter[app]": app_id(), "filter[version]": str(number),
               "filter[preReleaseVersion.version]": version, "limit": "10"})["data"]
    return rows[0] if rows else None


def main():
    p = argparse.ArgumentParser(description=__doc__)
    commands = p.add_subparsers(dest="command", required=True)
    cert = commands.add_parser("cert-sha1"); cert.add_argument("id")
    profile_command = commands.add_parser("profile")
    profile_command.add_argument("--cert", required=True)
    profile_command.add_argument("--name", default="OpenAgents App Store")
    profile_command.add_argument("--out", required=True)
    state = commands.add_parser("status"); state.add_argument("--build", required=True)
    state.add_argument("--version", default="1.0.0")
    beta = commands.add_parser("testflight"); beta.add_argument("--build", required=True)
    beta.add_argument("--version", default="1.0.0"); beta.add_argument("--group", default="Owner")
    beta.add_argument("--notes-file", type=argparse.FileType("r"), required=True)
    a = p.parse_args()
    if a.command == "cert-sha1":
        content = get(f"/v1/certificates/{a.id}")["data"]["attributes"]["certificateContent"]
        print(hashlib.sha1(base64.b64decode(content)).hexdigest().upper())
    elif a.command == "profile":
        profile(a.cert, a.name, a.out)
    else:
        build = find_build(a.build, a.version)
        if not build:
            if a.command == "testflight": raise SystemExit("Build not yet listed")
            print("NOT_YET_LISTED"); return
        bid = build["id"]; state = build["attributes"]["processingState"]
        print(f"OpenAgents {a.version} ({a.build}): {state}")
        if a.command == "status":
            if state == "VALID":
                detail = get(f"/v1/builds/{bid}/buildBetaDetail")["data"]["attributes"]
                print("Internal:", detail.get("internalBuildState"))
            return
        if state != "VALID":
            raise SystemExit("Build must finish processing before beta assignment")
        groups = get(f"/v1/apps/{app_id()}/betaGroups", limit="50")["data"]
        group = next((g for g in groups if g["attributes"]["name"] == a.group), None)
        if not group or not group["attributes"].get("isInternalGroup"):
            raise SystemExit("An existing internal beta group is required")
        notes = a.notes_file.read(4001)
        if len(notes) > 4000: raise SystemExit("Beta notes exceed 4000 characters")
        localizations = get(f"/v1/builds/{bid}/betaBuildLocalizations")["data"]
        english = next((item for item in localizations if item["attributes"]["locale"] == "en-US"), None)
        if english:
            call("PATCH", f"/v1/betaBuildLocalizations/{english['id']}", {"data": {
                "type": "betaBuildLocalizations", "id": english["id"], "attributes": {"whatsNew": notes}}})
        else:
            call("POST", "/v1/betaBuildLocalizations", {"data": {"type": "betaBuildLocalizations",
                "attributes": {"locale": "en-US", "whatsNew": notes},
                "relationships": {"build": {"data": {"type": "builds", "id": bid}}}}})
        call("POST", f"/v1/betaGroups/{group['id']}/relationships/builds", {"data": [{"type": "builds", "id": bid}]})
        assigned = get(f"/v1/builds/{bid}/betaGroups", limit="50")["data"]
        if group['id'] not in {item['id'] for item in assigned}:
            raise SystemExit("Could not verify beta group assignment")
        print("Verified build in", a.group)
        detail = get(f"/v1/builds/{bid}/buildBetaDetail")["data"]["attributes"]
        print("Internal:", detail.get("internalBuildState"))


if __name__ == "__main__": main()

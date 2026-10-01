#!/usr/bin/env python3
"""App Store Connect reads for shipping the OpenAgents iOS app (#10118).

    asc.py builds [--version 1.0.0]        the version's builds, newest first, as JSON lines
    asc.py next-build [--version 1.0.0] [--at-least N]
                                           one more than the highest build number used
    asc.py wait-valid --build N [--version 1.0.0] [--after ISO] [--timeout SECONDS]
                                           waits until build N (uploaded after ISO) is
                                           VALID and in Internal Testers

The key comes from ASC_API_KEY_ID, ASC_API_ISSUER_ID, and
ASC_API_PRIVATE_KEY_PATH, which `testflight.sh` loads from the owner's env
file. The key and the token are never printed.

Builds are always matched by their pre-release version (the marketing
version) and, when waiting, by an upload time after the upload started:
the app has older builds with the same numbers under other versions.
"""

import argparse
import base64
import datetime
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

APP_ID = "6748620735"
API = "https://api.appstoreconnect.apple.com"


def b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def token():
    """An ES256 App Store Connect token, signed with openssl so nothing
    beyond the standard library and the system's openssl is needed (a
    host's commands may run with another HOME, without the owner's Python
    packages)."""
    for name in ("ASC_API_KEY_ID", "ASC_API_ISSUER_ID", "ASC_API_PRIVATE_KEY_PATH"):
        if not os.environ.get(name):
            sys.exit(f"asc: {name} is not set")
    now = int(time.time())
    header = {"alg": "ES256", "kid": os.environ["ASC_API_KEY_ID"], "typ": "JWT"}
    claims = {"iss": os.environ["ASC_API_ISSUER_ID"], "iat": now, "exp": now + 1200, "aud": "appstoreconnect-v1"}
    signing = (b64(json.dumps(header).encode()) + "." + b64(json.dumps(claims).encode())).encode()
    signed = subprocess.run(
        ["/usr/bin/openssl", "dgst", "-sha256", "-sign", os.environ["ASC_API_PRIVATE_KEY_PATH"]],
        input=signing,
        capture_output=True,
    )
    if signed.returncode != 0:
        sys.exit("asc: openssl could not sign with the App Store Connect key")
    return signing.decode() + "." + b64(raw_signature(signed.stdout))


def raw_signature(der):
    """A DER ECDSA signature (SEQUENCE of two INTEGERs) as JWS wants it:
    r and s, 32 bytes each."""
    def length(data, at):
        first = data[at]
        if first < 0x80:
            return first, at + 1
        count = first & 0x7F
        return int.from_bytes(data[at + 1:at + 1 + count], "big"), at + 1 + count

    if der[0] != 0x30:
        sys.exit("asc: openssl returned no ECDSA signature")
    _, at = length(der, 1)
    parts = []
    for _ in range(2):
        if der[at] != 0x02:
            sys.exit("asc: openssl returned no ECDSA signature")
        size, at = length(der, at + 1)
        parts.append(int.from_bytes(der[at:at + size], "big").to_bytes(32, "big"))
        at += size
    return parts[0] + parts[1]


def get(path, params=None):
    url = API + path
    if params:
        url += "?" + urllib.parse.urlencode(params)
    request = urllib.request.Request(url, headers={"Authorization": "Bearer " + token()})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(request, timeout=60) as reply:
                return json.load(reply)
        except urllib.error.HTTPError as error:
            if error.code < 500 or attempt == 3:
                sys.exit(f"asc: {path} answered {error.code}")
        except (urllib.error.URLError, TimeoutError) as error:
            if attempt == 3:
                sys.exit(f"asc: {path} failed: {error}")
        time.sleep(5 * (attempt + 1))
    return {}


def builds(version):
    reply = get(
        "/v1/builds",
        {
            "filter[app]": APP_ID,
            "filter[preReleaseVersion.version]": version,
            "sort": "-uploadedDate",
            "limit": "200",
            "fields[builds]": "version,uploadedDate,processingState",
        },
    )
    return [
        {
            "id": row["id"],
            "build": row["attributes"]["version"],
            "uploaded": row["attributes"].get("uploadedDate"),
            "state": row["attributes"].get("processingState"),
        }
        for row in reply.get("data", [])
    ]


def internal_state(build_id):
    reply = get(f"/v1/builds/{build_id}/buildBetaDetail")
    return reply.get("data", {}).get("attributes", {}).get("internalBuildState")


def internal_groups():
    reply = get(
        f"/v1/apps/{APP_ID}/betaGroups",
        {"fields[betaGroups]": "name,isInternalGroup,hasAccessToAllBuilds", "limit": "50"},
    )
    return [
        row["attributes"]
        for row in reply.get("data", [])
        if row["attributes"].get("isInternalGroup")
    ]


def parse_time(text):
    return datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=["builds", "next-build", "wait-valid"])
    parser.add_argument("--version", default="1.0.0")
    parser.add_argument("--build")
    parser.add_argument("--at-least", type=int, default=1)
    parser.add_argument("--after")
    parser.add_argument("--timeout", type=int, default=3600)
    args = parser.parse_args()

    if args.command == "builds":
        for row in builds(args.version):
            print(json.dumps(row))
    elif args.command == "next-build":
        used = [int(row["build"]) for row in builds(args.version) if row["build"].isdigit()]
        print(max([args.at_least - 1] + used) + 1)
    else:
        if not args.build:
            sys.exit("asc: wait-valid needs --build")
        after = parse_time(args.after) if args.after else None
        deadline = time.time() + args.timeout
        said = None
        while True:
            match = None
            for row in builds(args.version):
                if row["build"] != args.build or not row["uploaded"]:
                    continue
                if after and parse_time(row["uploaded"]) < after:
                    continue
                match = row
                break
            state = match["state"] if match else "not in App Store Connect yet"
            if state != said:
                print(f"Build {args.build}: {state.lower().replace('_', ' ')}", flush=True)
                said = state
            if match and match["state"] in ("FAILED", "INVALID"):
                sys.exit(f"asc: build {args.build} is {match['state']}")
            if match and match["state"] == "VALID":
                internal = internal_state(match["id"])
                groups = [g["name"] for g in internal_groups() if g.get("hasAccessToAllBuilds")]
                print(json.dumps({
                    "build": args.build,
                    "version": args.version,
                    "uploaded": match["uploaded"],
                    "state": match["state"],
                    "internal": internal,
                    "groups": groups,
                }))
                if internal not in ("IN_BETA_TESTING", "READY_FOR_BETA_TESTING"):
                    sys.exit(f"asc: build {args.build} is VALID but its internal testing state is {internal}")
                return
            if time.time() > deadline:
                sys.exit(f"asc: build {args.build} was not VALID within {args.timeout} seconds")
            time.sleep(30)


if __name__ == "__main__":
    main()

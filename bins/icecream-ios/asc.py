#!/usr/bin/env python3
"""App Store Connect steps for The Ice Cream Man iOS app.

    asc.py register          registers the bundle ID com.openagents.icecreamman
                             (no-op when it is already registered)
    asc.py app               prints the App Store app record's id, or exits 3
                             when the owner has not created it yet
    asc.py profile           makes (or reuses) the "The Ice Cream Man App Store"
                             provisioning profile and installs it locally
    asc.py next-build        one more than the highest build number uploaded
    asc.py wait-valid --build N [--after ISO] [--timeout SECONDS]
                             waits until build N is VALID, then adds it to
                             every internal TestFlight group
    asc.py metadata          pushes metadata/ (description, keywords, subtitle,
                             promotional text, support URL, privacy URL) to the
                             editable App Store version and app info

The key comes from ASC_API_KEY_ID, ASC_API_ISSUER_ID, and
ASC_API_PRIVATE_KEY_PATH (build.sh loads them from ~/work/.secrets/
appstoreconnect.env). The key and token are never printed. It reuses the
token and HTTP helpers of scripts/release/asc.py.
"""

import argparse
import base64
import datetime
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "../../scripts/release"))
import asc as base  # noqa: E402

BUNDLE_ID = "com.openagents.icecreamman"
NAME = "The Ice Cream Man"
PROFILE = "The Ice Cream Man App Store"
VERSION = "1.0.0"
TEAM = "HQWSG26L43"
METADATA = os.path.join(HERE, "metadata")


def bundle_resource():
    reply = base.get("/v1/bundleIds", {"filter[identifier]": BUNDLE_ID, "limit": "5"})
    for row in reply.get("data", []):
        if row["attributes"]["identifier"] == BUNDLE_ID:
            return row
    return None


def register():
    row = bundle_resource()
    if row:
        print(f"registered already: {row['id']}")
        return row["id"]
    reply = base.send("POST", "/v1/bundleIds", {"data": {"type": "bundleIds", "attributes": {
        "identifier": BUNDLE_ID, "name": NAME, "platform": "IOS"}}})
    print(f"registered: {reply['data']['id']}")
    return reply["data"]["id"]


def app_id():
    reply = base.get("/v1/apps", {"filter[bundleId]": BUNDLE_ID, "limit": "5"})
    for row in reply.get("data", []):
        if row["attributes"]["bundleId"] == BUNDLE_ID:
            return row["id"]
    return None


def profile():
    bundle = register()
    reply = base.get("/v1/profiles", {"filter[name]": PROFILE, "limit": "10"})
    content = None
    for row in reply.get("data", []):
        attributes = row["attributes"]
        if attributes["name"] == PROFILE and attributes["profileState"] == "ACTIVE":
            content = attributes["profileContent"]
            break
    if content is None:
        certs = base.get("/v1/certificates", {"filter[certificateType]": "DISTRIBUTION,IOS_DISTRIBUTION", "limit": "50"})
        rows = [r for r in certs.get("data", []) if r["attributes"].get("expirationDate", "") > datetime.datetime.utcnow().isoformat()]
        if not rows:
            sys.exit("asc: no unexpired distribution certificate")
        rows.sort(key=lambda r: r["attributes"]["expirationDate"], reverse=True)
        made = base.send("POST", "/v1/profiles", {"data": {"type": "profiles",
            "attributes": {"name": PROFILE, "profileType": "IOS_APP_STORE"},
            "relationships": {
                "bundleId": {"data": {"type": "bundleIds", "id": bundle}},
                "certificates": {"data": [{"type": "certificates", "id": r["id"]} for r in rows]}}}})
        content = made["data"]["attributes"]["profileContent"]
        print(f"made profile {PROFILE}")
    raw = base64.b64decode(content)
    folder = os.path.expanduser("~/Library/MobileDevice/Provisioning Profiles")
    os.makedirs(folder, exist_ok=True)
    path = os.path.join(folder, "icecreamman-appstore.mobileprovision")
    with open(path, "wb") as handle:
        handle.write(raw)
    print(f"installed {path}")


def require_app():
    found = app_id()
    if not found:
        print("no App Store app record for " + BUNDLE_ID + " yet (see NEEDS_OWNER.md in the repository)")
        sys.exit(3)
    return found


def builds(app):
    reply = base.get("/v1/builds", {"filter[app]": app, "sort": "-uploadedDate", "limit": "50",
                                    "fields[builds]": "version,processingState,uploadedDate"})
    return reply.get("data", [])


def next_build():
    app = require_app()
    numbers = [int(r["attributes"]["version"]) for r in builds(app) if r["attributes"]["version"].isdigit()]
    print(max(numbers, default=0) + 1)


def wait_valid(number, after, timeout):
    app = require_app()
    deadline = time.time() + timeout
    while True:
        for row in builds(app):
            attributes = row["attributes"]
            if attributes["version"] != str(number):
                continue
            if after and attributes.get("uploadedDate", "") < after:
                continue
            state = attributes["processingState"]
            if state == "VALID":
                groups = base.get(f"/v1/apps/{app}/betaGroups", {"limit": "50"}).get("data", [])
                for group in groups:
                    if group["attributes"].get("isInternalGroup") and not group["attributes"].get("hasAccessToAllBuilds"):
                        base.send("POST", f"/v1/betaGroups/{group['id']}/relationships/builds",
                                  {"data": [{"type": "builds", "id": row["id"]}]})
                names = ", ".join(g["attributes"]["name"] for g in groups if g["attributes"].get("isInternalGroup"))
                print(f"build {number} VALID; internal groups: {names or 'none (owner adds one)'}")
                return
            if state in ("FAILED", "INVALID"):
                sys.exit(f"build {number} {state}")
        if time.time() > deadline:
            sys.exit(f"build {number} still processing")
        time.sleep(30)


def read(name):
    with open(os.path.join(METADATA, name), encoding="utf-8") as handle:
        return handle.read().strip()


def metadata():
    app = require_app()
    versions = base.get(f"/v1/apps/{app}/appStoreVersions", {"limit": "10"}).get("data", [])
    editable = [v for v in versions if v["attributes"]["appStoreState"] in
                ("PREPARE_FOR_SUBMISSION", "DEVELOPER_REJECTED", "REJECTED", "METADATA_REJECTED")]
    if not editable:
        sys.exit("asc: no editable App Store version")
    version = editable[0]["id"]
    localizations = base.get(f"/v1/appStoreVersions/{version}/appStoreVersionLocalizations").get("data", [])
    english = [l for l in localizations if l["attributes"]["locale"] == "en-US"]
    attributes = {"description": read("description.txt"), "keywords": read("keywords.txt"),
                  "promotionalText": read("promotional_text.txt"), "supportUrl": read("support_url.txt"),
                  "marketingUrl": read("marketing_url.txt")}
    if english:
        base.send("PATCH", f"/v1/appStoreVersionLocalizations/{english[0]['id']}", {"data": {
            "type": "appStoreVersionLocalizations", "id": english[0]["id"], "attributes": attributes}})
    infos = base.get(f"/v1/apps/{app}/appInfos").get("data", [])
    for info in infos:
        for row in base.get(f"/v1/appInfos/{info['id']}/appInfoLocalizations").get("data", []):
            if row["attributes"]["locale"] == "en-US":
                base.send("PATCH", f"/v1/appInfoLocalizations/{row['id']}", {"data": {
                    "type": "appInfoLocalizations", "id": row["id"], "attributes": {
                        "subtitle": read("subtitle.txt"), "privacyPolicyUrl": read("privacy_url.txt")}}})
    print("metadata pushed")


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("register")
    sub.add_parser("app")
    sub.add_parser("profile")
    sub.add_parser("next-build")
    sub.add_parser("metadata")
    wait = sub.add_parser("wait-valid")
    wait.add_argument("--build", type=int, required=True)
    wait.add_argument("--after")
    wait.add_argument("--timeout", type=int, default=3600)
    args = parser.parse_args()
    if args.command == "register":
        register()
    elif args.command == "app":
        print(require_app())
    elif args.command == "profile":
        profile()
    elif args.command == "next-build":
        next_build()
    elif args.command == "metadata":
        metadata()
    elif args.command == "wait-valid":
        wait_valid(args.build, args.after, args.timeout)


if __name__ == "__main__":
    main()

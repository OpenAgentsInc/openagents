#!/usr/bin/env python3
"""Check the public app-auth contract without reading chats or approving a login."""

import argparse
import json
import os
import urllib.error
import urllib.parse
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--api", default="https://api.openagents.com")
    parser.add_argument("--website", default="https://openagents.com")
    args = parser.parse_args()
    opener = urllib.request.build_opener(NoRedirect())

    def call(method, path, data=None, token=None):
        headers = {"Content-Type": "application/json"}
        # Staging shares its website host; only the separate API host ignores cookies.
        if args.api.rstrip("/") != args.website.rstrip("/"):
            headers["Cookie"] = "oa_cloud_session=invalid"
        if token:
            headers["Authorization"] = f"Bearer {token}"
        req = urllib.request.Request(
            args.api.rstrip("/") + path,
            data=json.dumps(data).encode() if data is not None else None,
            headers=headers,
            method=method,
        )
        try:
            result = opener.open(req, timeout=20)
        except urllib.error.HTTPError as error:
            result = error
        with result:
            raw = result.read(8 * 1024 * 1024 + 1)
            assert len(raw) <= 8 * 1024 * 1024, "Response exceeded the limit"
            assert not result.headers.get("Set-Cookie"), "API returned a cookie"
            return result.status, json.loads(raw)

    status, spec = call("GET", "/v1/openapi.json")
    assert status == 200
    for path in ("/v1/device/code", "/v1/device/token", "/v1/device/sign-out", "/v1/account"):
        assert path in spec["paths"], f"Missing public schema: {path}"
    assert "AppSession" in spec["components"]["schemas"]
    for path in ("/v1/session", "/v1/sessions/github", "/v1/sessions/device/decide"):
        assert path not in spec["paths"], "Internal auth operation was advertised"
    status, error = call("GET", "/v1/account")
    assert status == 401 and isinstance(error["error"], dict), "Unauthenticated account read was allowed"
    status, grant = call("POST", "/v1/device/code", {"app": "SDK contract check", "computer": "disposable-check"})
    assert status == 200 and grant["expires_in"] == 600
    assert grant["verification_uri"] == args.website.rstrip("/") + "/device"
    assert urllib.parse.urlsplit(grant["verification_uri_complete"]).netloc == urllib.parse.urlsplit(args.website).netloc
    status, pending = call("POST", "/v1/device/token", {"device_code": grant["device_code"]})
    assert status == 400 and pending["error"] == "authorization_pending"
    status, slow = call("POST", "/v1/device/token", {"device_code": grant["device_code"]})
    assert status == 400 and slow["error"] == "slow_down" and slow["interval"] == 10
    status, error = call("POST", "/v1/device/token", {"device_code": "invalid"})
    assert status == 400 and error["error"] == "invalid_grant"
    status, error = call("POST", "/v1/device/sign-out", {}, "invalid")
    assert status == 401 and error["error"] == "invalid_token"
    if token := os.environ.get("OPENAGENTS_TOKEN"):
        status, account = call("GET", "/v1/account", token=token)
        assert status == 200 and account["account"]["id"] and isinstance(account["workspaces"], list)
        print("PASS dedicated test-account bearer and workspace discovery")
    print("PASS public auth schema, unauthenticated refusal, pending, slowdown, and invalid credentials")
    if args.api.rstrip("/") != args.website.rstrip("/"):
        print("PASS API-host cookie isolation")
    print("Cleanup: unapproved device grant expires in 600 seconds; no account, session, or chat created")


if __name__ == "__main__":
    main()

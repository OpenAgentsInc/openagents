#!/usr/bin/env python3
"""Write docs/api/openapi.first-party.json from the gateway's audience table.

The gateway's test `audience::tests::the_committed_first_party_document_is_current`
holds the file to `crates/gateway/src/audience.rs` (`first_party_document`), and
can write it itself (`OPENAGENTS_WRITE_OPENAPI=1 cargo test -p gateway audience`).
This script makes the same document without a Rust build, and adds the
website's first-party routes ([`WEB`], marked `x-served-by: web`), which the
gateway's test leaves alone.

Usage: python3 scripts/api/first_party_openapi.py
"""

import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
TABLE = ROOT / "crates/gateway/src/audience.rs"
OUT = ROOT / "docs/api/openapi.first-party.json"

ROUTE = re.compile(r'^\s*route\("([^"]+)", (Public|FirstParty|Internal), "([^"]*)", &\[(.*)\]\),\s*$')
OP = re.compile(r'op\("(\w+)", "(\w+)", "((?:[^"\\]|\\.)*)"\)')

TAGS = {
    "Workspaces": "Workspaces, members, invitations, and single sign-on.",
    "Usage": "What the workspace spent, its balance, and top-ups.",
    "Sessions": "Signing in: sessions, device sign-in, recovery.",
    "GitHub": "Connected GitHub repositories.",
    "Projects": "Projects: one repository each.",
    "Billing": "Plans, card checkout, and purchase authority.",
    "Earnings": "What you earned and where it goes.",
    "Referrals": "Referral links and attribution.",
    "Skills": "The older skills directory.",
    "Reports": "Team reports.",
    "Computers": "Your computers and whether their chats sync.",
    "Threads": "Chats synced from the terminal.",
    "Traces": "Agent traces (ATIF).",
}


# The website's FIRST-PARTY API routes (crates/openagents-web), marked
# `x-served-by: web`: our apps call them on openagents.com with the app's
# session (`Authorization: Bearer sess_...`). Each older path (#11158)
# answers the same, with `Deprecation` and `Link` headers.
WEB = [
    ("/v1/device/code", "Sessions", [("post", "startAppSignIn", "An app asks to sign in on this computer (RFC 8628). Older path: `/device/code`.")]),
    ("/v1/device/token", "Sessions", [("post", "pollAppSignIn", "The app's poll: its session once the person approves. Older path: `/device/token`.")]),
    ("/v1/device/sign-out", "Sessions", [("post", "signOutThisApp", "The app signs its own session out. Older path: `/device/sign-out`.")]),
    ("/v1/threads", "Threads", [("get", "listThreads", "The account's chats (web, terminal, phone) and computers.")]),
    ("/v1/threads/{id}", "Threads", [("get", "getThread", "One chat and its newest messages.")]),
    ("/v1/threads/{id}/messages", "Threads", [("post", "replyToThread", "Reply in a chat.")]),
    ("/v1/threads/synced", "Threads", [("get", "listSyncedThreads", "Chats synced from Coder, and which were deleted on the website. Older path: `/coder/sessions`.")]),
    ("/v1/threads/synced/{session}", "Threads", [("put", "syncThread", "Save a Coder chat's messages. Older path: `/coder/sessions/{session}`."), ("delete", "forgetSyncedThread", "Deleted in Coder: remove it here.")]),
    ("/v1/threads/synced/{session}/status", "Threads", [("post", "setThreadWorking", "Coder is replying, or not.")]),
    ("/v1/threads/synced/{session}/replies", "Threads", [("post", "takeThreadReplies", "Take the replies sent on the website, each once.")]),
    ("/v1/computers", "Computers", [("get", "listComputers", "The account's computers and phones.")]),
    ("/v1/computers/check-in", "Computers", [("post", "checkInComputer", "Coder runs on this computer with sync on: chats waiting for it, and its sync choice. Older path: `/coder/check-in`.")]),
    ("/v1/computers/{name}/sync", "Computers", [("get", "getComputerSync", "The computer's sync choice. Older path: `/coder/sync?computer=`."), ("put", "setComputerSync", "Set the computer's sync choice.")]),
    ("/v1/computers/{name}/activity", "Computers", [("post", "reportComputerActivity", "What Coder runs on the computer; answers the commands waiting for it.")]),
    ("/v1/agents", "Computers", [("get", "listAgents", "What Coder runs on each computer.")]),
    ("/v1/agents/actions", "Computers", [("post", "actOnAgent", "Stop, approve, deny, or message one running item.")]),
    ("/v1/traces", "Traces", [("get", "listTraces", "The account's traces, newest first. Older path: `/api/traces`."), ("post", "uploadTrace", "Save an ATIF trace (`?share=true` to share it at once).")]),
    ("/v1/traces/{id}", "Traces", [("get", "getTrace", "The saved ATIF document."), ("delete", "deleteTrace", "Delete a trace and its agents.")]),
    ("/v1/traces/{id}/share", "Traces", [("post", "shareTrace", "Share or stop sharing a trace.")]),
    ("/v1/traces/{id}/agents", "Traces", [("get", "listTraceAgents", "A trace's agents, parents first."), ("post", "uploadTraceAgent", "Save an agent under a trace (`?parent=`).")]),
    ("/v1/traces/{id}/agents/{agent}", "Traces", [("get", "getTraceAgent", "One agent's ATIF document.")]),
]


def error_response(description):
    return {
        "description": description,
        "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ErrorBody"}}},
    }


def path_item(path, tag, ops):
    parameters = [
        {
            "name": name,
            "in": "path",
            "required": True,
            "description": f"The {name} id.",
            "schema": {"type": "string"},
        }
        for name in (seg[1:-1] for seg in path.split("/") if seg.startswith("{") and seg.endswith("}"))
    ]
    item = {}
    for method, op_id, summary in ops:
        operation = {
            "operationId": op_id,
            "summary": summary,
            "tags": [tag],
            "security": [{"bearer": []}],
            "responses": {
                "200": {"description": "Done.", "content": {"application/json": {"schema": {"type": "object"}}}},
                "400": error_response("The request isn't valid (`invalid_request`)."),
                "401": error_response("No key or session, or it was rejected (`authentication`)."),
                "403": error_response("Not allowed here (`permission`)."),
                "404": error_response("Nothing with that id here (`not_found`)."),
            },
        }
        if parameters:
            operation["parameters"] = parameters
        if method in ("post", "put", "patch"):
            operation["requestBody"] = {
                "required": False,
                "content": {"application/json": {"schema": {"type": "object"}}},
            }
        item[method] = operation
    return item


def main():
    text = TABLE.read_text()
    block = text[text.index("pub const ROUTES: &[Route] = &[") :]
    block = block[: block.index("\n];")]
    paths, tags = {}, []
    for line in block.splitlines():
        match = ROUTE.match(line)
        if not match or match.group(2) != "FirstParty":
            continue
        path, _, tag, ops = match.groups()
        ops = [(m, i, s.replace('\\"', '"')) for m, i, s in OP.findall(ops)]
        paths[path] = path_item(path, tag, ops)
        if tag not in tags:
            tags.append(tag)
    old = json.loads(OUT.read_text()) if OUT.exists() else {"paths": {}, "tags": []}
    for path, item in old.get("paths", {}).items():
        if "x-served-by" in item:
            paths[path] = item
    for path, tag, ops in WEB:
        item = path_item(path, tag, ops)
        item["x-served-by"] = "web"
        paths[path] = item
        if tag not in tags:
            tags.append(tag)
    tag_list = [{"name": tag, "description": TAGS.get(tag, "")} for tag in tags]
    for tag in old.get("tags", []):
        if tag["name"] not in tags:
            tag_list.append(tag)
    document = {
        "openapi": "3.1.0",
        "info": {
            "title": "OpenAgents first-party API",
            "version": "1.0.0-beta",
            "description": "Routes our own web, mobile, desktop, and terminal clients call. Not a public contract: they may change with a client release and keep working for the two newest releases (docs/api/design.md, section 2.4). The public API is https://openagents.com/api/v1/openapi.json.",
            "license": {"name": "CC0-1.0", "identifier": "CC0-1.0"},
        },
        "servers": [{"url": "https://api.openagents.com"}],
        "tags": tag_list,
        "paths": paths,
        "components": {
            "securitySchemes": {
                "bearer": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "An account session (`sess_...`) from sign-in, or an API key (`oak_...`).",
                }
            },
            "schemas": {
                "Error": {
                    "type": "object",
                    "required": ["type", "code", "message"],
                    "properties": {
                        "type": {"type": "string"},
                        "code": {"type": ["string", "null"]},
                        "message": {"type": "string"},
                        "param": {"type": ["string", "null"]},
                        "request_id": {"type": "string"},
                    },
                },
                "ErrorBody": {
                    "type": "object",
                    "required": ["error"],
                    "properties": {"error": {"$ref": "#/components/schemas/Error"}},
                },
            },
        },
    }
    OUT.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(paths)} paths")


if __name__ == "__main__":
    main()

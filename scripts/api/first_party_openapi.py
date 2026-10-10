#!/usr/bin/env python3
"""Write docs/api/openapi.first-party.json from the gateway's audience table.

The gateway's test `audience::tests::the_committed_first_party_document_is_current`
holds the file to `crates/gateway/src/audience.rs` (`first_party_document`), and
can write it itself (`OPENAGENTS_WRITE_OPENAPI=1 cargo test -p gateway audience`).
This script makes the same document without a Rust build. Entries marked
`x-served-by` (the website's first-party routes) are kept as they are.

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

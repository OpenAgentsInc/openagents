# NIP-DEPLOY — Public Deploy Status

`draft` `optional` — v1, 2026-10-21. **Implemented**: the records and the
reader's checks in `crates/nostr` (`nostr::deploy`), the publisher
`openagents deploy event`, and the Grid's build billboard.

This NIP lets a deploy pipeline say in public what it is doing: that a
staging build started, that a smoke test failed, that production now serves
a commit. One announcer key signs every event, and a reader trusts that key
alone. An event is the announcer's claim; it grants nothing and proves
nothing about the deployed service.

## Kinds

| Kind | Class | Meaning |
| --- | --- | --- |
| `3205` | Regular | One phase of one deploy run. |
| `30205` | Addressable | The lane head: the newest phase for one environment. |

The announcer publishes both events for every phase, with the same content
and `created_at`: the phase event keeps the history, and the head lets a
reader fetch each lane's current state with one filter.

## Content

The content of both kinds is one JSON object with exactly these fields,
`v: "openagents.deploy-status.v1"`:

| Field | Contract |
| --- | --- |
| `env` | `staging` or `production`. |
| `tier` | What ships: `image` (a server image), `module` (browser modules), or `content` (the livehost content tier). |
| `phase` | One of the phases below. |
| `run` | The deploy run every phase of one deploy shares: 1 to 32 lowercase letters, digits, or hyphens. |
| `commit` | Optional. The commit's SHA, 7 to 40 lowercase hex characters. |
| `subject` | Optional. The commit's subject line: 1 to 72 bytes of printable ASCII, single spaces. |
| `by` | Who started the deploy, as a short label such as `agent` or `owner`: 1 to 24 lowercase letters, digits, or hyphens. |
| `build` | Optional. The build's ID, such as a Cloud Build ID or an image tag: 1 to 64 letters, digits, `-`, `_`, or `.`. |
| `started` | When the run started, Unix seconds. `created_at` is never earlier. |
| `test` | Optional, `true` only. A test event; readers show it only when asked to. |

Unknown fields are refused. The content carries no secrets, URLs,
credentials, or internal hostnames: everything on the relay is public.

### Phases

| Phase | Meaning |
| --- | --- |
| `build_started`, `build_done`, `build_failed` | The build of the run's tier. |
| `deploying`, `deploy_failed` | The new version is being put in place: a staging revision, a content pointer, or production's candidate revision. |
| `promoted` | Production has a candidate revision with no traffic. |
| `smoke_running`, `smoke_passed`, `smoke_failed` | A smoke test against the new version. |
| `traffic_shifted` | All traffic moved to the new version. |
| `rolled_back` | Traffic moved back to an earlier version. |
| `live` | The environment serves the run's commit. |

`live`, `rolled_back`, and the three `_failed` phases end a run.

## Tags

| Tag | Kinds | Value |
| --- | --- | --- |
| `d` | `30205` only | The lane: `staging` or `production`, or `test/staging` and `test/production` for a test event. |
| `t` | both | `oa:deploy:v1`. |
| `env` | both | The content's `env`. |
| `phase` | both | The content's `phase`. |

Each tag appears once. A `3205` event has no `d` tag.

## Reader rules

A reader such as the Grid's billboard:

1. Subscribes with `{"kinds": [30205], "authors": [ANNOUNCER]}` and
   `{"kinds": [3205], "authors": [ANNOUNCER], "limit": N}`.
2. Refuses an event of another kind, from another key, with an ID or
   signature that doesn't verify, more than 300 seconds in the future, or
   whose content or tags break the rules above.
3. Groups phases by `run` and orders them by `created_at`.
4. Ignores `test` events unless its user asked to see them.

The announcer's public key is
`eaf77c570b5cdfbd7056a5ebcde063446b86c767c17e50e0c7992ed078ddffb4`
(`nostr::deploy::ANNOUNCER`). Its secret lives only in the operator's
secrets directory, at mode 600.

A reader may also show livehost deploy pointers (kind `30440`, `d` =
`TENANT/CHANNEL`, signed by the tenant's key; see `crates/livehost`) as
content deploys, checking each against the tenant key it already trusts.

## Example

```json
{"kind": 3205, "pubkey": "eaf77c57…", "created_at": 1792629000,
 "tags": [["t", "oa:deploy:v1"], ["env", "staging"], ["phase", "build_started"]],
 "content": "{\"v\":\"openagents.deploy-status.v1\",\"env\":\"staging\",\"tier\":\"image\",\"phase\":\"build_started\",\"run\":\"5f1c0a9e2b7d\",\"commit\":\"2ac6f0c4ce\",\"subject\":\"List the Grid Flint pin check as an after-close check\",\"by\":\"agent\",\"build\":\"stg-2ac6f0c4ce\",\"started\":1792629000}"}
```

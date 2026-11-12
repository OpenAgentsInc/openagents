# Protocol compliance plan: Coder and Everglade

Date: 2026-11-11. This plan follows the
[protocol review](2026-11-11-protocol-review.md). It checks the review against
the code at `78ab78f450`, lists every wire and stored shape in two areas, and
orders the work. Tracker: [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250).

Areas:

1. Coder, Microcoder and Golemancy (with the hosted runner and /chat).
2. Everglade (rooms, netcode, saves, realms, NPC chat, edits).

## Verdict on the review

The direction is right: one place defines every wire shape, and the code moves
to it one consumer at a time. The review's facts and order need these fixes.

| # | The review says | The code says | Effect on the plan |
| --- | --- | --- | --- |
| 1 | "The Everglade wire profiles already use generated codecs and are the pattern" | No file under `aethyr/`, `everglade/`, `coax/web/room/` or `apps/everglade*` has an `#import_proto`. The session wire is hand-packed text frames in `apps/openagents-web/everglade-wire.coax`. A JSON profile, `proto/verse/everglade-session-v5.json`, pins it frame by frame through `everglade-wire-profile.coax` | The pattern is real and good, but it is "one shared codec plus a profile with golden bytes", not a generated codec. This plan names that pattern and uses it |
| 2 | Each step: "generate the Coax codec … keep the JSON the same bytes" | `#import_proto` generates binary codecs only. `coax/proto.md` lists JSON conversion as separate work, and `coax/lib/proto/importer.coax` emits no JSON mapper. Almost every shape in both areas is JSON or text | The recipe can't run for JSON today. This plan adds the mapper as its own step and does not block on it |
| 3 | "Chat already writes `openagents.session.v1` JSON lines, which become its first generated mapper" (spec section 17) | The id on disk names the transcript log (`coax/lib/agent/session/log.coax`). The spec's `openagents.session.v1` (section 18.3, not 17) is NIP-SESS: adapters, commands and phases. They share a name and nothing else | Define the log in its own package and fix the spec's name clash |
| 4 | Not mentioned | `proto/` holds a second schema family: 26 JSON files under `proto/coder/`, `proto/verse/`, `proto/aethyrforge/` and `proto/aethyr/`. `proto/README.md` and `modules.json` don't list them. At least 16 have no checker | Register them, check them or delete them |
| 5 | Not mentioned | Protos exist that nothing uses. The five Coder and CLI protos (`coder/observability/v1`, `v2`, `coder/qualification/v1`, `cli/local/v1`, `cli/dictation/v1`) are in no profile, have no descriptor, and don't match the JSON that ships (`inputTokens` on the wire, `input_tokens` in the proto; no `kind`, `schema` or `usd_micros`). `engine/v1` (except two HUD messages), `verse/presence` and `host`/`reach` have no product consumer | A proto with no consumer and no check is not compliance. Each step must end with a check that ties the schema to the bytes |
| 6 | "A page or route never invents a JSON shape" | A third language builds wire JSON: plpgsql (`chat.api`, `chat.record`, `verse.npc_request`, the realm document, the undo log, `worldbuilder.lines`). Several shapes have two to six hand-written encoders (session log: 4 header encoders in 2 shapes; NPC ask: 4 clients; seat ticket: C and Coax) | Duplicate encoders are the real drift risk. Collapse to one codec first; that step needs no proto tooling |
| 7 | Order: API envelope, auth, sync, edits, chat, rooms, payments, "everything else" | The envelope touches 28 route files and nothing stored. Chat is fifth, though the owner asked for chat first. Step 6 rests on row 1. Step 8 has no end | Start with the check that stops growth, then the shapes with the most encoders and stored bytes |
| 8 | "A check can enforce part of it: a new route must name the proto message" | Most shapes here are files, streams and frames, not routes | The check keys on schema ids, route files and frame codecs |
| 9 | "Each book chapter … should name the proto package" | The review's commit changed two files. No book chapter links `proto/` yet | Each step updates the chapter that describes its shape |

Correct in the review: 42 Coax files import protos, most of them tests, Nostr
adapters and book examples; one API file (`coax/web/api/agent-wire.coax`) uses
a generated codec; the spec's inventories describe deleted code.

## What compliance means

A shape is compliant when all four hold:

1. **Registered.** `proto/registry.json` names its id, its schema, its one
   codec file and its check.
2. **One codec.** Every producer and consumer loads the same Coax file. C calls
   it through a generated unit (`agent-wire-unit.c` is the model). SQL that
   builds the shape has a test against the same fixtures.
3. **Pinned.** A fixture file in `proto/` holds exact bytes, and a check writes
   and reads them through the codec.
4. **Schema.** The fields are in a `.proto` in a `modules.json` profile, or, for
   a hand-packed frame, in a JSON profile like `everglade-session-v5.json`.

Three kinds of codec satisfy rule 2:

| Kind | Use it for | Example today |
| --- | --- | --- |
| Generated binary (`#import_proto`) | New binary bodies and files | `agent-wire.coax`, `verse/haunt.coax` |
| Generated JSON mapper | JSON shapes, once step S2 lands | none |
| Hand-written, pinned by a profile | Hot netcode; signed, hashed or stored bytes; JSON until S2 | `everglade-wire.coax` |

A third party's format (Claude Code stream-json, Codex JSON-RPC, ACP, MCP,
NIPs, LNURL) is registered as `external` with its fixture. We don't write a
schema for it.

## Shared steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| S1 | [#13236](https://github.com/OpenAgentsInc/openagents/issues/13236) | `coax/web/tool.sh wire-check`, `proto/registry.json` and `proto/wire-allowlist.txt` (see Enforcement) | The check passes on main with every shape below registered or allowlisted, and fails on a new unregistered id, route file or second encoder |
| S2 | [#13237](https://github.com/OpenAgentsInc/openagents/issues/13237) | `#import_proto` generates JSON writers and readers with exact keys (`json_name`), key order and null rules from the descriptor | A generated mapper reproduces one registered fixture byte for byte; `coax/proto.md` documents it |

S1 comes first in both areas. S2 runs beside the area steps; no area step
waits for it.

## Area 1: Coder, Microcoder and Golemancy

No code in this area uses a generated codec. Decisions: **move** (schema plus
one codec plus fixture), **pin** (keep the hand-written codec, add a profile
and fixture), **external** (third party), **keep** (local, one program),
**delete**.

### Inventory

| Shape | Where | Producers and consumers | Proto today | Decision |
| --- | --- | --- | --- | --- |
| Session log `openagents.session.v1` (header, `user`, `reply`, `thinking`, `tool`, `result`, `routed`, `notice`, `ask`, `end`), stored in `~/.openagents/coder/sessions/ID.jsonl` | `coax/lib/agent/session/log.coax` | Writers: `log.coax:72`, `session/writer.coax:153` (no `schema` key, other fields), `coder/tui/import.coax:259`, SQL `0139_chat_folders_tabs.sql:72`; `end` lines also at `coder/tui/coder.coax:2482`, `router.coax:1380`, `import.coax:146`. Readers: `log.coax` `sl_load`, `session/fork.coax`, `coder.coax` replay, `coder/tui/host.coax`, `coder/tui/acp.coax`, `apps/aethyrforge-desktop/chat.coax:140`, `observe/transcripts.coax` | none (spec 18.3 is a different thing) | Move: C1, C2 |
| Other session lines: `request` (SHA-256 of the prompt, checked by `coder replay`), `memory`, `engine_session`, `agent`, `hook`, `goal`, `job`, `answer`, `approval` (two encoders: `engine/run.coax:956`, `coder/tui/approvals.coax:55`), `diff` (legacy) | `session/writer.coax`, `coder/tui/coder.coax` | Coder only | none | Move with C1. Golden for `request`. Stop writing `diff` after C2 |
| Session sibling files: `ID.pid`, `ID.stop`, `ID.turnstop`, `ID.answer`, `ID.inbox/*.msg`, `ID.acp`, `ID.remote` | `coder/tui/coder.coax`, `acp.coax`, `remote.coax` | Coder only, same machine | none | Keep. Document in the session log's doc |
| `coder.reply-stream.v1` (`reply_delta`, `reply_end`) | `session/writer.coax:361` | Reader: `apps/aethyrforge-desktop/chat.coax:141` | none; JSON Schema `proto/coder/reply-stream-v1.schema.json`, unchecked | Move: C3 |
| `coder.observability.v1` (`model`, `usage`, `availability`) | `session/writer.coax:410-452`; `availability` also at `engine/run.coax:304` and `coder/tui/availability.coax:10` | Session readers, `coder usage` | `coder/observability/v1/events.proto`, unused, fields differ | Move: C3. Fix the proto to the JSON |
| `coder.observability.v2` (`attempt`) | `engine/micro-observation.coax:26` | `coder/tui/mcworker.coax`, `microcoder.coax`, `run.coax` | `v2/events.proto`, unused, lacks `usd_micros` | Move: C3 |
| `coder.route.v2` | `coder/tui/route.coax:287` | Session readers | none; JSON Schema `proto/coder/route-v2.schema.json`, unchecked | Move: C3 |
| `--json` and `--stream-json` output (`usage`, `ls`, `status`, `import`, `run --detach`, `stop`, `replay`, `bench`, `jobs`, `queue`, credentials, plugins) | `coder/tui/coder.coax`, `daemon.coax`, `credential.coax`, `plugins.coax` | Scripts, Aethyrforge, Golems | none | Move: wave 2 (C8). `--stream-json` is the session lines plus C3 |
| Qualification evidence `coder.seal.v1`, `coder.checkpoints.v1`, `coder.execution.v1` (two encoders: `coder/tests/conversation.coax:53`, `issue.coax:10`), `coder.qualification.v1` | `coder/tests/qualify.coax` | Stored and hashed in `receipt.json` under `docs/verification/` | `coder/qualification/v1/evidence.proto`, unused | Move: C4. Golden, byte for byte |
| Hosted runner lines (`turn`, `event`, `call`, `reply`, `done`, `stop`) | `coax/web/agent/runner.c:494,660`, `chat-runner.c:338`; `en_event` in `turn.coax:62` and `ch_event` in `chat.coax:63` | Runner and engine | none | Move: C5 |
| Runner to API: event batches, end bodies (two shapes: `runner.c:597`, `chat-runner.c:269`), `next` answer; stored in `chat.events(type, data jsonb)` | `coax/web/agent/`, `coax/web/api/chat.c`, SQL `chat.api` (`0021_chat.sql`) | Runner, API, page | none. `docs/chat.md` says the events follow `openagents.agent.managed.v1`, which exists only as a spec sketch | Move: C5 |
| /chat routes and page lines `Z C S U T` | `chat.c:8-19`, SQL `chat.lines` | `apps/openagents-web/chat.coax`, polled; there is no event stream | none | Move: wave 2 (C9), after C5 |
| `chat.record` (the session log from SQL) | `0139_chat_folders_tabs.sql:57-95` | `GET /api/chat/record` | none | Pin in C2 against C1's fixtures |
| Sealed chat credential: AES-256-GCM, AAD `oa-chat-credential.v1\|OWNER\|KIND` | `coax/web/agent/chat-runner.c:182` | Runner only; stored sealed | none | Pin. Golden for the AAD and layout |
| NIP-98 events built in C | `coax/web/agent/runner.c:221` | API | Nostr | External. Wave 2: build them with `coax/lib/nostr`, not by hand |
| AI-call ledger `openagents.ai-call.v1`, `~/.openagents/usage/ai-calls.jsonl` | Two encoders with different fields: `apps/everglade-desktop/next.coax:398`, `tiers.coax:123` | Readers: `next-context.coax:483`, `golems.coax:784` | none | Move: C6 |
| `openagents.next-issue.decision.v1`, `decisions.jsonl` | `next.coax:432` | The Workshop | none | Move: C6 |
| Tier rubric (`scope`, `ambiguity`, `risk`, `depth`, `delegable`) | `coax/lib/agent/tier.coax:109` | Sent to the model as its answer schema; read by `next.coax`, `tiers.coax` | none | Move: C6 |
| Price tables | `golems.coax:193` (cents) and `coax/lib/gateway/rates.coax:33` (micro-USD) | Golems board, gateway | none | Delete `gs_price` in C6; use `rates.coax` |
| Golems cache `golem5`, snapshot lines `T G M Q`, `issues.tsv`, `hosts.txt` | `apps/everglade-desktop/golems.coax:40-62,471-499` | `golem-board.coax`, same app | none | Keep. A cache one app writes and reads; the name carries its version |
| Remote-host pull | `golems.coax:736` (`ssh find`, `rsync`) | Copies other tools' transcripts | none | External. No shape of ours |
| Issue claim file `everglade-owner` (a pid) | `next-context.coax:430` | The Workshop | none | Keep |
| `openagents.coder.agents.v1`, `openagents.models.v1` | `coax/lib/agent/observe/processes.coax:50`, `apps/openagents-cli/models.coax:38` | `apps/aethyrforge-desktop/setup.coax` | `cli/local/v1/inventory.proto`, unused | Move: wave 2 (C7) |
| `openagents.dictation.v1`, `openagents.dictation.stream.v1` | `apps/openagents-cli/dictate.coax:99`, `dictate-stream.coax:40` | `coder/tui/dictate.coax` | `cli/dictation/v1/stream.proto`, unused | Move: wave 2 (C7) |
| `coder.session-reader.v1` envelope | Two writers: `observe/claude-history.coax:446`, `coder/tui/session-reader.coax:165` | `coder mcp`, the Codex tool | none | Move: wave 2 (C10) |
| Microcoder worker lines (`ready`, `reply`, `attempt`, `command`, `view`, `ask`, `ended`, `embedding`, `journal`) | `engine/mcworker-client.coax:80`, `coder/tui/mcworker.coax:50-160` | Coder and its worker | none | Move: wave 2 (C11) |
| Microcoder journal AJR1 (binary, hash-chained), `sessions/worker-US.ajr` | `coax/lib/agent/journal.coax:182` | Worker, replay | none | Pin: wave 2. Hand-packed is right for a hash chain; add a profile and golden |
| Microcoder prompt state and action schema | `coax/lib/microcoder/state.coax:333`, `action.coax` | Sent to the model; the prefix must stay byte-stable for the prompt cache | none | Pin: wave 2. Golden for the prefix |
| Consent ledger and memory log (hash-chained JSON lines) | `engine/consent-gate.coax:80`, `coder/tui/memory.coax:100` | Coder, Aethyrforge chat | none | Pin: wave 2. The digest covers typed data, so the golden pins fields |
| Router and daemon files (`router.json` as `coder.router.v1`, `log.jsonl`, `sources.json`, `outbox/`, `daemon.json`, `jobs/`) | `coder/tui/router.coax`, `daemon.coax` | Coder only | none; the spec's `coder.router.v1` is a different thing | Keep; register the ids. Fix the spec's name clash |
| `coder host`: `openagents.host-request.v1`, `openagents.host-reply.v1` in signed kind 3188 events | `coax/lib/nostr/host/browser.coax:13-59` | Paired browser | `host/v1/host.proto` has a different shape | External (NIP-HOST is the definition). Golden from the NIP's examples. The spec decides what `host.v1` is for |
| System One request JSON | `coax/lib/systemone/request.coax` | Psionic's `/v1/systemone` | `decision/v1alpha1/systemone.proto`, used on the server only | Pin. The JSON contract is the definition (Psionic book chapter 21) |
| Claude Code stream-json and control frames; Codex app-server JSON-RPC; ACP (client and `coder acp`); MCP; hook output; usage APIs; other tools' transcripts; provider request bodies | `coax/lib/agent/claude/`, `codex/`, `cursor/`, `coder/tui/acp.coax`, `mcp.coax`, `usage.coax`, `coax/lib/gateway/` | The other party | theirs | External. Register each with its fixture |
| Credentials (`NAME.env`), plugin manifests (plain or a signed NIP-78 event) | `coder/tui/credential.coax`, `plugins.coax` | Coder only | none / Nostr | Keep / external |

### Steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| C1 | [#13238](https://github.com/OpenAgentsInc/openagents/issues/13238) | Session log schema and one writer. Package `openagents.session.log.v1alpha1`; the id on disk stays `openagents.session.v1` | Every writer calls `log.coax`. The header always has `schema`. `proto/coder/session-log-v1.json` holds golden lines and its check passes. Old files still load |
| C2 | [#13239](https://github.com/OpenAgentsInc/openagents/issues/13239) | One reader. Move the six other readers to `sl_load`; test `chat.record` SQL against C1's fixtures; stop writing `diff` | No file but `log.coax` parses a session line. `wire-check` shows one codec for the id |
| C3 | [#13240](https://github.com/OpenAgentsInc/openagents/issues/13240) | Stream records: reply-stream, observability v1 and v2, route v2. Fix the protos to the JSON, add a profile, one codec file | One encoder per record. Golden lines pass. The two JSON Schema files are deleted |
| C4 | [#13241](https://github.com/OpenAgentsInc/openagents/issues/13241) | Qualification evidence: one `coder.execution.v1` encoder, profile, goldens | Committed receipts under `docs/verification/` re-encode to the same bytes |
| C5 | [#13242](https://github.com/OpenAgentsInc/openagents/issues/13242) | Hosted runner: `openagents.agent.managed.v1alpha1` events, runner lines and end bodies in one codec that `turn.coax`, `chat.coax` and the C runner share | `en_event`/`ch_event` and the two end-body builders are gone. `test_zchat.py` and `test-chat-engine.py` pass on the fixtures |
| C6 | [#13243](https://github.com/OpenAgentsInc/openagents/issues/13243) | Golemancy records: AI-call ledger, next-issue decision, tier rubric; one price table | One encoder for each. Existing ledger files still load. `gs_price` is deleted |

Wave 2, in order: C7 local inventory and dictation; C8 `--json` outputs; C9
/chat routes and page lines; C10 session-reader envelope; C11 Microcoder worker
lines, journal and prompt prefix; C12 consent and memory ledgers; C13 register
every external format with a fixture; C14 NIP-98 through `coax/lib/nostr`.

## Area 2: Everglade

Generated codecs in the game area: `verse/haunt.coax`, `verse/voice-wire.coax`
and `verse/home.coax`. None is Everglade's.

### Inventory

| Shape | Where | Producers and consumers | Schema today | Decision |
| --- | --- | --- | --- | --- |
| Session v5 text frames (`P H I C E R K t T O A`, `W D s S x n e w o k r g`, rest and kit records) | `apps/openagents-web/everglade-wire.coax` | Page (`everglade-net.coax`, `everglade2-net.coax`) and room (`coax/web/room/room-everglade.coax`) load the same file | Profile `proto/verse/everglade-session-v5.json`, checked | Pin. Already compliant except registration. 30 Hz, 1,000 bytes a seat a tick, packed rows under half the text size: a hand-packed codec is justified |
| Relay frames `y j l t` | `coax/web/room/room-relay.coax`; parsed again in `everglade2-net.coax:452` | Relay, page | In the v5 profile | Pin. Move the page's parser into the shared file (E2) |
| Link input v1 (`J1`) | `apps/openagents-web/everglade-link-wire.coax` | Page, room | `proto/verse/everglade-link-input-v1.json`, unchecked | Pin: E1 adds its check |
| Everglade II familiar frames `M1 M3-M7`, `m1-m8`, snapshot `m2` | Room: `room-e2-familiar.coax`. Page: `everglade2-net.coax:406-441,1062-1085` | Two hand-written codecs | Seven JSON files, unchecked | Pin: E2. One shared codec |
| Healer's Kit `U1`, `u1` | `room-e2-aid.coax:14,91`; `everglade2-net.coax:444,876` | Two codecs | JSON, unchecked | Pin: E2 |
| Path guard `g2`, `g3`; monsters `h2`, `h3` | `room-e2-guard.coax:377`, `room-e2-monsters.coax:321`; `everglade2-net.coax:407-410` | Two codecs | none, no doc | Pin: E3 |
| Projectiles `F a d` | `aethyr/net/projectile.coax`, `record.coax` | Shared | none | Pin: E3 adds the profile |
| Latency observation `@1` | `aethyr/net/latency.coax` | Shared | Profile, checked | Pin. Compliant |
| WebTransport framing (mark byte, channels, 1,200-byte datagrams) | `aethyr/net/transport.coax`, `room-wt.coax` | Shared | none | Pin: E3 |
| Old wire (`H 1`) | `coax/web/room/room-oldwire.coax` | Ends 2026-11-12 | none | Delete: E1 |
| Session v4 profile | `proto/verse/everglade-session-v4.json` | No code reads it | — | Delete: E1 |
| Orbit League v2 frames | `apps/openagents-web/orbit-session.coax` | Shared | Profile, checked | Pin. Compliant |
| Seat ticket v2/v3 (HMAC-SHA256) | Written by C `snprintf` in `coax/web/api/player.c:524`; read by `room-ticket.coax` `tk_check` | Two languages | Fixture in the v5 profile | Pin: E5. One Coax writer that C calls. Signed: golden |
| Ticket v4 resume envelope (signed) | `aethyr/session/resume-payload.coax`, `resume-envelope.coax` | Shared, exported to C | none | Pin: E5 adds fixtures |
| Character checkpoint v2/v3 (signed, stored in `verse.character_checkpoints_v2`) | `aethyr/session/character-checkpoint.coax`; SQL rewrites v3 to v2 at `player.c:350`; v1 keys duplicated in `coax/web/api/checkpoint-policy.coax` | Room, API | JSON Schema, unchecked | Pin: wave 2 (E7). Golden for the SQL rewrite |
| Room reports to the API (vitals, directory, stats; signed) and the control poll reply | `room-vitals.coax`, `room-directory.coax`, `room-stats.h`, `room-control.coax`; `player.c`, `rooms.c` | Coax room, C API | none | Pin: wave 2 (E8) |
| Item ledger op | `player.c:703`, `aethyr/session/ledger.coax` | No room sends it | none | Wave 2: wire it or delete it |
| `room-state {look,class}` | `player.c` | Accepted and dropped | none | Delete: wave 2 |
| Player appearance `aethyr.player.appearance.v1` (CC2, CC3, CC4 text) | `aethyr/character-appearance.coax:59,72` | Stored in localStorage (`coax/web/host.js:570`) and the iOS store | none | Move: wave 2 (E9), before sync stores it on the server. Golden for each version |
| Realm save AESV (CRC-32), stored in IndexedDB and `verse.progress` | `apps/openagents-web/save.coax` | Page, API | none | Pin: wave 2 (E10). Golden |
| Realm document `aethyr.realm` v1 (SHA-256 per revision) | Built in SQL (`0093`, `0109`, `0111`); read by `aethyr/forge/doc.coax` | `myrealm.coax`, `apps/openagents-cli/realm.coax`, API | none | Move: E6 |
| Edit operations and the undo log `{to,card,ops:[{slot,before,after}]}`; save body `{rev,moved,shaped}` | `aethyr/forge/edit.coax:339-514`; again in plpgsql (`0111`) | Aethyrforge, My Realm, API | none | Move: E6 |
| Engine pin manifest `aethyr-engine 1` (hashed) | `aethyr/forge/publish.coax`; again in SQL (`0107`) | Shared fixture | none | Pin. Register |
| Realm API routes (`plan`, `pick`, `settings`, `switch`, `revise`, `fork`, `publish`, `visibility`, `place`, `lore`, `quest`, `asks`, text lists) | `coax/web/api/realms.c:7-60` | Site pages, CLI | none | Move: wave 2 (E11), on the `api.v1` envelope |
| Everglade city edits `eg-edit 1` | `verse/everglade/edits.coax:13` | `everglade-edit.coax`, `apps/aethyrforge-desktop/scene-cli.coax`, room (`room-world.c`) | none | Pin: wave 2 |
| NPC ask `{npc,world,history,question,controls,visit}` and `{text,model}` | At least four client encoders: `verse/everglade/people.coax:128`, `verse/fairy-forest/talk.coax:149`, `apps/openagents-web/realm.coax:1708`, `myrealm.coax:2040`. Server: SQL `verse.npc_request`. A bounded codec, `aethyr/talk/wire.coax`, exists and only an example uses it | Pages, API | none | Move: E4 |
| WorldBuilder: act request, reply lines `S`, `A op\|…`, save `WB1` | `apps/openagents-web/worldbuilder*.coax`, `coax/web/api/worldbuilder.c`, SQL `worldbuilder.lines` | One page | none | Move: wave 2 (E12), after #13226 puts its edits on the forge log. Its operations become E6's |
| Aethyrforge local commands (scene v1/v2, NPC, raid) and `aethyrforge.scene-session.v1` | `apps/aethyrforge-desktop/scene-policy.coax`, `aethyr/forge/npc-command.coax`, `raid-command.coax` | Local tools | Four JSON Schemas, unchecked | Pin: E1 adds checks |
| Asset packs VPK1, VPK2, VPK3 (VPK3 hashed) | `aethyr/forge/pack.coax`, `page-pack.coax`; also `verse/tools/pack_realm.py` and a JS reader at `coax/web/host.js:1932` | Build, page, room | `proto/verse/asset-pack-v2.json`, unchecked | Pin: wave 2. Hand-packed is right for a mapped file. One writer |
| Presence and room chat JSON (`/api/playtest/presence`, `chat`, `inbox`) | `coax/web/api/room.c` | Older realms | `verse/presence/v1alpha1`, unused | Wave 2: decide with the room servers' reach; don't build on the unused proto before then |
| Player, world, progress, seats and packs routes | `player.c`, `seats.c`, `verse_packs.c` | Pages | none | Wave 2 (E11) |
| Zone geometry and logic | `everglade/zone.coax`, `everglade/world/` | Compiled in | — | Keep. Code, not a wire. The `P` frame's content pin covers it |
| SRD rules data | `aethyr/rules/srd/data/*.coax`, generated from the pinned PDF | Compiled in; only indices travel | — | Keep. The `P` frame's rules pin covers it |
| Haunted House save and scene | `verse/haunt.coax` | Generated codec | `metareal/haunt/v1alpha1` | Compliant |
| `engine/v1` (scene, agent, control, state, assets, most of HUD), `verse/presence`, `verse/common` | `proto/openagents/` | Tests and tools only | published | Keep frozen. Mark "no product consumer" in `proto/README.md` |

### Steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| E1 | [#13244](https://github.com/OpenAgentsInc/openagents/issues/13244) | Clean the profile family: delete the v4 profile and `room-oldwire.coax`; add a check for every unchecked profile and schema under `proto/verse/` and `proto/aethyrforge/`, or delete it | Every file under those folders names a check that passes. `proto/README.md` lists the family |
| E2 | [#13245](https://github.com/OpenAgentsInc/openagents/issues/13245) | Familiar and Healer's Kit frames in one shared codec file that room and page load; profiles with golden frames | `room-e2-familiar.coax`, `room-e2-aid.coax` and `everglade2-net.coax` hold no frame text of their own. The profile check passes |
| E3 | [#13246](https://github.com/OpenAgentsInc/openagents/issues/13246) | Guard, monster and projectile frames and the WebTransport framing: shared codec, profile, doc | Same as E2 for `g2 g3 h2 h3 F a d`. `docs/protocol/` describes them |
| E4 | [#13247](https://github.com/OpenAgentsInc/openagents/issues/13247) | NPC ask: package `openagents.verse.npc.v1alpha1`; the four clients use `aethyr/talk/wire.coax`; a SQL test on the same fixtures | One client encoder. `verse.npc_request` accepts every fixture |
| E5 | [#13248](https://github.com/OpenAgentsInc/openagents/issues/13248) | Seat ticket: one Coax writer that `player.c` calls; fixtures for v2, v3 and the v4 envelope | No `snprintf` builds a ticket. A ticket the writer makes passes `tk_check`; goldens match byte for byte |
| E6 | [#13249](https://github.com/OpenAgentsInc/openagents/issues/13249) | Realm document and edit operations: package `openagents.verse.doc.v1alpha1`; fixtures shared by `aethyr/forge/` and the SQL | The same fixtures give the same document bytes and SHA-256 in Coax and SQL, for save, undo and redo |

Wave 2, in order: E7 character checkpoint; E8 room reports; E9 appearance
record; E10 realm save; E11 realm, player and seat routes on the `api.v1`
envelope; E12 WorldBuilder operations (after #13226); then packs, city edits
and presence.

## Golden tests

A golden pins exact bytes. These shapes need one before any codec change,
because a signature, a hash or a stored file depends on the bytes.

| Shape | Why | Step |
| --- | --- | --- |
| Session `request` line | `coder replay` compares the prompt's SHA-256 | C1 |
| Qualification seals, checkpoints, receipts | Hashed; the doc requires exact seals | C4 |
| Sealed chat credential | The AAD string is part of the ciphertext's authenticity | C5 |
| AJR1 journal; consent and memory ledgers | Hash chains | wave 2 |
| Microcoder prompt prefix | The prompt cache keys on bytes | wave 2 |
| NIP-HOST request and reply; NIP-98; plugin manifests | Signed events | wave 2 |
| Seat tickets, resume envelope | HMAC | E5 |
| Character checkpoint; room reports | HMAC, stored | wave 2 |
| Realm document, undo log, engine pin | SHA-256 per revision; two languages | E6 |
| Realm save, VPK3 packs | CRC-32, SHA-256 | wave 2 |
| Appearance record | Stored on players' devices | wave 2 |

## What to delete

| What | When |
| --- | --- |
| `proto/verse/everglade-session-v4.json`, `coax/web/room/room-oldwire.coax` | E1 |
| `proto/coder/reply-stream-v1.schema.json`, `route-v2.schema.json` | C3 |
| Duplicate encoders: session header and `end`, `approval`, `availability`, `coder.execution.v1`, `openagents.ai-call.v1`, runner events and end bodies, NPC ask clients, the C ticket writer | The step that owns each |
| `gs_price` in `golems.coax` | C6 |
| The `diff` session line | C2 |
| The `room-state` route; the item ledger route if no room sends to it | wave 2 |
| `aethyr/talk/wire.coax`'s "example only" status: it becomes the codec | E4 |

## Enforcement

One tool, `coax/web/tool.sh wire-check`, written in Coax beside
`everglade-parity.coax` and `components-check.coax`.

`proto/registry.json` has one row per shape: `id`, `kind` (`proto`, `profile`,
`external`, `local`), `schema` (a proto message or a profile file), `codec`
(the one file that may write the id), `check` (a command) and `step`.

The check fails when:

1. A schema-id literal appears in code (`"NAME.vN"` after a `schema` key, or a
   `*.vN` id in a registry row's pattern) and the id is not in the registry.
2. A registered id is written in a file other than its `codec`, its tests or
   its fixtures.
3. A `coax/web/api/*.c` file with a `*_route` function, or a
   `coax/web/room/room-*.coax` file that writes frames, is in neither the
   registry nor the allowlist.
4. A file under `proto/coder/`, `proto/verse/`, `proto/aethyrforge/` or
   `proto/aethyr/` has no registry row with a `check`.
5. A `.proto` under `proto/openagents/` is in no `modules.json` profile and not
   marked `no-consumer` in the registry.
6. `proto/wire-allowlist.txt` has a line that `origin/main`'s copy lacks. The
   allowlist only shrinks.

S1 seeds the registry and the allowlist from the two inventories above. Each
later step removes its lines from the allowlist. `AGENTS.md` lists the command
under "Build and test".

## Changes to the review and the spec

Review:

- Replace step 6's "already use generated codecs" with "use one shared
  hand-packed codec pinned by a profile".
- Say that JSON mappers are not generated yet, and link S2.
- Reorder: the check first, then the session log and chat, then the envelope.
- Give step 8 an end: the allowlist reaches zero.

Spec:

| Section | Change |
| --- | --- |
| 5 | Add the two name clashes and their fixes: the transcript log's package is `openagents.session.log`; Coder's `router.json` id is not `openagents.coder.router.v1` |
| 8 | State what Coax generates today (binary) and what S2 adds (JSON). Add "hand-written, pinned by a profile" as an allowed codec with its conditions |
| 18.2 | `agent.managed` starts as `v1alpha1` from C5's real events, not the proposal's full API |
| 18.3 | Rename or split: NIP-SESS records stay; add the transcript log as its own package |
| 23 (host) | Say which is the definition for `coder host`: NIP-HOST's JSON. Say what `host.v1`'s `AgentRequest` is for, or mark it history |
| 25 | Rewrite for today's Coder: session log, stream records, qualification, Microcoder worker and journal, Golemancy records. Move task store, Loro sync, Foreman and Lab to the history appendix |
| 33 | Document `proto/registry.json`, the profile family and `wire-check` |
| 36 | Add the compliance definition above as the exit test for P4 and P6 |
| Verse chapter 12.2 | The realm session is text frames with profiles; a binary session is a new negotiated version, not a silent swap |

Books: C1 and C3 update the Coder book chapters that describe the log and the streams (`coder/book/ch002`, `ch016`); C5 updates
`docs/chat.md`; E2 to E5 update the Everglade multiplayer spec
(`docs/verse/2026-11-03-everglade-multiplayer-spec.md`); the OpenAgents book's
chapters 46 to 48 link `proto/` in S1.

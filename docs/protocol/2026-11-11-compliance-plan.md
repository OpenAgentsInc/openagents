# Protocol compliance plan: Coder and Everglade

Date: 2026-11-11. This plan follows the
[protocol review](2026-11-11-protocol-review.md). It checks the review against
the code at `78ab78f450`, lists every wire and stored shape in two areas, and
orders the work. Tracker: [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250).

Step S1 (#13236) checked both inventories against the code at `fc248a31dc`,
line by line, and corrected the rows that were wrong. It also built the check:
[`proto/registry.json`](../../proto/registry.json),
[`proto/wire-allowlist.txt`](../../proto/wire-allowlist.txt) and
`coax/web/tool.sh wire-check`. To register a shape, see
[Register a shape](#register-a-shape).

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
| 4 | Not mentioned | `proto/` holds a second schema family: 25 JSON files under `proto/coder/`, `proto/verse/`, `proto/aethyrforge/` and `proto/aethyr/`. `modules.json` doesn't list them. 22 have no checker: only session v5, Orbit v2 and latency v1 have a check that passes, and the v4 profile's own check would fail | Register them, check them or delete them |
| 5 | Not mentioned | Protos exist that nothing uses. The five Coder and CLI protos (`coder/observability/v1`, `v2`, `coder/qualification/v1`, `cli/local/v1`, `cli/dictation/v1`) are in no profile (only in the catch-all `owned` module and its `owned.pb`), no code imports them, and they don't match the JSON that ships (`inputTokens` on the wire, `input_tokens` in the proto; no `kind`, `schema` or `usd_micros`). `engine/v1` (except `hud.proto`'s `HudAction` and `HudInvocation`, which `verse/home.coax` writes), `verse/presence`, `verse/common` and `host`/`reach` (a library adapter that only tests and an example load) have no product consumer | A proto with no consumer and no check is not compliance. Each step must end with a check that ties the schema to the bytes |
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
| S1 | [#13236](https://github.com/OpenAgentsInc/openagents/issues/13236) | `coax/web/tool.sh wire-check`, `proto/registry.json` and `proto/wire-allowlist.txt` (see Enforcement). Done | The check passes on main with every shape below registered or allowlisted, and fails on a new unregistered id, route file or second encoder |
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
| Session log `openagents.session.v1` (header, `user`, `reply`, `thinking`, `tool`, `result`, `routed`, `notice`, `ask`, `end`), stored in `~/.openagents/coder/sessions/ID.jsonl` | `coax/lib/agent/session/log.coax` | Before C1 there were four header writers in two shapes: `log.coax` `sl_header` (the desktop app, through `apps/openagents-desktop/zeron/store.coax`), `session/writer.coax` `sw_open` (Coder and the turn runner: no `schema` key, more fields), `coder/tui/import.coax` `ci_import_claude`, and SQL `0139_chat_folders_tabs.sql` `chat.record`; `end` lines in `log.coax`, `writer.coax`, `coder/tui/coder.coax` (`coder stop`), `router.coax` (the watchdog), `import.coax`; `tool` and `result` in `log.coax`, `writer.coax` and `import.coax`. After C1 (#13238) `log.coax` writes every line (`sl_session` ... `sl_end`) and the header always has `schema`; only the SQL is left (C2). Readers (each parses `kind` itself; none compares the id): `log.coax` `sl_load` (through `zeron/store.coax`), `session/fork.coax` (`sf_fork`, `sf_turns_text`), `coder/tui/coder.coax` (`cd_show_past`, `cd_render` for `coder log`, `cd_cmd_replay`), `coder/tui/host.coax` (`hs_step`), `coder/tui/acp.coax` (`cd_acp_line` and the turn reader), `coder/tui/session.coax` (`cs_read`), `coder/tui/router.coax` (three readers), `coder/tui/remote.coax`, `engine/run.coax` (`tr_fork`), `apps/aethyrforge-desktop/chat.coax` (`fc_read`), and `coder/tui/memory.coax`, which finds user lines by their first bytes (`{"kind":"user"`). `coax/lib/agent/observe/transcripts.coax` only lists the files. | Proto `openagents.session.log.v1alpha1` (profile `session-log`); spec 18.3 is a different thing | Writer: compliant after C1. Readers: one, `log.coax` (C2) |
| Other session lines: `request` (SHA-256 of the prompt, checked by `coder replay`), `memory`, `engine_session`, `agent`, `context`, `keys`, `note`, `hook`, `goal`, `job`, `answer`, `approval` (two encoders before C1: `engine/run.coax` `tr_approval`, `coder/tui/approvals.coax`), `diff` (legacy) | `session/log.coax` since C1; `session/writer.coax`, `coder/tui/coder.coax` and `coder/tui/session.coax` call it | Coder only | In the session log's proto | Moved in C1, with a golden for `request`. Stop writing `diff` in C2. Microcoder's `journal` line rides in the same file and is C11's |
| Session sibling files: `ID.pid`, `ID.stop`, `ID.turnstop`, `ID.answer`, `ID.inbox/*.msg`, `ID.acp`, `ID.remote` | `coder/tui/coder.coax`, `acp.coax`, `remote.coax` | Coder only, same machine | none | Keep. Document in the session log's doc |
| `coder.reply-stream.v1` (`reply_delta`, `reply_end`) | `session/writer.coax:361` | Reader: `apps/aethyrforge-desktop/chat.coax:141` | none; JSON Schema `proto/coder/reply-stream-v1.schema.json`, unchecked | Move: C3 |
| `coder.observability.v1` (`model`, `usage`, `availability`) | `session/writer.coax:410-435` (`model`, two `usage` encoders); `availability` at `engine/run.coax:304` and `coder/tui/availability.coax:10` | Session readers, `coder usage` (none compares the id) | `coder/observability/v1/events.proto`, unused, fields differ | Move: C3. Fix the proto to the JSON |
| `coder.observability.v2` (`attempt`) | `engine/micro-observation.coax:26` | `coder/tui/mcworker.coax`, `microcoder.coax`, `run.coax` | `v2/events.proto`, unused, lacks `usd_micros` | Move: C3 |
| `coder.route.v2` | `coax/lib/agent/engine/route.coax:287` (`agr_json`) | Session readers (by `kind`; none compares the id) | none; JSON Schema `proto/coder/route-v2.schema.json`, unchecked | Move: C3 |
| `--json` and `--stream-json` output (`usage`, `ls`, `status`, `import`, `run --detach`, `stop`, `replay`, `bench`, `jobs`, `queue`, credentials, plugins) | `coder/tui/coder.coax` (plugins at `:1645`), `daemon.coax`, `credential.coax`, `host.coax:893` | Scripts, Aethyrforge, Golems | none | Move: wave 2 (C8). `--stream-json` is the session lines plus C3 |
| Qualification evidence `coder.seal.v1`, `coder.checkpoints.v1`, `coder.execution.v1` (two encoders: `coder/tests/conversation.coax:53`, `issue.coax:10`), `coder.qualification.v1` | `coder/tests/qualify.coax` (seal, qualification; reads the others), `coder/tests/conversation.coax:34` (checkpoints) | Stored and hashed in `receipt.json` under `docs/verification/` | `coder/qualification/v1/evidence.proto`, unused | Move: C4. Golden, byte for byte |
| Hosted runner lines (`turn`, `event`, `call`, `reply`, `done`, `stop`) | `coax/web/agent/runner.c:494,660`, `chat-runner.c:338`; `en_event` in `turn.coax:62` and `ch_event` in `chat.coax:63` | Runner and engine | none | Move: C5 |
| Runner to API: event batches (`runner.c:421`), end bodies (two shapes: `runner.c:597`, `chat-runner.c:269`), `next` answer; stored in `projects.events` (`0013_project_agent.sql`) for the project runner and `chat.events(type, data jsonb)` for /chat | `coax/web/agent/`, `coax/web/api/chat.c`, SQL `chat.api` (`0021_chat.sql`, redefined in `0139_chat_folders_tabs.sql:131`) | Runner, API, page | none. `docs/chat.md` says the events follow `openagents.agent.managed.v1`, which exists only as a spec sketch | Move: C5 |
| /chat routes and page lines `Z C S U T` | `chat.c:8-19`, SQL `chat.lines` (`0021_chat.sql:104`, replaced in `0139_chat_folders_tabs.sql:100`) | `apps/openagents-web/chat.coax`, polled; there is no event stream | none | Move: wave 2 (C9), after C5 |
| `chat.record` (the session log from SQL) | `0139_chat_folders_tabs.sql:57-95`, `0162_chat_record_log.sql` | `GET /api/chat/record` | none | Pinned in C2: `test_zchat.py` reads every line through `log.coax` and requires it back byte for byte |
| Sealed chat credential: AES-256-GCM, AAD `oa-chat-credential.v1\|OWNER\|KIND` | `coax/web/agent/chat-runner.c:43` (`aad_of`; `seal` at `:46`, `unseal` at `:65`) | Runner only; stored sealed | none | Pin. Golden for the AAD and layout |
| NIP-98 events built in C | `coax/web/agent/runner.c:221` | API | Nostr | External. Wave 2: build them with `coax/lib/nostr`, not by hand |
| AI-call ledger `openagents.ai-call.v1`, `~/.openagents/usage/ai-calls.jsonl` | Two encoders with different fields: `apps/everglade-desktop/next.coax:398`, `tiers.coax:123`; appended by `Sources/Workshop.swift:400`, `next-drive.coax:83` and `tiers.coax:139` | Readers: `next-context.coax:483`, `golems.coax:784` (neither compares the id) | none | Move: C6 |
| `openagents.next-issue.decision.v1`, `decisions.jsonl` | `next.coax:433`; appended by `Sources/Workshop.swift:402` and `next-drive.coax:85` | The Workshop writes it; no file in the repository reads it | none | Move: C6 |
| Tier rubric (`scope`, `ambiguity`, `risk`, `depth`, `delegable`) | `coax/lib/agent/tier.coax:109` | Sent to the model as its answer schema; read by `next.coax`, `tiers.coax` | none | Move: C6 |
| Price tables | `golems.coax:193` (cents) and `coax/lib/gateway/card.coax:36` (`GW_RATES`, micro-USD; `gw_price` in `rates.coax:33` reads it) | Golems board, gateway | none | Delete `gs_price` in C6; use `rates.coax` |
| Golems cache `golem5`, snapshot lines `T G M Q`, `issues.tsv`, `hosts.txt` | `apps/everglade-desktop/golems.coax:40-62,471-499` | `golem-board.coax`, same app | none | Keep. A cache one app writes and reads; the name carries its version |
| Remote-host pull | `golems.coax:736` (`ssh find`, `rsync`) | Copies other tools' transcripts | none | External. No shape of ours |
| Issue claim file `everglade-owner` (a pid) | `next-context.coax:430` | The Workshop | none | Keep |
| `openagents.coder.agents.v1`, `openagents.models.v1` | `coax/lib/agent/observe/processes.coax:50`, `apps/openagents-cli/models.coax:38` | `apps/aethyrforge-desktop/setup.coax` | `cli/local/v1/inventory.proto`, unused | Move: wave 2 (C7) |
| `openagents.dictation.v1`, `openagents.dictation.stream.v1` | `apps/openagents-cli/dictate.coax:99`, `dictate-stream.coax:40` | `coder/tui/dictate.coax:173` reads `openagents.dictation.v1` only; the stream id has no reader outside its test | `cli/dictation/v1/stream.proto`, unused; it covers the stream id only | Move: wave 2 (C7) |
| `coder.session-reader.v1` envelope | Two writer files, three encoder sites each: `observe/claude-history.coax:446,462,475`, `coder/tui/session-reader.coax:165,264,282` | `coder mcp`, the Codex tool, `coder session-read` | none | Move: wave 2 (C10) |
| Microcoder worker lines (`ready`, `reply`, `attempt`, `command`, `view`, `ask`, `ended`, `embedding`, `journal`) | `engine/mcworker-client.coax:80`, `coder/tui/mcworker.coax:50-160` | Coder and its worker | none | Move: wave 2 (C11) |
| Microcoder journal AJR1 (binary, hash-chained), `sessions/worker-US.ajr` | `coax/lib/agent/journal.coax:182` | Worker, replay | none | Pin: wave 2. Hand-packed is right for a hash chain; add a profile and golden |
| Microcoder prompt state and action schema | `coax/lib/microcoder/state.coax:333`, `action.coax` | Sent to the model; the prefix must stay byte-stable for the prompt cache | none | Pin: wave 2. Golden for the prefix |
| Consent ledger and memory log (hash-chained JSON lines) | `engine/consent-gate.coax:80`, `coder/tui/memory.coax:100` | Coder (`engine/run.coax`, `coder/tui/gate.coax`) | none | Pin: wave 2. The digest covers typed data, so the golden pins fields |
| Router and daemon files (`router/log.jsonl`, `router/sources.json`, `outbox/`, `daemon.json`, `jobs/`) and `router.json` as `coder.router.v1` | `coder/tui/router.coax`, `daemon.coax`; `router.json` is read at `coax/lib/agent/engine/route.coax:195` and its default is written at `apps/aethyrforge-desktop/chat.coax:20` | Coder; `router.json` also Aethyrforge | none; the spec's `coder.router.v1` is a different thing | Keep; register the ids. Fix the spec's name clash |
| `coder host`: `openagents.host-request.v1`, `openagents.host-reply.v1` in signed kind 3188 events | `coax/lib/nostr/host/browser.coax:13-59` | Paired browser | `host/v1/host.proto` has a different shape | External (NIP-HOST is the definition). Golden from the NIP's examples. The spec decides what `host.v1` is for |
| System One request JSON | `coax/lib/systemone/request.coax` | Psionic's `/v1/systemone` | `decision/v1alpha1/systemone.proto`, used on the server only | Pin. The JSON contract is the definition (Psionic book chapter 21) |
| Claude Code stream-json and control frames; Codex app-server JSON-RPC; ACP (client and `coder acp`); MCP; hook output; usage APIs; other tools' transcripts; provider request bodies | `coax/lib/agent/claude/`, `codex/`, `cursor/`, `coder/tui/acp.coax`, `mcp.coax`, `usage.coax`, `coax/lib/gateway/` | The other party | theirs | External. Register each with its fixture |
| Credentials (`NAME.env`), plugin manifests (plain or a signed NIP-78 event) | `coder/tui/credential.coax`, `plugins.coax` | Coder only | none / Nostr | Keep / external |

### Steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| C1 | [#13238](https://github.com/OpenAgentsInc/openagents/issues/13238) | Session log schema and one writer. Package `openagents.session.log.v1alpha1`; the id on disk stays `openagents.session.v1` | Done. Every writer calls `log.coax`. The header always has `schema`. `proto/coder/session-log-v1.json` holds golden lines and its check passes. Old files still load, fork and replay ([session-log-v1.md](session-log-v1.md)) |
| C2 | [#13239](https://github.com/OpenAgentsInc/openagents/issues/13239) | One reader. Move the ten other reader files (fork, coder, host, acp, session, router, remote, run, memory and the Aethyrforge chat) to `sl_load` or a streaming reader beside it; test `chat.record` SQL against C1's fixtures; stop writing `diff` | Done, but for `diff`. `log.coax` reads the log (`sl_next`, `sl_read`, `sl_load`) for every reader file; the stream records beside its lines stay C3's. `test_zchat.py` reads every line of `GET /api/chat/record` through it and requires it back byte for byte (0162 makes the SQL leave out an empty title, model and detail, as the writer does). `wire-check` shows one codec for the id and no allowlist line. Coder still writes `diff`: its tool line carries only a label, so the ACP, host and Aethyrforge views of an edit need the `tool` line's `old` and `new` first, and the bytes on disk change; that moves to wave 2 (#13250) |
| C3 | [#13240](https://github.com/OpenAgentsInc/openagents/issues/13240) | Stream records: reply-stream, observability v1 and v2, route v2. Fix the protos to the JSON, add a profile, one codec file | Done. `coax/lib/agent/session/stream.coax` writes all five records and the Aethyrforge chat reads them through it (`cst_read`). Profile `coder-stream`: `observability/v1/events.proto`, `v2/events.proto`, `stream/v1/reply.proto` and `route/v2/route.proto`, each `json_name` the wire key. `stream-goldens.coax` writes each line of `proto/coder/stream-v1.json` through the codec, holds every key to a `json_name` in the descriptor, and reads and writes `availability`, `attempt`, `reply_delta` and `reply_end` through the JSON mapper too (`model`, `usage` and `route` carry a double, which the mapper refuses). The two JSON Schema files are deleted, and `wire-check` has no allowlist line for these ids |
| C4 | [#13241](https://github.com/OpenAgentsInc/openagents/issues/13241) | Qualification evidence: one `coder.execution.v1` encoder, profile, goldens | Committed receipts under `docs/verification/` re-encode to the same bytes |
| C5 | [#13242](https://github.com/OpenAgentsInc/openagents/issues/13242) | Hosted runner: `openagents.agent.managed.v1alpha1` events, runner lines and end bodies in one codec that `turn.coax`, `chat.coax` and the C runner share | Done. `proto/openagents/agent/managed/v1alpha1/session.proto` (profile `agent-managed`) holds only what ships: the engine's lines, `EventBatch`, `TurnEnd`, `NextTurn` and the sealed credential. `coax/web/agent/runner-wire.coax` is the one writer: `turn.coax` and `chat.coax` load it (`engine-line.coax`; `en_event`, `ch_event` and their kin are deleted), and `runner.c` and `chat-runner.c` call it through `runner-wire.c` instead of `snprintf` (NIP-98 signing stays). `proto/coder/agent-managed-v1alpha1.json` holds lines and bodies from the old writers, byte for byte; `runner-wire-goldens.coax` writes each through the codec, holds every key to a `json_name`, reads `EventLine`, `StopLine` and `EventBatch` back through the JSON mapper (the rest carry a double) and seals the credential golden with `aes_gcm.coax`. `test_zchat.py` posts the batches and /chat end bodies to the API for the goldens' `chat.events` rows and checks the runner's OpenSSL seal against the same golden; `wire-check` has no allowlist line for these shapes |
| C6 | [#13243](https://github.com/OpenAgentsInc/openagents/issues/13243) | Golemancy records: AI-call ledger, next-issue decision, tier rubric; one price table | Done. `proto/openagents/coder/golemancy/v1alpha1/records.proto` (profile `golemancy`); `coax/lib/agent/ledger.coax` is the one writer and reader of both records, the decision through the JSON mapper, the AI-call line by hand (its `model_usage` is Claude Code's object, with floats and model-name keys). `proto/coder/golemancy-v1alpha1.json` holds lines from both of the old encoders; every line of the ledger and decisions files on the owner's Mac reads and writes back byte for byte. A check in `ledger.coax` holds `tier.coax`'s rubric properties to the `Rubric` message. `gs_price` is deleted: the Golems board prices from `coax/lib/gateway/card.coax` (`gw_rate_family`), and its totals over 94 recorded agents are the same to the hundred-millionth |

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
| Session v5 text frames (`P H I C E R K t T O A`, `W D s S x n e w o k r g`, rest and kit records) | `apps/openagents-web/everglade-wire.coax` | Page (`everglade-net.coax`; `everglade/client/net.coax` through `everglade/client/net-core.coax`) and room (`coax/web/room/room-everglade.coax`, through `everglade-link-wire.coax`) load the same file | Profile `proto/verse/everglade-session-v5.json`. Its check covers `P H I C E R L Z T O K V X`, `y j l t`, `W D s x n`, `e cast/hit/stop/town/xp/level/rc/wm/mend/pburn/pgone`, `w r z g o v`; not the page's `t`, `A`, packed `S`, `k`, kit `b` or `e heal/die/break/burn/rebuild/edit` | Pin. Registered as compliant for the frames the profile holds. 30 Hz, 1,000 bytes a seat a tick, packed rows under half the text size: a hand-packed codec is justified |
| Session frames outside the codec and the profile: `g`, `v`, `e xp`, `e level`, `e rc` (`room-events.coax`, read in `everglade-events.coax:174-218`); `e wm`, `e mend`, `e pburn`, `e pgone` (`room-everglade.coax:1342-1420`, read in `everglade-events.coax` and `everglade-net.coax:891-906`); `V LOOK KIND` (`everglade/client/net.coax:791`); admin `X` lines (`everglade/client/admin.coax`, `room-e2-admin.coax:39`); `K` read by hand in `room-blocks.coax:133`; `e break`, `e burn` and `w` hand-built in `room-everglade.coax:1379-1786` though the codec has writers | One codec, `everglade/shared/everglade-wire.coax`: the room (`progress.coax`, `send.coax`, `input.coax`, `rest.coax`, `admin.coax`, `room-blocks.coax`) and the pages (`everglade/client/net.coax`, `everglade-events.coax`, `everglade-net.coax`, `everglade/client/admin.coax`) call it | `everglade-session-v5.json` | Pinned: E3b ([#13364](https://github.com/OpenAgentsInc/openagents/issues/13364)) |
| Relay frames `y j l t m` and the rooms JSON (`rm_rooms`) | Written in `coax/web/room/room-relay.coax` (`rl_head`, `rl_queue`); parsed in `everglade/client/net.coax:452`, `everglade-net.coax:545-626`, `lockroom.coax:361`, `oneshot.coax:363`. No codec file reads or writes them | Relay, pages | `y j l t` are in the v5 profile; `m` and the JSON are not | Pin. One shared reader and writer (E2) |
| Link input v1 (`J1`) | `apps/openagents-web/everglade-link-wire.coax` | Page (`everglade-net.coax:1092`), room | `proto/verse/everglade-link-input-v1.json`, unchecked | Pin: E1 adds its check |
| Everglade II familiar frames `M1 M3-M7`, `m1-m8`, snapshot `m2` | Room: `room-e2-familiar.coax`. Page: `everglade/client/net.coax:406-441,1062-1085` | Two hand-written codecs | Seven JSON files, unchecked | Pin: E2. One shared codec |
| Healer's Kit `U1`, `u1` | `room-e2-aid.coax:14,91`; `everglade/client/net.coax:444,876` | Two codecs | JSON, unchecked | Pin: E2 |
| Path guard `g2`, `g3`; monsters `h2`, `h3` | `room-e2-guard.coax:377` (`g3`), `:398` (`g2`); `room-e2-monsters.coax:321,478` (`h3`), `:501` (`h2`); `everglade/client/net.coax:407-410` | One codec, `everglade/server/creature-wire.coax` | `everglade2-combat-v1.json` | Pinned: E3 |
| Projectiles `F a d` | `aethyr/net/projectile.coax`, `record.coax` | Shared | `everglade2-combat-v1.json` | Pinned: E3 |
| Latency observation `@1` | `aethyr/net/latency.coax` | Shared | Profile, checked by `coax/web/room/latency-profile.coax` | Pin. Compliant |
| WebTransport framing (mark byte, channels, 1,200-byte datagrams) | `aethyr/net/transport.coax`, `room-wt.coax` | Shared | `aethyr-transport-v1.json` | Pinned: E3 |
| Old wire (`H 1`) | `coax/web/room/room-oldwire.coax` | Ends 2026-11-12 | none | Delete: E1 |
| Session v4 profile | `proto/verse/everglade-session-v4.json` | No code reads it | — | Delete: E1 |
| Orbit League v2 frames | `apps/openagents-web/orbit-session.coax` (with `orbit-snapshot.coax`) | Shared | Profile, checked by `orbit-wire-profile.coax` | Pin. Compliant |
| Seat ticket v2/v3 (HMAC-SHA256) | Written by C `snprintf` in `coax/web/api/player.c:524` and by Coax `tk_sign*` in `room-ticket.coax:230-268` (the dev stand-in and the checks); read by `room-ticket.coax` `tk_check` | Two writers, two languages | Fixture in the v5 profile | Pin: E5. One Coax writer that C calls (`tk_sign_owner` can be it). Signed: golden |
| Ticket v4 resume envelope (signed) | `aethyr/session/resume-payload.coax`, `resume-envelope.coax` | Shared, exported to C | none | Pin: E5 adds fixtures |
| Character checkpoint v2/v3 (signed, stored in `verse.character_checkpoints_v2`) | `aethyr/session/character-checkpoint.coax`; SQL rewrites v3 to v2 at `player.c:350`; v1 keys duplicated in `coax/web/api/checkpoint-policy.coax` | Room, API | JSON Schema, unchecked | Pin: wave 2 (E7). Golden for the SQL rewrite |
| Room reports to the API (vitals, directory, stats, the checkpoint post and its receipt; signed) and the control poll reply | `room-vitals.coax`, `room-directory.coax`, `room-stats.h` (the body; `room-stats.coax` signs it), `room-checkpoint.coax:221-246`, `room-control.coax`; `player.c`, `rooms.c` | Coax room, C API | none | Pin: wave 2 (E8) |
| Item ledger op | Route `room_items` at `player.c:706`. `aethyr/session/ledger.coax` is a store model with no codec for the op; only a test loads it | No room sends it | none | Wave 2: wire it or delete it |
| `room-state {look,class}` | `player.c:624-652` (`room_state`) | Kept: look and class go into `verse.room_vitals` and come back in the ticket reply (`:543`); only `wanted` is refused. Sent by `everglade-net.coax` and `everglade5e-face.c:82`. Only the local stand-in drops it (`room-dev.coax:214`) | none | Move: wave 2 (E11), with the other player routes |
| Player appearance `aethyr.player.appearance.v1` (CC2, CC3, CC4 text) | `aethyr/character-appearance.coax:59,72` (the reader also takes `CC1`) | Stored in localStorage (`coax/web/host.js:570`) and the iOS store (`aethyr/host/apple/PageHost.m`) | none | Move: wave 2 (E9), before sync stores it on the server. Golden for each version |
| Realm save AESV (CRC-32), stored in IndexedDB and `verse.progress` | `apps/openagents-web/save.coax`; a second validator in C (`container_ok`, `player.c:94`) | Page, API | none | Pin: wave 2 (E10). Golden |
| Realm document `aethyr.realm` v1 (SHA-256 per revision) | Built in SQL (`0093`, `0109`, `0111`). `aethyr/forge/doc.coax` reads it, but only examples, the book and `coax/web/api/tests/realm_walkthrough.coax` load that file | `apps/openagents-cli/realm.coax:108-122` parses `/doc` with its own JSON reads; `myrealm.coax:436` reads the spec's text lines, not the document; API | `verse-doc` | Done: E6. `doc.coax` is the one reader and writer; `realm-document-v1.json` |
| Edit operations and the undo log `{to,card,ops:[{slot,before,after}]}`; save body `{rev,moved,shaped}` | `aethyr/forge/edit.coax:339-514` holds the operations in memory and builds the save body (`forge_save_body`, `:479`). The undo log JSON is built only in plpgsql (`0111:181`) | Aethyrforge, My Realm, API | `verse-doc` | Done: E6. `edit.coax` writes the bodies; `doc.coax` applies the logged steps |
| Engine pin manifest `aethyr-engine 1` (hashed) | `aethyr/forge/publish.coax`; again in SQL (`0107`); read by `coax/web/engine-served.coax` | A fixture pair: `publish.coax:242` and `coax/web/api/tests/realm_engine_pin.sql` | none | Done: E6, row `engine-pin-manifest-v1` with its check |
| Realm API routes (`plan`, `pick`, `settings`, `switch`, `revise`, `fork`, `publish`, `visibility`, `place`, `lore`, `quest`, `asks`, text lists) | `coax/web/api/realms.c:7-60` | Site pages, CLI | none | Move: wave 2 (E11), on the `api.v1` envelope |
| Everglade city edits `eg-edit 1` | `verse/everglade/edits.coax:13` | `everglade-edit.coax`, `apps/aethyrforge-desktop/scene-cli.coax`, room (`room-world.c`) | none | Pin: wave 2 |
| NPC ask `{npc,world,history,question,controls,visit}` and `{text,model}` | Was four client encoders (`people.coax`, `fairy-forest/talk.coax`, `realm.coax`, `myrealm.coax`); since E4 one codec, `aethyr/talk/wire.coax`, that all four call. Server: `coax/web/api/npc.c` and SQL `verse.npc_ask` | Pages, API | `openagents.verse.npc.v1alpha1` | Done: E4 |
| WorldBuilder: act request, reply lines `S`, `A op\|…`, save `WB1` | `apps/openagents-web/worldbuilder*.coax`, `coax/web/api/worldbuilder.c`, SQL `worldbuilder.lines` | One page | none | Move: wave 2 (E12), after #13226 puts its edits on the forge log. Its operations become E6's |
| Aethyrforge local commands (scene v1/v2, NPC, raid), `aethyrforge.scene-session.v1` and `aethyrforge.npcs.v1` | `apps/aethyrforge-desktop/scene-policy.coax` (`fs_command`), `scene-session.coax` (the session record; `Sources/app.m` writes it through `fs_session_put`), `aethyr/forge/npc-command.coax`, `raid-command.coax`, `npc-document.coax` | Local tools | JSON Schemas, each with a check | Pinned: E1 (#13244) |
| Aethyrforge voice input `aethyrforge.voice-coder.v1` and `aethyrforge.microphone-capture.v1` | Written in `apps/aethyrforge-desktop/Sources/app.m:282-288`; read in `voice-coder.coax:69` and `app.m:1035` | Aethyrforge | `proto/coder/voice-input-v1.schema.json`, `microphone-capture-v1.schema.json`, unchecked | Pin: E1 adds checks or deletes them |
| Physics lab record | `aethyr/lab/physics-record.coax` | The lab | `proto/aethyr/physics-lab-v1.json`; the file's tests use their own literals, not the profile | Pin: give it a check or delete it (tracker) |
| Asset packs VPK1, VPK2, VPK3 (VPK3 hashed) | `aethyr/forge/pack.coax`, `page-pack.coax`; also `verse/tools/pack_realm.py`, a JS reader at `coax/web/host.js:1931`, and many more readers and writers: at least 16 files under `verse/tools`, `PageHost.m`, `Engine.swift`, `apps/everglade-mobile/android/host.c`, `pack-host.js`, `pack-fetch.js`, `pack-face.c`, `verse/native/realm-host.c`, `drops.c`, `verse_packs.c`, `room-catalog.coax` | Build, page, room, native hosts | `proto/verse/asset-pack-v2.json`, unchecked | Pin: wave 2. Hand-packed is right for a mapped file. One writer and one reader in each language is the goal; today it is far from that |
| Presence and room chat JSON (`/api/playtest/presence`, `chat`, `inbox`) | `coax/web/api/room.c` | Older realms | `verse/presence/v1alpha1`, unused | Wave 2: decide with the room servers' reach; don't build on the unused proto before then |
| Player, world, progress, seats and packs routes | `player.c`, `seats.c`, `verse_packs.c` | Pages | none | Wave 2 (E11) |
| Zone geometry and logic | `everglade/zone.coax`, `everglade/world/` | Compiled in | — | Keep. Code, not a wire. The `P` frame's content pin covers it |
| SRD rules data | `aethyr/rules/srd/data/*.coax`, generated from the pinned PDF | Compiled in; only indices travel | — | Keep. The `P` frame's rules pin covers it |
| Haunted House save and scene | `verse/haunt.coax` | Generated codec | `metareal/haunt/v1alpha1` | Compliant |
| `engine/v1` (scene, agent, control, state, assets), `verse/presence`, `verse/common`, `host/v1`, `reach/v1` | `proto/openagents/` | Tests and tools only. `hud.proto` is not in this row: `verse/home.coax` writes its `HudInvocation` | published | Keep frozen. Marked `no-consumer` in the registry and in `proto/README.md` |

### Steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| E1 | [#13244](https://github.com/OpenAgentsInc/openagents/issues/13244) | Clean the profile family: delete the v4 profile and `room-oldwire.coax`; add a check for every unchecked profile and schema under `proto/verse/` and `proto/aethyrforge/`, or delete it | Every file under those folders names a check that passes. `proto/README.md` lists the family |
| E2 | [#13245](https://github.com/OpenAgentsInc/openagents/issues/13245) | Familiar and Healer's Kit frames in one shared codec file that room and page load; profiles with golden frames | `room-e2-familiar.coax`, `room-e2-aid.coax` and `everglade/client/net.coax` hold no frame text of their own. The profile check passes |
| E3 | [#13246](https://github.com/OpenAgentsInc/openagents/issues/13246) | Guard, monster and projectile frames, the session frames that are outside the codec today (`g`, `v`, `e xp/level/rc/wm/mend/pburn/pgone/break/burn`, `w`, `V`, `X`, the `K` reader) and the WebTransport framing: shared codec, profile, doc | Same as E2 for `g2 g3 h2 h3 F a d` and those frames: `room-everglade.coax`, `room-events.coax`, `room-e2-admin.coax` and `room-blocks.coax` hold no frame text of their own. `docs/protocol/` describes them. The session frames are E3b ([#13364](https://github.com/OpenAgentsInc/openagents/issues/13364)): done; `everglade-wire.coax` writes and reads them on both sides and `everglade-session-v5.json` pins them ([everglade-session-v5.md](everglade-session-v5.md)) |
| E4 | [#13247](https://github.com/OpenAgentsInc/openagents/issues/13247) | NPC ask: package `openagents.verse.npc.v1alpha1`; the four clients use `aethyr/talk/wire.coax`; a SQL test on the same fixtures | Done. `proto/openagents/verse/npc/v1alpha1/ask.proto` (profile `verse-npc`); `aethyr/talk/wire.coax` writes the ask and reads the answer for Everglade, Fairy Forest, the Haunted House and made realms (a hand codec: history is written even when empty). `proto/verse/npc-ask-v1.json` holds the pages' asks, refused bodies and the API's answers; `aethyr/talk/wire-goldens.coax` writes and reads each through the codec, and `test_npc.py` posts each to the API and gets its status. `wire-check` has no allowlist line for it |
| E5 | [#13248](https://github.com/OpenAgentsInc/openagents/issues/13248) | Seat ticket: one Coax writer that `player.c` calls; fixtures for v2, v3 and the v4 envelope | No `snprintf` builds a ticket. A ticket the writer makes passes `tk_check`; goldens match byte for byte |
| E6 | [#13249](https://github.com/OpenAgentsInc/openagents/issues/13249) | Realm document and edit operations: package `openagents.verse.doc.v1alpha1`; fixtures shared by `aethyr/forge/` and the SQL | Done. `proto/openagents/verse/doc/v1alpha1/realm.proto` (profile `verse-doc`). `proto/verse/realm-document-v1.json` holds the stored bytes: a made realm, ten steps (moves, turns, sizes, a removal, lore, a quest, a restore of every shape, two undos, a redo, a stale save), and after each the document, its SHA-256, the reply and the logged undo step. `aethyr/forge/doc-goldens.coax` replays it through `edit.coax` and `doc.coax` (`forge_doc_json` writes jsonb's text; `forge_apply_step` applies the server's logged steps; replies and keys checked against the descriptor, the small replies through the JSON mapper too), and `coax/web/api/tests/realm_doc_checks.coax` replays it through the routes. My Realm and the CLI build and read the save, undo and redo bodies and replies through `edit.coax` and `doc.coax`. `coax/web/cli-journey-fixture.coax`, the CLI journey's fake server, answers in the fixture's shapes: it reads `realm-document-v1.json` as data (the toolchain context loads no `aethyr/forge/` code) and writes its save, undo and stale replies and its document from them (#13354). Follow-ups (#13354): the edit model names board rows `feature:N` and `row:N` as the server's log does, with `forge_switch` the server's switch and a switch step in the fixture; moves and shapes stand in entity order (migration 0166), so an undo or redo gives back the restored revision's bytes but for `rev` (the fixture's `restores`). The save now always sends `shaped`: without it, restoring the last turned, sized or removed thing was never saved. The engine pin manifest has its row and check. Rows `realm-document-v1`, `realm-edit-v1`, `engine-pin-manifest-v1`; no allowlist line |

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
| The `diff` session line | wave 2 (#13250; C2 kept writing it, see C2) |
| The item ledger route if no room sends to it (`room-state` stays: the API keeps the look and class) | wave 2 |
| `aethyr/talk/wire.coax`'s "example only" status: it becomes the codec | E4: done |

## Enforcement

One tool, `coax/web/tool.sh wire-check` (`coax/web/wire-check.coax`), written
in Coax beside `everglade-parity.coax` and `components-check.coax`. It runs in
about three seconds and needs no build. The after-landing check runs it with
the other report checks (`coax/remote/check.sh`), so `main` goes red on a new
unregistered shape.

`proto/registry.json` has one row per shape: `id`, `kind` (`proto`, `profile`,
`external`, `local`), `schema` (a proto file or a profile file), `codec` (the
one file that may write the id), `check` (a command) and `step`. Optional:
`files`, `tests`, `sql`, `status` (`pending`, `no-consumer`) and `note`.
`proto/README.md` describes each field.

The check fails when:

1. A schema-id literal in code (`"NAME.vN"` after a `schema` key, or a whole
   string on a line that names a schema) is in no registry row. All tracked
   code is scanned, not only the two areas.
2. A registered id is in a file other than its `codec`, its tests or its
   fixtures. A reader that compares the id counts: it is a second parser.
3. A `coax/web/api/*.c` file with a `*_route` function, or a
   `coax/web/room/room-*.coax` file, is in neither the registry nor the
   allowlist. "Writes frames" has no test a tool can run, so every room file
   is accounted for: 20 have frames of their own and are on the allowlist, and
   20 hold no wire bytes and are in the registry's `no_frames` list. A new
   room file must go in one or the other.
4. A file under `proto/coder/`, `proto/verse/`, `proto/aethyrforge/` or
   `proto/aethyr/` has no registry row with a `check`.
5. A `.proto` under `proto/openagents/` is in no `modules.json` profile and not
   marked `no-consumer` in the registry.
6. `proto/wire-allowlist.txt` has a line that the base's copy lacks
   (`origin/main`, or the commit `--base` names; the after-landing check
   passes the last green commit) and the line carries no reason and issue
   number. The allowlist only shrinks.

It also fails when the registry or the allowlist is not true: a row names a
file that is gone, a `check` names a missing file, or an allowlist line no
longer excuses anything.

What S1 seeded:

| What | Count |
| --- | --- |
| Rows that comply today | 8: session v5, latency v1, Orbit v2, account agents, Haunted House, voice, the iOS home, System One's protobuf door. Each row's check was run and passes |
| Pending rows: the id and its intended codec, with rule 2 already on | 21, each with a `shape` line |
| Rows marked `no-consumer` | 4: `engine/v1` (not `hud.proto`), `verse/presence`, `verse/common`, `host/v1` with `reach/v1` |
| External rows | 12 |
| Local rows from this plan's "Keep" decisions | 9 |
| Rows outside the two areas, registered but not audited (`"step": "unaudited"`) | 42: ten families of evidence and tool records (`psionic.*`, `coax.*`, `aethyr.host.*` and so on) and 32 single ids |
| Allowlist lines | 141: 28 `shape` lines without a row, 21 for pending rows, 14 `id`, 25 `route`, 20 `room`, 22 `profile`, 11 `proto` |

Each later step removes its lines from the allowlist. `AGENTS.md` lists the
command under "Build and test".

### Register a shape

1. Write the schema in `proto/`: a `.proto` message in a `modules.json`
   profile, or a JSON profile for a hand-packed frame.
2. Write the one codec file and move every producer and consumer onto it.
3. Add the fixture and the check that compares the codec's bytes with it.
4. Add or finish the row in `proto/registry.json`: `schema`, `codec`, `check`,
   and `files` for the API route files and room files that now carry the
   shape through the codec. Take `"status": "pending"` out.
5. Delete the lines the step owns from `proto/wire-allowlist.txt`.
6. Run `coax/web/tool.sh wire-check`. It names each file that still carries
   the id and each line that no longer excuses anything.

[`proto/README.md`](../../proto/README.md#register-a-wire-shape) has the row's
fields and an example.

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

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
| 1 | "The Everglade wire profiles already use generated codecs and are the pattern" | No file under `metaverse-engine/`, `everglade/`, `coax/web/room/` or `apps/everglade*` has an `#import_proto`. The session wire is hand-packed text frames in `apps/openagents-web/everglade-wire.coax`. A JSON profile, `proto/verse/everglade-session-v5.json`, pins it frame by frame through `everglade-wire-profile.coax` | The pattern is real and good, but it is "one shared codec plus a profile with golden bytes", not a generated codec. This plan names that pattern and uses it |
| 2 | Each step: "generate the Coax codec … keep the JSON the same bytes" | `#import_proto` generates binary codecs only. `coax/proto.md` lists JSON conversion as separate work, and `coax/lib/proto/importer.coax` emits no JSON mapper. Almost every shape in both areas is JSON or text | The recipe can't run for JSON today. This plan adds the mapper as its own step and does not block on it |
| 3 | "Chat already writes `openagents.session.v1` JSON lines, which become its first generated mapper" (spec section 17) | The id on disk names the transcript log (`coax/lib/agent/session/log.coax`). The spec's `openagents.session.v1` (section 18.3, not 17) is NIP-SESS: adapters, commands and phases. They share a name and nothing else | Define the log in its own package and fix the spec's name clash |
| 4 | Not mentioned | `proto/` holds a second schema family: 25 JSON files under `proto/coder/`, `proto/verse/`, `proto/aethyrforge/` and `proto/aethyr/`. `modules.json` doesn't list them. 22 have no checker: only session v5, Orbit v2 and latency v1 have a check that passes, and the v4 profile's own check would fail | Register them, check them or delete them |
| 5 | Not mentioned | Protos exist that nothing uses. The five Coder and CLI protos (`coder/observability/v1`, `v2`, `coder/qualification/v1`, `cli/local/v1`, `cli/dictation/v1`) are in no profile (only in the catch-all `owned` module and its `owned.pb`), no code imports them, and they don't match the JSON that ships (`inputTokens` on the wire, `input_tokens` in the proto; no `kind`, `schema` or `usd_micros`). `engine/v1` (except `hud.proto`'s `HudAction` and `HudInvocation`, which `metaverse/home.coax` writes), `metaverse/presence`, `metaverse/common` and `host`/`reach` (a library adapter that only tests and an example load) have no product consumer | A proto with no consumer and no check is not compliance. Each step must end with a check that ties the schema to the bytes |
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
| Generated binary (`#import_proto`) | New binary bodies and files | `agent-wire.coax`, `metaverse/haunt.coax` |
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
| Session log `openagents.session.v1` (header, `user`, `reply`, `thinking`, `tool`, `result`, `routed`, `notice`, `ask`, `end`), stored in `~/.openagents/coder/sessions/ID.jsonl` | `coax/lib/agent/session/log.coax` | Before C1 there were four header writers in two shapes: `log.coax` `sl_header` (the desktop app, through `apps/openagents-desktop/zeron/store.coax`), `session/writer.coax` `sw_open` (Coder and the turn runner: no `schema` key, more fields), `coder/tui/import.coax` `ci_import_claude`, and SQL `0139_chat_folders_tabs.sql` `chat.record`; `end` lines in `log.coax`, `writer.coax`, `coder/tui/coder.coax` (`coder stop`), `router.coax` (the watchdog), `import.coax`; `tool` and `result` in `log.coax`, `writer.coax` and `import.coax`. After C1 (#13238) `log.coax` writes every line (`sl_session` ... `sl_end`) and the header always has `schema`; only the SQL is left (C2). Readers (each parses `kind` itself; none compares the id): `log.coax` `sl_load` (through `zeron/store.coax`), `session/fork.coax` (`sf_fork`, `sf_turns_text`), `coder/tui/coder.coax` (`cd_show_past`, `cd_render` for `coder log`, `cd_cmd_replay`), `coder/tui/host.coax` (`hs_step`), `coder/tui/acp.coax` (`cd_acp_line` and the turn reader), `coder/tui/session.coax` (`cs_read`), `coder/tui/router.coax` (three readers), `coder/tui/remote.coax`, `engine/run.coax` (`tr_fork`), `apps/metaverse-engine-desktop/chat.coax` (`fc_read`), and `coder/tui/memory.coax`, which finds user lines by their first bytes (`{"kind":"user"`). `coax/lib/agent/observe/transcripts.coax` only lists the files. | Proto `openagents.session.log.v1alpha1` (profile `session-log`); spec 18.3 is a different thing | Writer: compliant after C1. Readers: one, `log.coax` (C2) |
| Other session lines: `request` (SHA-256 of the prompt, checked by `coder replay`), `memory`, `engine_session`, `agent`, `context`, `keys`, `note`, `hook`, `goal`, `job`, `answer`, `approval` (two encoders before C1: `engine/run.coax` `tr_approval`, `coder/tui/approvals.coax`), `diff` (legacy) | `session/log.coax` since C1; `session/writer.coax`, `coder/tui/coder.coax` and `coder/tui/session.coax` call it | Coder only | In the session log's proto | Moved in C1, with a golden for `request`. Stop writing `diff` in C2. Microcoder's `journal` line rides in the same file and is C11's |
| Session sibling files: `ID.pid`, `ID.stop`, `ID.turnstop`, `ID.answer`, `ID.inbox/*.msg`, `ID.acp`, `ID.remote` | `coder/tui/coder.coax`, `acp.coax`, `remote.coax` | Coder only, same machine | none | Keep. Document in the session log's doc |
| `coder.reply-stream.v1` (`reply_delta`, `reply_end`) | `session/writer.coax:361` | Reader: `apps/metaverse-engine-desktop/chat.coax:141` | none; JSON Schema `proto/coder/reply-stream-v1.schema.json`, unchecked | Move: C3 |
| `coder.observability.v1` (`model`, `usage`, `availability`) | `session/writer.coax:410-435` (`model`, two `usage` encoders); `availability` at `engine/run.coax:304` and `coder/tui/availability.coax:10` | Session readers, `coder usage` (none compares the id) | `coder/observability/v1/events.proto`, unused, fields differ | Move: C3. Fix the proto to the JSON |
| `coder.observability.v2` (`attempt`) | `engine/micro-observation.coax:26` | `coder/tui/mcworker.coax`, `microcoder.coax`, `run.coax` | `v2/events.proto`, unused, lacks `usd_micros` | Move: C3 |
| `coder.route.v2` | `coax/lib/agent/engine/route.coax:287` (`agr_json`) | Session readers (by `kind`; none compares the id) | none; JSON Schema `proto/coder/route-v2.schema.json`, unchecked | Move: C3 |
| `--json` and `--stream-json` output (`usage`, `ls`, `status`, `import`, `run --detach`, `stop`, `replay`, `bench`, `jobs`, `queue`, credentials, plugins) | `coder/tui/coder.coax` (plugins at `:1645`), `daemon.coax`, `credential.coax`, `host.coax:893` | Scripts, the OpenAgents Metaverse Engine, Golems | none | Move: wave 2 (C8). `--stream-json` is the session lines plus C3 |
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
| Golems cache `golem5`, snapshot lines `T G M Q`, `issues.tsv`, `hosts.txt` | `coder/golemancy/golems.coax:40-62,471-499` | `golem-board.coax`, same app | none | Keep. A cache one app writes and reads; the name carries its version |
| Remote-host pull | `golems.coax:736` (`ssh find`, `rsync`) | Copies other tools' transcripts | none | External. No shape of ours |
| Issue claim file `everglade-owner` (a pid) | `next-context.coax:430` | The Workshop | none | Keep |
| `openagents.coder.agents.v1`, `openagents.models.v1` | `coax/lib/agent/observe/processes.coax:50`, `apps/openagents-cli/models.coax:38` | `apps/metaverse-engine-desktop/setup.coax` | `cli/local/v1/inventory.proto`, unused | Move: wave 2 (C7) |
| `openagents.dictation.v1`, `openagents.dictation.stream.v1` | `apps/openagents-cli/dictate.coax:99`, `dictate-stream.coax:40` | `coder/tui/dictate.coax:173` reads `openagents.dictation.v1` only; the stream id has no reader outside its test | `cli/dictation/v1/stream.proto`, unused; it covers the stream id only | Move: wave 2 (C7) |
| `coder.session-reader.v1` envelope | One writer file since 2026-11-13: `coax/lib/agent/observe/claude-history.coax` holds every opening of the envelope (`csr_error`, `csr_sessions_open`, `csr_ambiguous_open` and the session's own); `coder/tui/session-reader.coax` calls them and names no id | `coder mcp`, the Codex tool, `coder session-read` | `coder/session_reader/v1/reader.proto` (profile `coder-session-reader`) | Done: C10. `proto/coder/session-reader-v1.json` holds 14 replies the CLI printed and the transcripts it read; `session-reader-goldens.coax` holds each reply's keys to its message and requires the writer's bytes. [coder-session-reader-v1.md](coder-session-reader-v1.md) |
| Microcoder worker lines (`ready`, `reply`, `attempt`, `command`, `view`, `ask`, `ended`, `embedding`, `journal`) | `engine/mcworker-client.coax:80`, `coder/tui/mcworker.coax:50-160` | Coder and its worker | none | Move: wave 2 (C11) |
| Microcoder journal AJR1 (binary, hash-chained), `sessions/worker-US.ajr` | `coax/lib/agent/journal.coax:182` | Worker, replay | none | Pinned: C11. `proto/coder/microcoder-journal-ajr1.json` holds a chain of 11 frames, one of each kind, made with Python's hashlib, and nine refused frames; `coax/lib/agent/journal-profile.coax` holds `aj_prepare` and `aj_read` to them. [microcoder-journal-ajr1.md](microcoder-journal-ajr1.md) |
| Microcoder prompt state and action schema | `coax/lib/microcoder/state.coax:333`, `action.coax` | Sent to the model; the prefix must stay byte-stable for the prompt cache | none | Pinned: C11. `proto/coder/microcoder-prompt-v1.json` holds the system prompt, the schema and the state after each of six steps; `coax/lib/microcoder/prompt-profile.coax` requires them byte for byte and requires each state to begin with the one before it. [microcoder-prompt-v1.md](microcoder-prompt-v1.md) |
| Consent ledger and memory log (hash-chained JSON lines) | `engine/consent-gate.coax` (`gt_line`, `gt_unline`), `engine/memory-log.coax` (`mem_line`, `mem_unline`; moved from `coder/tui/memory.coax`) | Coder (`engine/run.coax`, `coder/tui/gate.coax`) | none | Pinned: C12. `proto/coder/consent-memory-ledgers-v1.json` holds ledgers the Coder CLI wrote; `coax/lib/agent/engine/ledger-profile.coax` reads and writes each line back and replays each chain. [consent-memory-ledgers-v1.md](consent-memory-ledgers-v1.md) |
| Router and daemon files (`router/log.jsonl`, `router/sources.json`, `outbox/`, `daemon.json`, `jobs/`) and `router.json` as `coder.router.v1` | `coder/tui/router.coax`, `daemon.coax`; `router.json` is read at `coax/lib/agent/engine/route.coax:195` and its local default is the same file's (`agr_router_local`, which the Metaverse Engine's chat calls) | Coder; `router.json` also Metaverse Engine | none; the spec's `coder.router.v1` is a different thing | Keep; register the ids. Fix the spec's name clash |
| `coder host`: `openagents.host-request.v1`, `openagents.host-reply.v1` in signed kind 3188 events | `coax/lib/nostr/host/browser.coax:13-59` | Paired browser | `host/v1/host.proto` has a different shape | External (NIP-HOST is the definition). Golden from the NIP's examples. The spec decides what `host.v1` is for |
| System One request JSON | `coax/lib/systemone/request.coax` | Psionic's `/v1/systemone` | `decision/v1alpha1/systemone.proto`, used on the server only | Pin. The JSON contract is the definition (Psionic book chapter 21) |
| Claude Code stream-json and control frames; Codex app-server JSON-RPC; ACP (client and `coder acp`); MCP; hook output; usage APIs; other tools' transcripts; provider request bodies | `coax/lib/agent/claude/`, `codex/`, `cursor/`, `coder/tui/acp.coax`, `mcp.coax`, `usage.coax`, `coax/lib/gateway/` | The other party | theirs | External. Register each with its fixture |
| Credentials (`NAME.env`), plugin manifests (plain or a signed NIP-78 event) | `coder/tui/credential.coax`, `plugins.coax` | Coder only | none / Nostr | Keep / external |

### Steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| C1 | [#13238](https://github.com/OpenAgentsInc/openagents/issues/13238) | Session log schema and one writer. Package `openagents.session.log.v1alpha1`; the id on disk stays `openagents.session.v1` | Done. Every writer calls `log.coax`. The header always has `schema`. `proto/coder/session-log-v1.json` holds golden lines and its check passes. Old files still load, fork and replay ([session-log-v1.md](session-log-v1.md)) |
| C2 | [#13239](https://github.com/OpenAgentsInc/openagents/issues/13239) | One reader. Move the ten other reader files (fork, coder, host, acp, session, router, remote, run, memory and the Metaverse Engine chat) to `sl_load` or a streaming reader beside it; test `chat.record` SQL against C1's fixtures; stop writing `diff` | Done, but for `diff`. `log.coax` reads the log (`sl_next`, `sl_read`, `sl_load`) for every reader file; the stream records beside its lines stay C3's. `test_zchat.py` reads every line of `GET /api/chat/record` through it and requires it back byte for byte (0162 makes the SQL leave out an empty title, model and detail, as the writer does). `wire-check` shows one codec for the id and no allowlist line. Coder still writes `diff`: its tool line carries only a label, so the ACP, host and Metaverse Engine views of an edit need the `tool` line's `old` and `new` first, and the bytes on disk change; that moves to wave 2 (#13250) |
| C3 | [#13240](https://github.com/OpenAgentsInc/openagents/issues/13240) | Stream records: reply-stream, observability v1 and v2, route v2. Fix the protos to the JSON, add a profile, one codec file | Done. `coax/lib/agent/session/stream.coax` writes all five records and the Metaverse Engine chat reads them through it (`cst_read`). Profile `coder-stream`: `observability/v1/events.proto`, `v2/events.proto`, `stream/v1/reply.proto` and `route/v2/route.proto`, each `json_name` the wire key. `stream-goldens.coax` writes each line of `proto/coder/stream-v1.json` through the codec, holds every key to a `json_name` in the descriptor, and reads and writes `availability`, `attempt`, `reply_delta` and `reply_end` through the JSON mapper too (`model`, `usage` and `route` carry a double, which the mapper refuses). The two JSON Schema files are deleted, and `wire-check` has no allowlist line for these ids |
| C4 | [#13241](https://github.com/OpenAgentsInc/openagents/issues/13241) | Qualification evidence: one `coder.execution.v1` encoder, profile, goldens | Committed receipts under `docs/verification/` re-encode to the same bytes |
| C5 | [#13242](https://github.com/OpenAgentsInc/openagents/issues/13242) | Hosted runner: `openagents.agent.managed.v1alpha1` events, runner lines and end bodies in one codec that `turn.coax`, `chat.coax` and the C runner share | Done. `proto/openagents/agent/managed/v1alpha1/session.proto` (profile `agent-managed`) holds only what ships: the engine's lines, `EventBatch`, `TurnEnd`, `NextTurn` and the sealed credential. `coax/web/agent/runner-wire.coax` is the one writer: `turn.coax` and `chat.coax` load it (`engine-line.coax`; `en_event`, `ch_event` and their kin are deleted), and `runner.c` and `chat-runner.c` call it through `runner-wire.c` instead of `snprintf` (NIP-98 signing stays). `proto/coder/agent-managed-v1alpha1.json` holds lines and bodies from the old writers, byte for byte; `runner-wire-goldens.coax` writes each through the codec, holds every key to a `json_name`, reads `EventLine`, `StopLine` and `EventBatch` back through the JSON mapper (the rest carry a double) and seals the credential golden with `aes_gcm.coax`. `test_zchat.py` posts the batches and /chat end bodies to the API for the goldens' `chat.events` rows and checks the runner's OpenSSL seal against the same golden; `wire-check` has no allowlist line for these shapes |
| C6 | [#13243](https://github.com/OpenAgentsInc/openagents/issues/13243) | Golemancy records: AI-call ledger, next-issue decision, tier rubric; one price table | Done. `proto/openagents/coder/golemancy/v1alpha1/records.proto` (profile `golemancy`); `coax/lib/agent/ledger.coax` is the one writer and reader of both records, the decision through the JSON mapper, the AI-call line by hand (its `model_usage` is Claude Code's object, with floats and model-name keys). `proto/coder/golemancy-v1alpha1.json` holds lines from both of the old encoders; every line of the ledger and decisions files on the owner's Mac reads and writes back byte for byte. A check in `ledger.coax` holds `tier.coax`'s rubric properties to the `Rubric` message. `gs_price` is deleted: the Golems board prices from `coax/lib/gateway/card.coax` (`gw_rate_family`), and its totals over 94 recorded agents are the same to the hundred-millionth |

Wave 2, in order: C7 local inventory and dictation; C8 `--json` outputs; C9
/chat routes and page lines; C10 session-reader envelope; C11 Microcoder worker
lines, journal and prompt prefix; C12 consent and memory ledgers; C13 register
every external format with a fixture; C14 NIP-98 through `coax/lib/nostr`.

Wave 2 steps done:

| Step | Work | Done |
| --- | --- | --- |
| C7 | Local inventory and dictation | `proto/openagents/cli/local/v1/inventory.proto`, `cli/dictation/v1/stream.proto` and the new `cli/dictation/v1/event.proto` (profile `cli-local`), each `json_name` the wire key. `coax/lib/local/inventory-wire.coax` writes and reads `openagents.coder.agents.v1` and `openagents.models.v1`: `processes.coax` (`la_json`) and `apps/openagents-cli/models.coax` write through it and the Metaverse Engine's setup reads through it. `coax/lib/local/dictation-wire.coax` writes and reads `openagents.dictation.v1` and `openagents.dictation.stream.v1`: `dictate.coax` and `dictate-stream.coax` write through it and Coder's terminal dictation (`coder/tui/dictate.coax`) reads through it. `proto/coder/local-inventory-v1.json` holds reports a build host's CLI printed; `proto/coder/dictation-v1.json` holds 34 lines real runs printed (every type and every status state of both schemas). `coax/lib/local/local-goldens.coax` holds each body's keys to its message, reads it through the codec and writes it back for the same bytes. The CLI built before and after printed the same bytes for six dictation runs, and `dictate-check.coax` (25 cases) and `coder/tui/dictation_check.coax` (13) pass on a build host. The four rows are owned by `toolchain` (the codec is a library every app loads); no allowlist line is left for the four ids |
| C10 | The session reader's envelope | `proto/openagents/coder/session_reader/v1/reader.proto` (profile `coder-session-reader`): `Sessions`, `Conversation`, `Context`, `Ambiguous`, `Refusal` and `RefusalCode`, each `json_name` the wire key, every member always written in field order. `proto/coder/session-reader-v1.json` holds 14 replies `coder session-read` printed in a throwaway home (a list, pages of a conversation with tool calls and results, a title reference, `latest`, a compaction context, an ambiguous reference and four refusals) and the three transcripts it read. `coax/lib/agent/observe/session-reader-goldens.coax` holds each reply's keys to its message exactly, reads each transcript through the one writer (`claude-history.coax`) and requires the page's bytes, writes each refusal again with `csr_error`, and requires the writer's opening on a list and an ambiguous reply. No bytes changed |
| C11 | Microcoder journal and prompt | Two of three. The journal: `proto/coder/microcoder-journal-ajr1.json`, 11 chained frames and nine refused ones made with Python's hashlib; `coax/lib/agent/journal-profile.coax` holds the one codec (`coax/lib/agent/journal.coax`) to them, and its `--file` walked the five journals with frames on the owner's Mac (128 frames) and wrote each back to the same bytes. The prompt: `proto/coder/microcoder-prompt-v1.json`, the system prompt, the answer schema and the state after each of six steps, written from the code; `coax/lib/microcoder/prompt-profile.coax` requires every byte, the cache's prefix rule and that each reply reads as an action. Rows `microcoder-journal-ajr1` and `microcoder-prompt-v1`. Left: the worker lines (`mcworker-client.coax`, `coder/tui/mcworker.coax`): one codec for both ends and lines from a worker that ran |
| C12 | Consent ledger and memory log | `coax/lib/agent/engine/consent-gate.coax` is the consent ledger's one codec and `coax/lib/agent/engine/memory-log.coax` the memory log's (`mem_line` and `mem_unline`, moved unchanged out of `coder/tui/memory.coax`). `proto/coder/consent-memory-ledgers-v1.json` holds files the Coder CLI of the tree wrote in a throwaway home: grants, revokes, a send through the gate and two refusals; two remembers, a revise and a forget. `ledger-profile.coax` reads every line and writes it back to the same bytes, replays the consent chains and verifies and replays the memory log, and each reader refuses five malformed lines. Rows `coder-consent-ledger-v1` and `coder-memory-log-v1` |

## Area 2: Everglade

Generated codecs in the game area: `metaverse/haunt.coax`, `metaverse-engine/net/voice-wire.coax`
and `metaverse/home.coax`. None is Everglade's.

### Inventory

| Shape | Where | Producers and consumers | Schema today | Decision |
| --- | --- | --- | --- | --- |
| Session v5 text frames (`P H I C E R K t T O A`, `W D s S x n e w o k r g`, rest and kit records) | `apps/openagents-web/everglade-wire.coax` | Page (`everglade-net.coax`; `everglade/client/net.coax` through `everglade/client/net-core.coax`) and room (`coax/web/room/room-everglade.coax`, through `everglade-link-wire.coax`) load the same file | Profile `proto/verse/everglade-session-v5.json`. Its check covers `P H I C E R L Z T O K V X`, `y j l t`, `W D s x n`, `e cast/hit/stop/town/xp/level/rc/wm/mend/pburn/pgone`, `w r z g o v`; not the page's `t`, `A`, packed `S`, `k`, kit `b` or `e heal/die/break/burn/rebuild/edit` | Pin. Registered as compliant for the frames the profile holds. 30 Hz, 1,000 bytes a seat a tick, packed rows under half the text size: a hand-packed codec is justified |
| Session frames outside the codec and the profile: `g`, `v`, `e xp`, `e level`, `e rc` (`room-events.coax`, read in `everglade-events.coax:174-218`); `e wm`, `e mend`, `e pburn`, `e pgone` (`room-everglade.coax:1342-1420`, read in `everglade-events.coax` and `everglade-net.coax:891-906`); `V LOOK KIND` (`everglade/client/net.coax:791`); admin `X` lines (`everglade/client/admin.coax`, `room-e2-admin.coax:39`); `K` read by hand in `room-blocks.coax:133`; `e break`, `e burn` and `w` hand-built in `room-everglade.coax:1379-1786` though the codec has writers | One codec, `everglade/shared/everglade-wire.coax`: the room (`progress.coax`, `send.coax`, `input.coax`, `rest.coax`, `admin.coax`, `room-blocks.coax`) and the pages (`everglade/client/net.coax`, `everglade-events.coax`, `everglade-net.coax`, `everglade/client/admin.coax`) call it | `everglade-session-v5.json` | Pinned: E3b ([#13364](https://github.com/OpenAgentsInc/openagents/issues/13364)) |
| Relay frames `y j l t m` | One codec, `metaverse-engine/net/relay-wire.coax`: the room (`coax/web/room/room-relay.coax`) writes them and reads the ping with it | Pages read them and write the ping with it: `everglade/client/net.coax`, `apps/openagents-web/old/v1/everglade-net.coax`, `lockroom.coax`, `oneshot.coax` | `proto/verse/room-relay-v2.json` ([room-relay-v2.md](room-relay-v2.md)); `y j l t` are also in the v5 profile, whose check runs the same codec | Pinned: E2 |
| The rooms list JSON (`GET /api/playtest/rooms/SLUG`) | `rm_rooms` in `coax/web/room/room-relay.coax` for one shard; SQL `verse.room_list` through `coax/web/api/rooms.c` in production | Read by `apps/openagents-web/old/v1/everglade-join.coax` with `net_json_field` | none | Pin: wave 2 (E8), with the room reports: one codec and a route test on the same fixture |
| Link input v1 (`J1`) | `apps/openagents-web/everglade-link-wire.coax` | Page (`everglade-net.coax:1092`), room | `proto/verse/everglade-link-input-v1.json`, unchecked | Pin: E1 adds its check |
| Everglade II familiar frames `M1 M3-M7`, `m1-m8`, snapshot `m2` | Room: `everglade/server/familiar.coax` (`e2_familiar_read`, `e2_familiar_put_*`). Page: `everglade/client/net.coax` | Two hand-written codecs | Seven profiles, checked against the room's codec by `coax/web/room/familiar-profile.coax` (E1) | On hold: #13276 deletes these frames. If it doesn't, E2 moves the page onto the room's codec |
| Healer's Kit `U1`, `u1` | One codec, `everglade/server/kit-wire.coax` | The page (`everglade/client/net.coax`) writes `U1` and reads `u1` with it; the room (`everglade/server/aid.coax`) reads `U1` and writes `u1` with it | `proto/verse/everglade-healers-kit-v1.json`, checked by `coax/web/room/familiar-profile.coax` | Pinned: E2 |
| Path guard `g2`, `g3`; monsters `h2`, `h3` | `room-e2-guard.coax:377` (`g3`), `:398` (`g2`); `room-e2-monsters.coax:321,478` (`h3`), `:501` (`h2`); `everglade/client/net.coax:407-410` | One codec, `everglade/server/creature-wire.coax` | `everglade2-combat-v1.json` | Pinned: E3 |
| Projectiles `F a d` | `metaverse-engine/net/projectile.coax`, `record.coax` | Shared | `everglade2-combat-v1.json` | Pinned: E3 |
| Latency observation `@1` | `metaverse-engine/net/latency.coax` | Shared | Profile, checked by `coax/web/room/latency-profile.coax` | Pin. Compliant |
| WebTransport framing (mark byte, channels, 1,200-byte datagrams) | `metaverse-engine/net/transport.coax`, `room-wt.coax` | Shared | `aethyr-transport-v1.json` | Pinned: E3 |
| Old wire (`H 1`) | `coax/web/room/room-oldwire.coax` | Ends 2026-11-12 | none | Delete: E1 |
| Session v4 profile | `proto/verse/everglade-session-v4.json` | No code reads it | — | Delete: E1 |
| Orbit League v2 frames | `metaverse-engine/net/orbit-session.coax` (with `orbit-snapshot.coax`) | Shared | Profile, checked by `orbit-wire-profile.coax` | Pin. Compliant |
| Seat ticket v2/v3 (HMAC-SHA256) | Written by C `snprintf` in `coax/web/api/player.c:524` and by Coax `tk_sign*` in `room-ticket.coax:230-268` (the dev stand-in and the checks); read by `room-ticket.coax` `tk_check` | One writer since E5 (`seat_ticket_hmac_write`) | `proto/verse/seat-ticket-hmac-v1.json` | Done: E5 |
| Ticket v4 resume envelope (signed) | `metaverse-engine/session/resume-payload.coax`, `resume-envelope.coax` | Shared, exported to C | none | Pin: E5 adds fixtures |
| Character checkpoint v2/v3 (signed, stored in `verse.character_checkpoints_v2`) | `metaverse-engine/session/character-checkpoint.coax`; SQL rewrites v3 to v2 at `player.c:350`; v1 keys duplicated in `coax/web/api/checkpoint-policy.coax` | Room, API | JSON Schema, unchecked | Pin: wave 2 (E7). Golden for the SQL rewrite |
| Room reports to the API (vitals, directory, stats, the checkpoint post and its receipt; signed) and the control poll reply | `room-vitals.coax`, `room-directory.coax`, `room-stats.h` (the body; `room-stats.coax` signs it), `room-checkpoint.coax:221-246`, `room-control.coax`; `player.c`, `rooms.c` | Coax room, C API | `proto/verse/room-reports-v1.json` | Room side pinned: E8. One codec, `metaverse-engine/session/room-report.coax`: the room writes the vitals and directory posts and the control ask through it and reads the vitals receipt, the control answer and the checkpoint receipt through it; the API writes both receipts through it (`ticket-sign.coax`). Left: the API reads the two posts in C and SQL builds the control answer (`player.c`, `rooms.c`), and the stats body (`room-stats.h`). [room-reports-v1.md](room-reports-v1.md) |
| Item ledger op | Route `room_items` at `player.c:706`. `metaverse-engine/session/ledger.coax` is a store model with no codec for the op; only a test loads it | No room sends it | none | Wave 2: wire it or delete it |
| `room-state {look,class}` | `player.c:624-652` (`room_state`) | Kept: look and class go into `verse.room_vitals` and come back in the ticket reply (`:543`); only `wanted` is refused. Sent by `everglade-net.coax` and `everglade5e-face.c:82`. Only the local stand-in drops it (`room-dev.coax:214`) | none | Move: wave 2 (E11), with the other player routes |
| Player appearance `aethyr.player.appearance.v1` (CC2, CC3, CC4 text) | `metaverse-engine/character/character-appearance.coax:59,72` (the reader also takes `CC1`) | Stored in localStorage (`coax/web/host.js:570`) and the iOS store (`metaverse-engine/host/apple/PageHost.m`) | `proto/aethyr/player-appearance-v1.json` | Pinned: E9. [player-appearance-v1.md](player-appearance-v1.md) |
| Realm save AESV (CRC-32), stored in IndexedDB and `verse.progress` | `apps/openagents-web/save.coax`; a second validator in C (`container_ok`, `player.c:94`) | Page, API | `proto/verse/realm-save-aesv-v1.json` | Pinned: E10. [realm-save-aesv-v1.md](realm-save-aesv-v1.md) |
| Realm document `aethyr.realm` v1 (SHA-256 per revision) | Built in SQL (`0093`, `0109`, `0111`). `metaverse-engine/editor/doc.coax` reads it, but only examples, the book and `coax/web/api/tests/realm_walkthrough.coax` load that file | `apps/openagents-cli/realm.coax:108-122` parses `/doc` with its own JSON reads; `myrealm.coax:436` reads the spec's text lines, not the document; API | `verse-doc` | Done: E6. `doc.coax` is the one reader and writer; `realm-document-v1.json` |
| Edit operations and the undo log `{to,card,ops:[{slot,before,after}]}`; save body `{rev,moved,shaped}` | `metaverse-engine/editor/edit.coax:339-514` holds the operations in memory and builds the save body (`editor_save_body`, `:479`). The undo log JSON is built only in plpgsql (`0111:181`) | The Metaverse Engine, My Realm, API | `verse-doc` | Done: E6. `edit.coax` writes the bodies; `doc.coax` applies the logged steps |
| Engine pin manifest `aethyr-engine 1` (hashed) | `metaverse-engine/editor/publish.coax`; again in SQL (`0107`); read by `coax/web/engine-served.coax` | A fixture pair: `publish.coax:242` and `coax/web/api/tests/realm_engine_pin.sql` | none | Done: E6, row `engine-pin-manifest-v1` with its check |
| Realm API routes (`plan`, `pick`, `settings`, `switch`, `revise`, `fork`, `publish`, `visibility`, `place`, `lore`, `quest`, `asks`, text lists) | `coax/web/api/realms.c:7-60` | Site pages, CLI | `verse-doc` for the record routes (`record.proto`); none for the rest | Move: wave 2 (E11). Part 1 done (see the route table below) |
| Everglade city edits `eg-edit 1` | `everglade/world/edits.coax:13` | `everglade-edit.coax`, `everglade-voice.coax`, `apps/metaverse-engine-desktop/scene-cli.coax` and `guide-route.coax`, all through `edits.coax`; the room only notes that a list came (`room-world.c` `world_edits`) | `proto/verse/everglade-edits-v1.json` | Pinned: [everglade-edits-v1.md](everglade-edits-v1.md) |
| NPC ask `{npc,world,history,question,controls,visit}` and `{text,model}` | Was four client encoders (`people.coax`, `fairy-forest/talk.coax`, `realm.coax`, `myrealm.coax`); since E4 one codec, `metaverse-engine/talk/wire.coax`, that all four call. Server: `coax/web/api/npc.c` and SQL `verse.npc_ask` | Pages, API | `openagents.verse.npc.v1alpha1` | Done: E4 |
| WorldBuilder: act request, reply lines `S`, `A op\|…`, save `WB1` | `apps/openagents-web/worldbuilder*.coax`, `coax/web/api/worldbuilder.c`, SQL `worldbuilder.lines` | One page | none | Move: wave 2 (E12), after #13226 puts its edits on the forge log. Its operations become E6's |
| Metaverse Engine local commands (scene v1/v2, NPC, raid), `aethyrforge.scene-session.v1` and `aethyrforge.npcs.v1` | `apps/metaverse-engine-desktop/scene-policy.coax` (`fs_command`), `scene-session.coax` (the session record; `Sources/app.m` writes it through `fs_session_put`), `metaverse-engine/editor/npc-command.coax`, `raid-command.coax`, `npc-document.coax` | Local tools | JSON Schemas, each with a check | Pinned: E1 (#13244) |
| Metaverse Engine voice input `aethyrforge.voice-coder.v1` and `aethyrforge.microphone-capture.v1` | Written in `apps/metaverse-engine-desktop/Sources/app.m:282-288`; read in `voice-coder.coax:69` and `app.m:1035` | Metaverse Engine | `proto/coder/voice-input-v1.schema.json`, `microphone-capture-v1.schema.json`, unchecked | Pin: E1 adds checks or deletes them |
| Physics lab record | `metaverse-engine/lab/physics-record.coax` | The lab | `proto/aethyr/physics-lab-v1.json`; the file's tests use their own literals, not the profile | Pin: give it a check or delete it (tracker) |
| Asset packs VPK1, VPK2, VPK3 (VPK3 hashed) | `metaverse-engine/editor/pack.coax`, `page-pack.coax`; also `metaverse/tools/pack_realm.py`, a JS reader at `coax/web/host.js:1931`, and many more readers and writers: at least 16 files under `metaverse/tools`, `PageHost.m`, `Engine.swift`, `apps/everglade-mobile/android/host.c`, `pack-host.js`, `pack-fetch.js`, `pack-face.c`, `metaverse/native/realm-host.c`, `drops.c`, `verse_packs.c`, `room-catalog.coax` | Build, page, room, native hosts | `proto/verse/asset-pack-v2.json`, unchecked | Pin: wave 2. Hand-packed is right for a mapped file. One writer and one reader in each language is the goal; today it is far from that |
| Presence and room chat JSON (`/api/playtest/presence`, `chat`, `inbox`) | `coax/web/api/room.c` | Older realms | `metaverse/presence/v1alpha1`, unused | Wave 2: decide with the room servers' reach; don't build on the unused proto before then |
| Player, world, progress, seats and packs routes | `player.c`, `seats.c`, `verse_packs.c` | Pages | none | Wave 2 (E11) |
| Zone geometry and logic | `everglade/zone.coax`, `everglade/world/` | Compiled in | — | Keep. Code, not a wire. The `P` frame's content pin covers it |
| SRD rules data | `rules/srd/data/*.coax`, generated from the pinned PDF | Compiled in; only indices travel | — | Keep. The `P` frame's rules pin covers it |
| Haunted House save and scene | `metaverse/haunt.coax` | Generated codec | `metareal/haunt/v1alpha1` | Compliant |
| `engine/v1` (scene, agent, control, state, assets), `metaverse/presence`, `metaverse/common`, `host/v1`, `reach/v1` | `proto/openagents/` | Tests and tools only. `hud.proto` is not in this row: `metaverse/home.coax` writes its `HudInvocation` | published | Keep frozen. Marked `no-consumer` in the registry and in `proto/README.md` |

### Steps

| Step | Issue | Work | Done when |
| --- | --- | --- | --- |
| E1 | [#13244](https://github.com/OpenAgentsInc/openagents/issues/13244) | Clean the profile family: delete the v4 profile and `room-oldwire.coax`; add a check for every unchecked profile and schema under `proto/verse/` and `proto/aethyrforge/`, or delete it | Every file under those folders names a check that passes. `proto/README.md` lists the family |
| E2 | [#13245](https://github.com/OpenAgentsInc/openagents/issues/13245) | Familiar and Healer's Kit frames in one shared codec file that room and page load; profiles with golden frames; the relay's frames read through one codec | Done for the Healer's Kit and the relay: `everglade/server/kit-wire.coax` writes and reads `U1` and `u1` on both sides, and `metaverse-engine/net/relay-wire.coax` writes and reads `y j l m` and the clock's ping and pong for the room and four pages (`proto/verse/room-relay-v2.json`, `coax/web/room/relay-profile.coax`). On hold for the familiar frames (`M1 M3-M7`, `m1-m8`): #13276 moves familiars onto rows and deletes them; if it doesn't start by 2026-11-25, the page moves onto `everglade/server/familiar.coax`'s codec here. The rooms list JSON moved to E8 |
| E3 | [#13246](https://github.com/OpenAgentsInc/openagents/issues/13246) | Guard, monster and projectile frames, the session frames that are outside the codec today (`g`, `v`, `e xp/level/rc/wm/mend/pburn/pgone/break/burn`, `w`, `V`, `X`, the `K` reader) and the WebTransport framing: shared codec, profile, doc | Same as E2 for `g2 g3 h2 h3 F a d` and those frames: `room-everglade.coax`, `room-events.coax`, `room-e2-admin.coax` and `room-blocks.coax` hold no frame text of their own. `docs/protocol/` describes them. The session frames are E3b ([#13364](https://github.com/OpenAgentsInc/openagents/issues/13364)): done; `everglade-wire.coax` writes and reads them on both sides and `everglade-session-v5.json` pins them ([everglade-session-v5.md](everglade-session-v5.md)) |
| E4 | [#13247](https://github.com/OpenAgentsInc/openagents/issues/13247) | NPC ask: package `openagents.verse.npc.v1alpha1`; the four clients use `metaverse-engine/talk/wire.coax`; a SQL test on the same fixtures | Done. `proto/openagents/verse/npc/v1alpha1/ask.proto` (profile `verse-npc`); `metaverse-engine/talk/wire.coax` writes the ask and reads the answer for Everglade, Fairy Forest, the Haunted House and made realms (a hand codec: history is written even when empty). `proto/verse/npc-ask-v1.json` holds the pages' asks, refused bodies and the API's answers; `metaverse-engine/talk/wire-goldens.coax` writes and reads each through the codec, and `test_npc.py` posts each to the API and gets its status. `wire-check` has no allowlist line for it |
| E5 | [#13248](https://github.com/OpenAgentsInc/openagents/issues/13248) | Seat ticket: one Coax writer that `player.c` calls; fixtures for v2, v3 and the v4 envelope | No `snprintf` builds a ticket. A ticket the writer makes passes `tk_check`; goldens match byte for byte. Done: the one v2, v3 and v4 writer is `seat_ticket_hmac_write` (`metaverse-engine/session/seat-ticket.coax`); `player.c` calls it through `ticket-sign-unit.c`, and `room-ticket.coax`'s signers for dev rooms and checks (`tk_sign` to `tk_sign_owner`) all end in it (`tk_sign_acct`), so they take the API's bounds: a pid and a room of a-z, 0-9 and `-`, a name `tk_name_ok` takes, a key of 32 bytes or more, and XP 0 written as none. `tk_check` stays the room's reader; it reads the HP field and the name through the codec's `tk_hp_read` and `tk_name_ok` |
| E6 | [#13249](https://github.com/OpenAgentsInc/openagents/issues/13249) | Realm document and edit operations: package `openagents.verse.doc.v1alpha1`; fixtures shared by `metaverse-engine/editor/` and the SQL | Done. `proto/openagents/verse/doc/v1alpha1/realm.proto` (profile `verse-doc`). `proto/verse/realm-document-v1.json` holds the stored bytes: a made realm, ten steps (moves, turns, sizes, a removal, lore, a quest, a restore of every shape, two undos, a redo, a stale save), and after each the document, its SHA-256, the reply and the logged undo step. `metaverse-engine/editor/doc-goldens.coax` replays it through `edit.coax` and `doc.coax` (`editor_doc_json` writes jsonb's text; `editor_apply_step` applies the server's logged steps; replies and keys checked against the descriptor, the small replies through the JSON mapper too), and `coax/web/api/tests/realm_doc_checks.coax` replays it through the routes. My Realm and the CLI build and read the save, undo and redo bodies and replies through `edit.coax` and `doc.coax`. `coax/web/cli-journey-fixture.coax`, the CLI journey's fake server, answers in the fixture's shapes: it reads `realm-document-v1.json` as data (the toolchain context loads no `metaverse-engine/editor/` code) and writes its save, undo and stale replies and its document from them (#13354). Follow-ups (#13354): the edit model names board rows `feature:N` and `row:N` as the server's log does, with `editor_switch` the server's switch and a switch step in the fixture; moves and shapes stand in entity order (migration 0166), so an undo or redo gives back the restored revision's bytes but for `rev` (the fixture's `restores`). The save now always sends `shaped`: without it, restoring the last turned, sized or removed thing was never saved. The engine pin manifest has its row and check. Rows `realm-document-v1`, `realm-edit-v1`, `engine-pin-manifest-v1`; no allowlist line |

Wave 2, in order: E7 character checkpoint; E8 room reports; E9 appearance
record; E10 realm save; E11 realm, player and seat routes on the `api.v1`
envelope; E12 WorldBuilder operations (after #13226); then packs, city edits
and presence.

Wave 2 steps done:

| Step | Work | Done |
| --- | --- | --- |
| E8 | Room reports (the room's side and the receipts) | `metaverse-engine/session/room-report.coax` is the one codec for the vitals post and its receipt, the directory post, a drain's signed text, the control ask and the lines of its answer, and the checkpoint's receipt. `room-vitals.coax`, `room-directory.coax`, `room-control.coax` and `room-checkpoint.coax` hold no report text of their own, and the room's local stand-in answers with the codec's receipts. `player.c` writes the vitals receipt and the checkpoint receipt through it (`ticket-sign.coax`, `ticket-sign-unit.c`). `proto/verse/room-reports-v1.json` holds posts a room wrote and receipts and control answers a running API gave; `coax/web/room/report-profile.coax` writes each through the codec and reads the receipts and the control answer back. Row `room-reports-v1`; four `room` lines gone. Left for `player.c` and `rooms.c`: the API reads the two posts in C, SQL builds the control answer, and the stats body is `room-stats.h`'s |
| E9 | Appearance record | `proto/aethyr/player-appearance-v1.json` holds golden records for CC2, CC3 and CC4, CC1 and corrected records with the bytes they are written back as, and malformed records. `metaverse-engine/tools/appearance-profile.coax` runs `ca_save` and `ca_restore` on each and holds the layout to the bytes. Every page reads and writes the record through `metaverse-engine/character/character-appearance.coax`. The two page hosts keep the text under the ID as a storage key and never parse it: they are the row's `keys` (rule 2 lets a `keys` file name the id as a whole string, never after a `schema` key), so their two `id` lines are gone |
| E10 | Realm save | `proto/verse/realm-save-aesv-v1.json` holds golden AESV containers (made with zlib's CRC-32, not the codec), a version 1 container, malformed ones, the browser's record of two saves step by step, the bare-hex record from before containers, and the post bodies. `apps/openagents-web/save-profile.coax` runs `save_pack`, `save_open`, `save_read`, `save_record`, `save_pick` and `save_post_rev` on them. `test_player.py` posts every container to `/api/playtest/progress`: the API's `container_ok` keeps each golden and refuses each malformed one the profile says it refuses. It checks less than the page (no world, schema or lengths), and the profile names which containers only the page refuses. Row `realm-save-aesv-v1`; the codec stays in `apps/openagents-web/` with a `codec` line |
| City edits | Everglade city edits | `proto/verse/everglade-edits-v1.json` holds the published list (version 21), a list with rows, lifts, colours, hidden buildings and people taken away, lists the reader corrects with the text they are written back as, malformed lists, the publish body and the API's answers. `everglade/world/edits-profile.coax` runs `eg_edits_text`, `eg_edits_read`, `eg_edits_body` and `eg_edits_answer` on them. Every reader and writer in the product calls `edits.coax`; the room does not parse the list. Row `everglade-edits-v1`; the codec stays in `verse/everglade/` with a `codec` line |

E11 goes one route file at a time ([#13250](https://github.com/OpenAgentsInc/openagents/issues/13250)).
The envelope it lands on is what ships: `openagents.api.v1alpha1.ErrorReply`,
`{"error":"WORDS"}`, written by `coax/web/api/api-wire.coax`. The spec's
`openagents.api.v1` (typed errors, pages, idempotency) is a new version, not
a change to these bytes. Each route file gets its own codec, which loads
`api-wire.coax`, and the C route calls it through its generated unit.

| Route file | State |
| --- | --- |
| `coax/web/api/service.c` | Done. `proto/openagents/ops/service/v1alpha1/service.proto` (profile `api-routes`); `coax/web/api/service-wire.coax` writes the health body, the service-watch job's result, the alerts handed to `verse.service_sync` and the refusals, and `service.c` holds no body text. `proto/verse/service-health-v1alpha1.json` holds the bytes the C wrote before; `service-wire-goldens.coax` writes each through the codec, holds its keys to the descriptor (each field once, in order), reads it back and writes it again, and sends refusals through the JSON mapper. `test_jobs.py` runs the same check on a running API's health body, job result and refusal. Rows `service-health-v1alpha1` and `api-error-v1alpha1`; no allowlist line |
| `coax/web/api/realms.c` | Done (parts 1 to 9). The record routes are `proto/openagents/verse/doc/v1alpha1/record.proto` (profile `verse-doc`): `GET /realms/mine` and its `?as=lines` text, `POST /<slug>/visibility` (also under `edit-v1/`), a made realm's `POST /<slug>/publish`, `GET /<slug>/published`, and a refusal as SQL answers it (`RouteError`, `{"error": "WORDS"}`, jsonb's text; the refusals `realms.c` writes itself are `ErrorReply`'s bytes already, though still C literals). `metaverse-engine/editor/record.coax` is the one codec; the CLI (`realm list`, `visibility`, `publish`) and My Realm (`publish`) call it. `proto/verse/realm-record-v1.json` holds what the server answers; `record-goldens.coax` reads and writes each body through the codec and holds its keys to the descriptor, and `coax/web/api/tests/realm_record_checks.coax` makes the same calls against the API. Row `realm-record-v1`. Part 2 done: the bytes `realms.c` writes itself are `route.proto` (`Paged`, `CardReply`; the lines of a made realm and a refusal's words for a caller that reads lines) and `ErrorReply`. `coax/web/api/realm-wire.coax` is the one writer (it loads `api-wire.coax`), `realms.c` calls it through `realm-wire.generated.c` and holds no body text, and `proto/verse/realm-wire-v1alpha1.json` holds the bytes the C wrote before; row `realm-wire-v1alpha1`. A plan's refusal is now SQL's own object (`jsonb_build_object`), the same bytes. Part 3 done: a made or forked realm's answer (`made.proto`'s `MadeReply`, and its `?as=lines` text) and a fork's body (`ForkRequest`) are `metaverse-engine/editor/made.coax`'s; the builder, My Realm and the CLI call it. `proto/verse/realm-made-v1.json` holds a made realm's forks, checked through the codec (`made-goldens.coax`) and the routes; row `realm-made-v1`. Part 4 done: the fork pane. Fork info's lines, a built-in fork with its rules and a fork's publish (`ForkPublishRequest`, `ForkPublishReply`) are `made.coax`'s too, and Everglade's pane (`apps/openagents-web/old/v1/everglade-fork.coax`) calls it; the fixture forks Everglade, reads both panes and lists and unlists the fork. Part 5 done: the spec's lines (`GET /<slug>`). Profile `proto/verse/realm-spec-v1.json` names all 28 lines and holds seven specs as the server answers them; `metaverse-engine/editor/spec.coax` is the one reader (a line's kind and its fields), and My Realm, the realm edit layer (`metaverse/edit/edit.coax`) and the CLI's two readers take every line from it; `spec-goldens.coax` requires each line back byte for byte from its fields; row `realm-spec-v1`. Part 6 done: the builder. `builder.proto` names the requests (`PlanRequest`, `PickRequest`, `MakeRequest`, `DocRequest`) and the JSON answers; profile `proto/verse/realm-board-v1.json` names the board's and the pick's 11 lines and holds requests and answers as they travel. `metaverse-engine/editor/board.coax` writes every body and reads every line for the builder page and the CLI; `board-goldens.coax` holds them through it and `test_realm_builder.py` sends them to the API; row `realm-board-v1`. Part 7 done: the edit routes. `edits.proto` names the requests and answers of `settings`, `switch`, `revise`, `asks`, an ask marked done, the deletes, `describe`, `place`, the proposal decisions, `lore` and `quest`; `metaverse-engine/editor/change.coax` writes every request for the realm edit layer, My Realm and the CLI (the revision first under `edit-v1/`); `proto/verse/realm-edits-v1.json` holds bodies and answers, checked through the codec (`change-goldens.coax`) and the routes; row `realm-edits-v1`. `describe` has no caller in a product. Part 8 done: the other text answers. Profile `proto/verse/realm-lists-v1.json` names the lines of the feature list, an ask's and a revision's answers, the panes, the proposals, the listed realms, the made realms' rows for `/og` and the card's want, and holds each as the server answers it. `metaverse-engine/editor/lists.coax` reads the asks, panes and proposals for the realm edit layer; `metaverse-engine/editor/listed.coax` (moved from `apps/openagents-web/realms-listed.coax`) reads the listed answer, and its `listed_field` reads the `/og` page's rows; row `realm-lists-v1`. Every body the file serves now has a schema, a fixture and a route test (the table below). Part 9 done, and the `route` line is gone: `coax/lib/media/realm_seat.coax` (`openagents realm play`) reads the spec's `character` lines through `metaverse-engine/editor/spec.coax` (`editor_spec_line`, `editor_spec_person`). The file has its own line in `contexts.txt` (context `realm-seat`, which may load realm authoring; Coder may load it), so the load passes `layer-check`. A `character` line with no name is now passed over, as the spec's reader does. `coax/web/card.js` compares the card's answer with the one word `want` in the browser; that is host glue, like the host that finishes the plan's body, and `realm-lists-v1.json` pins the word. `realms.c` is in row `realm-wire-v1alpha1`'s `files` |
| `coax/web/api/seats.c` | Done; no allowlist line. `proto/openagents/verse/seat/v1alpha1/seat.proto` (profile `api-routes`) names the open request (`OpenRequest`), the grant (`Grant`, `Player`), the account's seats (`SeatsReply`, `Seat`), `Kept`, `Closed` and the two bodies the API sends the runner that holds a hosted seat's page (`RunnerOpen`, `RunnerClose`). `coax/lib/media/realm_seat_wire.coax` is the one codec; `seats.c` calls it through `seat-wire.generated.c` (`coax/web/api/seat-wire.coax`, which adds the API's `ErrorReply`) and holds no body text. The grant and the list stay SQL's, as jsonb's text. `proto/verse/realm-seat-v1alpha1.json` holds the bodies as a running API answered them; `seat-wire-goldens.coax` writes each through the codec, holds its keys to the descriptor, reads it back and writes it again, and `test_room.py` requires the API's bytes to be the fixture's (but for ids, times, the player and its key) and runs each through the same check. Row `realm-seat-v1alpha1`. Part 2 done: the CLI (`openagents realm play`, `apps/openagents-cli/realm-play.coax`) and the realm's YOUR AGENT pane (`metaverse/edit/agent.coax`) write the open body and read the grant through the codec; a body that is not a whole grant is refused. Part 3 done: the runner's side. `seat.proto` adds `RunnerAct` (with `Action`, the members of one action) and `SeatStart`, the first line the runner hands the seat's own process. `coax/web/agent/seat-runner.c` reads the open, act and close bodies, writes the first line, `{"closed":true}` and its refusals through `seat-runner-wire.c` (`coax/web/agent/seat-runner-wire.coax`, compiled beside the runner) and has no parser of its own for them; `oa-seat` (`coax/web/agent/seat.coax`) reads the first line through the codec. The fixture holds the act as the API sends it and the first line and five refusals as a running runner wrote them, and `test_room.py` sends the runner the API's spelling and holds its answers to the fixture. Part 4: `.../act`'s action (`Action`) and the observation it answers (`Observation`, `Near`) are the codec's too. `seatw_action_json` writes an action for the CLI (`seat_action`) and the realm's page (`ra_action`), and `seatw_read_action` reads it for the seat (`realm_seat.coax` `seat_act`); `seatw_obs_json` writes the observation for the seat (`seat_observation`) and the local stand-in runner (`coax/web/local-origin.coax`), and `seatw_read_obs` reads it for the CLI (`seat_obs_text`) and the page (`ra_saw`). The fixture holds ten actions and thirteen observations a seat in a running realm printed (headless Chrome, this checkout's page on a local origin, a throwaway API and database), printed by the seat's writer before it moved: the codec reads each and writes the same bytes, and a run through the codec printed the first three lines byte for byte. Changes: a number that is not a decimal, or a word too long for its place, is refused where the action is written (the CLI, the page) rather than dropped by the seat; a body that is no observation shows the page `Not done: .` and the CLI nothing; the budgets of `myrealm`, `icecream` and `deadgears` rise by 10,000 bytes for the reader |
| `coax/web/api/jobs.c` | Done. `proto/openagents/ops/jobs/v1alpha1/jobs.proto` (profile `api-routes`) names what `POST /api/jobs/<job>` answers (`JobRun`) and the two jobs' own results (`Heartbeat`, `AuthSweep`); the service watch's result is `service.proto`'s `WatchResult`. `coax/web/api/jobs-wire.coax` is the one writer; `jobs.c` calls it through `jobs-wire.generated.c` and holds no body text (ten refusals are `ErrorReply`). `proto/verse/ops-routes-v1alpha1.json` holds what a running API answered before the codec; `coax/web/api/ops-wire-goldens.coax` holds each body's keys to its message, reads it through the codec and writes it back, and `test_jobs.py` hands it every body of a run (`--read-all`). A day that ran before answers its stored result as jsonb's text; the codec carries a result as it came. The auth-sweep job failed on every database until 2026-11-13 (its two deletes were `WITH` items inside sub-selects, which Postgres refuses); with that fixed in `jobs.c` the fixture holds its result as a running API answered it, the day it ran and the same day again, and `test_jobs.py` requires the job to run and to sweep a spent signature. Row `jobs-v1alpha1`; no allowlist line |
| `coax/web/api/email.c` | Done. `proto/openagents/ops/email/v1alpha1/email.proto` (profile `api-routes`) names the request (`SendRequest`, read in SQL by `email.begin`) and the reply (`SendReply`). `coax/web/api/email-wire.coax` writes the reply and the refusal; `email.c` calls it through `email-wire.generated.c` and holds no body text. A refusal's words still lose any quote or backslash (the codec drops them as the C did); a control byte in them is now escaped where the C wrote it raw. What the mail service is sent is its own format, built in SQL. Same fixture and check as `jobs.c`. Row `email-v1alpha1`; no allowlist line |
| `coax/web/api/verse_packs.c` | Done. `proto/openagents/verse/packs/v1alpha1/packs.proto` (profile `api-routes`) names the session (`PackSession`). `coax/lib/media/pack_session_wire.coax` is the one codec: `verse_packs.c` writes through it (`coax/web/api/packs-wire.coax`, `packs-wire.generated.c`) and holds no body text (14 refusals are `ErrorReply`), and the native host's `hpk_private` (`metaverse-engine/host/private-pack.coax`) reads through it. The page host reads the same members in the browser (host glue). `proto/verse/pack-session-v1alpha1.json` holds what a running API answered before the codec; `packs-wire-goldens.coax` holds each body's keys to its message and reads and writes it through the codec, and `tests/verse_packs.coax` does the same for every answer of a run. Row `pack-session-v1alpha1`; no allowlist line |
| `player.c` | Not yet |

### `realms.c`, route by route

Every route `coax/web/api/realms.c` serves, the schema that names its bodies, the codec its
product callers use, and the fixture a route test runs. "SQL" means the body is built in SQL and
carried by the route as it came. A refusal the route decides itself is `ErrorReply`
(`api-wire.coax` through `realm-wire.coax`); one SQL decides is `RouteError` (`record.proto`).
Under `edit-v1/` each edit route takes the revision first and answers it.

| Route | Request | Answer | Codec (callers) | Fixture, route test |
| --- | --- | --- | --- | --- |
| `POST /plan` | `builder.proto` `PlanRequest` | `PlanReply`; as lines the board (`realm-board-v1`) | `board.coax` (builder page, CLI) | `realm-board-v1.json`, `test_realm_builder.py` |
| `POST /plan/doc` | `DocRequest` | as `/plan` | `board.coax` writes the setup; the page's host adds the picked file's name and data (browser glue) | the setup in `board.coax`'s checks; the route in `test_realm_rights_asks.py` |
| `POST /pick` | `PickRequest` | `PickReply`; as lines `field`, `value` | `board.coax` (builder page) | `realm-board-v1.json`, `test_realm_builder.py` |
| `POST /` (make) | `MakeRequest` | `made.proto` `MadeReply`; as lines slug, path, page | `board.coax` writes, `made.coax` reads (builder page, CLI); `realm-wire.coax` writes the lines and `page` | `realm-board-v1.json`, `realm-wire-v1alpha1.json`, `test_realm_builder.py` |
| `GET /mine` | none | `record.proto` `MineReply`; as lines `realm ...` | `record.coax` (CLI) | `realm-record-v1.json`, `realm_record_checks.coax` |
| `GET /made` | none | text: a row of `/og`'s table for each public realm (`realm-lists-v1`) | `listed.coax` `listed_field` (the `/og` page) | `realm-lists-v1.json`, `realm_record_checks.coax` |
| `GET /listed` | none | text: `total N` and a row a realm (`realm-lists-v1`) | `listed.coax` (the list page) | `realm-lists-v1.json`, `realm_record_checks.coax` |
| `GET /<slug>` | none | text: the spec (`realm-spec-v1`) | `spec.coax` (My Realm, the edit layer, CLI). Not on it: `coax/lib/media/realm_seat.coax` | `realm-spec-v1.json`, `realm_record_checks.coax` |
| `GET /<slug>/doc`, `POST /save`, `/undo`, `/redo` | `realm.proto` `SaveRequest`, `RevRequest` | `DocReply`, `SaveReply`, `UndoReply`, `Conflict` | `doc.coax`, `edit.coax` (My Realm, CLI) | `realm-document-v1.json`, `realm_doc_checks.coax` |
| `POST /<slug>/publish` (a made realm) | `RevRequest`, `record.proto` `PublishRequest` | `PublishReply` | `edit.coax`, `record.coax` (My Realm, CLI) | `realm-record-v1.json`, `realm_record_checks.coax` |
| `GET /<slug>/published` | none | `PublishRecord` | `record.coax`; no product reads it but `coax/web/engine-served.coax`'s check | `realm-record-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/visibility` | `VisibilityRequest` | `VisibilityReply` (`page` added by `realm-wire.coax`) | `record.coax` (CLI) | `realm-record-v1.json`, `realm_record_checks.coax` |
| `GET /<slug>/fork` | none | text: fork info (`made.proto`'s comment) | `made.coax` (Everglade's fork pane) | `realm-made-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/fork` | `made.proto` `ForkRequest` | `MadeReply`; as lines slug, path, page | `made.coax` (My Realm, the fork pane, CLI) | `realm-made-v1.json`, `realm_record_checks.coax` |
| `POST /<fork>/publish` | `ForkPublishRequest` | `ForkPublishReply` | `made.coax` (the fork pane); the CLI's is `RevRequest` or `PublishRequest` | `realm-made-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/settings`, `/switch`, `/place`, `/lore`, `/quest` | `edits.proto` `SettingsRequest`, `SwitchRequest`, `PlaceRequest`, `LoreRequest`, `QuestRequest` | their replies; no product reads more than the status and the revision | `change.coax` (the edit layer, My Realm, CLI) | `realm-edits-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/revise` | `ReviseRequest` | `ReviseReply`; as lines `plan N` and the rows | `change.coax` (CLI), `spec.coax` reads the rows (the edit layer) | body in `realm-edits-v1.json`; answers in `realm-lists-v1.json`, `test_realm_builder.py` |
| `GET /<slug>/asks` | none | text: the feature list (`realm-lists-v1`) | `lists.coax` (the edit layer); the CLI prints it | `realm-lists-v1.json`, `test_realm_builder.py` |
| `POST /<slug>/asks` | `AskRequest` | text: `ask ID STATE`, `plan N`, the rows | `change.coax` writes, `lists.coax` and `spec.coax` read (the edit layer, CLI) | body in `realm-edits-v1.json`; answer in `realm-lists-v1.json`, `test_realm_builder.py` |
| `POST /<slug>/asks/doc` | `edits.proto` `AskDocRequest`; the page's host builds it from the picked file (browser glue) | text: `doc N M applied K waiting` and the rows | `change.coax` (CLI); the edit layer reads the rows with `spec.coax` | body in `realm-edits-v1.json`; the line in `realm-lists-v1.json`; the route in `test_realm_rights_asks.py` |
| `POST /<slug>/asks/<id>/delete`, `/rows/<key>/delete` | `RevRequest` under `edit-v1/`, else none | `DeleteReply` | `change.coax` `editor_change_deleted` (the edit layer) | the refusals in `realm-edits-v1.json`; a deletion in `test_realm_rights_asks.py` |
| `POST /<slug>/asks/<id>/done` | `AskDoneRequest` | `DeleteReply` | `change.coax` (CLI) | body in `realm-edits-v1.json`; the route in `test_realm_rights_asks.py` |
| `GET /<slug>/proposals` | none | text: `P ...` a proposal (`realm-lists-v1`) | `lists.coax` (the edit layer); the CLI prints it | `realm-lists-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/proposals/<id>/accept`, `/decline` | `RevRequest` under `edit-v1/`, else none | `DecideReply`; no product reads more than the status | none needed | the refusal in `realm-edits-v1.json`; a decision in `test_realm_rights.py` |
| `GET /<slug>/panes` | none | text: `lore`, `quest` (`realm-lists-v1`) | `lists.coax` (the edit layer); the CLI prints it | `realm-lists-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/describe` | `DescribeRequest` | `DescribeReply` | none: no product calls it | `realm-edits-v1.json`, `realm_record_checks.coax` |
| `GET /<slug>/card` | none | text: `want` or `no` (`realm-lists-v1`) | Not on one: `coax/web/card.js` (browser glue) | `realm-lists-v1.json`, `realm_record_checks.coax` |
| `POST /<slug>/card` | a 1200x630 JPEG under 300 KB (not ours to give a schema) | `route.proto` `CardReply` | `realm-wire.coax` writes the answer; `card.js` posts the picture and reads nothing back | `realm-wire-v1alpha1.json`; the route in `test_realm_builder.py` |

What is not through a codec, and why the allowlist line stays or does not for each:

- **`coax/lib/media/realm_seat.coax`** (`coder realm play`) splits the spec's `character NAME|ROLE` lines itself, in the seat's page and again in Coax. It is in the toolchain context, which may not load `metaverse-engine/editor/spec.coax`. This keeps the line.
- **`coax/web/card.js`** compares the card's answer with `want`. It is browser glue that then takes the picture. This keeps the line until it is decided to be glue like the two below.
- **`plan/doc` and `asks/doc`**: the codec writes the setup; the page's host adds the picked file's name and data. Browser glue; does not keep the line.
- **`describe`'s request**: no product calls it. Schema and fixture only; does not keep the line.
- **The JSON answers no product reads** (`PlanReply`, `PickReply`, the edit routes' beyond their status, revision and whether a deletion deleted): the schema names them and the checks hold their keys; does not keep the line.
- **The card's POST body**: a JPEG; does not keep the line.

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
| Realm save, VPK3 packs | CRC-32, SHA-256 | E10 (the save); wave 2 (packs) |
| Appearance record | Stored on players' devices | E9 |

## What to delete

| What | When |
| --- | --- |
| `proto/verse/everglade-session-v4.json`, `coax/web/room/room-oldwire.coax` | E1 |
| `proto/coder/reply-stream-v1.schema.json`, `route-v2.schema.json` | C3 |
| Duplicate encoders: session header and `end`, `approval`, `availability`, `coder.execution.v1`, `openagents.ai-call.v1`, runner events and end bodies, NPC ask clients, the C ticket writer | The step that owns each |
| `gs_price` in `golems.coax` | C6 |
| The `diff` session line | wave 2 (#13250; C2 kept writing it, see C2) |
| The item ledger route if no room sends to it (`room-state` stays: the API keeps the look and class) | wave 2 |
| `metaverse-engine/talk/wire.coax`'s "example only" status: it becomes the codec | E4: done |

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
   A row's `keys` files are the exception: a page host that keeps the
   record's text under the id as a storage key and passes it unread may name
   the id as a whole string, but never after a `schema` key.
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
| Rows marked `no-consumer` | 4: `engine/v1` (not `hud.proto`), `metaverse/presence`, `metaverse/common`, `host/v1` with `reach/v1` |
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
| Metaverse chapter 12.2 | The realm session is text frames with profiles; a binary session is a new negotiated version, not a silent swap |

Books: C1 and C3 update the Coder book chapters that describe the log and the streams (`coder/book/ch002`, `ch016`); C5 updates
`docs/chat.md`; E2 to E5 update the Everglade multiplayer spec
(`docs/metaverse/2026-11-03-everglade-multiplayer-spec.md`); the OpenAgents book's
chapters 46 to 48 link `proto/` in S1.

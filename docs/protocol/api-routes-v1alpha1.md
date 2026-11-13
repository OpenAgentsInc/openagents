# The API's route answers (`openagents.api.v1alpha1`)

What each route file of `coax/web/api/` answers, by route: the method, the path, the status, the
kind of body and the message that names its members. Profile `api-routes`; the fixture is
[`proto/verse/api-routes-v1alpha1.json`](../../proto/verse/api-routes-v1alpha1.json).

## The refusal

A route that refuses or fails answers one compact JSON object under the HTTP status that
classifies it:

```
{"error":"no database"}
```

`ErrorReply` in `proto/openagents/api/v1alpha1/error.proto`. One writer,
`coax/web/api/error-wire.coax` (over `api-wire.coax`), which every route file's C calls through
`api_error`, `api_error_with` (with headers) and `api_error_text` (the body alone) in `api.c`. No
route file spells the body. The words are escaped as JSON.

The same unit writes four more small bodies. Each key is a member of the route's own message:

| Body | Writer | Used by |
| --- | --- | --- |
| `{"error": "not found"}` | `erw_error_spaced_write` | `private.c`: the bytes Postgres writes for `projects.no()`, so every refusal there is the same |
| `{"error":"muted","until":1794554719282}` | `erw_error_num_write` | `room.c` and `player.c` (`until`), `player.c` (`retry_ms`), `lnaddr.c` (`code`) |
| `{"error":"invite only","need":"invite"}` | `erw_error_word_write` | `access.c` (`need`), `player.c` (`refused`) |
| `{"kept":5}` | `erw_num_write` | `analytics.c` (`kept`), `inference.c` (`credit_msat`) |
| `{"file":"g-0a47","upload":"https://..."}` | `erw_words2_write` | `villa.c` |

SQL writes many refusals too. Those carry jsonb's spacing (`{"error": "unknown"}`); they are the
same message, and the check below holds them to it.

## Every other answer

Most answers are built in SQL (a `jsonb` value, or text lines) and passed by the C as they came.
Each has a message in `proto/openagents/api/v1alpha1/<file>.proto`, named for its route file and
path: `GET /api/drops/me` answers `DropsMe`, `POST /api/villa/pets/*/emote` answers
`VillaPetsOneEmotePost`, and a refusal with more than its words answers `...Refusal`. `json_name`
is the key on the wire. A member that holds JSON carried as it came (a project document, a
model's answer) is `bytes`.

A body is one of six kinds:

| Kind | What it is | What the check holds |
| --- | --- | --- |
| `json` | A JSON object, or a list of them | Every key, nested and in lists, is a `json_name` of the message |
| `lines` | Text lines, each a word and its value (`invites 0`) | Each line's first word is a `json_name` of the message |
| `jsonl` | One JSON object a line (the account export, the session log, whose lines have codecs of their own) | Each line is a JSON object |
| `text` | Plain words or a table for a page (`log in first`, an admin table) | It is not JSON and not empty |
| `empty` | No body (a redirect) | It is empty |
| `other` | A page, an image or a stream the same route function serves | Nothing; the row says the route exists |

## The check

```
coax/web/tool.sh coax/web/api/routes-goldens.coax
coax/web/tool.sh coax/web/api/routes-goldens.coax --read-all DIR
```

The first form checks the fixture: every row has a real answer, and each answer holds to its row.
A refusal is also read and written again through `error-wire.coax` for the same bytes.

The second form checks a run. `coax/web/api/test.sh` records every request and answer of the suite
(`API_CAPTURE`) with the API's `X-OA-Route` header on, which names the route function that
answered, and hands the record to this check. An answer from a covered route function that no row
names fails: add its message and its row first. The refusal with only its words needs no row: any
route may give it under any status from 400 up. The header exists only under
`OA_API_ROUTE_HEADER=1`, which production never sets.

## Adding an answer

1. Add the member or the message to the route file's `.proto` and run `scripts/proto.sh generate`.
2. Add a row to `routes` in the fixture, and a real answer to `goldens`.
3. Write a refusal with `api_error`. Write any other body the C builds through a Coax unit.
4. Run the API suite. It fails until the row and the answer agree.

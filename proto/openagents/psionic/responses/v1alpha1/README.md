# Psionic Responses profile v1

`psionic-gptoss-responses-v1` projects the normalized messages in
`responses.proto` onto Responses JSON at `POST /v1/responses`. It adds a new
alpha contract; preserved `openagents.inference.v1` identities are unchanged.

The baseline reference is Open Responses **2026-04-24**:
<https://www.openresponses.org/specification> and
<https://www.openresponses.org/openapi/openapi.json>. The gpt-oss compatibility
profile follows OpenAI's reasoning replay guidance:
<https://developers.openai.com/cookbook/articles/gpt-oss/handle-raw-cot>.

Scope: original 20b CUDA, greedy generation, 2048 context positions, 1–512
output tokens (default 256), low/medium/high reasoning, 48 input items and
8 sequential externally executed function tools. Bodies are at most 65536
bytes. Tool schemas are flat objects with at most 16 primitive fields and
optional string enums; strict decoding and arbitrary JSON Schema refuse.
The Responses adapter also accepts the original model’s optional
`<|constrain|>json` function-header label; other format labels refuse.
No tool executes in this process. Listed tools authorize only a handoff.
A call to an unlisted name (including the model's trained `browser.*` and
`python` built-ins) is returned as a function call for the client to refuse;
it replays under its own name and authorizes nothing.

Input supports text messages and text parts, raw `reasoning_text` replay,
function calls and matched string tool outputs. Analysis is kept through the
current tool sequence and removed from completed prior final turns. It is
transported as reasoning, never assistant display text. Input reasoning
content arrays are an explicit gpt-oss extension to the baseline's null-only
input content. The default SSE reasoning event names match gpt-oss's
`response.reasoning_text.delta/done`. Request header
`X-Psionic-Responses-Profile: openresponses-2026-04-24` selects the baseline's
`response.reasoning.delta/done` names. Item identities and event ordering are
otherwise shared. SSE is incremental, with a terminal response event.

Full input replay is required. `store` defaults false; storage, non-null
`previous_response_id`, background jobs, WebSockets, compaction, modalities,
parallel tools, nonzero temperature and unsupported parameters refuse.
This bounded profile is not a claim of passing every Open Responses test.

Coax owns HTTP parsing, authentication, JSON/schema admission, Harmony
mapping, item lifecycle, serializers, limits and deadline decisions. C owns
sockets, clocks, thread/device lifecycle and calls the admitted GPU program.
The server has one active model request, bounded connections and no request
content logging. Default binding is loopback; deployment is separate work.

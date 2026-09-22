# Classification callers

`oak classify` and `oak-mcp` use the gateway's `POST /v1/classify` contract.
They preserve ordered results, raw judgments, selections, aggregates, no-match
and uncertainty flags, review status, and incomplete usage. They do not infer
missing scores or turn an unreviewed result into an approved one.

## Configure the caller

Build both binaries with `cargo build --locked -p oak`. Keep the bearer key in
`OPENAGENTS_API_KEY` or a protected config file, as described in the
[caller guide](caller.md). Never put it in command arguments or MCP tool arguments.
The endpoint resolves from `--url`, `OPENAGENTS_BASE_URL`, then the config's
`base_url`. Workspace selection resolves from `--workspace`,
`OPENAGENTS_WORKSPACE`, then `workspace` in the config. The workspace becomes
`X-Workspace-Id` for `ask`, `models`, `classify`, and both MCP tools.

```sh
export OPENAGENTS_BASE_URL=https://gateway.example.com
export OPENAGENTS_WORKSPACE=workspace-id
oak models --quiet
oak classify --envelope request.json --request-id caller-owned-id --quiet
cat request.json | oak classify --envelope - --quiet
```

Use the discovered model's classification limits and a versioned
`openagents.classify.v1` envelope. The caller reads one JSON object, not NDJSON.
For example, a single-label request has this shape:

```json
{
  "v": "openagents.classify.v1",
  "model": "shared-kev",
  "capacity": "shared",
  "mode": "single-label",
  "policy": {
    "v": "openagents.classify-policy.v1",
    "name": "routing-v1",
    "select": {
      "single_label": {"ties": "no-match", "no_match": {"kind": "null"}}
    }
  },
  "inputs": [{"id": "ticket-1", "text": "I need a refund."}],
  "labels": [{"id": "refund"}, {"id": "other"}]
}
```

Replace the model and capacity with authorized discovery values. The gateway
validates the full envelope and policy. Thresholds are workload decisions;
the caller does not supply a universal confidence cutoff.

The CLI writes one JSON report to stdout. A transport or malformed-response
failure writes a JSON `error` object and exits nonzero. Diagnostics go to stderr;
`--quiet` suppresses routine summaries. An answered report exits 0, mixed results
use the existing mixed-result exit code, and refusals or unavailable results
remain nonzero. Inspect the report rather than treating every nonzero exit as an
unattempted request.

## Contract files

See the [OpenAPI contract](../api/openapi.yaml), [request schema](../schemas/classify-request-v1.json),
[response schema](../schemas/classify-response-v1.json),
and [request fixture corpus](../fixtures/classify-v1/README.md) for machine-readable
shapes and validation examples. Schema validation does not replace admission.

## Routes and private input

The gateway serves classification only through `POST /v1/classify`; it does not
publish GET classification or compatibility aliases. `POST /v1/systemone`
remains the native typed-question route. Both use the gateway's authorization,
capacity, quota, identity, and receipt checks, plus monetary admission when
configured. Use `GET /healthz` for process liveness and `GET /v1/models` for
authorized discovery. For example:

```sh
curl --fail --silent --show-error http://127.0.0.1:8080/healthz
oak models --quiet
```

The local URL is an operator-run gateway, not a public service promise. The
`oak models` command uses the configured endpoint and protected credential.
Keep input, keys, and workspace credentials out of query strings. Private
classification content belongs in a POST body; keys belong in authorization
headers supplied by the configured caller. A future compatibility alias must
use the same admission and accounting path before it can be advertised.

## MCP stdio

Configure an MCP client to launch `oak-mcp`, with the same protected environment
or config file as the CLI. Its optional flags are `--url`, `--config`,
`--workspace`, `--timeout`, and `--retries`.

The server implements newline-delimited JSON-RPC over stdin/stdout, initialization,
version negotiation, `notifications/initialized`, `ping`, `tools/list`, and
`tools/call`. It supports protocol versions `2025-11-25` and `2025-06-18`, preferring
`2025-11-25` when the requested version is unsupported. Initialization requires
client capabilities and client name/version. It advertises only the tools
capability; it does not offer resources, prompts, sampling, or tasks. The same
tool set also serves over Streamable HTTP through `oak-mcp-http`; the
[MCP server guide](mcp-server.md) covers both transports, sessions, and
per-request credential forwarding.

The tools are:

- `list_models`: no arguments; returns the authorized discovery document.
- `classify`: `request` is the complete classification envelope; optional
  `request_id` supplies an idempotency key. Endpoint, credentials, and workspace
  are operator configuration and are refused as tool arguments.
- `classify_texts`, `classify_dimensions`, `classify_multi_label`,
  `count_labels`, `review_uncertain`, and `decide`: the bounded inference
  tools — label sets and dimensions as arguments rather than raw envelopes,
  `decide` reaching `/v1/systemone` directly. The
  [MCP server guide](mcp-server.md) has the full table and bounds.
- `list_docs`, `read_doc`, `search_docs`, and `get_examples`: read the versioned
  public documentation bundled into the binary without resolving inference
  configuration or contacting a service. See the [documentation tools](mcp-documentation.md).

Inference can consume quota or money. Its tool annotation does not claim a
read-only or idempotent effect. Documentation tools are read-only, idempotent,
and limited to their bundled corpus; annotations do not grant execution authority.

Tool results carry both `structuredContent` and the same JSON serialized as a
text content block. A reported partial classification stays a report. HTTP
refusals and transport failures use `isError: true`; malformed protocol messages
use JSON-RPC errors. Closing stdin ends the server. Calls run sequentially;
there is no cancellation or progress extension, and an in-flight HTTP call
remains bounded by its timeout. A client that needs immediate termination can
terminate the subprocess.

This surface follows the MCP [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle),
[stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
and [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)
contracts for the capabilities it advertises.

## Bounds and retries

Envelopes are limited to 16 MiB, HTTP responses to 64 MiB, and MCP input messages
to 16 MiB plus 256 KiB of framing. The gateway's lower configured bounds still
apply. Oversized MCP messages are drained through their newline, rejected, and
the following message can proceed. HTTP redirects are refused.

The default per-attempt timeout is 60 seconds, with at most three configured
retries. `Retry-After` is honored up to 60 seconds per wait. Completed classification
reports are never retried, even under a failing HTTP status: their observed
outcomes must not be replaced by another inference attempt. Typed temporary
admission refusals can retry. A POST transport failure or incomplete body retries
only when the caller supplied a request ID; retries preserve that key and advance
`x-attempt`. Without a request ID, an uncertain POST completion returns a failure
without forwarding again. GET discovery can retry transport failures. These are
bounded attempts, not a promise of free or exactly-once inference.

Subprocess/HTTP tests compare complete documents across CLI and MCP for all four
modes, including mixed outcomes, uncertainty, no-match, ranking, and unreviewed
status. They also cover workspace resolution, retries, partial reports under 503,
redirect refusal, malformed and oversized input, timeouts, authentication errors,
and MCP lifecycle errors. Request fixtures pass the actual gateway planner.
These fixtures establish caller behavior, not model quality or deployed service
availability.

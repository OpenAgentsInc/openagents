# Cloud computers from Coder

2026-10-17. Status: implemented for 1.0.0
([#11823](https://github.com/OpenAgentsInc/openagents/issues/11823)).

Coder reaches the account's cloud computers through the public Computers API
and its Rust client, `openagents_sdk::computers`
([contract](../../sdk/computers.md)). The `openagents` CLI holds the signed-in
session and the client; Coder's `cloud_computer` agent tool runs the CLI, the
same way the `computer` tool runs `openagents computer` for linked computers.

[Runtime index](README.md)

## What uses the Computers API

| Surface | What it does |
| --- | --- |
| `openagents computer cloud list`, `show`, `capabilities` | Read the account's cloud computers, one computer, and the sizes, placements, and file limits the account can use |
| `openagents computer cloud create` | Create a computer with an optional size, public placement (`auto`, `our-cloud`, or `fast-sandbox`), and saved environment version |
| `openagents computer cloud exec`, `output`, `cancel` | Run a non-interactive command in the workspace, read its output from byte offsets, and cancel it |
| `openagents computer cloud read`, `write` | Copy a file of at most 4 MiB from or into the workspace, checked by SHA-256 |
| `openagents computer cloud stop`, `wake`, `delete` | Lifecycle intents, with an optional bounded wait for the observed state |
| Coder's `cloud_computer` tool | The same verbs for the agent, offered with the OpenAgents CLI tools |

Each verb's help is in `openagents computer --help`.

## Request identity and retries

`create` and `exec` save their request under
`~/.openagents/computers/cloud-requests/` (mode `0600`) before sending it, and
print its `key`. The saved file holds the account, deployment, key, and
settings, never the session token.

- After a lost answer, a timeout, or a restart, rerun with only `--key KEY`.
  The CLI sends the saved request again with the same `Idempotency-Key` and
  the same bytes, so the service answers with the computer or command it
  already made. It never starts a second one.
- A key with different settings is refused before anything is sent
  (`kind: key_conflict`).
- A file write whose answer is lost is checked by reading the file's hash; the
  upload is never sent again.

## Waiting, detaching, and cancelling

`--wait` bounds how long the CLI waits: at most 900 seconds for a computer
and 3,600 seconds for a command (the tool caps both at 600). When the wait
ends, the work keeps running and the result says so (`running: true` for a
command). Stopping the tool call or the CLI process also only stops waiting.
Only `cancel`, `stop`, and `delete` change remote work, and each is an
explicit request.

## Permissions

- Every call carries the signed-in session; the service checks the account on
  each one. The CLI can't name another account, a provider, a Boat sandbox,
  or a GCE instance.
- The agent tool asks the user before `create`, `wake`, `stop`, and `delete`,
  before an `exec` that Coder's effect classes don't mark as read-only, and
  before a `read` or `write` that replaces a file. Denied commands are
  refused.
- `openagents mcp serve` runs only the read-only verbs: `cloud list`,
  `cloud show`, `cloud capabilities`, and `cloud output`. The command tree
  declares the others as spending, publishing, or writing here, so an MCP
  caller can't create, run, copy, stop, or delete.

## Failures

Under `--json`, a failure is `{"error": TEXT, "kind": KIND}`, plus `key` when
the result is unknown. Kinds:

| Kind | Meaning |
| --- | --- |
| `uncertain` | The answer was lost. Rerun with the same `--key` |
| `signed_out` | The sign-in expired. Run `coder login`, then retry |
| `unsupported` | The computer or placement doesn't offer that, including an explicit placement the service can't meet. Nothing was started |
| `quota`, `rate_limited` | The account's computer limit, or too many requests |
| `access_denied`, `missing` | Not this account's computer, or it's gone |
| `key_conflict` | The key names different settings |
| `not_awake` | Wake the computer before copying files |
| `file_exists`, `file_changed`, `too_large` | File transfer refusals |
| `usage` | The arguments are wrong; nothing was sent |

## What stays on its existing path

These paths keep their meaning and code in 1.0.0:

- **`coder delegate AGENT --on boat`** and `coder remote ...` run on the
  operator's own Boat account (`BOAT_API_KEY`). Boat's integrated agents,
  steering, event pages, saved templates, and per-turn credentials in the
  command's memory (#11766, and the #11771 Vertex memfd handoff) aren't in
  the public Computers contract. Moving them would drop those features or
  put credentials in saved command settings, which the contract forbids.
  `--on boat` never becomes a Computers API placement, and generic
  placement never takes over a Boat job.
- **`--on gce`** runs on the operator's own GCE pool from `openagents cloud
  up`, under its local grant. It isn't an account computer.
- **`openagents boat run|stop|delete|list`** keeps its Boat sandbox names.

Moving remote delegation onto the Computers API waits on two public
capabilities: agent prompts on a computer, and scoped credential references
for model and Git clients
([#11813](https://github.com/OpenAgentsInc/openagents/issues/11813)).

## Migration and rollback

- **Migrate.** Use `openagents computer cloud ...` or the `cloud_computer`
  tool for account computers. The older `/v1/computers` list and the
  enrolled-machine `computer` verbs are unchanged.
- **Roll back.** The verbs are additive. To stop using them, stop calling
  them; nothing else reads `cloud-requests/`. Delete computers you made with
  `openagents computer cloud delete ID` first. Removing the saved request
  files only loses the ability to retry those exact requests.
- **Server compatibility.** The CLI uses only published operations; Coder
  1.0.0 and older clients keep their existing addresses and behavior.

# NIP-AGENT: Saved custom agents

`draft` · `openagents.agent.definition.v1` · `openagents.agent-definition-export` v1 · kind `30187`

## Scope

A saved agent combines a name, instructions, a runtime ID, an optional model,
and requested access. It is inert configuration. This profile gives private
exports a portable payload and exact definition reference. It does not replace
AP identity, CAP discovery, SESS conversations, WORK ownership, HOST admission,
CJ execution, POL permissions, or RUN evidence.

The OpenAgents app initially stores account-owned definitions through the
[SDK API](../../docs/sdk/agents.md). Account identity and a Nostr key are separate:
an export's signer must be selected explicitly by the reader. An account ID
is never proof that the signer controls that account.

## Definition payload

The JSON object contains `schema` (`openagents.agent.definition.v1`),
`definition`, and, only for a fork, `forked_from`: `{published, revision}`,
the 64-hexadecimal published agent ID it was copied from and the positive
revision it copied. A fork's export never carries grants. Readers that
predate `forked_from` refuse an export that has it. The definition contains `id` (64 lowercase hexadecimal
characters), `revision` (positive integer), `spec`, and `deleted` (boolean).
`spec` contains the fields below; `fallbacks` may be omitted when empty, and
`autonomy` when unset:

| Field | Contract |
| --- | --- |
| `name` | Nonblank, at most 80 UTF-8 bytes, no control characters. |
| `description` | At most 512 UTF-8 bytes, no control characters. |
| `instructions` | Nonblank, at most 65,536 UTF-8 bytes, no NUL. Skills a definition was built with are part of it. |
| `runtime` | 1–128 ASCII letters, digits, `.`, `_`, `:`, or `-`. An installed implementation ID, never a command or URL. |
| `model` | Null or 1–256 UTF-8 bytes without control characters. |
| `access` | `host_default`, `read_only`, or `workspace_write`. A request that the executing host must enforce or refuse. |
| `fallbacks` | Optional array of at most 4 `{runtime, model?}` objects, tried in order when `runtime` is not ready on the host. Each runtime uses the `runtime` rules, appears once, differs from `runtime`, and is not `sdk` or `any`. |
| `autonomy` | Optional object: how the agent runs on its own. `check_ins` (boolean) asks hosts to turn timed check-ins on for its chat without being asked, every `every_minutes` (5–1440); `authority` (boolean) asks for standing authority over other chats with them, which the host grants with a log entry and the owner can revoke. `stall_minutes` (5–240) is how long a chat or claim it manages may go with no progress before the host raises it (hosts default to 20). `shell_timeout_secs` (10–3600) is the longest one shell command may run; the host passes it to the runtime. `wakes` (at most 16 of `chat_finished`, `chat_errored`, `chat_needs_answer`, `issue_opened`, `issue_closed`, `claim_stale`, `chat_stalled`, `claim_no_progress`; empty is every wake) is what the agent wakes on, and `coalesce_secs` (0–1800) how long events wait for quiet. Readers ignore fields and wakes they don't know. Grants are not part of a definition: they belong to the account that grants them. |

The `instructions` limit was 16,384 bytes until 2026-10-18
([#12114](https://github.com/OpenAgentsInc/openagents/issues/12114)). A reader
from before then refuses a longer definition as invalid.

`runtime` `any` (with a null model) leaves the choice to the host. The runtime,
model, fallbacks, and autonomy are settings of the agent, not part of its identity: a
revision that changes only them is the same agent, and an assigned conversation
uses the new order from its next turn. The identity fields are `name`,
`description`, `instructions`, and `access`.

An SDK runner uses runtime `sdk` and a model naming an operator-installed
runner. `host_default` means the host's configured access; it promises no sandbox.
Definitions cannot add executable paths, credentials, spending, approval, or
enrollment authority. Unknown fields, schemas, and access values are refused.

## Private carriage and selection

Use the existing private artifact envelope, kind `3188`, NIP-44 v2, and the
shared [artifact contract](contracts.md). The artifact schema is the payload
schema above. Its digest covers JCS payload bytes; its size is the exact byte
length. The signed recipient and mailbox have their existing meanings.

The consumer verifies the original signature, recipient, decryption, artifact
digest, schema, payload bounds, and an explicitly selected signer, definition
ID, revision, and digest. A newer revision cannot silently replace an assigned
revision. A deletion prevents new assignments; it does not erase prior evidence
or cancel a running session. Sharing the encrypted envelope grants no execution
rights. Public AP export is a separate deliberate action and is not implemented
by this profile; it is the next section.

## Public agents

A published agent is a Nostr key of its own that a person owns. Revised
2026-10-20 for openagents.com profiles ([#12255](https://github.com/OpenAgentsInc/openagents/issues/12255)).

### The agent key

Each published agent has its own key, independent of its owner's. The host
that runs the agent holds the secret. An implementation MAY derive it from a
secret only that host holds; it MUST NOT be derivable from the owner's
public key or from anything published. Rotating the key makes a new agent
identity; grants and follows don't carry over.

### Owner attestation

The owner proves ownership with a [NIP-OA](../block/NIP-OA.md) `auth` tag:
the owner's key signs `nostr:agent-auth:<agent pubkey>:<conditions>`. The
conditions MUST include `created_at<T` with `T` at most one year after the
signature, and MAY add nothing else, so one tag covers the agent's kind 0
and its definition events. The owner's client signs; the host never holds
the owner's secret. A host that receives the tag verifies the signature and
that the owner key is one of the publishing account's keys before it uses
it.

Every event below carries exactly that `auth` tag. A reader MUST verify it
under NIP-OA before showing "owned by" and MUST NOT treat the owner as the
author.

### Agent profile (kind 0)

The agent publishes an ordinary kind 0 with `bot: true` (NIP-24), `name`,
`about` (the definition's description), `website` (the definition's HTTPS
URL), and, when it has one, `picture`. Its tags are the `auth` tag and
nothing that would make clients list it in the owner's timeline.

### Public definition (kind 30187)

An addressable event by the agent key:

| Part | Contract |
| --- | --- |
| `d` tag | The published agent ID, 64 lowercase hexadecimal characters. |
| `auth` tag | The owner attestation above. |
| `p` tag | The owner's key, so a reader can list an owner's agents with `{"kinds":[30187],"#p":[owner]}`. |
| `r` tag | The HTTPS URL of the same definition as JSON (`/agent/{id}/definition.json` on openagents.com). |
| `status` tag | Optional. `revoked` retires the agent: readers drop it from the owner's list and say it was retired. |
| `alt` tag | `OpenAgents agent definition` (NIP-31). |
| `content` | The `openagents.agent-definition-export` version 1 document as JSON, the same bytes the `r` URL serves, or empty when revoked. At most 262,144 bytes. |

A reader takes the newest event per `(pubkey, d)` with a valid signature and
`auth` tag, and drops any whose `p` tag differs from the `auth` tag's owner.
Instructions in a public definition are public; a private agent stays on
kind `3188` above.

Revoking publishes a replacement with `["status", "revoked"]` and empty
content, signed by the agent key under the same attestation. The kind 0
stays, so links keep resolving, and a reader that sees the revoked
definition says the agent was retired.

### Runtime evidence

A live `30187` MAY carry one or both of these tags:

- `["e", "<3202 release event ID>", "", "att-release"]`
- `["e", "<30203 endpoint event ID>", "", "att-endpoint"]`

Each marker occurs at most once. IDs are 64 lowercase hexadecimal
characters. These tags are the owner's declaration of what the agent runs
on. A matching workload name, model name, or operator is not an agent link.
A new runtime publishes a replacement definition; `{}` clears both references
through the site's runtime API. Retired definitions supply no runtime evidence.

Readers verify each release with NIP-ATT, and verify independent `3203`
statements against that release's listed builders, source, and image digest.
A release signature alone does not establish hardware attestation. Before
showing a verified endpoint, readers fetch its exact release and the current
release head and run the NIP-ATT client checks, including evidence roots,
expiry, image measurement, and endpoint key binding. If both tags occur,
the endpoint MUST name the pinned release. Unsupported or failed checks
supply no verified endpoint. The site uses the same publisher trust policy
and Confidential Space verifier as `/att`. Every displayed release, rebuild,
endpoint, and receipt links to `/event/{id}`.

### Published gateway receipts — kind `3204`

This regular event publishes one execution receipt the owner explicitly
chooses. It is separate from NIP-PYLON's buyer receipts (`3201`). No execution
or page read automatically publishes a gateway receipt.

| Part | Contract |
| --- | --- |
| Signer | The owner named by the agent's current verified `30187`. |
| `p` tag | Exactly `["p", "<agent public key>"]`. |
| `a` tag | Exactly `["a", "30187:<agent public key>:<published agent ID>"]`. |
| `x` tag | Exactly `["x", "<execution receipt digest>"]`. |
| `alt` tag | `Owner-published gateway receipt`. |
| `content` | `{"v":"openagents.agent-gateway-receipt.v1","requires":[],"receipt":<ExecutionReceipt>}`. At most 262,144 bytes. |

There is exactly one of each binding tag (`p`, `a`, `x`). The receipt is the
`openagents.receipt.execution.v1` document defined by `crates/receipts`:
its existing seal covers all fields except `digest`, using sorted-key JSON
and SHA-256. Readers verify the event signature, publication version and
features, agent address, current owner, receipt schema, required fields,
and receipt seal. Unknown features, duplicate bindings, another agent or
owner, and retired agents refuse. A page shows at most 20 receipts, newest
first, deduplicated by receipt digest. A changed owner no longer authorizes
the prior owner's publications on that agent's page.

The owner signature attributes a claim. A receipt's self-computed seal does
not prove the gateway issued it, that a remote execution occurred, or which
hardware ran it. Readers label these as owner-published receipts and do not
turn them into independent reviews, XP, or hardware-attestation badges.

Publication discloses the entire selected receipt, including any account,
workspace, job, timing, and operational references it carries. Owners review
that metadata before signing. Request and answer bodies and credentials
remain outside the receipt. Private receipts remain private unless selected.
Removing a runtime pin or retiring an agent does not erase public events
already copied by other readers.

### Implementation

openagents.com (`crates/openagents-web`, `published_agents` and
`agent_keys`) holds the key for agents published there, takes the owner's
tag at `PUT /v1/published-agents/{id}/owner`, and publishes both events.
`/agent/{id}` and `/agent/{npub}` draw the same agent. `nostr::agent_public`
builds and reads the events.

The publisher can `PUT /v1/published-agents/{id}/runtime` with
`{"release":"<event ID>","endpoint":"<event ID>"}` (either field may be
omitted) to save runtime pins and publish a replacement definition. An
unexpired owner tag is required. The response includes `runtime`, the signed
`definition`, and the number of relays that accepted it (`published`).

`receipts_nostr::event` signs a selected execution receipt with the
owner key. The publisher then posts that signed event as JSON to
`POST /v1/published-agents/{id}/receipts`, using the account bearer. The site
checks the publisher account, current agent owner, event time (within 120
seconds), bindings, and seal, and forwards the event unchanged. A relay
failure returns an error and can be retried with the same signed event.
The page reads public events from its configured profile relays; it never
reads the gateway's private receipt log.

For a local owner key, review the chosen receipt, set `NOSTR_SECRET_KEY`
without writing it to a file, and sign and upload that one receipt:

```sh
cargo run -p receipts-nostr --example publish -- definition-event.json selected-receipt.json > signed-receipt.json
curl --fail-with-body -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H 'Content-Type: application/json' --data-binary @signed-receipt.json \
  "https://openagents.com/v1/published-agents/$AGENT_ID/receipts"
```

`definition-event.json` is the signed `30187` event, for example the
`definition` returned by the runtime API. Keep private key material out of
these files. A browser or remote signer can sign the same event shape without
using the local-key example.

## Work and sessions

An assignment pins the definition revision and selecting account, host, project,
and conversation. Existing SESS command identities distinguish retry from new
work. The host checks current account, project, and access before dispatch, then uses
the first runtime in the agent's order that is installed, enabled, signed in by
a check that starts no login, offers the model, and enforces the access; then
its own default. It records in the conversation which runtime ran and why it
differs from the first choice. A saved assignment is not proof of execution. Lost
contact cannot mean completion; uncertain execution must be inspected before
new work is issued. Stop is an intent until the session reports it stopped.
Completion is distinct from WORK acceptance and LAB settlement.

The app's HTTP assignment book supplies bounded summaries and durable retry
identities. The existing coder-sync session owns the transcript and controls.
There is no second writable session transport in this profile. Remote relay
execution would require an admitted CAP/CJ role; it is not implied here.

## Implemented roles

`agent-contract` validates the shared payload. `nostr::agent_profile` publishes
and consumes pinned private artifacts with producer/consumer tests covering
wrong signers, pins, recipients, and modified bytes. The app's account store and
local host assignment adapter are separate roles. General Nostr discovery,
public publication, remote runner enrollment, and paid work are outside this
implementation. See [the implementation plan](../../docs/custom-agents.md).

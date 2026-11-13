# Consent ledger and memory log v1

Coder keeps two hash-chained logs on disk. Each is JSON lines: one event a
line, each event chained to the one before by SHA-256.

| Log | File | Codec | Rules |
| --- | --- | --- | --- |
| Consent ledger | `~/.openagents/coder/consent/events.jsonl` | [`coax/lib/agent/engine/consent-gate.coax`](../../coax/lib/agent/engine/consent-gate.coax) (`gt_line`, `gt_unline`) | `psionic/lib/harness/consent.coax` |
| Memory log | `~/.openagents/coder/memory/events.jsonl` | [`coax/lib/agent/engine/memory-log.coax`](../../coax/lib/agent/engine/memory-log.coax) (`mem_line`, `mem_unline`) | `psionic/lib/harness/memory.coax` |

- Fixtures: [`proto/coder/consent-memory-ledgers-v1.json`](../../proto/coder/consent-memory-ledgers-v1.json)
- Check: `coax/web/tool.sh coax/lib/agent/engine/ledger-profile.coax proto/coder/consent-memory-ledgers-v1.json`
- Registry rows: `coder-consent-ledger-v1`, `coder-memory-log-v1` (step C12, [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250))

The digest covers typed data, not the line's text. So the line's key order
and number format belong to the codec, and the fixtures pin them.

## A consent event

Every member is a whole number, except the three digests, which are 64
lowercase hex digits.

| Member | Meaning |
| --- | --- |
| `sequence` | 0 for the first event, then one more for each |
| `op` | 0 grant, 1 revoke, 2 send, 3 refuse |
| `tick` | When, in seconds |
| `kind`, `id`, `purpose` | The destination. `kind`: 0 tool, 1 agent, 2 peer, 3 export. `purpose`: 0 call, 1 delegate, 2 compute, 3 train. System One is tool 1, call |
| `classes` | The content classes the grant covers, as bits (1 is session content) |
| `start`, `until` | The grant's window, in seconds |
| `person` | Who granted it. 1 is the person at this machine |
| `grant` | The grant this event is, uses or revokes |
| `why` | 0 ok. On a refusal, the reason (5: revoked; 1: no grant) |
| `c0` to `c4` | The grant that covered each content class of a send |
| `sends`, `bytes` | The grant's sends and bytes so far |
| `content` | The digest of what was sent. The ledger never holds the text |
| `previous`, `digest` | The chain |

## A memory event

| Member | Meaning |
| --- | --- |
| `sequence` | 0 for the first event, then one more for each |
| `op` | `remember`, `revise` or `forget` |
| `scope`, `owner` | `session`, `user` or `project`, and whose: 1 for the user, a number made from the folder's path for a project |
| `reason` | `none`, `correction`, `update` or `resolve` |
| `source`, `origin` | `event` or `cite`, and the digest of the session line the fact came from (zeros: the command line) |
| `from`, `to` | The cited passage's range |
| `start`, `until` | When the fact holds, in seconds |
| `keys`, `texts`, `ids` | How many of `bytes` are the key, the text and the cited ids |
| `redacted` | True once the fact was forgotten: its text's bytes are hidden and its digests stay, so the chain still verifies |
| `content`, `previous`, `digest` | The content's digest and the chain |
| `bytes` | The key, text and ids, in hex |
| `key` | The key again as text, for a person reading the file. The reader ignores it |

## What the check holds

The fixture's ledgers are files that the Coder CLI of this tree wrote in a
throwaway home on 2026-11-13: two grants, a revoke, a task sent through the
gate, a second revoke and a refused task; a refusal with no grant; and a
memory log with two remembers, a revise and a forget.

- Every line reads through its codec and is written back to the same bytes.
- Each consent ledger replays (`cs_replay`), and the memory log verifies
  and replays (`mm_verify`, `mm_replay`).
- Each reader refuses its five `refused` lines.

`ledger-profile.coax --gate LEDGER TEXT` runs the gate that Coder runs
before a task leaves the machine. It appends the send or the refusal to the
ledger and sends nothing.

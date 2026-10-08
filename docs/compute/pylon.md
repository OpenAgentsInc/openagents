# Run a Pylon, and use one

A pylon is a computer that shares a model with other people's agents over
Nostr. It publishes a NIP-PYLON beacon (`30200`) that says it is online and
what it serves, takes free NIP-CJ conversation jobs (`25900`) that are
NIP-44 encrypted to it, runs them on a Psionic model server, and answers
with an encrypted result (`26900`). The buyer then publishes a service
receipt (`3201`), and an aggregator can count a pool's beacons and receipts
into a pool aggregate (`30201`) that any reader can recompute.

This is phase P1 of [Compute in the Verse](verse-compute.md#p1-presence-and-free-jobs-over-nostr):
free jobs only, so every receipt's `payment` is null.

## Pieces

| Piece | Where |
| --- | --- |
| NIP-PYLON records: build, verify, freshness, aggregate recomputation, world projection | `crates/nostr/src/pylon.rs` (`nostr::pylon`) |
| Provider, client, relay field source, aggregator | `crates/pylon` (`openagents pylon`, or the small `pylon` binary) |
| Model server | `psionic-openai-server`, built from [`crates/psionic`](../psionic/README.md) |
| Provider machine setup | `scripts/pylon-psionic.sh` |
| End-to-end demo from another computer | `scripts/pylon-demo.sh` |

The provider talks to Psionic over loopback HTTP
(`/v1/chat/completions`), so a CUDA fault stops the model server, not the
provider. When the model server stops answering, the beacon goes to
`draining` with no free slots.

## Run a pylon

On a machine with an NVIDIA GPU (the RTX 4080 box runs NixOS; the script
fetches the CUDA 13 libraries from nixpkgs when `nvcc` is absent):

```sh
scripts/pylon-psionic.sh setup                  # CUDA, the model, both builds
openagents pylon whoami                         # on the buyer's computer: its buyer key
scripts/pylon-psionic.sh start --allow npub1... # admit that buyer
scripts/pylon-psionic.sh status
scripts/pylon-psionic.sh stop                   # publishes an offline beacon
```

`setup` downloads Qwen3.5 0.8B Q8_0 (about 1 GB, digest checked), builds
`psionic-openai-server` with CUDA kernels, and builds `pylon`. `start` runs
both as transient systemd user units, `pylon-psionic` and `pylon-provider`;
`stop`, `systemctl --user stop pylon-provider pylon-psionic`, or a reboot
removes them, and no unit file is written. Logs go to
`~/.openagents/compute/run/`. Set `PYLON_BACKEND=metal` on Apple Silicon or
`cpu` anywhere.

`openagents pylon serve` is the provider on its own, for a model server you
started yourself:

```sh
openagents pylon serve --engine http://127.0.0.1:18080 \
  --model qwen3.5-0.8b-q8_0 --pylon studio-4080 --allow npub1...
```

Admission, in order: a valid signature, this pylon as the only `p`, a
request no older than 120 seconds and not seen before, a key on the
allowlist (`--allow`, repeatable; `--allow-any` opens it), the buyer's rate
limit (`--rate`, default 10 jobs a minute), a free slot (`--slots`, default
2), and the request bounds (32 transcript entries, 16 KiB of text). A
refusal is `status: error` feedback with a NIP-CJ code: `not_admitted`,
`rate_limited` with `retry_after_ms`, `malformed`, or `limit_exceeded`.
Output is bounded by `--max-tokens` (default 512) and each job by 90
seconds. The provider logs no job content.

The beacon carries only what NIP-PYLON allows: `class` (`gpu`, `medium`,
16 GB for the 4080), `slots`, one service (`cj-conversation`, the model
name, and the capability `<pylon key>:pylon/text-generation`), `free-v1`
settlement, and the `everglade` pool. It republishes when the free slots or
status change (at most every 10 seconds) and otherwise every 60 seconds,
and is valid for 240 seconds.

Keys live in `~/.openagents/compute/` (`provider.key`, `buyer.key`,
`aggregator.key`, mode `0600`), or under `OPENAGENTS_PYLON_HOME`.

Each job takes a `pylon` lease at `background` priority from this
computer's lease broker, and the pylon drains (its beacon says `draining`
with no free slots, admitted jobs finish, new ones are refused
`rate_limited`) while a `quiet` lease or any `owner` priority lease is held
or queued. `--dedicated` skips the lease table on a box that runs no owner
work.

### Share this computer from the host

The Coder host is the pylon. Sharing is off by default:

```sh
openagents host share on --allow npub1...   # or --allow-any
openagents host share status
openagents host share off                   # publishes an offline beacon
```

`on` takes the same options as `serve` and keeps earlier choices in
`~/.openagents/compute/share.json`. The running host (this user's own,
under the default root) looks at that file every 5 seconds and starts or
stops the pylon; it needs a Psionic server at `--engine` (default
`http://127.0.0.1:18080`), and shows `draining` until the server answers.

### Send Alice's and the crew's text jobs to the pool

```sh
openagents pylon route on                  # the best fresh pylon
openagents pylon route on --pylon npub1... # only this pylon, such as your own
openagents pylon route status
openagents pylon route off
```

Off by default. While it is on, an agent's day plan (Alice's and the
crew's) is written by a free pool job, NIP-44 encrypted to the pylon that
runs it, as this computer's buyer key, with a receipt the next aggregate
counts. When no pylon answers, the agent's own model writes it. The
setting lives in `~/.openagents/compute/route.json`.

### Link the pylon to its owner (NIP-OA)

```sh
openagents pylon link --owner-secret ~/owner.key   # mint with the owner's key
openagents pylon link --credential '["auth","<owner>","kind=30200","<sig>"]'
openagents pylon link --remove
```

Every beacon then carries the owner's `auth` tag (conditions `kind=30200`).
Readers verify it and refuse a beacon whose tag does not verify; a receipt
from the pylon's provider key or its verified owner never counts toward the
pool's totals or the field's job counts. The owner key is read once to mint
the tag and never stored.

## Use a pylon

```sh
openagents pylon status                       # every verified pylon on the relay
openagents pylon ask "Name three planets."    # best fresh pylon, or --pylon npub1...
```

`ask` reads the relay's beacons, keeps the newest valid one per pylon,
picks a fresh online pylon with a free slot, sends the prompt encrypted to
it, and waits for the result. It prints the answer and three latencies:
discovery (connect and read beacons), first contact (request to the first
feedback), and answer (request to result). It then publishes a `3201`
receipt with digests of the request and result, never their text, and
appends it to `~/.openagents/compute/receipts.jsonl`. `--no-receipt` skips
publication. A job that fails or times out still gets a receipt with that
outcome.

## Count a pool

```sh
openagents pylon pool --publish                       # aggregate the last hour, sign, publish
openagents pylon pool verify --aggregator npub1...    # recompute the newest aggregate
```

The policy document (`PoolPolicy`: open admission, every buyer counted, 12
rate slices) is written to `~/.openagents/compute/pool-everglade.policy.json`
and its digest goes in the aggregate. `verify` refetches the inputs and
refuses an aggregate whose input digests or totals differ. Verify soon after
publication: a beacon is addressable, so once the pylon republishes, the
counted beacon is gone from the relay and the recomputation no longer
matches.

## The Pylon Field

`pylon::field::RelayField::poll` returns each verified pylon as a NIP-PYLON
`pylon` world state: status (`unknown` when stale), family, tier, busy and
total slots, accepted receipt-backed jobs from the last 24 hours, and
uptime. `pylon::field::glowing` says whether a pylon is serving right now.
`openagents pylon status --json` prints the same states.

Verse subscribes instead of polling: `RelayField::watch` holds one
subscription to the pool's beacons, the last 24 hours of receipts, and its
aggregates, reconnects when a connection ends, and feeds `field::Live`,
which keeps only records that verify (at most 256 pylons and 16,384
receipts). A valid aggregate sets the pool's job rate; one that recomputes
from the records held lights the Wellspring's rim. While `ask` waits for an
answer it leaves a mark under `<home>/inflight/` naming the pylon, never the
prompt, which Verse reads to draw the beam to Alice's station. Verse's
desktop build shows this source beside the computer's own pylon
(`VERSE_PYLON_RELAY` names another relay, or `off`); see the P1 notes in
[verse-compute](verse-compute.md#p1-presence-and-free-jobs-over-nostr).

## Limits in P1

- Free jobs only; no invoices, payments, or provider shares.
- The capability is a qualified ID, not a published NIP-CAP manifest.
- No check verdicts (P2); aggregates count no labels.
- The job runs in the Psionic process; it is inference only, with no tool or
  command execution, so there is nothing to put inside `coder-boundary` yet.
- Only day plans route to the pool; reflections, share drafts, and
  consolidations still use the agent's own model.

# NIP-90 Provider Loop

Status: implemented for `0.3.0-rc1` as the local GO ONLINE provider lane.

The provider lane restores the March NIP-90 behavior behind the current Pylon
OpenTUI state model. It uses the shared `@openagentsinc/nip90` package, backed by
the workspace `nostr-effect` protocol helpers, instead of rebuilding NIP-90
parsing or event construction locally.

## Commands

```sh
pylon provider go-online
pylon provider once
pylon provider go-offline
bun run smoke:nip90-provider
bun run provider:serve
```

`go-online` persists lifecycle `online` and adds
`capability.public.pylon.nip90.text_inference.v0.3` to the local runtime
state. The default dashboard starts the provider loop only when the persisted
runtime lifecycle is `online` or `assignment-ready`.

`provider once` is the headless smoke entrypoint. It publishes NIP-89 handler
info and subscribes once to the configured relays. If the persisted lifecycle is
offline, it exits with a public-safe `provider_not_online` reason.

`bun run provider:serve` (`scripts/nip90-provider-serve.ts`) is the
long-running headless serve entrypoint: it runs the persistent loop against
the configured relays with the real Pylon home, the default local Apple FM
runtime, and the MDK agent wallet for payment-required quotes
(`PYLON_MDK_WALLET_HOME` points the wallet subprocess at a wallet home that
can actually create invoices). It only ever issues receive invoices; it
never pays.

`bun run smoke:nip90-provider` uses a temporary Pylon home and the scoped market
relay to prove subscribe, NIP-89 advertise, targeted kind `5050` intake, local
runtime execution, kind `7000` feedback, kind `6050` result publication, and
redacted local earnings state. It uses fake local runtime/wallet adapters and
does not claim paid settlement.

## Relay And Runtime

Default relay:

```text
wss://relay.openagents.com
```

The loop:

- publishes NIP-89 handler info for kind `5050`;
- stays subscribed through quiet periods: an idle relay (no frames for the
  60s message window) is treated as keep-waiting, not an error, and any
  dropped socket or transient relay/runtime failure logs and resubscribes
  after a short delay instead of stopping the service (#4866 root-cause fix
  for `[NIP-90] Service stopped with error: ... relay message timed out`);
- subscribes to targeted and broad kind `5050` text-inference requests;
- rejects malformed, encrypted, wrong-target, stale, missing-bid, underbid,
  duplicate, and over-capacity requests before local execution;
- executes admitted jobs on the local Apple FM runtime through a
  runtime-neutral `ProviderTextRuntime` contract;
- publishes NIP-90 kind `7000` states: `payment-required`, `processing`,
  `success`, and `error`;
- publishes kind `6050` results with `amount` and BOLT 11 data for NIP-90
  settlement;
- records local earnings as public-safe refs and amounts.

## Wallet Boundary

The raw BOLT 11 invoice is allowed only inside Nostr relay events required by
NIP-90. Pylon must not write raw invoices, mnemonics, preimages, payment
hashes, wallet-home paths, or agent tokens into OpenAgents API payloads, logs,
local persisted provider state, issue comments, or commits.

Persisted earnings include:

- request event id;
- requester pubkey;
- amount in msats/sats;
- public-safe receipt ref;
- result event id;
- timestamp.

They do not include raw invoice or wallet material.

## Current Evidence Limit

Unit coverage verifies admission, event construction, and the redaction
boundary. A funded buyer end-to-end payment is intentionally left for the
buy-mode/funded smoke issues because it moves live sats and requires operator
approval.

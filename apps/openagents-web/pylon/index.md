# Pylon

> A live, free toy decision network. Run one command on a Mac or Linux computer; its signed Pylon beacon appears at https://openagents.com/pylon.

## Join

Inspect the script before executing it if you prefer:

```sh
curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh
bash /tmp/join-pylon.sh
```

Prerequisites: Git, a working C/C++ toolchain, Python 3 and an Internet connection that permits HTTPS and secure WebSockets. On macOS install Apple's Command Line Tools (`xcode-select --install`). On Linux install Nix; the script uses the repository's pinned development environment. The first build takes longer than later starts. No Bun, wallet, subscription, API credential, inbound port or public IP is required to join.

The script clones or updates `OpenAgentsInc/openagents`, builds Coax, `psionic-serve` and Pylon, starts the toy engine locally, then joins `wss://relay.openagents.com`. It runs in the foreground. Wait for **online, waiting for jobs**, then open the live page.

```sh
bash /tmp/join-pylon.sh --label "My laptop" --slug my-laptop
```

The script's `--help` is the authoritative option list. `--source DIR` reuses an existing checkout for rehearsal; `--no-update` retains that checkout's revision. The default working directory is `~/.openagents/pylon` and state/logs live under `~/.openagents/pylon/state`. `--relay`, `--port` and `--slug` support isolated rehearsals. Ctrl-C stops both processes; another terminal can run `bash /tmp/join-pylon.sh --stop`. The last beacon can remain visible until its expiry (at most five minutes).

## What the live page means

- Each machine row comes from a kind-30200 NIP-PYLON beacon. The shared Coax reader verifies the original event ID and BIP-340 signature, hashes the decoded content, validates its schema and provider binding, and checks freshness. A self-signed beacon proves control of its publishing key, not the operator's identity or hardware claims. Optional owner authorization signatures are not verified by this demo view.
- Rows show the label, first advertised full model identity (`model@algorithm:digest`), free/total slots, observation age, and full `npub`. Identities wrap without hiding their algorithm or digest; row height expands with them. Later observations replace a row only at a newer generation or a newer observation in the same generation. Expired or non-online machines remain visible as offline.
- **Answers** counts verified kind-26910 events; **status** counts verified kind-27010 events received since this page opened, for providers with a verified beacon in this view. The contents remain encrypted. A status is not necessarily success or failure. Counts are provider-wide, not independent per machine when a key publishes multiple slugs.
- These work events are ephemeral (kinds 20000–29999). Reload resets the counters. The page never presents stored history as a live answer count. It de-duplicates 64 event IDs per page session and freezes the count when full; reload starts a new window. At most 64 machine rows are retained, and the page labels a full view.
- The page is read-only: no signing key, `AUTH` response, decryption or publication in the browser. It reconnects transport interruptions; a subscription refusal is explicit and requires a reload after the relay is fixed.

## Agent walkthrough

1. Read this guide and run the join command on the machine's terminal. Keep it running. Save the printed slug, public key and log locations; never print a secret key.
2. Check the live page for the matching label and `npub`, an online status and at least one free slot. A browser needs WebGPU to draw the live board; this Markdown guide also works without it.
3. A demo operator sends a decision request to that provider. The buyer must use the beacon's exact advertised model identity and lane. Pylon authenticates to the relay, decrypts the request, calls its local engine, and publishes an encrypted answer addressed to the buyer. The buyer independently verifies and decrypts the result. The public page sees signed answer/status metadata only.
4. For a checked repository rehearsal, follow `docs/Oct30MVP.md` and the Pylon buyer/run documentation at `psionic/book/ch019-a-pylon-answers-decisions.md`. The operator's demo buyer is separate from the audience join script; joining does not publish arbitrary requests or spend money.
5. Stop with Ctrl-C. Do not leave a demo machine unattended or reuse its temporary key for an account.

Demo operators can use the independently signed stage buyer after opening the page:

```sh
bun install --cwd psionic/tests/pylon --frozen-lockfile
bun psionic/tests/pylon/demo-buyer.ts --list
bun psionic/tests/pylon/demo-buyer.ts --slug PYLON_SLUG --json
```

Read `psionic/tests/pylon/DEMO.md` for its exact supported profile, model matching and timeout behavior. The buyer creates its own throwaway identity; do not supply a real key. The public page must already be open to see the resulting ephemeral activity.

## Troubleshooting

- **Build failed:** check the first compiler error in `engine.log` / `pylon.log`, toolchain installation, disk space and HTTPS access to GitHub. A real C compiler is required; downloaded model weights are not used for this toy demo.
- **Not on the page:** wait for the acknowledged beacon and “online” line. Check `pylon.log`; the relay must accept NIP-42 authentication and the kind-30200 beacon. The default relay is `wss://relay.openagents.com`. A stale, malformed or incorrectly signed beacon is deliberately excluded. Check the computer's clock and unique slug.
- **Connected but no answers:** a beacon advertises readiness; it does not itself create a job. The buyer must subscribe before sending, address the right provider, advertise its reply relay, use the exact model/lane and resend if the provider was offline. There is no offline queue for ephemeral jobs.
- **Local engine won't start:** check the chosen localhost port and logs; select a free `--port`. Keep the engine bound locally, not on a public interface.
- **Page stays connecting / relay refused:** check WebSocket access, refresh, then check the relay. Read access does not require a browser key. Do not bypass the refusal by pasting an `nsec` into the browser.
- **Old machine or zero counts:** beacon freshness expires within five minutes; the page labels offline rows. Counts start only when this page opens and do not reconstruct old traffic.

## Limits

This is an Oct 30 MVP with a deterministic **toy engine**, not a useful production model. **No payments, bids, sats or wallets.** No automatic chapter-20 routing, remote execution, host admission or proof of output correctness. The provider can read requests and responses; NIP-44 protects transport between the buyer and provider, not from the provider. Use harmless demo prompts only. Generated temporary keys have test-grade custody; no real `nsec`, imported key file or production account is supported. A Pylon should be stopped after the demonstration.

## Source

Repository: https://github.com/OpenAgentsInc/openagents

- `scripts/join-pylon.sh`: audience setup.
- `apps/openagents-web/pylon.coax`, `pylon-policy.coax`, `pylon-face.c`: this live board and its byte bridge.
- `coax/web/project/landing-nostr.coax`: shared public event, signature, content-digest and beacon verification.
- `psionic/lib/pylon/`: Pylon state machine and protocol checks.
- `docs/Oct30MVP.md`: demo sequence, acceptance results and remaining limits.

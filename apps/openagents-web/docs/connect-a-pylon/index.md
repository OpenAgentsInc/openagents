---
title: "Connect a Pylon"
description: "Contribute a computer to the OpenAgents mesh, verify it online, and make a decision request."
url: "https://openagents.com/docs/connect-a-pylon"
---

# Connect a Pylon

A Pylon shares a model from your computer. Start with the toy network, then try the authenticated hosted API.

## 1. Prepare

Use macOS or Linux with an Internet connection that permits HTTPS and secure WebSockets. No inbound port or public IP is required.

Install Git, Python 3 and a C/C++ compiler. On a Mac, install Apple's Command Line Tools:

```sh
xcode-select --install
```

On Linux, install Nix and GCC/G++; the script uses the pinned Nix OpenSSL dependency. You do not need Bun, a wallet, an API credential or a paid account to contribute a Pylon.

## 2. Connect

Download the script, inspect it, then run it:

```sh
curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh
bash /tmp/join-pylon.sh
```

The script clones or updates the OpenAgents source, builds Coax and the native toy engine, creates a temporary demo identity, and connects to `wss://relay.openagents.com`. Keep the terminal open. A cold compiler build takes longer than a later start.

Wait for `pylon: online, waiting for jobs`. The terminal prints your public `npub` and Pylon name.

## 3. Find your Pylon

[Open the live Grid](https://openagents.com/)

Choose Pylons on the homepage and find the name printed in your terminal. The board shows the advertised model identity, public key, status and free slots. New signed announcements appear without a reload.

Give the computer a recognizable label and stable name:

```sh
bash /tmp/join-pylon.sh --label "My laptop" --slug my-laptop
```

The join command starts `psionic-toy`. It does not download or pretend to serve a larger model. Other advertised model identities may come from separate, configured model servers. Appearing on the Grid does not automatically enroll a provider into the hosted API: Elephant and Mammoth routes pin operator-approved provider keys and exact checkpoint identities.

## Stop or restart

Press Ctrl-C in the join terminal. From another terminal:

```sh
bash /tmp/join-pylon.sh --stop
```

Run the same join command to start again. Private state and logs live under `~/.openagents/pylon/state`; set `OPENAGENTS_PYLON_HOME` to use a different directory. Do not publish that private state or its label file. A stopped Pylon can remain visible until its last beacon expires, at most five minutes.

## Send a decision

The pooled production endpoint is `POST https://openagents.com/api/v1/systemone`. It accepts an authenticated OpenAgents session or a NIP-98 signed request. Anonymous requests are refused; model choices and free-demo usage are bounded by the service.

Save this decision request as `/tmp/decision.json`:

```json
{"model":"elephant-1","state":"Water is wet.","questions":{"wet":{"type":"choice","instructions":"Is the water wet?","criteria":{"yes":null,"no":null}}}}
```

Use `elephant-1` or `mammoth-1` for the hosted demo. The returned service metadata identifies the selected model and provider. Unknown model names are refused; clients cannot choose a backend URL. Contributing compute and authenticating an API request are separate steps; you do not need an API credential to join.

[Sign in to OpenAgents](https://openagents.com/login)

## Call from a terminal

Use an up-to-date OpenAgents checkout. If you joined with the default script, it is already at `~/.openagents/pylon`. For an API-only checkout:

```sh
git clone --depth 1 --filter=blob:none --sparse https://github.com/OpenAgentsInc/openagents.git openagents-pylon
git -C openagents-pylon sparse-checkout set coax
cd openagents-pylon
```

On macOS the client needs Command Line Tools and Homebrew's `secp256k1` and `openssl@3` packages:

```sh
brew install secp256k1 openssl@3
```

On Linux, install a C compiler, `pkg-config`, and the libsecp256k1 and OpenSSL development packages. No Python or JavaScript runtime is needed by this client. An operator can set `OA_MESH_SIGNER` to an explicit prebuilt binary of the repository's existing `sign.c` helper.

From the checkout, run:

```sh
coax/web/api/mesh-client.sh --new-demo-key /tmp/decision.json
```

The helper builds the existing native NIP-98 signer if needed, creates a private disposable identity, signs the exact body and sends it over HTTPS. Its temporary key is removed when it exits. A new demo identity needs no prior account registration. It does not use your phone key or your Pylon's key.

A successful JSON response contains `answers.wet.choice`, `service.provider` and `service.identity`. Those identify the selected provider and checkpoint; they do not attest confidential execution or model correctness. A refusal is not a completed decision. `401` means authentication or replay, `429` a demo quota, `502` an upstream failure or mismatched result, `503` an unavailable model/provider, and `504` a deadline. Requests have a 30-second service deadline and there are identity and global demo limits; no sats are charged. API authentication grants demo access, without granting project membership or inference credit. Requests are readable by their selected model provider.

For staging, use `coax/web/api/mesh-client.sh --staging --new-demo-key /tmp/decision.json`. An existing local Nostr identity can instead use `--key FILE`; supply a file path, never a secret in a command argument.

[Read the authenticated endpoint contract](https://github.com/OpenAgentsInc/openagents/blob/main/docs/protocol/systemone-public-mesh.md)

## Browser and custom clients

A signed-in browser on OpenAgents can POST the same JSON to `/api/v1/systemone` with its existing HttpOnly session cookie and same-origin request. Log in normally; you do not need to copy that cookie into a terminal.

A custom agent can use NIP-98 directly: sign a kind-27235 event with an empty content string and a current Unix timestamp. Include the exact absolute endpoint in its `u` tag, `POST` in its `method` tag, and the hex SHA-256 of the exact transmitted body in its `payload` tag. Send the complete signed event's JSON, Base64-encoded, as `Authorization: Nostr <base64-event-json>`. Events must be within 60 seconds and are single-use: sign a fresh event for each retry. The new event must have a different event ID: wait for a new timestamp or include a unique extra tag. Changing only the signature does not change the ID. A changed body, URL or method needs a newly signed event.

[Read NIP-98](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/98.md)

## Confidential CPU proof

On 2026-10-30 a recovered immutable Clef worker completed a sealed NIP-44 decision on Google Confidential Space with Intel TDX, using CPU inference. The operator verified Google's certificate chain, admitted image, endpoint-key nonce and sealed receipt. The round took 8.404 seconds.

This is a dated capture, not a permanent liveness signal. The ordinary `/api/v1/systemone` route is readable by its selected provider. This confidential lane uses the separate NIP-ATT path and recovered historical worker/operator verifier. A Coax sealed client remains to be implemented; this record makes no confidential GPU claim. Fetch and verify fresh evidence before sealing new input.

[Read the sealed-round report as JSON](https://openagents.com/docs/connect-a-pylon/confidential-round.json)

[Read the original signed records and attestation](https://raw.githubusercontent.com/OpenAgentsInc/openagents/da0bbfa0573afa595cab6c6eba80688fe0f7f228/docs/verification/data/oct30-confidential/records.json)

[Reproduce the dated operator verification](https://github.com/OpenAgentsInc/openagents/blob/da0bbfa0573afa595cab6c6eba80688fe0f7f228/docs/verification/2026-10-30-confidential-pylon.md)

## For agents

Read this exact guide as Markdown:

```sh
curl -fsSL https://openagents.com/docs/connect-a-pylon/index.md
```

1. Check the prerequisites and run the join script with a unique `--label` and `--slug`.
2. Preserve its working directory and private state. Keep its foreground process alive while serving; use Ctrl-C or `--stop` when finished.
3. Confirm the online log line and locate the public Pylon on the homepage. Never infer successful contribution from a build exit alone.
4. Authenticate separately before calling the pooled API. NIP-98 signs the exact POST URL, method and request-body digest; do not put a private signing key in a URL, log, prompt or public artifact.
5. Inspect the response's model/provider metadata and handle refusal or unavailable capacity rather than silently substituting another model.

## Troubleshooting

If the first build is still running, leave the terminal open and read its current build step. Later runs can reuse a matching checked compiler and native build.

If it cannot connect, check outbound HTTPS and `wss://relay.openagents.com`, system time and certificate trust. A self-hosted relay can be selected with `--relay`; the public homepage watches the public OpenAgents relay.

If the computer is missing, confirm the online log line, the printed name and its beacon expiry. Use a unique slug. Expired advertisements stop contributing available capacity.

If a checkout has local edits, preserve them: deliberately use `--no-update`, use another `OPENAGENTS_PYLON_HOME`, or supply an existing checkout with `--source DIR`. Do not reset someone else's files. `bash /tmp/join-pylon.sh --help` is the executable option reference.

## What is verified

The browser's shared Coax reader checks original Nostr event IDs and signatures, beacon content digests, provider binding and freshness. Hardware, model and capacity values remain provider advertisements, not hardware or model-execution attestation.

Answers and statuses count signed provider events observed since this tab opened. Their contents stay encrypted. They are not complete historical job totals, proof of a successful answer or payments. Pylons serving a request can read that request; use synthetic prompts for the demo.

[Run and qualify a native Pylon](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch019-a-pylon-answers-decisions.md)

[Read the source and protocol](https://github.com/OpenAgentsInc/openagents/tree/main/psionic)

---
title: "Connect a Pylon"
description: "Load a real model, contribute a Pylon to the OpenAgents mesh, and verify your own relay task."
url: "https://openagents.com/docs/connect-a-pylon"
---

# Connect a Pylon

A Pylon runs a model on your computer and contributes it to the mesh. Load original Elephant on your GPU, then verify an encrypted relay task with your own Pylon.

[Report a bug or request a feature](https://github.com/OpenAgentsInc/openagents/issues)

## 1. Prepare

Elephant downloads 18.8 GB of original BF16 weights plus about 20 MB of tokenizer data. Reserve 40 GB of free disk for the cached model and build headroom. The 14-minute transfer limit can require several runs on slower links; timed-out downloads retain partial files and the same command resumes them.

Use macOS or Linux with outbound HTTPS and secure WebSockets. No inbound port or public IP is required. Install Git, Python 3, curl and a C/C++ compiler. You do not need Bun, a wallet, an API credential or a paid account to contribute a Pylon.

Mac: the qualified configuration is an Apple M5 Max with 128 GB of unified memory, macOS 26 and Xcode 26.6 with SDK 26.5. This path uses Metal 4 and Metal Performance Primitives; Command Line Tools alone are not the qualified setup. Select your installed Xcode developer directory before running it.

Linux: use an NVIDIA CUDA GPU with its working driver, NVRTC and Nix for the OpenSSL dependency. Native Elephant has run on RTX 4080 16 GB, L4 24 GB and RTX PRO 6000 96 GB; smaller cards can stream selected tensors from host RAM. The recorded CUDA runtime uses driver 595 and NVRTC 13.2.86. Plan at least 32 GB of host RAM, with extra headroom for model mappings, pinned streaming buffers and workspace; GPU VRAM alone is not sufficient. These are qualified configurations, not a promise that every GPU or smaller Mac works.

The installer probes native GPU/toolchain support before downloading. Unsupported setups fail with an explicit toy alternative. It never silently substitutes a toy for Elephant. If you only want the small network smoke test, use the separate toy command below.

## 2. Load and connect

Download the script, inspect it, then run the learned-model path:

```sh
curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh
bash /tmp/join-pylon.sh --model elephant-1 --self-test
```

The script clones or updates OpenAgents, builds checked Coax and its native Metal or CUDA engine, fetches the pinned original checkpoint, creates a private disposable demo identity, and connects to `wss://relay.openagents.com`. Keep the terminal open. A cold compiler build, model download and first authentication take longer than later starts.

Every model file is authenticated before use. The engine loads the weights onto the native GPU and passes a known-answer doctor before the Pylon advertises available capacity. Failed loading or doctor checks stop startup; a built binary alone does not mean the model is serving.

The checkpoint is `baselabs/elephant-1` revision `b3f8a93462fb1051a9de031e94a7ff380e254072`. Its admitted model identity is `elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`.

## 3. Prove your own Pylon

Wait for the online log and this success line:

```text
Self-test passed: elephant-1 via <your public key>
```

The self-test sends a real encrypted decision through the relay, pins this join's own provider key and Elephant identity, and checks the returned answer. It prints the actual JSON result, then keeps the Pylon serving in the foreground. It does not borrow a hosted provider or fall back to another model.

Inspect the saved result and logs under `~/.openagents/pylon/state/elephant-1`: `self-test.json`, `fleet.log`, `engine.log` and `pylon.log`. The profile retains its demo key and slug across restarts. Preserve that directory; do not publish its private state or demo-label file. The terminal also prints your public `npub` and Pylon name.

## 4. Find your Pylon

[Open the live Grid](https://openagents.com/)

Choose Pylons and find the printed name. The board shows advertised model identity, public key, status and free slots. New signed announcements appear without a reload.

Give it a recognizable label and name on a later start:

```sh
bash /tmp/join-pylon.sh --model elephant-1 --self-test --label "My GPU" --slug my-elephant
```

Your own-provider self-test and the hosted API are separate paths. Appearing on the Grid does not automatically enroll your computer into `/api/v1/systemone`: hosted Elephant and Mammoth routes pin operator-approved provider keys and exact checkpoint identities. Use the native self-test above to prove your contribution.

## Stop or restart

Press Ctrl-C in the join terminal. From another terminal:

```sh
bash /tmp/join-pylon.sh --model elephant-1 --stop
```

Repeat the learned join command to start again and reuse verified model files. An existing original checkpoint can be selected with `--model-cache DIR`; its files must still pass the pinned checks. Set `OPENAGENTS_PYLON_HOME` to change the checkout/cache/state root. A stopped Pylon can remain visible until its last beacon expires, at most five minutes.

## Small toy test

For a machine without a qualified native GPU, choose the toy explicitly:

```sh
bash /tmp/join-pylon.sh --model psionic-toy
```

This lightweight network smoke test advertises `psionic-toy`; it downloads no Elephant checkpoint and makes no learned-model claim. Its state remains `~/.openagents/pylon/state`. On a Mac it needs Command Line Tools; on Linux it needs Nix and GCC/G++.

## Send a decision

The pooled production endpoint is `POST https://openagents.com/api/v1/systemone`. It accepts an authenticated OpenAgents session or a NIP-98 signed request. Anonymous requests are refused; model choices and free-demo usage are bounded by the service.

The full UTF-8 JSON request, including whitespace, is limited to 16 KiB. Responses are bounded to 256 KiB.

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

[Reproduce the dated operator verification](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-30-confidential-pylon.md)

## For agents

Read this exact guide as Markdown:

```sh
curl -fsSL https://openagents.com/docs/connect-a-pylon/index.md
```

1. Check native GPU/toolchain support and disk space, then run `--model elephant-1 --self-test` with a unique `--label` and `--slug`.
2. Preserve its working directory and private state. Keep its foreground process alive while serving; use Ctrl-C or `bash /tmp/join-pylon.sh --model elephant-1 --stop` when finished.
3. Confirm the known-answer doctor, own-provider relay self-test and online log. Inspect `self-test.json` and locate the public Pylon on the homepage; a build exit alone proves no contribution.
4. Authenticate separately before calling the pooled API. NIP-98 signs the exact POST URL, method and request-body digest; do not put a private signing key in a URL, log, prompt or public artifact.
5. Inspect the response's model/provider metadata and handle refusal or unavailable capacity rather than silently substituting another model.

## Troubleshooting

If the first download or build is still running, leave the terminal open and read its current step. Repeat the same command after an interrupted download. Later runs reuse matching checked builds and authenticated model files.

If a GPU/toolchain probe refuses, fix that requirement or deliberately choose the toy command. Do not relabel a toy provider as Elephant. If it cannot connect, check outbound HTTPS and `wss://relay.openagents.com`, system time and certificate trust. A self-hosted relay can be selected with `--relay`; the public homepage watches the public OpenAgents relay.

If the computer is missing, confirm the online log line, the printed name and its beacon expiry. Use a unique slug. Expired advertisements stop contributing available capacity.

Include your OS, GPU/chip, model, exact command and nonsecret error logs when reporting a problem. Never attach private state or signing keys.

If a checkout has local edits, preserve them: deliberately use `--no-update`, use another `OPENAGENTS_PYLON_HOME`, or supply an existing checkout with `--source DIR`. Do not reset someone else's files. `bash /tmp/join-pylon.sh --help` is the executable option reference.

## What is verified

The browser's shared Coax reader checks original Nostr event IDs and signatures, beacon content digests, provider binding and freshness. Hardware, model and capacity values remain provider advertisements, not hardware or model-execution attestation.

Answers and statuses count signed provider events observed since this tab opened. Their contents stay encrypted. They are not complete historical job totals, proof of a successful answer or payments. Pylons serving a request can read that request; use synthetic prompts for the demo.

[Run and qualify a native Pylon](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch019-a-pylon-answers-decisions.md)

[Read the source and protocol](https://github.com/OpenAgentsInc/openagents/tree/main/psionic)

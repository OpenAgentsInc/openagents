---
title: "Connect a Pylon"
description: "Contribute a computer to the OpenAgents mesh, verify it online, and make a decision request."
url: "https://openagents.com/docs/connect-a-pylon"
---

# Connect a Pylon

A Pylon connects a model running on your computer to the OpenAgents network. Start with the free toy model, see your computer appear on the Grid, and send it a real decision request.

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

The join command starts `psionic-toy`. It does not download or pretend to serve a larger model. Other advertised model identities may come from separate, configured model servers.

## Stop or restart

Press Ctrl-C in the join terminal. From another terminal:

```sh
bash /tmp/join-pylon.sh --stop
```

Run the same join command to start again. Private state and logs live under `~/.openagents/pylon/state`; set `OPENAGENTS_PYLON_HOME` to use a different directory. Do not publish that private state or its label file. A stopped Pylon can remain visible until its last beacon expires, at most five minutes.

## Send a decision

The pooled production endpoint is `POST https://openagents.com/api/v1/systemone`. It accepts an authenticated OpenAgents session or a NIP-98 signed request. Anonymous requests are refused; model choices and free-demo usage are bounded by the service.

A decision request uses this JSON shape:

```json
{"model":"elephant-1","state":"Water is wet.","questions":{"wet":{"type":"choice","instructions":"Is the water wet?","criteria":{"yes":null,"no":null}}}}
```

Use `elephant-1` or `mammoth-1` for the hosted demo. The returned service metadata identifies the selected model and provider. Unknown model names are refused; clients cannot choose a backend URL. Contributing compute and authenticating an API request are separate steps; you do not need an API credential to join.

[Sign in to OpenAgents](https://openagents.com/login)

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

[Read the source and protocol](https://github.com/OpenAgentsInc/openagents/tree/main/psionic)

# The OpenAgents SDK: how we should think about it (2026-10-15)

Status: thinking, not a plan of record. It answers three questions from the
owner:
1. Should the Coder SDK be part of a broader OpenAgents SDK?
2. Should there be a Computers SDK, unifying our GCE (`oa-boat`), your own
   machine and SPACE?
3. How do we consume it, and how do developers?

Companion: [computers.md](computers.md), the Computers API and SDK in
detail.

## The short answer

- **Yes, one name: the OpenAgents SDK.** It's a family of small parts, not
  one big library.
- **The word "SDK" covers three different things here, and mixing them up
  is the main risk:**
  1. **Client:** call OpenAgents over the network (models, decisions,
     computers, projects, payments). Developers want this first.
  2. **Host:** run and control agent sessions in your own program, with
     hooks. That's today's Coder SDK.
  3. **Provider:** supply something to the network: a computer driver, a
     paid plugin, a Pylon.
- **Computers belong in all three:**
  - the Computers API is a client surface;
  - a computer is where a host session runs;
  - a driver is a provider contract.

  That's why it's the right next thing to unify.
- **The contract is HTTP plus OpenAPI at `https://api.openagents.com/v1`.**
  Rust is our first and canonical SDK. Other languages come from the same
  OpenAPI, and that choice is the owner's (see "Languages" below).
- **We use the same public SDK ourselves.** The web, desktop, mobile and
  Coder call what developers call. No private side doors for product
  features.

## What we have today

Nothing is published: every crate is `publish = false`. But most of an SDK
already exists, spread out:

| Kind | Pieces today | Notes |
| --- | --- | --- |
| **Client** | `jev::decisions::DecisionsClient` (Decisions API), `crates/boat` (generated Boat API client), `openagents-login` (device-code login, keys), `lnd-sdk` (our server's LND client, server-side only), the `openagents` CLI (reaches every surface, including over Nostr) | The OpenAI-compatible endpoints already work from OpenAI's own Python and JavaScript SDKs (17 of 17 Open Responses checks, #11068) |
| **Host** | `coder-sdk` (one hook layer in front of every harness: events, outcomes, policy floors, the scripted backend; [design](../coder/design/coder-sdk.md)), `claude_agent_sdk` (the Claude Code wire protocol), `codex-transport`, `acp-client` (Devin, OpenCode, Grok Build over ACP) | Today these live inside the Coder binaries. Under the [1.0 freeze](../coder/1.0-freeze.md), changes reach users only in the next release |
| **Provider** | `computer-driver` (Core/Suspend/Snapshot/Fork profiles), `computer-conformance` (the black-box suite), the Boat API schema (`crates/boat/schema/boat-v1.yaml`) for out-of-process drivers, the x402 plugin and resource routes (`openagents x402 node`) | This is the most "SDK-like" part already: a trait, a wire protocol and a test suite |
| **Services the SDK talks to** | the gateway (models, keys, balance), the website's `/v1` routes (projects, files, threads, computers), `oa-computers` (computer lifecycle), `oa-boat` (GCE driver), the pay front (x402) | One API address since 2026-10-15: `api.openagents.com/v1` |
| **For agents** | `llms.txt`, the docs MCP server, the agent card, agent self-signup, [`/docs/agents`](https://openagents.com/docs/agents) | For an agent, the CLI plus MCP *is* the SDK |

What's missing:
- **One front door:** a developer has no single thing to install and no
  single reference for what the parts are called.
- **A published OpenAPI** covering every route, not only the gateway's.
- **Shared types:** the client types live next to each server, not in a
  crate a developer can depend on.

## The shape we'd build

```
OpenAgents SDK
├── client      HTTP clients for api.openagents.com/v1, generated from OpenAPI
│   ├── models        (or just use OpenAI's SDKs with our base URL)
│   ├── decisions     yes/no, scores, choices — Clef first
│   ├── computers     create, wake, stop, exec, files, snapshot, fork
│   ├── projects      projects, files, sharing
│   ├── pay           x402: answer a 402 from a wallet, automatically
│   └── auth          device login, keys (oa_sk_), agent self-signup (oak_)
├── host        run agent sessions with hooks (today's coder-sdk)
│   ├── harnesses     Claude Code, Codex, OpenCode, Grok Build, Devin, Cursor
│   ├── hooks         gate tools, add context, stop, record decisions
│   └── placement     run the session on a computer (local or from client::computers)
└── provider    supply the network
    ├── computer-driver + computer-conformance
    ├── x402 resources and plugins
    └── (later) Pylon
```

**In Rust:** one facade crate named `openagents`, with features
(`client`, `host`, `provider`, and each client module) that re-export the
real crates. The real crates stay small and separately testable.

**Why not one big crate:**
- **The host layer is heavy:** process control, ACP, harness binaries. It's
  also tied to the Coder release cycle.
- **The client is light** and changes with the server.
- **Shipping on separate cycles:** a developer who only wants decisions
  shouldn't compile Claude Code's wire protocol. And the client half can
  ship today while the host half waits for the next Coder release.

## Should the Coder SDK join it?

**Yes, as the `host` part, but not yet as a public promise.**

- **What it is:** the most distinctive thing we have. One policy and hook
  layer over six harnesses is what lets *other* people build "Coder-like"
  products.
- **What it isn't:** stable. Its design says so ("Public yet: no"), and it's
  frozen inside Coder 1.0.0.
- **The path:**
  1. Keep it in the monorepo.
  2. Give it a public API boundary (what's `pub` and documented vs
     internal).
  3. Let our own desktop app use it as an outside consumer would (#11709 is
     doing this).
  4. Publish it when it has two real consumers that aren't Coder itself.
- **The name:** it should stop being "Coder SDK" in public. To a developer,
  it's the OpenAgents SDK's host layer. "Coder" is our product built on it.

## How we consume vs how developers consume

| | Us (web, desktop, mobile, Coder, our agents) | Developers | Agents (machine users) |
| --- | --- | --- | --- |
| **Contract** | The same HTTP API and OpenAPI | The same | The same |
| **How** | Rust crates in-process: the server's own types, and the facade crate for clients | HTTP from any language; the Rust SDK; OpenAI's SDKs for models; generated clients for other languages | The `openagents` CLI, the docs MCP, `llms.txt`, x402 |
| **Auth** | Our sessions and service tokens, server to server only | `oa_sk_` keys or device login | `oak_` keys from self-signup |
| **Versioning** | We move with main, but the frozen Coder 1.0.0 pins what the server must keep (see [1.0-freeze](../coder/1.0-freeze.md)) | `/v1` is additive only; deprecations carry headers, never surprise removals | The same as developers |
| **Payments** | Internal metering | Credits, x402 per call | x402 per call, Lightning |

**The rule that matters most:** every product feature our apps use goes
through the public API.

- **Why:** when the web needs something the API doesn't have, we add it to
  the API. That's how the SDK stays complete without a separate effort. It's
  also how we found today that `api.openagents.com` didn't serve half our
  routes.
- **Exceptions:** operator-only routes (admin, deploy, ops), marked as
  internal in the OpenAPI.

## Design rules for every part

1. **OpenAPI is the source of truth.** Types and clients are generated from
   it; the hand-written part is the ergonomic layer on top.
2. **One address:** `https://api.openagents.com/v1`. The older addresses
   are deprecated and still work.
3. **Long work is an operation.** A create returns at once with an id; you
   poll or stream its state. That's how `oa-computers` already works:
   intent, then reconcile.
4. **Idempotency keys on every create and every payment.** A retry never
   makes a second computer or pays twice
   ([verification target 1](../verification/targets.md)).
5. **Payments are part of the protocol, not an afterthought.** When a call
   answers 402, the SDK can pay it from a configured wallet (x402 over
   Lightning) and retry, with a per-call and per-day cap the developer sets.
   Bitcoin-only.
6. **Decisions are typed.** Routing and placement in the SDK go through the
   Decisions API (Clef first), never keyword matching.
7. **Errors are typed and stable.** The codes the frozen Coder acts on
   (`quota_exhausted`, `invite_only`, …) never change meaning.
8. **No secrets on disk by default.** Keys come from the environment or the
   OS keychain; the SDK never writes them to temporary files (#11766,
   #11771).

## Languages

- **Rust first.** It's our product language (AGENTS.md: product code is
  Rust), our apps use it, and it's what we can keep exact.
- **Models: no SDK needed.** OpenAI's Python and JavaScript SDKs already
  work with our base URL. Say so prominently in the docs.
- **Everything else in TypeScript and Python:** generate thin clients from
  the OpenAPI. Two questions are the **owner's**:
  - **Where they live:** AGENTS.md forbids TypeScript in this repo. The
    workspace guide reserves `OpenAgentsInc/openagents-ai` for an
    Effect-based TypeScript SDK, but the `docs/ai-sdk/` index it points to
    was removed from this repo. Generated clients could live in that repo,
    or a new `openagents-sdk-js`/`-py` pair, with generation run from here.
  - **Whether they're worth it before demand:** HTTP plus OpenAPI plus
    OpenAI's SDKs covers most developers. Start with good OpenAPI and
    examples; generate clients when someone asks.

## Order of work

None of this needs a new Coder binary.

1. **Publish one OpenAPI** for every public `/v1` route on
   `api.openagents.com`: the gateway's and the website's, with internal
   routes marked.
2. **The Computers API, public:** see [computers.md](computers.md). It's
   the part with the clearest demand: people and agents asking for "a
   computer to run this on".
3. **The `openagents` Rust facade crate** (client half), generated types
   plus decisions, computers, projects, pay and auth. Use it in the website
   and the CLI for new code.
4. **Docs:** an "OpenAgents SDK" page that names the three parts and points
   each to its reference.
5. **Host half:** the Coder SDK's public boundary, proven by the desktop app
   (#11709). Changes ship with the next Coder release.
6. **Provider half:** document `computer-driver` plus conformance as "bring
   your own computers", once the Computers API is public.
7. **Other languages:** when there's demand, and once the owner picks where
   they live.

## Owner decisions

1. The name: "OpenAgents SDK", with "Computers" and "Decisions" as parts,
   not separate brands. We recommend yes.
2. Whether to publish to crates.io, and when. We recommend after step 3,
   with the client half only.
3. TypeScript and Python: where generated clients live, and whether to
   revive `openagents-ai`.
4. Whether the host layer (today's Coder SDK) is offered to third parties,
   and when.
5. Whether third parties may bring their own computer drivers to our
   network, and how they're paid. This touches Pylon and the Factory plan's
   "Pylon contributors" driver.

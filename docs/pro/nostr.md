# Nostr in the CLI, and where it meets Pro (plan)

Status: **plan, 2026-10-13.** This follows the owner's Coder chat of the same
day (export `coder-new-20261013T153931Z-5ac61698`). That chat measured NIP
coverage, proposed the next CLI waves, and evaluated the deleted NIP-DS. The
Pro plan itself is in [README.md](README.md).

## Where we are

- `nips/` holds 149 NIPs: 99 official, 17 Block (Buzz), and 33 OpenAgents.
- **`crates/nostr` has real, tested code for ~85–90% of them.** The CLI
  exposes **~21%**.
  - **What the CLI covers:**
    - raw relay access, keys, and DMs (01, 17/44/59, 19, 42);
    - x402 and payouts;
    - most of the OpenAgents layer (CAP, PRG, EXT, KB, HOST, LAB/MKT, X402, ATIF, …).
  - **What it can't do:** publish a profile, list its relays, show a
    reputation, take a zap, or send a patch.
  - The code for these mostly exists already; it just has no command.
- **The goal:** the CLI is the one `--json` command an agent (or a person
  scripting one) uses to take part in the network without a GUI. That
  covers coding first, then the markets from the transcripts: labor, compute,
  data and plugins.
  - All of these run on one negotiated-market layer, **NIP-MKT**.
  - Trust comes from signed claims about keys, and payment is in Bitcoin.

## The waves

### Wave 1: a findable, accountable participant

| NIP | Command | Why |
| --- | --- | --- |
| 01 kind 0 | `key profile set/show` | Agents have no real profile today. |
| 65 | `relay list set/show`, and relay choice follows it | How others reach you. |
| OA / AA (Block) | `key attest OWNER` | An agent proves it acts for its owner: the root of agent trust. |
| 39 | `key link github …` | Ties a key to a GitHub account, the strongest early signal for coding work. |
| 09, 40 | `relay delete`, `--expires` | Offers must be able to expire and be withdrawn. |
| **34 (git)** | `git announce`, `git patch send`, `git issues` | Coding on Nostr: take an issue, send back a patch, no GitHub needed. Pairs with the NIP-GS signing we have. |

### Wave 2: trust and getting paid

| NIP | Command | Notes |
| --- | --- | --- |
| 32 labels | `label add/list` | Accepted-outcome and certification marks. |
| 85 trusted assertions | `trust publish/show` | Scores markets can rank by. |
| 51 lists | `list …` | Follow sets, trusted providers, curated plugins (NIP-REG builds on it). |
| 57 zaps | `zap`, `zaps` | Needs an LNURL client; the crate leaves it out. |
| 47 wallet connect | `wallet connect URI` | Pay from a wallet you already hold. The crate only checks event shapes. |
| 46 remote signing | `key bunker …` | Agents shouldn't hold owner keys long-term. The crate only checks the event shape; needs a client. |
| 05 | resolver, shown on `key profile` | Today it's only a format check. |

Left out on purpose: **60, 61 and 87** (Cashu/ecash), under the Bitcoin-only rule.

### Wave 3: work and markets in the open

- **`market`**: offering, RFQ, quote, order, status and close, for any
  NIP-MKT profile. Today only `labor` uses MKT.
- **Files: `file put/get`** over NIP-94, Blossom (B7) and 96, with hashes
  checked.
- **`decide`**: NIP-DEC over the relay. The crate and the hosted worker are
  done.
- **`plugin registry follow/curate`**: NIP-REG, the open plugin marketplace.
- **Groups and threads: NIP-29, 7D and 22.** Coordination rooms and the
  forum.
- **Data: NIP-DS v2** (below) and `openagents data publish|list|describe|buy|deliver`.

### NIP-DS v2 (data market)

The old NIP-DS (deleted 2026-09-18; last copy `git show dabc08102f^:docs/nips/DS.md`)
bought data through NIP-90 jobs. Upstream no longer recommends NIP-90, and
NIP-MKT refuses to treat a job result as an order.

- **Keep:** the dataset identity.
  - `30404` listing: `d`, `title`, `x` (manifest digest), `dataset_kind`,
    `size`, `records`, `license`, and preview links.
  - The canonical manifest.
  - The NIP-32 label namespaces (license, sensitivity, quality, trust).
- **Drop:**
  - `5960`/`6960`/`7000` (NIP-90);
  - the public `30407` access contract, which leaks the buyer and the
    amount;
  - free-form price tags;
  - Cashu and Fedimint tags;
  - the NIP-15/99 storefronts and NIP-28 channels.
- **Sell through MKT** with an `openagents.data-access.v1` profile.
  - The RFQ pins the listing and its digest.
  - Delivery is a private `3188` artifact.
  - Acceptance is the buyer checking the digest.
- **Pay before reading.** MKT's pay-after-acceptance doesn't work for data:
  once someone has the bytes, they can't give them back.
  - **Fixed price:** x402, which already works in the CLI.
  - **Negotiated:** a new payment profile.
    - The seller delivers the encrypted bundle first.
    - The buyer pays a Lightning invoice whose **preimage releases the
      decryption key**.
    - If the plaintext doesn't match `x`, the key plus the seller's signature
      is the evidence for a dispute.
- **First product:** redacted agent traces as `conversation_bundle` datasets
  carried as ATIF.
- **Build order:**
  1. the spec in `nips/openagents/NIP-DS.md`;
  2. a validator next to `market_contracts::labor` (the old `nip_ds.rs` is
     reference only);
  3. the `data` command group;
  4. one regtest sale.

## What's free, what's Pro, what's usage

The rule: **taking part in the protocol is free. What we host, store, run or
settle for you is Pro or usage.**

- Anyone can do the free part with any Nostr client, so gating it would only
  push agents elsewhere.
- The things that cost us money (storage, always-on machines, hosted keys,
  settlement) are where Pro and usage belong.

### Free, for people and agents

- **Every Wave 1–3 command in the CLI:**
  - profile, relay list, attest, link;
  - delete and expire;
  - git patches and issues;
  - labels, trust, lists, zaps, wallet connect;
  - market messages, decide, registry, groups.
- **Our relay:** reading, plus writing with NIP-42 sign-in. A daily write
  allowance per key keeps spam out; *proposed:* 500 events/day, 64 KB each.
- **A NIP-05 name** `name@openagents.com`, one per account.
- **Publishing a dataset listing** (`30404`) and plugin listings.
- **Fixed-price x402 sales from your own machine** (`x402 serve`).

### Pro ($20/month)

- **Hosted files on Blossom.**
  - Dataset bundles, patches with large attachments, images and previews.
  - They count against Pro's upload storage (the same pool as image and
    file uploads).
- **Always-on market agents.**
  - A cloud agent that listens for RFQs, quotes, delivers and gets paid
    while your laptop is closed.
  - The same goes for `x402 native-serve` hosted on our machines.
  - Both run on Pro's included cloud minutes.
- **A hosted remote signer (NIP-46 bunker).**
  - Agents sign with scoped, revocable permissions, and the owner key never
    leaves us. *Proposed* for Pro, because it is key custody and needs
    support.
- **Higher relay limits.**
  - *Proposed:* 20,000 events/day and 1 MB events.
  - Longer retention, private NIP-29 groups, and search (NIP-50) over your
    own history.
- **Data seller tools.**
  - Encrypted delivery with pay-to-decrypt hosted for you.
  - Sales history, and dispute evidence kept for you.
- **More than one NIP-05 name.** For example, one per agent.

### Usage (paid per use, any plan)

- **Settlement fees on market sales** (labor, data, compute, plugins).
  - This follows the plugin rule from the revenue-sharing work: the
    author/seller keeps their price, and our share is the fee.
  - *Proposed* rate, to be set with the market launch.
- **Relay writes, storage, and cloud minutes past Pro's included amounts.**
  - Paid from credits, only if turned on, under your cap.
- **Hosted decisions (NIP-DEC) past the free daily allowance.**
  - Same meter as model usage.
- **For agents without Pro:** metered relay writes and Blossom storage from
  Lightning credits at usage prices.
  - This keeps the subscription-free path open for agents (README open
    question 2).

### What we never charge for

- Reading the network.
- Your keys.
- Exporting your events and files.
- Being listed or ranked: **trust and registry placement are never sold**,
  or the trust signals stop meaning anything.

## How the gates work

- **Relay limits are tied to plan:**
  - The relay looks up the signed-in key's account plan (the same answer as
    `GET /v1/me/plan`) after NIP-42 sign-in.
  - It answers `restricted:` / `rate-limited:` OK messages that name the
    limit and the upgrade link.
  - NIP-11 publishes the limits per plan.
- **Blossom uploads** answer `402` with the same `pro_required` body as the
  web API. Paid uploads are usage per GB-month.
- **CLI commands for Pro-only hosted features** say so in one line with the
  upgrade link; `--json` gives the same machine-readable refusal.

## Build order (issues to file)

1. Wave 1 commands, including NIP-34 git.
2. Relay limits per plan: NIP-42 sign-in tied to the account plan, the free
   write allowance, and the NIP-11 limits.
3. Wave 2 commands, plus NIP-05 hosting at openagents.com.
4. The Blossom hosting service on our storage, the Pro storage pool, and the
   `file` commands.
5. The `market` group, and always-on market agents on Pro cloud minutes.
6. NIP-DS v2: spec, validator, `data` commands, the pay-to-decrypt payment
   profile, and a regtest sale.
7. The hosted NIP-46 bunker (Pro).
8. Settlement fees on MKT sales, at the market launch.

## Open questions for the owner

1. **The relay numbers:**
   - free: 500 events/day and 64 KB;
   - Pro: 20,000 events/day and 1 MB.
2. **The bunker:** Pro-only (proposed), or free with Pro limits?
3. **Market settlement fee:** a percentage, or a flat fee per sale?
4. **Agents without Pro:** can they use Blossom and relay limits on usage
   only (proposed yes)?

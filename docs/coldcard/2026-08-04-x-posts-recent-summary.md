# Coldcard: recent X posts summary (2026-08-04)

**Label:** external observation (X public posts via X API v2 recent search and
official account queries). Not accepted live forensic evidence. Not a loss
total, exploitability verdict, or publication claim.

**Probe time:** 2026-08-04 ~15:30–15:45 UTC  
**Access path:** X API v2 app-only Bearer → `GET /2/tweets/search/recent`  
**Tooling receipt:** Omega `crates/x_api` + openagents access note
[`docs/grok/2026-08-04-x-api-and-xai-x-search-access.md`](../grok/2026-08-04-x-api-and-xai-x-search-access.md)

Cross-check: xAI Grok Responses API with `x_search` produced a compatible
theme list (same day); figures still disagree across posts.

## What we queried

```text
(coldcard OR Coldcard OR #Coldcard OR from:COLDCARDwallet OR from:coinkite) -is:retweet lang:en
from:COLDCARDwallet -is:retweet
(from:coinkite OR from:nvk) (coldcard OR Coldcard OR firmware OR TRNG OR advisory) -is:retweet
```

Sample sizes: 25+25 general English posts (two pages), 15 official
`@COLDCARDwallet` posts, 5 Coinkite/nvk hits in-window.

Official profile snapshot at probe time:

- `@COLDCARDwallet` — name COLDCARD — ~67.9k followers  
- Bio began with a security-advisory notice pointing users to a blog post

## Dominant themes (timeline discourse)

### 1. Ongoing theft / weak-key narrative

Many posts claim a multi-year firmware/RNG defect (often dated to ~2021) that
produced weak keys, with attackers draining affected single-sig wallets.

Reported loss figures **vary widely** across posts (examples seen in-window:
~$89M, ~1,755 BTC / ~5k wallets, ~2,055 BTC / ~$130M / 7,300+ addresses). Treat
every dollar/BTC total as **unverified social reporting** until reconciled to
independent chain analysis receipts outside this note.

### 2. Official urgency to migrate

High-signal official posts (likes at probe time):

| Time (UTC) | ID | Summary | Likes (probe) |
| --- | --- | ---: | ---: |
| 2026-08-04 11:07 | `2084596971268956161` | Urgent: migrate funds; follow model advisory; upgrade device; new seed; move carefully; threat ongoing | 271 |
| 2026-08-04 13:29 | `2084632863756955661` | NOTICE: claims that current firmware **permanently bricks** devices are **incorrect**; TRNG fault is volatile; power-cycle; fails closed; fix link | 67 |
| 2026-08-03 14:10 | `2084280694864224641` | Reply: an earlier reporter’s issue is unrelated; patch + credit already issued | 35 |

Related Coinkite:

| Time (UTC) | Handle | ID | Summary | Likes |
| --- | --- | --- | --- | ---: |
| 2026-08-04 13:19 | `@Coinkite` | `2084630256296624637` | Urgent; threat active; update COLDCARD; new seed; move funds; share with less-online users | 8 |
| 2026-07-30 21:47 | `@nvk` | `2082946325088231471` | Investigation ongoing; blog post incoming | 310 |
| 2026-07-31 15:42 | `@nvk` | `2083216713693151552` | Link-only post (high engagement) | 1513 |
| 2026-07-31 21:27 | `@nvk` | `2083303465749479654` | Confirms firmware bug framing; still requires new seed + fund move | 6 |

### 3. Secondary phishing / social engineering

Posts (e.g. `@TFTC21` citing American HODL) warn that scammers send phishing
mail for a fake **“ColdCard Desktop”** app. Community reminder: there is no
official Coldcard desktop app; panic is being used as bait.

### 4. Market / custody behavior chatter

Recurring non-technical claims in the firehose:

- Record inflows to centralized exchanges attributed to the incident (some cite
  OKX commentary — unverified here).
- Active-address spikes and “BTC holding ~$63–64k despite drama” narratives.
- Calls to re-evaluate self-custody stacks, multisig, and “gold standard”
  setups.
- Meme / satirical acquisition jokes and price-bottom takes (noise).

### 5. Technical dissection and blame narratives

Fragments seen repeatedly (all still external observation):

- Historical entropy / `libngu` / software RNG concerns; references to prior
  auditor warnings (e.g. James O’Beirne / May 2025 claims in media posts).
- AI-assisted discovery memes (“found with frontier models for $2”, “vibecode”).
- Criticism of slogans vs. response speed; defense that advanced users use
  layered security / multisig.

### 6. Misinformation control from the brand account

`@COLDCARDwallet` is actively replying to outlets and users to:

- Correct permanent-brick claims (power-cycle for volatile TRNG fault).
- Push migration while team is “heads down” helping key moves.
- Separate unrelated historical reports from the current advisory.

## Representative public URLs (for follow-up)

Use as conversation anchors only:

- Official urgent migrate:  
  <https://x.com/COLDCARDwallet/status/2084596971268956161>
- Official brick-claim correction:  
  <https://x.com/COLDCARDwallet/status/2084632863756955661>
- Coinkite urgent:  
  <https://x.com/Coinkite/status/2084630256296624637>
- Phishing warning (TFTC):  
  <https://x.com/TFTC21/status/2084666893999419823>
- Loss-figure example posts (unverified):  
  <https://x.com/dpivovar_off/status/2084666386895257737>  
  <https://x.com/Bit_Ledger11/status/2084664088089821444>

## Operator takeaways for OpenAgents / Omega

1. **Access works.** App-only X API recent search is sufficient for live
   discourse monitoring; Omega `x-api` CLI is the durable operator entry.
2. **Do not promote social loss totals.** Route numbers through independent
   chain/forensic receipts already governed by the coldcard evidence ladder.
3. **Official migrate + brick-correction posts are the primary brand
   signals** in this window; amplify those carefully only if product surfaces
   need public-status links, still labeled external.
4. **Expect phishing parallel to the advisory.** Any user-facing copy should
   warn against fake desktop apps and unsolicited download links.
5. **xAI `x_search` is useful for synthesis**, not for replacing exact post
   IDs/metrics in receipts.

## Reproduce

```sh
export X_BEARER_TOKEN='…from console.x.com; never commit…'

# Omega workspace
cargo run -p x_api -- search \
  '(coldcard OR Coldcard OR from:COLDCARDwallet) -is:retweet lang:en' \
  --max-results 25

cargo run -p x_api -- search 'from:COLDCARDwallet -is:retweet' --max-results 15
cargo run -p x_api -- user COLDCARDwallet
```

## Related repo docs

- [`README.md`](README.md) — coldcard doc index and evidence labels  
- [`2026-08-02-wallet-security-posts-and-omega-thread-audit.md`](2026-08-02-wallet-security-posts-and-omega-thread-audit.md)  
- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md)  
- Access map: [`../grok/2026-08-04-x-api-and-xai-x-search-access.md`](../grok/2026-08-04-x-api-and-xai-x-search-access.md)

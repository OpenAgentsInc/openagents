# Kitchen Sink, second pass: the X archive

Draft, 2026-10-09, for [#11125](https://github.com/OpenAgentsInc/openagents/issues/11125).
For the owner to review. The first pass read the 289 episodes; this pass reads
three years of public posts on [@OpenAgentsInc](https://x.com/OpenAgentsInc)
and asks one question about each claim: **what must OpenAgents ship, measure,
or publish so that this is true?**

The owner's brief: "make all of the smack talking … brought to life … ensure
that essentially everything that I've ever said becomes true from our V1 and
our roadmap." The answer is below, theme by theme. New ledger rows and the
tweet references for existing rows are in the [ledger](ledger.md#t-references-from-the-x-archive).

## What was read

| | Count |
| --- | --- |
| Posts read (own posts, threads, replies, long posts; retweets skipped) | 3,727 |
| Date range | 2023-08-25 to 2026-10-08 |
| Claims extracted (promises, boasts, jabs, principles, predictions) | 1,435 |
| of which promises | 473 |
| of which jabs at a named competitor | 274 |
| Themes | 23 |
| Ledger rows that got post references | 91 open rows and 14 dropped rows |
| New ledger rows | 7 |
| Contradictions with the current direction | 12 |
| New issues | 7 |

Only the account's own public posts were read. Direct messages, account data,
and other people's posts were not opened. Private people are not named.
Companies are named only as the product a promise has to beat, in neutral words.

Most-jabbed competitors, by number of claims: OpenAI (90), Anthropic (42),
closed labs in general (22), Cursor (16), Claude Code (9), Microsoft (7),
Google, Meta, Devin (6 each).

## What the archive adds to the episodes

The episodes say what to build. The posts say **what the owner wants to be
able to say out loud**, and almost every boast has a number in it. Five things
stand out.

1. **Pay people, and prove it.** The single most repeated boast is "we've paid
   infinity percent more developers than OpenAI" (2024-02-10 through
   2026-04-13). Today there is no public page that proves it. A payouts page
   with totals and proofs is the cheapest way to make the loudest claim true.
2. **Cheaper and just as good, measured.** Coder One "~45% cheaper than Opus
   in Claude Code for about the same success rate" (2026-09-23), "53x cheaper"
   (2026-09-26), "$2.50 per PR, 10x cheaper than a human" (2024-08-28),
   "$0.57 PR" (2025-03-09). These need a public, re-runnable benchmark page,
   not posts.
3. **Neutral across labs is the product.** "Labs can't lead on coding agents,
   a neutral provider must" (2026-09-05); "sign up once and we use the best
   model" (2024-10-04). That is principle 5, and it means supporting every
   lab, including the ones jabbed most.
4. **Open, all of it.** "They DRM, we torrent" (2024-05-09). The repo is
   Apache 2.0 today, but the posts have claimed AGPL3, CC0, and Apache 2.0 at
   different times, and once said the web UI is *not* open source
   (2026-05-15). One statement, kept, is needed.
5. **Never a token; Bitcoin to contributors.** Said at least 15 times from
   2023 to 2026. The owner's newer payments decision (accept every rail:
   card, Lightning, x402, MPP, ACP, UCP, AP2) fits the oldest version of this
   promise: "customers pay with card like OpenAI; contributors earn bitcoin"
   (2023-12-21), "customers can pay in dollars and we pay out in bitcoin"
   (2024-11-10). See decision 1.

It also adds a few promises the episodes don't make plainly: no ads in chats,
no tracking cookies, per-helper CPU and memory shown in Coder, the "accepted
outcomes per kilowatt-hour" measure, desktop speed numbers, and a stated rule
for refusals. They are the new ledger rows below.

## Themes and the promises they become

Tweet links use the date as the label. Ledger IDs refer to the
[ledger](ledger.md); **new** marks a row added in this pass.

| Theme | Claims | What the posts say | Product promise | Ledger | Example posts |
| --- | --- | --- | --- | --- | --- |
| Coding agent | 149 | Issue in, PR out; autopilots beat copilots; work overnight; command a fleet; replace the editor | Hand it an issue, get a tested PR; walk away and it keeps going; many agents at once | C6, C8, C17, C14 | [2024-08-27](https://x.com/OpenAgentsInc/status/1828511026410655781), [2024-08-28](https://x.com/OpenAgentsInc/status/1828629873063014755), [2025-12-21](https://x.com/OpenAgentsInc/status/2002757002863546853), [2026-09-05](https://x.com/OpenAgentsInc/status/2096361490806657113) |
| Open source | 139 | 100% open source; "open or die"; agents must be inspectable; one license | Everything shipped is in the public repo under one license | K4, K5, **K7** | [2023-10-03](https://x.com/OpenAgentsInc/status/1709305653469086115), [2024-04-09](https://x.com/OpenAgentsInc/status/1777692495012405439), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| Compute market | 92 | Sell spare compute for Bitcoin with one button; 1M Pylons; 20 GW of idle compute | "Go Online" earns sats for accepted work; live Pylon count | J1, J2, J6 | [2023-09-12](https://x.com/OpenAgentsInc/status/1701628445648736626), [2026-01-08](https://x.com/OpenAgentsInc/status/2009142870775644644), [2026-04-08](https://x.com/OpenAgentsInc/status/2041991629662277651) |
| Contributor pay | 88 | Rev-share to builders; bounties; referrals for life; "pay the people" | Authors, providers, and referrers are paid in Bitcoin, with a public split and public totals | F3, I5, I6, I7, J6, J7 | [2023-12-12](https://x.com/OpenAgentsInc/status/1734694536482529334), [2024-05-22](https://x.com/OpenAgentsInc/status/1793146221504451013), [2026-06-19](https://x.com/OpenAgentsInc/status/2068102703092543974) |
| Open protocols, Nostr | 88 | Agents talk over Nostr; publish NIPs; no single front door | Anyone can join the network by following open specs | J3, J4, A7 | [2024-04-29](https://x.com/OpenAgentsInc/status/1785045575123005713), [2026-02-02](https://x.com/OpenAgentsInc/status/2018180415035408860), [2026-09-25](https://x.com/OpenAgentsInc/status/2103467660121878775) |
| Bitcoin and Lightning | 87 | Bitcoin only; never a token; Lightning address for every user | No token, ever; payouts in Bitcoin; self-custody wallet | I1, I5, I8 | [2024-02-18](https://x.com/OpenAgentsInc/status/1759278173148135788), [2025-01-08](https://x.com/OpenAgentsInc/status/1876869977581527550), [2026-06-12](https://x.com/OpenAgentsInc/status/2065535602905448870) |
| One network of agents | 73 | One market, not 300 vertical agents; meta-agents; general agents are back | One conversation, with the network behind it | B1, F2 | [2024-12-20](https://x.com/OpenAgentsInc/status/1869951506398552245), [2025-04-14](https://x.com/OpenAgentsInc/status/1911622886223208634), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| Neutral across labs | 70 | Switch models mid-chat; use every lab at once; not dependent on one lab | Pick or switch models; delegate to several labs' agents in one chat; keep working when one fails | B2, B3, C7, E5 | [2024-04-06](https://x.com/OpenAgentsInc/status/1776665794048409999), [2025-03-18](https://x.com/OpenAgentsInc/status/1901807373980758366), [2026-09-05](https://x.com/OpenAgentsInc/status/2096058017133261230) |
| Agent payments | 69 | Agents hold balances, pay L402 endpoints, pay each other | Your agent pays and gets paid under your limits; agents can pay us any standard way | I3, E3, E4, K6 | [2024-01-28](https://x.com/OpenAgentsInc/status/1751700732963672411), [2025-05-05](https://x.com/OpenAgentsInc/status/1919419077887410389), [2026-02-18](https://x.com/OpenAgentsInc/status/2024259092810703136) |
| Speed of shipping | 66 | Auto-deploy in seconds; "built in 30 minutes"; use OpenAgents to build OpenAgents | A plain-words changelog per release, linked to the work | H6 | [2024-03-14](https://x.com/OpenAgentsInc/status/1768323023705182393), [2024-04-15](https://x.com/OpenAgentsInc/status/1779907555977769160), [2026-08-23](https://x.com/OpenAgentsInc/status/2091533226837721279) |
| Surfaces | 63 | Phone app for coding; desktop not a VS Code fork; StarCraft hotkeys; opens in 50 ms | Phone, desktop, terminal, web share one account; desktop is fast | L3–L7, D4, **L10** | [2025-10-22](https://x.com/OpenAgentsInc/status/1981124099318415633), [2025-03-29](https://x.com/OpenAgentsInc/status/1906077696989565297), [2026-07-10](https://x.com/OpenAgentsInc/status/2075680613731078265) |
| Pricing | 53 | $10 Pro replaces two $21 plans; won't cost $200/mo; best free plan; $1/hour cloud agents | Free to start, one Pro plan, cloud computers by the hour, prices published side by side | B14, E2, C15 | [2024-04-09](https://x.com/OpenAgentsInc/status/1777496991099998302), [2024-12-06](https://x.com/OpenAgentsInc/status/1865112576830693650), [2026-08-31](https://x.com/OpenAgentsInc/status/2094539323374698817), [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936) |
| Plugins and skills | 52 | Plugin authors earn per use; npm for agents; no review queue, but measured | New abilities in a shared registry, admitted on measured results, paying authors | F1–F6 | [2024-01-18](https://x.com/OpenAgentsInc/status/1747994309549318228), [2024-06-11](https://x.com/OpenAgentsInc/status/1800665114573521029), [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) |
| Business use | 46 | Replace a $5,000/mo developer with a $500/mo agent; teams; Autopilot for businesses | Team workspaces; paid capacity for businesses | B15 | [2024-12-18](https://x.com/OpenAgentsInc/status/1869430520381354057), [2026-06-04](https://x.com/OpenAgentsInc/status/2062626257443909886) |
| Local models | 42 | Faster than Ollama and llama.cpp; run open models at home | Run open models on your machine, with published speed numbers | E9, G8 | [2026-03-10](https://x.com/OpenAgentsInc/status/2031255043903549942), [2026-03-28](https://x.com/OpenAgentsInc/status/2037717730707542232), [2026-06-18](https://x.com/OpenAgentsInc/status/2067627478626001349) |
| You see everything | 40 | Show chain of thought and action; public dashboard of every step; no hidden routing | Every step, model, and cost visible; helper agents' resource use visible | C2, E8, **C20** | [2023-12-03](https://x.com/OpenAgentsInc/status/1731156734335398303), [2025-01-20](https://x.com/OpenAgentsInc/status/1881354835943129125), [2026-09-03](https://x.com/OpenAgentsInc/status/2095552147433587011) |
| Decentralized training | 37 | Largest distributed training run; pay providers per minute | Dropped as a product (X6); research only | X6 | [2026-04-10](https://x.com/OpenAgentsInc/status/2042450069857693926), [2026-06-11](https://x.com/OpenAgentsInc/status/2065196586817216622) |
| Privacy | 31 | No cookie banner; no personal info needed; keys on your device; no ads in chats | Private by default; no ads; no tracking; your keys stay with you | E6, A6, D3, **B16** | [2024-05-20](https://x.com/OpenAgentsInc/status/1792360190689386537), [2024-10-21](https://x.com/OpenAgentsInc/status/1848211700450726214), [2024-12-18](https://x.com/OpenAgentsInc/status/1869417096163316093) |
| Traces and data | 26 | Sell your scrubbed traces; data market; open redaction | Upload privately; later sell scrubbed traces with consent | H2, H3, H4 | [2025-12-19](https://x.com/OpenAgentsInc/status/2002057459574452667), [2026-09-10](https://x.com/OpenAgentsInc/status/2098051068605215146) |
| Agent-ready API | 26 | OpenAI-compatible drop-in API; one API for everything; openapi.json for agents | One OpenAI-compatible key; agent-readable docs | E1, K1–K3 | [2023-10-29](https://x.com/OpenAgentsInc/status/1718766857388204090), [2026-02-17](https://x.com/OpenAgentsInc/status/2023659999268864077), [2026-06-19](https://x.com/OpenAgentsInc/status/2068102703092543974) |
| Proof and benchmarks | 25 | SWE-bench high score; leaderboards that mix tools and models are misleading; accepted outcomes per kWh | Head-to-head results with scripts; a public promises page; a cost-per-outcome measure | G2, G6, **G8**, **G9** | [2024-08-13](https://x.com/OpenAgentsInc/status/1823455135143518470), [2025-02-03](https://x.com/OpenAgentsInc/status/1886216375649120722), [2026-06-08](https://x.com/OpenAgentsInc/status/2064074384881463459), [2026-06-09](https://x.com/OpenAgentsInc/status/2064390975267480060) |
| The Verse | 12 | Agent MMO; guilds; agents keep working in the world | The Verse shows real agent work | M1–M7 | [2024-08-01](https://x.com/OpenAgentsInc/status/1819071289740644563), [2024-07-11](https://x.com/OpenAgentsInc/status/1811507069398483192), [2026-10-06](https://x.com/OpenAgentsInc/status/2107294286328787243) |
| Refusals and control | (in coding, 6) | "The user is always right"; "Coder should never show refusals" | Coder adds no refusals of its own; when a provider refuses, it says which one and offers another model | C3, **C21** | [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936), [2026-09-14](https://x.com/OpenAgentsInc/status/2099561282944860387) |

## Make it true: the smack talk, turned into measures

Each row is a jab or boast, the plain thing OpenAgents must do or show so the
claim holds, how to measure it, and when. Phases: **V1** = Monday 10-12;
**V1 week** = the rest of 1.0, the week of 10-12; **Late Oct**, **Nov**, and
**After** follow the [sequencing](README.md#sequencing).

Rule for launch copy: a competitive claim goes out only after its measure is
public. Until then, the copy states the feature, not the comparison
([#11129](https://github.com/OpenAgentsInc/openagents/issues/11129)).

| # | What was said | What must be true | Measure | Phase | Ledger, issue |
| --- | --- | --- | --- | --- | --- |
| 1 | "We've paid infinity percent more developers than OpenAI"; "paid more community contributors than all other AI labs combined" ([2024-05-22](https://x.com/OpenAgentsInc/status/1793146221504451013), [2025-01-11](https://x.com/OpenAgentsInc/status/1878103613127438817), [2026-04-13](https://x.com/OpenAgentsInc/status/2043782380171767849)) | A public payouts page: total sats paid, number of distinct people and agents paid, split by bounties, compute, tips, and plugin revenue, each linked to a payment proof | Distinct non-staff recipients and total sats, updated daily | Late Oct (page); claim in copy only after | J6, [#11130](https://github.com/OpenAgentsInc/openagents/issues/11130) |
| 2 | "Claude Code, we're here to eat your lunch"; "~45% cheaper than Opus in Claude Code at about the same success rate"; "53x cheaper than Fable" ([2026-09-23](https://x.com/OpenAgentsInc/status/2102773483109335209), [2026-09-26](https://x.com/OpenAgentsInc/status/2103695213680091618)) | A public benchmark page: Coder against Claude Code and Codex on the same tasks, with cost per solved task, pass rate, time, and failures, and the scripts to re-run it | Equal or better pass rate at 60% or less of the cost, on the full Terminal-Bench set (today: 8 tasks) | V1 week (page), Late Oct (full set) | G2, **G8**, [#11131](https://github.com/OpenAgentsInc/openagents/issues/11131) |
| 3 | "Cursor, your days are numbered"; "autopilots > copilots"; "GitHub issue to pull request, world first"; "$2.50 a PR" ([2024-08-27](https://x.com/OpenAgentsInc/status/1828511026410655781), [2024-08-28](https://x.com/OpenAgentsInc/status/1828640179579490632), [2025-02-12](https://x.com/OpenAgentsInc/status/1889766594798379419), [2025-10-29](https://x.com/OpenAgentsInc/status/1983640557696987612)) | One-click issue to tested pull request from the web, running on your connected computer or a cloud computer | Share of a fixed set of real issues merged without human edits; median cost per merged PR | Late Oct | C6, C8 |
| 4 | "Labs can't lead on coding agents; a neutral provider must"; "does any closed lab let you use both models in one chat?" ([2025-03-18](https://x.com/OpenAgentsInc/status/1901807373980758366), [2026-09-05](https://x.com/OpenAgentsInc/status/2096058017133261230)) | In one conversation: switch between labs' models, and hand parts to Codex and Claude Code, combining the results | A release smoke test where one chat uses models from at least two labs and delegates to two agents | V1 (switching), V1 week (delegation) | B2, C7 |
| 5 | "Never care about OpenAI's API downtime again"; "DVMs fix OpenAI outages"; "we were up during the ChatGPT outage" ([2023-10-20](https://x.com/OpenAgentsInc/status/1715371396090507618), [2024-06-04](https://x.com/OpenAgentsInc/status/1798030930764001624), [2024-12-13](https://x.com/OpenAgentsInc/status/1867386677091791003)) | When a provider fails or refuses, the chat keeps going on another, and says that it switched | A test that blocks the primary provider and still completes the chat, with the switch shown | V1 week | B3, [#11132](https://github.com/OpenAgentsInc/openagents/issues/11132) |
| 6 | "Closed agents hide their decisions; ClosedAI loses to OpenAgents"; "no 'Ran 33 shell commands'"; "they don't show you resource usage" ([2025-01-20](https://x.com/OpenAgentsInc/status/1881354835943129125), [2026-09-03](https://x.com/OpenAgentsInc/status/2095552147433587011), [2026-09-05](https://x.com/OpenAgentsInc/status/2096357223253389345)) | Every step shows the command, the model that ran it, and its cost; Coder shows CPU and memory per helper agent | 100% of steps in a trace name their model and cost; per-helper resource view in Coder | V1 (steps), V1 week (resources) | C2, E8, **C20** |
| 7 | "ClosedAI makes secret changes; we publish a public changelog with all the code" ([2024-04-15](https://x.com/OpenAgentsInc/status/1779907555977769160)) | Release notes for every release, each change linked to its commit or issue | Every 1.0 release has linked notes | V1 | H6, #11104 |
| 8 | "They DRM, we torrent"; "open source will wreck every closed-source AI employee"; "if it's not open source we aren't going to use it" ([2024-04-09](https://x.com/OpenAgentsInc/status/1777649212420698513), [2024-05-09](https://x.com/OpenAgentsInc/status/1788669950409777512), [2026-02-10](https://x.com/OpenAgentsInc/status/2021285999830024690)) | Everything we ship, including the website and API, is in the public repo under Apache 2.0, and you can run your own copy | The site shows the commit it was built from; a self-host guide that a stranger completes | V1 (license, commit), Late Oct (self-host) | K4, K5, **K7**, [#11133](https://github.com/OpenAgentsInc/openagents/issues/11133) |
| 9 | "OpenAgents Pro blows away ChatGPT Pro … won't cost $200/month"; "$10 replaces two $21 plans"; "best free plan on the market"; "$1/hour per agent" ([2024-04-09](https://x.com/OpenAgentsInc/status/1777496991099998302), [2024-12-06](https://x.com/OpenAgentsInc/status/1865112576830693650), [2026-08-31](https://x.com/OpenAgentsInc/status/2094539323374698817), [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936)) | A public price page: what Free and Pro include, cloud computers by the hour, set next to the $20 plans of the two biggest labs | The price page exists; Pro is at or below their $20 tier with more included | V1 week (with Pro) | B14, E2, C15 |
| 10 | "GPTs, but we actually pay you"; "your coding agent pays you"; OpenAI and Anthropic "launched marketplaces with no monetization" ([2024-04-25](https://x.com/OpenAgentsInc/status/1783609488064311322), [2025-12-16](https://x.com/OpenAgentsInc/status/2001063504053223838), [2026-07-02](https://x.com/OpenAgentsInc/status/2072540408324722771)) | A published split, then the first plugin author paid from someone else's paid use | First author payout with a proof link on the payouts page | Nov | F2, F3, I6 |
| 11 | "Psionic beats Ollama on Qwen 3.5"; "faster than llama.cpp on GPT-OSS 20B" ([2026-03-10](https://x.com/OpenAgentsInc/status/2031255043903549942), [2026-03-28](https://x.com/OpenAgentsInc/status/2037717730707542232), [2026-06-18](https://x.com/OpenAgentsInc/status/2067627478626001349)) | Local-inference results on the benchmark page: tokens per second by model and GPU, with the commands to re-run | Re-run within 10% of the published numbers | Late Oct | E9, **G8**, #11131 |
| 12 | "OpenAgents will NEVER issue a token"; "the only coin is bitcoin" ([2024-02-18](https://x.com/OpenAgentsInc/status/1759278173148135788), [2025-01-08](https://x.com/OpenAgentsInc/status/1876869977581527550), [2026-06-12](https://x.com/OpenAgentsInc/status/2065535602905448870)) | No token, ever; contributor payouts in Bitcoin; buyers pay with whatever standard rail they use (decision 1) | Written on the promises and payments pages | V1 | I8, K6 |
| 13 | "Hundreds of billions in VC and nobody made a good mobile app for coding on the go" ([2025-10-22](https://x.com/OpenAgentsInc/status/1981124099318415633), [2026-09-01](https://x.com/OpenAgentsInc/status/2094626130275811333)) | The phone shows your account's chats and runs Coder on your computer | Phone joins account chats; pairing works from a public build | V1 week | D4, D5, C14 |
| 14 | "Codex Desktop crashed three times"; "Cursor can't open a file"; "threads open in under 50 ms" ([2026-07-10](https://x.com/OpenAgentsInc/status/2075680613731078265), [2026-07-19](https://x.com/OpenAgentsInc/status/2078687966210204011)) | Desktop opens a file in under a second and switches chats in under 50 ms; a crash in one pane never takes the app down | Timings in the release smoke test; crash isolation test | V1 week | L3, L9, **L10** |
| 15 | "Agents beat models"; "labs burn tokens on loops to inflate metrics"; "accepted outcomes per kilowatt-hour" ([2026-06-08](https://x.com/OpenAgentsInc/status/2064074384881463459), [2026-07-20](https://x.com/OpenAgentsInc/status/2079311647068283131)) | Publish accepted outcomes per dollar now, and per kilowatt-hour once energy is metered | A number on the stats page with its method | After | **G9** |
| 16 | "We will never put ads in chats"; "you'll never see a cookie banner on our sites" ([2024-05-20](https://x.com/OpenAgentsInc/status/1792360190689386537), [2024-10-21](https://x.com/OpenAgentsInc/status/1848211700450726214)) | No ads in your chats and no tracking cookies, written in the privacy policy | Policy text; no third-party trackers on the site | V1 | **B16** |
| 17 | "A year from now there will be 1M+ live Pylons"; "20 GW of stranded compute vs OpenAI's 2 GW" ([2026-04-03](https://x.com/OpenAgentsInc/status/2040059166061170741), [2026-04-08](https://x.com/OpenAgentsInc/status/2041991629662277651)) | A live count of online Pylons and paid jobs on `/stats`; growth comes after paid jobs return | Live count; no forecast in copy | Nov | J1, J6 |
| 18 | "Agents can pay openagents.com"; "L402 beats x402" ([2024-01-26](https://x.com/OpenAgentsInc/status/1750729304504213964), [2026-02-25](https://x.com/OpenAgentsInc/status/2026754692873646279)) | Per the owner's payments decision: agents discover the site, call the API, and pay on Bitcoin (Lightning through L402, x402, MPP) or by card (ACP, UCP, AP2, Stripe) | Each rail has a working paid request in the smoke test | V1 week to Nov | E3, K6, #11085 |

## Contradictions and decisions for the owner

| # | The posts said | The current direction says | Recommendation |
| --- | --- | --- | --- |
| 1 | Bitcoin only; x402 is "permissioned Coinbase spyware"; "will use bitcoin regardless, not USDC" ([2026-02-25](https://x.com/OpenAgentsInc/status/2026754692873646279), [2026-03-16](https://x.com/OpenAgentsInc/status/2033546613084360875), [2024-09-03](https://x.com/OpenAgentsInc/status/1830827588047999058)) | Owner, 2026-10-09: Bitcoin and Bitcoin-based stablecoins only (Lightning through x402, MPP, L402; Taproot Assets stablecoins later), plus card for credits and Pro; no USDC, Base, Solana, EVM or Tempo | **Decided: "no token, ever; Bitcoin is our money; agents pay on Lightning or by card."** The posts stand: not USDC. I8 and K6 reworded to match. |
| 2 | "All wallet infra and what runs on your computer is open source … web UI and business products are not" ([2026-05-15](https://x.com/OpenAgentsInc/status/2055377718993207365)) | K4: all of it is open source | **Everything shipped is open.** Treat the May post as superseded; show the build commit on the site. |
| 3 | AGPL3 (2024-04, 2025-03), CC0 (2024-08, 2025-07), Apache 2.0 (2025-10, 2026-08, 2026-10) | `LICENSE` is Apache 2.0 | **Apache 2.0 for everything**, stated once on `/promises` (K7). |
| 4 | "All chats are public, both read and write" ([2024-03-14](https://x.com/OpenAgentsInc/status/1768302921819570260)) | Private by default (E6, H2) | **Keep private by default.** Sharing stays opt-in per chat or trace. |
| 5 | "We will never put ads in chats" (2024-05-20) vs. "OpenAI will show ads to humans, we'll show ads to agents"; ads with kickbacks ([2026-03-25](https://x.com/OpenAgentsInc/status/2036932224839262703), [2026-06-15](https://x.com/OpenAgentsInc/status/2066626765397647635)) | No ad product | **No ads in your chats, ever (B16).** Any paid listing that agents see must be labeled, opt-in, and later; leave it off the roadmap for now. |
| 6 | "Coder should NEVER show refusals"; "virtually no guardrails" ([2026-09-14](https://x.com/OpenAgentsInc/status/2099561282944860387), [2026-09-03](https://x.com/OpenAgentsInc/status/2095596118080151936)) | C3: acts without asking | **Promise that Coder adds no refusals of its own (C21)**, but not "never refuses": providers' rules still apply. When one refuses, say which, and offer another model. |
| 7 | Cancel the Anthropic subscription and remove all Claude code; a standing position against Anthropic ([2026-01-11](https://x.com/OpenAgentsInc/status/2010429987703464142), [2026-03-31](https://x.com/OpenAgentsInc/status/2038971422840922301)) | Neutral across labs (principle 5); Claude keys and Claude Code delegation supported (C7, C11) | **Neutrality wins.** Support every lab's models and agents; compete on measured cost and quality (items 2 and 4), not exclusion. |
| 8 | Pro at $10/month (2024-04) | B14: $20 Pro | **Keep $20** and show what's included next to other $20 plans (item 9). |
| 9 | Dated predictions now past due: "Claude Code-style agents obsolete by Q3 2026" ([2026-01-15](https://x.com/OpenAgentsInc/status/2011625411562909909)); "best agents in six months" ([2026-02-02](https://x.com/OpenAgentsInc/status/2018180415035408860)); Agent Store relaunch Q1 2026; five markets, one per week (2026-03-07) | Not on the roadmap with dates | **Don't repeat dated predictions.** Put the underlying features on `/roadmap` without dates; measures (items 1–3) replace forecasts. |
| 10 | "World's largest distributed training run" ([2026-04-10](https://x.com/OpenAgentsInc/status/2042450069857693926), [2026-06-11](https://x.com/OpenAgentsInc/status/2065196586817216622)) | Paid training dropped (X6) | **Keep dropped**, and don't use "largest run" in copy unless a public count backs it. |
| 11 | "We're building a GitHub replacement" ([2026-08-24](https://x.com/OpenAgentsInc/status/2091887799137866064)) | X5 dropped ("too big an apple") | **Keep dropped.** GitHub stays the home for repos. |
| 12 | Name-calling of labs and founders across all three years | The spec names competitors only as the thing to beat | **Launch copy cites numbers, not insults.** The table above is the smack talk made true. |

## Issues opened from this pass

| Issue | What | Phase | On the V1 board |
| --- | --- | --- | --- |
| [#11129](https://github.com/OpenAgentsInc/openagents/issues/11129) | Launch claims check: every competitive claim in the 1.0 announcement links to public evidence, or is cut | V1 | Yes |
| [#11130](https://github.com/OpenAgentsInc/openagents/issues/11130) | Public payouts page: total sats paid, distinct recipients, by kind, with proofs | Late Oct | No |
| [#11131](https://github.com/OpenAgentsInc/openagents/issues/11131) | Public benchmark page: Coder vs Claude Code vs Codex (cost, pass, time), plus local inference speed, with re-run scripts | V1 week → Late Oct | No |
| [#11132](https://github.com/OpenAgentsInc/openagents/issues/11132) | Visible provider fallback: keep the chat going when a provider fails or refuses, and say so | V1 week | No |
| [#11133](https://github.com/OpenAgentsInc/openagents/issues/11133) | Self-host guide: run your own OpenAgents (web, API, relay) from the public repo | Late Oct | No |
| [#11134](https://github.com/OpenAgentsInc/openagents/issues/11134) | Whole-account export: download all your chats, projects, traces, and settings | Late Oct | No |
| [#11135](https://github.com/OpenAgentsInc/openagents/issues/11135) | Turn on referral links for users (backend built in REV-27 to REV-32) once the split is published | Nov | No |

## How this pass was made

- Source: the owner's X archive, files `data/tweets.js` and `data/note-tweet.js`
  (long posts were joined to their tweets by text prefix: 322 of 420). The
  community and deleted-post files were empty or skipped. The handle was read
  from `data/account.js`; nothing else from that file was used.
- A small script in a scratch directory parsed the files (Python `-I`, path as
  an argument), dropped 1,113 retweets, and split the 3,727 remaining posts
  into eight date ranges. Eight readers each pulled every claim from one range
  into a table (id, date, theme, type, competitor, claim). Nothing from the
  archive was copied into the repo except post IDs, dates, and short quotes.
- Claims were mapped to ledger rows by hand. Status words for new rows follow
  the [ledger](ledger.md#status-words) and were checked against `main`, not
  against the posts.

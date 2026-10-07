# Selling spare compute for bitcoin: a history

This document records OpenAgents' public history with one idea: anyone can
sell a computer's spare capacity for bitcoin, and buyers, including agents,
pay the providers over Lightning. It covers GPUtopia, the OpenAgents compute
network, Pylon, the compute market, and the revenue splits built around them.
Every claim links to a post on X, a retained transcript, or a file in this
repository.

## Summary

- **The idea.** Pool spare GPUs and computers into one open market, route
  inference and other jobs to them, and pay each provider in sats over
  Lightning, with splits to the other contributors in a workflow.
- **First public mention.** On 2023-08-26 the account that is now
  @OpenAgentsInc, then named GPUtopia, posted that GPUs would not stay supply
  constrained
  "[if everyone can sell their spare GPU capacity for Bitcoin](https://x.com/OpenAgentsInc/status/1695244914806722627)".
  The next day it asked people to
  "[sell us your excess GPU capacity for #bitcoin](https://x.com/OpenAgentsInc/status/1695896960983498880)".
- **Main milestones.**
  - 2023-09-12: the GPUtopia beta
    [pays bitcoin for spare GPU compute](https://x.com/OpenAgentsInc/status/1701628445648736626)
    from a browser tab over WebGPU.
  - 2023-09-17: beta v1 closes with
    [560 users and about 150,000 Lightning payments](https://x.com/OpenAgentsInc/status/1703389072394162321).
  - 2023-09-25: the first buyer interface charges
    [7 sats per inference, 6 of them to the GPU provider](https://x.com/OpenAgentsInc/status/1706321126970802366).
  - 2023-10-03: the code is open-sourced with 1 BTC of bounties to build
    "[one decentralized market anchored to #bitcoin](https://x.com/OpenAgentsInc/status/1709304987560358007)".
  - 2023-12-12: [GPUtopia is folded into OpenAgents](https://x.com/OpenAgentsInc/status/1734694536482529334),
    keeping the swarm compute network.
  - 2024-05-14 to 2024-05-30: the Agent Store pays
    [revenue share in bitcoin](https://x.com/OpenAgentsInc/status/1790500162491523138),
    first daily and then
    [every minute](https://x.com/OpenAgentsInc/status/1796195661752246705).
  - 2024-12-13: [Pylon is introduced](https://x.com/OpenAgentsInc/status/1867458253661114610)
    as desktop node software, after a
    [NIP-90 data vending machine demo](https://x.com/OpenAgentsInc/status/1866695147632889902).
  - 2025-05-14: [GPUtopia 2.0](https://x.com/OpenAgentsInc/status/1922738011621687492)
    reboots the network as OpenAgents Compute, and on 2025-05-24 the
    [Swarm Inference demo](https://x.com/OpenAgentsInc/status/1926403708658544794)
    pays bitcoin for a chat message served over NIP-90.
  - 2026-01-08: [Pylon v0.1.0 and Nexus v0.1.0](https://x.com/OpenAgentsInc/status/2009142870775644644)
    let anyone with an M-series Mac sell spare compute for bitcoin.
  - 2026-03-12: the [OpenAgents compute market launches](https://x.com/OpenAgentsInc/status/2032108547333304421).
  - 2026-04-08 to 2026-04-14: the [Pylon launch](https://x.com/OpenAgentsInc/status/2041970265471480298)
    leads to [825,000 sats paid to Pylons in a week](https://x.com/OpenAgentsInc/status/2043931888910229690).
  - 2026-06-18: [the training run begins](https://x.com/OpenAgentsInc/status/2067700091750879691),
    described as the first AI model training run with compute providers paid
    in bitcoin.

## Contents

- [Timeline](#timeline)
- [The account's names](#the-accounts-names)
- [Posts from @OpenAgentsInc](#posts-from-openagentsinc)
- [Posts from the founder's account](#posts-from-the-founders-account)
- [Episodes in the transcript archive](#episodes-in-the-transcript-archive)
- [Where it lives today](#where-it-lives-today)
- [Sources and method](#sources-and-method)
- [Appendix: every GPUtopia-handle post](#appendix-every-gputopia-handle-post)

## Timeline

| Date | Event |
| --- | --- |
| 2023-08-25 | The account is created under the GPUtopia name and [posts for the first time](https://x.com/OpenAgentsInc/status/1695182400479732005). |
| 2023-08-26 | [First public statement of the idea](https://x.com/OpenAgentsInc/status/1695244914806722627): sell spare GPU capacity for bitcoin. |
| 2023-08-29 | [Preview of the provider dashboard](https://x.com/OpenAgentsInc/status/1696599955010392525). |
| 2023-09-12 | [Beta v1 live](https://x.com/OpenAgentsInc/status/1701628445648736626): load a model in Chrome, take jobs, and get paid through an Alby login. |
| 2023-09-13 | [100 users](https://x.com/OpenAgentsInc/status/1702023590923644939); per-job payments [strain one LNbits instance](https://x.com/OpenAgentsInc/status/1702045220072755289). |
| 2023-09-17 | [Beta v1 wrap-up](https://x.com/OpenAgentsInc/status/1703389072394162321): 560 users, about 150,000 payments. |
| 2023-09-18 | [Beta v2 live](https://x.com/OpenAgentsInc/status/1703879908898775119). |
| 2023-09-22 | [The `workerbee` command-line provider](https://x.com/OpenAgentsInc/status/1705189920812212250) reaches GPUs the browser cannot. |
| 2023-09-25 | [Beta v3 and the first buyer interface](https://x.com/OpenAgentsInc/status/1706321126970802366): 7 sats per inference, 6 to the provider. |
| 2023-10-02 | [Site updated to v4](https://x.com/OpenAgentsInc/status/1708919795406479845). |
| 2023-10-03 | [Open source and 1 BTC of bounties](https://x.com/OpenAgentsInc/status/1709305653469086115); the goals include [revenue-share micropayments to every contributor](https://x.com/OpenAgentsInc/status/1709312066731823501). |
| 2023-10-05 | [Swarm inference](https://x.com/OpenAgentsInc/status/1710055873572110558): one prompt to 8 sellers at once. |
| 2023-10-19 | [Bounty pool raised to 4 BTC](https://x.com/OpenAgentsInc/status/1714817725497757908). |
| 2023-10-21 | [Swarm fine-tune](https://x.com/OpenAgentsInc/status/1715697777970147791) of Mistral on the open GPU network. |
| 2023-11-07 | [The OpenAgents video series begins](https://x.com/OpenAgentsInc/status/1721942435125715086), with "open compute via GPUtopia". |
| 2023-11-09 | [L402 bounty](https://x.com/OpenAgentsInc/status/1722650919500759422): a GPUtopia endpoint answers `402 Payment Required`, then serves after payment. |
| 2023-11-19 | [About 200 GPUs connected](https://x.com/OpenAgentsInc/status/1726278284328226969). |
| 2023-12-08 | By this date the handle has moved: a placeholder account named "GPUtopia is now OpenAgents" holds [@GPUtopia](https://x.com/GPUtopia). See [The account's names](#the-accounts-names). |
| 2023-12-12 | [GPUtopia is now OpenAgents](https://x.com/OpenAgentsInc/status/1734694536482529334). |
| 2023-12-21 | [About 150 `workerbee` nodes on one `queenbee`](https://x.com/OpenAgentsInc/status/1737955145093112201); [customers pay by card and contributors earn bitcoin](https://x.com/OpenAgentsInc/status/1737946716190740950). |
| 2023-12-22 | [Episode 37, Flow of Funds](https://x.com/OpenAgentsInc/status/1738221896234373387): how payments split between contributors. |
| 2024-01-11 | [900+ embedding jobs](https://x.com/OpenAgentsInc/status/1745523847611257231) completed on the GPUtopia network. |
| 2024-04-22 | [9-way Lightning split payments](https://x.com/OpenAgentsInc/status/1782556228184424492) tested. |
| 2024-05-14 | [Agent Store open beta](https://x.com/OpenAgentsInc/status/1790500162491523138) with revenue share paid in bitcoin. |
| 2024-05-30 | [Revenue share paid every minute](https://x.com/OpenAgentsInc/status/1796195661752246705). |
| 2024-08-29 | [Plan to reboot GPUtopia](https://x.com/OpenAgentsInc/status/1829020859341660665) once buyer demand exists; in 2023 sellers outnumbered buyers. |
| 2024-12-11 | [Episode 142, Data Vending Machines](https://x.com/OpenAgentsInc/status/1866695147632889902): NIP-90 inference jobs. |
| 2024-12-13 | [Episode 144 introduces Pylon](https://x.com/OpenAgentsInc/status/1867458253661114610). |
| 2025-05-14 | [Episode 174, GPUtopia 2.0](https://x.com/OpenAgentsInc/status/1922738011621687492): OpenAgents Compute. |
| 2025-05-24 | [Episode 178, Swarm Inference](https://x.com/OpenAgentsInc/status/1926403708658544794): a chat message paid in bitcoin over NIP-90. |
| 2026-01-05 | [Episode 201, Fracking Apple Silicon](https://x.com/OpenAgentsInc/status/2008326849613476335). |
| 2026-01-08 | [Episode 203, Pylon and Nexus](https://x.com/OpenAgentsInc/status/2009142870775644644): v0.1.0 of both. |
| 2026-03-07 | [Episode 213, Agent Markets](https://x.com/OpenAgentsInc/status/2030132739672887561): five markets, compute first. |
| 2026-03-12 | [Episode 214, Compute Market](https://x.com/OpenAgentsInc/status/2032108547333304421). |
| 2026-04-08 | [Episode 221, Pylon Launch](https://x.com/OpenAgentsInc/status/2041970265471480298); [25,000 sats paid on day one](https://x.com/OpenAgentsInc/status/2042127525820686402). |
| 2026-04-14 | [825,000 sats paid to Pylons in a week](https://x.com/OpenAgentsInc/status/2043931888910229690). |
| 2026-05-15 | [Pylon and Nexus v0.2 move to LDK](https://x.com/OpenAgentsInc/status/2055373697079181428) for high-volume micropayments. |
| 2026-06-18 | [Episode 238, The Training Run Begins](https://x.com/OpenAgentsInc/status/2067700091750879691): compute providers paid in bitcoin. |
| 2026-07-31 | [Pylon folded into the IDE](https://x.com/OpenAgentsInc/status/2083270739596029963). |
| 2026-09-15 | [Shared compute to return in Coder](https://x.com/OpenAgentsInc/status/2099944114129186867). |
| 2026-10-02 | [Episode 289, OpenAgents](https://x.com/OpenAgentsInc/status/2105903502060859718): plugins pay contributors through streaming micropayments. |

## The account's names

The X API reports that the account with ID `1695152065255788545`, now
@OpenAgentsInc, was created on 2023-08-25. It launched as GPUtopia, and the
2023 posts below were published under the @GPUtopia handle. Two facts date the
change:

- Links the founder posted in September 2023 resolve to
  `twitter.com/GPUtopia/status/...` addresses of posts that now live at
  [@OpenAgentsInc](https://x.com/OpenAgentsInc/status/1701628445648736626).
- A new account holds the freed [@GPUtopia](https://x.com/GPUtopia) handle
  with the display name "GPUtopia is now OpenAgents". The X API gives its
  creation time as 2023-12-08T09:17:08Z.

The handle therefore changed on or before 2023-12-08. The account announced
the change on 2023-12-12:
"[GPUtopia is now OpenAgents](https://x.com/OpenAgentsInc/status/1734694536482529334)",
saying that the swarm compute network would continue inside OpenAgents and
that the company was renaming itself OpenAgents, Inc. The posts shift from
operating the GPU market to building agents in November 2023, after the
[OpenAgents series](https://x.com/OpenAgentsInc/status/1721942435125715086)
began.

## Posts from @OpenAgentsInc

Each table lists posts about the idea in date order. Summaries are
paraphrases. Engagement is likes, reposts, and views as reported by the X API
on 2026-10-07. Themes: **GPUtopia**, **Pylon**, **marketplace** (supply, demand,
and job routing), **payments** (wallets, payouts, withdrawals), **split**
(revenue share between contributors), **DVM** (NIP-90 data vending machines),
**training**, **bounties**, and **metrics**. A few rows name a narrower
theme, such as **L402** or **Psionic**.

### 2023-08 to 2023-12: GPUtopia

GPUtopia launched as a browser app: a provider signed in with Alby, loaded an
open model over WebGPU in Chrome, and was paid sats per completed job. The
team ran the network through three public betas in September, learning that
per-job Lightning payments overloaded one node and moving to balances with
periodic withdrawals. The `workerbee` daemon added native GPUs, the
`queenbee` coordinator assigned jobs, and a buyer interface and an
OpenAI-compatible API created paid demand. In October the code was
open-sourced with bitcoin bounties, and the network served swarm inference,
fine-tunes, embeddings, and image generation. The recurring problem was
demand: [sellers outnumbered buyers](https://x.com/OpenAgentsInc/status/1706374550785536135).
109 posts are indexed here; the
[appendix](#appendix-every-gputopia-handle-post) links every other post from
the GPUtopia handle.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2023-08-25](https://x.com/OpenAgentsInc/status/1695182400479732005) | First post from the account: "Shall we begin?" | launch | 7 likes, 2 reposts, 673 views |
| [2023-08-26](https://x.com/OpenAgentsInc/status/1695244914806722627) | GPUs are supply constrained, but not for long if everyone can sell spare GPU capacity for bitcoin | GPUtopia | 17 likes, 5 reposts, 83K views |
| [2023-08-27](https://x.com/OpenAgentsInc/status/1695896960983498880) | "Sell us your excess GPU capacity for #bitcoin"; beta next week, Alby login to get paid | GPUtopia, payments | 44 likes, 10 reposts, 91K views |
| [2023-08-29](https://x.com/OpenAgentsInc/status/1696581390353969302) | Pool the "GPU poor" together | GPUtopia | 8 likes, 2 reposts, 849 views |
| [2023-08-29](https://x.com/OpenAgentsInc/status/1696599955010392525) | Preview of the provider dashboard for selling spare GPU capacity for bitcoin; invites in about a week | GPUtopia | 66 likes, 9 reposts, 87K views |
| [2023-09-12](https://x.com/OpenAgentsInc/status/1701626728324747387) | Asks whether people would sell unused GPU compute for universal access to compute | GPUtopia | 76 likes, 24 reposts, 46K views |
| [2023-09-12](https://x.com/OpenAgentsInc/status/1701628445648736626) | Beta v1 live: "pay you #bitcoin for your spare GPU compute"; Chrome and WebGPU, video walkthrough | GPUtopia, launch | 55 likes, 18 reposts, 24K views |
| [2023-09-12](https://x.com/OpenAgentsInc/status/1701637672660353156) | Thread on the GPU arms race and regulatory capture as the reason for open compute | marketplace | 53 likes, 11 reposts, 75K views |
| [2023-09-13](https://x.com/OpenAgentsInc/status/1701982745835004410) | A server daemon for desktops and cloud servers planned by the end of September | GPUtopia | 8 likes, 1 reposts, 1.2K views |
| [2023-09-13](https://x.com/OpenAgentsInc/status/1701983121212592610) | Earnings to rise once the buy side of the market launches | marketplace | 4 likes, 0 reposts, 117 views |
| [2023-09-13](https://x.com/OpenAgentsInc/status/1702023590923644939) | 100 users, 35 connected at once | metrics | 20 likes, 3 reposts, 1.0K views |
| [2023-09-13](https://x.com/OpenAgentsInc/status/1702045220072755289) | 40 connected users strain one LNbits instance with automatic payments; plan balance plus auto-sweep | payments | 20 likes, 1 reposts, 1.6K views |
| [2023-09-14](https://x.com/OpenAgentsInc/status/1702379930841915801) | AI and ML workloads first, then generalize | marketplace | 2 likes, 0 reposts, 12 views |
| [2023-09-14](https://x.com/OpenAgentsInc/status/1702411323630391695) | Moves from per-job instant payouts to a balance that sweeps every few minutes | payments | 8 likes, 0 reposts, 609 views |
| [2023-09-16](https://x.com/OpenAgentsInc/status/1703019258970521810) | Rebuild the beta in Next.js and open-source it with bounties | GPUtopia | 24 likes, 5 reposts, 2.6K views |
| [2023-09-16](https://x.com/OpenAgentsInc/status/1703106840773439967) | Payments still going out to more people than the setup supports | payments | 22 likes, 3 reposts, 3.0K views |
| [2023-09-16](https://x.com/OpenAgentsInc/status/1703143500743680436) | Drops WebLN for now because generating invoices needs user approval; keeps Alby OAuth for Lightning and Nostr | payments | 18 likes, 0 reposts, 1.9K views |
| [2023-09-17](https://x.com/OpenAgentsInc/status/1703389072394162321) | Beta v1 wrap-up: 560 users and about 150K Lightning payments; owed balances to be settled after a node crash | metrics, payments | 43 likes, 3 reposts, 5.3K views |
| [2023-09-17](https://x.com/OpenAgentsInc/status/1703389580039266564) | Will batch withdrawals instead of paying every job | payments | 13 likes, 1 reposts, 1.2K views |
| [2023-09-18](https://x.com/OpenAgentsInc/status/1703879908898775119) | Beta v2 live; next are history, chat, and the buy side | GPUtopia | 78 likes, 21 reposts, 11K views |
| [2023-09-19](https://x.com/OpenAgentsInc/status/1704155611116507386) | Compares providing to early GPU mining: earnings not guaranteed | marketplace | 12 likes, 2 reposts, 1.6K views |
| [2023-09-19](https://x.com/OpenAgentsInc/status/1704200005525938607) | Command-line provider in progress | GPUtopia | 12 likes, 1 reposts, 943 views |
| [2023-09-19](https://x.com/OpenAgentsInc/status/1704212294643622233) | 200 simultaneous users without failure | metrics | 32 likes, 3 reposts, 1.3K views |
| [2023-09-19](https://x.com/OpenAgentsInc/status/1704242250098639178) | Asks what to do with "our new GPU compute swarm" | GPUtopia | 22 likes, 2 reposts, 1.4K views |
| [2023-09-19](https://x.com/OpenAgentsInc/status/1704274718738551212) | Dashboard shows the last 25 payments | payments | 26 likes, 2 reposts, 1.2K views |
| [2023-09-20](https://x.com/OpenAgentsInc/status/1704505362362089905) | Asks for channel liquidity to the two GPUtopia Lightning nodes | payments | 12 likes, 2 reposts, 726 views |
| [2023-09-20](https://x.com/OpenAgentsInc/status/1704586702646399115) | Provider chat live | GPUtopia | 23 likes, 2 reposts, 913 views |
| [2023-09-20](https://x.com/OpenAgentsInc/status/1704618436653981859) | Beta v3 rollout connects the buy side and ramps down placeholder spending | marketplace | 64 likes, 8 reposts, 5.6K views |
| [2023-09-21](https://x.com/OpenAgentsInc/status/1704646977504293179) | Server daemons will need only a Lightning address; moving toward noncustodial | payments | 14 likes, 3 reposts, 1.0K views |
| [2023-09-21](https://x.com/OpenAgentsInc/status/1704648181064089909) | Text inference first, image models later | marketplace | 12 likes, 1 reposts, 721 views |
| [2023-09-21](https://x.com/OpenAgentsInc/status/1704670131769540719) | Withdrawals paused for a security fix after a tester exploited a withdrawal bug; v3 on track | payments | 23 likes, 0 reposts, 1.6K views |
| [2023-09-21](https://x.com/OpenAgentsInc/status/1704871602763030797) | Script pays all outstanding balances | payments | 43 likes, 2 reposts, 4.2K views |
| [2023-09-21](https://x.com/OpenAgentsInc/status/1704881646510477634) | Paid about 330 providers from v2 | payments | 17 likes, 2 reposts, 1.3K views |
| [2023-09-22](https://x.com/OpenAgentsInc/status/1705189920812212250) | "workerbee" command-line provider reaches more GPUs than the web version | GPUtopia | 30 likes, 3 reposts, 4.3K views |
| [2023-09-22](https://x.com/OpenAgentsInc/status/1705341683087315401) | Withdrawals to Alby and payment history restored | payments | 28 likes, 2 reposts, 1.9K views |
| [2023-09-22](https://x.com/OpenAgentsInc/status/1705344655909409247) | "Why pay advertisers when we can pay our users" | payments | 14 likes, 0 reposts, 629 views |
| [2023-09-22](https://x.com/OpenAgentsInc/status/1705368584342372381) | The company seeded the network with a few thousand dollars of sats | payments | 3 likes, 0 reposts, 55 views |
| [2023-09-23](https://x.com/OpenAgentsInc/status/1705382562393399562) | Adds a standard Lightning invoice withdrawal flow | payments | 55 likes, 6 reposts, 4.6K views |
| [2023-09-24](https://x.com/OpenAgentsInc/status/1705735371009491035) | Explains the Vicuna and Llama 2 model options for providers | GPUtopia | 27 likes, 1 reposts, 1.6K views |
| [2023-09-24](https://x.com/OpenAgentsInc/status/1705761697011540324) | Server daemon built on llama.cpp | GPUtopia | 4 likes, 0 reposts, 334 views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706321126970802366) | First buy-side interface: chat with inference paid in sats, 7 sats per inference with 6 to the GPU provider | marketplace, split | 59 likes, 8 reposts, 18K views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706355109322481872) | Beta v3 live | marketplace | 61 likes, 14 reposts, 12K views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706374550785536135) | Stop paying for capacity without buyers; build the buy side | marketplace | 8 likes, 1 reposts, 829 views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706377244883443852) | Plans a referral program: a lifetime percentage of referred buyers' spending | split | 13 likes, 2 reposts, 912 views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706378955568083230) | Jobs go to every connected provider; the first to lock one completes it | marketplace | 16 likes, 1 reposts, 2.6K views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706384858770002167) | Latency to the issuing server decides who wins jobs for now | marketplace | 8 likes, 1 reposts, 1.2K views |
| [2023-09-25](https://x.com/OpenAgentsInc/status/1706419535681581450) | Plans an anonymized buy and sell order book on the dashboard | marketplace | 26 likes, 1 reposts, 1.9K views |
| [2023-09-26](https://x.com/OpenAgentsInc/status/1706686007251214635) | One provider won 75% of recent jobs; fix coming | marketplace | 13 likes, 0 reposts, 1.9K views |
| [2023-09-26](https://x.com/OpenAgentsInc/status/1706698293139354017) | Plans to price slower inference lower; a Mac M2 is 2-3x faster than a slow GPU | marketplace | 9 likes, 0 reposts, 920 views |
| [2023-09-26](https://x.com/OpenAgentsInc/status/1706724558013972818) | Links a long interview on GPUtopia's past, present, and future | GPUtopia | 17 likes, 6 reposts, 1.8K views |
| [2023-09-27](https://x.com/OpenAgentsInc/status/1707014307353948434) | Adds a basic reward for seller availability until buy-side demand grows | payments | 28 likes, 5 reposts, 3.4K views |
| [2023-09-27](https://x.com/OpenAgentsInc/status/1707026171836149862) | Explains v2 paid per demo inference; v3 pays per real buyer inference | payments | 9 likes, 1 reposts, 583 views |
| [2023-09-27](https://x.com/OpenAgentsInc/status/1707033426807669185) | Llama 2 next; first model for the workerbee daemon almost ready | GPUtopia | 6 likes, 1 reposts, 1.0K views |
| [2023-09-27](https://x.com/OpenAgentsInc/status/1707043875905208460) | Fixes a bug that let providers lock jobs while busy | marketplace | 19 likes, 1 reposts, 2.0K views |
| [2023-09-27](https://x.com/OpenAgentsInc/status/1707087392723616127) | Executables for macOS, Windows, and Linux and the CLI planned for v4 | GPUtopia | 20 likes, 4 reposts, 1.4K views |
| [2023-09-27](https://x.com/OpenAgentsInc/status/1707167571441529117) | Wants payments like this spread among sellers as sats | payments | 28 likes, 6 reposts, 7.8K views |
| [2023-09-28](https://x.com/OpenAgentsInc/status/1707416182574199008) | Skips to a gradual v4 rollout; then "crank up the sats-flow" for a stress test | GPUtopia | 57 likes, 11 reposts, 8.8K views |
| [2023-09-28](https://x.com/OpenAgentsInc/status/1707543101554762041) | The "queenbee" scheduler will assign each job to one provider instead of a race | marketplace | 1 likes, 0 reposts, 117 views |
| [2023-09-29](https://x.com/OpenAgentsInc/status/1707781651357376738) | Community Discord to organize bounty work | GPUtopia | 41 likes, 4 reposts, 5.4K views |
| [2023-09-30](https://x.com/OpenAgentsInc/status/1708189982089711926) | Llama 2 70B through the v4 workerbee on the company A100 | GPUtopia | 21 likes, 3 reposts, 1.6K views |
| [2023-10-01](https://x.com/OpenAgentsInc/status/1708549108972056635) | First FAQ | GPUtopia | 10 likes, 2 reposts, 1.3K views |
| [2023-10-02](https://x.com/OpenAgentsInc/status/1708919795406479845) | Site updated to v4; open-sourcing and bounties next | GPUtopia | 43 likes, 7 reposts, 7.1K views |
| [2023-10-03](https://x.com/OpenAgentsInc/status/1709300332453371930) | Announces the first bitcoin bounties "to build a decentralized network for truly open AI" | bounties | 109 likes, 33 reposts, 44K views |
| [2023-10-03](https://x.com/OpenAgentsInc/status/1709304545556197731) | Thread: GPUtopia exists to counter closed AI | GPUtopia | 25 likes, 7 reposts, 42K views |
| [2023-10-03](https://x.com/OpenAgentsInc/status/1709304987560358007) | "connect all the world's GPU compute into one decentralized market anchored to #bitcoin" | marketplace | 13 likes, 1 reposts, 605 views |
| [2023-10-03](https://x.com/OpenAgentsInc/status/1709305653469086115) | Open-sources the codebase and launches 1 BTC of bounties | bounties | 13 likes, 2 reposts, 921 views |
| [2023-10-03](https://x.com/OpenAgentsInc/status/1709311711189111020) | Goal: 150 GPU sellers earning at least $500 a month from buyers within 30 days | marketplace | 10 likes, 3 reposts, 682 views |
| [2023-10-03](https://x.com/OpenAgentsInc/status/1709312066731823501) | Lists goals: sell unused GPU capacity for bitcoin, buy compute on an open market, and pay "revenue-share micropayments proportional to their contribution" to each contributor in a workflow | marketplace, split | 11 likes, 3 reposts, 650 views |
| [2023-10-05](https://x.com/OpenAgentsInc/status/1709886107964342735) | Bounties focus on the buyer UI, then new uses of the swarm architecture | bounties | 23 likes, 4 reposts, 3.8K views |
| [2023-10-05](https://x.com/OpenAgentsInc/status/1710055873572110558) | "Swarm inference": one prompt sent to 8 GPUtopia sellers at once | GPUtopia | 21 likes, 4 reposts, 2.5K views |
| [2023-10-06](https://x.com/OpenAgentsInc/status/1710098255684468973) | Workers could log requests; privacy to improve over time | GPUtopia | 3 likes, 0 reposts, 71 views |
| [2023-10-06](https://x.com/OpenAgentsInc/status/1710341882176327775) | "Cloning OpenAI" video series tied to "our open market of GPU compute" | marketplace | 66 likes, 13 reposts, 8.5K views |
| [2023-10-06](https://x.com/OpenAgentsInc/status/1710437847621312767) | Episode 4: GPUtopia API key for chat completions | marketplace | 11 likes, 2 reposts, 3.5K views |
| [2023-10-09](https://x.com/OpenAgentsInc/status/1711179050826510573) | Episode 5: unveils ChatGPU | marketplace | 19 likes, 7 reposts, 2.7K views |
| [2023-10-10](https://x.com/OpenAgentsInc/status/1711884575960633691) | First bounty, 5M sats, awarded for a DALL-E UI clone | bounties | 28 likes, 6 reposts, 7.6K views |
| [2023-10-11](https://x.com/OpenAgentsInc/status/1712135942662926610) | "Using GPUtopia to develop GPUtopia"; agents soon | GPUtopia | 29 likes, 6 reposts, 1.6K views |
| [2023-10-14](https://x.com/OpenAgentsInc/status/1713284051300229467) | Bounty #2 awarded for the Alby and next-auth refactor | bounties | 31 likes, 3 reposts, 6.9K views |
| [2023-10-16](https://x.com/OpenAgentsInc/status/1713974112513630351) | Web UI back up for buyers (chat and API) and sellers (WebGPU and workers) | marketplace | 13 likes, 3 reposts, 870 views |
| [2023-10-16](https://x.com/OpenAgentsInc/status/1713990270872641927) | Two OpenAI-compatible backends: local, or paid swarm compute | marketplace | 10 likes, 3 reposts, 1.8K views |
| [2023-10-18](https://x.com/OpenAgentsInc/status/1714790097827119354) | Selling GPU means going online and getting paid per job; best GPUs win most jobs | marketplace | 0 likes, 0 reposts, 31 views |
| [2023-10-19](https://x.com/OpenAgentsInc/status/1714816527227146342) | Episode 10: bounty pool raised to 4 BTC across 20 categories | bounties | 110 likes, 14 reposts, 500K views |
| [2023-10-19](https://x.com/OpenAgentsInc/status/1714817725497757908) | Bounty pool raised to 4 BTC to accelerate open AI | bounties | 60 likes, 22 reposts, 16K views |
| [2023-10-21](https://x.com/OpenAgentsInc/status/1715520984587931994) | Episode 13: first fine-tune job created through the queenbee with a GPUtopia key | GPUtopia | 9 likes, 1 reposts, 698 views |
| [2023-10-21](https://x.com/OpenAgentsInc/status/1715697777970147791) | Episode 14: "World's First Swarm Fine-tune" of Mistral on the open GPU network | GPUtopia | 14 likes, 3 reposts, 12K views |
| [2023-10-24](https://x.com/OpenAgentsInc/status/1716625129898115231) | Episode 15: billing dashboard, adding card payments alongside bitcoin | payments | 7 likes, 2 reposts, 593 views |
| [2023-10-26](https://x.com/OpenAgentsInc/status/1717354497750556994) | Bounty #3 awarded for a Nostr integration spec and proof of concept in queenbee and workerbee | Nostr | 23 likes, 6 reposts, 3.6K views |
| [2023-10-26](https://x.com/OpenAgentsInc/status/1717645064652898663) | "BUILD YOUR AI ARMY": train agents and deploy them to the GPU swarm | agents | 57 likes, 8 reposts, 202K views |
| [2023-10-27](https://x.com/OpenAgentsInc/status/1717700312151056680) | workerbee v0.2 supports fine-tuning | GPUtopia | 11 likes, 4 reposts, 819 views |
| [2023-10-29](https://x.com/OpenAgentsInc/status/1718766857388204090) | Atlantis Rising episode 2: recap of GPUtopia goals, GPU monetization tools for web and desktop, drop-in API | GPUtopia | 13 likes, 5 reposts, 794 views |
| [2023-10-31](https://x.com/OpenAgentsInc/status/1719468093947232330) | "You pay for it, you can run it" fine-tune limits | marketplace | 11 likes, 3 reposts, 2.1K views |
| [2023-11-01](https://x.com/OpenAgentsInc/status/1719560255166681579) | Imagines workerbee switching between AI jobs and bitcoin mining | GPUtopia | 13 likes, 3 reposts, 1.1K views |
| [2023-11-01](https://x.com/OpenAgentsInc/status/1719799068933702101) | Third server type, "databee", for data ingestion | GPUtopia | 14 likes, 8 reposts, 2.4K views |
| [2023-11-02](https://x.com/OpenAgentsInc/status/1720173304302985292) | Bounty #4 awarded for a fine-tuning dataset algorithm | bounties | 11 likes, 4 reposts, 2.0K views |
| [2023-11-04](https://x.com/OpenAgentsInc/status/1720898116201611582) | "a decently large GPU cloud" of live workers needs buy-side demand; agents and fine-tuning UI | marketplace | 0 likes, 0 reposts, 46 views |
| [2023-11-07](https://x.com/OpenAgentsInc/status/1721942435125715086) | "OPEN AGENTS" video series: open platform for agents with "Open compute via GPUtopia" and open money | agents, GPUtopia | 168 likes, 36 reposts, 594K views |
| [2023-11-09](https://x.com/OpenAgentsInc/status/1722615631634436361) | Withdrawal bug fixed on the GPUtopia site | payments | 10 likes, 3 reposts, 689 views |
| [2023-11-09](https://x.com/OpenAgentsInc/status/1722650919500759422) | Bounty #6: L402 demo hitting a GPUtopia endpoint, 402 Payment Required then paid | payments, L402 | 11 likes, 4 reposts, 4.5K views |
| [2023-11-09](https://x.com/OpenAgentsInc/status/1722658326259920903) | Bounty #7: Stable Diffusion support in workerbee | GPUtopia | 7 likes, 3 reposts, 7.9K views |
| [2023-11-09](https://x.com/OpenAgentsInc/status/1722683574837481693) | Bounty #8: GPUtopia inference through the PlebAI agent on Nostr (kind-1 message) | Nostr | 14 likes, 5 reposts, 7.6K views |
| [2023-11-10](https://x.com/OpenAgentsInc/status/1722770704913854777) | "Unknown large GPU cluster checking in"; building profit pools | marketplace | 12 likes, 3 reposts, 1.4K views |
| [2023-11-10](https://x.com/OpenAgentsInc/status/1723069060768793054) | Stripe billing works in demo mode alongside bitcoin | payments | 5 likes, 0 reposts, 441 views |
| [2023-11-16](https://x.com/OpenAgentsInc/status/1725197866409267544) | Atlantis Rising episode 17: embeddings generated through queenbee | GPUtopia | 7 likes, 3 reposts, 841 views |
| [2023-11-17](https://x.com/OpenAgentsInc/status/1725349984952827929) | Episode 19: chat with PDF, GPUtopia edition, no third-party services | GPUtopia | 8 likes, 3 reposts, 2.2K views |
| [2023-11-18](https://x.com/OpenAgentsInc/status/1725727843311579231) | Pitches the OpenAI-compatible API on "an open GPU network and open models" | marketplace | 15 likes, 4 reposts, 1.2K views |
| [2023-11-18](https://x.com/OpenAgentsInc/status/1725730258454741503) | Accelerates the OpenAgents app with the parent company, Arcade Labs | agents | 12 likes, 5 reposts, 1.2K views |
| [2023-11-19](https://x.com/OpenAgentsInc/status/1726278284328226969) | About 200 GPUs connected between web and workerbee, "one of the largest decentralized clusters" | metrics | 18 likes, 5 reposts, 1.3K views |
| [2023-11-19](https://x.com/OpenAgentsInc/status/1726288817324503204) | Jobs go to the best GPUs; low-end systems may get none | marketplace | 0 likes, 0 reposts, 36 views |
| [2023-11-26](https://x.com/OpenAgentsInc/status/1728828127781249075) | Contrasts with rental marketplaces: focused on bundled services like an OpenAI-compatible API | marketplace | 4 likes, 0 reposts, 63 views |
| [2023-11-27](https://x.com/OpenAgentsInc/status/1729155086444527908) | Bounty #9: end-to-end image generation through workerbee | bounties | 14 likes, 4 reposts, 1.6K views |

### 2023-12 to 2024-04: the network inside OpenAgents

After the rename, OpenAgents kept the GPUtopia network as its compute
backend. The posts describe a flow of funds in which customers pay by card or
Lightning and contributors, including compute sellers, earn bitcoin. Agents
gained bitcoin balances, L402 payments, and Lightning withdrawals.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2023-12-12](https://x.com/OpenAgentsInc/status/1734694536482529334) | "GPUtopia is now OpenAgents": the swarm compute network folds into the OpenAgents platform | rename | 59 likes, 21 reposts, 22K views |
| [2023-12-21](https://x.com/OpenAgentsInc/status/1737942649544335811) | Year-end statement on competing with closed AI on agents | agents | 284 likes, 25 reposts, 473K views |
| [2023-12-21](https://x.com/OpenAgentsInc/status/1737946716190740950) | Customers pay by card; contributors earn bitcoin; open compute network formerly GPUtopia | payments, split | 14 likes, 4 reposts, 5.4K views |
| [2023-12-21](https://x.com/OpenAgentsInc/status/1737951685270491262) | Top-ups with BTC, Lightning, or card; L402 integration planned | payments, L402 | 7 likes, 2 reposts, 582 views |
| [2023-12-21](https://x.com/OpenAgentsInc/status/1737955145093112201) | About 150 workerbees connected to one central queenbee; anyone can run a node | metrics | 4 likes, 2 reposts, 259 views |
| [2023-12-22](https://x.com/OpenAgentsInc/status/1738221896234373387) | Episode 37: "Flow of Funds", how payments will work; "The sats must flow" | payments, split | 6 likes, 2 reposts, 922 views |
| [2024-01-04](https://x.com/OpenAgentsInc/status/1742952006166225330) | Episode 42: agents hold a bitcoin balance | payments | 7 likes, 2 reposts, 1.3K views |
| [2024-01-11](https://x.com/OpenAgentsInc/status/1745521898824356193) | Episode 46: PDF embeddings processed by GPUtopia workerbees | GPUtopia | 4 likes, 2 reposts, 2.0K views |
| [2024-01-11](https://x.com/OpenAgentsInc/status/1745523847611257231) | 900+ embedding jobs completed on the GPUtopia open compute network | GPUtopia, metrics | 13 likes, 3 reposts, 1.2K views |
| [2024-01-18](https://x.com/OpenAgentsInc/status/1747994309549318228) | Episode 54: plugin authors get "a stream of residual revenue" per use | split | 4 likes, 2 reposts, 1.7K views |
| [2024-04-01](https://x.com/OpenAgentsInc/status/1774858563111780556) | Short-term plan: OpenAgents sends volume to the existing GPUtopia network | GPUtopia | 5 likes, 2 reposts, 547 views |
| [2024-04-09](https://x.com/OpenAgentsInc/status/1777693491008577695) | Pro plan purchasable with $10 of sats over Lightning | payments | 5 likes, 1 reposts, 1.1K views |
| [2024-04-10](https://x.com/OpenAgentsInc/status/1778128133427757275) | First Pro plan sold for bitcoin | payments | 7 likes, 1 reposts, 799 views |
| [2024-04-20](https://x.com/OpenAgentsInc/status/1781703101327757410) | Users can add a Lightning address in settings so the platform can pay them | payments | 15 likes, 5 reposts, 12K views |
| [2024-04-22](https://x.com/OpenAgentsInc/status/1782556228184424492) | Tests 9-way Lightning split payments through Prism and Nostr Wallet Connect | split | 21 likes, 9 reposts, 10K views |
| [2024-04-23](https://x.com/OpenAgentsInc/status/1782836821883330599) | A 12-way split payment: 10 of 12 succeed; explorer for prism payments | split | 7 likes, 2 reposts, 938 views |
| [2024-04-29](https://x.com/OpenAgentsInc/status/1785045575123005713) | "THE OPEN AGENTS PROTOCOL": eight repos, first pool and node cluster powering v1 of the Agent Store | marketplace | 34 likes, 11 reposts, 15K views |

### 2024-04 to 2024-11: the Agent Store and Lightning revenue share

The Agent Store paid agent builders in bitcoin, using Lightning split payments
to many addresses at once. By August the account said it hoped to
[reboot the original GPUtopia idea](https://x.com/OpenAgentsInc/status/1829020859341660665)
once agents supplied buyer demand, and it planned a NIP-90 relay and service
provider.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2024-05-07](https://x.com/OpenAgentsInc/status/1787878776187076899) | Machine-to-machine payments over Lightning, communication over Nostr | payments | 14 likes, 5 reposts, 3.9K views |
| [2024-05-08](https://x.com/OpenAgentsInc/status/1788303479020138508) | Payouts for agent usage start through Lightning addresses | payments | 3 likes, 2 reposts, 588 views |
| [2024-05-14](https://x.com/OpenAgentsInc/status/1790500162491523138) | Episode 92: Agent Store open beta, "AI agent marketplace with revenue sharing paid daily in bitcoin" | marketplace, split | 53 likes, 23 reposts, 27K views |
| [2024-05-15](https://x.com/OpenAgentsInc/status/1790805640392122627) | Episode 93 "The Sats Must Flow": first payout of 100,000 sats across four agent builders | split | 9 likes, 4 reposts, 5.7K views |
| [2024-05-16](https://x.com/OpenAgentsInc/status/1791120423381274800) | Daily payouts to agent builders versus a store with no monetization | split | 6 likes, 3 reposts, 657 views |
| [2024-05-22](https://x.com/OpenAgentsInc/status/1793146221504451013) | Payouts week 1 complete | split | 5 likes, 3 reposts, 637 views |
| [2024-05-28](https://x.com/OpenAgentsInc/status/1795535732032831719) | Episode 96: Lightning wallet for all users | payments | 8 likes, 3 reposts, 4.0K views |
| [2024-05-29](https://x.com/OpenAgentsInc/status/1795879532525695228) | Episode 97: agent chats cost sats; store shows sats each agent earned | payments | 6 likes, 2 reposts, 904 views |
| [2024-05-30](https://x.com/OpenAgentsInc/status/1796195661752246705) | Episode 98: revenue share paid to agent builders every minute | split | 19 likes, 8 reposts, 5.5K views |
| [2024-05-30](https://x.com/OpenAgentsInc/status/1796251292798464265) | Builders set a sats-per-message price up to 3000 sats | split | 14 likes, 8 reposts, 4.8K views |
| [2024-06-03](https://x.com/OpenAgentsInc/status/1797738481097077001) | Episode 99: bitcoin deposits and username@openagents.com Lightning addresses | payments | 5 likes, 2 reposts, 928 views |
| [2024-06-12](https://x.com/OpenAgentsInc/status/1800709528696013110) | Agents hold sats balances, swept to owners every minute; agent-to-agent payments next | payments | 3 likes, 2 reposts, 367 views |
| [2024-06-14](https://x.com/OpenAgentsInc/status/1801653533810319867) | AI service marketplaces on Nostr "happening already with NIP90, L402" | DVM | 7 likes, 3 reposts, 754 views |
| [2024-08-29](https://x.com/OpenAgentsInc/status/1829020859341660665) | "We hope to reboot our original gputopia idea" once buy-side demand exists; sellers outnumbered buyers | GPUtopia | 5 likes, 3 reposts, 759 views |
| [2024-08-30](https://x.com/OpenAgentsInc/status/1829632437573259433) | Plan to implement NIP-01 and NIP-90 from scratch; first NIP-90 service is Whisper transcription | DVM | 8 likes, 2 reposts, 7.7K views |
| [2024-09-18](https://x.com/OpenAgentsInc/status/1836434952763691317) | v3 plan includes paying and earning bitcoin for chat usage and shared knowledge | payments | 10 likes, 3 reposts, 2.1K views |
| [2024-09-29](https://x.com/OpenAgentsInc/status/1840462194519179464) | "May need to reboot our GPUtopia network sooner than later" | GPUtopia | 3 likes, 2 reposts, 195 views |
| [2024-11-12](https://x.com/OpenAgentsInc/status/1856167182943633486) | Plans to reboot the agent marketplace with bitcoin revenue share in v3 | split | 7 likes, 2 reposts, 616 views |

### 2024-12 to 2025-04: Onyx, data vending machines, and the first Pylon

The Onyx mobile app combined a Lightning wallet with NIP-90 data vending
machines. Pylon appeared as desktop node software that serves models to the
phone and is meant to earn bitcoin for running them.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2024-12-05](https://x.com/OpenAgentsInc/status/1864528026765062439) | Onyx mobile app: Breez SDK wallet and Nostr identity from one seed | payments | 18 likes, 7 reposts, 2.7K views |
| [2024-12-10](https://x.com/OpenAgentsInc/status/1866351898376405220) | Episode 141 "One Market": one decentralized global marketplace of AI agents and services | marketplace | 11 likes, 6 reposts, 1.8K views |
| [2024-12-11](https://x.com/OpenAgentsInc/status/1866695147632889902) | Episode 142: NIP-90 data vending machines in Onyx; inference handled by a separate DVM service | DVM | 11 likes, 4 reposts, 2.3K views |
| [2024-12-12](https://x.com/OpenAgentsInc/status/1867035550198403206) | "DVMs on Nostr fixes this" during a centralized provider outage | DVM | 4 likes, 1 reposts, 360 views |
| [2024-12-12](https://x.com/OpenAgentsInc/status/1867070611928846640) | Episode 143: self-custodial Lightning wallet in Onyx | payments | 24 likes, 7 reposts, 5.1K views |
| [2024-12-13](https://x.com/OpenAgentsInc/status/1867458253661114610) | Episode 144: introduces Pylon, desktop node software anyone can run, powering Onyx | Pylon | 9 likes, 4 reposts, 1.7K views |
| [2024-12-14](https://x.com/OpenAgentsInc/status/1867815868131836103) | Episode 145: phone connects through MCP to a local Pylon node to run Llama 3.3 70B | Pylon | 17 likes, 5 reposts, 2.8K views |
| [2024-12-15](https://x.com/OpenAgentsInc/status/1868366478258602080) | Building through Pylon and MCP, then NIP-90 | Pylon, DVM | 3 likes, 1 reposts, 225 views |
| [2024-12-15](https://x.com/OpenAgentsInc/status/1868412539463467023) | On-device model calls tools through a desktop Pylon MCP server with no inference cost | Pylon | 13 likes, 2 reposts, 723 views |
| [2024-12-18](https://x.com/OpenAgentsInc/status/1869220404079800704) | Episode 147: data marketplace as part of the one market | marketplace | 9 likes, 4 reposts, 2.1K views |
| [2024-12-20](https://x.com/OpenAgentsInc/status/1870186842152165835) | "LOCAL AI = INFINITE TEST-TIME COMPUTE": edge compute for open agents | Pylon | 5 likes, 2 reposts, 373 views |
| [2024-12-22](https://x.com/OpenAgentsInc/status/1870630156987195476) | Two services named Pylon and Nexus | Pylon | 3 likes, 2 reposts, 426 views |
| [2024-12-29](https://x.com/OpenAgentsInc/status/1873410901396562165) | Flow of funds doc; streaming micropayments experiments with the wallet release | payments | 5 likes, 2 reposts, 333 views |
| [2025-01-03](https://x.com/OpenAgentsInc/status/1875239229510300013) | Onyx v0.1.0 adds a Lightning wallet on the Breez SDK | payments | 19 likes, 8 reposts, 2.8K views |
| [2025-04-09](https://x.com/OpenAgentsInc/status/1909773588674019808) | MCP server authors could earn revenue share per use through micropayments | split | 16 likes, 5 reposts, 982 views |
| [2025-04-16](https://x.com/OpenAgentsInc/status/1912306694274589005) | Plans to reboot the agent store with Lightning revenue share | split | 4 likes, 2 reposts, 358 views |

### 2025-05 to 2025-12: OpenAgents Compute and swarm inference

Episode 174 renamed the network OpenAgents Compute and restated the goal:
anyone can sell spare compute for bitcoin. The Commander desktop app carried
the first paid NIP-90 job. The account then put the network second to its
coding agent and planned to return to it.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2025-05-05](https://x.com/OpenAgentsInc/status/1919419077887410389) | Episode 169: Agent Payments API for agents to send and receive bitcoin | payments | 8 likes, 5 reposts, 1.9K views |
| [2025-05-07](https://x.com/OpenAgentsInc/status/1920222323963277553) | Episode 171: agent-to-agent bitcoin payment between Spark wallets in Commander | payments | 13 likes, 3 reposts, 1.3K views |
| [2025-05-11](https://x.com/OpenAgentsInc/status/1921369849860645255) | New Discord; recalls the GPUtopia Discord two years earlier | GPUtopia | 7 likes, 2 reposts, 671 views |
| [2025-05-13](https://x.com/OpenAgentsInc/status/1922303008617984363) | Episode 173: open-source self-custodial bitcoin wallet for humans and agents | payments | 14 likes, 5 reposts, 2.2K views |
| [2025-05-14](https://x.com/OpenAgentsInc/status/1922738011621687492) | Episode 174 "GPUtopia 2.0": reboots the swarm compute network as OpenAgents Compute so anyone can sell spare compute for bitcoin | GPUtopia, marketplace | 22 likes, 8 reposts, 3.2K views |
| [2025-05-20](https://x.com/OpenAgentsInc/status/1924881394351722940) | Compute network to provide fallback to other providers during a closed API outage | marketplace | 4 likes, 2 reposts, 371 views |
| [2025-05-23](https://x.com/OpenAgentsInc/status/1926029384340640153) | First message over the swarm compute network on Nostr NIP-90; Commander v0.0.4 runs local models | DVM | 9 likes, 2 reposts, 616 views |
| [2025-05-24](https://x.com/OpenAgentsInc/status/1926403708658544794) | Episode 178 "Swarm Inference": pays bitcoin for a chat message served by the compute network; "sell your compute for bitcoin doing nothing but clicking this button" | marketplace, DVM | 25 likes, 7 reposts, 4.0K views |
| [2025-05-25](https://x.com/OpenAgentsInc/status/1926607187977080974) | Sell side "caught up to where we left off with GPUtopia 18 months ago"; buy side will be coding agents | GPUtopia | 8 likes, 3 reposts, 2.1K views |
| [2025-06-12](https://x.com/OpenAgentsInc/status/1933238226397114615) | "P2P compute networks sounding even better" during a cloud outage | marketplace | 7 likes, 4 reposts, 352 views |
| [2025-08-02](https://x.com/OpenAgentsInc/status/1951451144561238240) | Will reboot the "BTC-for-compute network" after the coding agent product | marketplace | 0 likes, 0 reposts, 50 views |
| [2025-10-31](https://x.com/OpenAgentsInc/status/1984334754859164029) | Local inference plus a marketplace to monetize spare compute, buyers paying sats for inference | marketplace | 7 likes, 2 reposts, 394 views |
| [2025-11-03](https://x.com/OpenAgentsInc/status/1985373843893170556) | Bridge to connect local models to an opt-in swarm inference network | marketplace | 8 likes, 2 reposts, 509 views |
| [2025-11-08](https://x.com/OpenAgentsInc/status/1987058004617781672) | NIP-90 data vending machine built in minutes with Codex | DVM | 8 likes, 3 reposts, 741 views |
| [2025-11-20](https://x.com/OpenAgentsInc/status/1991571302466167233) | Quotes a reference to "the OpenAgents compute marketplace" | marketplace | 4 likes, 3 reposts, 214 views |
| [2025-11-26](https://x.com/OpenAgentsInc/status/1993728286191677479) | "You must construct additional pylons" | Pylon | 9 likes, 4 reposts, 475 views |

### 2026-01 to 2026-06: Pylon, the compute market, and paid training

Pylon v0.1.0 targeted the idle Apple silicon in millions of Macs. The compute
market launched in March inside Autopilot, the Pylon launch in April reached
dozens of online nodes and hundreds of thousands of sats paid, and Pylon and
Nexus moved to LDK for micropayments. Pylons then supplied a distributed
training run whose providers were paid in bitcoin. Psionic, the Rust machine
learning stack, ran the workloads and planned trusted execution environment
support for verifiable compute.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2026-01-02](https://x.com/OpenAgentsInc/status/2006956979298685216) | Episode 200: six themes for 2026 including local and swarm AI and agent networks | marketplace | 28 likes, 8 reposts, 12K views |
| [2026-01-03](https://x.com/OpenAgentsInc/status/2007253876488196332) | "a few million idle Macs" of untapped supply to lower costs | Pylon | 3 likes, 2 reposts, 724 views |
| [2026-01-05](https://x.com/OpenAgentsInc/status/2008326849613476335) | Episode 201 "Fracking Apple Silicon": connect millions of Apple silicon chips into a network for agentic compute | Pylon | 15 likes, 10 reposts, 6.2K views |
| [2026-01-06](https://x.com/OpenAgentsInc/status/2008382678429553029) | First version of the "sell-compute-for-bitcoin software Pylon" launches January 7 | Pylon | 2 likes, 1 reposts, 207 views |
| [2026-01-07](https://x.com/OpenAgentsInc/status/2008704591110541567) | Episode 202: recursive language models and the swarm compute network of edge devices | Pylon | 12 likes, 8 reposts, 3.3K views |
| [2026-01-08](https://x.com/OpenAgentsInc/status/2009142870775644644) | Episode 203: Pylon (client) and Nexus (server) v0.1.0; anyone with an M-series Mac can convert spare compute to bitcoin | Pylon | 11 likes, 6 reposts, 2.8K views |
| [2026-01-08](https://x.com/OpenAgentsInc/status/2009144207181836426) | Pylon v0.1.0 and Nexus v0.1.0 release notes | Pylon | 3 likes, 1 reposts, 192 views |
| [2026-01-08](https://x.com/OpenAgentsInc/status/2009328381050212393) | Thanks the operators of the first 7 Pylons | Pylon, metrics | 10 likes, 4 reposts, 567 views |
| [2026-01-09](https://x.com/OpenAgentsInc/status/2009728520130363812) | Agents paying humans: micropayments streamed to the wallets of software authors | split | 4 likes, 1 reposts, 783 views |
| [2026-01-26](https://x.com/OpenAgentsInc/status/2015906902517903614) | Autopilot bundles local-first compute, a skills marketplace, and a bitcoin wallet for revenue share | marketplace | 9 likes, 4 reposts, 490 views |
| [2026-01-28](https://x.com/OpenAgentsInc/status/2016423268564001059) | Episode 207: seed-phrase keys for payments and identity | payments | 7 likes, 4 reposts, 2.1K views |
| [2026-02-18](https://x.com/OpenAgentsInc/status/2024259092810703136) | Episode 212: Autopilot gets Lightning send, receive, and L402 | payments | 24 likes, 10 reposts, 2.7K views |
| [2026-02-23](https://x.com/OpenAgentsInc/status/2025940608871772661) | Owners of idle Mac minis could rent compute for bitcoin instead of selling the machine | Pylon | 7 likes, 3 reposts, 1.3K views |
| [2026-02-24](https://x.com/OpenAgentsInc/status/2026298355987410959) | Relaunching next month: "sell spare compute for bitcoin", with Autopilot as constant buy-side demand missing in 2023 | marketplace | 6 likes, 2 reposts, 404 views |
| [2026-02-24](https://x.com/OpenAgentsInc/status/2026352309270167924) | Draft spec for Hydra, a Lightning liquidity engine | payments | 7 likes, 5 reposts, 422 views |
| [2026-03-06](https://x.com/OpenAgentsInc/status/2029741510183239753) | GPU compute "the largest commodity in the world" needs a market; launch March 11 teased | marketplace | 8 likes, 3 reposts, 504 views |
| [2026-03-07](https://x.com/OpenAgentsInc/status/2030132739672887561) | Episode 213 "Agent Markets": five markets launching weekly from March 11, compute first | marketplace | 33 likes, 12 reposts, 183K views |
| [2026-03-10](https://x.com/OpenAgentsInc/status/2031255043903549942) | Psionic Rust inference engine benchmarked against llama.cpp | Psionic | 10 likes, 2 reposts, 563 views |
| [2026-03-12](https://x.com/OpenAgentsInc/status/2032108547333304421) | Episode 214 "Compute Market": anyone can sell spare compute for bitcoin through Autopilot v0.1 | marketplace | 35 likes, 11 reposts, 261K views |
| [2026-03-13](https://x.com/OpenAgentsInc/status/2032328895916126317) | Autopilot v0.1.1 adds GPT-OSS 20B on NVIDIA; a few thousand sats sent to providers | payments | 13 likes, 3 reposts, 1.6K views |
| [2026-03-13](https://x.com/OpenAgentsInc/status/2032524880306962765) | Providers drained the buyer budget; 10K more sats added | payments | 5 likes, 1 reposts, 265 views |
| [2026-03-13](https://x.com/OpenAgentsInc/status/2032530876232294686) | "Come trade your compute for bitcoin" | marketplace | 5 likes, 2 reposts, 380 views |
| [2026-03-25](https://x.com/OpenAgentsInc/status/2036908227019809259) | Episode 216: introduces Psionic, a Rust ML stack | Psionic | 38 likes, 7 reposts, 7.0K views |
| [2026-03-28](https://x.com/OpenAgentsInc/status/2037717730707542232) | Episode 217: Psionic Qwen 3.5 faster than Ollama on one 4080 | Psionic | 19 likes, 6 reposts, 3.5K views |
| [2026-03-28](https://x.com/OpenAgentsInc/status/2037997582115307618) | Add compute to a decentralized training run and get paid bitcoin | training | 7 likes, 2 reposts, 556 views |
| [2026-03-28](https://x.com/OpenAgentsInc/status/2038014648314732698) | "pay them bitcoin for their spare compute" | marketplace | 5 likes, 2 reposts, 469 views |
| [2026-03-30](https://x.com/OpenAgentsInc/status/2038637496922161250) | Stranded compute (20 GW) to join a decentralized training network in April | training | 11 likes, 4 reposts, 1.7K views |
| [2026-03-30](https://x.com/OpenAgentsInc/status/2038638171169128775) | "paying your users bitcoin for their compute" | marketplace | 4 likes, 2 reposts, 268 views |
| [2026-03-30](https://x.com/OpenAgentsInc/status/2038720128779694085) | Providers paid over Lightning; protocol extensible to other buyers | payments | 6 likes, 2 reposts, 211 views |
| [2026-03-31](https://x.com/OpenAgentsInc/status/2038780815430283674) | First draft of NIP-TRN for coordinating AI model training over Nostr, implemented in Psionic | training | 6 likes, 3 reposts, 492 views |
| [2026-04-02](https://x.com/OpenAgentsInc/status/2039724441178112218) | Answers a claim that AI compute will have its own currency with "Bitcoin" | payments | 7 likes, 3 reposts, 278 views |
| [2026-04-02](https://x.com/OpenAgentsInc/status/2039810630832795874) | Beta compute market build distributed a few thousand sats; combining with Psionic | payments | 11 likes, 5 reposts, 1.4K views |
| [2026-04-02](https://x.com/OpenAgentsInc/status/2039831062059388993) | Splitting one inference call across several providers so devices can jointly serve a large model | marketplace | 2 likes, 0 reposts, 16 views |
| [2026-04-03](https://x.com/OpenAgentsInc/status/2040059166061170741) | "FRACK" stranded consumer compute into one market, a decentralized datacenter | marketplace | 13 likes, 4 reposts, 2.9K views |
| [2026-04-04](https://x.com/OpenAgentsInc/status/2040485599119863907) | A permissionless global compute market aggregating spare and stranded compute | marketplace | 4 likes, 2 reposts, 413 views |
| [2026-04-04](https://x.com/OpenAgentsInc/status/2040530393066357098) | The moat: contributors of compute, data, labor, liquidity, and risk get paid the most bitcoin | marketplace | 9 likes, 5 reposts, 1.7K views |
| [2026-04-06](https://x.com/OpenAgentsInc/status/2041172930009108750) | Training spend going to retail compute providers as bitcoin | training | 5 likes, 1 reposts, 418 views |
| [2026-04-07](https://x.com/OpenAgentsInc/status/2041622770811842943) | Website shows a live count of connected Pylons; bitcoin payouts launch next day | Pylon | 6 likes, 3 reposts, 475 views |
| [2026-04-08](https://x.com/OpenAgentsInc/status/2041970265471480298) | Episode 221 "Pylon Launch": "compute miner" node software to sell spare compute for bitcoin in a permissionless market | Pylon | 19 likes, 10 reposts, 4.1K views |
| [2026-04-08](https://x.com/OpenAgentsInc/status/2041988398475248004) | 14 Pylons online, a record | Pylon, metrics | 6 likes, 3 reposts, 338 views |
| [2026-04-08](https://x.com/OpenAgentsInc/status/2041991629662277651) | 18 Pylons online | Pylon, metrics | 4 likes, 3 reposts, 815 views |
| [2026-04-08](https://x.com/OpenAgentsInc/status/2041992244089999796) | Bitcoin payouts every ~20 seconds from Nexus | payments | 6 likes, 3 reposts, 234 views |
| [2026-04-09](https://x.com/OpenAgentsInc/status/2042127525820686402) | 25K sats paid on day one; peak of 20 Pylons online | metrics | 8 likes, 4 reposts, 1.1K views |
| [2026-04-10](https://x.com/OpenAgentsInc/status/2042439829011460467) | Summary: Pylon is a lightweight compute miner to sell spare power for bitcoin | Pylon | 6 likes, 2 reposts, 274 views |
| [2026-04-10](https://x.com/OpenAgentsInc/status/2042449109726973999) | Pylon network doubled in 21 hours | Pylon, metrics | 6 likes, 4 reposts, 964 views |
| [2026-04-10](https://x.com/OpenAgentsInc/status/2042450069857693926) | Network to power a distributed training run with providers paid in bitcoin | training | 7 likes, 4 reposts, 725 views |
| [2026-04-10](https://x.com/OpenAgentsInc/status/2042465428371296544) | Invites outside compute communities to earn bitcoin in the training run | training | 10 likes, 4 reposts, 3.9K views |
| [2026-04-10](https://x.com/OpenAgentsInc/status/2042626501451919412) | Passed 100K sats paid | metrics | 6 likes, 3 reposts, 1.0K views |
| [2026-04-10](https://x.com/OpenAgentsInc/status/2042728128351531396) | Code pointers: Pylon (node), Nexus (coordinator), Psionic (ML) | Pylon | 4 likes, 2 reposts, 152 views |
| [2026-04-11](https://x.com/OpenAgentsInc/status/2042946084226347153) | 70+ Pylons online | metrics | 6 likes, 3 reposts, 597 views |
| [2026-04-13](https://x.com/OpenAgentsInc/status/2043782380171767849) | Episode 223 "Pay the People" | payments | 21 likes, 12 reposts, 6.1K views |
| [2026-04-14](https://x.com/OpenAgentsInc/status/2043931888910229690) | 825K sats (about $600) paid to Pylons in a week | metrics | 9 likes, 5 reposts, 749 views |
| [2026-04-14](https://x.com/OpenAgentsInc/status/2044072290380333348) | Agents can withdraw Pylon earnings wherever the owner asks | payments | 7 likes, 3 reposts, 325 views |
| [2026-04-14](https://x.com/OpenAgentsInc/status/2044104797318451537) | TEE support planned in Psionic | verifiable | 7 likes, 3 reposts, 1.1K views |
| [2026-05-02](https://x.com/OpenAgentsInc/status/2050560780815196656) | "Running a Pylon"; sats flowing ahead of a new training run | Pylon | 7 likes, 3 reposts, 1.4K views |
| [2026-05-15](https://x.com/OpenAgentsInc/status/2055373697079181428) | Pylon and Nexus v0.2 move to LDK to send many agent micropayments | payments | 19 likes, 9 reposts, 1.4K views |
| [2026-05-15](https://x.com/OpenAgentsInc/status/2055377718993207365) | Wallet infrastructure and the software that runs on your computer are open source | Pylon | 7 likes, 3 reposts, 672 views |
| [2026-05-19](https://x.com/OpenAgentsInc/status/2056786881627439400) | Pylon and Nexus v0.2 go live with training-run code | Pylon | 7 likes, 3 reposts, 358 views |
| [2026-05-19](https://x.com/OpenAgentsInc/status/2056844888780533785) | Episode 227: oceanic phase three of the compute network | marketplace | 13 likes, 8 reposts, 1.7K views |
| [2026-05-22](https://x.com/OpenAgentsInc/status/2057902476209107051) | "compute fracking": bring tens of GW of stranded compute online, starting with consumer devices | marketplace | 7 likes, 5 reposts, 666 views |
| [2026-06-10](https://x.com/OpenAgentsInc/status/2064757440537563349) | Treasury set up; revenue flows to contributors of compute, data, labor, and training | split | 7 likes, 3 reposts, 229 views |
| [2026-06-11](https://x.com/OpenAgentsInc/status/2065196586817216622) | Episode 236: Tassadar training run on consumer edge compute through Pylon | training | 15 likes, 7 reposts, 3.8K views |
| [2026-06-12](https://x.com/OpenAgentsInc/status/2065535602905448870) | No token; earn bitcoin via revenue share | split | 6 likes, 4 reposts, 346 views |
| [2026-06-15](https://x.com/OpenAgentsInc/status/2066601306668810615) | Episode 237 "You Must Construct Additional Pylons": Autopilot 1.0 launch | Pylon | 14 likes, 9 reposts, 5.1K views |
| [2026-06-18](https://x.com/OpenAgentsInc/status/2067700091750879691) | Episode 238: training run launch; first training run with compute providers paid in bitcoin | training | 21 likes, 7 reposts, 357K views |
| [2026-06-19](https://x.com/OpenAgentsInc/status/2068102703092543974) | Episode 239: OpenAgents Cloud referral program, lifetime revenue share | split | 9 likes, 5 reposts, 2.1K views |

### 2026-07 to 2026-10: Coder

Pylon was folded into the new agent interface, and the account said shared
compute would return in Coder. The current product pays plugin contributors
through streaming micropayments.

| Date | Summary | Theme | Engagement |
| --- | --- | --- | --- |
| [2026-07-31](https://x.com/OpenAgentsInc/status/2083270739596029963) | Folding Pylon into the IDE | Pylon | 1 likes, 0 reposts, 24 views |
| [2026-09-15](https://x.com/OpenAgentsInc/status/2099943679444357241) | Episode 284: plans to pay people for spare compute in one inference network for Coder | marketplace | 7 likes, 3 reposts, 630 views |
| [2026-09-15](https://x.com/OpenAgentsInc/status/2099944114129186867) | "reviving the shared compute idea into our new Coder app" (reply mentioning @GPUtopia) | GPUtopia | 2 likes, 0 reposts, 23 views |
| [2026-10-02](https://x.com/OpenAgentsInc/status/2105903502060859718) | Episode 289: plugins pay contributors a revenue share via streaming micropayments | split | 13 likes, 6 reposts, 1.4K views |
| [2026-10-02](https://x.com/OpenAgentsInc/status/2105907276749943119) | Micropayments stream through a built-in Spark wallet | payments | 1 likes, 1 reposts, 495 views |
| [2026-10-03](https://x.com/OpenAgentsInc/status/2106270735752675797) | Plans per-call Lightning payments over x402; starts with a credit balance | payments | 0 likes, 0 reposts, 43 views |

## Posts from the founder's account

Much early discussion also appeared on the founder's account,
[@AtlantisPleb](https://x.com/AtlantisPleb). These posts were found through
Grok's X search and each one was confirmed to exist, with its author and date,
from X's public post data. The list is not exhaustive; see
[Sources and method](#sources-and-method).

| Date | Summary | Theme | Likes |
| --- | --- | --- | --- |
| [2023-09-13](https://x.com/AtlantisPleb/status/1701765445223641382) | Points a developer to the beta launch post that pays bitcoin for spare GPU compute | GPUtopia | 2 |
| [2023-09-22](https://x.com/AtlantisPleb/status/1705318503094370652) | Notes the first AI-generated article about @GPUtopia | GPUtopia | 3 |
| [2023-09-27](https://x.com/AtlantisPleb/status/1706834250283024715) | Shares the long interview on GPUtopia's past, present, and future | GPUtopia | 5 |
| [2023-10-03](https://x.com/AtlantisPleb/status/1709243051380552001) | Meets ASIC designers: the "far future of GPUtopia" | GPUtopia | 5 |
| [2023-10-08](https://x.com/AtlantisPleb/status/1711163866846294069) | At a conference, offers @GPUtopia as attendees' "GPU dealer" | GPUtopia | 21 |
| [2023-11-21](https://x.com/AtlantisPleb/status/1727113116843176053) | Recommends @gputopia and its selling guide, but advises against buying new hardware until the network matures | GPUtopia | 1 |
| [2023-12-09](https://x.com/AtlantisPleb/status/1733579742178374050) | Consumer GPUs exposed through an OpenAI-compatible API: "what we already built and shipped with @GPUtopia" | GPUtopia, marketplace | 8 |
| [2023-12-12](https://x.com/AtlantisPleb/status/1734696935813181454) | "All-in on open AI agents": open GPU compute, bitcoin, and an agent marketplace with revenue share | GPUtopia, split | 36 |
| [2024-04-29](https://x.com/AtlantisPleb/status/1785011926382072154) | Compares a token-based GPU network to "our previous GPUtopia network" | GPUtopia | 5 |
| [2024-12-12](https://x.com/AtlantisPleb/status/1867198641125159049) | Wants people new to bitcoin to earn sats within 60 seconds of signing up | payments | 3 |
| [2024-12-14](https://x.com/AtlantisPleb/status/1867817428606234998) | Onyx chats run on a local Pylon node with Ollama, sent to no closed lab | Pylon | 10 |
| [2025-12-09](https://x.com/AtlantisPleb/status/1998530298536145286) | Several devices combined can do what one cannot: "Swarm eats cloud!" | marketplace | 2 |
| [2025-12-17](https://x.com/AtlantisPleb/status/2001387873426174447) | "The big idea is aggregating edge compute into one market" | marketplace | 0 |
| [2026-03-06](https://x.com/AtlantisPleb/status/2029985891352224253) | Lightning is "what we're doing for our compute market" | payments | 1 |

## Episodes in the transcript archive

The [transcript archive](../transcripts/README.md) starts with the OpenAgents
series on 2023-11-07, so it covers GPUtopia only in retrospect. Earlier
GPUtopia-era video series appear in the tweet index above. Dates are upload
dates from each file's header or the date of the linked video post; the
dates for 288 and 289 are transcription dates. Line numbers point into the
retained files.

| Episode | Date | What it says about the idea | Lines |
| --- | --- | --- | --- |
| [001 Intro](../transcripts/001.md) | 2023-11-07 | Starts the series as an open agent platform tied to the existing GPU network, with open money and open compute. | 15, 50-60 |
| [009 Building the UI](../transcripts/009.md) | 2023-11-11 | Tours the GPUtopia codebase; inference streams from the swarm network of GPUs. | 123-149 |
| [013](../transcripts/013.md), [014](../transcripts/014.md), [058](../transcripts/058.md) | 2023-11-14 to 2024-01-22 | GPUtopia's `queenbee` and worker software on 150-200 consumer GPUs serve inference and embeddings. | 013: 25-27; 014: 461-479; 058: 279-281 |
| [036 Agent Modules 101](../transcripts/036.md) | 2023-12-22 | An agent step served by a GPUtopia provider, with the buyer paying the seller. | 467-469 |
| [037 Flow of Funds](../transcripts/037.md) | 2023-12-22 | Lightning revenue splits; GPU sellers are contributors, and one earns 1,000 sats for providing compute. | 31, 81 |
| [042 Agent Bitcoin Balance](../transcripts/042.md) | 2024-01-04 | Agent withdrawals modeled on GPUtopia's. | 147-151 |
| [064 Lightning Withdrawals](../transcripts/064.md) | 2024-01-29 | Recalls GPUtopia paying sats every 10 seconds until the node failed, and the move to balances with withdrawals. | 45-55 |
| [066 Nostr Plugin Registry](../transcripts/066.md) | 2024-01-31 | A bounty produced a Nostr version of the `workerbee` software. | 15 |
| [086 MVP Launch](../transcripts/086.md) | 2024-03-15 | GPUtopia history: about 250 consumer devices behind an OpenAI-compatible API, and too few buyers for the sellers. | 117-170 |
| [125 The Master Plan](../transcripts/125.md) | 2024-09-12 | Open-sources a Nostr relay and NIP-90 service provider. | 13 |
| [138 Year One Recap](../transcripts/138.md) | 2024-11-10 | GPUtopia lesson: many compute sellers, little buyer demand. | 13 |
| [139 Going Mobile](../transcripts/139.md) | 2024-12-06 | GPUtopia let people sell GPU access for sats; shared compute to return through NIP-90. | 13 |
| [141 One Market](../transcripts/141.md) | 2024-12-10 | NIP-89 and NIP-90 as an open market for AI services, linked to GPUtopia's paid inference. | 13 |
| [142 Data Vending Machines](../transcripts/142.md) | 2024-12-11 | A phone sends a NIP-90 job and a separate provider answers it. | 13 |
| [144 Pylon and the Model Context Protocol](../transcripts/144.md) | 2024-12-13 | Pylon: desktop node software that answers NIP-90 events and holds a wallet, so people can earn bitcoin for running models. | 13 |
| [145 Going Local](../transcripts/145.md) | 2024-12-14 | Pylon serves local models to the phone; no payments yet. | 13 |
| [170 Commander](../transcripts/170.md), [175](../transcripts/175.md), [177](../transcripts/177.md) | 2025-05-06 to 2025-05-19 | Commander as the desktop app to buy and sell compute on a Nostr compute market. | 170: 105-125; 175: 17-31; 177: 37 |
| [174 GPUtopia 2.0](../transcripts/174.md) | 2025-05-14 | Reboots GPUtopia as OpenAgents Compute: "sell your spare compute for Bitcoin"; agents are the missing buyers. | 13-71 |
| [178 Swarm Inference](../transcripts/178.md) | 2025-05-24 | A Go Online button takes NIP-90 jobs and the provider wallet earns sats. | 15-25, 87-97 |
| [195](../transcripts/195.md), [196](../transcripts/196.md), [198](../transcripts/198.md) | 2025-11-11 to 2025-11-19 | Paying others for swarm compute and folding the compute market back into the product. | 195: 277-307; 196: 109-153; 198: 81-121 |
| [200 The Agent Network](../transcripts/200.md) | 2026-01-02 | Spare or stranded compute sold for bitcoin; "compute dividends", where an idle laptop "sells verified jobs". | 243, 923, 1037 |
| [201 Fracking Apple Silicon](../transcripts/201.md) | 2026-01-05 | Paying for spare Apple silicon; recalls about 300 GPUtopia providers online; Pylon provider mode overnight. | 181-199, 271-335 |
| [202 Recursive Language Models](../transcripts/202.md) | 2026-01-07 | Fan-out of NIP-90 jobs to many providers; Pylon launches the next day. | 33-53, 271, 345-349 |
| [203 Pylon and Nexus](../transcripts/203.md) | 2026-01-08 | Pylon "lets you sell your compute for Bitcoin" with the Nexus relay; payments on Spark regtest first. | 135-221, 261-313, 401 |
| [206](../transcripts/206.md), [207](../transcripts/207.md) | 2026-01-27 to 2026-01-28 | Pylon to join Autopilot so users sell agentic compute for streamed sats. | 206: 405-423; 207: 291-305 |
| [213 Agent Markets](../transcripts/213.md) | 2026-03-07 | Launches the compute market and explains GPUtopia's oversupply of sellers. | 19-99 |
| [214 Compute Market](../transcripts/214.md) | 2026-03-12 | Apple silicon beta: Go Online, a Spark wallet, and NIP-90 kind 5050 jobs. | 3-33, 53-63 |
| [215 Data Market](../transcripts/215.md) | 2026-03-22 | Recaps the compute market. | 15, 31 |
| [216 Psionic](../transcripts/216.md) | 2026-03-25 | Introduces Psionic; payouts so far are demo sats; compute to be paid for decentralized training. | 9-21 |
| [220 Propaganda Podcast](../transcripts/220.md) | 2026-04-06 | Pylon is the part that sells your compute. | 45, 65 |
| [221 Pylon Launch](../transcripts/221.md) | 2026-04-08 | Launches Pylon, "our compute miner", to sell compute for bitcoin. | 11-27, 115 |
| [222 Templar Merge](../transcripts/222.md) | 2026-04-10 | The Pylon network doubles; Psionic runs inside Pylon. | 14-29, 50 |
| [223 Pay the People](../transcripts/223.md) | 2026-04-13 | Payouts overwhelmed; argues for paying compute providers in bitcoin. | 3-9 |
| [224 Distributed Training 101](../transcripts/224.md) | 2026-04-16 | Over 1,300 Pylons; payment moves from uptime to real training work. | 9-36 |
| [225 Developer Bounties](../transcripts/225.md) | 2026-04-17 | Run a Pylon to earn bitcoin. | 13, 25 |
| [227 Ocean Power](../transcripts/227.md) | 2026-05-19 | Payouts paused until v0.2; Lightning bottlenecks lead to LDK. | 57-79 |
| [230 Calling All Agents](../transcripts/230.md) | 2026-06-06 | Market one is compute: run Pylon to sell spare compute for bitcoin. | 84, 95 |
| [234 Product Promises](../transcripts/234.md) | 2026-06-09 | Audits the Pylon v0.3 release-candidate promises. | 60-87 |
| [236 Tassadar](../transcripts/236.md) | 2026-06-11 | Pylon v0.3 pays bitcoin for contributing to a training run. | 3-9 |
| [237 You Must Construct Additional Pylons](../transcripts/237.md) | 2026-06-15 | Every Autopilot carries a Pylon that earns sats from spare compute. | 7, 19, 27 |
| [238 The Training Run Begins](../transcripts/238.md) | 2026-06-18 | The training run is live: share compute, earn bitcoin. | 7-9, 36 |
| [239 Let's Make Money](../transcripts/239.md) | 2026-06-19 | Compute supply is ready; the buy side is missing. | 7-19 |
| [240 The Verse](../transcripts/240.md) | 2026-06-21 | Shows Tassadar Pylons and sats paid. | 7-48 |
| [241](../transcripts/241.md)-[244](../transcripts/244.md) | 2026-06-22 to 2026-06-26 | Requests fan out to Pylon workers. | 241: 81-85; 242: 13-17; 243: 100-132; 244: 6-107 |
| [246](../transcripts/246.md), [247](../transcripts/247.md) | 2026-07-03 to 2026-07-10 | Agents turn stranded compute into bitcoin; "run a pylon". | 246: 155-164; 247: 53 |
| [250 Ready the Fleet](../transcripts/250.md) | 2026-07-12 | Pylon has become a local runtime with a wallet. | 128-152 |
| [266 Single Points of Failure and Nostr Markets](../transcripts/266.md) | 2026-08-04 | Recalls selling compute over NIP-90; NIP-MKT replaces it. | 19, 45-57 |
| [274 The First Repo](../transcripts/274.md) | 2026-08-21 | Pylon and Psionic left out of the new repository, to return later. | 63 |
| [275 Coder](../transcripts/275.md) | 2026-08-27 | Swarm inference with Pylon, selling compute for bitcoin, listed to bring back. | 23 |
| [284 Gamifying Coder](../transcripts/284.md) | 2026-09-15 | Paying people for spare compute in one inference network. | 58 |
| [288](../transcripts/288.md), [289](../transcripts/289.md) | 2026-09-30, 2026-10-02 | Revenue share and Lightning micropayments to plugin authors; adjacent to compute. | 288: 19-49; 289: 22 |

Earliest mentions in the archive:

- **GPUtopia's network:** [001](../transcripts/001.md), line 15, "connect it
  to our already built open network of GPUs".
- **Paying a compute seller:** [036](../transcripts/036.md), lines 467-469, and
  [037](../transcripts/037.md), line 81, where a contributor earns 1,000 sats
  "for providing compute".
- **NIP-90:** [125](../transcripts/125.md), line 13; data vending machines by
  name in [139](../transcripts/139.md), line 13.
- **Pylon:** [144](../transcripts/144.md), line 13.
- **"Sell your spare compute for Bitcoin" as the stated product:**
  [174](../transcripts/174.md), line 17.

## Where it lives today

This repository does not contain Pylon, Nexus, or Psionic. These documents
record the idea's current status:

- [`docs/roadmap.md`](../roadmap.md), line 303: GPUtopia, Pylon, Psionic,
  distributed inference, and broader compute markets are optional research
  directions.
- [`docs/psionic-and-pylon.md`](../psionic-and-pylon.md): source map of the
  Pylon compute miner and the separate Psionic repository.
- [`docs/glossary.md`](../glossary.md), lines 1104-1105: Pylon, Nexus, and
  Psionic are marked historical.
- [`docs/agents/market-infrastructure.md`](../agents/market-infrastructure.md),
  line 49: the agent-labor plan, which cites GPUtopia's demand problem and
  treats compute as support for labor.
- [`docs/payments/2026-10-02-central-receive-and-splits.md`](../payments/2026-10-02-central-receive-and-splits.md),
  line 188: the deployed split ledger, where a computer provider earns
  nothing yet and a `provider` role is reserved.
- [`docs/payments/later-markets.md`](../payments/later-markets.md):
  qualification profiles for workers, bids, contribution, training, and
  custody; no paid compute market is advertised.
- [`docs/coder-earn.md`](../coder-earn.md): survey of an earlier earn mode in
  which a machine serves inference and confined runs for credit.
- [`docs/kev/mesh-plan.md`](../kev/mesh-plan.md): proposed serving and
  training of decision models on Pylon-owned hardware; not implemented.
- [`docs/breez/history.md`](../breez/history.md) and
  [`docs/bitcoin/2026-09-28-bitcoin-node-history.md`](../bitcoin/2026-09-28-bitcoin-node-history.md):
  the payment rails Pylon used, from Spark to LDK.
- [`docs/history/2026-09-25-transcript-roadmap.md`](2026-09-25-transcript-roadmap.md):
  maps the compute episodes 201-238.
- [`docs/protocol/official-nip-ledger.md`](../protocol/official-nip-ledger.md):
  NIP-90 job kinds as configured and proven.
- [`nips/openagents/NIP-MKT.md`](../../nips/openagents/NIP-MKT.md),
  [`NIP-LAB.md`](../../nips/openagents/NIP-LAB.md), and
  [`NIP-X402.md`](../../nips/openagents/NIP-X402.md): provider offerings,
  agent labor, and x402 Lightning payment with Nostr discovery.
- [`docs/coder/runtime/free-labor.md`](../coder/runtime/free-labor.md): the
  buyer and provider labor order in `crates/coder-labor`.
- [`docs/sales/revenue-roadmap.md`](../sales/revenue-roadmap.md) and
  [`docs/cloud/compute-balance.md`](../cloud/compute-balance.md): compute that
  OpenAgents sells; no outside provider pool.

Related crates: `crates/wallet` (Lightning receiver and payer on LDK),
`crates/x402` (x402 Lightning over HTTP), `crates/spark-wallet`,
`crates/pay-ledger` (settlement, splits, and the purchased compute balance),
`crates/coder-labor`, and `crates/eval-runner`.

## Sources and method

- **X API.** The account's full post history from 2023-08-25 to 2024-03-31,
  including replies and reposts (959 posts), and every non-repost from
  2024-04-01 to 2026-10-07 (2,869 posts), came from full-archive search
  (`GET /2/tweets/search/all`) with `from:OpenAgentsInc`, deduplicated by ID.
  Four keyword queries over the same period cross-checked the later posts.
  Every post in the @OpenAgentsInc tables was read in full from these
  results. The pay-per-use credit ran out after about 25 requests, so the
  planned keyword searches of the founder's account did not run.
- **Grok.** Ten requests to xAI's Responses API with the `x_search` tool,
  restricted to @OpenAgentsInc or @AtlantisPleb, looked for posts the
  keyword search missed. Every @OpenAgentsInc post Grok returned was already
  in the X API results. Founder posts Grok returned were confirmed one by one
  from X's public post data (`cdn.syndication.twimg.com`); posts that
  did not exist or did not match their description were left out.
- **Transcripts.** The retained files in `docs/transcripts/` were searched
  for GPUtopia, Pylon, NIP-90, data vending machines, providers, sats,
  Lightning, and selling compute, and each match was read in context.
  Transcripts are machine-generated; check the media before quoting.
- **Exclusions.** One September 2023 post naming a tester by email address is
  left out of the index. Reposts of other accounts are counted but not
  listed.

## Appendix: every GPUtopia-handle post

From 2023-08-25 to the rename announcement on 2023-12-12 the account
published 492 posts and replies, plus 184 reposts. The tables
above index 111 of them. The rest are linked here by date (all in
2023), grouped by keywords in their text. Most concern running the market.

- **Beta operations and provider support (WebGPU and browser help, model loading, job assignment, site status, the CLI worker)** (130): [09-08](https://x.com/OpenAgentsInc/status/1699969792189608411), [09-12](https://x.com/OpenAgentsInc/status/1701658422372905399), [09-12](https://x.com/OpenAgentsInc/status/1701724517083533471), [09-13](https://x.com/OpenAgentsInc/status/1701748543134876026), [09-13](https://x.com/OpenAgentsInc/status/1701946969361350974), [09-13](https://x.com/OpenAgentsInc/status/1701947351189934211), [09-13](https://x.com/OpenAgentsInc/status/1701960919914254423), [09-13](https://x.com/OpenAgentsInc/status/1701978864153510059), [09-13](https://x.com/OpenAgentsInc/status/1701979190487118108), [09-13](https://x.com/OpenAgentsInc/status/1702008899547590735), [09-13](https://x.com/OpenAgentsInc/status/1702011032124739979), [09-13](https://x.com/OpenAgentsInc/status/1702011441304186947), [09-13](https://x.com/OpenAgentsInc/status/1702014395495137477), [09-13](https://x.com/OpenAgentsInc/status/1702064700656132225), [09-14](https://x.com/OpenAgentsInc/status/1702114415473033607), [09-14](https://x.com/OpenAgentsInc/status/1702122490766434688), [09-14](https://x.com/OpenAgentsInc/status/1702317899719770128), [09-14](https://x.com/OpenAgentsInc/status/1702423606112989418), [09-14](https://x.com/OpenAgentsInc/status/1702448565883191418), [09-14](https://x.com/OpenAgentsInc/status/1702450121294635119), [09-14](https://x.com/OpenAgentsInc/status/1702458055336571349), [09-14](https://x.com/OpenAgentsInc/status/1702458275067814202), [09-15](https://x.com/OpenAgentsInc/status/1702486257819935068), [09-15](https://x.com/OpenAgentsInc/status/1702666247513489908), [09-15](https://x.com/OpenAgentsInc/status/1702703715155693936), [09-15](https://x.com/OpenAgentsInc/status/1702704297618575619), [09-15](https://x.com/OpenAgentsInc/status/1702764762533179525), [09-16](https://x.com/OpenAgentsInc/status/1702843926980211196), [09-16](https://x.com/OpenAgentsInc/status/1702858466765754846), [09-16](https://x.com/OpenAgentsInc/status/1702860593244389456), [09-16](https://x.com/OpenAgentsInc/status/1702977424235925865), [09-16](https://x.com/OpenAgentsInc/status/1702978036176470237), [09-16](https://x.com/OpenAgentsInc/status/1702981221477752908), [09-16](https://x.com/OpenAgentsInc/status/1702994825056866367), [09-16](https://x.com/OpenAgentsInc/status/1702997392625131621), [09-16](https://x.com/OpenAgentsInc/status/1703002934068334929), [09-16](https://x.com/OpenAgentsInc/status/1703023483293188478), [09-16](https://x.com/OpenAgentsInc/status/1703029836174778530), [09-18](https://x.com/OpenAgentsInc/status/1703881162282705073), [09-18](https://x.com/OpenAgentsInc/status/1703884230806135205), [09-18](https://x.com/OpenAgentsInc/status/1703892491672391799), [09-19](https://x.com/OpenAgentsInc/status/1703948843539206592), [09-19](https://x.com/OpenAgentsInc/status/1703950031105982617), [09-19](https://x.com/OpenAgentsInc/status/1704173188987441299), [09-19](https://x.com/OpenAgentsInc/status/1704175222474019315), [09-19](https://x.com/OpenAgentsInc/status/1704179323765199227), [09-19](https://x.com/OpenAgentsInc/status/1704195866095915414), [09-19](https://x.com/OpenAgentsInc/status/1704280198114377942), [09-20](https://x.com/OpenAgentsInc/status/1704486212768014669), [09-20](https://x.com/OpenAgentsInc/status/1704524942300074439), [09-20](https://x.com/OpenAgentsInc/status/1704570173485633707), [09-20](https://x.com/OpenAgentsInc/status/1704586217864483118), [09-20](https://x.com/OpenAgentsInc/status/1704630035250925654), [09-20](https://x.com/OpenAgentsInc/status/1704631360290169201), [09-21](https://x.com/OpenAgentsInc/status/1704650695305998354), [09-21](https://x.com/OpenAgentsInc/status/1704872341849743785), [09-21](https://x.com/OpenAgentsInc/status/1704879236538872042), [09-21](https://x.com/OpenAgentsInc/status/1704889865194639453), [09-22](https://x.com/OpenAgentsInc/status/1705251984255643764), [09-22](https://x.com/OpenAgentsInc/status/1705261322395377886), [09-22](https://x.com/OpenAgentsInc/status/1705274484490444897), [09-22](https://x.com/OpenAgentsInc/status/1705309500385739011), [09-22](https://x.com/OpenAgentsInc/status/1705309973142503633), [09-22](https://x.com/OpenAgentsInc/status/1705318260747460685), [09-22](https://x.com/OpenAgentsInc/status/1705325533993570402), [09-23](https://x.com/OpenAgentsInc/status/1705591508642988492), [09-23](https://x.com/OpenAgentsInc/status/1705618540164796510), [09-25](https://x.com/OpenAgentsInc/status/1706322648865587612), [09-25](https://x.com/OpenAgentsInc/status/1706358565340876818), [09-25](https://x.com/OpenAgentsInc/status/1706361265004621837), [09-25](https://x.com/OpenAgentsInc/status/1706362477376688417), [09-25](https://x.com/OpenAgentsInc/status/1706369818591441030), [09-25](https://x.com/OpenAgentsInc/status/1706369937667752010), [09-25](https://x.com/OpenAgentsInc/status/1706374807372128685), [09-25](https://x.com/OpenAgentsInc/status/1706405548755722547), [09-26](https://x.com/OpenAgentsInc/status/1706470934356361556), [09-26](https://x.com/OpenAgentsInc/status/1706654041860624428), [09-26](https://x.com/OpenAgentsInc/status/1706679210708779505), [09-26](https://x.com/OpenAgentsInc/status/1706679798171324776), [09-26](https://x.com/OpenAgentsInc/status/1706681012250443870), [09-26](https://x.com/OpenAgentsInc/status/1706692065436823662), [09-26](https://x.com/OpenAgentsInc/status/1706699358018707701), [09-26](https://x.com/OpenAgentsInc/status/1706700744861040753), [09-26](https://x.com/OpenAgentsInc/status/1706702250448482618), [09-27](https://x.com/OpenAgentsInc/status/1707032634386215397), [09-27](https://x.com/OpenAgentsInc/status/1707045866526191865), [09-27](https://x.com/OpenAgentsInc/status/1707048377295290840), [09-27](https://x.com/OpenAgentsInc/status/1707051055450579167), [09-27](https://x.com/OpenAgentsInc/status/1707052816047419755), [09-27](https://x.com/OpenAgentsInc/status/1707054540418068797), [09-27](https://x.com/OpenAgentsInc/status/1707054735616782844), [09-27](https://x.com/OpenAgentsInc/status/1707056529747202414), [09-27](https://x.com/OpenAgentsInc/status/1707067357057761376), [09-27](https://x.com/OpenAgentsInc/status/1707091395847061509), [09-27](https://x.com/OpenAgentsInc/status/1707115417989239120), [09-28](https://x.com/OpenAgentsInc/status/1707253262221545879), [09-28](https://x.com/OpenAgentsInc/status/1707253461392228858), [10-03](https://x.com/OpenAgentsInc/status/1708998345320992977), [10-03](https://x.com/OpenAgentsInc/status/1709302660849942777), [10-03](https://x.com/OpenAgentsInc/status/1709332433798054363), [10-04](https://x.com/OpenAgentsInc/status/1709396548835024918), [10-05](https://x.com/OpenAgentsInc/status/1709873601380323495), [10-10](https://x.com/OpenAgentsInc/status/1711553086941208923), [10-16](https://x.com/OpenAgentsInc/status/1713991029324464143), [10-16](https://x.com/OpenAgentsInc/status/1713992872226168870), [10-16](https://x.com/OpenAgentsInc/status/1713999940702105895), [10-17](https://x.com/OpenAgentsInc/status/1714292061028139058), [10-17](https://x.com/OpenAgentsInc/status/1714366996098212331), [10-18](https://x.com/OpenAgentsInc/status/1714692037495169422), [10-18](https://x.com/OpenAgentsInc/status/1714717749430415408), [10-18](https://x.com/OpenAgentsInc/status/1714778244816257071), [10-18](https://x.com/OpenAgentsInc/status/1714783502497968158), [10-19](https://x.com/OpenAgentsInc/status/1715009135853375976), [10-22](https://x.com/OpenAgentsInc/status/1716119221556428804), [10-25](https://x.com/OpenAgentsInc/status/1717164258004230168), [10-25](https://x.com/OpenAgentsInc/status/1717184994618057137), [10-25](https://x.com/OpenAgentsInc/status/1717194214327329279), [10-26](https://x.com/OpenAgentsInc/status/1717578275403272449), [10-31](https://x.com/OpenAgentsInc/status/1719460522246963625), [10-31](https://x.com/OpenAgentsInc/status/1719477938632986963), [11-02](https://x.com/OpenAgentsInc/status/1719900327036457079), [11-05](https://x.com/OpenAgentsInc/status/1720997137893187776), [11-06](https://x.com/OpenAgentsInc/status/1721565174019035224), [11-10](https://x.com/OpenAgentsInc/status/1722802196725510562), [11-10](https://x.com/OpenAgentsInc/status/1722808599775166549), [11-10](https://x.com/OpenAgentsInc/status/1723021686520283166), [11-11](https://x.com/OpenAgentsInc/status/1723343600568741945), [11-17](https://x.com/OpenAgentsInc/status/1725351782912569530), [11-17](https://x.com/OpenAgentsInc/status/1725616576525738039), [12-03](https://x.com/OpenAgentsInc/status/1731304104398373254)
- **Payments, balances, withdrawals, and Lightning node operations** (38): [08-28](https://x.com/OpenAgentsInc/status/1696127443050889570), [09-12](https://x.com/OpenAgentsInc/status/1701631377983058210), [09-14](https://x.com/OpenAgentsInc/status/1702347745598538233), [09-14](https://x.com/OpenAgentsInc/status/1702410606064615874), [09-14](https://x.com/OpenAgentsInc/status/1702422441325441253), [09-14](https://x.com/OpenAgentsInc/status/1702429677183463514), [09-15](https://x.com/OpenAgentsInc/status/1702796606859366897), [09-16](https://x.com/OpenAgentsInc/status/1703110195155464604), [09-16](https://x.com/OpenAgentsInc/status/1703170384604873049), [09-17](https://x.com/OpenAgentsInc/status/1703415344860180516), [09-18](https://x.com/OpenAgentsInc/status/1703892995865518353), [09-19](https://x.com/OpenAgentsInc/status/1704102597815066893), [09-19](https://x.com/OpenAgentsInc/status/1704189699840004208), [09-19](https://x.com/OpenAgentsInc/status/1704217572856488017), [09-20](https://x.com/OpenAgentsInc/status/1704503272013242469), [09-20](https://x.com/OpenAgentsInc/status/1704505544701038637), [09-21](https://x.com/OpenAgentsInc/status/1704669752231096597), [09-21](https://x.com/OpenAgentsInc/status/1704888257568293359), [09-22](https://x.com/OpenAgentsInc/status/1705235631926911044), [09-22](https://x.com/OpenAgentsInc/status/1705290288833441917), [09-22](https://x.com/OpenAgentsInc/status/1705294156334006411), [09-22](https://x.com/OpenAgentsInc/status/1705308701723079140), [09-22](https://x.com/OpenAgentsInc/status/1705325055813501191), [09-22](https://x.com/OpenAgentsInc/status/1705329673478037823), [09-22](https://x.com/OpenAgentsInc/status/1705342924404207792), [09-25](https://x.com/OpenAgentsInc/status/1706322007472635959), [09-25](https://x.com/OpenAgentsInc/status/1706325930539467049), [09-25](https://x.com/OpenAgentsInc/status/1706360962993775042), [09-25](https://x.com/OpenAgentsInc/status/1706365494251381045), [09-25](https://x.com/OpenAgentsInc/status/1706378519058530526), [09-25](https://x.com/OpenAgentsInc/status/1706407052661211533), [09-26](https://x.com/OpenAgentsInc/status/1706812003258347630), [09-27](https://x.com/OpenAgentsInc/status/1707062337985745093), [09-28](https://x.com/OpenAgentsInc/status/1707535654077288558), [10-03](https://x.com/OpenAgentsInc/status/1709307503329050770), [10-10](https://x.com/OpenAgentsInc/status/1711856734040649838), [10-24](https://x.com/OpenAgentsInc/status/1716954157712850997), [11-09](https://x.com/OpenAgentsInc/status/1722619548757852474)
- **Bitcoin bounties and the open-source program** (12): [09-13](https://x.com/OpenAgentsInc/status/1701952502155903037), [09-17](https://x.com/OpenAgentsInc/status/1703523098627051856), [09-28](https://x.com/OpenAgentsInc/status/1707206284343140456), [10-02](https://x.com/OpenAgentsInc/status/1708931446314762677), [10-03](https://x.com/OpenAgentsInc/status/1709306517046890687), [10-03](https://x.com/OpenAgentsInc/status/1709352984948183258), [10-18](https://x.com/OpenAgentsInc/status/1714728392933662921), [11-01](https://x.com/OpenAgentsInc/status/1719800852104282474), [11-01](https://x.com/OpenAgentsInc/status/1719836542649209209), [11-01](https://x.com/OpenAgentsInc/status/1719864257439744109), [11-09](https://x.com/OpenAgentsInc/status/1722648638290034947), [11-14](https://x.com/OpenAgentsInc/status/1724488722018640001)
- **Build-in-public video episodes (Cloning OpenAI, Atlantis Rising, and the first OpenAgents episodes), most built on the GPUtopia network** (42): [10-06](https://x.com/OpenAgentsInc/status/1710341886894846369), [10-06](https://x.com/OpenAgentsInc/status/1710375500193939553), [10-09](https://x.com/OpenAgentsInc/status/1711223415477346729), [10-10](https://x.com/OpenAgentsInc/status/1711883890036805764), [10-13](https://x.com/OpenAgentsInc/status/1712953173705621765), [10-16](https://x.com/OpenAgentsInc/status/1714028154434589182), [10-20](https://x.com/OpenAgentsInc/status/1715180950840115537), [10-20](https://x.com/OpenAgentsInc/status/1715371396090507618), [10-30](https://x.com/OpenAgentsInc/status/1719035293154586726), [11-06](https://x.com/OpenAgentsInc/status/1721562029373128986), [11-07](https://x.com/OpenAgentsInc/status/1721966796515754266), [11-07](https://x.com/OpenAgentsInc/status/1721979219763155232), [11-08](https://x.com/OpenAgentsInc/status/1722068606714835283), [11-08](https://x.com/OpenAgentsInc/status/1722274309727752427), [11-08](https://x.com/OpenAgentsInc/status/1722287956419871177), [11-08](https://x.com/OpenAgentsInc/status/1722313899771347362), [11-09](https://x.com/OpenAgentsInc/status/1722742595409830389), [11-11](https://x.com/OpenAgentsInc/status/1723164712957862115), [11-11](https://x.com/OpenAgentsInc/status/1723203092647137636), [11-12](https://x.com/OpenAgentsInc/status/1723525820357005661), [11-13](https://x.com/OpenAgentsInc/status/1723888973213286760), [11-14](https://x.com/OpenAgentsInc/status/1724432749275095365), [11-14](https://x.com/OpenAgentsInc/status/1724509783086989333), [11-14](https://x.com/OpenAgentsInc/status/1724568957598708192), [11-15](https://x.com/OpenAgentsInc/status/1724801372602950026), [11-16](https://x.com/OpenAgentsInc/status/1725246583623590158), [11-17](https://x.com/OpenAgentsInc/status/1725597044981617119), [11-18](https://x.com/OpenAgentsInc/status/1725910351563165748), [11-18](https://x.com/OpenAgentsInc/status/1725928497367908432), [11-18](https://x.com/OpenAgentsInc/status/1725948809593638971), [11-18](https://x.com/OpenAgentsInc/status/1725969687102534110), [11-18](https://x.com/OpenAgentsInc/status/1725977712043372666), [11-21](https://x.com/OpenAgentsInc/status/1727018763915247784), [11-22](https://x.com/OpenAgentsInc/status/1727424427825193041), [11-22](https://x.com/OpenAgentsInc/status/1727433378063135085), [11-26](https://x.com/OpenAgentsInc/status/1728590361805672788), [11-26](https://x.com/OpenAgentsInc/status/1728614813675274300), [11-30](https://x.com/OpenAgentsInc/status/1730253928896291251), [12-02](https://x.com/OpenAgentsInc/status/1731086330694651924), [12-03](https://x.com/OpenAgentsInc/status/1731156734335398303), [12-04](https://x.com/OpenAgentsInc/status/1731733390641050106), [12-11](https://x.com/OpenAgentsInc/status/1734044762255036737)
- **Other posts: short replies, AI policy and closed-lab commentary, and teasers** (159): [09-13](https://x.com/OpenAgentsInc/status/1701770923794206802), [09-13](https://x.com/OpenAgentsInc/status/1701983126363246763), [09-13](https://x.com/OpenAgentsInc/status/1702006123203563657), [09-13](https://x.com/OpenAgentsInc/status/1702017452891054129), [09-14](https://x.com/OpenAgentsInc/status/1702275102727958904), [09-14](https://x.com/OpenAgentsInc/status/1702313976179101880), [09-14](https://x.com/OpenAgentsInc/status/1702328698098393218), [09-14](https://x.com/OpenAgentsInc/status/1702347949940802013), [09-14](https://x.com/OpenAgentsInc/status/1702390105816576198), [09-14](https://x.com/OpenAgentsInc/status/1702398045726376080), [09-14](https://x.com/OpenAgentsInc/status/1702399267770396802), [09-14](https://x.com/OpenAgentsInc/status/1702419001069142266), [09-14](https://x.com/OpenAgentsInc/status/1702422218217771408), [09-14](https://x.com/OpenAgentsInc/status/1702434846302228514), [09-15](https://x.com/OpenAgentsInc/status/1702665650072633674), [09-15](https://x.com/OpenAgentsInc/status/1702704527252599246), [09-15](https://x.com/OpenAgentsInc/status/1702794344313294873), [09-15](https://x.com/OpenAgentsInc/status/1702798676748337453), [09-16](https://x.com/OpenAgentsInc/status/1702853051751579946), [09-16](https://x.com/OpenAgentsInc/status/1702861307551133707), [09-16](https://x.com/OpenAgentsInc/status/1702978626847744342), [09-16](https://x.com/OpenAgentsInc/status/1702991958866534402), [09-16](https://x.com/OpenAgentsInc/status/1703022118814777525), [09-16](https://x.com/OpenAgentsInc/status/1703024151735246953), [09-16](https://x.com/OpenAgentsInc/status/1703070071130628150), [09-16](https://x.com/OpenAgentsInc/status/1703111720317689904), [09-16](https://x.com/OpenAgentsInc/status/1703121841219309917), [09-16](https://x.com/OpenAgentsInc/status/1703126450323247371), [09-16](https://x.com/OpenAgentsInc/status/1703129989309137127), [09-17](https://x.com/OpenAgentsInc/status/1703444460028191153), [09-18](https://x.com/OpenAgentsInc/status/1703752567518355927), [09-18](https://x.com/OpenAgentsInc/status/1703782686777598040), [09-18](https://x.com/OpenAgentsInc/status/1703787170446573892), [09-18](https://x.com/OpenAgentsInc/status/1703883929244090587), [09-18](https://x.com/OpenAgentsInc/status/1703886828212736119), [09-18](https://x.com/OpenAgentsInc/status/1703889466283184408), [09-18](https://x.com/OpenAgentsInc/status/1703897384726523999), [09-19](https://x.com/OpenAgentsInc/status/1703953882525671439), [09-19](https://x.com/OpenAgentsInc/status/1704098404068896791), [09-19](https://x.com/OpenAgentsInc/status/1704181074996158765), [09-19](https://x.com/OpenAgentsInc/status/1704184903036756423), [09-19](https://x.com/OpenAgentsInc/status/1704189292598239400), [09-19](https://x.com/OpenAgentsInc/status/1704189786926231996), [09-19](https://x.com/OpenAgentsInc/status/1704191309102166339), [09-19](https://x.com/OpenAgentsInc/status/1704193502467285350), [09-20](https://x.com/OpenAgentsInc/status/1704525113750593751), [09-20](https://x.com/OpenAgentsInc/status/1704540517449576477), [09-20](https://x.com/OpenAgentsInc/status/1704570481305612784), [09-20](https://x.com/OpenAgentsInc/status/1704620408262086895), [09-20](https://x.com/OpenAgentsInc/status/1704634521935626252), [09-21](https://x.com/OpenAgentsInc/status/1704670396706922707), [09-21](https://x.com/OpenAgentsInc/status/1704879516118618470), [09-21](https://x.com/OpenAgentsInc/status/1704887051546476777), [09-21](https://x.com/OpenAgentsInc/status/1704890435217285356), [09-21](https://x.com/OpenAgentsInc/status/1704906743921406152), [09-21](https://x.com/OpenAgentsInc/status/1704983104174096645), [09-21](https://x.com/OpenAgentsInc/status/1704989662450782633), [09-21](https://x.com/OpenAgentsInc/status/1704995064567513412), [09-22](https://x.com/OpenAgentsInc/status/1705191536344850596), [09-22](https://x.com/OpenAgentsInc/status/1705218588565520684), [09-22](https://x.com/OpenAgentsInc/status/1705221278569730553), [09-22](https://x.com/OpenAgentsInc/status/1705226957636444204), [09-22](https://x.com/OpenAgentsInc/status/1705234959298974190), [09-22](https://x.com/OpenAgentsInc/status/1705237534245028321), [09-22](https://x.com/OpenAgentsInc/status/1705239600858337773), [09-22](https://x.com/OpenAgentsInc/status/1705242667339157920), [09-22](https://x.com/OpenAgentsInc/status/1705246405630087432), [09-22](https://x.com/OpenAgentsInc/status/1705253603714256910), [09-22](https://x.com/OpenAgentsInc/status/1705255312020721910), [09-22](https://x.com/OpenAgentsInc/status/1705259242125435026), [09-22](https://x.com/OpenAgentsInc/status/1705293402416308335), [09-22](https://x.com/OpenAgentsInc/status/1705296546537800167), [09-22](https://x.com/OpenAgentsInc/status/1705300994215563325), [09-22](https://x.com/OpenAgentsInc/status/1705302021731918242), [09-22](https://x.com/OpenAgentsInc/status/1705316317077622867), [09-22](https://x.com/OpenAgentsInc/status/1705338051310821767), [09-22](https://x.com/OpenAgentsInc/status/1705347233174999067), [09-22](https://x.com/OpenAgentsInc/status/1705369064497905945), [09-23](https://x.com/OpenAgentsInc/status/1705374364206551422), [09-23](https://x.com/OpenAgentsInc/status/1705380989000880355), [09-23](https://x.com/OpenAgentsInc/status/1705383138422338035), [09-23](https://x.com/OpenAgentsInc/status/1705589592030630094), [09-24](https://x.com/OpenAgentsInc/status/1705734996772655449), [09-24](https://x.com/OpenAgentsInc/status/1705756661049168357), [09-24](https://x.com/OpenAgentsInc/status/1705773520804892994), [09-25](https://x.com/OpenAgentsInc/status/1706407269670191131), [09-25](https://x.com/OpenAgentsInc/status/1706416341245182172), [09-25](https://x.com/OpenAgentsInc/status/1706434139832852488), [09-26](https://x.com/OpenAgentsInc/status/1706687498858922276), [09-26](https://x.com/OpenAgentsInc/status/1706716142960550391), [09-26](https://x.com/OpenAgentsInc/status/1706722313679380981), [09-26](https://x.com/OpenAgentsInc/status/1706738660341583978), [09-27](https://x.com/OpenAgentsInc/status/1707038800138908007), [09-27](https://x.com/OpenAgentsInc/status/1707069959900221637), [09-27](https://x.com/OpenAgentsInc/status/1707075556091613673), [09-27](https://x.com/OpenAgentsInc/status/1707092091556217296), [09-27](https://x.com/OpenAgentsInc/status/1707158176838176983), [09-28](https://x.com/OpenAgentsInc/status/1707468656869794214), [09-29](https://x.com/OpenAgentsInc/status/1707782600016658473), [10-02](https://x.com/OpenAgentsInc/status/1708836881717158028), [10-03](https://x.com/OpenAgentsInc/status/1708996128669094349), [10-03](https://x.com/OpenAgentsInc/status/1709197636924842362), [10-03](https://x.com/OpenAgentsInc/status/1709199667777458668), [10-03](https://x.com/OpenAgentsInc/status/1709200406113398838), [10-03](https://x.com/OpenAgentsInc/status/1709242720345108751), [10-03](https://x.com/OpenAgentsInc/status/1709249896321007749), [10-04](https://x.com/OpenAgentsInc/status/1709359050092151012), [10-06](https://x.com/OpenAgentsInc/status/1710379683546628483), [10-06](https://x.com/OpenAgentsInc/status/1710439147050057934), [10-10](https://x.com/OpenAgentsInc/status/1711886666208825356), [10-16](https://x.com/OpenAgentsInc/status/1713991238943191342), [10-17](https://x.com/OpenAgentsInc/status/1714310562811617670), [10-20](https://x.com/OpenAgentsInc/status/1715356921614504007), [10-25](https://x.com/OpenAgentsInc/status/1717169358093349024), [10-25](https://x.com/OpenAgentsInc/status/1717200129562751368), [10-25](https://x.com/OpenAgentsInc/status/1717271157760467076), [10-26](https://x.com/OpenAgentsInc/status/1717377267481882902), [10-26](https://x.com/OpenAgentsInc/status/1717633224980021662), [10-28](https://x.com/OpenAgentsInc/status/1718378508206879231), [10-29](https://x.com/OpenAgentsInc/status/1718760819360473458), [10-30](https://x.com/OpenAgentsInc/status/1719140644856500587), [10-31](https://x.com/OpenAgentsInc/status/1719353292910678130), [10-31](https://x.com/OpenAgentsInc/status/1719426369354916108), [11-01](https://x.com/OpenAgentsInc/status/1719780212865429647), [11-02](https://x.com/OpenAgentsInc/status/1719897195334213840), [11-02](https://x.com/OpenAgentsInc/status/1720114275174252877), [11-04](https://x.com/OpenAgentsInc/status/1720896500580913196), [11-05](https://x.com/OpenAgentsInc/status/1721202983088660678), [11-06](https://x.com/OpenAgentsInc/status/1721607001476644888), [11-06](https://x.com/OpenAgentsInc/status/1721623202214949316), [11-07](https://x.com/OpenAgentsInc/status/1721912258471632956), [11-08](https://x.com/OpenAgentsInc/status/1722043840490566012), [11-09](https://x.com/OpenAgentsInc/status/1722626959891689648), [11-10](https://x.com/OpenAgentsInc/status/1722790186017292417), [11-10](https://x.com/OpenAgentsInc/status/1722793033630982255), [11-10](https://x.com/OpenAgentsInc/status/1722797112600015304), [11-10](https://x.com/OpenAgentsInc/status/1722797621549334948), [11-10](https://x.com/OpenAgentsInc/status/1722797803011756436), [11-10](https://x.com/OpenAgentsInc/status/1722799137291137450), [11-10](https://x.com/OpenAgentsInc/status/1723009799720231240), [11-10](https://x.com/OpenAgentsInc/status/1723010975140311416), [11-10](https://x.com/OpenAgentsInc/status/1723052191747850431), [11-10](https://x.com/OpenAgentsInc/status/1723058062657761741), [11-14](https://x.com/OpenAgentsInc/status/1724541852064305598), [11-14](https://x.com/OpenAgentsInc/status/1724543805855060137), [11-16](https://x.com/OpenAgentsInc/status/1724975683007300070), [11-17](https://x.com/OpenAgentsInc/status/1725313558513111257), [11-18](https://x.com/OpenAgentsInc/status/1725687543968956888), [11-18](https://x.com/OpenAgentsInc/status/1725726561100288286), [11-19](https://x.com/OpenAgentsInc/status/1726109717221814492), [11-19](https://x.com/OpenAgentsInc/status/1726329208601977013), [11-20](https://x.com/OpenAgentsInc/status/1726469930315559192), [11-20](https://x.com/OpenAgentsInc/status/1726628054154280988), [11-21](https://x.com/OpenAgentsInc/status/1727034716329787406), [11-22](https://x.com/OpenAgentsInc/status/1727323856493310220), [11-22](https://x.com/OpenAgentsInc/status/1727453376307200301), [11-23](https://x.com/OpenAgentsInc/status/1727495358949789833), [12-09](https://x.com/OpenAgentsInc/status/1733549072894833071), [12-09](https://x.com/OpenAgentsInc/status/1733555667473231882)

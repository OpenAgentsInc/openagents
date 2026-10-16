# Research follow-ups: what to build from the 48 hours of research (2026-10-15)

On 2026-10-15 we reviewed everything added under `docs/research/` in the previous
48 hours, and `docs/payments/2026-10-15-lnd-sdk-plan.md`, against the open and
recently closed issues. This is the roadmap that came out of it.

- **Before Coder 1.0.0:** only small launch-safety work. Each item has an issue:
  #11763–#11769.
- **Everything else is roadmap, not issues.** Issues get opened when an item
  starts. That doesn't apply to items that already have one (noted below).

## Before 1.0.0 (issues open, being done now)

| Issue | Item | Source |
| --- | --- | --- |
| #11763 | `INVARIANTS.md`: the gateway's door order says Jev first; the code is Clef first | `research/decisionmodels/s1-llm-auto-router.md` |
| #11764 | Every launch figure, caption and post is in `docs/launch/1.0/claims.md`; no x401-style or competitor claims | `research/garyvee/strategy.md`, `research/2026-10-15-x401-agent-authorization.md`, `research/dhruv/analysis.md` |
| #11765 | Privacy wording: "sealed against the cloud", never "sealed against us"; never call Gemini on Vertex, Macs or GeForce cards private | `research/opaque/README.md` |
| #11766 | Cloud computers: the BYOK plan token stays out of `/tmp`, passed per turn in memory | `research/opaque/README.md`, `research/perplexity/2026-10-15-space-assessment.md` |
| #11767 | Clean up `research/task-index/task-index-research.md` | `research/task-index/` |
| #11768 | AI labels on AI-made launch clips; captions follow the we/I voice rule; no fake personas | `research/garyvee/` |
| #11769 | Credit Genesis in the ported physics code (and a NOTICE if any file was translated directly) | `research/genesis/repo.md` |

## Soon after 1.0.0 (the next 2–4 weeks)

| # | Item | Size | Source | Issue | Owner |
| --- | --- | --- | --- | --- | --- |
| 1 | Generate the `webmcp.js` tool list from the `docs_mcp.rs` definitions (they've already drifted). Then per-tool `POST /mcp/tool/{name}`, then effect and confirm metadata on each operation | S→M | `research/2026-10-15-builderio-agent-native.md` | when started | agent |
| 2 | Desktop SDK sessions: slice 4 (explicit provider, model and reasoning choice; native resume that can't silently become a new chat), then slices 5–7 | M | `research/zeron/coder-sdk-integration.md` | #11709 | agent |
| 3 | SPACE spike as a `computer-driver` on one GCE host with nested virtualization. Pass bar: create at p50 under 2 s (today `oa-boat` takes 77 s to a first turn) | M | `research/perplexity/2026-10-15-space-assessment.md`, `research/factory/README.md` | #11728 | agent |
| 4 | Credentials out of the agent process, injected at egress, before any computer runs on someone else's money or repository | L | `research/perplexity/`, `research/opaque/` | when started | agent |
| 5 | The launch content loop: 3–6 real clips a day, one-tap owner approval, scored by signups and Pro starts. First test: 5 cost-hook clips against 5 capability-hook clips | M | `research/garyvee/strategy.md` | when started | owner go + agent |
| 6 | Affiliates part 2: short `/r/{handle}`, the code carried on `/contact`, self-serve links. Named Pro sales wait for Stripe | M | `research/garyvee/`, `research/layertree/` | #11727 | agent; linking `/affiliates` is the owner's call |
| 7 | Task index: learn from verified misses, after its evaluation gate | M | `research/task-index/` | #11719 (and #11716–#11718) | agent |
| 8 | Psionic native parity (CPU, digit GPU, Metal, accelerator serving). These results are not proof of routing quality | M each | `research/decisionmodels/psionic-onnx-native-roadmap.md` | #11722–#11726 | agent |
| 9 | Router experiment: a 300+ request eval set; s1 only as an offline "cheap answer?" signal; then our own small router with Clef as teacher (≥90% agreement, ECE ≤ 0.03). Synthetic, Clef-labelled data until the owner approves real request text | M | `research/decisionmodels/s1-llm-auto-router.md` | when started | agent; real data is the owner's call |
| 10 | The Dhruv NIP backlog, on fixtures and fake money only, in order: ATIF, REG and WORK first | L | `research/dhruv/implementation-proposal.md` | #11729, #11731–#11749 | agent |
| 11 | The x401 holder role only: detect `PROOF-REQUEST`, hand it to the person, never fill it in automatically. Identity disclosure becomes a floor, with `INVARIANTS.md` updated in the same change | S–M | `research/2026-10-15-x401-agent-authorization.md` | when started | agent |
| 12 | Opaque's small items: a policy digest in NIP-ATT releases and receipts; release credentials only to attested machines | S each | `research/opaque/README.md` | when started | agent |

## Later: research bets, each with its trigger

| # | Item | Trigger | Source | Owner |
| --- | --- | --- | --- | --- |
| 1 | LND as our server-side Lightning backend via `crates/lnd-sdk` (epic #11762: P0 regtest, P1 one GCE node for credits and Pro, P2 payouts, P3 liquidity and Taproot Assets, P4 LDK stays embedded) | Owner go on P0, plus a budget | `payments/2026-10-15-lnd-sdk-plan.md`, `research/layertree/integration-plan.md` | owner decision |
| 2 | `crates/ui-port`, the shadcn converter: M0 is `/landing` from MIT primitives with our copy, then M1–M5 | After 1.0; licence check | `ui/2026-10-15-shadcn-converter-and-ui-system-plan.md` | agent (M0 copy needs the owner) |
| 3 | `data-graph`, a typed-query compiler: a 40-task offline experiment, then a read-only admin `data_query` tool | It beats text-to-SQL by ≥10 points | `research/2026-10-15-graphjin-compiler-harness.md` | agent |
| 4 | The self-funded agent loop: earn sats from accepted work, spend them on inference credit | Pro billing and Lightning receive live | `research/dhruv/implementation-proposal.md` (#11741/#11742) | owner decision (live money) |
| 5 | A desktop "Earn" switch for contributors serving Clef-Flash decisions; mainnet pricing and pay come first | The owner sets the price | `research/decisionmodels/README.md` | owner decision |
| 6 | Confidential VMs for `oa-boat`, signed run receipts, a Rekor transparency log | After soon-after #4 | `research/opaque/README.md` | agent |
| 7 | `oa-computers` driver contract, Postgres reconciler, warm pool, metering at 40 msat/s | The SPACE spike's results | `research/factory/README.md` | agent; pricing is the owner's |
| 8 | A Muse Spark trial on Vertex: about 6 h, about $450 of credit, a pinned-only route | The owner accepts the licence | `research/muse/2026-10-15-muse-spark-serving.md` | owner decision |
| 9 | LayerTree on mainnet with a TEE in place of covenants: [spec](../research/layertree/2026-10-15-mainnet-tee-spec.md); sandbox `crates/layertree-tee-lab` (outside the default build) holds the tree, the enclave predicate, the unroll and light clients (P0 started) | No soft fork needed. P1 (signet on Confidential VMs) needs the owner's OK to run the VMs. P2 (capped mainnet) needs the owner's go/no-go and the #11761 custody review. Any reuse of upstream code waits for a LICENSE file | `research/layertree/` | agent; owner for P1/P2 |
| 10 | A CAD kernel on SolidCraft/Truck, `part.v1`, an agent-built rover | The owner settles CAD scope | `research/cad/` (#11627) | owner decision |
| 11 | An Aether-style stage-checked SO-101 planner experiment (3–5 agent-days) | Robot Lab capacity | `research/aether-ai/README.md` | agent |
| 12 | Zeron parity packages A–Q | After #11709 | `research/zeron/` | agent + 2 owner calls |

## Not doing

| Item | Why | Source |
| --- | --- | --- |
| Fake-avatar or AI-persona account networks, undisclosed paid posts | Breaks Meta's rules and the FTC endorsement rules (16 CFR 255/465); conflicts with our voice rule | `research/garyvee/100-ai-instagram-pages.md` |
| s1's hosted API as a routing door | Sends user text to a third party; it underrates agentic work (20/32 against Clef's 24/32 on our fixture) | `research/decisionmodels/s1-llm-auto-router.md` |
| Adopting GraphJin itself | About 745k lines of Go and a second policy authority; its lexical catalog search would break the routing rule | GraphJin doc |
| The Builder.io framework, its default engines, its Stripe paths, generative UI | Breaks Google-first and Bitcoin-only; generative UI breaks the CSP; unclear licence | Builder.io doc |
| Becoming an x401 verifier or issuer; paying EVM x402 | Brings PII/KYC duties; a non-Bitcoin rail | x401 doc |
| Paid shadcnblocks code (Landing Page 5) in the public repo, or AI-made derivatives of it | A licence violation | `ui/` plan |
| Vendoring SPACE; adopting its EKS/E2B stack; Kubernetes in product services | Out of scope; SPACE stays an out-of-process driver | Perplexity doc, #11728 |
| Vendoring LayerTree code; production builds on it | No licence; needs soft forks that aren't active; unaudited | `research/layertree/` |
| A ClassCAD licence; OCCT | About CHF 5k a year and would make us a distributor; LGPL | `research/cad/` |
| Genesis Python, Quadrants kernels, unlicensed assets, `isaacgym/terrain_utils.py` | Licence problems; we already use MuJoCo | `research/genesis/repo.md` |
| Opaque's hosted product, SGX, post-quantum keys now, compliance marketing | An Azure/Auth0 lean; poor value now | `research/opaque/README.md` |
| Mesh networks, packet routing, "AI is alive" framing | No product value; poor framing for copy | `research/dhruv/analysis.md` |

## Owner decisions this roadmap waits on

1. **Muse Spark licence:** a "Built with Muse Spark Open" label in the product; no training Clef or Psionic on its outputs; outputs can't be presented as human-made.
2. **LND** (epic #11762, plan §7): go on P0; the P1 budget (about $240–350/mo plus 5–10M sats of inbound liquidity); the hot-wallet cap and cold address; the remote signer's timing; the custody review (#11761). P2 changes the workspace rule that payouts stay on the MDK/Nexus bridge.
3. **`tapd` vs `tap-ldk`** for server-side Taproot Assets.
4. **Paid shadcn blocks:** buy a licence and use a private repo, or MIT primitives only.
5. **Router training data:** whether real request text may be used (a separate opt-in and a privacy review).
6. **The Earn switch:** price; whether contributors serve our internal traffic by default; battery Macs; a launch bonus; whether to name Decision Models/REAX publicly.
7. **The content loop:** a go, and the owner as the one-tap approver for every post.
8. **CAD scope:** our own worlds only, or a user-facing CAD product.
9. **Zeron parity:** a model/harness picker before the first send (COM-01, which conflicts with the no-modes rule); local-only sessions before login (RUN-11).
10. **Anthropic:** whether to ask about an attested Claude endpoint, and about stored plan tokens for people other than the owner.
11. **Linking `/affiliates` from the site** (#11727).

## Where research docs disagree, and which wins

- **Routing:** s1 (as a signal), our own small router trained from Clef, and Muse as a pinned route all touch routing. **The Clef-first Decisions API wins**: it's the owner's standing decision and what the code does. s1 can only add an input after passing its gate, and Muse never joins the default route list.
- **Sandboxes and computers:** Factory's own Firecracker driver, the SPACE spike (#11728) and Opaque's Confidential VMs overlap. **SPACE is the Firecracker driver** behind Factory's `computer-driver` contract, replacing Factory item 11. Confidential VMs are a separate hardening track for the GCE driver, not a third sandbox.
- **Memory:** the task index (verified recipes), Dhruv's NIP-AM memory, and Cowork memory in the Claude teardown. **The task index wins**: it's the only one with an evaluation gate and issues. NIP-AM stays spec-only, and the Claude teardown is descriptive.
- **The Lightning stack:** the LND plan (server side), LayerTree (claimable credit leaves) and `tap-ldk`. **The LND plan** is the only one with a production path. It needs owner decisions 2–3. LayerTree stays research; its TEE route has a capped pilot path that waits on the owner.
- **Credentials:** Opaque's "token in memory per turn" (#11766) and SPACE's "inject at egress" are stages, not rivals: memory first, egress injection later.
- **Desktop modes:** Zeron's "explicit coding-session mode" and COM-01 conflict with the no-Chat/Code-modes rule. **The rule wins** unless the owner says otherwise.

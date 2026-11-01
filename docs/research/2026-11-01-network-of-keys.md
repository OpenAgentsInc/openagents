# A network of keys: custody as a graph

- Source: Dhruv Bansal (Unchained), "Bitcoin needs a network of keys: Why
  Unchained is building markets for collaborative custody". Unchained blog,
  <https://unchained.com/blog-author/dhruvbansal>. The copy we read is
  undated. It says Unchained coined "collaborative custody" in 2018, and
  Unchained's 2023 Coincover announcement already uses the phrase "network of
  keys".
- Read in full on 2026-11-01. This note covers the idea, not Unchained's
  product line.

## The idea in one paragraph

Today you choose between two ways to hold bitcoin. Self-custody gives you
full control and leaves all the risk with you. A custodian takes the risk
off you, but it becomes a single point of failure and you lose control.
Multisig removes that false choice. A wallet that needs *m of n* keys can
split the keys across several parties, so no one party can lose or steal
the funds alone. The essay's step is to treat each key holder as a **key
agent**: a role that anyone able to run a key safely can sell. Once key
agency is a market, custody stops looking like hubs with spokes and starts
looking like a network. Holders are nodes. A key-agency relationship is an
edge. When a hub fails, holders sweep to new wallets with other agents, and
the network routes around the damage.

## The argument, step by step

1. **Control and burden are separate dials.** The common setup is 2-of-3:
   the client holds two keys and the platform holds one. The client keeps
   control, but a client who loses two keys at once loses everything. Many
   people and companies won't take that risk.
2. **You can hold a minority of keys and still matter.** A client with one
   key out of three can't move funds alone and can't lose them alone. They
   can still check addresses, balances and transactions against their own
   key. Their signature is also strong evidence of identity and intent to
   the other signers.
3. **You can hold no keys and still avoid a single point of failure.**
   Split the keys across three independent agents instead of trusting one
   custodian. "Bitcoins aren't eggs": you don't need several baskets when
   one wallet can require several independent hands.
4. **Majority delegation needs more than one agent.** If you hold one key
   and give the other two to a single agent, that agent is a custodian again.
   Minority-key holders need *third-party* agents, so the market needs many
   agents.
5. **Agents appear when agency pays.** Building a custodian from scratch
   costs a lot (key operations, software, compliance, support, marketing),
   so there are few custodians. A platform that already has the
   coordination software, hardware-wallet integrations and client base can
   cut an agent's job to "run a key well" and let agents sell that service
   directly.
6. **More agents makes the system stronger.** Each agent protects less
   value, so a breach hurts fewer people. Replacing an agent is easy, and
   holders can choose the agent that fits their size and budget.
7. **The network is multiscale.** Large custodians and trust companies serve
   large holders. Advisers, accountants and lawyers serve the middle. Friends
   and family serve each other at small scale.
8. **The edges carry trust.** An edge is a relationship in which both sides
   can verify each other's real identity and intent. That trust makes the
   graph a natural base for lending, insurance, inheritance, trading and
   payments.
9. **Custodians won't build it.** Their model assumes they control the
   coins, often to lend them out. Collaborative-custody platforms have both
   the capability and the incentive.
10. **Holders can grow over time.** Start with no keys, move to a minority,
    and maybe reach a majority, without changing platforms.

## Topologies

| Shape | Who holds keys | Single point of failure? | Burden on holder |
|---|---|---|---|
| Custodial | One custodian | Yes: the custodian | None |
| Self-custody | Holder holds all | Yes: the holder | All |
| Hub and spoke (classic collaborative) | Holder holds a majority, platform holds one | No for loss; platform is the recovery path | High |
| Minority holder | Holder holds one, two independent agents hold the rest | No | Low |
| Fully delegated | Three independent agents | No | None, but no veto |
| Network | Any mix, agents chosen per holder from a market | No, and agent failure is routable | Chosen by the holder |

The essay's claim isn't really "multisig is good". Multisig has been
possible since 2012. The claim is that the *topology* of custody across the
whole ecosystem matters, and that a market for key agency is what moves it
from a few big hubs to a mesh.

## The primitives that make it work

The concept depends on open, standard building blocks. Without them an
agent's key only works on one platform, and the "network" collapses back
into the platform.

- **Script multisig** (P2WSH, or tapscript under Taproot). Anyone can audit
  the spending policy. Any compliant tool can rebuild the wallet.
- **Output descriptors and xpubs** (BIP-32, BIP-380 and later). A descriptor
  fully describes the wallet. Each agent and holder keeps a copy, so any of
  them can rebuild it without the coordinator.
- **PSBT** (BIP-174/370). It passes an unsigned transaction between signers
  on any software and hardware.
- **Miniscript and timelocks.** These add time to the policy: a recovery key
  that becomes valid after a delay, or a threshold that drops over time. Liana
  ships "decaying multisig", for example 3-of-5 that becomes 3-of-6 with a
  notary after a year and 2-of-6 later. That turns "network of keys" into
  "network of keys and clocks". An inheritance agent can hold a key that is
  useless until the owner has been inactive for a set time.
- **Threshold signatures (FROST, MuSig2).** On-chain they look like a single
  key, which is cheaper and more private. The essay prefers script multisig
  over proprietary MPC because anyone can recover it with open tools. FROST is
  an open standard (RFC 9591), so the real tradeoff is between hidden policy
  with interactive key generation and visible policy that any tool can
  recover.

## Where the concept is weak

- **The coordinator is a soft hub.** In a network of keys, no one can
  steal alone. But one platform still stores descriptors, assembles PSBTs,
  shows balances and runs the agent market. If it goes down, liveness and
  discovery break even though the funds are safe. The essay says agents can
  recover with open tools. That works only if every party actually keeps the
  descriptor.
- **Agents can fail together.** Independence is the whole point, but agents
  may share a jurisdiction, a regulator, an insurer, an HSM vendor or a
  platform-run signing service. Three agents in one country can be one point
  of failure for seizure purposes.
- **Recovery is an attack surface.** Social key recovery means an agent signs
  when someone asks. The main attack on a minority-key or no-key holder is to
  impersonate them to two agents. The essay's answer is strong real-world
  relationships. That works for a family member or a long-time lawyer and
  doesn't scale to strangers in a market.
- **A market for agents needs a reputation system.** How does a holder judge
  a key agent they don't know? Audits, insurance and track record work for
  large firms. For small agents the essay offers only training and
  certification, run by the platform, which makes the platform a hub again.
- **Privacy.** Every agent with an xpub sees your whole wallet history. More
  agents means more parties who can see your balance. Taproot key-path spends
  and per-agent descriptor sharing help but don't solve this.
- **Liability and regulation.** An adviser holding a client's key raises
  real questions about fiduciary duty, licensing and insurance. The essay
  mentions them only in passing.

## What it means for OpenAgents

We already have several pieces of a network of keys, built for other
reasons:

- **Sovereign agents.** `openagents.sovereign.v1` (protocol spec §27.3)
  gives an agent a `Custody` adapter (NIP-46 today), a `GuardianPolicy` (a
  list of guardians and a quorum) and a `TreasuryPolicy` (daily limit,
  per-purchase limit, allowed purposes). A guardian quorum over an agent's
  Nostr key is a network of keys for identity instead of coins. The same
  graph could hold the agent's Bitcoin keys.
- **Spark.** A Spark wallet (see `docs/lightning/breez-spark.md`) splits
  each leaf's key between the user and a FROST threshold of operators
  (Lightspark, Breez and Flashnet, 2-of-3). That's a fixed, small network of
  keys chosen by the protocol, not by the holder. It has the same main risk
  the essay names: two agents colluding. Here it means keeping old shares
  they were meant to delete.
- **Bitcoin-only rails.** Our payments run on Bitcoin, Lightning and Taproot
  Assets only. Collaborative custody is the strongest model for holding
  treasury balances on those rails.

Three ideas worth taking from the essay:

1. **Agents as key agents.** An agent with a hardened, attested environment
   (we already model `AttestationLevel` up to TEE) can be one signer in a
   holder's quorum. It can enforce policy before signing, for example:
   follow the treasury limits, only pay invoices tied to a market order, and
   delay large spends. It's an always-on co-signer that is cheaper than a
   human professional and more predictable. It must never be the majority.
2. **Holders that are agents.** A sovereign agent's treasury shouldn't sit
   on a single hot key. The essay's minority-holder shape fits: the agent
   holds one key and runs day-to-day, its operator holds one, and an
   independent guardian holds one, with a timelocked recovery path back to
   the operator. The `GuardianPolicy` quorum is already the right shape.
3. **Edges as trust.** If key-agency edges carry verified identity and
   intent, they're a good base for the market features we build: credit for
   compute buyers, escrow for labor, and inheritance for an agent's earnings.
   That's the essay's point 8 applied to machine work.

We won't run a custody business or put customer funds behind our own keys
as a result of this note. A network-of-keys feature would be a policy and
coordination layer: descriptors, PSBT routing and agent co-signers over open
standards. Any party must be able to leave with the descriptor and recover
with a different tool.

## Open questions

- Should a sovereign agent's `Custody` grow a Bitcoin adapter (descriptor
  plus PSBT signer) next to NIP-46, using the same guardian quorum?
- Can attestation and published signing policy replace the "strong real-world
  relationship" that makes human key agents hard to impersonate?
- How do we make the coordinator optional, so descriptors and PSBTs move
  over Nostr between parties instead of through one service?
- For small balances, is a fixed operator network (Spark) good enough, with
  holder-chosen multisig only above a set amount?

## Sources

- Unchained, Dhruv Bansal author page:
  <https://unchained.com/blog-author/dhruvbansal>
- Unchained and Coincover partnership, 2023 ("network of keys"):
  <https://www.businesswire.com/news/home/20231003198099/en/Unchained-Announces-Partnership-with-Coincover-to-Further-Strengthen-its-Bitcoin-Collaborative-Custody-Network>
- Bitcoin Optech, "A Miniscript Journey" (Wizardsardine, Liana):
  <https://bitcoinops.org/en/wizardsardine-miniscript/>
- Delving Bitcoin, "Using OP_VAULT for recovery":
  <https://delvingbitcoin.org/t/using-op-vault-for-recovery/150>
- Liana v10.0 release notes:
  <https://www.nobsbitcoin.com/liana-wallet-v10-0/>
- In this repo: `docs/protocol/openagents-protocol-spec.md` §27.3,
  `docs/lightning/breez-spark.md`.

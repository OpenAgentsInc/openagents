# Ark Protocol and Money Dev Kit (MDK) Agent Payments Audit

Date: 2026-06-09

Workspace: `/Users/christopherdavid/work`

Scope reviewed:
- `projects/ark/` (ark-bitcoin group repositories: `bark`, `bark-ffi`, `bark-ffi-bindings`, `bark-btcpay`, `faucet`, `handshark`, `barkd-clients`, `docs`, `bark-qa`)
- `projects/moneydevkit/` (`mdk-checkout`, `api-contract`, `mdkd`, `ldk-node`, `vss-server`, etc.)
- `openagents/` (Bun + Effect monorepo)
- `probe/` (coding-agent runtime repo)

---

## Executive Answer

To build a fully self-custodial, high-performance, and economically viable payment substrate for autonomous agents, OpenAgents should utilize **Ark (specifically Second’s `bark` implementation) alongside Money Dev Kit (MDK)**. 

These two payment technologies are not competitors; rather, they form a highly complementary **dual-channel payment stack** where **MDK handles external public payments** (pay-as-you-go paid APIs via L402 and web-checkout flows over standard Lightning) and **Ark handles internal, high-frequency, and inter-agent transfers** (fully out-of-round, zero-fee, zero-liquidity VTXO chaining):

1. **Ark (Bark)** solves the onboarding and liquidity bottleneck for agents. It lets agents receive and hold self-custodial bitcoin with **zero channels, zero inbound liquidity constraints, and zero routing overhead**. Payments between agents on the same Ark server occur instantly out-of-round (*arkoor*), settling in milliseconds with zero fees and no capital lockups.
2. **Money Dev Kit (MDK)** solves the cross-app, public paid-service boundary. It provides the **L402 paid-API layer** (`withPayment`), the unified agent wallet client, and checkout flows. An agent can spend Ark VTXOs atomically through the Ark server's **Lightning Gateway** to settle MDK invoices and access protected HTTP endpoints across the internet.

By combining them, an agent's MDK wallet can hold its primary balance in **Ark VTXOs** (inherently trustless/round-confirmed), spend them instantly to another agent on the same Ark out-of-round, or spend them through the Ark server's Lightning Gateway to buy API calls or products via MDK on standard Lightning.

---

## Technical Deep-Dive: Ark Protocol (Bark)

Ark is a second-layer scaling protocol on Bitcoin based on **Virtual UTXOs (VTXOs)** organized in **Transaction Trees**, coordinated periodically by an **Ark Service Provider (ASP)**.

### How it Works:
* **Out-of-Round (arkoor) Payments:** Under the hood, payments on Bark are executed out-of-round. The sender works with the Ark server to construct a **spend VTXO** chained directly from the sender’s existing tree leaf. This settles **instantly** and has **zero liquidity costs** because it does not require on-chain locking of new capital.
* **The Temporary Trust Trade-Off:** The receiver can spend or chain this VTXO immediately, but operates under a temporary trust model: they must trust that the sender and Ark server do not collude to double-spend. 
* **Trustless Upgrades (Refreshing):** To return to a completely trustless, standard UTXO-equivalent security model, the wallet automatically participates in a subsequent periodic **round** (conducted by the server every 1-2 hours) to **refresh** the spend VTXO into a confirmed **refresh VTXO** committed to the root of a new on-chain transaction tree.
* **Lightning Gateway (HTLCs):** Bark servers act as full Lightning Gateways. Users can pay Lightning invoices or receive incoming Lightning payments as VTXOs. All gateway operations utilize Hash Time-Locked Contracts (HTLCs) to ensure that the off-chain VTXO exchange and the on-chain/Lightning route are fully atomic.

---

## The Co-existence Model: Ark + MDK

Integrating Ark into our agent architecture alongside Money Dev Kit yields several immediate architectural synergies:

```text
+-------------------------------------------------------------------------+
|                           MDK Agent Wallet                              |
+------------------------------------+------------------------------------+
|       MDK L402 Client Engine       |        Bark Client Daemon          |
|  - Pays L402 micro-invoices        |  - Manages VTXO leaf trees         |
|  - Authenticates paid API requests |  - Participates in periodic rounds |
+-----------------+------------------+-----------------+------------------+
                  |                                    |
                  | Standard Lightning                 | Out-of-Round (arkoor)
                  | (via Ark Gateway)                  | Zero-Fee Transfers
                  v                                    v
+-----------------+------------------+       +---------+------------------+
|           Ark Server               |       |       SISTER AGENT         |
|  (Acts as LDK/LN Gateway & ASP)    |       |  (Same Ark, Same ASP)      |
+------------------------------------+       +----------------------------+
```

### 1. Unified Agent Wallet (Bark Client + MDK Client)
Instead of forcing our autonomous agents to manage complex LDK channel funding, channel rebalancing, or pay high startup routing fees, we should integrate a **Bark client node (`barkd`)** into the MDK Agent Wallet. 
* **Resting State:** The agent’s funds sit securely in high-security **Refresh VTXOs** (which expire after ~30 days, requiring automatic, background refreshes).
* **Onboarding:** A newly deployed agent can immediately receive funds and spend them without any pre-funded Lightning channels.

### 2. Micro-fees for API Invocations (L402)
MDK uses **L402 (HTTP 402 Payment Required)** to protect model APIs, vector search databases, and execution sandboxes.
* When an agent needs to query a paid API protected by MDK, MDK returns an invoice.
* The agent wallet pays this invoice by instructing the local `barkd` client to spend a VTXO into an outgoing Lightning HTLC.
* The Ark server routes this payment over the Lightning Network to the provider, obtains the preimage, and returns it to the agent to satisfy the L402 challenge.
* **Result:** Trustless pay-as-you-go microservices with zero channel capital lockup for the agent.

### 3. Intranet Agent Swarms (Out-of-Round arkoor)
In multi-agent collaborative workflows (e.g., a planner agent hiring an execution agent, a verification agent auditing code), agents must pay one another for subtasks.
* If both agents share the same Ark server, they can pay one another **out-of-round**.
* **Result:** Near-instant, zero-fee, zero-liquidity peer-to-peer transfers. The recipient agent receives the VTXO and continues its work, batching its security refresh at the end of the day or alongside other VTXO expiries.

---

## Pros & Cons for Autonomous Agents

| Category | Ark (Bark) Pros | Ark (Bark) Cons |
|---|---|---|
| **Liquidity & Funding** | **Zero channel management.** No inbound/outbound liquidity constraints. Agents require no upfront capital to start receiving payments. | The Ark Server has a high capital cost to maintain the liquidity pools for funding new rounds and sweeps, which is passed down as fees. |
| **Speed & Finality** | **Out-of-round payments are near-instantaneous** (settling in milliseconds) with zero fee overhead. | Until VTXOs are refreshed in a periodic round, they are subject to a **temporary trust trade-off** requiring 1-of-2 honesty. |
| **Wallet Interactivity** | **Offline receive is supported.** Agents do not need to be online to receive incoming Lightning payments (HTLCs are preimage-secured). | The recipient **must come online** eventually to cooperatively sign and claim the incoming Lightning HTLC before it expires. |
| **Self-Custody** | **Full control.** Pre-signed exit transactions ensure agents can perform an **emergency exit** unilaterally if the server goes offline. | Emergency exits can be expensive if performed during high on-chain fee spikes, as it requires broadcasting multiple tree branches. |
| **Client Overhead** | Lightweight footprint. No persistent peer connections or channel gossip storage required. | Mobile/background agents face OS constraints on scheduled wakes, requiring **delegated refreshes** with a 1-of-n trust model. |

---

## Deployment & Operational Requirements

To successfully deploy and utilize this dual-payment architecture across the OpenAgents network, we need the following infrastructure pieces:

### 1. Ark Service Provider (ASP) Server
An operational `bark` server must be deployed by OpenAgents (or a trusted third-party LSP) containing:
* **gRPC API Daemon:** For coordinating periodic rounds (refreshes) and out-of-round payments with client wallets.
* **Lightning Node Integration:** A connected LDK/Core Lightning/LND instance with funded, active channels to serve as the Lightning Gateway (routing sends and receives).
* **On-Chain Funding Pool:** A Taproot-enabled UTXO wallet with sufficient bitcoin liquidity to fund transaction tree roots and handle optimistic sweeps of expired rounds.

### 2. Bark Client Daemon (`barkd`)
Every agent deployment environment (Pylon, Workspace, or Sandbox) must run `barkd` alongside the agent runtime:
* **Local Database:** To securely store the pre-signed transactions making up the VTXO exit paths (crucial for emergency exits).
* **Scheduled Event Worker:** A background worker task to monitor VTXO lifetimes (~30 days) and automatically queue refreshes.
* **UniFFI Binding Bridge:** Using the `bark-ffi` library to expose the wallet interface to the agent's TypeScript runtime (Effect/Bun).

### 3. Money Dev Kit API Updates
* Expand the `@moneydevkit/agent-wallet` CLI and TypeScript SDK to support a dual-mode provider back-end: `LDK` or `Bark`.
* Support automatic routing decisions: if target address is an Ark address on the same server, route natively via `arkoor`; if it's a standard BOLT11/BOLT12 target, route via the Ark Lightning Gateway.

---

## The Covenants Roadmap

Under current Bitcoin consensus rules, Ark is forced to utilize timelocks and interactive signatures (or delegated co-signers) to manage transactions safely.

If a covenant opcode such as **`OP_CHECKTEMPLATEVERIFY` (CTV)** or `TEMPLATEHASH` were adopted by the Bitcoin network:
1. **Fully Trustless Delegated Refreshes:** Mobile and offline agents could submit scheduled refreshes that co-signers could execute on their behalf with **zero trust required**, because the outputs of those round transactions would be mathematically locked by the blockchain to the user's keys.
2. **Infinite Transaction Tree Scaling:** Trees could scale to millions of leaf users without requiring interactive n-of-n multisigs during round construction.

---

## Strategic Recommendations for OpenAgents

1. **Keep Psionic out of wallet custody.** Let Psionic emit task-verification and performance proof-of-work certificates, and use the MDK-Bark agent wallet as the isolated execution layer for payments.
2. **Integrate `barkd` into Pylon.** Provide our distributed Pylon operators with a built-in LDK/Ark wallet. This allows operators to immediately receive micro-stipends without configuring `payout_destination` or managing custom channels.
3. **Pioneer L402 over Ark.** Deploy a public OpenAgents Ark server to act as a Lightning Gateway for our agents, allowing them to settle L402 paid APIs on-demand with zero local liquidity overhead.

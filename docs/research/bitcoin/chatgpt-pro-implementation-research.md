I completed the survey and saved the full, linked report:

### [Bitcoin and Lightning Implementation Architecture and Tradeoffs](https://chatgpt.com/space/page_6eb6c06414748191a29779d956ef4795)

The report covers full validators, wallet and cryptographic libraries, light clients, indexers, Lightning engines, SDKs, deployment layers, and historical or experimental implementations. **Research checked: 11 November 2026. Scope: BTC mainnet and Lightning settled on BTC.**

There is no authoritative registry of every implementation ever written, so I can’t honestly guarantee every private implementation or abandoned prototype. I have covered the major public implementation families and a substantial long tail, with primary sources and explicit maturity qualifications.

**The most important finding: BDK, LDK, and Bitcoin Core solve different problems.** BDK supplies on-chain wallet mechanisms; LDK supplies the Lightning protocol engine; Bitcoin Core supplies full chain validation. Choosing among them is mostly about deciding which responsibilities your application owns. [GitHub](https://github.com/bitcoindevkit/bdk_wallet?utm_source=chatgpt.com)

## 1. The implementation landscape

These layers need to be compared separately:

| Layer | What it does | Main families |
|---|---|---|
| **Bitcoin validator** | Determines whether blocks and transactions obey consensus | Bitcoin Core, btcd, libbitcoin, Gocoin; Utreexo-based approaches |
| **Validation library** | Embeds validation machinery into another program | libbitcoinkernel; historically libbitcoinconsensus |
| **Wallet engine** | Tracks owned coins, derives addresses, constructs transactions and coordinates signing | BDK, bitcoinj, NBitcoin, libwally, bitcoinjs-lib, Bitcoin-S |
| **Light client** | Obtains wallet/chain information with reduced local verification or storage | Neutrino, Kyoto, Nakamoto, SPV wallet stacks |
| **Indexer** | Makes address, transaction and history queries efficient | electrs, ElectrumX, Fulcrum, Esplora, NBXplorer |
| **Lightning engine** | Maintains channels, commitments, payments and on-chain enforcement | LND, Core Lightning, Eclair, LDK, Electrum, lightning-kmp, Immortan |
| **Lightning assembly** | Packages an engine with storage, networking, wallet and application interfaces | LDK Node, LDK Server, Phoenix, phoenixd, Greenlight, Alby Hub |

The report links the sources for each layer and implementation. A language binding, wallet interface, RPC client or hosted deployment is not automatically another independent protocol implementation.

I treated “no forks” as excluding alternate chains, while allowing BTC-compatible derivatives of a codebase. **One current exception is Knots:** its currently advertised branch follows release notes introducing a backward-incompatible BLAKE2b proof-of-work change. I excluded that branch from the BTC-mainnet recommendations; older Knots releases remain relevant historical policy variants. [bitcoinknots.org](https://bitcoinknots.org/?utm_source=chatgpt.com)

## 2. Bitcoin validators and validation libraries

### The main choices

| Implementation | Important design decision | Resulting tradeoff |
|---|---|---|
| **Bitcoin Core — C++** | Integrates consensus, networking, mempool policy, RPC and optional wallet | A substantial codebase, but the surrounding relationships are already implemented. My default production validation baseline. |
| **btcd — Go** | Independent validator with a separate wallet and reusable btcsuite packages | Good Go composition and implementation diversity; independent consensus compatibility must be maintained. |
| **libbitcoin — C++** | Layered libraries, specialized database design and explicit processing stages | Greater architectural specialization; storage, recovery and benchmark settings require careful interpretation. |
| **Gocoin — Go** | Keeps the UTXO set in RAM and separates the offline wallet | Trades memory capacity for avoiding disk lookups in that path; custom licensing also affects adoption. |
| **Utreexod — Go** | Uses Utreexo proofs instead of retaining a conventional full UTXO set | Lower local state requirements, with proof generation, bandwidth, availability and bootstrap assumptions to consider. |
| **Floresta — Rust** | Combines Utreexo, a watch-only wallet and an Electrum interface | A compact integrated approach; current development also reuses Core’s kernel, while surrounding components remain separately relevant to correctness. |
| **bcoin — JavaScript/native acceleration** | Integrated node, wallet, SPV and library architecture | Convenient JavaScript composition, but the inspected implementation has a consequential current validation-coverage gap. |

Sources for the architectural descriptions: Core, btcd, libbitcoin, Gocoin, Utreexod and Floresta’s own documentation. [bitcoincore.org](https://bitcoincore.org/en/releases/32.1/?utm_source=chatgpt.com)

### Bitcoin Core: integration and compatibility

Core is the baseline I would use when a product needs authoritative local validation. That recommendation is an engineering judgment: it lets the application build above a well-defined consensus boundary.

Two Core decisions are especially useful to study:

- **Pruning:** validate the chain while discarding older block files. This reduces retained history, but can restrict later rescans, historical queries and block serving.
- **AssumeUTXO:** start from a recognized snapshot while historical validation proceeds in the background. The snapshot and subsequent verification are both part of the design. [GitHub](https://github.com/bitcoin/bitcoin/blob/master/doc/design/assumeutxo.md?utm_source=chatgpt.com)

Also keep **relay policy separate from consensus**. A node declining to relay a transaction does not necessarily mean the transaction would be invalid in a block.

### libbitcoinkernel: reuse Core inside another application

**libbitcoinkernel and libbitcoin are unrelated implementation families.**

libbitcoinkernel exposes Core validation through a C-facing interface. That is attractive for another language or runtime because it permits reuse of Core’s machinery. Its current header explicitly says the API is unstable and unversioned, with breaking changes expected. Treat it as a pinned integration project, rather than a settled SDK contract. [GitHub](https://github.com/bitcoin/bitcoin/blob/master/src/kernel/bitcoinkernel.h?utm_source=chatgpt.com)

The older **libbitcoinconsensus** exposes script verification rather than an entire chain validator. Its deprecation makes it a weak starting point for a new general-purpose node architecture. **Kernel-node** is an experimental example of composing a node around the newer kernel. [GitHub](https://github.com/rust-bitcoin/rust-bitcoinconsensus?utm_source=chatgpt.com)

### btcd: modularity exposes shared compatibility risks

btcd’s separation of the node, wallet and reusable packages is useful. It also demonstrates why the dependency graph matters more than the name of the chain backend.

A historical signed-versus-unsigned transaction-version bug caused different BIP68/BIP112 behavior from Core. That was a fixed implementation bug, but an instructive one: unusual integer values and parser behavior belong in differential testing. “Implements the BIPs” is not sufficient evidence of consensus equivalence. [Delving Bitcoin](https://delvingbitcoin.org/t/disclosure-btcd-consensus-bugs-due-to-usage-of-signed-transaction-version/455?utm_source=chatgpt.com)

### libbitcoin: storage architecture is a first-class concern

libbitcoin’s memory-mapped database structures and staged processing architecture make it worth studying independently of whether you adopt it. Its performance claims must be interpreted alongside hardware, indexes, checkpoints and historical validation settings.

The v4 line is currently published as **previews**, distinct from the older stable v3 line. Its license also includes AGPL terms with a project-specific lesser clause; it should not be casually classified as an ordinary permissive library. [GitHub](https://github.com/libbitcoin/libbitcoin-server/releases?utm_source=chatgpt.com)

### Utreexo: reduced state and bootstrap trust are separate decisions

Utreexo can change the representation of spendable-output state without inherently changing BTC consensus. But **Utreexod’s documented default bootstrap also uses a hardcoded accumulator and skips historical initial block download**. Validating history is a separate configuration choice. The project labels itself beta. [GitHub](https://github.com/utreexo/utreexod?utm_source=chatgpt.com)

That distinction is central: accumulator verification, proof availability and trusted bootstrap are three different subjects.

### bcoin: following the chain is weaker than enforcing all its rules

The inspected bcoin script implementation handles witness version 0 while later versions retain forward-compatibility behavior. Together with the outstanding Taproot work, that means I would not select this inspected version as the sole current consensus authority for a new production system. [GitHub](https://github.com/bcoin-org/bcoin/issues/1032?utm_source=chatgpt.com)

### The additional node inventory

The report also covers **Mako, Parity Bitcoin, Toshi, Picocoin, Haskoin-node, Swift Bitcoin, gosuda/bitcoin-rs, rbitcoin, satd, BitcoinWolfe, BitcoinPR and Qubitcoin**.

Their classifications matter. Mako explicitly lacks important current functionality; Parity Bitcoin is archived; several newer Rust projects are experiments. Haskoin-node is a networking/header component, while BitcoinWolfe assembles existing Core-kernel, BDK and LDK components. These should not all be presented as interchangeable production validators. [GitHub](https://github.com/chjj/mako?utm_source=chatgpt.com)

## 3. BDK: the consequential wallet decisions

BDK deserves its own treatment because it is a toolkit for constructing a wallet, rather than a complete wallet product.

### Descriptors make wallet policy explicit

BDK uses descriptors to describe scripts and key derivation. That helps represent receive/change branches, watch-only operation, multisignature and external signing without burying those choices in application conventions.

The practical consequence is that recovery requires understanding the wallet policy and derivation information. A seed alone does not describe every possible multisignature or custom-script wallet. [GitHub](https://github.com/bitcoindevkit/bdk_wallet?utm_source=chatgpt.com)

### Wallet state is separated from networking

The lower-level stack includes transaction graphs, script indexing, chain checkpoints and explicit changes. Network adapters obtain data that the wallet can consume.

This gives an application freedom to choose its runtime and backend. It also means the application must understand what the backend establishes:

| Backend arrangement | Useful property | Remaining concern |
|---|---|---|
| **Your own Core node** | Local validation authority | Node operation, synchronization and available historical data |
| **Electrum server** | Efficient wallet-oriented queries | Query privacy, service availability and completeness |
| **Esplora server** | Convenient HTTP interfaces | Similar server trust/privacy concerns, with different query behavior |
| **Compact-filter client** | Matches watched scripts locally | Peer availability, block-fetch privacy and reduced validation guarantees |

BDK’s backend abstraction makes integration more consistent; it does not make those security properties identical. [Rust](https://docs.rs/bdk_chain/latest/bdk_chain/?utm_source=chatgpt.com)

### Persistence is deliberately an application boundary

BDK exposes changes that the application persists. This makes custom database integration possible, but the application must correctly commit address and wallet state.

A concrete trap: **`bdk_file_store` is explicitly described as intended for testing and development, not production.** An example storage backend should not become the production database by accident. [GitHub](https://github.com/bitcoindevkit/bdk?utm_source=chatgpt.com)

### Recovery scanning is different from routine synchronization

A full scan explores derivation space until a stopping rule is reached. A sync updates scripts already known to the wallet.

That means a fast sync cannot, by itself, establish that a restored wallet has discovered every address previously issued. This is a product-level recovery requirement, not merely a performance setting. [Book of BDK](https://bookofbdk.com/cookbook/syncing/full-scan-vs-sync/?utm_source=chatgpt.com)

### Signing is moving toward caller-owned keys

BDK 3.2 deprecates older Wallet-held signing APIs in favor of caller-owned signing material and interfaces such as `Psbt::sign` and `Wallet::sign_with_signers`. The old APIs were **deprecated, not simply removed**. New transaction-building interfaces have separate instability gates. [Book of BDK](https://bookofbdk.com/release-guide/3.2/notes/?utm_source=chatgpt.com)

This is a valuable architectural choice: transaction discovery and construction can operate separately from secrets. Hardware, separate-process or custom signers become easier to accommodate. [Book of BDK](https://bookofbdk.com/design/signing/?utm_source=chatgpt.com)

**My assessment:** BDK is a strong default for a new custom on-chain wallet when you want control over storage, chain source, signing and application behavior. That control comes with responsibility for integration and recovery.

## 4. Lightning: engines, operating models and maturity

### LND, Core Lightning and Eclair

These all provide a substantial operating system around their channel engine, but make different structural choices.

| Implementation | Documented architectural choice | What you gain and what you accept |
|---|---|---|
| **LND — Go** | Integrated daemon, wallet and RPC service | Convenient gRPC/REST integration and authorization; you adopt its internal wallet, database and process model. |
| **Core Lightning — principally C** | Multiple processes, signer separation and subprocess plugins | Explicit component boundaries and language-independent extensions; more process and hook behavior to understand. |
| **Eclair — Scala/JVM** | Actor-oriented peers, channels and payments | Clear concurrent state ownership and service-oriented structure; JVM scheduling, storage and backend requirements remain operational concerns. |

Sources: the projects’ repositories and architecture/plugin documentation. [GitHub](https://github.com/LightningNetwork/lnd?utm_source=chatgpt.com)

**LND’s database migration is particularly instructive.** A SQL-backed compatibility layer and native SQL tables are different designs. Its migrations are staged by subsystem, so “supports PostgreSQL” does not establish identical behavior throughout the node. [GitHub](https://github.com/lightningnetwork/lnd/blob/master/docs/db_migration_guide.md?utm_source=chatgpt.com)

**CLN’s plugin model is a significant extension boundary.** Business logic can live in another process and language. The practical question is which hooks can delay or influence protocol progress, and how extension failures are isolated.

**Eclair’s actor model is worth studying even if you choose another implementation.** It provides a natural way to give a channel an owner that processes ordered messages. That helps structure concurrency, but still requires careful persistence, backpressure and failover design.

### LDK: the application supplies the operating machinery

LDK provides the Lightning protocol engine as Rust libraries. Its core does not impose one mandatory application runtime. That makes it relevant to mobile, embedded, WASM, servers and custom runtimes. [Lightning Dev Kit Documentation](https://lightningdevkit.org/introduction/?utm_source=chatgpt.com)

Its important components include:

- **ChannelManager:** off-chain channel and payment state.
- **ChannelMonitor:** information needed for on-chain enforcement.
- **PeerManager:** peer-protocol interaction.
- **Graph, router and scorer:** network knowledge and payment-path decisions. [GitHub](https://github.com/lightningdevkit/rust-lightning/blob/main/ARCH.md?utm_source=chatgpt.com)

The host supplies or selects chain notifications, reorg handling, transaction broadcasting, fee estimates, networking, timers, signing, persistence and event processing.

The most consequential contract is persistence. **Reporting success must mean the required channel-monitor update is durable.** Asynchronous persistence has an explicit in-progress/completion mechanism that gates protocol progress. A write queued in memory is not equivalent to a completed durable write. [Rust](https://docs.rs/lightning/latest/lightning/chain/chainmonitor/trait.Persist.html?utm_source=chatgpt.com)

My assessment: raw LDK is compelling when your runtime, storage, signing or product behavior requires that degree of control. Otherwise, it can make you own machinery that does not differentiate your product.

### LDK Node and LDK Server are separate choices

**LDK Node** packages LDK with BDK and surrounding components behind a smaller library API. It is the first embedded option I would evaluate before deciding that raw LDK integration is necessary. [GitHub](https://github.com/lightningdevkit/ldk-node/blob/main/README.md?utm_source=chatgpt.com)

**LDK Server** exposes an LDK Node-based daemon/API. Its current README explicitly says it is not ready or tested for production, with evolving persisted formats and APIs. The maturity of LDK itself cannot be transferred automatically to this newer assembly. [GitHub](https://github.com/lightningdevkit/ldk-server/blob/main/README.md?utm_source=chatgpt.com)

### The other independent Lightning families

**Electrum** has its own Python Lightning implementation. It is a wallet-oriented engine, not just a remote interface for another daemon. Trampoline routing reduces the need for a wallet to process a full routing graph, while introducing routing-service availability and privacy considerations. Its watchtower functionality addresses on-chain monitoring, not complete restoration of live channel state. [GitHub](https://github.com/spesmilo/electrum?utm_source=chatgpt.com)

**lightning-kmp** is ACINQ’s Kotlin Multiplatform engine, separate from Scala Eclair. Phoenix and phoenixd belong to this family. The wallet-oriented send/receive model reduces channel-operation work through liquidity/routing services, at the cost of provider dependency, fees and different privacy assumptions. That dependency is distinct from custody of the channel’s keys. [GitHub](https://github.com/ACINQ/lightning-kmp/blob/master/README.md?utm_source=chatgpt.com)

**Immortan** is a separate lightweight Scala engine emphasizing private channels and per-peer identities. Its optional hosted-channel extensions are outside the standard BTC-enforced channel scope of this report. [nbd-wtf/immortan · GitHub](https://github.com/nbd-wtf/immortan?utm_source=chatgpt.com)

### Feature support needs a release and a protocol context

Several current details differ from older comparison charts:

| Implementation | Verified qualification |
|---|---|
| **Core Lightning** | Splicing is enabled by default from **26.04**. The old experimental-splicing setting is deprecated. Dual funding remains a separate experimental setting. |
| **LND 0.21.6** | Onion-message infrastructure exists, while the complete native BOLT12 offers flow remains under the open **0.22** roadmap. |
| **Eclair 0.14** | Finalized splicing, simple Taproot private-channel support and experimental zero-fee commitments come with backend and migration requirements. |
| **LDK 0.2** | Splicing and initial asynchronous-payment capabilities have protocol, interoperability and upgrade qualifications. |

These findings come from current official release notes, deprecations and development tracking. [docs.corelightning.org](https://docs.corelightning.org/docs/deprecated-features?utm_source=chatgpt.com)

“Taproot support” can mean an on-chain wallet output, a channel funding output or a commitment construction. “BOLT12 support” can mean encoding, message transport, receiving, paying or a complete offer flow. A useful comparison must say which.

### Assemblies and historical implementations

The report separately covers:

- **Greenlight:** hosted CLN computation with a user-controlled signing component.
- **Breez SDK Greenlight:** now officially deprecated and unmaintained; successor protocols must be assessed separately.
- **Alby Hub, Lampo and Mutiny:** applications or assemblies using LDK, with different operating environments and maturity.
- **LNDK:** an experimental companion that can add functionality around LND; it does not establish native LND feature completeness.
- **VLS:** validating signer infrastructure rather than another channel engine. [Greenlight Documentation](https://blockstream.github.io/greenlight/?utm_source=chatgpt.com)

Historical and experimental protocol work includes **Ptarmigan, NLightning, DotNetLightning, Node-Lightning, lit, Thunder and LNP Node**. Their incomplete, archived, historical or unestablished production status is recorded individually in the report. [GitHub](https://github.com/nayutaco/ptarmigan?utm_source=chatgpt.com)

## 5. The broader Bitcoin library inventory

The full report gives each project a linked description and tradeoff. The most useful groupings are:

| Ecosystem | Implementations covered | Main distinction to evaluate |
|---|---|---|
| **Rust** | rust-bitcoin, rust-miniscript, BDK | Primitives versus spending-policy tooling versus stateful wallet mechanisms |
| **C/native** | libsecp256k1, libwally-core, GDK, libbtc, breadwallet/BRD core | Narrow cryptography versus wallet primitives versus integrated SDK/SPV behavior |
| **Go** | btcsuite, btcwallet, Neutrino | Reusable protocol/wallet packages versus full or lightweight chain access |
| **Java/JVM/.NET** | bitcoinj, Drongo, NBitcoin, Bitcoin-S, bitcoin-kmp | Integrated wallet behavior, runtime conventions and native-crypto deployment choices |
| **JavaScript/TypeScript** | bitcoinjs-lib, scure-btc-signer, noble-curves, Bitcore, bcoin | Focused transaction/signing tools versus complete application ecosystems |
| **Python** | python-bitcoinlib, python-bitcointx, btclib, embit, pycoin, bitcoinlib | Low-level primitives, embedded signing, educational clarity or integrated wallet/database behavior |
| **Other environments** | bitcoinrb, bitcoin-ruby, Bit-Wasp bitcoin-php, Haskoin, Swift Bitcoin, BitcoinCore.Swift, Armory | Language-native integration, platform limitations and differing maintenance histories |

Three decisions stand out across those libraries.

**rust-bitcoin deliberately does not claim to be a full consensus validator.** That boundary is valuable: implementing Bitcoin data types is different from deciding whether the entire chain is valid. [GitHub](https://github.com/rust-bitcoin/rust-bitcoin?utm_source=chatgpt.com)

**NBitcoin deliberately maintains managed cryptographic code.** Its rationale is that distributing reproducible native binaries across .NET environments creates significant friction. This trades native packaging work for maintaining a faithful port. [GitHub](https://github.com/MetacoSA/NBitcoin/blob/master/NBitcoin.Secp256k1/README.md?utm_source=chatgpt.com)

**Small or portable code is not automatically equivalent code.** libsecp256k1 focuses narrowly on cryptography; libwally builds wallet primitives around a native interface; embit targets constrained Python/MicroPython environments. Each is useful because of its chosen boundary, and each leaves other responsibilities to its caller. [GitHub](https://github.com/bitcoin-core/secp256k1?utm_source=chatgpt.com)

Two classification cautions also matter:

- bitcoinj’s wallet tooling is primarily associated with SPV, but its codebase also contains full-pruned validation machinery. “SPV-only” would be too strong. [GitHub](https://github.com/bitcoinj/bitcoinj/blob/master/core/src/main/java/org/bitcoinj/core/PeerGroup.java?utm_source=chatgpt.com)
- Armory and breadwallet/BRD deserve historical coverage, but an old advertised release or an unmaintained application should not be silently converted into a current production recommendation. [btcarmory.com](https://btcarmory.com/docs/faq?utm_source=chatgpt.com)

## 6. Light clients and indexers make different tradeoffs

**Neutrino, Kyoto and Nakamoto** represent lightweight chain-access approaches. Compact-filter clients move script matching toward the client, but still depend on peers for data and do not perform every full-node validation step. [GitHub](https://github.com/lightninglabs/neutrino?utm_source=chatgpt.com)

For query infrastructure, the important comparison is the workload:

- **electrs:** smaller indexes with more dependence on fetching data from Core.
- **Fulcrum / ElectrumX:** arbitrary-address query services with their own storage and operational requirements.
- **Blockstream electrs / Esplora:** broader indexes and APIs, trading disk for query capability and fewer backend reads.
- **Electrum Personal Server / bwt / NBXplorer:** track known wallets or derivations rather than necessarily indexing every address.
- **Blockbook, mempool’s indexer and Haskoin Store:** additional backend designs covered in the report. [GitHub](https://github.com/romanz/electrs?utm_source=chatgpt.com)

A private wallet watching known descriptors has a different database problem from a public explorer serving arbitrary queries. **Choose the indexer from the query workload**, then benchmark initial indexing, steady-state writes, historical lookup latency, disk amplification and reorg recovery.

Also distinguish four concepts that are often conflated:

1. A **pruned full node** discards historical data after validation.
2. An **SPV client** performs fewer validation checks.
3. A **Utreexo validator** changes authenticated UTXO-state storage.
4. An **indexer** supplies convenient queries, with its own chain-source assumptions.

## 7. The most reusable engineering lessons

### Consensus compatibility includes parsing and edge cases

Independent implementations provide diversity and can expose mistakes. They also need to reproduce exact behavior in obscure cases.

The btcd transaction-version disclosure illustrates this well. Differential testing should cover integer boundaries, parser limits, activation history, serialization and rejected inputs—not just normal transactions. [Delving Bitcoin](https://delvingbitcoin.org/t/disclosure-btcd-consensus-bugs-due-to-usage-of-signed-transaction-version/455?utm_source=chatgpt.com)

### Lightning persistence is part of protocol correctness

A routing graph can often be rebuilt. Current commitment and revocation-related state has different consequences if lost.

LND’s static channel backups illustrate the distinction: they are a funds-recovery mechanism involving peers and channel closure, not a current database that resumes live operation. A seed is not the whole off-chain recovery story. [docs.lightning.engineering](https://docs.lightning.engineering/lightning-network-tools/lnd/disaster-recovery?utm_source=chatgpt.com)

My design recommendation is to classify every store as **authoritative, reconstructible or recovery-only**, and make that distinction visible in the API.

### Key isolation needs signing policy

A separate process or hardware device can prevent some key-extraction paths while still signing a harmful request from a compromised host.

VLS is useful to study because it puts state and policy checks at the signing boundary. The signer’s own state and recovery then become part of the architecture. [vls.tech](https://www.vls.tech/docs/latest/overview/intro/?utm_source=chatgpt.com)

### Parallelize independent channels before introducing multiple writers

This is my engineering conclusion from the actor, process and library models: give each channel state an authoritative owner, and explicitly fence that owner during failover.

A replicated database alone does not prove that two runtimes cannot act on the same channel. Likewise, a backup being byte-for-byte intact does not prove it is current enough to resume safely.

### Privacy is a property of the whole data path

A uniform wallet API does not equalize the privacy of its backends. Querying watched scripts, fetching matching blocks and relying on a routing provider reveal different information.

Similarly, timely chain access is particularly important for Lightning’s on-chain enforcement. An integration should test stale backends, missed notifications and reorgs, not merely successful synchronization. [GitHub](https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki?utm_source=chatgpt.com)

### Cryptography can fail outside the signature algorithm

Two case studies belong in any architectural review:

- **Milk Sad:** weak entropy in specific historical Libbitcoin Explorer seed-generation behavior. Strong elliptic-curve mathematics cannot repair a predictable seed.
- **bitcoinjs-lib’s October 2026 Merkle advisory:** duplicate-transaction handling could preserve a matching Merkle root. Checking a commitment is not the same as checking every validity rule. [milksad.info](https://milksad.info/disclosure.html?utm_source=chatgpt.com)

These are different failures and should not be generalized into blanket claims about every component in either ecosystem.

### Language and database labels are insufficient performance evidence

“Rust,” “C,” “PostgreSQL,” “memory mapped” and “in memory” describe mechanisms. They do not establish comparable throughput.

A meaningful comparison must hold constant the validation work, bootstrap assumptions, hardware, indexes and durability settings. Lightning comparisons must also distinguish attempted payments from successful payments and local execution speed from liquidity-constrained routing success.

I did not find a common reproducible benchmark covering this entire landscape, so the report does not manufacture a fastest-to-slowest ranking.

## 8. What I would evaluate first

These are my recommendations based on the documented boundaries above:

| Goal | Starting point |
|---|---|
| **Authoritative BTC validation** | Bitcoin Core as a separate process |
| **A new custom on-chain wallet** | BDK with a deliberately chosen backend and signer |
| **Embedded Lightning** | LDK Node first; raw LDK when deeper control is justified |
| **A conventional Lightning service** | Compare LND, CLN and Eclair against your operating and extension requirements |
| **Native C/Linux integration** | CLN plugins, libwally and libsecp256k1 |
| **A new language or runtime** | Core RPC initially; narrow C interfaces; pinned libbitcoinkernel experiments where needed |
| **Resource-constrained validation research** | Utreexod and Floresta, including proof availability and bootstrap behavior |
| **Public address/history queries** | Fulcrum, ElectrumX and Esplora-family servers on a representative workload |
| **Merchant wallet tracking** | NBXplorer or another descriptor-scoped backend |

For implementation study, I would prioritize **Core for consensus boundaries, BDK for wallet composition, LDK for explicit integration contracts, CLN for process/plugin structure, Eclair for actor ownership, and LND for daemon/API and database migration design**.

The full report contains the individual project links, historical inventory, current-release qualifications, licensing distinctions, and additional signing/mining infrastructure. It is an architecture and status review with targeted source inspection; it does not claim an independent security audit or a successful full-chain test of every listed node.

---

**Research checked 11 November 2026. Scope: Bitcoin’s SHA256d BTC mainnet and Lightning channels settled on that chain.**

This report maps the public implementation families, explains the engineering choices that distinguish them, and separates deployable systems from libraries, wrappers, experimental projects, and historical work. It covers the major independent engines and a substantial long tail. There is no authoritative registry of every implementation ever written, and private code cannot be surveyed; this is a source-backed public inventory, not a claim that every abandoned toy repository has been found.

**The central finding:** the choice is less about programming language than about where consensus validation, keys, chain-data trust, durable channel state, and operational responsibility live. Bitcoin Core, BDK, LDK, and an Electrum server solve different parts of that problem.

Project descriptions below are drawn from primary repositories, release notes, architecture documents, and security disclosures. Recommendations and consequences are my engineering assessment. “Current source” means the inspected development branch unless a release is named; it does not imply that every described feature has shipped.

# What belongs in the comparison

| Layer | Responsibility | Representative implementations |
| - | - | - |
| Bitcoin full validator | Decide whether blocks and transactions obey Bitcoin consensus | Bitcoin Core, btcd, libbitcoin, Gocoin; Utreexo-based implementations with different state arrangements |
| Validation library | Embed selected validation machinery in another application | libbitcoinkernel; historically libbitcoinconsensus |
| Wallet engine | Track owned scripts and coins, derive addresses, build and sign transactions | BDK, bitcoinj, NBitcoin, libwally, bitcoinjs-lib, Bitcoin-S |
| Light client | Learn chain and wallet state with less local validation or storage | Neutrino, Kyoto, Nakamoto, SPV wallet engines |
| Indexer and query service | Answer address, history, transaction, and balance queries over a chain backend | electrs, ElectrumX, Fulcrum, Esplora, NBXplorer |
| Lightning protocol engine | Maintain channel commitments, route or make payments, and enforce on-chain outcomes | LND, Core Lightning, Eclair, LDK, Electrum, lightning-kmp, Immortan |
| Lightning assembly or product | Package a protocol engine with runtime, storage, wallet, and application interfaces | LDK Node, LDK Server, Phoenix, phoenixd, Greenlight, Alby Hub, Lampo |
| Signing or mining infrastructure | Hold keys, enforce signing policy, negotiate mining work, or drive hardware | libsecp256k1, HWI, VLS, Stratum V2 implementations, ckpool |

A code fork that continues to implement BTC is not automatically an excluded blockchain fork. Conversely, a project’s name or historical Bitcoin compatibility does not prove that its current branch remains compatible. Multichain libraries are covered only for their BTC functionality. Bitcoin Cash, BSV, Litecoin, Liquid/Elements chain functionality, Spark, RGB extensions, and custodial hosted-channel balances are outside this comparison.

**A current Knots caveat:** older Bitcoin Knots releases belong to the history of BTC policy variants. The currently advertised 29.5 branch follows release notes introducing a backward-incompatible BLAKE2b proof-of-work change. That branch is excluded from the BTC-mainnet recommendations here. Sources: [official site](https://bitcoinknots.org/), [29.4.1 consensus-change notes](https://github.com/bitcoinknots/bitcoin/blob/v29.4.1.knots20260508/doc/release-notes.md), [29.5 notes](https://github.com/bitcoinknots/bitcoin/blob/v29.5.knots20260508/doc/release-notes.md).

# Bitcoin full validators

## Bitcoin Core

Bitcoin Core is the default baseline I would choose for an application that needs authoritative local Bitcoin validation. Its C++ codebase combines the validator, P2P networking, mempool and relay policy, RPC, and optional wallet/GUI. This creates a large integration surface, but also avoids asking an application team to recreate the relationships between those components. The official release checked was [32.1](https://bitcoincore.org/en/releases/32.1/).

Three distinctions matter:

- **Pruning changes retained history, not the definition of full validation.** A pruned node can validate the chain while discarding old block files. Historical rescans, indexers, and serving old blocks may require data it no longer retains.

- **AssumeUTXO changes the bootstrap schedule.** It can bring a snapshot-based chainstate online while historical validation proceeds in the background. A recognized snapshot and later verification are part of the design, so describe both rather than calling it either ordinary initial synchronization or permanent blind trust.

- **Relay policy is not consensus.** A transaction can be consensus-valid while a node declines to relay it. Different mempool settings therefore do not necessarily imply different chains.

For most products, running Core as a separate process offers a useful failure and upgrade boundary. The application speaks a stable operational interface rather than linking all consensus internals into its own process. Sources: [Core repository](https://github.com/bitcoin/bitcoin), [AssumeUTXO design](https://github.com/bitcoin/bitcoin/blob/master/doc/design/assumeutxo.md).

## libbitcoinkernel and the older libbitcoinconsensus

**libbitcoinkernel is Bitcoin Core validation being exposed as a library; it is not the independent libbitcoin project.** Its C-facing API can create validation contexts, process blocks, and expose block/index data. This is attractive for a new language or runtime that can call C, because it offers reuse of Core’s implementation instead of a new consensus rewrite.

The important qualification is explicit in the [current header](https://github.com/bitcoin/bitcoin/blob/master/src/kernel/bitcoinkernel.h): the API is unversioned and unstable, breaking changes are expected, and it is not yet included in Core releases. An integration needs a pinned revision, lifecycle and ownership rules, and a plan for API changes. A C ABI does not make the implementation pure C or remove its C++ build requirements.

The older [libbitcoinconsensus wrapper](https://github.com/rust-bitcoin/rust-bitcoinconsensus) exposes script verification, not a full block/chain validator. Its deprecation and removal from recent Core releases make it a poor foundation for a new general-purpose node architecture.

[Kernel-node](https://github.com/kernel-node/kernel-node) demonstrates a Rust node around the Core kernel. It is useful evidence that this composition is possible, but its own experimental status and missing operational capabilities should not be confused with a production replacement for bitcoind.

## btcd and btcsuite

[btcd](https://github.com/btcsuite/btcd) is an independent Go full validator. Its wallet is deliberately separate: btcwallet and reusable packages supply wallet, wire, script, crypto, database, and RPC functionality.

The benefit is a modular Go ecosystem with components that can be embedded in other applications. The cost is that consensus compatibility must be maintained independently, and apparently small shared packages can become critical dependencies far beyond the btcd daemon itself.

Two historical failures make that cost concrete. A [witness parsing bug](https://github.com/btcsuite/btcd/pull/1896) imposed an inappropriate limit before higher-level validation. A later [signed transaction-version disclosure](https://delvingbitcoin.org/t/disclosure-btcd-consensus-bugs-due-to-usage-of-signed-transaction-version/455) showed that BIP68/BIP112 comparisons differed from Core for negative versions. These were fixed bugs, not proof that Go is unsuitable. They demonstrate why differential testing must cover parsing and unusual integer values, and why using Core as a chain backend does not eliminate every consensus-sensitive library elsewhere in a Lightning application.

## libbitcoin

[libbitcoin](https://libbitcoin.org/) is an independent C++ implementation family: system, database, network, node, server, and explorer components. It is separate from both Bitcoin Core and libbitcoinkernel.

Its architecture makes storage and scheduling explicit design subjects. The [database design](https://libbitcoin.org/architecture/database/index.html) uses memory-mapped structures, append-oriented data bodies, and comparatively small heads/indexes. The [node architecture](https://libbitcoin.org/architecture/node/index.html) coordinates processing stages through “chasers” and an event system. These choices aim to control write amplification, recovery, and parallel work.

The tradeoff is specialization: hardware, memory mapping, storage layout, recovery behavior, and configuration materially affect results. Benchmarks that use checkpoints, milestones, or different historical script-validation work cannot be compared directly with a default Core initial sync.

As of this review, v4 work is published as **preview releases**, while the older stable line is v3. Current server work spans several client-facing protocols, so describing the project solely as an old ZeroMQ server is incomplete. Conversely, an advertised API should not be assumed to be a perfect substitute for every Core, Electrum, or Esplora client expectation. Sources: [server releases](https://github.com/libbitcoin/libbitcoin-server/releases), [server repository and license](https://github.com/libbitcoin/libbitcoin-server).

The license includes AGPL terms with the project’s additional exception/clause. Evaluate the exact component and license text for the intended integration rather than reducing it to a generic permissive or LGPL label.

## Gocoin

[Gocoin](https://github.com/piotrnar/gocoin) is a separate Go full node with an offline wallet. Its deliberate choice to keep the UTXO set in RAM trades memory capacity for avoiding disk lookups in that path; current instructions call for substantial RAM, including a 32 GB minimum configuration.

Its [custom license](https://github.com/piotrnar/gocoin/blob/master/LICENSE) is an important practical distinction: noncommercial use and certain research are permitted, while commercial derived products and uses covered by its restrictions require written licensing. This is source-available software, not a drop-in assumption of MIT/BSD-style commercial freedom.

My assessment: useful to study for its data structures and deliberately narrow architecture, but memory requirements, maintainer concentration, and licensing belong in any adoption decision.

## Utreexod and Floresta

[Utreexod](https://github.com/utreexo/utreexod) is a Go, btcd-derived node using Utreexo accumulators. The accumulator permits validation without locally retaining the conventional full UTXO database. The system instead needs proofs and infrastructure to supply them. This changes storage, bandwidth, proof-generation, and availability requirements without inherently requiring a change to BTC consensus.

There is a second, separate choice in the implementation: **the documented default bootstrap uses a hardcoded accumulator and skips historical initial block download**. A configuration for validating history is available. A review must distinguish that bootstrap assumption from the mathematical properties of Utreexo itself. The project labels the software beta and advises using only negligible amounts on mainnet.

[Floresta](https://github.com/getfloresta/Floresta) combines a Rust library and daemon, Utreexo-based resource reduction, a watch-only wallet, and an Electrum interface. Current development documentation uses rust-bitcoinkernel/Core validation. That reduces the need for an independently maintained script implementation; it does not automatically verify every surrounding networking, proof, chainstate, or persistence component. Development-branch architecture should also be distinguished from what a particular released binary contains.

For both projects, the engineering question is not merely “how little disk?” It is “who generates proofs, how are they retrieved, what happens when peers withhold them, and what assumptions bootstrap the state?”

## bcoin

[bcoin](https://github.com/bcoin-org/bcoin) implements a broad Node.js/JavaScript stack with native acceleration: node, SPV, wallet, and reusable components. That is a powerful integrated model for a JavaScript application.

However, the inspected [script interpreter](https://github.com/bcoin-org/bcoin/blob/master/lib/script/script.js) handles witness version 0 and treats later witness versions as forward-compatible unless a policy flag rejects them. The [Taproot issue](https://github.com/bcoin-org/bcoin/issues/1032) remains relevant. I would therefore not present this inspected implementation as a complete current Bitcoin consensus validator or use it as the sole validation authority for a new production deployment. Being able to follow the current chain is weaker than enforcing every activated spending rule.

# Additional Bitcoin node and networking projects

These include experimental validators, historical projects, and current networking components. They belong in an implementation survey, but listing them is not an endorsement for protecting production funds.

| Project | Design decision | Status and implication |
| - | - | - |
| [Mako](https://github.com/chjj/mako) | Portable C, with a low-level library separated from node I/O | Explicitly incomplete: Taproot and important wallet/RPC functionality are missing. Valuable native-code study material. |
| [Parity Bitcoin](https://github.com/paritytech/parity-bitcoin) | Independent Rust implementation | Archived historical project. Its old mainnet support is not evidence of current rule coverage. Only its BTC history is in scope. |
| [Toshi](https://www.coinbase.com/blog/introducing-toshi-an-open-source-bitcoin-node-for-developers) | Ruby and PostgreSQL, emphasizing a developer query backend | Historical architecture, not a verified contemporary production validator. |
| [Picocoin](https://github.com/jgarzik/picocoin) | Small C libraries, wallet and relay components | Historical and incomplete; avoid treating every component as a full node. |
| [Haskoin-node](https://github.com/jprupp/haskoin-node) | Haskell P2P/header work and RocksDB storage | Its README explicitly separates header downloading from full transaction validation. |
| [Swift Bitcoin](https://github.com/swift-bitcoin/swift-bitcoin) | Swift concurrency/actors, networking and embedded storage, native crypto reuse | Experimental node roadmap; library usefulness does not establish mainnet node readiness. |
| [gosuda/bitcoin-rs](https://github.com/gosuda/bitcoin-rs) | Embeddable Rust node work with native/kernel validation choices | Explicit experimental scope rather than a demonstrated production parity claim. |
| [rbitcoin](https://github.com/rbitcoin-org/rbitcoin) | Rust archive/database design with integrated query interfaces | Experimental; milestone-based skipping of historical script work affects trust and benchmark comparisons. |
| [satd](https://github.com/epochbtc/satd) | Rust node experiments with native validation and Core-kernel cross-checking | Interesting differential-validation design; production equivalence remains unestablished. |
| [BitcoinWolfe](https://github.com/refined-element/BitcoinWolfe) | Combines a Core-kernel node with BDK and LDK | An integration experiment, not an additional independent Lightning engine. |
| [BitcoinPR](https://github.com/BitcoinPR/BitcoinPR) | Rust node and database/indexing work | Experimental claims require code and operational verification before adoption. |
| [Qubitcoin](https://github.com/subfrost/qubitcoin) | Rust work aiming at close Bitcoin compatibility | A current research candidate, not independently established production consensus equivalence. |

The last few projects have considerably weaker deployment evidence than the established implementations. Their inclusion is primarily to make the long tail visible. [Bitcoin Inquisition](https://github.com/bitcoin-inquisition/bitcoin), meanwhile, is a signet consensus-research vehicle and is excluded as a BTC-mainnet alternative.

# BDK and the on-chain wallet stack

## What BDK actually supplies

[BDK](https://github.com/bitcoindevkit/bdk_wallet) is a Rust on-chain wallet toolkit. It is neither a Bitcoin full node nor a Lightning implementation. Its high-level wallet layer builds on lower-level chain, transaction-graph, descriptor, and cryptographic libraries.

A useful dependency model is:

| Component | Job | Boundary that remains outside it |
| - | - | - |
| rust-bitcoin | Bitcoin data types, encodings, keys, transactions, and related primitives | Not an independently sufficient consensus validator |
| rust-miniscript | Structured spending policies, descriptors, and script satisfaction | Does not discover wallet transactions or operate a node |
| bdk_core / bdk_chain | Chain checkpoints, transaction graphs, indexing, and changes | Application decides data sources and persistence |
| bdk_wallet | Address derivation, wallet accounting, coin selection, transaction/PSBT construction | Application integrates storage, synchronization, signing, and user policy |
| Backend adapters | Fetch chain data through Core RPC, Electrum, Esplora, or other integrations | The backend’s privacy, availability, and validation assumptions remain relevant |

Sources: [BDK component repository](https://github.com/bitcoindevkit/bdk), [bdk_chain documentation](https://docs.rs/bdk_chain/latest/bdk_chain/), [rust-bitcoin scope warning](https://github.com/rust-bitcoin/rust-bitcoin), [rust-miniscript](https://github.com/rust-bitcoin/rust-miniscript).

## Its consequential design decisions

**Descriptors make wallet policy explicit.** A descriptor records how scripts and keys relate, rather than leaving wallet recovery to an implicit collection of address conventions. This supports watch-only operation, multisignature and hardware-signing workflows. The application still needs to preserve the relevant descriptor/policy and derivation information; a seed does not by itself describe every possible multisignature or custom-script wallet.

**The chain model is separate from transport.** BDK can consume updates without embedding one mandatory network service or asynchronous runtime in the wallet logic. That makes it adaptable to a desktop application, mobile wallet, server, or custom runtime. It also means choosing a backend is an architectural decision, not a detail hidden by the toolkit.

**Changes are explicit.** TxGraph, IndexedTxGraph, LocalChain, and ChangeSet-oriented persistence let an application decide how durable state is stored. The benefit is composability. The cost is responsibility for committing changes and handling recovery correctly. In particular, the current lower-level repository describes bdk_file_store as for testing/development rather than production use. A demo backend should not silently become the production database.

**Discovery and synchronization are different operations.** A full scan searches derivation space until its stopping rule is met; a sync updates scripts already known to the wallet. A fast sync of known scripts cannot prove that a recovered wallet has discovered every previously issued address. Sources: [full scan versus sync](https://bookofbdk.com/cookbook/syncing/full-scan-vs-sync/), [BDK components and persistence caveat](https://github.com/bitcoindevkit/bdk).

**Signing is being separated from wallet state.** The [3.2 release notes](https://bookofbdk.com/release-guide/3.2/notes/) deprecate older Wallet-held signing APIs in favor of caller-owned signers and PSBT-oriented signing. This supports explicit control over where secrets live and makes hardware, process-separated, or custom signing easier to integrate. Newly introduced transaction-building APIs have their own instability gates; the release should not be read as declaring every new interface stable. Sources: [signing design](https://bookofbdk.com/design/signing/), [3.2 notes](https://bookofbdk.com/release-guide/3.2/notes/).

My assessment: **BDK is a strong default when building a new custom on-chain wallet.** Its modularity is especially useful when the product’s runtime, database, signer, or chain source is unusual. Budget integration effort for address issuance, recovery, backend privacy, reorg handling, and durable storage. BDK language bindings expose the same engine; they are not additional Bitcoin implementations. Do not assume that the existence of mobile bindings means BDK itself supplies a stable general-purpose C API.

# Wallet and Bitcoin primitive libraries

A library can implement transaction serialization, signature checking, PSBTs, or scripts without validating the entire Bitcoin chain. “Supports Bitcoin” must therefore be followed by “which layer?”

## Rust and native libraries

| Project | Distinctive choice | Tradeoff or limitation |
| - | - | - |
| [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin) | Reusable Rust Bitcoin types and primitives, including no_std-oriented use | Explicitly warns against treating it as a full consensus implementation. Build higher-level policy and chain services separately. |
| [rust-miniscript](https://github.com/rust-bitcoin/rust-miniscript) | Typed analysis and construction of structured spending policies | Makes many policies easier to reason about, but not every arbitrary script becomes safe automatically. Its archived GitHub location reflects a repository move, not project abandonment. |
| [libsecp256k1](https://github.com/bitcoin-core/secp256k1) | Focused C elliptic-curve library with careful secret-dependent operations and optional modules | Excellent narrow crypto boundary; it does not manage wallets, transaction intent, or channel state. |
| [libwally-core](https://github.com/ElementsProject/libwally-core) | C wallet primitives and language bindings across native/mobile/WASM environments | Good C ABI building block, but application orchestration remains yours. Only BTC functionality is in scope here. |
| [GDK](https://github.com/Blockstream/gdk) | A higher-level SDK shaped around Green wallet functionality | More integrated wallet behavior, at the cost of service and product conventions that a low-level library does not impose. BTC mode only. |
| [breadwallet / BRD core](https://github.com/breadwallet/breadwallet-core) | Independent C SPV/wallet engine with Java and Swift integration | Historical mobile P2P architecture; later apps offered server-assisted synchronization. The [old iOS app](https://github.com/breadwallet/breadwallet-ios) is explicitly unmaintained; current C-core production suitability was not established. BTC component only. |
| [libbtc](https://github.com/libbtc/libbtc) | Small native C Bitcoin components and lightweight wallet/P2P work | Attractive footprint and embeddability; current feature coverage and maintenance must be verified for the specific intended use. |
| [btcsuite / btcwallet](https://github.com/btcsuite/btcwallet) | Separate Go wallet, wire, script, key and RPC packages | Natural Go composition; importing a package also imports its parser and compatibility assumptions. |

## JVM and .NET

| Project | Distinctive choice | Tradeoff or limitation |
| - | - | - |
| [bitcoinj](https://github.com/bitcoinj/bitcoinj) | Java wallet, peer networking, and SPV architecture | Wallet tooling primarily uses SPV. Full-pruned validation classes also exist, but that does not establish a current consensus-complete Core replacement. |
| [Drongo](https://github.com/sparrowwallet/drongo) | Java Bitcoin library shaped by Sparrow’s needs, with bitcoinj lineage | Useful desktop-wallet components without adopting an entire old wallet stack; application lifecycle remains external. |
| [NBitcoin](https://github.com/MetacoSA/NBitcoin) | Broad C#/.NET Bitcoin primitives, PSBT, wallet and RPC tooling | Strong fit for .NET. The managed secp256k1 port avoids native deployment difficulties while requiring faithful maintenance of the port. NBitcoin.Altcoins is excluded. |
| [Bitcoin-S](https://github.com/bitcoin-s/bitcoin-s) | Scala modules for keys, wallet, chain/light-client work, RPC and additional protocols | Broad composable toolkit with JVM/runtime dependencies. Its LND/Eclair clients are integrations, not another Lightning engine. |
| [bitcoin-kmp](https://github.com/ACINQ/bitcoin-kmp) | Kotlin Multiplatform transaction, script, key, PSBT and Taproot primitives | Shared Kotlin code across targets; target maturity differs. It is not an independently sufficient full node. |

bitcoinj’s [PeerGroup implementation](https://github.com/bitcoinj/bitcoinj/blob/master/core/src/main/java/org/bitcoinj/core/PeerGroup.java) documents the distinction between SPV and full-pruned operation; the classification here describes its principal wallet usage rather than claiming the latter code does not exist.

NBitcoin’s [crypto design rationale](https://github.com/MetacoSA/NBitcoin/blob/master/NBitcoin.Secp256k1/README.md) is particularly instructive: the project chose a managed port because reproducibly building and distributing native binaries across users’ environments was a substantial burden. That exchanges foreign-library packaging work for port-maintenance work. Neither choice makes cryptographic correctness automatic.

## JavaScript and TypeScript

| Project | Distinctive choice | Tradeoff or limitation |
| - | - | - |
| [bitcoinjs-lib](https://github.com/bitcoinjs/bitcoinjs-lib) | Transaction and PSBT tooling; key-management packages separated | Flexible browser/server composition and smaller dependency choices, with wallet synchronization and custody left to the application. |
| [scure-btc-signer](https://github.com/paulmillr/scure-btc-signer) | Focused transaction signing and modern script/PSBT support | Deliberately narrower than a complete wallet or node; applications still provide chain state and policies. |
| [noble-curves](https://github.com/paulmillr/noble-curves) | Portable JavaScript cryptographic implementations | Avoids native compilation, but JIT, garbage collection and runtime behavior complicate timing guarantees. |
| [Bitcore](https://github.com/bitpay/bitcore) | A family of BTC libraries, P2P components, wallet clients/services and explorer infrastructure | Convenient ecosystem assembly; count the distinct layers, not each package as a new consensus engine. |
| [bcoin](https://github.com/bcoin-org/bcoin) | Integrated JavaScript node/wallet/library model | See the current validation-coverage caveat in the full-node section. |

## Python

| Project | Distinctive choice | Tradeoff or limitation |
| - | - | - |
| [python-bitcoinlib](https://github.com/petertodd/python-bitcoinlib) | Low-level Core-like structures, scripts and RPC tools | Useful explicit primitives and research tools; not a full chain-validation authority. |
| [python-bitcointx](https://github.com/Simplexum/python-bitcointx) | Transaction-focused extension with PSBT/native crypto integration and less networking scope | Its Python script machinery should not be assumed to perfectly reproduce Core consensus. |
| [btclib](https://github.com/btclib-org/btclib) | Typed Bitcoin and cryptographic code suited to understanding and experimentation | The project describes beta status; evaluate native crypto and secret-handling paths separately from educational clarity. |
| [embit](https://github.com/diybitcoinhardware/embit) | Python/MicroPython wallet and signing primitives for constrained devices | Memory and backend choices support embedded use; resource limits and platform-specific security are application concerns. |
| [pycoin](https://github.com/richardkiss/pycoin) | Key, script, transaction and command-line tooling | Flexible toolkit rather than an integrated modern wallet service. |
| [bitcoinlib by 1200wd](https://github.com/1200wd/bitcoinlib) | Higher-level wallet/database/provider integrations | More built-in behavior and dependencies; AGPL licensing. It is a different project from python-bitcoinlib. |

## Other language ecosystems and bindings

| Project | Distinctive choice | Tradeoff or limitation |
| - | - | - |
| [Armory](https://btcarmory.com/docs/faq) | Desktop wallet with Core integration, separate ArmoryDB, offline signing and multisignature workflows | Physical key separation and flexible cold storage add database/software complexity. The [advertised 0.96.5 release](https://btcarmory.com/0.96.5-release/) dates to December 2018; this alone does not prove all later development stopped. |
| [bitcoinrb](https://github.com/chaintope/bitcoinrb) | Ruby Bitcoin primitives including modern wallet formats | Useful Ruby-native integration; work-in-progress APIs require version pinning. |
| [bitcoin-ruby](https://github.com/lian/bitcoin-ruby) | Older Ruby utility and protocol toolkit | Historical ecosystem coverage rather than an automatic current recommendation. |
| [Bit-Wasp bitcoin-php](https://github.com/Bit-Wasp/bitcoin-php) | PHP transaction/crypto tooling with optional native components | Fits PHP services; architecture and integer/platform requirements need attention. It is not a production full-node replacement. |
| [Haskoin](https://github.com/jprupp/haskoin-core) | Haskell types and Bitcoin processing libraries | Useful functional implementation; distinguish the library and header/node projects from full validation. |
| [Swift Bitcoin](https://github.com/swift-bitcoin/swift-bitcoin) | Native Swift toolkit and node research | Experimental node maturity should not be inferred from useful library APIs. |
| [BitcoinCore.Swift / BitcoinKit](https://github.com/horizontalsystems/BitcoinCore.Swift) | Native mobile SPV wallet stack | Despite the name, it is not Bitcoin Core’s full validator. Only the BTC extension is in scope. |

BDK bindings, rust-secp256k1, ACINQ’s secp256k1-kmp, and Swift secp256k1 bindings are important integration surfaces but generally wrap an existing engine or cryptographic implementation. Counting them separately would exaggerate implementation diversity.

# Chain synchronization and query infrastructure

## Light-client choices

| Family | Decision | Consequence |
| - | - | - |
| [Neutrino](https://github.com/lightninglabs/neutrino) | Go compact-filter client: download filters and inspect matches locally | Avoids sending every watched script to a query server, but still depends on peers for chain/filter/block availability and does not fully validate all transactions. |
| [Kyoto](https://docs.rs/kyoto-cbf/latest/kyoto/) | Rust compact-filter client and BDK integration with a reactive network model | Fits constrained/local wallet synchronization; the application still owns integration and availability handling. |
| [Nakamoto](https://github.com/cloudhead/nakamoto) | Rust light-client work separating protocol state machines from networking | Useful architectural research, with explicitly early-stage scope. |
| bitcoinj, Bitcoin-S, Haskoin and native mobile SPV stacks | Header verification and wallet-focused transaction discovery | Lower resource cost than a full validator, with security and discovery assumptions that must remain explicit. |

[BIP157](https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki) is the primary reference for client-side compact filtering. Local filter matching can improve script privacy, but subsequent block fetches and network metadata still reveal information. Compact filters also do not by themselves provide a complete view of unconfirmed transactions.

These are four different architectures: a **pruned full node** discards old data after validation; an **SPV client** does not perform all full-node checks; a **Utreexo validator** changes authenticated UTXO-state storage; an **indexer** supplies convenient queries over another chain source. Disk usage alone does not distinguish their trust models.

## Indexers and wallet backends

| Implementation | Design decision | Practical tradeoff |
| - | - | - |
| [electrs](https://github.com/romanz/electrs) | Rust, compact RocksDB indexes, retrieving more transaction data from Core | Good personal/small-service footprint; Core data availability and public-service DoS considerations matter. Does not require Core txindex. |
| [ElectrumX](https://github.com/spesmilo/electrumx) | Python global address/history server | Supports arbitrary client queries at the cost of indexing/storage. Use the BTC-maintained spesmilo lineage, not the original project after it dropped BTC. |
| [Fulcrum](https://github.com/cculianu/Fulcrum) | C++ multithreaded/async Electrum server | Targets query throughput with its own operational requirements, including an unpruned BTC backend with txindex. BTC mode only. |
| [Blockstream electrs / Esplora](https://github.com/Blockstream/electrs) | Expanded indexes and REST interfaces | More retained/indexed data buys fewer expensive backend reads and broader public-service queries; lightmode makes a different disk/read tradeoff. |
| [mempool electrs](https://github.com/mempool/electrs) | Derived indexing engine for mempool’s explorer stack | Application-specific schema and integration; not another independent consensus validator. |
| [Electrum Personal Server](https://github.com/chris-belcher/electrum-personal-server) | Track selected wallet descriptors/keys instead of indexing every address | Lower resource use and pruning compatibility, but wallet setup/rescanning is required; its documented interface does not support Electrum Lightning. |
| [bwt](https://github.com/bwt-dev/bwt) | Core-backed descriptor/xpub wallet tracking with Electrum/REST/event interfaces | Avoids a universal address index; historical project status and current compatibility need separate verification. |
| [NBXplorer](https://github.com/dgarage/NBXplorer) | Register tracked wallets/derivations, use PostgreSQL and expose wallet-oriented APIs | Fits merchant/multiwallet infrastructure with pruning-friendly design; keep the service on an appropriate trusted boundary. |
| [Haskoin Store](https://github.com/jprupp/haskoin-store) | Haskell P2P-connected indexer, RocksDB, atomic block writes and REST APIs | Avoids requiring a separate full node, but explicitly does not revalidate all scripts/consensus. Peering it to a trusted local validator changes that trust arrangement. BTC mode only. |
| [Blockbook](https://github.com/trezor/blockbook) | Address-indexing backend with HTTP/WebSocket APIs | Broad wallet-backend functionality and storage cost; only BTC functionality is included. |
| [btc-rpc-explorer](https://github.com/janoside/btc-rpc-explorer) and [mempool](https://github.com/mempool/mempool) | Explorer applications on top of node/indexing services | User interfaces and aggregation are separate from the chain’s validation authority. |

My assessment: choose the indexer from the query workload. A private wallet watching known descriptors does not need the same database as a public explorer serving arbitrary addresses. Compare initial indexing, historical query latency, steady-state writes, disk amplification, and reorg recovery under the same configuration before choosing a “fastest” implementation.

# Lightning protocol implementations

The key comparison is between a **daemon that owns the operating machinery** and a **library whose host application supplies that machinery**. Both can implement the same channel protocol while exposing very different safety boundaries.

| Engine or assembly | Architectural center | Main responsibility tradeoff |
| - | - | - |
| LND | Integrated Go daemon and RPC service | Convenient operational API versus adopting its wallet, database and process model |
| Core Lightning | C-oriented daemon family, signer separation and plugins | Explicit process/plugin boundaries versus more components and extension behavior to manage |
| Eclair | Scala/JVM actors and service architecture | Actor-based channel concurrency and server integration versus JVM/runtime and backend requirements |
| LDK | Embeddable Rust protocol library | Maximum host control versus responsibility for persistence, chain events, networking and lifecycle |
| LDK Node | Opinionated LDK plus BDK library assembly | Less integration work with more predefined choices |
| Electrum | Python wallet with its own Lightning implementation | Desktop-wallet convenience and lightweight operation versus server/routing dependencies |
| lightning-kmp | Kotlin Multiplatform wallet-oriented engine | Mobile/receive-send focus rather than a general-purpose forwarding service |
| Immortan | Scala lightweight/private-channel engine | Narrow wallet scope and privacy choices versus a smaller implementation/deployment base |

## LND

[LND](https://github.com/LightningNetwork/lnd) is a Go daemon with gRPC/REST, macaroon-based authorization, an integrated on-chain wallet, and several chain backend choices. The primary design benefit is that a product can operate a Lightning service through APIs instead of implementing the surrounding runtime itself. The application accepts the daemon’s internal organization and storage model in return.

Database labels require care. The [migration guide](https://github.com/lightningnetwork/lnd/blob/master/docs/db_migration_guide.md) distinguishes the older key-value layout, SQL-backed compatibility layers, and genuinely native SQL tables. Native migrations are staged by subsystem: invoices, graph, and payments have separate release milestones, with wallet/channel migration still treated separately. Saying “LND supports PostgreSQL” does not establish that every subsystem has the same transactional or scaling behavior.

The [0.21 announcement](https://lightning.engineering/posts/2026-06-11-lnd-0.21-launch/) includes simple Taproot private-channel work and onion-message infrastructure. At the checked [0.21.6 release](https://github.com/lightningnetwork/lnd/releases/tag/v0.21.6-beta), the [native BOLT12 roadmap](https://github.com/lightningnetwork/lnd/issues/10736) remains open for complete offer creation/payment flows. Code in master is not a released interoperability promise.

That roadmap also records an instructive future choice: reconstructing invoices from a signed envelope on receipt of an HTLC can avoid a database write for every invoice request. The intended benefit is reducing resource-exhaustion exposure; it remains a development design, not a claim about every released payment path.

For recovery, [static channel backups](https://docs.lightning.engineering/lightning-network-tools/lnd/disaster-recovery) help recover funds through peers and channel closure. They are not a current live-channel database that can simply resume normal operation. That distinction should shape a production recovery plan.

## Core Lightning

[Core Lightning](https://github.com/ElementsProject/lightning), formerly c-lightning, is principally C, with some newer components in other languages. Its [architecture](https://docs.corelightning.org/docs/contribute-to-core-lightning) separates responsibilities among processes for connectivity, gossip, channels, signing, and on-chain resolution.

The [plugin interface](https://docs.corelightning.org/docs/plugin-development) uses subprocesses and structured messages/hooks rather than requiring every extension to be compiled into the main daemon. This makes it attractive for teams wanting a native/Linux service whose business logic can live in another language.

The benefit of process separation is a narrower and more observable boundary. The cost is orchestration and the need to understand which hooks can block or alter protocol progress. Separating a signer process also does not automatically turn it into a policy-validating signer.

SQLite is the ordinary database choice; PostgreSQL is also available. The database backend should be evaluated together with single-writer and recovery behavior, not as a standalone promise of horizontal scaling.

A version-sensitive correction matters: **splicing is enabled by default from v26.04**. The old experimental-splicing setting is deprecated. Dual funding remains a separate experimental setting. Sources: [configuration](https://docs.corelightning.org/reference/lightningd-config), [deprecation table](https://docs.corelightning.org/docs/deprecated-features).

My assessment: a strong candidate when extensibility, Unix process boundaries, and native-code integration matter more than having every feature exposed through one integrated application framework.

## Eclair

[Eclair](https://github.com/ACINQ/eclair/blob/master/README.md) is a Scala/JVM Lightning implementation. Its [architecture](https://github.com/ACINQ/eclair/blob/master/docs/Architecture.md) uses actors for objects such as peers, channels, and payment attempts, with modules separating the core protocol and service/front-end responsibilities.

The design maps naturally to independently progressing channels: each actor owns state and processes messages. This is a useful reference for actor-oriented runtimes. It does not eliminate the need to reason about shared storage, ordering, dispatcher saturation, backpressure, or garbage-collection pauses.

Eclair deliberately depends on a suitably configured Bitcoin Core backend. The checked documentation requires wallet functionality, ZMQ, transaction indexing, and retained blocks. This makes backend assumptions explicit, but is a heavier deployment than a phone-oriented light client.

The [0.14 release notes](https://github.com/ACINQ/eclair/blob/master/docs/release-notes/eclair-v0.14.0.md) describe finalized splicing, compatible simple Taproot private channels, and experimental zero-fee commitments using newer transaction-policy mechanisms. They also require migration away from older non-anchor channels. The release requires Core 30.x; current development documentation references a later Core baseline. Do not mix master requirements with a deployed release’s requirements.

SQLite serves simpler operation, while PostgreSQL and coordination/lease mechanisms support larger deployments. My assessment: especially valuable to study for channel state machines, actor ownership, and service-scale operational design.

## LDK

[LDK](https://lightningdevkit.org/introduction/) is a complete Lightning protocol implementation delivered as Rust libraries. It is not a Bitcoin full validator, and using it does not mean every operational component is already supplied.

Its [architecture](https://github.com/lightningdevkit/rust-lightning/blob/main/ARCH.md) separates several important responsibilities. ChannelManager manages off-chain channel/payment state; ChannelMonitor tracks the information needed for on-chain enforcement; PeerManager processes peer protocol interactions; graph, routing and scoring components handle network/payment decisions. Helper crates can supply more of the surrounding system.

The host application supplies or selects chain notifications and reorg handling, transaction broadcast, fee estimates, keys/signers, networking, timers, storage, and event handling. The core does not require one particular application runtime. That makes LDK attractive for mobile, embedded, WASM, servers, unusual databases, or a new language’s runtime. It also enlarges the integration contract.

**Durable persistence is the most important example.** The [Persist contract](https://docs.rs/lightning/latest/lightning/chain/chainmonitor/trait.Persist.html) defines when a channel-monitor update is actually durable. A successful response must reflect the required durability, including appropriate filesystem synchronization. Asynchronous persistence reports an in-progress state and later completion; the protocol uses that information to gate progress.

The engineering consequence is simple: a callback whose data is merely queued in memory is not equivalent to a durable commit. Treating the monitor as a reconstructible cache can break the protection that the library’s state machine is designed to provide.

The [0.2 changelog](https://github.com/lightningdevkit/rust-lightning/blob/main/CHANGELOG.md) includes splicing and initial asynchronous-payment work, with important protocol and upgrade qualifications. Early support for a feature is not evidence that every other engine supports the same version or that downgrades are safe.

My assessment: choose raw LDK when control over runtime, storage, signing, or product behavior justifies owning those integration details. Use a higher-level assembly when those choices are not the differentiator.

## LDK Node and LDK Server

[LDK Node](https://github.com/lightningdevkit/ldk-node/blob/main/README.md) is an opinionated library assembling LDK, BDK, networking, persistence choices, chain backends, and gossip synchronization behind a smaller API. It reduces the number of surrounding components an application must design while preserving an embedded-library model.

[LDK Server](https://github.com/lightningdevkit/ldk-server/blob/main/README.md) exposes an LDK Node-based service through RPC. Its current README explicitly says it is not ready/tested for production and warns about evolving APIs and persisted formats.

These are three separate adoption choices: **raw LDK, LDK Node, and LDK Server**. Production use of the underlying protocol engine does not automatically confer the same maturity on every assembly built above it.

## Electrum

[Electrum](https://github.com/spesmilo/electrum) contains its own Python Lightning implementation; it is not merely a user interface controlling LND or CLN. Its wallet-oriented architecture combines lightweight on-chain synchronization with payment/channel functionality.

[Trampoline routing](https://electrum.readthedocs.io/en/latest/swapserver.html) reduces the need for a wallet to maintain and process the entire routing graph. The trade is dependence on available routing services and different privacy/information exposure. It does not by itself mean that the wallet’s channel funds are custodial.

Electrum also offers a separate [watchtower](https://electrum.readthedocs.io/en/latest/watchtower.html) mechanism using the transactions needed to respond on chain. A watchtower addresses monitoring and response; it is not a substitute for restoring the full current wallet/channel state.

My assessment: relevant both as a deployed wallet implementation and as a study of integrating Lightning into an existing lightweight desktop wallet.

## lightning-kmp, Phoenix and phoenixd

[lightning-kmp](https://github.com/ACINQ/lightning-kmp/blob/master/README.md) is ACINQ’s Kotlin Multiplatform Lightning library. It is a different implementation from the Scala Eclair engine, even though both come from ACINQ and share ecosystem concepts.

The project focuses on wallet-style send/receive use. Phoenix and [phoenixd](https://github.com/ACINQ/phoenixd) package this family for applications and services. Their operational model uses liquidity/routing services to reduce channel-management work for the user.

That exchanges some operational autonomy for provider availability, fees, and privacy considerations. It should not be collapsed into “custodial”: signing/control of channel funds and dependence on a liquidity provider are separate questions. It is also not the same design target as a general-purpose forwarding node with operator-chosen peers and routing policy.

## Immortan

[Immortan](https://github.com/nbd-wtf/immortan) is a Scala lightweight Lightning implementation focused on private channels. It describes independent development rather than being an Eclair fork, while reusing some ecosystem types/codecs.

Its choices include per-peer identities, limited scope, and a simple channel-execution model. Its multi-peer gossip comparisons are a lightweight trust heuristic, not equivalent to independently verifying every funding output against a fully validated chain.

The project also supports hosted-channel extensions. Those custodial balances are outside this report’s standard BTC-enforced Lightning scope. Applications such as OBW/Cliché belong to the same underlying implementation family rather than adding independent channel engines.

# Lightning assemblies, extensions and historical work

## Reuse and deployment layers

| Project | Underlying engine or role | Decision and consequence |
| - | - | - |
| [Greenlight](https://blockstream.github.io/greenlight/) | Hosted Core Lightning compute with a user-controlled signing component | Separates node operation from key custody. Service availability, signing policy and recovery remain explicit trust/operational questions. |
| [Breez SDK Greenlight](https://sdk-doc-greenlight.breez.technology/guide/about_breez_sdk_greenlight.html) | SDK built around Greenlight | Official documentation now marks it deprecated and unmaintained. Its successors must be assessed separately; Spark and Liquid paths are outside this BTC/LN-only scope. |
| [Alby Hub](https://github.com/getAlby/hub) | LDK-based application | Product interfaces, connectivity and wallet experience over an existing channel engine. |
| [Lampo](https://github.com/vincenzopalazzo/lampo.rs) | Experimental Rust daemon around LDK/BDK | A plugin-oriented assembly, not a new independent commitment protocol implementation. |
| [LNDK](https://github.com/lndk-org/lndk) | Experimental LND-connected companion using LDK functionality | Can supply features such as offers through a sidecar; this must not be recorded as complete native LND support. |
| [Mutiny node](https://github.com/MutinyWallet/mutiny-node) | LDK/BDK in a Rust/WASM/browser architecture | Valuable constrained-environment design history; repository archived in 2025. |
| [VLS](https://www.vls.tech/docs/latest/overview/intro/) | Validating Lightning signer | Keeps keys behind state/policy validation. It is signing infrastructure, not another routing/channel engine. |

Wallet front ends, [BTCPay Server](https://docs.btcpayserver.org/LightningNetwork/), [LNbits](https://github.com/lnbits/lnbits), [LNURL](https://github.com/lnurl/luds), [Nostr Wallet Connect](https://github.com/nostr-protocol/nips/blob/master/47.md) and RPC-client libraries can be central to a product, but they should not be counted as independent BOLT channel implementations merely because they expose Lightning payments.

## Historical and experimental protocol engines

| Project | Architecture or aim | Current qualification |
| - | - | - |
| [Ptarmigan](https://github.com/nayutaco/ptarmigan) | C/C++ BOLT implementation using an external chain backend and embedded storage | Archived; useful native design history rather than a current default. |
| [NLightning](https://github.com/nGoline/NLightning) | C# protocol implementation | Incomplete major BOLT functionality; not a verified runnable production equivalent of LND/CLN/Eclair. |
| [DotNetLightning](https://github.com/joemphilips/DotNetLightning) | F#/.NET protocol utilities and CLN integration/plugin tools | Beta library work; do not infer a complete production daemon. |
| [Node-Lightning](https://github.com/node-lightning/node-lightning) | TypeScript/Node protocol modules with an educational focus | Archived and lacking a full operational runtime. |
| [lit](https://github.com/mit-dci/lit) | MIT DCI Go Lightning research | Historical research architecture; not a current interoperable real-funds recommendation. |
| [Thunder](https://github.com/blockchain/thunder) | Early Blockchain.com Lightning work | Historical alpha, explicitly unsuitable for real funds. |
| [LNP Node](https://github.com/LNP-WG/lnp-node) | Rust service components and generalized Lightning-network work | Mainnet production readiness is unestablished here; Bifrost/RGB/nonstandard modes are excluded. |

# Version-sensitive feature decisions

Feature labels need a release, a channel type, and an interoperability target. “Supports Taproot,” for example, may refer to an on-chain wallet output, a private channel funding output, or a different commitment construction.

| Implementation | Verified finding at this review | Consequence |
| - | - | - |
| LND 0.21 series / checked 0.21.6 | Simple Taproot private channels and onion-message infrastructure; complete native BOLT12 offers flow remains on the open 0.22 roadmap | Do not use master package names or sidecar support as evidence of a complete released native flow. |
| Core Lightning from 26.04 | Splicing enabled by default; old experimental-splicing option deprecated | Separate this from the still-experimental dual-funding configuration. |
| Eclair 0.14 | Finalized splicing, compatible simple Taproot private channels; experimental zero-fee commitments | Core version and channel migration requirements are part of the feature, not optional deployment details. |
| LDK 0.2 | Splicing and initial asynchronous-payment capabilities with version/protocol qualifications | Test peer compatibility and recovery/upgrade behavior for the exact selected versions. |
| LDK Server current source | Explicit preproduction warning and evolving persistence/API formats | Evaluate independently from raw LDK and LDK Node maturity. |

Sources: [LND 0.21](https://lightning.engineering/posts/2026-06-11-lnd-0.21-launch/), [LND offers roadmap](https://github.com/lightningnetwork/lnd/issues/10736), [CLN deprecations](https://docs.corelightning.org/docs/deprecated-features), [Eclair 0.14](https://github.com/ACINQ/eclair/blob/master/docs/release-notes/eclair-v0.14.0.md), [LDK changelog](https://github.com/lightningdevkit/rust-lightning/blob/main/CHANGELOG.md), [LDK Server](https://github.com/lightningdevkit/ldk-server/blob/main/README.md).

# Signing and mining infrastructure

These components are Bitcoin implementations in a broad systems sense, but they do not replace a full validator or a Lightning channel engine.

| Project or family | Boundary it implements | Design lesson |
| - | - | - |
| [Hardware Wallet Interface](https://github.com/bitcoin-core/HWI) | Common software interface to hardware signers | Separate device transport and key access from wallet transaction construction. A common API does not mean every device has identical policy checks. |
| [VLS](https://www.vls.tech/docs/latest/overview/intro/) | State- and policy-aware Lightning signing | A key-isolation boundary is stronger when it can reject unsafe protocol actions, not merely sign opaque requests. |
| [Stratum V2 Reference Implementation](https://github.com/stratum-mining/stratum) | Rust mining-protocol libraries and associated applications | Mining transport and job negotiation are separate from Bitcoin consensus. Check the maturity of the particular library/application rather than applying one status to the whole ecosystem. |
| [ckpool](https://github.com/ckolivas/ckpool/blob/master/README-CKPOOL_MODES.md) | Pool work distribution and related modes over a Bitcoin backend | Pool coordination does not eliminate the need for an authoritative chain/template source. |
| [P2Pool](https://github.com/p2pool/p2pool) | Distributed pool coordination using a sharechain | Its coordination ledger is distinct from the BTC chain on which mining rewards settle. Included as mining architecture, not as a separate Bitcoin asset. |

Hardware-wallet firmware and miner firmware contain many additional implementations of signing, parsing, and device protocols. They are not all independently reviewed in this report. The reusable boundary to study is the division between transaction intent, policy approval, signature generation, transport, and chain validation.

# The decisions that matter across implementations

The following are engineering conclusions from the surveyed designs, not a benchmark ranking or a security audit.

## 1. Reuse consensus code where protocol exactness matters

A new implementation can improve architecture, portability, or resource usage and provide valuable independent scrutiny. It also inherits an unusually demanding compatibility obligation: integer behavior, parsing limits, activation history, script rules, serialization, and invalid-block handling all matter.

The [btcd signed-version disclosure](https://delvingbitcoin.org/t/disclosure-btcd-consensus-bugs-due-to-usage-of-signed-transaction-version/455) illustrates why “implements the BIPs” is not a sufficient test strategy. Differential tests must exercise unusual values and failure paths. A Core backend helps, but consensus-sensitive parsing may still happen in a wallet or Lightning dependency before data reaches the application.

A practical architecture can innovate above Core through RPC, or experimentally embed libbitcoinkernel, while keeping independent validation work in a research track. This preserves room for better runtimes without making a product launch depend on proving a new consensus engine.

## 2. Distinguish what can be reconstructed from what must be preserved

Historical block data, wallet indexes, and routing gossip can often be fetched or rebuilt, subject to the chosen data source and available history. Current channel state, revocation-related information, and the ordered record of commitments have different consequences if lost.

The [LND recovery documentation](https://docs.lightning.engineering/lightning-network-tools/lnd/disaster-recovery) makes this distinction concrete. A seed is not the entire current off-chain state. A static backup designed for funds recovery is not the same as a safe snapshot for resuming live channels.

In a design review, label each store as authoritative, reconstructible, or recovery-only. Define when an operation may tell a peer that a state transition has completed. The correct place to enforce that rule is in the protocol/storage boundary, not in an operational convention that someone must remember.

## 3. Separate keys from authority to request signatures

Moving a private key to another process, HSM, device, or service protects against some key-extraction paths. It does not necessarily protect against a compromised host that asks the signer to authorize a harmful transaction.

BDK’s [signer separation](https://bookofbdk.com/design/signing/) clarifies ownership of signing material. [VLS](https://www.vls.tech/docs/latest/overview/intro/) goes further for Lightning by validating relevant state and policy. The signer’s own persisted state and recovery therefore become part of the system design.

For an on-chain wallet, show and enforce the intended destinations, amounts, change, fees, and sighash policy. For Lightning, the signing boundary must understand more than a generic “sign this digest” request if it is intended to constrain a compromised node.

## 4. Scale channel ownership carefully

Parallelism across independent channels is much easier to reason about than concurrent writers to the same channel’s state. Eclair’s actors, CLN’s process separation, and LDK’s explicit integration boundaries provide different mechanisms for expressing ownership.

My recommendation is to establish one authoritative writer per channel state, with fencing during failover. A replicated database alone does not establish that two runtimes cannot both act on the same channel. A restored stale replica can be dangerous even when every stored byte passes a checksum.

Measure the time spent waiting for durable storage, signer round trips, chain access, and peer responses before assuming the CPU language is the bottleneck.

## 5. Chain backends change privacy as well as performance

An Electrum or Esplora service can learn the scripts or addresses queried. A locally operated server changes who sees that information. Compact filters move matching toward the client, but fetching matching blocks still creates observable traffic.

The important choices are whether the client locally validates the chain, whether the server can omit relevant data, how complete discovery is checked, and how the application recovers from unavailable peers. A uniform wallet API does not make these backend properties uniform.

For a wallet, separately test initial recovery, ordinary sync, reorgs, missed notifications, conflicting transactions, and a backend that is stale or incomplete. For Lightning, delayed chain knowledge also interacts with time-sensitive on-chain enforcement described in [BOLT5](https://github.com/lightning/bolts/blob/master/05-onchain.md).

## 6. Feature names conceal several protocols and maturity levels

Separate BOLT11 invoices from BOLT12 offers; on-chain Taproot support from Taproot channel constructions; splicing from dual funding; and a private-channel implementation from public routing support.

The [Eclair release notes](https://github.com/ACINQ/eclair/blob/master/docs/release-notes/eclair-v0.14.0.md) and [LDK changelog](https://github.com/lightningdevkit/rust-lightning/blob/main/CHANGELOG.md) also show that features can carry migration, downgrade, and interoperability conditions. A feature matrix should identify the exact release and peer combination used for verification.

## 7. Storage benchmarks need equivalent work

An in-memory UTXO set, a memory-mapped archive, a compact accumulator, a global address index, and a wallet-specific tracker solve different workloads.

A meaningful node comparison holds constant the chain range, script-validation work, bootstrap assumptions, hardware, cache limits, retained indexes, and durability settings. A meaningful Lightning comparison additionally distinguishes payment attempts from successful payments, channel count from active forwarding load, and local throughput from liquidity-constrained routing success.

No common reproducible benchmark covering all these implementations was established in this research. Claims such as “Rust is fastest,” “PostgreSQL means scalable,” or “smallest disk means least trust” are not justified conclusions.

## 8. Cryptographic quality includes entropy and validation boundaries

The [Milk Sad disclosure](https://milksad.info/disclosure.html) concerned weak entropy in specific historical Libbitcoin Explorer seed-generation behavior. It should not be generalized into a claim that every libbitcoin component was the same vulnerability. The architectural lesson is that a wallet can use strong signature mathematics and still fail at seed generation.

The [bitcoinjs-lib Merkle-root advisory](https://github.com/bitcoinjs/bitcoinjs-lib/security/advisories/GHSA-8vv3-6qpc-gpw6), published in October 2026, documents duplicate-transaction handling that could preserve a matching root. The lesson is different: checking one cryptographic commitment does not establish every condition of block validity.

These cases argue for narrow, explicit contracts: where entropy originates, what exactly a helper validates, and which stronger claim the caller is allowed to make.

## 9. Licensing is an architecture input

The ecosystem includes permissive licenses, copyleft licenses, and custom restrictions. Gocoin’s [custom terms](https://github.com/piotrnar/gocoin/blob/master/LICENSE), libbitcoin’s [license and exception](https://github.com/libbitcoin/libbitcoin-system), and [bitcoinlib’s AGPL terms](https://github.com/1200wd/bitcoinlib) deserve particular attention in a commercial dependency review.

Check the exact module and shipped dependency graph. A permissively licensed wrapper does not necessarily change the license of the native library or service implementation behind it.

# Practical architecture choices

These are recommendations for evaluation, not claims that one implementation is best for every workload.

| Goal | Starting point I would evaluate | Why |
| - | - | - |
| Authoritative BTC chain service | Bitcoin Core as a separate process | Keeps the application’s consensus dependency clear and upgrades operationally bounded. |
| New custom on-chain wallet | BDK over a deliberately chosen chain backend | Strong wallet mechanisms with explicit storage/signing boundaries. |
| Embedded Lightning application | LDK Node first; raw LDK when deeper control is necessary | Avoids building runtime plumbing that does not differentiate the product. |
| Conventional Lightning service | Compare LND, CLN and Eclair against the required operating model | All supply surrounding machinery; their interfaces, extensions and runtime assumptions differ. |
| Native C/Linux extension work | CLN plugins, libwally and libsecp256k1 | Natural process/C boundaries with focused responsibilities. |
| New language or runtime | Core RPC first; C ABI wrappers for narrow libraries; pinned libbitcoinkernel experiments where justified | Lets runtime innovation proceed without simultaneously replacing every critical protocol layer. |
| Phone-oriented receive/send wallet | Evaluate lightning-kmp/Phoenix-style integration or an LDK-based wallet assembly | Make liquidity-provider, background-execution and recovery assumptions explicit. |
| Small validating node research | Compare Utreexod and Floresta | Study proof availability, bootstrap assumptions and hardware behavior together. |
| Public arbitrary-address queries | Compare Fulcrum, ElectrumX and Esplora-family servers on representative workloads | Their indexes and backend requirements differ materially. |
| Merchant wallet tracking | NBXplorer or a descriptor-scoped backend | Avoids paying for a global index when the application knows which wallets it watches. |

A sensible implementation sequence for a custom platform is:

1. Establish one authoritative chain source and its reorg/notification behavior.

2. Integrate wallet accounting and transaction construction with explicit signer ownership.

3. Add Lightning through a mature daemon or LDK Node, and document the durable-state boundary.

4. Exercise crash recovery, stale-backup handling, reconnect/replay, chain reorgs, signer refusal, and asynchronous persistence failure.

5. Move to lower-level libraries only where a measured requirement justifies owning more machinery.

6. Evaluate independent consensus, novel database designs, and new channel protocols as separate experiments before relying on them for production correctness.

# Research limits and how to read the inventory

The report intentionally includes historical and experimental projects so that “implementation” does not silently mean only the most popular four repositories. It also avoids counting every wrapper, wallet UI, device firmware, sample node, or RPC client as a distinct consensus or Lightning engine.

Maintenance status is based on official repository notices, release information, and current documentation. An archived mirror can indicate a hosting move, as with parts of the Rust Bitcoin ecosystem; it is not sufficient evidence of abandonment. Conversely, recent commits or an attractive README do not establish complete consensus validation or safe real-funds operation.

This was a primary-source architecture and status review, with targeted source inspection for consequential claims. It was not an independent security audit, a fresh full-chain synchronization test for every node, a penetration test, or a cross-implementation performance benchmark. Unknown production maturity is marked as unknown rather than converted into confidence by repository activity.


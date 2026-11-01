# NIP-KEYS — A Network of Keys

`draft` `optional` — v1, 2026-11-01. **Unimplemented: design only.** No Rust or
Coax code implements these records yet, and every role here refuses until a
supported implementation and its conformance evidence exist. The
[shared contracts](contracts.md) are normative. KEYS allocates no event kinds.

This NIP defines collaborative custody of on-chain bitcoin: a wallet whose
coins move only when enough independent **key agents** sign. It specifies how
the parties agree on the wallet, enroll their keys, pass a transaction for
signatures, recover when a key is lost, replace a failed key agent, and offer
key agency as a paid service. Every record travels as a signed,
recipient-encrypted private `3188` artifact.

The motivation and the market argument are in the research note
[A network of keys](../../../docs/research/2026-11-01-network-of-keys.md).

## Why this exists

[NIP-SOV](NIP-SOV.md) answers who can stop an agent: lifecycle control,
fencing, revocation and an exact-action guardian quorum. It doesn't answer who
holds the agent's coins. Its guardian quorum is an approval rule the host
enforces, its custody profile covers the agent's Nostr key through one NIP-46
signer, and its treasury points at one separately protected wallet host. A
host that ignores the quorum, or a wallet host that fails, is a single point
of failure for the funds.

KEYS moves the quorum into the coins. A spend needs *m* signatures from *n*
keys held by parties that declare who they are. The host can't skip that
check, because Bitcoin script enforces it. If one key agent fails, the others
sweep the coins to a new wallet with a replacement agent. Any party holding
the wallet description can recover the coins with any compliant wallet tool,
without OpenAgents.

## Terms

- A **holder** is the principal whose coins the wallet protects: a person, an
  organization, or a sovereign agent.
- A **key agent** is a party that holds one key in a holder's wallet and signs
  under a stated signing policy. The holder can be one of its own key agents.
- A **coordinator** stores the wallet description, routes signing requests
  and combines partial signatures. A coordinator holds no key unless it also
  enrolls as a key agent, and its loss never strands coins.
- A **signing quorum** is any set of keys that can spend on some spending
  path, including recovery paths.

## Relationship to the existing contracts

| Contract | Reuse and boundary |
| --- | --- |
| [contracts](contracts.md) `3188` | Carries every KEYS record between named recipients. Relay privacy rules apply: authors, recipients and traffic patterns remain visible. |
| [SOV](NIP-SOV.md) | Adds the `bitcoin-multisig` coin-custody mechanism next to SOV's identity custody, and a reserve to the SOV treasury. SOV guardians stay an application approval policy; they are not KEYS signers unless separately enrolled. |
| [POL](NIP-POL.md) | A signing request may require an exact POL action and approval. A key agent's signing policy is a POL policy it publishes to the holder. |
| [ATT](NIP-ATT.md) | An attested co-signer is an ATT endpoint whose key was generated inside the TEE and whose release pins its signing policy. |
| [MKT](NIP-MKT.md) | Key agency is sold as an MKT offering with the terms below. Settlement uses MKT's existing Bitcoin profiles. |
| [RUN](NIP-RUN.md) | Signing attempts, refusals and unknown outcomes are recorded as RUN evidence when an agent lifecycle is involved. |
| NIP-46, NIP-47 | Unchanged. NIP-46 remains the agent's identity signer. NIP-47 remains a Lightning wallet transport for the hot balance. Neither is a KEYS signer. |

## Standards this NIP builds on

KEYS defines coordination, not cryptography. Wallets and transactions use
existing Bitcoin standards so that any compliant tool can recover them:

- Output script descriptors (BIP-380 to BIP-386) with key origins, and wallet
  policies (BIP-388).
- Miniscript (BIP-379) for timelocked recovery paths.
- Partially signed Bitcoin transactions, PSBT v0 and v2 (BIP-174, BIP-370).
- Generic signed messages (BIP-322) for proof that an enrolled key is held.

This version supports two script families: `wsh(sortedmulti(...))` and
Taproot `tr(...)` with `multi_a` or `sortedmulti_a` leaves, optionally
combined with miniscript `older` or `after` recovery leaves. Threshold
signatures such as MuSig2 or FROST, where the quorum looks like one key on
chain, are out of scope for v1 and refuse; they need their own profile with
interactive key generation and nonce handling.

## Encoding

Every KEYS body has required `v` and `requires: []`, optional inert `meta`,
and exactly the fields specified. Public keys are lowercase 64-hex x-only
Nostr keys unless a field says otherwise. Bitcoin amounts are integer
satoshis in fields named `*_sat`. Unknown versions, required features, script
families or fields refuse before any effect.

## Wallet

`openagents.keys-wallet.v1` is one revision of a wallet's agreed description.

| Field | Type and meaning |
| --- | --- |
| `wallet` | Fresh common ID for the wallet lineage. |
| `holder` | Holder public key. |
| `revision`, `previous` | Lineage, with SOV's zero/null and successor rules. |
| `network` | `mainnet`, `testnet4` or `signet`. Other networks refuse on production hosts. |
| `descriptor` | The complete BIP-380 descriptor string with checksum, using key origins and multipath derivation for receive and change. |
| `paths` | One to eight spending paths. Each has `name`, `threshold`, `keys` (enrollment ArtifactRefs) and `timelock` (null, `{older: blocks}` or `{after: height}`). Exactly one path has a null timelock: the primary path. |
| `enrollments` | ArtifactRefs to every key agent's enrollment, in descriptor key order. |
| `recipients` | Public keys allowed to receive this wallet description. Every key agent is a recipient. |
| `starts_at`, `expires_at` | Validity of this revision. Expiry stops new signing requests; it never moves coins. |

Admission recomputes the descriptor from `paths` and `enrollments` and refuses
any mismatch, including key order, derivation or checksum. The wallet is
fully described by this record: every recipient must be able to rebuild it
alone. A coordinator that holds the only copy is a refusal condition, not an
implementation detail.

Composition rules, checked at admission:

1. **No one party reaches a threshold alone.** For every path, no set of keys
   whose enrollments name the same `operator` (see Independence) can meet
   that path's threshold. The holder's own keys count as one operator.
2. **A sovereign agent never holds a primary-path majority.** When `holder`
   is a SOV agent, keys controlled by that agent's runtime are fewer than the
   primary threshold. The agent can initiate and co-sign; it can't spend
   alone.
3. **Recovery paths are later and wider.** Every timelocked path requires a
   timelock of at least 4,320 blocks (about 30 days) unless the holder's
   policy states a shorter value, and must not depend solely on an attested
   co-signer, because a TEE-held key can be lost with its host.
4. **No silent weakening.** A revision that lowers a threshold, removes a key
   agent or shortens a timelock needs the primary quorum's approval of the
   exact successor bytes, as a signed POL approval from each signer.

## Enrollment

`openagents.keys-enrollment.v1` is a key agent's offer of one key to one
wallet.

| Field | Type and meaning |
| --- | --- |
| `agent` | The key agent's Nostr public key. |
| `wallet` | Wallet lineage ID. |
| `xpub` | Extended public key with origin `[fingerprint/path]`. |
| `possession` | BIP-322 signature by the key at its first receive derivation over the digest of `{wallet, agent, xpub}`. |
| `class` | `software`, `hardware`, `attested` or `institutional`. |
| `attestation` | Null, or an ATT attested-endpoint reference. Required when `class` is `attested`. |
| `signing_policy` | ArtifactRef to the POL policy this agent applies before signing. |
| `independence` | ArtifactRef to the agent's independence declaration. |
| `expires_at` | Last time this enrollment can be admitted into a wallet revision. |

The key agent signs the enrollment. Admission verifies `possession` against
`xpub`. An enrollment for one wallet can't be reused in another; the digest
binds the wallet ID.

## Independence

`openagents.keys-independence.v1` is a key agent's declaration of who it is
and what it shares with others. It is a signed claim, not proof.

| Field | Type and meaning |
| --- | --- |
| `agent` | Key agent public key. |
| `operator` | Stable identifier for the person or legal entity that controls the key. |
| `jurisdiction` | ISO 3166-1 alpha-2 country codes where the key and its operator are subject to legal process. |
| `custody` | Device or HSM vendor, hosting provider and region, and any platform service the signer depends on. |
| `affiliations` | Operators this agent shares ownership, staff or contracts with. |
| `insurance` | Null, or a declared policy reference. Declared, not verified. |
| `observed_at` | When the agent made this declaration. |

Clients show a holder every attribute that two or more keys in one signing
quorum share: an operator, a jurisdiction, a vendor, a host, or an
affiliation. Shared operators refuse under composition rule 1. Other shared
attributes are shown, not refused, because the holder decides how much
correlation to accept. A holder's claim that its wallet is independent must
list these shared attributes.

## Signing

### Request

`openagents.keys-sign-request.v1` asks one key agent for one signature.

| Field | Type and meaning |
| --- | --- |
| `wallet` | Exact wallet revision ArtifactRef. |
| `requester` | Public key of the holder or an admitted initiator. |
| `psbt` | ArtifactRef to the PSBT bytes. |
| `outputs` | The requester's statement of every output: `{address, amount_sat, change}`. |
| `fee_sat` | The stated absolute fee. |
| `purpose` | Short machine-readable purpose, such as `hot-topup`, `payment`, `rotation-sweep` or `recovery`. |
| `action` | Null, or the exact approved POL action the signing policy requires. |
| `treasury` | Null, or the SOV purchase or top-up request this spend settles. |
| `nonce`, `deadline` | Idempotency and expiry. |

A signer never trusts `outputs` or `fee_sat`. It recomputes both from the PSBT
and refuses any difference. It derives each `change: true` output from the
wallet descriptor and refuses change it can't derive. It refuses inputs that
aren't the wallet's, a fee above its signing policy's ceiling, a spend from a
retired wallet other than its rotation sweep, and any timelocked path before
its timelock has passed.

### Receipt

`openagents.keys-sign-receipt.v1` records the outcome.

| Field | Type and meaning |
| --- | --- |
| `request` | Exact request ArtifactRef. |
| `status` | `signed`, `refused` or `unknown`. |
| `psbt` | ArtifactRef to the PSBT with this agent's partial signatures, when signed. |
| `checks` | The signing-policy checks run and their results. |
| `reason` | Refusal reason, or null. |

Idempotency is `(agent, requester, nonce)` with the request digest. An exact
retry returns the retained receipt. A changed request with a used nonce
refuses. A signature already returned can't be recalled, so a key agent
records consumption before releasing its partial signature.

The coordinator combines partial signatures and broadcasts. A combined
transaction is not settlement: the holder counts a spend only after the
transaction confirms to the depth its policy states.

## Attested co-signer profile

A key agent with `class: attested` runs as an [ATT](NIP-ATT.md) workload:

- Its signing key is generated inside the TEE, and its attested endpoint
  binds that key's xpub.
- Its release pins the signing policy. Changing the policy is a new release
  under ATT's release-head rules, not a configuration change.
- Before signing, it checks the SOV treasury policy named by the request:
  per-operation ceiling, allowance, fee ceiling and destinations, with a hold
  recorded before the partial signature leaves the TEE.
- It can add a delay for large spends: a request above its policy's
  `delay_above_sat` is signed no earlier than `delay_seconds` after receipt,
  and any primary key agent's refusal during the delay cancels it.

This makes the SOV guardian rule cryptographic for coins: the host can skip a
guardian check, but it can't produce a signature the co-signer withheld.
Composition rule 3 keeps a lost TEE from stranding funds.

## Recovery and liveness

Timelocked paths in the wallet descriptor are the recovery mechanism. With
`older` (relative) timelocks, a path becomes spendable a set number of blocks
after each coin confirms, so the holder must refresh coins before then.

`openagents.keys-liveness.v1` is a holder's or key agent's statement that it
is present: `wallet`, `agent`, `observed_at` and `next_refresh_height`, the
height by which the holder will refresh coins whose recovery path would
otherwise open. A coordinator warns every primary key agent when a coin is
within the holder's warning window of an open recovery path. A refresh is an
ordinary primary-path spend back to the wallet with `purpose: refresh`.

Inheritance is a recovery path whose keys belong to heirs or their key
agents, with a long timelock. KEYS doesn't define a death or incapacity
oracle: the timelock and the holder's failure to refresh are the trigger.

## Rotation

`openagents.keys-rotation.v1` replaces a key agent by moving coins to a new
wallet.

| Field | Type and meaning |
| --- | --- |
| `from` | Exact retiring wallet revision. |
| `to` | Exact new wallet revision, already admitted. |
| `reason` | `compromised`, `unavailable`, `retired`, `scheduled` or `upgrade`. |
| `sweeps` | Signing-request ArtifactRefs that move every coin to `to`. |
| `completed_at` | Null until every swept coin confirms. |

The primary quorum signs the rotation. From then on, signers of the retiring
wallet refuse every spend except its sweeps. A key agent marked `compromised`
is excluded from the new wallet, and its keys are excluded from the sweep
quorum when the remaining agents can meet the threshold without it. The
network routes around a failed agent this way: no coins depend on it once the
sweep confirms.

## Key agency offerings

A key agent sells its service through [MKT](NIP-MKT.md). The offering's terms
body is `openagents.keys-agency-terms.v1`:

| Field | Type and meaning |
| --- | --- |
| `classes` | The enrollment classes offered. |
| `signing_policy` | ArtifactRef to the default POL signing policy, and which parts the holder may tighten. |
| `response_seconds` | Stated maximum time from a valid request to a receipt. |
| `independence` | ArtifactRef to the agent's independence declaration. |
| `fees` | `{period_seconds, period_sat, per_signature_sat}`. |
| `recovery` | Whether the agent serves on recovery paths, and the identity checks it runs before signing one. |
| `exit` | Notice the agent gives before ending service, and its duty to sign the rotation sweep. |

An order for key agency settles under MKT's Bitcoin profiles. Accepting an
order doesn't enroll the agent; enrollment and wallet admission happen as
above. LAB's pay-on-acceptance model doesn't apply, because key agency is an
ongoing service, not a deliverable.

Reputation for key agents comes from receipts the holder can check: how many
signing requests an agent received, signed, refused and left unknown, and its
response times. A market reputation score never admits a key agent by itself.

## Hot and cold balances

Lightning balances can't practically sit behind a multisig wallet. A SOV
treasury that uses KEYS therefore has two parts:

- A **hot** Lightning balance in the wallet host, governed by the existing SOV
  treasury policy, with a `hot_ceiling_msat` added to that policy.
- A **reserve** in a KEYS wallet. Moving funds from the reserve to the hot
  balance is a reserve spend with `purpose: hot-topup`, signed by the primary
  quorum, and the attested co-signer refuses a top-up that would raise the hot
  balance above `hot_ceiling_msat`.

Losing the wallet host then loses at most the hot ceiling.

## Privacy

Every key agent that holds the descriptor can see every address and balance
in the wallet. Holders should give each wallet only the key agents it needs,
and use separate wallets for unrelated funds. Wallets, enrollments,
independence declarations and receipts are private by default. There is no
public wallet tag, public balance or relay listing of key agents beyond what
a key agent chooses to publish in an MKT offering.

## Not in scope

- Threshold signatures (MuSig2, FROST) for coins, and threshold custody of the
  agent's Nostr identity. Both need their own profiles; SOV already refuses
  threshold identity custody.
- Lightning channel custody.
- Cashu, Fedimint and other non-Bitcoin-script custody, which the repository's
  Bitcoin-only rule excludes for now.
- Proving independence. KEYS records declarations and shows shared
  attributes; it doesn't verify them.

## Implementation order and conformance

1. **Pure contracts.** Closed schemas for every record; descriptor recomputation
   from paths and enrollments; composition rules 1 to 4.
2. **Signing loop on signet.** Request, PSBT recomputation, receipts and
   coordinator combination among three software key agents, with exact retries.
3. **Attested co-signer.** An ATT workload with a pinned signing policy that
   checks SOV treasury limits and the large-spend delay.
4. **Recovery and rotation.** Timelocked recovery, refresh warnings and a
   rotation away from an unavailable agent, on signet.
5. **Hot and cold treasury.** The SOV reserve binding and hot ceiling.
6. **Key agency offerings.** MKT terms and receipt-based reputation.

Conformance fixtures must include a descriptor that doesn't match its paths;
an enrollment with a bad possession proof; two keys under one operator meeting
a threshold; a SOV agent holding a primary majority; a PSBT whose outputs
differ from the request; change to an underivable address; a fee above the
ceiling; nonce reuse with changed bytes; a recovery spend before its
timelock; a spend from a retired wallet that isn't its sweep; a compromised
agent excluded from its sweep; a top-up above the hot ceiling; a large spend
refused during its delay; and a coordinator outage recovered from a key
agent's copy of the descriptor with an independent wallet tool.

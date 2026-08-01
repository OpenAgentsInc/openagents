# Assessing the Wizardsardine Coldcard RNG report and feature blast radius

Status: **independent synthesis of public material and a pinned source review.**
This document is not a new disclosure, an exploit, or incident-response authority.
Operational facts were changing quickly on 2026-08-01, so time-sensitive claims are
identified as such.

Scope:

- Wizardsardine's 2026-08-01 article and its supplied feature-impact graphic.
- The supplied "Independent Code Review & Evidence Report," described by its
  distributor as an AI-generated K3 review that had not yet been personally peer
  reviewed.
- Coldcard firmware at pre-fix commit
  [`bcc2c382`](https://github.com/Coldcard/firmware/tree/bcc2c382a324690a2fcf972c0bac3b79bf923f7b)
  and libngu at
  [`537519a8`](https://github.com/switck/libngu/tree/537519a829259622ea6b0334fbafd6cae852852f).
- The existing analyses in this directory.

Reading order:

- [`chatgpt-pro-analysis.md`](chatgpt-pro-analysis.md) explains the linker and
  configuration failure, affected firmware, entropy estimates, exploitability,
  and remediation.
- [`2026-08-01-kelbie-independent-postmortem-analysis.md`](2026-08-01-kelbie-independent-postmortem-analysis.md)
  separates on-chain impact evidence from key-recovery evidence and records the
  review-history and AI-audit lessons.
- This document adds the wallet-policy analysis, audits the feature graphic, and
  evaluates the supplied K3 report.

---

## 1. Executive assessment

The Wizardsardine article is technically strong on the central failure and adds
an important perspective missing from a seed-only account: the defective random
stream was also consumed directly by several features. A sound dice-generated or
imported master seed therefore protects deterministic keys derived from that seed,
but it does not retroactively make an unrelated ephemeral key, paper-wallet key,
password, or random mask sound.

Its second important contribution is policy-level analysis. For a multisignature
wallet, counting uncompromised devices is insufficient. The relevant question is:
**can any satisfiable spending path be completed entirely with affected keys?** A
2-of-3 policy with two affected Coldcards can fail even if its third key is sound.
Taproot and script-revelation details can also change which paths become visible to
an attacker during use or migration.

The central technical claims are supported by the pinned source and by independent
primary-source analyses from Coinkite and Block. The supplied K3 report is useful as
a source map and contributes two worthwhile refinements: standard encrypted backups
used the working hardware RNG, while the device-clone flow did not; and a captured
USB session could expose a desktop-entered BIP-39 passphrase. It should not be
treated as an independent evidentiary pillar: it was disclosed as AI-generated and
not peer reviewed, and several of its feature conclusions need qualification.

The graphic is a good dependency map, not a complete exploitability map. Its blue
"exploitable remotely" category should be read as **potentially exploitable without
later physical possession of the Coldcard once the attacker has the necessary
artifact, transcript, verifier, or public-key oracle**. It does not mean that every
blue box is exposed to an unauthenticated Internet attacker.

Wizardsardine also writes from the perspective of the company behind Liana and ends
with a Liana Business recommendation. That commercial context does not weaken the
source-backed defect analysis, but product recommendations and urgent migration
advice should be corroborated independently and evaluated for the user's actual
descriptor, disclosure state, and threat model.

---

## 2. What is established, and at what evidence level

### Established from source and reproduced behavior

- Firmware seed generation called `ngu.random.bytes(32)`.
- A build-configuration mismatch caused libngu's randomness mixer to receive
  MicroPython's Yasmarang fallback rather than Coldcard's hardware RNG.
- Mk2/Mk3 devices received no later secret reseed. Mk4/Mk5/Q devices injected only
  a 32-bit value from secure-element output into the defective stream.
- The same stream reached direct callers outside master-seed generation.
- A public end-to-end recovery reproduced an affected Mk3 wallet from a constrained
  timing search and matched its xpub. See the reproduction evidence summarized in
  the [Kelbie analysis](2026-08-01-kelbie-independent-postmortem-analysis.md#the-reproduction-belongs-to-someone-else-and-the-repository-says-so).

### Supported, but estimate- or incident-dependent

- Coinkite's approximately 40-bit and 72-bit figures are preliminary work-factor
  estimates, not properties proved by one source line. Block's analysis gives a
  stricter structural upper bound for the later-device reseed contribution.
- On-chain theft attribution was still evolving. The snapshot analyzed in the
  Kelbie material derives 2,348 victim addresses and `1,129.13105121 BTC`; Bitcoin
  Optech reported estimated losses exceeding 1,000 BTC as of 2026-07-31. These are
  time-bounded findings, not a final total or proof that every output was Coldcard
  derived.
- Statements in the Wizardsardine article such as funds being drained "right now"
  or compromise being imminent were urgent forecasts based on the incident state at
  publication. They are not timeless technical conclusions.

### Not repaired by installing fixed firmware

Fixed firmware prevents future outputs from using the bad path. It cannot add
entropy to an already generated seed, private key, password, mask, shared secret, or
captured session. Existing affected material must be assessed and, where necessary,
rotated or migrated.

---

## 3. Audit of the supplied feature graphic

| Graphic claim                                          | Source-level assessment                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Attacker prerequisite and practical verdict                                                                                                                                                                                                                                                                                        |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Paper wallets: private key = raw PRNG output**       | Confirmed. [`paper.py:94`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/paper.py#L94) constructs a secp256k1 keypair without supplying a key; the pinned libngu constructor fills that private key from `my_random_bytes`. The optional dice path is different.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | A derived address is a public verifier, so candidate keys can be tested offline. This is the clearest direct remote-theft path in the graphic.                                                                                                                                                                                     |
| **Device cloning: seed recovered in the clear**        | Confirmed only for **Backup > Clone Coldcard**, not ordinary encrypted backups. Both clone-session ECDH keys are generated from the defective stream ([target](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/backups.py#L743-L760), [source](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/backups.py#L830-L868)). Standard backup passwords, salts, and IVs used `ckcc.rng_bytes`, the working hardware path.                                                                                                                                                                                                                                                                                                                    | The attacker needs a clone artifact or the exchange material. The encrypted clone is a durable offline verifier; dice/import origin of the seed does not repair its predictable transport key. The phrase "in the clear" describes the result after successful decryption, not the file's stored format.                           |
| **USB session: encryption treated as absent**          | The confidentiality claim is substantially correct. The device's ephemeral ECDH key comes from the bad stream ([`usb.py:702-729`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/usb.py#L702-L729)). The affected channel can carry xpubs and a desktop-entered BIP-39 passphrase.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Requires a captured USB transcript plus a feasible PRNG-state search. This compromises session confidentiality, not automatically the master signing key. Contents and consequences depend on the commands used in that session.                                                                                                   |
| **Co-signing Key C: 12-word policy key**               | Conditional. The default CCC/SSSP path calls `generate_seed()` ([`ccc.py:896-900`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/ccc.py#L896-L900)), but the same flow also supports dice, import, and Seed Vault selection.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | A device-generated Key C is affected; a sufficiently random independently generated/imported Key C is not affected merely because it was used on the device. Policy exposure still depends on whether Key C participates in a satisfiable path.                                                                                    |
| **Password generator: two modes; strict mode 49 bits** | The dependency is confirmed, but "two modes" is an imprecise summary. The code offers 12-word, 24-word, dense, and short strict-format choices. The first three call `generate_seed`; the short format is documented in code as 49 bits and uses the same stream.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | A password still needs a verifier: a site login, password hash, encrypted object, or other target. Online rate limits may dominate. Manually entered passwords and BIP-85-derived passwords from a sound master seed are separate paths.                                                                                           |
| **Secret Teleport: 40-bit password**                   | Confirmed for ordinary Key Teleport. Its receiver and sender ephemeral keypairs and five-byte password use the defective stream ([`teleport.py:75-84`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/teleport.py#L75-L84), [`teleport.py:192-230`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/teleport.py#L192-L230)).                                                                                                                                                                                                                                                                                                                                                                                                         | Requires capture of the relevant exchange. A successful recovery exposes the teleported secret; it is not a blind remote read from the device.                                                                                                                                                                                     |
| **Secret Teleport: 28-bit index**                      | Mechanism confirmed, consequence overstated. Multisig PSBT teleport chooses a predictable 28-bit child index, but derives the actual keypair from the wallet seed ([`multisig.py:1184-1228`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/multisig.py#L1184-L1228)).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | If the underlying wallet seed is sound, predicting the index does not reveal the private key. This path should not be grouped with ordinary Key Teleport as direct secret recovery.                                                                                                                                                |
| **HSM mode: 2FA secret and confirmation codes**        | Mixed. The local-confirmation HMAC material uses `ngu.random.bytes(15)` ([`hsm.py:853-857`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/hsm.py#L853-L857)). HSM User Management TOTP/HOTP/HMAC secrets are not generated from that path: supplied secrets arrive from the host, and the code's device-side generator, when used, calls the working `ckcc.rng_bytes` ([`users.py:110-145`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/users.py#L110-L145), [`users.py:177-189`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/users.py#L177-L189)). CCC/SSSP **Web2FA**, a different feature, does use the defective stream for enrollment secret, nonce, and transport ECDH key. | The graphic conflates three mechanisms. Local-confirmation unpredictability is weakened. A host-provisioned HSM user secret may be exposed through a captured broken USB session, but its generation is not faulty. Web2FA is directly affected. None of those statements alone demonstrates an unauthenticated remote HSM bypass. |
| **Random PIN keypad shuffle**                          | Confirmed: the shuffle ultimately calls `ngu.random.uniform`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Predictability weakens the layout defense, but exploiting it requires observation or physical interaction and a separate route to learn the PRNG state. It is not a remote seed-recovery primitive.                                                                                                                                |
| **Side-channel masking**                               | Confirmed at the dependency level. libsecp256k1 context randomization is seeded via `my_random_bytes`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | A defense-in-depth mask becomes predictable. Key extraction still requires a physical side-channel capability and measurement/exploit work not established by this source review.                                                                                                                                                  |
| **Dice/imported master seed never in the path**        | Correct for the graphic's **direct PRNG callers**, with exceptions above. Deterministic BIP-85 material from a sound imported/dice seed does not touch the bad generator.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | It must not be generalized to every feature. Clone, ordinary teleport, USB, device-generated Key C, Web2FA, random Seed XOR, and generated passwords can be affected independently of master-seed origin. BIP-85 material from a weak device-generated seed is compromised transitively.                                           |

One important omission from the graphic is **random Seed XOR**. The optional random
split uses `ngu.random.bytes`; the default deterministic split does not
([`xor_seed.py:75-86`](https://github.com/Coldcard/firmware/blob/bcc2c382a324690a2fcf972c0bac3b79bf923f7b/shared/xor_seed.py#L75-L86)). Exploitation is conditional on obtaining a relevant
share and a public-key/address oracle; the bad random mask is not by itself a public
secret.

---

## 4. Assessment of the supplied K3 report

The report is valuable as a structured source-reading aid. Its pinned commits match
the relevant pre-fix state, and spot checks of its central call-chain references
agree with the firmware. Its most useful additions are:

1. **Clone is not standard backup.** The ordinary encrypted backup path used the
   direct hardware RNG for its password, salt, and IV. The clone path substituted
   predictable ephemeral ECDH keys. This is an important scope correction.
2. **USB carried more than public transaction data.** The `pass` command accepts a
   desktop-entered BIP-39 passphrase only inside the nominally encrypted channel.
   A captured and recoverable session can therefore have sharper consequences than
   the Wizardsardine article's short treatment suggests.
3. **The fix is fail-closed in more than one layer.** It adds a real board-level
   `rng_get`, makes inclusion of the fallback object fail compilation, and checks
   linked symbols during the build. That is stronger than merely changing one call
   site.

The following claims need correction or narrower wording:

1. **It does not provide a complete USB secret inventory.** The report lists the
   three commands that explicitly assert an encrypted request (`xpub`, `mitm`, and
   `pass`). In protocol version 2, however, binding rejects every later cleartext
   request, so other command traffic can also cross the compromised channel. HSM
   user provisioning and Storage Locker responses are examples. The correct unit of
   analysis is the captured session transcript, not a three-command list.
2. **Its HSM conclusion is internally incomplete.** The report correctly rejects
   the graphic's blanket claim, but says no device-side generation exists. The
   pinned `Users.pick_secret` implementation does exist and uses the correct
   hardware RNG. Host-supplied user secrets are a separate confidentiality issue if
   they traverse a recoverable USB session. Local-confirmation material remains a
   direct defective-PRNG caller.
3. **A predictable 28-bit PSBT-teleport index is not a predictable private key.**
   The report verifies the random call but does not carry the data flow through to
   the seed-derived keypair. A sound underlying seed preserves that private key.
4. **Several verified dependencies are not verified exploits.** Predictable PIN
   shuffling and secp256k1 blinding establish weakened defenses, not remote key
   extraction. Key C is affected only on the generated path. Feature-specific
   prerequisites must remain attached to those findings.
5. **The report is corroboration, not independent peer review.** Its distributor
   expressly said it was generated through K3 and had not been personally peer
   reviewed. Permanent source links make its claims auditable, but the report's own
   confidence icons do not substitute for reproduction or review.

Overall verdict: **high value as an index of code locations and scope questions;
medium value as analysis; low independent evidentiary weight until its conclusions
are reviewed or reproduced by humans.** The backup/clone distinction and USB
passphrase observation should be retained. Its blanket "every material claim"
language and feature-level exploit implications should not.

---

## 5. Wallet-policy and migration implications

### Analyze spending paths, not device count

For every descriptor or policy, enumerate the satisfiable branches. Mark which keys
were created by affected firmware, which were imported or dice-generated, and which
have independent passphrases. A policy is exposed if an attacker can satisfy any
branch using only recoverable material. Vendor diversity reduces common-mode risk
only when no one-vendor subset can satisfy a path.

### Script revelation can change the attacker's information

In SegWit script policies, spending can reveal the witness script and its key/path
structure. Taproot can keep unused script leaves hidden, but a used leaf reveals
that leaf. Migration may therefore reveal exactly the weak branch an attacker needs
or create a race after the old policy is disclosed. This does not imply that one
generic migration transaction is always wrong; it means the migration plan should
be wallet- and policy-specific and should follow current incident guidance.

### Passphrases reduce some risk but do not repair the defect

A genuinely random, independently generated BIP-39 passphrase can add meaningful
entropy to a weak mnemonic. Human-chosen passphrases are often cheap to test once
the mnemonic is known because BIP-39 uses a deliberately modest derivation cost.
Neither case fixes direct callers such as clone-session keys, generated passwords,
or Web2FA secrets.

### Inventory ancillary outputs separately

An affected-device inventory should include more than wallet seeds:

- paper-wallet keys;
- clone artifacts and ordinary Key Teleport exchanges;
- captured USB sessions and any passphrases or HSM data they carried;
- device-generated CCC/SSSP Key C and Web2FA enrollment;
- random Seed XOR shares;
- Secure Notes generated passwords; and
- any local-confirmation configuration relying on generated HSM material.

Standard encrypted backup files do not need to be treated as having weak file
encryption solely because of this bug. Their contained seed can still be weak, and a
clone artifact is not a standard backup.

---

## 6. Engineering assessment

The lasting lesson is not simply "test the RNG." It is to prove entropy provenance
through the shipped artifact:

1. **Make the approved entropy source explicit and purpose separated.** Hardware
   collection, health testing, DRBG state, nonces, long-lived secrets, ephemeral
   keys, and cosmetic shuffling should not be indistinguishable behind one ambient
   API.
2. **Fail closed on configuration values and linked symbols.** A macro defined as
   zero is not the same as an undefined macro. CI should inspect the final symbol
   graph and artifact, not only the intended source implementation.
3. **Test reachability, not random-looking output.** Statistical tests cannot
   distinguish a deterministic generator from a secure one. Tests should inject or
   attest the source and verify which provider every security-sensitive caller
   reaches.
4. **Maintain a caller inventory.** A seed-generation audit would have missed paper
   wallets, clone ECDH, USB, Web2FA, passwords, masks, and defense-in-depth uses.
   Each caller needs its own consequence and prerequisite analysis.
5. **Review dependency migrations as security-boundary changes.** The regression
   crossed firmware, MicroPython, libngu, compile-time configuration, and linker
   resolution. Review without the dependency-complete program is structurally
   unlikely to catch it; a focal-file session that can inspect the complete tree
   can reconstruct the chain.
6. **Model common-mode failure at the policy level.** Multiple devices from one
   implementation can be less independent than their physical separation implies.
   Multi-vendor designs reduce that risk but increase integration, backup, upgrade,
   and recovery complexity.

---

## 7. Bottom line

The core Coldcard RNG failure is confirmed and severe. Wizardsardine correctly
expands the analysis from weak wallet seeds to direct PRNG consumers and correctly
focuses multisignature users on satisfiable paths rather than nominal quorum size.
Its feature graphic is useful if read as a dependency overview, but its remote/local
legend compresses important prerequisites, and its HSM and PSBT-teleport boxes are
too broad.

The supplied K3 report materially improves two boundaries: clone files are affected
while standard encrypted-backup encryption is not, and captured USB sessions can
include BIP-39 passphrases. Its source map is strong, but it remains an unreviewed
AI-generated analysis with several conclusions that stop at a random call instead
of following the full security data flow.

For owners, updating firmware is necessary but does not repair existing material.
For engineers, the corrective standard is stronger than replacing Yasmarang: make
entropy provenance, final-artifact reachability, feature-call inventory, and
common-mode policy analysis continuously verifiable.

---

## Sources

Primary and near-primary sources:

- [Wizardsardine, “Critical Coldcard flaw: what happened, who is affected, and what to do”](https://wizardsardine.com/blog/coldcard-rng-vulnerability/)
- [Coinkite, “COLDCARD Entropy Issues: Technical Backgrounder”](https://blog.coinkite.com/entropy-technical-backgrounder/)
- [Block Engineering, “Predictable RNG fallback and 32-bit reseed in Coldcard firmware”](https://engineering.block.xyz/blog/predictable-rng-fallback-and-32-bit-reseed-in-coldcard-firmware)
- [Bitcoin Optech Newsletter #414](https://bitcoinops.org/en/newsletters/2026/07/31/)
- [Coldcard CCC documentation](https://coldcard.com/docs/coldcard-cosigning/)
- [Coldcard HSM local confirmation documentation](https://coldcard.com/docs/hsm/local-codes/)
- [Coldcard HSM user-management documentation](https://coldcard.com/docs/hsm/users/)
- [Coldcard Key Teleport documentation](https://coldcard.com/docs/key-teleport/)
- [Coldcard dice-roll documentation](https://coldcard.com/docs/verifying-dice-roll-math/)
- [Pinned affected Coldcard firmware](https://github.com/Coldcard/firmware/tree/bcc2c382a324690a2fcf972c0bac3b79bf923f7b)
- [Pinned affected libngu](https://github.com/switck/libngu/tree/537519a829259622ea6b0334fbafd6cae852852f)

Supplementary material:

- [Supplied K3 report paste](https://paste.rs/YEWpE.markdown), evaluated here as
  unreviewed AI-generated analysis rather than as an independent primary source.

---

## Addendum: Loupe-derived refinements

Addendum date: 2026-08-01.

This addendum is limited to what the Loupe evidence changes or qualifies in the
assessment above. It is not a synopsis of the Loupe corpus.

### Refinement to this assessment and the Wizardsardine article

The Coldcard experiment corrects the broad reading of section 6's statement about
file-local review and adds empirical support for the article's central source-level
causal chain.

- The [initial prediction](../loupe/2026-08-01-would-loupe-have-caught-coldcard.md)
  expected a per-file Loupe scan to miss the failure even if dependencies were
  present. The
  [pre-registered experiment](../loupe/2026-08-01-coldcard-prefix-experiment.md)
  refuted that prediction. Arm A, a default clone with empty submodules, scanned 12
  focal files, produced 12 findings, and missed. Arm B, with pinned libNgU and
  MicroPython source present, scanned 16 focal files, produced 22 findings, and hit
  the frozen rubric; the full causal chain appeared three times in the
  [results](../loupe/2026-08-01-coldcard-prefix-experiment-results.md).
- Loupe's focal file was not its evidence boundary: each session could inspect the
  mounted worktree. The accurate refinement is therefore not that file-focused
  analysis is structurally unable to find this class of failure. It is that review
  without the dependency-complete program is likely to miss a cross-repository
  configuration-and-linkage defect, while a focal-file session with the complete
  tree can reconstruct it.
- The Arm B result corroborates the Wizardsardine article's source-level account:
  the zero-valued board macro, libNgU's macro-existence check, MicroPython's
  deterministic fallback, symbol selection, and the use of that generator for
  wallet-secret material form a coherent chain visible in the pinned source.
- It does **not** prove which implementation was present in the exact shipped
  firmware artifact. The experiment used a known incident, hand-picked files, one
  run per arm, and one model family. Verification was disabled; `validate_poc`
  checked whether a patch applied but did not execute its test; and the run did not
  build the firmware or inspect its objects, link map, or image. As the
  [follow-up analysis](../loupe/2026-08-01-codex-analysis.md) notes, the vendor's
  build-time symbol-table assertion is stronger artifact evidence.
- Nothing in the Loupe result changes the feature-by-feature blast radius,
  prerequisite analysis, or remediation above. In particular, reaching
  `ngu.random` establishes a dependency, not remote exploitability without the
  relevant artifact, transcript, verifier, public oracle, policy path, or physical
  capability.

### Refinement to the Kimi/K3 assessment

The Loupe result raises confidence in one part of the supplied Kimi/K3 report and
does not resolve its overclaims.

- Loupe Arm B separately reconstructed the report's central cross-repository RNG
  chain from dependency-complete source. That is useful source-level corroboration
  of the causal map, not corroboration of every feature conclusion in the report.
- The report's statement that it independently verified every material technical
  claim remains too strong. Neither the Kimi/K3 report nor the Loupe experiment
  built and inspected the exact firmware artifact, executed an exploit
  reproduction, or supplied human peer review. Agreement between AI analyses does
  not promote a source claim into artifact or exploit proof.
- The report's useful scope refinements still stand: Clone Coldcard is distinct
  from ordinary encrypted backups, and a recoverable USB session can expose a
  desktop-entered BIP-39 passphrase.
- The corrections already recorded in section 4 also stand. The report's USB list
  is not a complete bound-session inventory; `Users.pick_secret` does provide
  device-side HSM user-secret generation through the working hardware RNG; a
  predictable 28-bit teleport index does not reveal a sound seed-derived private
  key; and predictable defense-in-depth randomness is not by itself a verified
  remote exploit.

### Refined bottom line

The Loupe experiment shows that a model with the complete pinned dependency tree
could reconstruct the known source defect, while Loupe's default incomplete clone
would have missed it. That narrows one review-method claim and corroborates the
central source narrative in both the Wizardsardine article and the Kimi/K3 report.
It does not upgrade either source analysis into proof of the shipped artifact or of
every downstream exploit claim, and it does not change the owner remediation
conclusion.

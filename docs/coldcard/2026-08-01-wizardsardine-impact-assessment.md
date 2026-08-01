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
   resolution. File-local review was structurally unlikely to catch it.
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

## Addendum: what the Loupe corpus changes

Addendum date: 2026-08-01.

Basis: all 12 documents in [`docs/loupe/`](../loupe/), including the Loupe source
study, the Omega scan, the pre-registered Coldcard prediction and experiment, the
experiment results, the independent defensive analyses, and the current forensic
roadmap.

### 1. The experiment corrects an important overstatement

The Loupe work first predicted that a per-file scanner would almost certainly miss
the Coldcard failure even with the relevant dependencies present. That prediction
was refuted.

The pre-registered experiment held the vulnerable Coldcard revision, model, effort,
prompt, and selected focal files constant:

| Arm | Mounted source                                   | Files | Findings | Frozen-rubric result |
| --- | ------------------------------------------------ | ----: | -------: | -------------------- |
| A   | Default clone; submodules empty                  |    12 |       12 | **MISS**             |
| B   | Pinned libNgU and MicroPython submodules present |    16 |       22 | **HIT**              |

Arm B reconstructed the complete causal chain three times. The strongest finding
identified the zero-valued macro, libNgU's existence test, MicroPython's deterministic
fallback, symbol resolution, and the use of `ngu.random.bytes()` for wallet-seed
material. Arm A produced plausible RNG hardening findings but did not identify the
defect.

The crucial correction is that Loupe assigns a focal file but exposes the whole
mounted worktree to the session. The winning board-header file existed in both arms;
only its surrounding dependency evidence changed. A focal-file scanner is therefore
not necessarily file-isolated. The statement above that file-local review was
unlikely to catch the failure remains valid only for a reviewer or harness that
actually restricts evidence to one file. It must not be generalized to Loupe's
worktree-visible sessions.

This result makes **input completeness** the first scanner invariant. Loupe's default
bare-clone path did not materialize submodules and did not announce the missing
program fraction. It still returned 12 findings, so the incomplete run looked
productive. A scan receipt must bind the exact commit, dependency revisions,
generated inputs, exclusions, and missing paths. If required source is absent, the
result is `incomplete`, not a comprehensive miss or a clean bill of health.

### 2. The hit is detection evidence, not final-artifact proof

The result does not establish that Loupe independently proved the shipped firmware
artifact:

- The Coldcard files were hand selected with hindsight. A cold operator would have
  faced 458 candidate Python/C/header files and would not already know to prioritize
  these 12–16.
- There was one stochastic run per arm using one model family. The known incident
  could be present in model knowledge, although the A/B divergence makes simple
  recall an incomplete explanation.
- Loupe verification was disabled because its verifier path was broken. The earlier
  Omega run produced 132 candidates and zero retained verdicts: the worker believed
  a verdict had been saved while the server had no verdict. Typed submission at one
  boundary did not protect the unacknowledged child-to-server flush boundary.
- `validate_poc` checked that a diff applied. It did not execute the proposed test.
- The Coldcard finding inferred which `rng_get()` would be selected. It did not build
  the exact board firmware and inspect the objects, link map, or final image.

In the Loupe evidence ladder, the experiment is T1 at best: a source-level claim
with an applicable PoC diff. The eventual Coldcard fix used stronger evidence—a
build-time symbol-table assertion that the intended board object provides `rng_get`
and the fallback object provides no RNG symbols. That is the decisive artifact-level
control.

The same boundary applies to the supplied K3 report. Its source map and the manual
spot checks in this assessment make it useful corroboration, but neither a detailed
model narrative nor agreement among models proves the linked release artifact.

### 3. Coldcard needs a claim ladder, not one vulnerability label

The forensic roadmap supplies a useful claim lattice. Applied here, the claims are:

1. **Source flaw:** can the dependency-complete source select the deterministic
   fallback for a secret consumer?
2. **Artifact selection:** which implementation did the pinned board build actually
   compile and link?
3. **Generator behavior:** what output follows from an exact initial state, reseed,
   and call trace?
4. **Exploitability model:** what candidate space follows from the hardware,
   firmware, UID, timers, interaction history, and attacker knowledge?
5. **Owned-fixture recovery:** can an independent implementation recover a known
   synthetic or explicitly owned xpub/address set?
6. **Historical program fingerprint:** which historical transactions share
   software-selected construction habits?
7. **Entity grouping:** which collectors and vaults are joined by observed graph
   edges?
8. **Unauthorized movement:** which transactions are tied to owner testimony that
   the movement was not authorized?
9. **Identity attribution:** who operated the recovery or collection system?

Evidence at one rung does not promote the next. A source hit is not artifact proof;
an entropy estimate is not a recovered wallet; a fee or transaction fingerprint is
not a person; a cluster is not a victim count; and movement on-chain is not proof of
unauthorized taking without owner evidence. This sharpens the time-bounded incident
caveats already stated in section 2.

It also explains why the ancillary-feature table must follow full data flow. A call
to the defective generator proves a dependency. It does not alone prove that an
attacker has the artifact, transcript, verifier, public oracle, policy path, or
physical capability needed to exploit that feature.

### 4. Replace the graphic's remote/local split with attacker economics

The Loupe analyses identify the most dangerous combination as:

> a reduced or structured candidate space plus a cheap, public, offline oracle.

That is a better prioritization rule than the graphic's binary remote/physical
legend. For each feature, assess four independent dimensions:

| Dimension        | Question                                                                                                                                                     |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Candidate space  | How many states remain after device, firmware, timing, call-trace, and attacker-knowledge assumptions?                                                       |
| Oracle           | Can a guess be verified publicly, offline, cheaply, and without rate limits?                                                                                 |
| Required capture | Does the attacker need an address, xpub, clone file, USB transcript, teleport payload, password verifier, Seed XOR share, or device observation?             |
| Consequence      | Does success expose a signing key, transported seed, passphrase, session confidentiality, policy factor, password, or only a defense-in-depth randomization? |

This ordering reinforces the feature conclusions above:

- Paper wallets are especially severe because the public address is an exact offline
  key verifier.
- Clone compromise can be tested against a durable encrypted artifact, but the
  attacker must obtain the clone file or exchange material.
- USB and ordinary Key Teleport require captured transcripts or payloads; their
  consequence depends on what crossed the session.
- Generated passwords require a verifier and may encounter online rate limits.
- Key C matters through the wallet's satisfiable spending policy, not merely because
  a key exists.
- PIN shuffle and side-channel masking weaken physical defenses but are not remote
  seed-recovery claims.
- The 28-bit multisig-teleport index remains an example of why a small random value
  is not automatically a small private-key space.

### 5. The prevention stack should be ordered by cost and evidence

Reading the full Loupe corpus reorders the defensive recommendations:

1. **Review policy first.** The four culprit changes recorded in the postmortem had
   no reviewer, and two had no pull request. A required reviewer for entropy, seed,
   key-derivation, board-configuration, and cryptographic-integration paths is the
   cheapest control.
2. **Materialize and attest inputs.** Fetch pinned submodules and required generated
   or vendored sources. Refuse a comprehensive result when the source/build graph is
   incomplete.
3. **Use source agents as candidate generators.** The complete-tree Loupe arm shows
   that a capable session can reconstruct the cross-repository mechanism from source.
4. **Prove symbol provenance in the build artifact.** Assert the exact provider of
   every security-critical symbol and forbid fallback providers. This is what makes
   the result deterministic and release-specific.
5. **Run fault builds.** Remove or disable the approved entropy provider; vary flags
   among undefined, zero, and one; change link order; and require secret creation or
   the build to fail closed.
6. **Trace runtime provenance.** On a faithful build or owned device, observe that
   secret creation reaches the approved source and aborts on source failure.
7. **Measure hardware properties separately.** Static and artifact provenance cannot
   prove that a physical entropy source is healthy, independent, or side-channel
   resistant.

Output randomness tests do not replace this stack. Hashing a small candidate set can
produce outputs that look statistically random while retaining the same small
candidate set.

### 6. What the scan methodology should learn from this incident

The corpus supports six practices that generalize beyond Coldcard:

- **Compare configurations.** The A/B divergence was the highest-value signal. One
  successful-looking run can preserve its own default's blind spot indefinitely.
- **Rank by security path and attacker economics.** Entropy, nonces, signing,
  firmware trust, wallet import/export, policy paths, and public oracles deserve
  priority over alphabetical file coverage.
- **Separate hypotheses from findings.** A question such as “does this seed path
  reach the approved source?” should survive as a typed unverified lead without
  becoming a vulnerability claim.
- **Execute evidence before external reporting.** A finding should not leave the
  private review boundary until its regression or invariant check has been observed
  failing on the vulnerable state and passing on the repaired state.
- **Measure misses and incomplete runs.** Finding count is not a success metric.
  Dependency gaps, verifier failures, retries, cancellations, unknown usage, and
  clean-control false positives stay in the denominator.
- **Use Coldcard as development data, not proof of generalization.** The known
  incident is contaminated by hindsight. Promotion needs renamed structural
  variants, unrelated historical failures, clean controls, and blinded holdouts.

The roadmap records OFR-001 through OFR-005 as implemented foundations for contracts,
managed GCE execution, immutable source delivery, the frozen Coldcard development
benchmark, and censor-aware metric evidence. This addendum did not independently
audit those implementations or their deployed state. The roadmap names OFR-006—the
configurable Loupe prompt/profile seam—as the next implementation gate.

### 7. Coordination is part of the security result

Loupe already supplies a disciplined candidate lifecycle. The remaining ecosystem
problem is allocating scans, comparing coverage, deduplicating private findings,
funding compute, coordinating disclosure, delivering fixes, and ensuring that fixes
persist.

The Coldcard experiment shows why configuration diversity matters: two otherwise
identical runs disagreed because one had the complete program. A useful scan ledger
would therefore record target, commit, source coverage, dependency policy, prompt,
model, effort, evidence tier, and missing inputs—not publish a context-free finding
count.

There is a dual-use boundary. A public project-by-project map of unscanned security
paths can become an attacker target list. Aggregate coverage can be public, while
specific gaps, findings, and divergence remain inside an authenticated coordination
group until disclosure. Hash commitments can establish discovery timing and private
dedup without revealing the vulnerability.

The durable output should be a proof-carrying fix: a minimal patch, an independently
re-executable failing-before/passing-after regression, pinned environment and source
digests, explicit assumptions, reviewer decision, and persistence watch. Human merge
review remains mandatory. The strategic asset is not model access or a new scanner;
it is a corpus of executed fixes plus maintainer trust.

### 8. Refined conclusion

The Loupe corpus does not change the user remediation conclusion: fixed firmware
does not repair previously generated weak material, and affected seeds and ancillary
secrets still require feature-specific assessment and rotation.

It does change the detection conclusion. Coldcard was not beyond AI source review in
principle. A dependency-complete, source-visible Loupe session found the full known
mechanism. The operational default still would have missed it because the required
dependencies were absent, and the hit still stopped before final-artifact proof.

The evidence supports a narrower lesson than either “AI could not find this” or
“source review was enough”:

> A defensible release process must combine complete inputs, risk-directed
> attention, and release-artifact proof.

Complete source lets an agent form the right hypothesis. A digest-bound build
invariant proves which implementation is in the checked artifact. Fault and runtime
tests prove that the secure path is load-bearing. A claim ladder prevents source
findings, chain fingerprints, and incident attribution from being collapsed
together. Coordination makes those checks repeatable across projects instead of
depending on one operator choosing the right configuration by chance.

### Loupe materials reviewed

- [`README.md`](../loupe/README.md)
- [`loupe-in-plain-words.md`](../loupe/loupe-in-plain-words.md)
- [`2026-07-31-omega-first-scan-preliminary.md`](../loupe/2026-07-31-omega-first-scan-preliminary.md)
- [`2026-07-31-omega-first-class-pentester-speculation.md`](../loupe/2026-07-31-omega-first-class-pentester-speculation.md)
- [`2026-07-31-fix-as-a-service-company-thesis.md`](../loupe/2026-07-31-fix-as-a-service-company-thesis.md)
- [`2026-08-01-would-loupe-have-caught-coldcard.md`](../loupe/2026-08-01-would-loupe-have-caught-coldcard.md)
- [`2026-08-01-coldcard-prefix-experiment.md`](../loupe/2026-08-01-coldcard-prefix-experiment.md)
- [`2026-08-01-coldcard-prefix-experiment-results.md`](../loupe/2026-08-01-coldcard-prefix-experiment-results.md)
- [`2026-08-01-codex-analysis.md`](../loupe/2026-08-01-codex-analysis.md)
- [`2026-08-01-hardening-against-ai-assisted-attacks.md`](../loupe/2026-08-01-hardening-against-ai-assisted-attacks.md)
- [`2026-08-01-coordination-not-scanners.md`](../loupe/2026-08-01-coordination-not-scanners.md)
- [`2026-08-01-omega-forensic-analysis-roadmap.md`](../loupe/2026-08-01-omega-forensic-analysis-roadmap.md)

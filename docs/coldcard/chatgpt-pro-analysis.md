# What happened

First, the date confusion: the mass theft began at **01:36 UTC on July 30, 2026**, which was **8:36 p.m. CDT on July 29** in Texas. Over roughly fifteen minutes, an automated operation swept **500 Bitcoin addresses containing 1,324 UTXOs and about 594.5 BTC**. At least some victims reported that their wallets had been generated on Coldcards. The confirmed Coldcard vulnerability explains how those private keys could have been reconstructed without physically accessing the devices. It has not yet been publicly established that every one of the 500 swept addresses came from Coldcard. ([Atlas21][1])

This was **not a breach of Coinkite’s servers**, theft of its firmware-signing key, or malware secretly inserted into Coldcard firmware. It was a latent software defect, introduced during a legitimate firmware refactor in 2021, that caused some Coldcards to generate wallet seeds from a predictable software pseudo-random generator instead of the device’s hardware random-number generator. An attacker who understood the defect could search possible seeds offline, compare the resulting addresses with addresses on the public blockchain, and recover the corresponding private keys. ([COINKITE Blog][2])

The particularly ugly part is that the Coldcard could remain completely air-gapped and uncompromised in every ordinary sense. The weakness happened **at key creation**. Once a wallet was born with insufficient randomness, no later security measure could make that original seed strong.

---

# The exact code failure

## 1. Seed generation was redirected during a 2021 crypto-library migration

Before firmware version 4, Coldcard created a seed using its own direct hardware-RNG interface:

```python
seed = bytearray(32)
rng_bytes(seed)
```

That reached `ckcc.rng_bytes()`, which read from the STM32 processor’s physical hardware RNG and performed failure checks.

During a March 2021 migration to Bitcoin Core’s well-regarded `libsecp256k1`, through an embedded library called libNgU, the code changed to approximately:

```python
seed = random.bytes(32)
```

That looked like an ordinary abstraction change. But `random.bytes()` now mapped to:

```text
ngu.random.bytes()
    → libNgU random code
    → CHIP_TRNG_32()
    → rng_get()
```

The crucial question was therefore: **which implementation of `rng_get()` did the linker select?** It selected the wrong one. The regression first appeared in the v4 firmware line released in March 2021. ([Block Engineering Blog][3])

## 2. Coldcard deliberately set an RNG configuration flag to zero

Coldcard’s board configuration contained:

```c
// We have our own version of this code.
#define MICROPY_HW_ENABLE_RNG (0)
```

The developers intended this to mean: *do not use MicroPython’s RNG implementation because Coldcard provides its own superior implementation.*

That assumption was reasonable at a glance, but it conflicted with how two separate codebases interpreted the flag. ([Block Engineering Blog][3])

## 3. LibNgU checked whether the flag existed—not whether it was enabled

LibNgU contained this guard:

```c
extern uint32_t rng_get(void);
#define CHIP_TRNG_32() rng_get()

#ifndef MICROPY_HW_ENABLE_RNG
#error "get a HW TRNG plz"
#endif
```

The bug is the use of `#ifndef`.

`#ifndef MICROPY_HW_ENABLE_RNG` asks:

> Has this macro been defined at all?

It does **not** ask:

> Is the macro’s value true or nonzero?

Because Coldcard defined the macro as `0`, the macro existed, so the safety check passed. The build did not fail. Had the check been something equivalent to:

```c
#if !MICROPY_HW_ENABLE_RNG
#error "hardware RNG required"
#endif
```

the mistake would have been caught during compilation. ([Block Engineering Blog][3])

## 4. MicroPython interpreted the same zero correctly—and enabled its fallback PRNG

MicroPython used the macro in the conventional way:

```c
#if MICROPY_HW_ENABLE_RNG
    // STM32 hardware RNG
#else
    // Yasmarang software fallback
#endif
```

Since the value was zero, MicroPython compiled its fallback `rng_get()` implementation.

Coldcard’s own board-specific RNG code exposed functions such as `random32()` and `random_buffer()`, but at that time it did not export the exact global symbol:

```c
uint32_t rng_get(void);
```

LibNgU nevertheless required that symbol. The linker found a perfectly valid function with the correct name and signature in MicroPython’s fallback object, so the firmware built and ran normally. No linker error indicated that the wrong implementation had won. ([Block Engineering Blog][3])

That is the heart of the vulnerability:

```text
What developers believed:

ngu.random.bytes()
    → Coldcard hardware TRNG

What actually happened:

ngu.random.bytes()
    → MicroPython rng_get()
    → deterministic Yasmarang software PRNG
```

---

# What the fallback RNG actually used

MicroPython’s fallback was not designed for generating high-value cryptographic keys. MicroPython’s own source comments describe it as a pseudo-RNG provided for systems without a hardware RNG and say that it is “not really ideal.”

Its initial state was constructed approximately as follows:

```c
pad = UID_low32 ^ SysTick->VAL;
n   = RTC->TR;
d   = RTC->SSR;
dat = 0;
```

Those values are:

* The low 32 bits of the processor’s fixed factory identifier.
* A processor tick-counter value.
* The real-time clock’s time register.
* The real-time clock’s subsecond register.

After that one-time initialization, every output followed the deterministic Yasmarang state-transition algorithm. No fresh physical entropy was collected on subsequent calls. ([GitHub][4])

Those inputs can make outputs *look* random, but they are poor cryptographic secrets:

* A chip UID is an identifier, not secret entropy.
* The tick counter has a small and structured range.
* The RTC values are tied to boot and execution timing.
* The timer values are correlated rather than independent.
* Devices running the same firmware tend to reach the first RNG call through similar execution paths.

Researchers estimate that controlled measurements from an attacker-owned Coldcard could help prioritize the likely timing states on victim devices. The exact practical narrowing still needs more empirical hardware testing, but this is radically smaller than searching a genuinely random 128- or 256-bit secret. ([Block Engineering Blog][3])

---

# A second PRNG did not save it

LibNgU also maintained its own Yasmarang generator, initialized with public constants:

```c
static uint32_t yasmarang_pad = 0x0a8ce26f;
static uint32_t yasmarang_n   = 69;
static uint32_t yasmarang_d   = 233;
static uint8_t  yasmarang_dat = 0;
```

It combined the two streams like this:

```c
chip = rng_get();       // unexpectedly MicroPython Yasmarang
chip ^= my_yasmarang(); // libNgU Yasmarang
```

This can look like “mixing two randomness sources,” but both streams were reproducible. XORing two predictable values merely produces another predictable value:

```text
predictable A XOR predictable B = predictable C
```

The code also performed a health check that rejected repeated adjacent outputs. That catches a catastrophically stuck hardware RNG returning the same number over and over. It does not catch a deterministic PRNG, because an ordinary deterministic PRNG produces plenty of different-looking numbers. ([Block Engineering Blog][3])

---

# Hashing did not restore the missing entropy

Coldcard then performed something like:

```python
seed = ngu.random.bytes(32)
assert len(set(seed)) > 4
return ngu.hash.sha256d(seed)
```

The assertion checked that the output was not trivially constant. Yasmarang easily passed it.

The double-SHA-256 operation made the resulting seed appear statistically random, but hashing is not an entropy generator. Suppose an attacker only has to consider one trillion possible RNG outputs. Hashing each output still produces only one trillion possible hashes—it does not magically create the full (2^{256}) possibilities implied by a 32-byte result.

In other words:

```text
small candidate set
       ↓ SHA-256
different-looking small candidate set
```

The BIP-39 checksum in the mnemonic words similarly detects errors but contributes no secret entropy. ([Block Engineering Blog][3])

This is one reason the flaw was difficult to notice by looking at generated seed words: the words looked completely normal.

---

# Why Mk4, Q and Mk5 were less broken—but still affected

Later Coldcards added a defense-in-depth reseeding step using the devices’ secure elements:

```python
a = callgate.read_rng(1)     # 32 bytes associated with SE1
b = callgate.read_rng(2)     # 8 authenticated bytes from SE2

n = ngu.hash.sha256d(a + b)
n, = ustruct.unpack("I", n[0:4])
ngu.random.reseed(n)
```

There were two major limitations.

First, although the hash was 32 bytes, the firmware retained only its first **four bytes**:

```text
4 bytes = 32 bits
```

Second, `ngu.random.reseed()` merely replaced one 32-bit word in libNgU’s Yasmarang state:

```c
STATIC mp_obj_t random_reseed(mp_obj_t arg)
{
    yasmarang_pad = mp_obj_get_int_truncated(arg);
    return mp_const_none;
}
```

It did not initialize a cryptographically secure DRBG, replace the MicroPython fallback state, reset the remaining Yasmarang state, or preserve all the secure-element input. For any fixed device/timer state and RNG call history, this introduced at most (2^{32}) securely distinguished possibilities. ([Block Engineering Blog][3])

Coinkite’s current preliminary estimates describe Mk3 seeds as having an effective search space of approximately **40 bits**, and later Mk4/Q/Mk5 seeds as approximately **72 bits**, rather than the intended minimum of 128 bits. Block’s analysis gives a more nuanced interpretation: the later devices have at most 32 bits of secure-element uncertainty once the fallback state is fixed, while a very loose enumeration that also treats every timer field as unknown can approach (2^{73}). That does **not** mean these devices had 72 clean bits of cryptographic entropy; much of that estimate consists of reconstructable or correlated timing state. ([COINKITE Blog][2])

For perspective, each additional bit doubles attack cost. A properly random 128-bit seed is unimaginably beyond brute force. A search involving 32–40 uncertain bits can be practical, particularly when candidates can be heavily precomputed, partitioned by firmware/device behavior, or checked using GPUs and public blockchain data. The precise cost for an individual wallet depends on what the attacker knows about its device UID, timing, firmware, and RNG-call history; Block explicitly has not claimed one universal end-to-end brute-force benchmark. ([Block Engineering Blog][3])

---

# How the attacker could turn this into stolen bitcoin

The attacker did not need to guess English seed words one by one. A likely attack pipeline is:

1. Reimplement the exact MicroPython and libNgU PRNGs.
2. Enumerate plausible UID/timer/boot-state/call-count combinations.
3. Produce the same 32 RNG bytes the Coldcard would have produced.
4. Apply the same SHA-256, BIP-39 and BIP-32 derivation steps.
5. Derive common Bitcoin receive paths and addresses.
6. Compare each candidate against funded addresses on the public blockchain.
7. When an address matches, derive its private key and sign a sweep transaction.

A wallet address, xpub or other generated public key acts as a **validation oracle**: the attacker does not have to wonder whether a candidate is correct. They derive its public address and compare it to known public data. A match confirms the candidate. ([Block Engineering Blog][3])

The peculiar on-chain pattern is consistent with this kind of operation. The July 29/30 attacker made one sweep transaction per address rather than clearly consolidating all addresses belonging to each reconstructed hierarchical wallet. Of the 500 swept addresses, 490 were native SegWit, five were legacy, and five were nested SegWit. Analysts have suggested this could indicate a tool that searched particular derivation paths or recovered individual keys rather than comprehensively reconstructing every wallet, but that remains an inference from the transaction pattern—not a confirmed description of the attacker’s program. ([Atlas21][1])

The attacker’s exact exploit code has not been published. Neither has the precise method used to obtain or constrain device identifiers and timing states for each victim. The firmware defect itself, however, is now confirmed and provides the essential mechanism needed for offline key reconstruction. Block published early because it assessed that active exploitation was underway, while noting that it had not yet completed full empirical testing of every scenario. ([Block Engineering Blog][3])

---

# Why years of reviews missed it

Several individually plausible decisions combined into a catastrophic end-to-end failure:

**The cryptographic migration appeared to improve security.** Moving elliptic-curve operations to Bitcoin Core’s `libsecp256k1` was a sound choice. The vulnerability was in how the new library obtained randomness, not in `libsecp256k1` itself. ([COINKITE Blog][2])

**The same macro had two meanings.** LibNgU checked whether `MICROPY_HW_ENABLE_RNG` was defined, while MicroPython checked whether its value was nonzero.

**The wrong function had the correct name and type.** Both implementations satisfied `uint32_t rng_get(void)`, so normal compilation and linking succeeded.

**The correct TRNG code was still in the firmware.** Reviewers could inspect the binary, find Coldcard’s carefully written hardware-RNG implementation, and conclude it was present without proving that wallet-seed generation actually reached it.

**The generated output looked random.** A deterministic PRNG plus SHA-256 produces visually normal bytes and normal BIP-39 words.

**The tests checked superficial failure symptoms.** Repeated-output and distinct-byte checks detect a stuck generator, not insufficient entropy provenance.

**There was no end-to-end build assertion.** Nothing required the linker’s `rng_get()` symbol to come specifically from the board’s hardware-RNG object. Coinkite says prior review verified the intended code’s presence but did not trace symbol resolution and call reachability across the Coldcard, libNgU and MicroPython submodules. ([COINKITE Blog][2])

This is a classic security-engineering lesson: verifying that secure code exists is not the same as verifying that security-critical callers actually use it.

---

# How the fix works

The hotfix makes the desired relationship explicit rather than depending on accidental linker behavior.

Coldcard’s board-specific RNG code now exports the exact required function:

```c
uint32_t rng_get(void)
{
    return rng_get_or_fault();
}
```

The build also deliberately prevents MicroPython’s fallback `rng.c` from supplying any symbols. A new `rng-code-check` examines the resulting object files and fails the build unless:

* The board-specific RNG object defines the global `rng_get`.
* MicroPython’s upstream fallback RNG object defines no RNG symbols.

That converts a subtle runtime security assumption into a mechanically enforced build invariant.

The fixed releases are:

| Device/track     |       Fixed firmware |
| ---------------- | -------------------: |
| Mk3              |   **4.2.0 or later** |
| Mk4/Mk5 standard |   **5.6.0 or later** |
| Q standard       |  **1.5.0Q or later** |
| Mk4/Mk5 Edge     |  **6.6.0X or later** |
| Q Edge           | **6.6.0QX or later** |

Coinkite’s current advisory officially identifies Mk3 firmware **4.0.1 through 4.1.9** as affected. Block’s source-history analysis traces the vulnerable generation path to the v4.0.0 release as well, so a seed generated on v4.0.0 should also be treated conservatively as suspect. Seeds generated on Mk4, Mk5 or Q before their corresponding fixed release are affected too, although the secure-element reseed made their situation less severe than Mk3’s. ([COINKITE Blog][5])

---

# What affected owners must understand

**Installing fixed firmware does not fix an existing seed.** The seed—and therefore every private key derived from it—was determined at its original creation. Restoring the same words onto a fixed Coldcard, another hardware wallet, or a software wallet preserves the weakness. The remedy is to update first, generate a completely new seed, verify its addresses, send a small test transaction, and then move the remaining funds. ([COINKITE Blog][5])

A strong, unique BIP-39 passphrase supplies an additional independent secret that may substantially impede the attacker. The device PIN does not. Coinkite nevertheless recommends replacing an affected seed even when a strong passphrase was used. ([COINKITE Blog][5])

There is also a dice exception. When the original wallet was created, at least **50 genuinely random, private six-sided-die rolls** supplied approximately 128 bits of independent entropy; 99 rolls supplied approximately 256 bits. Coinkite does not consider such seeds endangered by this RNG defect alone. Fabricated, recorded, exposed, reused or fewer-than-50 rolls do not qualify. ([COINKITE Blog][5])

The flaw was not limited conceptually to master seeds. Block found the same `ngu.random` construction underlying random paper-wallet keys, Seed XOR masks, some cloning and encrypted-communication keys, generated Secure Notes passwords, and other secrets. The impact differs by feature, but those values inherit the same entropy limitation when they were generated through the affected path. ([Block Engineering Blog][3])

TAPSIGNER, OPENDIME and SATSCARD use different codebases and are not affected. A sound seed created independently elsewhere is not made predictable merely by importing it into a Coldcard; the central defect concerns secrets generated through the affected Coldcard RNG path. ([COINKITE Blog][5])

# Bottom line

The Coldcard failure was an extraordinarily consequential **dependency-integration and symbol-resolution bug**:

```text
zero-valued configuration macro
        + incorrect #ifndef guard
        + missing board-level rng_get symbol
        + same-named MicroPython fallback
        + noncryptographic timer-seeded PRNG
        + hashes that concealed the weak source
        = predictable Bitcoin wallet seeds
```

Nothing needed to “break” Bitcoin, extract a secure element, compromise the air gap, or hack the Coldcard remotely. The attacker attacked the much smaller set of keys the flawed firmware was capable of creating. Once one of those candidate keys produced a publicly funded address, the blockchain itself confirmed the guess.

[1]: https://atlas21.com/594-bitcoin-drained-15-minutes-theft/ "594 BTC stolen in 15 minutes: what we know"
[2]: https://blog.coinkite.com/entropy-technical-backgrounder/ "Technical Deep Dive into the Entropy Issue | COINKITE Blog"
[3]: https://engineering.block.xyz/blog/predictable-rng-fallback-and-32-bit-reseed-in-coldcard-firmware "Predictable RNG Fallback and 32-Bit Reseed in COLDCARD Firmware | Block Engineering Blog"
[4]: https://github.com/micropython/micropython/blob/master/ports/stm32/rng.c "micropython/ports/stm32/rng.c at master · micropython/micropython · GitHub"
[5]: https://blog.coinkite.com/coldcard-mk3-seed-generation-warning/ "Coldcard Security Advisory | COINKITE Blog"



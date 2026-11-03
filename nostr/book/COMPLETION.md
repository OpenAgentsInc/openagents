# Completion route

The book has 102 implemented teaching profiles and two partly implemented
chapters. That count is not a count of complete NIPs or deployed applications.
This route separates those exits and orders the remaining work. The current
source baseline is `6f4c722640`; dated verification records retain the scope
and source identities of the runs they describe.

## Keep the four claims separate

| Claim | What establishes it | What it does not establish |
| --- | --- | --- |
| Teaching profile implemented | Worked trace, runnable example and transcript, broken case and independent check | Every feature of a cited NIP |
| Target qualified | Recorded execution of the stated profile on that target | An entire application or every browser engine |
| Source component adopted | A reviewed direct consumer in `chapters.json` | Adoption of the whole chapter or production approval |
| Production exit passed | Consumer-specific integration, operational evidence and the required owner decisions | Approval for unrelated consumers, keys or deployments |

The registry currently claims CPU qualification for all 104 chapter profiles
and Wasm qualification for 21. Do not infer Wasm support for the other 83.
The result column in [README](README.md#chapters) and each chapter's omissions
define the protocol subset taught. Chapter 97's report is a frozen example
from `73966b6359`, not a report generated from the current registry.

## 1. Close known integration and evidence debt

- **Settlement expiry:** the live API calls SQL
  `inference.topup_settle`, which has no expiry check at settlement.
  `coax/lib/x402/settle.coax` checks expiry, but the route does not call it.
  Resolve the implementation choice recorded in [NEEDS_OWNER.md](../../NEEDS_OWNER.md)
  and check the actual paid retry path, including the expiry boundary.
- **NIP-KEYS source pin:** review the added spec before re-pinning the
  OpenAgents folder. The current folder digest begins `fb7563e7`; the pin in
  `coax/lib/nostr/profiles.coax` and `profiles.json` begins `2fb1f51f`.
  The pin check remains failing until that review and update. A teaching
  chapter's completion does not bless a new protocol pin.
- **Consumer regressions: done 2026-11-03.** With generated project inputs,
  the project page's suites pass after the NIP-22 refusal change
  (`project-checks.coax` 88 of 88, both native suites, the NIP guide
  observer), and the project page's five-REQ relay differential matches the
  Rust relay's recording ([record](../../docs/verification/2026-11-03-nostr-consumers.md)).
  The Rust differential for COUNT, search and NIP-11 was refreshed in
  `57c3f6e63a` (#12631).
- **Scope acceptance:** retain the owner's pending review of the chapter 101
  and 102 done-criteria rewrites. Their offline examples do not replace the
  live application exits below.

The [chapter audit record](../../docs/verification/2026-11-01-nostr-book-chapters-11-104.md)
also records issues that have since been fixed. Do not reopen the compiler
borrow/alias fixes, host-supplied NIP-49 workspace, large-file hashing,
BIP-39 phrase limits or completed libsecp256k1 crosschecks merely because the
older record lists them. Reuse passing evidence for unchanged inputs.

## 2. Qualify the consumers and their target boundaries

The homepage's public verification and watch, the API's NIP-98 verification,
the CLI's signing and the Pylon runtime are real Coax consumers. Their direct
source use is recorded in `adopted_by`; it is narrower than adopting every
protocol feature in a chapter.

The API's `nostr.c` already calls `nip98_check_coax` from the checked generated
C. Preserve its route-specific replay, session and account rules. The hosted
runner still signs with libsecp256k1, and `coax/web/unlock/lib.ts` still uses
`nostr-effect` for the presentation opener. Migrate those actual consumers
with their existing identities, privacy behavior and rehearsed rollback.
The [web inventory](../../coax/nostr.md#live-web-surface-migration-inventory-2026-11-01)
separates the completed verification cutover from these remaining signers.

Qualify complete named consumer profiles on the native and browser targets
they will use. `coax/lib/nostr/consumers.coax` now names ten consumers and,
for each, how it signs, verifies, keeps keys and carries events, in which
file and with which record; dated reports generated from it are in
[reports/](reports/README.md). On 2026-11-03 none of the ten is complete. A Wasm arithmetic run, an iOS compile or an external NIP-07
signer does not qualify the complete phone-signer or browser client profile.
The signer source audit has completed, but real Pylon keys still require the
owner decision and a safe host key-file path: `psionic/lib/pylon/run.coax`
continues to refuse `--key-file`. Do not turn test-key evidence into approval
for production secrets.

This work depends on [Coax's verification boundaries](../../coax/book/ch23-verification-boundaries.md)
and its [complete boundary](../../coax/book/ch24-a-complete-boundary.md), as
well as the target-specific compiler and host evidence each consumer needs.

## 3. Finish the relay and its production migration

The running Coax relay now uses indexes, deletion, rate limits, COUNT, search,
protected events, optional proof of work, NIP-77, NIP-86 (with `--admin`) and
NIP-29 (with `--groups`, short of relay-signed group state). NIP-43 and the
rest of NIP-29 wait for the owner's relay-key decision. The 2026-11-03
[parity table](../../docs/verification/2026-11-03-coax-relay-parity.md) lists
what the Rust relay advertises that the Coax relay doesn't (17, 59, 65, 67,
94, 98). Its private-kind read rules are ported whole
(`relay_private.coax`) and match it in the differential. Do not advertise a library module merely because its chapter is
implemented.

`--store-capacity EVENTS` now selects host-allocated arrays. The default is
still 2,048 records and 4 MiB; it is no longer a hard maximum. The recorded
3,000-event replay at capacity 13,000 checks that allocation and replay path.
[Chapter 103](ch103-replace-relay-openagents-com.md) now also records the later
synthetic 50,000-event profile: 20.3 seconds to replay and 32.3 seconds to
restart (`fae56496a0`). Preserve that bounded improvement rather than treating
the earlier 3,000-event example as the latest measurement. Neither profile
qualifies the production event set, total original bytes, tombstones,
production memory/restart budgets or traffic. Select and measure those
budgets against an approved restored-backup export. The durable import
(`relay --import`), the restart check (`relay --check-store`) and the
freshness checkpoint (`--checkpoint`, `store_fresh.coax`) now exist and are
measured on synthetic exports against an independent oracle
([record](../../docs/verification/2026-11-03-coax-relay-import-restart.md)).
Admission now reads ID, address and tombstone lookups instead of scanning
(same decisions, same digests at 3,000 to 60,000 lines; 250,000 lines import
in about 5 minutes, almost all signature verification), the tombstones are
sized from the input, and `relay --export` writes the store back out as an
export for the rollback rehearsal; all in the same record.

Then follow [chapter 103's runbook](ch103-replace-relay-openagents-com.md#the-runbook):
import with every signature checked, compare the served event sets, shadow
normal traffic, classify every difference, rehearse rollback and obtain the
owner's cutover approval. Keep the Rust relay available until those exits
pass. Configurable capacity does not authorize reading production data or
changing DNS.

## 4. Connect the offline integrations to applications

| Integration | What exists | Required next result | Other book |
| --- | --- | --- | --- |
| Pylons and mesh, chapter 100 | Runtime NIP-42/NIP-44 exchange and checked beacons; test-key evidence | Consumer target and key-custody qualification; preserve provider, job and attempt binding | [Psionic 24](../../psionic/book/ch024-a-pylon-answers-decisions.md) and [25](../../psionic/book/ch025-the-api-fans-out.md) |
| Verse and wallet, chapter 101 | Offline NIP-MV and zap composition; existing Spark wallet rules | Two real clients exchanging frames within a measured latency budget, then an app zap with actual wallet payment and counted receipt; Spark-backed NWC | [Aethyr 49](../../aethyr/book/ch049-rooms-and-presence.md) |
| Agents, chapter 102 | Offline attestation, sealed job, atomic claim, journal and supplied payment proof | Runtime NIP-AA admission; schema-checked CJ/LAB binding; signed market/journal carriage; real wallet payment; staged runner migration and rollback | Coax boundary qualification and the Psionic Pylon/mesh consumers |

The chapter 101 example assumes 40 ms per hop; it does not measure sockets.
On 2026-11-03 two native clients exchanged signed NIP-MV frames through a
local Coax relay with test keys (median 5 ms); the relay's default per-key
rate refuses frames above one a second, which is an owner decision. The
Pylon's provider, job and attempt binding and the coder host's relay-carried
jobs also ran end to end with test keys; no wallet test environment exists
here ([record](../../docs/verification/2026-11-03-nostr-consumers.md)).
The chapter 102 example checks a supplied preimage; it does not pay an invoice.
Record those distinctions until the corresponding applications supply evidence.

For broader protocol completion, take the omitted records and paths in
chapters 63–92 one named consumer at a time. Examples include HOST enrollment
and routing, WORK/CTX records, RUN/SESS carriage, public agents and skills,
market disputes, full attestation verification, FI fetch/session binding,
and the remaining Block channel and private-state records. Preserve the
explicit Bitcoin-only exclusions in chapter 74.

## 5. Finish treasury signing and independent recovery

[Chapter 104](ch104-agent-treasuries-and-a-network-of-keys.md#not-built)
currently admits descriptors and signing requests and refuses mismatched
PSBTs. It produces no partial signature. Add the required ECDSA/BIP-143
signing and possession proofs, wire the private request transport and
coordinator, and run the stated three-agent 2-of-3 signet spend. Then recover
from a coordinator outage with an independent wallet tool using one agent's
descriptor copy. Miniscript spending, Taproot, PSBT v2 and other declared
omissions need their own scope and evidence; the policy checks alone do not
complete them.

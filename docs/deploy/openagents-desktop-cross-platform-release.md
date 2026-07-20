# OpenAgents Desktop cross-platform release ProductSpec

- ProductSpec version: 1.2.0
- Date: 2026-07-18
- Status: normative target contract. Support remains evidence-gated
- Owner: OpenAgents Desktop release authority
- Parent program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Source audit: [T3 Code and OpenCode Electron build/update analysis](../teardowns/2026-07-16-t3-code-opencode-electron-build-update-analysis.md)
- Operator runbook: [OpenAgents Desktop production release](./openagents-desktop-production-release.md)

## 1. Normative language and status truth

`MUST`, `MUST NOT`, `REQUIRED`, `SHOULD`, and `MAY` are normative. This
document owns cross-platform Desktop distribution policy. Code, a maker
configuration, a declared runner, or a downloadable candidate does not by
itself make a target supported.

There are three distinct states:

1. **Target contract** is the behavior required by this specification.
2. **Implemented** means the required build/update path exists and its bounded
   automated gates pass.
3. **Supported** means a promoted release has native clean-install, launch,
   agent-runtime, update, interruption, shutdown, and applicable rollback
   receipts for the exact target and format.

As of this version, ReleaseSet v2 and the public resolver exist. The release
coordinator contracts also exist. Concrete owned-worker adapters and the
complete five-target native receipt set are not yet converged. Therefore
**none of the five target keys is admitted as cross-platform supported by this
spec**. Existing experimental GitHub prereleases are discovery artifacts, not
evidence of signed-feed promotion or stable support.

Public copy and `/download` MUST derive availability from promoted evidence.
An absent or unadmitted target MUST be labeled unavailable, never inferred
from this target contract.

## 2. Product, install, and channel identities

Stable and RC are separate installed applications. They MUST be able to run
side by side. They MUST NOT share mutable state, credentials, browser storage,
or update state. This prohibition includes rollback slots, protocol handlers, and telemetry labels.
Stable never installs over RC and RC never migrates stable state.

| Identity                         | Stable                                                              | RC                                                         |
| -------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------- |
| Product/display name             | `OpenAgents`                                                        | `OpenAgents RC`                                            |
| Executable stem                  | `OpenAgents` on macOS/Windows. `openagents` on Linux                | `OpenAgents RC` on macOS/Windows. `openagents-rc` on Linux |
| macOS bundle ID                  | `com.openagents.desktop`                                            | `com.openagents.desktop.rc`                                |
| Windows application/installer ID | `com.openagents.desktop`                                            | `com.openagents.desktop.rc`                                |
| Windows AppUserModelID           | `OpenAgents.Desktop`                                                | `OpenAgents.Desktop.RC`                                    |
| Linux application ID             | `com.openagents.desktop`                                            | `com.openagents.desktop.rc`                                |
| Linux package name               | `openagents-desktop`                                                | `openagents-desktop-rc`                                    |
| Linux desktop entry              | `com.openagents.desktop.desktop`                                    | `com.openagents.desktop.rc.desktop`                        |
| Linux `StartupWMClass`           | `OpenAgents`                                                        | `OpenAgents-RC`                                            |
| Deep-link scheme                 | `openagents`                                                        | `openagents-rc`                                            |
| OAuth public client/redirect     | `openagents-desktop`. IPv4 loopback callback on an OS-assigned port | same client/callback contract. Separate credential custody |
| Update product                   | `openagents-desktop`                                                | `openagents-desktop`                                       |
| Update channel                   | `stable`                                                            | `rc`                                                       |
| Tag namespace                    | `openagents-desktop-v<version>`                                     | `openagents-desktop-v<version>`. Prerelease tag only       |

The canonical Electron `userData` directory name is `OpenAgents` for stable
and `OpenAgents RC` for RC beneath the operating system's application-data
root. Development uses `OpenAgents Dev` and cannot consume either production
feed. Browser/session partitions, persisted registries, usage outboxes, and
update slots inherit that channel isolation. Any migration from the historical
same-identity RC MUST be explicit, one-way, receipt-backed, and MUST NOT copy
secrets without the native session-custody contract.

Retired Khala Code and Autopilot identifiers, state roots, feeds, and tags
MUST NOT be reused.

## 3. Target keys, minimum operating systems, and support scope

The ReleaseSet target key is a closed enum:

- `darwin-arm64`
- `darwin-x64`
- `win32-x64`
- `linux-arm64`
- `linux-x64`

Windows is x64-only. `win32-arm64` is outside the current ReleaseSet,
promotion, `/download`, and support contract. The dormant target-staging
descriptor and runtime package aliases are non-promotable compatibility
scaffolding only. Adding Windows ARM64 later requires a reviewed ProductSpec
and ReleaseSet policy revision plus native Windows-on-Arm acceptance evidence.

Owner amendment 2026-07-20 (#8920) makes `win32-x64` an OPTIONAL, experimental
cell. This amendment removes `win32-x64` from the signed ReleaseSet, the
promotion required-cell set, `/download` signed availability, and auto-update.
The signed required cells are therefore only `darwin-arm64`, `darwin-x64`,
`linux-arm64`, and `linux-x64`. A missing or unsigned `win32-x64` MUST NOT block
convergence or atomic promotion, and `win32-x64` makes no support claim.

Windows x64 ships only as an unsigned experimental portable that stays outside
the signed feed. A future signed Windows path needs its own issue. That path
restores the `Valid` Authenticode publisher gate from exactly `OpenAgents, Inc.`
before publication and install. This amendment does not change any macOS or
Linux signing, notarization, or native-receipt requirement.

Minimum supported environments are:

| Target         | Minimum supported environment                                                    | Native proof requirement                                             |
| -------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `darwin-arm64` | macOS 13.5 Ventura on Apple Silicon                                              | Apple Silicon host at the minimum version and current macOS          |
| `darwin-x64`   | macOS 13.5 Ventura on an Intel Mac                                               | Intel host at the minimum version and current supported macOS        |
| `win32-x64`    | Windows 10 22H2 x64                                                              | Clean Windows 10 22H2 and current Windows 11 x64                     |
| `linux-x64`    | glibc 2.35, Linux kernel 5.15, X11 or Wayland. Ubuntu 22.04 LTS is the reference | Native x64 reference host plus one current RPM-family host for RPM   |
| `linux-arm64`  | glibc 2.35, Linux kernel 5.15, X11 or Wayland. Ubuntu 22.04 LTS is the reference | Native arm64 reference host plus one current RPM-family host for RPM |

Linux support is bounded to 64-bit glibc distributions meeting that floor.
musl-only distributions, 32-bit systems, containers without a desktop
session, and unsupported Electron environments are outside the support claim.

Raising a minimum version is a ProductSpec revision and release-notes event.
A release MAY continue to run on an older system but MUST NOT claim support
without the native gate above.

## 4. Package formats and install boundaries

| Target family | Required formats   | Install owner                                                                     | Update claim                                                                                      | Rollback claim                                                                                                          |
| ------------- | ------------------ | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| macOS         | DMG and ZIP        | DMG: user drag/install. ZIP: user or managed deployment                           | DMG is the preferred in-app full-artifact update. ZIP is direct/managed download only             | Retained previous application slot after DMG update. ZIP has no app-owned rollback claim                                |
| Windows       | NSIS EXE           | Per-user, one-click NSIS installer. No elevation and no per-machine mode          | App may verify and hand off the full signed NSIS installer                                        | Retained previous per-user application slot after first-launch failure. Installer/uninstaller repair remains NSIS-owned |
| Linux         | AppImage, DEB, RPM | AppImage: unprivileged user. DEB/RPM: explicit user/admin package-manager handoff | AppImage may update by verified full-image replacement. DEB/RPM are direct-download handoffs only | AppImage retains the previous image. DEB/RPM have **no in-app rollback claim**                                          |

No APT, YUM, DNF, or other package repository is in scope. Direct DEB/RPM
downloads MUST NOT imply repository signing, unattended package-manager
updates, dependency-repository service, or application-owned rollback.
Adding a package repository requires a ProductSpec revision, repository-key
custody, metadata signing, expiry/rotation policy, and native repository
upgrade evidence.

Windows installation is per-user only. Per-machine installation, enterprise
MSI, Microsoft Store distribution, and automatic elevation are out of scope.

## 5. Publisher and signing authorities

The expected platform identities are fixed:

- Apple Developer ID Application: `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`.
- Apple Team ID: `HQWSG26L43`.
- Windows Authenticode subject/publisher display: `OpenAgents, Inc.`.
- Linux package vendor: `OpenAgents, Inc.`. Package IDs are defined in section 2.
- Release-selection signature: the existing pinned OpenAgents Ed25519 release
  key and key-ID contract. The private key is isolated from build workers.

Production packaging MUST fail closed when the required signing authority is
unavailable. There is no unsigned production fallback. Local unsigned builds
MUST carry the conspicuous `UNSIGNED-DEV` marker and are ineligible for
receipts, upload, candidate admission, `/download`, or promotion.

The following are mandatory before promotion:

- macOS: sign the application and all nested native code. Notarize the required
  layers. Sign, notarize, and staple the DMG and app. Pass the `codesign`,
  `spctl`, staple, Team ID, entitlements, bundle-ID, architecture, and
  mounted-download checks.
- Windows: sign the installer, installed app executables, uninstaller, bundled
  CLIs, native helpers, and other executable payloads. Windows trust APIs and
  `Get-AuthenticodeSignature` MUST report `Valid` with publisher
  `OpenAgents, Inc.` before publication and immediately before install.
- Linux: the signed ReleaseSet digest/length gates every format. DEB/RPM
  metadata and payload architecture/ownership/mode MUST be inspected. Package
  manager install, upgrade, reinstall, and uninstall MUST pass natively.

TLS, object ACLs, package metadata, Electron Updater YAML, Git tags, and
GitHub Releases are transport or discovery only. None may replace the pinned
Ed25519 ReleaseSet signature.

## 6. Artifact names and immutable object identity

Every public artifact basename is self-describing and immutable:

```text
OpenAgents-<version>-<channel>-darwin-<arch>.dmg
OpenAgents-<version>-<channel>-darwin-<arch>.zip
OpenAgents-<version>-<channel>-win32-<arch>-setup.exe
OpenAgents-<version>-<channel>-linux-<arch>.AppImage
OpenAgents-<version>-<channel>-linux-<arch>.deb
OpenAgents-<version>-<channel>-linux-<arch>.rpm
```

`<channel>` is exactly `stable` or `rc`. `<arch>` is exactly `arm64` or
`x64`. Artifact names, URLs, SHA-256 digests, byte lengths, source revision,
version, target, format, component-ledger digest, and build receipt ref are
inside the signed ReleaseSet. An object key MUST NOT be overwritten. Rebuilt
bytes require a strictly newer version.

The current v1 macOS basename without a channel remains a bounded migration
input only. ReleaseSet v2 publication uses the names above.

## 7. Version and channel policy

All artifacts in one ReleaseSet MUST have the same exact semantic version and
source revision. Stable versions are `X.Y.Z`. RC versions are
`X.Y.Z-rc.N`. Stable rejects prereleases. Both channels are strictly
monotonic. Equal versions, downgrades, and version reuse are refused.

Promotion selects an already-verified immutable ReleaseSet. It MUST NOT
rebuild artifacts. RC-to-stable promotion may reuse identical artifact bytes
only if a separate signed stable ReleaseSet satisfies the version and channel
identity rules. The installed RC and stable identities must remain
separate. Remote downgrade and `allowDowngrade` behavior are prohibited.

Rollback means a local transition to the immediately retained previously
installed slot after typed first-launch failure. It does not mutate the
channel pointer or authorize an older remote manifest.

## 8. ReleaseSet selection and trust

ReleaseSet v2 is the single selection authority defined by [#8915](https://github.com/OpenAgentsInc/openagents/issues/8915).
It MUST be a bounded, canonical, schema-validated document signed by the
pinned Ed25519 release key. It contains exactly the four signed required
target cells (`darwin-arm64`, `darwin-x64`, `linux-arm64`, `linux-x64`) and
their required formats when a complete cross-platform set is promoted. Per the
#8920 owner amendment, the optional `win32-x64` cell never enters this signed
document. It also
contains the bounded, reviewed human release-notes text and digest-bound refs
for the full human and agent changelog artifacts defined in §15.1. Changelog
generation and review therefore finish before the final canonical bytes are
signed.

Selection is deterministic:

1. Determine the installed product identity and channel.
2. Determine the host OS/architecture using the native main process.
3. Map to exactly one closed target key.
4. Reject a missing/extra target, wrong channel, non-monotonic version,
   incompatible minimum OS, invalid signature, unknown key ID, wrong artifact
   identity, or unsupported format.
5. Select the target's preferred update format (DMG, NSIS, or AppImage).
6. Download immutable bytes and re-check signed length and SHA-256 before the
   platform verifier/applier receives them.

A small channel pointer MAY select an immutable versioned ReleaseSet. The
pointer must be signed or resolve only to content with a verified pinned
signature. A partial target set can exist only as a candidate.
it cannot become stable/RC current.

The v1 single-artifact manifest remains readable only for the bounded existing
macOS arm64 migration window specified by #8915. New targets never publish v1.

## 9. Target-specific staging and component ledger

Each target build starts from an exact source revision in a clean staging
workspace for that target. It MUST install the locked production dependency
closure. This requirement includes native components with an explicit target
triple. It also permits only approved resources and a target-capable
owned worker.

Each worker emits a machine-readable component ledger containing, at minimum:

- release version, source revision, lockfile digest, target key, OS image and
  toolchain identity.
- Electron, Node, pnpm, Forge/maker, Rust, and compiler versions.
- every bundled provider runtime, CLI, native Node module, shared library,
  helper, WASM module, executable architecture, version, SHA-256, provenance,
  and signing state.
- ASAR/unpacked placement and package-content allowlist result.
- every output artifact name, digest, length, and format.

Host-installed optional packages or global CLIs MUST NOT satisfy staging or
proof. A foreign-architecture or unledgered executable fails the target.

## 10. Build, signing, install, and provenance receipts

Receipts are public-safe references, not raw secret-bearing logs. At minimum
the coordinator retains:

- common source/preflight receipt.
- per-target build and component-ledger receipt.
- per-artifact platform-signing/package receipt.
- candidate upload and downloaded-byte receipt.
- native clean-install, first-launch, agent-runtime, shutdown, N-1 update,
  interruption/resume, rollback-or-explicit-no-rollback, reinstall, and
  uninstall receipt.
- complete-set convergence, signer, candidate-feed, promotion, `/download`,
  telemetry, post-promotion, and mobile-feed-preservation receipts.

Receipt schemas MUST bind version, channel, source revision, target, artifact
digest, worker identity, test-host identity class, timestamps, and gate
results. They MUST NOT include secrets, credentials, raw prompts, private
paths, machine identifiers, or customer content.

## 11. Owned runner inventory and orchestration

Required owned capacity is an inventory contract, not a statement that the
hosts exist today:

| Capability slot        | Build requirement                                      | Native acceptance host                     | Current admission         |
| ---------------------- | ------------------------------------------------------ | ------------------------------------------ | ------------------------- |
| `desktop-darwin-arm64` | Apple Silicon macOS worker                             | Apple Silicon minimum/current OS           | not admitted by this spec |
| `desktop-darwin-x64`   | Intel macOS worker                                     | Intel minimum/current OS                   | not admitted              |
| `desktop-win32-x64`    | x64 Windows worker                                     | Windows 10 22H2 and current Windows 11 x64 | not admitted              |
| `desktop-linux-x64`    | native x64 glibc worker                                | reference DEB- and RPM-family x64 hosts    | not admitted              |
| `desktop-linux-arm64`  | native arm64 glibc worker                              | reference DEB- and RPM-family arm64 hosts  | not admitted              |

[#8917](https://github.com/OpenAgentsInc/openagents/issues/8917) owns the
concrete runner registry. Each admitted entry MUST record an opaque runner ID,
ownership, native OS, and architecture. The entry also records the image and
toolchain revision, signing access class, and test-host coverage. It records the
attestation key, last-known-green receipt, and
quarantine state. No raw address or credential enters public receipts.

GitHub Actions and GitHub-hosted CI are prohibited. Owned orchestration fixes
one source revision, version, and channel. It runs common gates one time. It
starts the four required target builds and validates the native receipts. It
converges all required cells and requests an isolated signature. The optional
`win32-x64` portable, when built, stays outside this signed convergence.

Then, it serves a candidate and
performs one atomic promotion.
Signing workers never accept arbitrary source or unsigned receipt sets.

### 11.1 Canonical one-command release entrypoint

After [#8926](https://github.com/OpenAgentsInc/openagents/issues/8926) lands,
the only documented production entrypoint is the root package script:

```sh
pnpm run release -- --channel <stable|rc> --version <semver> \
  --trigger-kind <owner_direction|agent_change|tester_feedback|release_incident> \
  --trigger-actor <public-actor> --trigger-ref <public-ref> \
  [--source-feedback <OpenAgents-issue-URL>]
```

The `release` script MUST map exactly to
`node --import tsx scripts/release.ts`. Supported control flags are `--dry-run`
(fixture workers and no cloud spend), `--yes` (approve only gates declared
safe for unattended use), and `--resume <transaction-ref>` (continue one
durable idempotent transaction). The only release-channel human gate is a
stable release lacking current explicit owner direction. It is named before
effects and never becomes a silent prompt or stall.

The command freezes inputs. It checks for credentials without printing secrets.
It starts and verifies the GCE and Tailnet inventory. It runs all five builds
and native gates.

It generates both changelogs. It signs and smokes the
candidate. It promotes atomically. It verifies `/download`, homepage CTAs, `/changelog`, and
mobile preservation, then writes exactly one final public-safe receipt named
`docs/deploy/receipts/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md`.

It is idempotent and resumable. No failure before final promotion may mutate
the current channel pointer. The existing v1 compatibility procedure remains
the only temporary exception until its #8915 migration closes.

The default owned substrate is GCE in project `openagentsgemini`. GCE provides
Linux x64, Linux arm64, and Windows x64. It uses the existing scoped automation
service-account pattern. Owned Tailnet Macs provide both Darwin architectures.
Substrate availability never relaxes the native receipt gate.

### 11.2 Release-impact selection

Before provisioning a worker, the coordinator MUST classify the exact changed
paths between the last delivered source revision and the candidate:

- web-only changes deploy the web lane.
- mobile-only JavaScript/assets/config changes use the existing signed Expo OTA
  lane when its runtime-version contract admits them, otherwise they use the
  mobile native-build lane.
- `oa-updates` or release-infrastructure-only changes deploy/test those owned
  services and MUST NOT manufacture a Desktop version.
- documentation-only changes create no binary release.
- any Desktop main/renderer/native/package change, a Desktop-consumed shared
  package change, or the root lockfile triggers the complete four-target
  required Desktop matrix.

This is deterministic product-path classification after the release route has
already been selected. It is not user-intent or tool routing. Overlapping paths
select every affected lane. Unknown product paths fail closed to
`no_binary_release` plus an operator-visible explanation.

A renderer-only Desktop OTA is intentionally **not admitted** in version 1.2.0.
Before that lane may skip native packaging, a later ProductSpec revision MUST
define a signed renderer and runtime compatibility envelope. The revision must
also define immutable content identity and atomic activation. It defines first-launch
health proof, a retained fallback,
rollback receipt, and CSP/native-bridge compatibility gate. Until then,
"renderer-only" still means the complete Desktop matrix.

## 12. Update, first-launch, and rollback lifecycle

The common update host owns check state, signed target selection, download,
hash/length verification, child-process drain, first-launch receipt state, and
typed failure. Platform appliers own only native verification and replacement.

Before replacement, the application MUST drain Codex, Claude, ACP providers,
PTYs, audio helpers, local servers, and other child processes. The current
slot remains untouched until candidate verification succeeds. The previous
slot is retained until the new slot records a bounded healthy first launch
including provider-runtime startup and clean shutdown.

After power loss or process death, each durable transition MUST converge during
restart. It must select the old healthy slot, the verified candidate, or a
typed recovery state. It must never select a partly installed, unverified slot. A failed first launch invokes
the format-specific rollback claim in section 4. Repeated rollback is bounded
and reported without retry loops.

DEB/RPM update handoff records the verified package and package-manager
outcome, but never reports app-owned rollback. A user may use distribution
package-manager history according to that distribution's policy. OpenAgents
does not claim or automate it.

## 13. Publication, feeds, and mobile preservation

Google Cloud Storage candidate objects and the `oa-updates` Google Cloud Run
service are the publication path. Immutable candidate upload is not
promotion. The coordinator MUST refuse convergence unless all required
artifacts and receipts agree on version, channel, source revision, identities,
and digests.

Candidate feed verification downloads and re-verifies every target. Promotion
atomically changes the signed channel selection. It never copies, mutates, or
rebuilds artifacts. Post-promotion probes repeat target resolution and byte
verification through `updates.openagents.com`.

Every `oa-updates` build/deploy MUST preserve the existing mobile OTA assets,
manifest routes, headers, runtime/channel selection, and known-good mobile
probe. A Desktop-only metadata directory MUST NOT replace the baked mobile
export. Mobile preservation failure blocks Desktop promotion and rolls service
traffic back to the previous ready Cloud Run revision.

GitHub Releases MAY carry a non-authoritative prerelease mirror before signed
feed promotion. Named testers can then exercise immutable candidate bytes. The
publisher MUST create a draft. It uploads only manifest-verified local
paths.

It checks GitHub's server-reported digest for each asset before it makes the
prerelease public. A published tag/version is immutable: assets MUST NOT be
replaced in place, and corrected bytes require a strictly newer RC version.
The release body records the trigger kind, trigger ref, source revision, and
release actor. It records the exact authority revision or grant. It records if
the candidate is a complete signed ReleaseSet or a limited experimental build.

GitHub is never a client feed, signed-feed promotion barrier, completeness
oracle, or `/download` source of truth. Signed-feed promotion still requires
the complete ReleaseSet and native receipts. Candidate/publication messages to
linked GitHub issues and the Forum `release-candidates` board are bounded,
idempotent release-transaction communications. They are not a general outbound
communications grant.

## 14. Platform acceptance gates

### 14.1 macOS

Both architectures require DMG and ZIP convergence. The downloaded DMG and
mounted app MUST pass the signing/notarization checks in section 5. Bundle
identity, executable architecture, minimum OS, hardened runtime, entitlements,
Electron fuses, ASAR integrity, icon, nested runtime origin, and Team ID MUST
match. Clean install and N-1 update run on native Intel and Apple Silicon.
Translation/architecture migration is an explicit full-artifact scenario, not
a differential update. ZIP is verified as a managed/direct artifact but has
no app-owned update or rollback claim.

### 14.2 Windows

Windows x64 requires the per-user NSIS artifact. The downloaded
installer and every installed executable listed in the component ledger MUST
pass Windows trust with publisher `OpenAgents, Inc.`. Clean install, protocol
registration, Start menu/taskbar identity, uninstall, N-1 update,
interruption, locked-file drain, first launch, and retained-slot rollback run
on native acceptance hosts. No test or configuration may disable update code
signature/publisher verification.

### 14.3 Linux

Both architectures require AppImage, DEB, and RPM convergence. Package IDs,
desktop entries, executable names, icons, MIME/protocol integration,
`StartupWMClass`, dependencies, modes, ownership, architecture, install,
upgrade, reinstall, and uninstall MUST match sections 2–4. AppImage must prove
verified full-image replacement, executable-bit preservation, interruption,
first launch, and retained-image rollback. DEB/RPM prove explicit
package-manager handoff and an honest no-in-app-rollback receipt.

## 15. `/download`, public claims, and telemetry

`openagents.com` consumes a typed resolver derived from the same verified,
promoted ReleaseSet used by clients. `/download` MUST NOT contain handwritten
artifact URLs or infer support from an OS name. It selects platform and
architecture when detectable, always exposes explicit alternatives, shows
version/channel/format/minimum-OS truth, and renders unavailable targets
without a dead link.

Public CTA availability requires a promoted ReleaseSet plus the native
support receipt for that target/format. Direct DEB/RPM copy says
"download package" or "install with your package manager," never "automatic
updates" or "rollback." RC is conspicuously prerelease and never presented as
stable.

Download telemetry records only a server-generated event ref, day-bucketed
timestamp, version, channel, target, format, resolver outcome, and a bounded
first-party CTA/referrer category. It MUST NOT record prompts, paths, account
identity, credentials, durable machine IDs, raw user-agent strings, or IP
addresses in the product event. Telemetry counts successful resolver/download
responses, not installs, launches, users, or updates. Operational access logs
follow the service retention/privacy contract and are not product counters.

### 15.1 Human and agent changelogs

Every release publishes two reviewed, public-safe artifacts before ReleaseSet
signing:

- Human source:
  `docs/changelog/human/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md`.
  It explains what changed, why it matters, requirements, and honest caveats in
  user language. It MUST NOT expose commit-hash soup, internal codenames, lane
  names, or implementation ontology. Its bounded summary is embedded directly
  in the signed ReleaseSet release-notes field and drives `/changelog`,
  `/download`, and the in-app update prompt.
- Agent ledger:
  `docs/changelog/agent/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md`.
  Each entry records issue refs, commit refs, changed contracts/specs,
  invariants, evidence refs, and the public-safe CLAIM actor/session. It is
  linked from the human entry but is never the primary user-facing copy.

Both artifacts and the `/changelog` projection MUST identify the trigger kind
and exact trigger or source-feedback refs. They identify the release actor
and exact delegated authority revision or grant. They can instead identify the
historical pre-profile authority truth. They identify the source revision
and public release URL.

Attribution describes who or what caused the
release and who executed it. It never retroactively rewrites historical
authority. Under `AUTHORITY.md` revision 2, the delegated release operator MAY
review and publish an RC changelog without a second owner ceremony. A stable
release still requires current explicit owner direction.

`docs/changelog/UNRELEASED.md` is the sole accumulator. #8927 owns its bounded
entry schema and the requirement that CLAIM-RELEASE appends one entry. The
release command at §11.1 consumes the exact since-last-release range. It
requires reviewed human text and writes both immutable dated files.

It rolls the accumulator
forward idempotently, and supplies bounded notes plus refs/digests to
ReleaseSet v2. `/changelog` renders human releases newest-first with honest
empty and degraded states. It does not become release authority.

## 16. Retention, revocation, rollback, and support

Retention minimums are:

| Record                                                                                                                                                           | Retention                                                                               |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| ProductSpec, ReleaseSet schema versions, signed promoted manifests, both dated changelogs, embedded release notes, source revision, promotion/revocation records | indefinite                                                                              |
| Stable artifacts, component ledgers, build/signing/native acceptance receipts                                                                                    | seven years and never less than current plus N-1                                        |
| RC artifacts and complete receipts                                                                                                                               | 180 days after supersession and never while current                                     |
| Failed/unpromoted candidates and bounded failure receipts                                                                                                        | 30 days unless retained for an incident                                                 |
| Secret-free detailed worker logs                                                                                                                                 | one year. Longer only under incident policy                                             |
| Aggregate download telemetry                                                                                                                                     | per the public telemetry/privacy retention contract. Never promoted as install evidence |

Retention archives may become non-public but MUST preserve digest-bound audit
evidence. Deleting or hiding an artifact never performs a client rollback.

A compromised key, invalid platform signature, malware finding, or severe
release defect triggers a signed revocation/typed unavailable response and
removal from `/download`. Clients fail closed. Recovery is a strictly newer
signed release. Remote downgrade remains prohibited. Locally retained-slot
rollback remains available only under the installed format's claim.

Support answers are bounded to the exact target, minimum OS, format, channel,
and receipts above. Experimental builds, source builds, unsigned development
packages, foreign distributions, per-machine Windows installs, package
repositories, and manually modified bundles are unsupported.

## 17. Owner-only actions and readiness rule

Owner-only actions include access to Apple Developer and App Store Connect.
They include Windows code-signing provider accounts and certificates. They
include production Ed25519 key custody and Cloud DNS administration. They also
include certificate or account administration and enrollment of
owned native runner hardware. Automation MUST consume these only through
scoped secret/runner seams and MUST never inspect or print credentials.

After those scoped seams exist, `AUTHORITY.md` revision 2 delegates RC impact
selection and owned builds to the release operator. It also delegates candidate
publication, requested-tester outreach, and structured feedback intake. The
delegation includes linked issues, Forum status, changelog publication,
signed-feed promotion, and rollback.

Stable publication and bulk unsolicited outreach remain reserved. Unsafe
overlay or renderer OTA remains reserved. Partial-matrix feed promotion and
unsigned production artifacts remain reserved. Asset replacement, version
reuse, and a credential ceremony outside a scoped seam remain reserved.

This ProductSpec does not itself make any such action ready. Add an action to
root `NEEDS_OWNER.md` only after its implementation lands. The exact UI or
account operation and least privilege must be known. Automation must be ready
to verify the result immediately. A generic request to "get signing" or
"add runners" is not a ready owner action and MUST NOT be listed.

## 18. Retained invariants and evidence boundaries

The following invariants never weaken during this program:

| Invariant                                    | Automated boundary                                                    | Release receipt                                       |
| -------------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------- |
| Pinned Ed25519 manifest authority            | ReleaseSet schema/canonicalization/signature/selection tests          | signer + downloaded-byte verification                 |
| Strict monotonic update. No remote downgrade | model/property tests and publisher/current-feed tests                 | candidate convergence + promotion refusal/success     |
| Fail-closed production signing               | credential-absence and unsigned-marker tests                          | platform signing receipts                             |
| macOS app and outer-DMG trust                | Gatekeeper/notary/staple contract tests and native scripts            | downloaded mounted-artifact receipt per architecture  |
| Complete matrix before promotion             | coordinator model and missing/duplicate/mismatch tests                | four-target convergence receipt                       |
| Native target proof before support           | target/format acceptance registry tests                               | native clean-install/update/rollback boundary receipt |
| Mobile feed preservation                     | `oa-updates` route/asset regression and candidate probe               | pre/post-promotion mobile manifest receipt            |
| No GitHub Actions/hosted CI authority        | repository authority guard                                            | owned coordinator/runner attestations                 |
| `/download` equals promoted truth            | resolver/schema/route/accessibility tests                             | public resolution/download receipt                    |
| Dual changelogs are signed release inputs    | accumulator/generation/bound/idempotence and `/changelog` route tests | dated human+agent artifacts and signed notes refs     |
| Release attribution is exact                 | publication/comms/changelog authority and idempotence tests           | trigger/actor/authority refs on every public update   |
| Impact selection cannot overbuild            | deterministic changed-path table tests                               | selected-lanes receipt before worker provisioning     |
| Channel/state isolation                      | identity/state-root model and migration tests                         | clean side-by-side install receipt                    |

## 19. Delivery issue map

Every implementation child cites this ProductSpec rather than creating local
policy:

| Issue                                                                    | Owning sections                                                |
| ------------------------------------------------------------------------ | -------------------------------------------------------------- |
| [#8914 DIST-01](https://github.com/OpenAgentsInc/openagents/issues/8914) | This document, deployment index, runbook, and invariant ledger |
| [#8915 DIST-02](https://github.com/OpenAgentsInc/openagents/issues/8915) | §§6–8, 10, 16, 18                                              |
| [#8916 DIST-03](https://github.com/OpenAgentsInc/openagents/issues/8916) | §§3, 6, 9–10, 14                                               |
| [#8917 DIST-04](https://github.com/OpenAgentsInc/openagents/issues/8917) | §§7, 10–11, 13, 18                                             |
| [#8918 DIST-05](https://github.com/OpenAgentsInc/openagents/issues/8918) | §§4, 7–8, 12, 16                                               |
| [#8922 DIST-09](https://github.com/OpenAgentsInc/openagents/issues/8922) | §§8, 13, 16, 18                                                |
| [#8919 DIST-06](https://github.com/OpenAgentsInc/openagents/issues/8919) | §§2–7, 14.1, 16                                                |
| [#8920 DIST-07](https://github.com/OpenAgentsInc/openagents/issues/8920) | §§2–7, 14.2, 16                                                |
| [#8921 DIST-08](https://github.com/OpenAgentsInc/openagents/issues/8921) | §§2–7, 14.3, 16                                                |
| [#8923 DIST-10](https://github.com/OpenAgentsInc/openagents/issues/8923) | §§8, 13, 15–16                                                 |
| [#8924 DIST-11](https://github.com/OpenAgentsInc/openagents/issues/8924) | §§1, 4, 15–16                                                  |
| [#8927 DIST-14](https://github.com/OpenAgentsInc/openagents/issues/8927) | §§8, 11.1, 15.1–16, 18                                         |
| [#8926 DIST-13](https://github.com/OpenAgentsInc/openagents/issues/8926) | §§10–13, 15–16, 18                                             |
| [#8925 DIST-12](https://github.com/OpenAgentsInc/openagents/issues/8925) | §§10–16, 18                                                    |
| [#8993](https://github.com/OpenAgentsInc/openagents/issues/8993)         | §§11.2, 13, 15.1, 17–18, autonomous RC delivery/comms          |
| [#8995](https://github.com/OpenAgentsInc/openagents/issues/8995)         | §§6–7, 13, 15.1, RC17–RC20 tester-feedback incident evidence   |

The program closes only after each child meets its own close rule and one
stable ReleaseSet has the complete public-safe evidence required by #8913.

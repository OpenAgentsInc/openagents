# Coax Native Updates

The initial UIKit/JavaScriptCore delivery profile is implemented in
[`native/`](native/README.md). Its published contract is
[`coax.native.package.v1`](../docs/protocol/coax-native-package-v1.md).
It provides signed whole packages, installed runtime matching, generation
replay protection, staged activation and local fallback. The broader design
below also describes capabilities that this first profile does not implement.
Build 1070 replaces the WebKit profile of 1068–1069 with a new runtime identity.
The old 1069 feed remains compatible with that installed binary.

Written 2026-11-13. Status: specification extending [Coax Native](native.md),
with the initial signed-package client/service and native-control execution
profile implemented. Later stages remain proposals; they require an
implementer, date and bounded workload before implementation.

Coax Native Updates delivers signed application code and content to an
installed Coax Native runtime. The goal is to update as much of the product
as its installed runtime and distribution rules permit: views, actions,
navigation, ordinary app logic, supported realm scripts, assets and remote
services. A new native build is needed when the installed runtime cannot
execute the change or its distribution profile requires a binary release.

This is our counterpart to Expo Updates, using Coax-generated artifacts and
our own infrastructure. It is a first-class framework feature, not merely an
asset downloader or a development hot-reload feature. Coax Native Web is the
browser target; Coaxial is its former public name. The separate book and
product migration is tracked in [#13512](https://github.com/OpenAgentsInc/openagents/issues/13512).

## The execution boundary determines the maximum

Today Coax app logic is generally compiled to C11 and linked into a native
executable, or compiled to Wasm for the browser. Downloading a replacement
C file or native object cannot update an installed iPhone executable. The
maximum portable OTA surface requires a deliberate second execution profile:
Coax app logic compiled to Wasm, executed by a runtime already in the binary.
Both profiles use the same Coax domain and component libraries.

```text
installed native runtime                     signed update
  boot supervisor and update verifier          Coax app Wasm modules
  platform SDK adapters                       view composition and actions
  native control and GPU backends              routes and ordinary app rules
  capability and secret boundaries             compatible assets and content
  portable module executor                     supported realm scripts/shaders
                 |                              |
                 +--- versioned app/service ABI-+

Coax Native Web: browser Wasm executor + DOM/WebGPU + browser services
```

The installed runtime owns trust roots, executable loaders, capability
ceilings, OS permissions, secrets and the recovery supervisor. A payload
can compose and call only those services. Adding a new native service,
entitlement, SDK, execution engine or unsupported instruction requires a new
runtime. Changing code already represented in the portable profile need not.

There is no universal percentage of updateable source. Report it per app as
the changed modules, bytes and effects that can be delivered for its installed
runtime. The objective is to make most frequently changed app code portable;
performance-critical native code remains a measured tradeoff. AOT compilation
does not make an artifact OTA-capable on a platform that cannot load it.

## Expo patterns adapted

[Expo's client](https://docs.expo.dev/versions/latest/sdk/updates/) separates
checking, downloading and reloading, identifies runtime compatibility, and
supports embedded and downloaded updates. Its default launch behavior can
download an update for a later restart. We take that separation, with an
explicit safe activation boundary instead of unconditional UI reload.

| Pattern | Coax Native design |
| --- | --- |
| Runtime version | Generated fingerprint of the installed service/execution contract, not a manually reused marketing version |
| Channel and release stream | Signed deployment assignment to immutable payloads, separately scoped by platform, runtime and distribution profile |
| Manifest and assets | Signed exact manifest bytes, required artifact closure, SHA-256 content addresses and bounded staging |
| Check, fetch, reload | Check, download, verify, stage, trial and commit; activation waits for the app's safe boundary |
| Embedded launch | Complete immutable embedded app plus a smaller independent recovery screen |
| Rollout and rollback | Stable cohorts, new signed deployment generations and storage-compatible recovery |
| Update inspection | Current/pending payload, runtime, launch source, failure reason and local logs through shared state and driver |

[Expo's protocol](https://docs.expo.dev/technical-specs/expo-updates-1/)
describes manifests, launch assets, asset hashes and directives.
[Code signing](https://docs.expo.dev/eas-update/code-signing/),
[rollouts](https://docs.expo.dev/eas-update/rollouts/) and
[error recovery](https://docs.expo.dev/eas-update/error-recovery/) are useful
references. Our control-plane signatures, ordering and protocol shapes are
our own; this spec does not claim compatibility with EAS clients or servers.
An Expo-compatible gateway would require a separate adapter and conformance
work, not adoption of Expo's wire IDs for our records.

## Platform and distribution profiles

Technical execution support, runtime compatibility and distribution eligibility
are three independent gates. A signed payload must pass all three. The
installed binary pins its distribution profile; downloaded code cannot switch
an App Store install into a development or direct-distribution profile.

| Profile | Maximum intended OTA scope | What still needs another delivery mechanism |
| --- | --- | --- |
| Open web and PWA | Coax-generated Wasm app/realm code, DOM composition, WebGPU shaders, assets and server changes supported by the browser | New browser capabilities and permissions; service-worker/offline adoption requires its own lifecycle qualification |
| iOS App Store | Within the reviewed app purpose: compatible web/Wasm experiences and, if eligible, interpreted Coax modules through installed services; exact eligibility is recorded per release | Native executable/framework/SDK replacement, new entitlements or native APIs; unapproved feature expansion |
| Android Google Play | Compatible portable interpreted/VM code and assets through installed indirect service APIs | Native DEX/JAR/shared-library downloads as a substitute for Play updates; new native services/permissions need a Play runtime release |
| Mac App Store | Only artifacts admitted for that store profile; do not inherit direct-download desktop behavior | Changes requiring an app/store update, including native runtime replacement |
| Direct desktop distribution | Portable modules and content; also signed native/full-app packages through the existing qualified platform updater | OS signing/notarization, package ownership and restart requirements still apply |
| Development and managed installs | The profile's explicit development/management capabilities | Provisioning and platform rules remain; debug access is not an App Store entitlement |

Apple's [review rules](https://developer.apple.com/app-store/review/guidelines/)
restrict downloaded feature-changing code (2.5.2), hidden features (2.5.19),
and software offered under 4.7. That category has additional requirements for
content, commerce, native APIs, consent, indexing and age controls. Apple's
[program agreement](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/),
section 3.3.1 B–C, permits interpreted code subject to conditions while also
restricting additional functionality. These provisions must be satisfied
together; an interpreter, a signature or a web view is not blanket approval.

For iOS, classify first-party app updates separately from third-party realms,
plugins and mini apps. If an experience is offered under 4.7, a generic native
bridge cannot silently expose platform APIs to it; required Apple permission
and per-experience user consent are part of eligibility. No payload hides
functionality from review, impersonates content to carry native code, or
changes purchase behavior to evade applicable distribution requirements.

Google's [Device and Network Abuse policy](https://support.google.com/googleplay/android-developer/answer/16559646?hl=en)
restricts external self-updates and downloaded native executable code, with
an exception for VM/interpreter code using indirect platform access. Our
portable profile is designed for that boundary; policy compliance still
depends on what the code does.

These platform readings were checked on 2026-11-13 and are design constraints,
not a guarantee of acceptance. A publisher must attach a current eligibility
record for the binary/profile, feature delta and artifact classes. An unknown
case remains unqualified: use a reviewed binary release or the standalone
web product rather than relabeling code as data. No store-profile signing
rule can be disabled by an update or a remote flag.

Direct Android APK replacement can update the whole native runtime through
the [platform package installer](https://developer.android.com/reference/android/content/pm/PackageInstaller)
and its signing/authorization rules; it is
not the Play OTA module channel. Any Apple alternative distribution uses its
[authorized installation/update mechanism](https://developer.apple.com/support/web-distribution-eu/)
and applicable terms, not a custom
downloaded-native-code loader. These full-package delivery profiles require
separate implementation and qualification.

## Execution profiles and reuse

### Browser and embedded web

The first executable OTA consumer is the existing web Grid on iOS. Coax
already produces Wasm and the app already embeds WebKit. Add release pinning,
compatibility negotiation and verified package selection around that existing
path; preserve the native fallback. Do not claim that today's mutable hosted
page already has offline signed-package verification.

For the open web, a pinned release root resolves every required Wasm, host
script, stylesheet and asset to its exact digest. A running page stays on
that release until an admitted navigation/reload. Service workers cannot mix
an old HTML shell with a new Wasm or leave a new cache half installed.

A browser initially trusts its HTTPS origin and bootstrap; a compromised
origin can replace that verifier. An embedded native verifier provides a
stronger root for downloaded packages. Treat ordinary HTTPS page loading and
verified offline packages as distinct launch sources, with distinct evidence.
The hosted shell reports its build separately from a signed portable payload.

### Portable app modules with native presentation

To update shared app actions while keeping UIKit or other native controls,
use the portable app execution profile. Modules receive bounded input,
state, time and service results and return semantic component updates and
effect requests. The installed host presents controls and executes admitted
services. Nothing requires React, Hermes, a new authoring language or a
JavaScript business-logic implementation.

The initial implementation compiles Coax through C/Wasm, Binaryen's
nontrapping/bulk-memory lowering and pedantic JavaScript emission, then
executes the generated artifact in Apple's JavaScriptCore SDK. The generated
app code, component composition and actions can change in a signed OTA
package while UIKit and native services remain installed. No WebView or DOM
is created. Both native and book milestone screens reuse
`apps/shared/native/NativeControls.swift`.

Each module has a 32 MiB JS linear buffer. The host admits scripts up to
8 MiB, at most 4096 retained nodes and bounded component frames. SDK/JSCore
allocations are separate from those buffers. The original Wasm module is the
independent parity reference (`native/tests/runtime-oracle.cjs`), not something
JavaScriptCore is assumed to instantiate. This profile does not provide
instruction fuel or qualify untrusted third-party programs: only verified,
first-party signed app artifacts are eligible.

For a future direct native Wasm executor, reuse and factor the existing Coax Wasm machinery in
[`metaverse-engine/script/wasm.coax`](../metaverse-engine/script/wasm.coax)
and [`interp.coax`](../metaverse-engine/script/interp.coax) before writing
an executor. The generic core belongs in a lower shared library if ordinary
apps need it; engine adapters retain realm behavior. Do not copy the engine
interpreter or make the ordinary app runtime depend on a world.

The current executor is a realm-script subset: 64 KiB modules, four 64 KiB
memory pages, fixed exports/imports, no indirect calls and instruction fuel.
Its reader does not perform full Wasm operand-type validation. It is not a
loader for today's complete app Wasm modules. A general app profile needs a
new versioned face, complete validation for its admitted instruction/type
subset, measured memory/fuel budgets and actual-host performance evidence.
Preserve the frozen realm profile and limits while extracting common code.

No native JIT, executable-memory entitlement or writable/executable mapping
is required by the interpreter profile. Browser WebAssembly execution uses
the browser's supported implementation; do not assume that embedding the
standalone JavaScriptCore framework supplies the same Wasm APIs as WebKit.
Native first-party interpretation on iOS remains a separately eligible and
qualified profile, not an automatic consequence of the browser path working.

### Native and server code

Native Coax compiled to C remains supported for the installed runtime and
hot paths. It changes through a native release, except for a direct desktop
profile with an independently qualified signed native updater. Do not add a
remote native loader to the mobile profile.

Server-side agent orchestration, model routing and tools can change through
the existing server release process while old clients use compatible APIs.
That expands how much product behavior can change without client updates;
it is a server deployment, not code executing locally on an offline phone.
Client payloads never install server authority or room rules on a client.

## What moves into the payload

| Area | Portable update candidate | Installed boundary |
| --- | --- | --- |
| UI | Component composition, copy, skins, supported layout and routes | Native control/backend implementation, accessibility/IME adapter and OS UI |
| App logic | State machines, validation, request orchestration and supported protocol adapters | ABI, service capability ceilings and native request/span validation |
| Agents | Local portable composition, prompts and descriptors; compatible hosted behavior | Credentials, execution grants and actual task ownership |
| Wallet | Screens, ordinary formatting, permitted request flow, stricter portable validation | Key custody, SDK, owner-confirmation binding and mandatory spend/admission floor |
| Realms | Compatible engine Wasm where supported, authored content and existing script profile | Frozen engine/room contract, durable authority and installed native renderer services |
| Graphics | Coax-emitted shader artifacts admitted by a supported render/reflection profile | GPU service, resource ownership, limits and driver; arbitrary compute is not a content fallback |
| Content | Images, audio, documents, fonts/localizations where the distribution profile permits | Decoders, storage quotas, accessibility and platform packaging requirements |
| ML | Compatible model/config assets under existing model admission | Execution kernels, memory limits, license and device support |

The maximum native-control version updates almost all changing composition
and ordinary app behavior, while retaining a small stable platform kernel.
The maximum web version can also replace the supported renderer/engine Wasm.
Neither can add a native SDK the binary does not contain. Shaders, rules,
models and declarative workflows are classified by their behavior; calling
them assets does not remove their execution or distribution requirements.

Payload authority is the intersection of its signed requirements, installed
capability ceiling, account grant, user consent and current service policy.
A signature establishes authorship, not permission to spend. A payload cannot
export seeds, replace trust roots, widen an agent grant or weaken the native
spend floor. Wallet approval binds destination, amount, fee and operation ID
to the effect the protected service actually executes. Existing secret and
payment authorities remain consumers, not copied framework logic.

All later module loads use the same verified resolver. A portable payload
cannot turn a network response, an unlisted dynamic import or an embedded
remote page into executable code with native service access. External web
navigation remains an explicitly separate surface with its own origin policy.

## Runtime identity and compatibility

Use four separate identities:

- **Native build:** OS package and immutable host executable identity.
- **Runtime fingerprint:** executable/service contract available to a payload.
- **Payload identity:** digest of the exact manifest body, which names artifact digests.
- **Deployment generation:** monotonic selection decision for a channel/profile.

The runtime fingerprint covers app/service ABI, execution profile and version,
supported imports and signatures, component command semantics, native service
versions, compiled capability/distribution ceiling, GPU/reflection and asset
formats, persistent-store envelope and embedded update trust configuration.
The build records the source/compiler/native dependency identities that
produced it. Generate this fingerprint; never reuse it manually after a native
dependency or permission change. Compiler provenance is recorded separately
from compatibility; byte-compatible compilers need not force a new runtime.

V1 requires exact runtime-fingerprint match, supported platform/OS/architecture
and eligible artifact classes. A later compatible-runtime map must itself be
versioned and admitted; a semver string, channel or matching state size is not
proof. The payload also states its supported API/room versions, data-read/write
versions and required service capabilities. Missing imports refuse before
execution. An old binary keeps its last compatible release and can show
“Update the app” without downloading an unusable payload.

App version, native build and payload version need not be the same. Web and
store releases can land at different times. Servers retain the advertised
client compatibility window, and deployment tooling reports affected old
runtimes before removing an API. A live room pins its engine/rules identity;
an update never changes those rules underneath an existing seat.

## Package and control records

V1 is a custom signed protocol with bounded records, not an unconstrained
archive. Before implementation, add schema files/specification, registry rows,
one owning codec for every stored/wire shape and frozen byte fixtures under
the root protocol rules. The following are proposed fields and semantics;
they allocate no existing protocol numbers or published IDs.

| Record | Required content |
| --- | --- |
| Runtime descriptor | Native build, runtime fingerprint, distribution profile, execution/service versions, capability ceilings, limits, embedded payload and trusted root keys |
| Payload manifest | Project/app identity, source/build provenance, target runtime/profile/platform, entry modules, required/optional artifacts, imports/capabilities, data/API versions, activation and health contract |
| Artifact descriptor | Logical name, role/execution class, SHA-256, exact encoded/decoded sizes, media/encoding profile and dependency digests |
| Signed deployment head | Project/app, environment/channel, runtime/platform/profile, monotonically increasing generation, security epoch, allowed control/candidate payloads, rollout assignment, expiry/freshness and explicit recovery directives |
| Trust delegation | Root/key epoch, delegated key identity and bounded scope, expiration/revocation and authorized roles |
| Local activation journal | Previous/current/trial/pending payloads, deployment/security floors, storage versions, attempts, host-observed health and operation ownership |

Sign exact, bounded manifest/control bytes with domain separation for record
kind, version and app/project identity. Verify those original bytes before
decoding; do not reserialize an object and verify that instead. Unknown
required fields/versions refuse, and unsigned metadata cannot influence
selection, capability, rollback or trust. Every artifact, including executable
modules and the launch entry, has a mandatory digest and size.
The payload identity is the digest of the exact manifest body; that body
does not contain its own digest. The signature envelope and deployment head
reference that identity, avoiding a recursive hash definition.
Reject duplicate logical names, ambiguous repeated scalar fields, conflicting
artifact roles/sizes and cyclic required dependency graphs. Optional/lazy
artifacts may remain absent, but they are verified and admitted before first
use; an unavailable optional feature cannot hold startup or publish partial
ready state. A module cannot relabel a required executable as optional to
skip validation.

Use SHA-256 and the existing
[`Ed25519`](lib/crypto/ed25519.coax) implementation for the initial update
verification profile. Publishing uses a separately qualified signing path
and pinned test vectors. These release keys are distinct from wallet,
account/device and agent keys. Native package signatures remain the OS's
separate requirement. A Nostr release announcement may reference a digest,
but cannot replace the update trust chain.

Keep offline root/recovery authority separate from scoped online release and
deployment keys. The embedded runtime pins accepted roots and algorithms.
Signed, monotonically versioned delegations permit rotation without a binary
release while the installed root remains trusted. A compromised root or
verifier requires a native update unless a preinstalled independent recovery
root can repair it. A CDN supplies bytes only; it cannot mint deployments.

## Service and publishing

The service runs on our infrastructure and reuses the existing API, object
storage, static delivery and web release mechanisms. Coax owns selection,
signing-input construction, compatibility, budgets and rollout policy;
host adapters perform transport, storage and signing-provider effects.
New admission/selection policy is `#authority` with checks first and zero
surviving mutants. Do not create another release queue or GitHub Actions.

Publication order is:

1. Build once from a source snapshot with pinned compiler, dependencies and
   assets. Classify every native change and portable artifact.
2. Verify against each supported runtime/profile, including a release-mode
   installed app, prior persistent data and the recovery cases below.
3. Upload immutable content-addressed artifacts, then the signed manifest.
   Confirm the complete required closure is retrievable and matches hashes.
4. Publish a signed deployment head last with compare-and-swap against its
   expected generation. Concurrent promotion cannot overwrite another release.
5. Promote the same payload bytes through internal, preview and production
   assignments. Environment-specific code/assets are a different payload that
   must be verified again; credentials are never bundled public configuration.

An update check sends app, platform/profile and runtime, channel, current
payload and the last accepted deployment generation, plus a pseudonymous
installation cohort if enabled. It sends no wallet key, account token or chat
content. Private previews use scoped credentials that are not reused for
artifact/CDN requests to arbitrary origins. Response is a signed deployment
decision or no change; transient errors retain the current runnable version.
URLs, redirects and destinations must pass the installed origin policy.

Channels are signed environment assignments; a production payload cannot
choose preview or redirect its updater to a new origin. A stable cohort
selects control/candidate under a signed rollout ID, salt and threshold.
Assignments must not oscillate across checks or grow through unsigned server
headers. A publisher may stage 1%, 5%, 25%, then 100%, with explicit bounded
sample/error criteria. Decreasing a percentage stops new offers; moving
already activated clients back requires a new signed recovery deployment.

Differential transport is optional after whole artifacts work. Patches name
exact base and target digests and sizes; admit scratch/output space before
application and verify the entire reconstructed target before publication.
An unavailable base uses the full artifact. Chunked transfers resume only
with validated extents and final identity; no partially fetched module runs.

## Download and local storage

Only one install transaction publishes a selection at a time. Reserve the
complete manifest, artifact, decompression, temporary and retained budgets
before allocation/copy. V1 mobile defaults proposed for qualification are:
256 KiB manifest, 1,024 artifact rows, 64 MiB encoded/128 MiB decoded required
closure, 8 MiB portable app module, 32 MiB module linear memory and 512 MiB
update cache. These are app-update limits, not changes to realm-script limits.
Larger realm/model packs use their existing separately admitted stores and
pinned dependency identities, not a way to smuggle unbounded launch work.

Limits are upper bounds, not a required reservation on every launch. Disk
headroom covers old+new payloads, reconstruction and metadata before fetching.
An app can choose smaller limits; raising the installed ceiling needs a runtime
release. GC retains the embedded payload, current healthy payload, rollback
target, trial/pending dependencies and actual in-flight borrowers.

Write by digest into staging, reject traversal/symlinks and mismatched role or
encoding, check whole decoded sizes, then atomically mark a complete payload
ready. Never execute directly from a partial archive. Cancellation, process
death, disk full and concurrent cleanup cannot damage the running package.
SDK work retains its concrete buffers until completion/join, even after an
install is cancelled. Cache cleanup has no access to user documents or secrets.

Mobile files stay in the app sandbox; direct desktop storage uses the shared
`~/.openagents/` helpers and an app namespace. Store update code as permitted
non-executable data for the installed executor; never set native execute bits
as a substitute for a permitted loader. OS cache eviction is expected: a
missing dependency makes that payload unavailable and preserves rescue.

## Client lifecycle and activation

```text
embedded or healthy -> check -> download -> verify -> staged
                                                    |
                                       safe activation boundary
                                                    v
                                              trial launch
                                             /            \
                                        healthy          failed
                                          |                |
                                       commit       quarantine + recovery
```

Checking, staging and activation are separate APIs. Default launch selects a
complete locally admitted version immediately, with zero network wait. Check
after the first usable frame and on foreground, with backoff and a bounded
minimum interval (proposed 15 minutes). This is an eligibility check, not a
promise of background execution. Push can hint that a check is useful but
cannot carry executable content or force an unsafe restart. Airplane mode and
service outages do not block startup.

Proposed typed app operations are check, download, cancel, inspect and request
activation. They are framework API concepts, not implemented CLI spellings.
Expose current/native/runtime/pending identities, launch source, progress,
last error, emergency reason and a bounded log through shared app state and
`coax drive`. Production has no API to turn off verification or erase the
embedded fallback. A normal payload cannot replace the updater or certify
its own launch as healthy.

V1 activation restarts the portable app session at a safe route boundary or
next cold launch. No arbitrary in-place procedure patching. The old session
quiesces input, joins/cancels owned work and transfers a checked checkpoint.
The new session receives defaults and only admitted persisted state. Raw
memory, views, pointers, OS handles and callbacks never cross by byte copying.
Development field carry is not a production data migration.

For the planned [OpenAgents desktop/environment consumer](../docs/protocol/environments-claude-and-world-editing.md#34-replacement-storage-and-updates),
the client and resident execution service have separate lifetimes and version
negotiation. Activating UI code detaches/reattaches views; it does not cancel
an admitted Claude turn or transfer process ownership to the new UI. A UI
payload cannot widen the service's permission ceiling. Native service updates
use their qualified platform package path, with explicit busy-work disposition,
drain/restart and reconciliation, retaining old binaries/resources while in
use. Client launch or a newer UI alone never forces a service restart.

An in-flight payment, biometric/passkey prompt, file transaction, voice turn
or room handoff defers activation. Streaming work can finish, or be cancelled
with its outcome retained; it is never silently reissued. Logout and account
replacement fence the old owner first. Close the old surface/audio/GPU owners
before installing replacements, with exact effect-generation checks.

An unknown external outcome is checkpointed as unknown once its concrete SDK
work has joined; it need not defer every future update indefinitely. Recovery
must query/reconcile that operation rather than resend it. Revoking a running
payload can fence its code immediately while the protected native service
retains an already executing operation through its final completion.

The installed supervisor journals a trial intent before running candidate
code. Host-observed ready state requires constructed app state, a valid first
interactive presentation, admitted required services and completed storage
opening/migration. Qualification initially requires a 30-second active-
foreground health window; suspension pauses it. A trial that cannot reach
ready within a bounded active-time deadline fails. Health does not require
network success, biometrics, a payment or microphone access.

Use a persistent launch nonce, ready marker and bounded attempt budget
(proposed two failed trials per payload/runtime). The supervisor distinguishes
explicit failures, traps/watchdog timeouts, unclean termination and normal
backgrounding/user exit where the OS supplies that evidence. An ambiguous
kill is not proof that code crashed; it cannot cause an infinite trial loop.
Retain reasons and quarantine after the bounded attempts. Post-ready crashes
still inform rollout health; first render alone is not complete correctness.

## Recovery and durable data

Separate monotonically accepted **deployment/security floors** from the
payload selected for execution. A rollback is a newer signed deployment
generation naming an older admitted payload, or a preauthorized local trial
fallback. It never lowers the metadata floor or accepts a replayed old head.
Security epochs can revoke older payloads; a recovery directive cannot revive
one beneath that floor. Signed directives and revocations are scoped to app,
profile, runtime and channel; channel switching cannot reset the security floor.

Keep the previous healthy payload and immutable embedded payload. Local
trial recovery may select them only if their security/data contracts still
permit it. If none can open current storage, launch the installed recovery
screen and fetch a fix-forward or request a native app update. Never delete
user data, replay a send or lower a security floor to manufacture a successful
launch. Bad-payload identity is quarantined; a new deployment does not clear
it merely by offering the same bytes again.
The bootstrap and recovery screen execute outside downloaded code and retain
their own transport/diagnostic path. Payload failure cannot disable checking
for a compatible fix. If the update store or journal itself is corrupt,
quarantine that store and enter recovery without clearing account/user stores.

Metadata expiration and the device wall clock bound acceptance of new offers,
not the ability to start an already installed healthy app offline. Persist
the strongest known sequence/time facts without trusting a clock rollback.
A stale server/CDN cannot force a downgrade; an offline device cannot learn a
new revocation. Storage erase/reinstall and a compromised OS remain limits
on local anti-replay guarantees and must be stated in evidence.

Code rollback cannot undo an external effect or an incompatible database
migration. Payloads declare minimum read/write versions and supported
rollback targets. Prefer additive records and expand/contract migrations
whose writes old recovery code understands. Test against actual prior data.
An irreversible migration requires a native recovery-compatible runtime or
an explicit forward-only plan with a recovery UI; do not auto-activate it
while advertising rollback to embedded. A schema checksum is not a migration.
Wallet seeds, backup formats, signed records, task operation IDs and room
checkpoints keep their existing authority and compatibility rules.

Root/key rotation, crash quarantine and stored selection changes are committed
atomically with their journal records. A power cut after each write must leave
one recoverable interpretation. Rescue snapshots in the engine remain exact-
build snapshots; they are not compatible checkpoints for a different OTA
payload. The existing browser rescue fixture does not qualify mobile recovery.

## CLI and operations

Extend the existing Coax build/drive/release tooling with a proposed
`coax native update` command family. All commands below are proposals:

| Command concept | Outcome |
| --- | --- |
| `plan` | Compare against installed runtimes; classify OTA/native changes and explain missing services or eligibility |
| `build` and `verify` | Produce immutable artifacts and run bounded app/host/recovery evidence before signing |
| `publish` | Upload the verified closure and signed manifest, without promoting production |
| `promote` | CAS a signed deployment assignment to an existing payload and rollout |
| `rollback` | Publish a newer admitted recovery decision, preserving security/data floors |
| `status` | Show runtime coverage, current/control/candidate payloads, rollout and observed health |

Runtime planning should say precisely why a change needs a binary: a new
import, SDK, entitlement, module limit, component command, native policy
floor or distribution eligibility. Do not collapse those into “not supported.”
An eligible portable-only change should not rebuild native packages.

Record native build, runtime, payload and deployment identities in crash
reports and release health. Count successful launches, trial failures,
activation deferrals, recoveries and storage errors separately, with bounded
redacted logs. Do not add device fingerprinting or content collection to
choose rollout cohorts. Signed authority is required to promote or revoke;
agent tools call the same policy and cannot bypass publication checks.

## Implementation and qualification

Reuse `coax/web/release.sh`, the shared hosts, protocol codecs, crypto,
resource/transport admission, state-layout work and the engine's script and
rescue modules. [`os/coax-update-policy.coax`](../os/coax-update-policy.coax)
already has trial/switch/rollback/prune rules for OS programs. Extract generic
policy only when semantics match; a trusted source-build updater is not an
untrusted package validator or a mobile activation supervisor.

| Stage | Consumer and bounded exit |
| --- | --- |
| U1. Artifact boundary | Build planner classifies a UI/action-only Wasm change and a new native SDK change correctly; exact runtime fingerprint and signed record schemas/codecs exist |
| U2. Signed offline launch | iOS embedded-web consumer checks/downloads/stages one complete package; airplane-mode relaunch, corrupt artifact and disk-full preserve the prior package |
| U3. Trial and recovery | Installed supervisor survives power cuts, pre/post-ready failures, stale heads, revoked payloads and incompatible storage without a loop or erased user data |
| U4. Native-control execution | Factor the existing interpreter, implement the separate app validation/import profile, and update one native form/action without a binary rebuild; measure actual phone CPU/memory/input behavior |
| U5. Delivery control | Signed channels, deterministic cohorts, concurrent promotion refusal, key rotation and explicit recovery deployments pass on the self-hosted service |
| U6. Android and desktop | Same portable acceptance operations and package cases pass on each qualified distribution profile; native installers remain distinct |
| U7. Larger updates | Measured shader/realm/model profiles and verified differential downloads, only after whole-package recovery and compatibility pass |

Before executable OTA is offered on a store profile, qualify its distribution
eligibility and actual device/loader path. U4 is not required to obtain early
web/Wasm OTA benefits, and its unqualified state cannot be disguised as a
completed general app VM. The product/book rewrite remains #13512.

Write checks first and mutation-test each new authority. Acceptance must
include forged/cross-app signatures, altered assets, wrong runtime/profile,
unknown imports, missing required dependencies, decompression/size refusal,
cancelled old-owner completion, concurrent installs, cache eviction, replayed
heads, forward revocation, delegation rotation, cohort stability, offline
launch, data migration and activation during a pending payment. Compare the
same app action/result trace before and after update on web and native.

For the app executor, add differential instruction/state tests against an
independent Wasm engine plus hostile-module/fuel/stack/type cases. Keep the
existing realm-script qualification unchanged. For startup recovery, use
actual release-mode binaries and physical-device launches; a debugger/dev
server can mask the behavior being qualified. Record source/compiler/runtime/
payload/service identities and remaining device-only work in existing evidence
formats. Owner review/device steps go in `NEEDS_OWNER.md` when reached.

The feature is complete for a named profile when an installed offline-capable
app can accept a signed compatible code update, run the changed behavior,
refuse an incompatible/native change, survive a broken update, and retain
its account, wallet and user data through recovery. Browser delivery, mobile
interpretation and direct native desktop updates earn that claim separately.

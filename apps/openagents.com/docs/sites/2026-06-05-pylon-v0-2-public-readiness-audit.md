# Pylon v0.2 Public Readiness Audit

Date: 2026-06-05

Issue: `OpenAgentsInc/openagents#161`

Scope: current Pylon v0.2 public setup readiness for agent-facing local compute
instructions in OpenAgents product surface.

## Verdict

Pylon v0.2 is **partially ready, but blocked for broad public claims**.

The source-level LDK-compatible payout target model exists. Current
`openagents` source supports `pylon-payment-target/v0.2`, signs provider
payment-target registrations, rejects Spark as a new paid-work target, and
models the required target kinds:

- `bolt12_offer`
- `bolt11_invoice`
- `bip353_name`
- `lnurl_pay`

The public release story does not match that source readiness yet:

- the active `openagents` workspace package version is `0.1.23`;
- the latest GitHub Pylon release is `pylon-v0.1.23`, published
  2026-05-15;
- there is no public `pylon-v0.2.0` release visible in GitHub Releases;
- the latest `@openagentsinc/pylon` launcher source is `0.1.17`;
- current public release assets observed for `pylon-v0.1.17` through
  `pylon-v0.1.23` are Darwin ARM64 only; and
- Linux, WSL Ubuntu, and native Windows users must either resolve an older
  binary asset where one exists or fall back to source build.

Therefore OpenAgents product surface may instruct agents and users to use the current Pylon launcher
for controlled local-compute setup, but must not say that Pylon v0.2 is broadly
public-ready until the release, asset, and smoke-proof gates below are closed.

## Sources Reviewed

Local source:

- `openagents/AGENTS.md`
- `openagents/Cargo.toml`
- `openagents/packages/pylon-bootstrap/package.json`
- `openagents/packages/pylon-bootstrap/README.md`
- `openagents/packages/pylon-bootstrap/src/index.js`
- `openagents/crates/openagents-provider-substrate/src/payout_target.rs`
- `openagents/apps/pylon/src/lib.rs`
- `openagents/apps/pylon/src/wallet_runtime.rs`
- `openagents/apps/nexus-control/src/treasury.rs`
- `openagents/docs/MVP.md`
- `openagents/docs/v02.md`
- `openagents/docs/nexus-treasury.md`
- `openagents/docs/pylon/PYLON_PLAN.md`
- `openagents/docs/pylon/LDK_ACCEPTED_WORK_PAYOUT_PROOF.md`
- `openagents/docs/2026-04-21-run-pylon-get-paid-for-training.md`
- `openagents/docs/audits/2026-04-27-pylon-windows-build-and-binary-audit.md`
- `openagents/2026-05-22-pylon-v0.2-wallet-security-release-readiness-review.md`
- `psionic/V0.2_PYLON_RELEASE_AUDIT.md`

Live release metadata checked through GitHub CLI:

- `gh -R OpenAgentsInc/openagents release list --limit 30`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.1.23`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.1.17`
- `gh -R OpenAgentsInc/openagents release view pylon-v0.1.16`

## Current Version And Release Reality

Current source and package facts:

| Surface | Current observed value | Readiness meaning |
| --- | --- | --- |
| `openagents/Cargo.toml` workspace version | `0.1.23` | Source has v0.2 contracts, but the workspace is not versioned as v0.2. |
| `packages/pylon-bootstrap/package.json` | `0.1.17` | Public launcher floor is still a 0.1-line package. |
| Latest GitHub release | `pylon-v0.1.23` | No `pylon-v0.2.0` release exists. |
| Latest release asset matrix | Darwin ARM64 archive plus checksum | Public binary install is Mac-Apple-Silicon-first only. |

Current releases inspected:

| Tag | Published | Assets observed |
| --- | --- | --- |
| `pylon-v0.1.23` | 2026-05-15 | `pylon-v0.1.23-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.22` | 2026-05-13 | `pylon-v0.1.22-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.21` | 2026-05-13 | `pylon-v0.1.21-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.20` | 2026-05-13 | `pylon-v0.1.20-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.19` | 2026-05-13 | `pylon-v0.1.19-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.18` | 2026-05-13 | `pylon-v0.1.18-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.17` | 2026-05-12 | `pylon-v0.1.17-darwin-arm64.tar.gz`, checksum |
| `pylon-v0.1.16` | 2026-04-27 | `pylon-v0.1.16-darwin-arm64.tar.gz`, checksum |

The 2026-04-27 Windows audit said Linux assets existed for older releases, but
the current recommended release line visible now is Darwin-only. That remains a
public setup blocker for WSL Ubuntu and Linux users.

## LDK-Compatible Target Contract

The target contract is implemented in source.

`openagents/crates/openagents-provider-substrate/src/payout_target.rs` defines:

- `PYLON_PAYMENT_TARGET_VERSION_V0_2 = "pylon-payment-target/v0.2"`
- `LDK_PAYMENT_TARGET_CAPABILITY_V0_2 = "ldk_payment_target_v0_2"`
- the payment-target signing domain
  `openagents:nexus-treasury-payment-target:v2`
- inference for:
  - `bolt12_offer` from `lno...`
  - `bolt11_invoice` from `lnbc...`, `lntb...`, `lnbcrt...`, and other
    Lightning invoice prefixes
  - `bip353_name` from names containing `@`
  - `lnurl_pay` from `lnurl...`, `lnurlp:...`, `https://...`, or
    `http://...`
- explicit Spark rejection through `unsupported_payment_target_kind:spark`

`openagents/apps/pylon/src/lib.rs` builds provider wallet registration targets
from:

- an explicit `external_payout_target` override; or
- a wallet-owned BOLT12 offer; or
- a wallet-owned BOLT11 fallback invoice if BOLT12 offer creation is
  unavailable.

The registration payload includes:

- `payment_target_kind`
- `payment_target`
- `payment_target_capabilities`
- `pylon_payment_target_version`
- `wallet_node_id`
- `wallet_runtime_kind`
- `wallet_network`
- `wallet_target_kind`
- `wallet_derivation_version`
- `wallet_backup_status`
- `wallet_registration_mode`
- challenge and signature material

`openagents/docs/nexus-treasury.md` and `openagents/docs/pylon/PYLON_PLAN.md`
state the same contract: upgraded paid workers need an LDK-compatible target,
old Spark-only targets are retained only for audit, and Nexus should mark
workers without v0.2 targets ineligible for new paid work with
`payout_target_requires_ldk_v0_2`.

## Wallet And Runtime Readiness

Source-level wallet readiness is strong enough for controlled use:

- Pylon derives an LDK wallet path and exposes wallet status, balance, channel,
  backup, invoice, offer, send, withdrawal, and history surfaces.
- Local proof docs retain a regtest accepted-work payout harness:
  `scripts/pylon/ldk-accepted-work-payout-harness.sh`.
- The May 22 security review recorded passing Pylon and Pylon TUI tests plus a
  regtest accepted-work payout harness.
- The same review recorded that Nexus had an LDK payer rail with ready channel
  posture and no observed failed/skipped/degraded payout state during that
  audit window.

This does not make broad public v0.2 onboarding ready. The public instructions
need a current release asset matrix and current production proof, not only
source and prior audit evidence.

## State Model For Public Claims

OpenAgents product surface and OpenAgents must keep these states separate.

| State | Meaning | Public wording allowed |
| --- | --- | --- |
| Online | A Pylon process is reachable or recently heartbeating. | "Online Pylon" or "connected Pylon." |
| Eligible | Nexus can assign the Pylon a class of work because version, capability, payout target, and policy gates pass. | "Eligible for this work class." |
| Assigned | Nexus assigned a specific work item or lease. | "Assigned work item/run." |
| Accepted | The verifier or authority accepted the completed work outcome. | "Accepted work" only with accepted outcome ref. |
| Paid | A payout was dispatched or confirmed against accepted work. | "Paid" only with payment receipt/status. |
| Settled | Treasury/Nexus/Pylon settlement evidence proves terminal settlement. | "Settled" only with settlement receipt or equivalent final proof. |

Disallowed conflations:

- Online does not mean eligible.
- Eligible does not mean assigned.
- Assigned does not mean accepted.
- Accepted does not mean paid.
- Paid does not mean settled unless the receipt explicitly says settlement is
  terminal.

## Platform Posture

### macOS

Apple Silicon macOS is the only currently well-supported public binary path
observed in current releases. Public instructions can say the launcher should
resolve current Darwin ARM64 assets when running on an Apple Silicon Mac.

### Linux

Linux source-build fallback exists through the npm launcher. However, current
release assets inspected for `pylon-v0.1.17` through `pylon-v0.1.23` do not
include current Linux archives.

Public instructions should say Linux may require a source build unless and
until current Linux assets exist for the release being recommended.

Required release assets:

- `pylon-v<version>-linux-x86_64.tar.gz`
- `pylon-v<version>-linux-x86_64.tar.gz.sha256`
- `pylon-v<version>-linux-arm64.tar.gz`
- `pylon-v<version>-linux-arm64.tar.gz.sha256`

### Windows And WSL

The public posture should remain:

- Prefer WSL Ubuntu for Windows users.
- Do not steer normal users to native PowerShell or `cmd` unless they
  explicitly want the native Windows experimental lane.
- In WSL Ubuntu, the launcher sees a Linux platform and should use Linux
  assets when current assets exist.
- Without current Linux assets, WSL users may hit the source-build path.

Native Windows readiness improved after the 2026-04-27 audit:

- `packages/pylon-bootstrap/src/index.js` now maps `win32` to `windows`.
- The README describes `pylon-v<version>-windows-x86_64.zip`.
- Tests cover Windows archive naming and `.exe` install paths.

Native Windows is still not public-ready because no current
`pylon-v<version>-windows-x86_64.zip` asset was observed, and this audit did not
find a retained native Windows smoke proving `pylon.exe`, `pylon-tui.exe`, the
npm bootstrap, and the packaged Psionic runtime on a Windows host.

## Allowed Public Claims Today

Allowed:

- "Pylon has source-level support for the v0.2 LDK-compatible payout target
  contract."
- "The current launcher can bootstrap Pylon and falls back to source build when
  no matching asset exists."
- "Apple Silicon macOS has current public binary assets."
- "Windows users should prefer WSL Ubuntu today."
- "Eligible paid work requires an LDK-compatible payout target such as
  BOLT12, BOLT11, BIP353, or LNURL-pay."
- "Accepted-work payment claims require accepted outcome and payment receipts."
- "Settlement claims require settlement evidence."

## Disallowed Public Claims Today

Disallowed:

- "Pylon v0.2 is publicly released."
- "Pylon v0.2 is ready for all users."
- "Windows native Pylon is supported for normal users."
- "Linux/WSL users have current prebuilt binaries."
- "Running Pylon means you will earn money."
- "Online Pylons are eligible for paid work."
- "Assigned work will be accepted."
- "Accepted work is already paid."
- "Paid work is settled" without a settlement receipt.
- "Pylon can support broad Site-agent local compute jobs" before the Pylon
  setup packet and agent Site action policies are finished.

## Required Gates Before Public v0.2 Claim

1. Tag and publish a `pylon-v0.2.0` or equivalent release, or explicitly decide
   that the LDK-target release remains on the `0.1.x` semver line and document
   why.
2. Publish current binary assets and checksums for:
   - Darwin ARM64
   - Linux x86_64
   - Linux ARM64
   - native Windows x86_64, if native Windows is to be advertised
3. Retain launcher smokes for each platform path:
   - asset resolution
   - checksum verification
   - archive extraction
   - `pylon --help`
   - `pylon status --json`
   - `pylon wallet status --json`
   - TUI launch or `--no-launch` smoke
4. Retain a WSL Ubuntu smoke with the current Linux x86_64 asset.
5. Retain a native Windows smoke before advertising native Windows.
6. Run and retain the LDK accepted-work payout harness for the release commit.
7. Confirm Nexus production status after release:
   - LDK rail active
   - wallet runtime connected
   - no failed/skipped/degraded payout state
   - v0.2 target registration counters present
8. Publish an operator-facing state explanation that separates online,
   eligible, assigned, accepted, paid, and settled states.
9. Update OpenAgents product surface `https://openagents.com/AGENTS.md` only after these gates are current,
   or keep the current language as controlled/gated setup.

## Roadmap Impact

The next issue, `OPENAGENTS-PYLON-002`, should not write broad public setup
instructions. It should write a guarded instruction packet:

- current recommended path: `npx @openagentsinc/pylon`
- current version floor: launcher `0.1.17+`; Pylon release `0.1.23` observed
  latest, with previous paid-training floor `0.1.16+`
- Mac Apple Silicon: binary path expected
- Linux/WSL: source-build fallback likely until current Linux assets exist
- Windows: WSL Ubuntu first; native Windows experimental until a retained
  native smoke exists
- earnings: possible only when online, eligible, assigned, accepted, paid, and
  settled gates are satisfied in order
- payment target: LDK-compatible target required for paid work

Do not delay customer Sites fulfillment on full Pylon v0.2 release work. Treat
Pylon setup as an optional local-compute expansion path for agents until the
release gates above are complete.

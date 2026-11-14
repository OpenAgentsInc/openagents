# Claude accounts and Usage on iOS

Signed OTA **generation 3** targets TestFlight **1082**, runtime
`24aac0f832944da03a0f511fb3e476251baa262fa43a743a00a52f3e90a18cc0`.
The implementation landed in `c372765ac1` and `fee48af23e`; the asynchronous wake correction is
`9ccd52f420`. The package changes
only `build/app.js` and `build/app.wasm`; the eighteen other resources, including
worlds, retain their 1082 hashes. The public envelope and all twenty object
lengths/SHA-256 digests were verified after publication. Feed SHA-256:
`6f8060fc8c6afb2e4dbccfe78d8631a06b3804f9449caa7cb4e4179d6229ae62`.
Private artifacts and machine-readable observations are retained at
`~/.openagents/releases/openagents-ios-1082-usage/`.

**Chats → Settings → Usage** lists each enrolled computer's Claude profiles,
reported weekly/five-hour windows, remaining percentages, resets and stale or
unavailable observations. Connect uses the official computer-local CLI in an
isolated profile. Sign-in has a native sheet; the phone opens the CLI's Claude
URL and returns its pasted authorization code. Explicit selection applies to
new managed chats; an existing CLI keeps its profile. The contract and bounds
are [provider-accounts-v1.md](../protocol/provider-accounts-v1.md).

The Mac and CoderOS resident services were upgraded and the existing exact
broker grant opted into account operations on each. Both were idle before the
upgrade. Real encrypted broker reads returned Claude Max metadata and live
weekly windows on both. No inference call was needed to measure usage. A
no-browser probe of installed Claude Code 2.1.325 produced its actual manual
sign-in URL; the sign-in was then cancelled without completing authentication.
The shared no-UI Keychain effect selects the effective OS user, including
launchd's sparse environment, and never treats Keychain denial as absence.

The API is deployed as **oa-api-00161-kkt**, image
`sha256:bf444cbabd09b6829719698d853b0b64936af6c45a97ab59d631461a8bbc6e58`.
Staging and production migration/health/build identities passed the deployment
verifier. Diagnostic jobs using that exact image and production service
account returned typed account observations for both computers, without
refusal. Initial observations correctly report asynchronous checking; later
observations include quota windows. The diagnostic job and its request-only
secret were removed; the production broker secret remains. The public
account route rejects unauthenticated requests with 401.

Verification:

- 530 app checks passed, plus the added asynchronous wake regression. The native timer drives polling even with frame dt=0;
  an explicit account action queues behind a quota read.
- 854 host checks passed; the subsequently added account snapshot check passed
  separately. The shared profile environment survives model/resume choices.
- Account admission policy: **65 mutants killed, zero survivors**.
- Targeted API run: **41 account/history checks**, all passed, including
  email login, account-owned connection lookup, typed child carriage,
  no-store responses and refusal of malformed/token-injection requests.
  The complete targeted harness, independent process checks and SQL checks
  finished successfully in 515 seconds.
- Native Usage UI: **1/1 passed**, covering real UIKit fields, a second fixture
  account's code submission and selection. Generation 3 additionally passed
  **2/2** delayed-quota UI/interpreter checks: the first observations are
  checking, and actual percentages must arrive without another tap.
  Native interpreter/navigation
  regression: **6/6 passed**. The tested app.js bytes match the release payload.
- Legacy Coder usage checks: **16/16 passed** after moving Claude quota and
  credential parsing to the shared codec.
- Tools, components, wire, layers, state retention, agent reference and shader
  gates passed. Protobuf compilation of the new contract passed.

Actual provider consent for an additional real account and its first physical
phone chat remain owner steps in [NEEDS_OWNER.md](../../NEEDS_OWNER.md). The
native UI oracle uses fixture accounts and does not claim that a second real
account has already been connected. Codex account management is not implemented.
The remaining terminal probe-effect consolidation is
[#13598](https://github.com/OpenAgentsInc/openagents/issues/13598).

Generation 2 was initially published, then generation 3 corrected a missing
future wake after the last asynchronous observation. Each retained its own
signed identity; no published generation was rewritten. Its exact prior bytes
are kept in the private release directory's `generation-2/` folder. The final
asynchronous native test's app.js and app.wasm match generation 3's bytes.

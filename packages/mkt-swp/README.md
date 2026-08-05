# `@openagentsinc/mkt-swp`

SWAP-0 (openagents#9315): the swap widget's typed state, the engine
boundary, and the exported session view-model. Plan:
`docs/markets/2026-08-04-swap-demo-ui-rollout-plan.md` §2.2, §4.1.

This package owns three things and deliberately no more. Every refusal it
renders was computed by a sibling package; every profile-level verdict was
computed by the engine.

## 1. The engine boundary — `swap-engine.ts`, `engine-binding.ts`

MKT-SWP profile logic exists once, in the Immortal client crate
(immortal#12): script and tree parsing, output-key re-derivation, invoice
checks, MuSig2 transcript checks, timeout ladders, exit packages, and the
typestate fund-authorisation flow. It builds for `wasm32` and takes key
material as bytes with no randomness of its own. `swap-engine.ts` is the
Effect Schema contract that binding satisfies.

SWAP-3 (#9318) had already declared the half of the boundary its checklist
consumes — `FundVerifier`, `VerifyBeforeFundReport`, `VERIFY_CHECK_IDS` — so
`SwapEngine.Interface extends FundVerifier` and the compare surface consumes
this engine with no adapter. This module adds the rest of what can authorise
funding: `validateProfileRecord`, `openSession`, `buildExitPackage`,
`authorizeFunding`, `constructFundingTransaction`.

**Loading**: the wasm module is never statically imported. `engine-binding.ts`
takes an `EngineModuleLoader` port (`loaderLayer(async () => …)` wraps a
wasm-bindgen entry point) and `engineLayer` turns it into the service. Load
failure is a typed `EngineLoadFailed`, so `EngineLoading` and `EngineFailed`
are real lifecycle states and a failed load can never read as a pass. There
is no fallback engine — a fallback would be a second implementation of the
thing that authorises funding. `fixtureEngineLayer` satisfies the same tag
for dev/staging and every test.

**Entropy**: `entropy-source.ts`. The engine has none; the host supplies
WebCrypto bytes.

## 2. The exported session view-model — `view-model.ts`

The one contract both renderers consume: the web components here and Omega's
`market_ui` GPUI components. It is a composition — SWAP-6's
`SwapProgressView` for lanes/gaps/forks/rungs, SWAP-3's `FundingGate`, the
engine's exit package and funding authorization — plus what no sibling can
own: the session identity, the widget state, and the resolved primary action.
Nothing another package owns is redefined.

## 3. The typed state and the primary-action law

`widget-state.ts` is the discriminated state union with a total transition
fold. `compose.ts` is the derivation precedence over the siblings' verdicts —
the ordering _is_ the widget's behaviour, because the law says exactly one
refusal, the most proximate, is stated at a time.

`AwaitingFunding` is constructible only with an engine-issued
`FundingAuthorization`, so the fund action cannot be enabled while any
verify-before-fund check is unresolved or failed
(`swp_funding_not_authorized`). SWAP-3's `fundingGate` is the UI pre-check
that can only keep funding disabled; this typestate is the other half.

`primary-action.ts` computes label, tone, disabled, and busy independently.
Labels resolve through SWAP-8's catalog (`@openagentsinc/swap-i18n`) — amount
refusals through the parameterised `swap.refusal.*` keys so the limit is
stated in the user's current units, typed §17 identifiers through the shared
error table. No counterparty prose reaches a label.

## What lives elsewhere

| Concern                                               | Owner                                           |
| ----------------------------------------------------- | ----------------------------------------------- |
| §17 identifiers, message catalog, locales             | `@openagentsinc/swap-i18n` (SWAP-8)             |
| Asset/direction selection, amounts, limits, fees      | `@openagentsinc/mkt-swp-pair` (SWAP-1)          |
| Destination parse, verification, QR                   | `@openagentsinc/mkt-swp-destination` (SWAP-2)   |
| Quote compare, custody, `fundingGate`, `FundVerifier` | `@openagentsinc/mkt-swp-compare` (SWAP-3)       |
| Session store, history, resume, export/import         | `@openagentsinc/mkt-swp-session-store` (SWAP-5) |
| Per-signer status, §9 `classifySwpState`, rungs       | `@openagentsinc/mkt-swp-status` (SWAP-6)        |
| Routes, nav, gate, provenance, settings               | `apps/openagents.com/apps/start` (SWAP-7)       |

The mounted markup is
`apps/openagents.com/apps/start/src/features/swap/widget.tsx`; this package
stays framework-neutral so Omega renders the same contract.

No key material, no relay transport, and no mainnet path live here. Keys and
preimages arrive with SWAP-4 and stay in the browser.

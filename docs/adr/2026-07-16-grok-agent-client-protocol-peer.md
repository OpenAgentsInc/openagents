# Grok CLI as an admitted Agent Client Protocol peer

Status: accepted for implementation. Release support remains gated by #8897.

## Decision

OpenAgents controls Grok CLI through the Zed Agent Client Protocol command
`grok agent stdio`. This is not Agent Communication Protocol and is unrelated
to A2A.

The production Grok facade now composes the shared admitted stdio transport,
session runtime, canonical projector, authority brokers, and versioned Grok
extension handlers. The old raw JSON-RPC client remains only as an injected
fixture compatibility seam. Caller-provided command arrays are refused by the
production path. The RL/claimed-work terminal executor is intentionally
unchanged because its experiment contract is separate from ACP chat control.

## Admission and authentication

- The trusted profile launches exactly `grok agent stdio`, probes `grok
version`, resolves the real executable, and pins its SHA-256 at spawn.
- The initial release candidate is Grok `0.2.101`. It becomes `supported` only
  when admission receives fresh fixture and digest-bound live evidence.
- The child environment is reduced to `HOME` for cached local login and an
  intentionally supplied `XAI_API_KEY`. Values never enter receipts.
- Authentication is selected only from advertised methods. An intentional API
  key uses `xai.api_key`, cached login uses `cached_token`, and a peer that has
  neither may advertise interactive `grok.com` or enterprise `oidc`. Interactive
  authentication is fail-closed behind a typed owner decision and is never
  invoked after cancellation. Every selected method sends `_meta: { headless:
true }`. No OAuth-referrer metadata is enabled.
- Cached local login is the supported headless default. `xai.api_key` is
  optional and its absence never blocks ACP startup or a support claim.
- Desktop does not reinterpret an ambient shell `XAI_API_KEY` as an intentional
  provider setting. Its default runtime passes `HOME` only and selects the
  advertised cached-token method.
- A typed `requestedInteractiveAuthMethod` may select advertised `grok.com` or
  `oidc` ahead of cached authentication. This supports intentional re-login and
  proves cancellation without deleting or expiring an existing cached token.

## Capability truth

Filesystem, terminal, and Grok question extensions are advertised only when a
supported admission grant and an installed broker/handler are both present.
Experimental admission keeps them false. Session load follows the peer's
negotiated `loadSession` capability and uses the shared replay/live gate.

Stable advertised mode/configuration methods are preferred. Unstable
`session/set_model` and private prompt-completion fallbacks remain disabled
until an exact-version live matrix proves either is necessary.

## Evidence

The hermetic profile tests cover cached-token and API-key selection,
`grok.com`/`oidc` owner continuation and cancellation, truthful capabilities,
update streaming, stop reason, extension gating, and cleanup.
The candidate live record for Grok `0.2.101` is
`packages/agent-client-protocol-conformance/compatibility/live/grok-0.2.101-darwin-arm64.json`.
It contains no prompt/response text, auth material, provider metadata, or host
identity and is diagnostic evidence, not the final cross-platform release
claim. The separately checked two-peer candidate run exercises the production
composer through authentication, sequential prompts, and stream cancellation.
its closed artifact is validated by the release check but cannot promote Grok.

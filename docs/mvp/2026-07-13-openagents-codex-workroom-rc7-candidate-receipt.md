# OpenAgents Codex Workroom RC7 candidate receipt

- Date: 2026-07-13
- Tracking issue: [#8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
- Source commit: `94320bf4f73e21850260224d4bc1f1b67315b7e1`
- Candidate version: `0.1.0-rc.7`
- ProductSpec: `docs/mvp/openagents-codex-workroom-mvp.product-spec.md`
- Result: signed/notarized candidate and source-launched real-Codex journey passed; installed-candidate journey remains unproven

This receipt covers the corrected MVP boundary: OpenAgents Desktop used the
user's ordinary logged-in Codex session. It did not select, link, rotate, or
fall back to a named Pylon account.

## Exact artifacts

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.7-arm64.dmg` | `304467697` | `a2ae6bce77883c79d8d8c5f3a48450f3a7502b123d910275971749b0fc4df60f` | `d1e8c5b3-af4f-4dab-8332-7bdead225155` (`Accepted`) |
| `OpenAgents-darwin-arm64-0.1.0-rc.7.zip` | `308689802` | `278bfdea0d89fbac14c704859a5d87d0e5d211fd831872d871091ff0881b8825` | App submission `74aa2996-23f5-4786-8948-f9ff1ffae1b0` (`Accepted`) |

The DMG digest is post-staple. The contained app reports bundle ID
`com.openagents.desktop`, team ID `HQWSG26L43`, Developer ID authority
`OpenAgents, Inc.`, hardened runtime, and version `0.1.0-rc.7`. Deep strict
`codesign` verification and `stapler validate` passed for the app. The final
DMG was separately accepted, stapled, and validated. The packaged
`Contents/Resources/electron.icns` exactly matched the product-owned
`resources/openagents-icon.icns` bytes.

## Real-Codex proof and installed-driver falsifier

The source-checkout driver completed the real journey below using the user's
ordinary logged-in Codex session. It proves the current implementation spine,
but is not an installed-candidate receipt.

The driver recorded all required journey steps as passing:

1. Effect Native shell mounted.
2. Ordinary logged-in Codex session passed host preflight.
3. A validator-clean ProductSpec opened through the workroom.
4. The user-visible plan admitted two durable packets.
5. The root Codex packet ran, produced agent-authored evidence, and passed
   independent host verification.
6. The delegated Codex child packet ran and appeared as one causal child card.
7. The child card opened the child's independent transcript.
8. The child produced evidence and passed independent host verification.
9. Both verified packets remained behind an explicit pending owner gate; the
   driver did not fabricate owner acceptance.

The source-launched proof reported `summary OK
{"requiredSteps":10,"ownerAcceptanceFabricated":false}`.

The accepted DMG was then mounted read-only and its app was copied to the
reversible proof install `/Applications/OpenAgents RC7 Proof.app`. Version,
deep signature validity, and stapled-ticket validity passed. However, audit of
the driver found that its installed-app selector is
`OPENAGENTS_DESKTOP_MVP_PROOF_APP`; an earlier invocation used a stale variable
name and therefore fell back to source Electron. Corrected invocations did not
produce an isolated installed-app journal: packaged helper processes inherited
the normal profile instead of the requested proof profile and the driver
terminalized without the required steps. The proof copy was removed without
replacing the existing app. This is a falsifier, not a pass, for the installed
journey and must be repaired in a later candidate.

## Criterion disposition

- The artifact gives current evidence for signing, notarization, packaged
  identity, custom icon, and source-independent ordinary launch. The
  source-launched journey gives current real evidence for the
  ordinary-current-session execution portions of `CW-AC-02`, `CW-AC-04`,
  `CW-AC-06`, `CW-AC-07`, `CW-AC-08`, `CW-AC-11`, and `CW-AC-13`.
- This receipt does not by itself promote every clause in those criteria or
  every other criterion to live-proven. Existing deterministic suites remain
  the fixture evidence for retry, revision reconciliation, recovery, bounded
  files/Git, diagnostics, and update/rollback state machines.
- `CW-AC-01` remains incomplete for an installed real-journey receipt.
- `CW-AC-18` remains incomplete for an exact-candidate real journey, renderer reload and app
  restart during the real journey, interrupted update, signed-feed update,
  rollback/downgrade refusal, diagnostics export, and uninstall/reinstall
  sequence.
- Product owner acceptance/waiver remains an explicit unresolved gate. This
  receipt does not publish RC7 or mutate an update feed.

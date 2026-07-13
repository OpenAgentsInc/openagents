# OpenAgents Codex Workroom RC6 candidate receipt

- Date: 2026-07-13
- Tracking issue: [#8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
- Source commit: `8b468006234939cf8952e665d5c653af3cddd0a4`
- Candidate version: `0.1.0-rc.6`
- Result: technically valid signed candidate; MVP acceptance remains incomplete

This receipt records only the exact release-candidate work completed from the
Codex Workroom ProductSpec. It does not publish RC6, satisfy the real-Codex
vertical journey, dispose an owner gate, or close #8756.

## Exact artifacts

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.6-arm64.dmg` | `300334632` | `b1224d87a849f3d36c6324216af48042cce1eb675ca229ad0a63c811630c381c` | `42084aeb-0fef-4921-8c34-8d29b0049d60` (`Accepted`) |
| `OpenAgents-darwin-arm64-0.1.0-rc.6.zip` | `304540060` | `35c983b6500bb39466147d4b2117ba6fcdd4aa344c6f3a52e188e0cb03551ba8` | App bundle submission `bc1a098a-f789-48f9-93a1-6ede2eed3df0` (`Accepted`) |

The final DMG digest was computed after its accepted ticket was stapled. The
contained application reports bundle ID `com.openagents.desktop`, team ID
`HQWSG26L43`, Developer ID authority `OpenAgents, Inc.`, hardened runtime, and
bundle/build version `0.1.0-rc.6`. Deep strict `codesign` verification and
`stapler validate` passed for the app; `stapler validate` also passed for the
final DMG.

## Build and installed proof

The release preflight ran at clean `origin/main` and passed all eight gates:
clean source, monotonic RC5-to-RC6 version, attribution, stable app identity,
complete artifact set, no upstream updater remnants, no legacy UI entrypoints,
and no absolute source-checkout paths. The guarded source push passed 1,172
Desktop tests and the built Electron smoke before RC6 packaging.

The DMG was mounted read-only and its contained app passed version, deep
signature, and stapled-ticket checks. A reversible installed proof then:

1. preserved the existing `/Applications/OpenAgents.app` RC5 installation by
   same-volume rename;
2. copied RC6 from the mounted DMG to `/Applications/OpenAgents.app`;
3. rechecked version, deep signature, and the app ticket;
4. ran the complete packaged fixture smoke to `OK` with lifecycle teardown at
   zero active resources;
5. restored RC5 and verified its `0.1.0-rc.5` bundle version; and
6. removed the temporary RC6 proof copy and detached the volume.

The installed smoke covered shell/bootstrap, Runtime Gateway, bounded
workspace file/search/save behavior, durable editor recovery, command and
history routing, diagnostics redaction, Codex/Fable fixture streams, child
graph and metadata presentation, voice fixture state, Fleet, Git review, PTY,
image attachment, reload restoration, and clean teardown. The expected
non-repository Git diagnostics were explicit; the smoke still finished green.

## ProductSpec validation and criterion disposition

`bun packages/product-spec/src/cli.ts validate
docs/mvp/openagents-codex-workroom-mvp.product-spec.md` returned `ok` against
the exact checked-in revision.

- `CW-AC-01` has technical candidate evidence for signing, notarization,
  install, and source-independent launch. Its owner acceptance remains open.
- `CW-AC-18` has only the install, launch, reinstall/restore, and cleanup
  portion. It remains incomplete for the real named-account Codex workroom,
  renderer/app restart within that journey, interrupted update, signed-feed
  update, and rollback/downgrade-refusal proof against RC6.
- No criterion is promoted from fixture proof to real-Codex or owner-accepted
  proof by this receipt.

## Open gates

- This RC6 receipt used the now-superseded named-isolated-account criterion.
  The owner corrected the MVP boundary on 2026-07-13: the workroom must use the
  user's ordinary logged-in Codex session and expose no Pylon account-linking
  flow. RC6 therefore remains historical evidence and is not an acceptance
  blocker for the corrected candidate.
- RC6 has not been published to the production update feed. Publishing remains
  downstream of an explicit release decision; this receipt does not mutate the
  feed.
- The installed real-Codex ProductSpec journey and all ProductSpec owner gates
  remain pending. Issue #8756 must stay open.

# macOS arm64 signed + notarized Desktop candidate receipt (rc.26)

- Date: 2026-07-20
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issue: [DIST-06 #8919](https://github.com/OpenAgentsInc/openagents/issues/8919)
- ProductSpec: [OpenAgents Desktop cross-platform release](../openagents-desktop-cross-platform-release.md) §§2-7, 14.1
- Target key: `darwin-arm64` (Apple Silicon)
- Channel: `rc`
- Version: `0.1.0-rc.26`
- Source revision: `8476488361` (clean `origin/main`, version bumped rc.25 → rc.26)
- Host: local Apple Silicon Mac, macOS 26.4, `xcrun notarytool` 1.1.2 (41),
  Electron Forge maker path (`make:mac` → `stage-and-package.ts --target
  darwin-arm64 --mode make`).

## Status

This is a **signed + notarized CANDIDATE for acceptance evidence only**. It was
NOT published, tagged, promoted, or uploaded to any public feed. It refreshes
the prior rc.25 arm64 candidate
([`2026-07-20-macos-arm64-signed-candidate.md`](./2026-07-20-macos-arm64-signed-candidate.md))
against a current `origin/main` to re-prove the signing + notarization pipeline
end to end with the owned Developer ID. #8919 stays **OPEN**: a full promoted
release set (all required targets, converged, promoted, owner-accepted on a
clean machine) is DIST-12/13 work, and the real release-coordinator (#8917) and
feed/promotion (#8922) ports are still dry-run-only skeletons.

## Artifacts (this build)

| Artifact | SHA-256 | Bytes |
| --- | --- | --- |
| `OpenAgents-0.1.0-rc.26-rc-darwin-arm64.dmg` | `095f7281f4072bfc0a03b6103b0dcdaf26e34037be94c06d8b0ed68482dc4b73` | 204473917 |
| `OpenAgents-0.1.0-rc.26-rc-darwin-arm64.zip` | `db4f0980d006040c051550cd423cc5b0824d47e5eb0922deabf4ac891f64beda` | 206467027 |

Artifacts are kept OUTSIDE the repository (under
`apps/openagents-desktop/out/`, gitignored). Never committed.

## Trust verification (all PASS)

- **Preflight** (`release-preflight.ts --channel rc --latest-released
  0.1.0-rc.25`): all 9 oracles PASS, including `version_monotonic`
  (0.1.0-rc.25 → 0.1.0-rc.26 strict rc upgrade) and `signing_credentials_present`
  (Developer ID identity + ASC notary credentials resolved by name, values
  never printed).
- **Codesign identity**: `Developer ID Application: OpenAgents, Inc.
  (HQWSG26L43)`, team `HQWSG26L43`.
- **App** (`OpenAgents RC.app`): `codesign --verify --deep --strict` valid and
  satisfies its Designated Requirement; `spctl -a -t exec` reports
  `source=Notarized Developer ID`, origin the Developer ID identity above;
  `stapler validate` passed; `lipo -info` confirms `arm64`.
- **Notarization**: app submission `a2fd8691-8513-4f9e-aa0f-a7a2c77f5a4a`
  Accepted; DMG submission `0901d1f9-bf78-4f33-8d29-bfc21d511c24` Accepted (the
  DMG is notarized itself; the ticket covers the nested app).
- **DMG**: `stapler validate` passed; `spctl -a -t open --context
  context:primary-signature` reports `accepted`, `source=Notarized Developer
  ID`.

## Boundary

No unsigned fallback was used. No artifact was published, tagged, promoted, or
uploaded to a public feed. The ed25519 release-set signature, channel-pointer
promotion, `/download` convergence, and the clean-machine install / update /
rollback acceptance are the owner-gated DIST-12 (#8925) / DIST-13 (#8926) steps
and remain outstanding.

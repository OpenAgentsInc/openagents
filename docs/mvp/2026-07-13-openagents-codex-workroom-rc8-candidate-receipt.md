# OpenAgents Codex Workroom RC8 candidate receipt

- Date: 2026-07-13
- Tracking issue: [#8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
- Source commit: `ee1626fa43`
- Candidate version: `0.1.0-rc.8`
- Result: exact installed signed/notarized real-Codex and RC7-to-RC8 release-lifecycle journeys passed; owner gates remain

## Exact artifacts

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.8-arm64.dmg` | `304481641` | `1cc3a54a932be27c812d22359edee90d90e30ae4587b8ea7be06b396d1790a6b` | `eed3b837-3595-467b-a4b0-53a4b48c443e` (`Accepted`) |
| `OpenAgents-darwin-arm64-0.1.0-rc.8.zip` | `308690509` | `b468425faf78b1fa39f728a5c688d574c6f87154e726ef260500bbae2cf3142f` | App submission `eaf352e7-d304-4d79-bb27-027d512836be` (`Accepted`) |

The DMG digest is post-staple. The app passed deep strict signing and stapler
validation with bundle ID `com.openagents.desktop`, team ID `HQWSG26L43`,
Developer ID authority `OpenAgents, Inc.`, hardened runtime, and version
`0.1.0-rc.8`. The DMG was separately accepted, stapled, and validated.

## Exact installed journey

The accepted DMG was mounted read-only and its app copied to the reversible
proof install `/Applications/OpenAgents RC8 Proof.app`. The driver launched
that exact executable with closed proof coordinates carried in argv. Before
Electron selected driver mode or `userData`, main reconstructed only the
bounded isolated-proof environment. Packaged renderer, helper, worker, and
bundled Codex processes all used the unique OS-temp proof profile rather than
the normal OpenAgents profile.

The installed app used the user's ordinary logged-in Codex session. It did not
select, link, rotate, or fall back to a named Pylon account. It passed all 12
required journal steps:

1. Effect Native shell mounted.
2. Ordinary logged-in Codex session passed host preflight.
3. Validator-clean ProductSpec opened through the workroom.
4. Two-packet execution plan was accepted.
5. Real root Codex packet terminalized with non-text tool evidence.
6. Agent-produced root evidence passed independent host verification.
7. Real delegated Codex child packet terminalized with one causal child card.
8. The child card opened the child's independent transcript.
9. Agent-produced child evidence passed independent host verification.
10. Renderer reload restored the accepted plan, two verified packets, and two
    pending owner gates without redispatch.
11. Both verified packets remained behind explicit owner disposition.
12. A second installed app process on the same durable isolated profile
    restored the accepted plan, both verified packets, and both owner gates
    without redispatch.

The terminal verdict was `acceptance PASSED` and `summary OK
{"requiredSteps":12,"ownerAcceptanceFabricated":false}`. The reversible proof
copy was removed after receipt capture; the existing user installation was not
replaced.

## Exact release lifecycle

Verifier commit `e1e298d7b1` ran the product update client and real macOS update
applier against the exact accepted RC7 and RC8 DMGs. It used the production
pinned release key to generate and self-verify a local signed manifest, but did
not deploy that manifest or mutate any public feed or registry.

The public-safe journal passed all 11 entries:

1. Installed stapled RC7 as a reversible proof app.
2. Verified the production-pinned signed RC8 manifest through the exact client
   seam.
3. Downloaded and digest-verified RC8, then proved the staged update survived
   update-host destruction and reopen.
4. Verified the mounted candidate with deep strict code signing, exact bundle,
   version and Developer ID identity, Apple `syspolicy_check distribution`,
   and stapler validation; atomically replaced the real RC7 app with RC8.
5. Refused RC7 as a downgrade candidate outside the retained rollback slot.
6. Consumed the one retained rollback slot and restored exact RC7.
7. Exported a schema-valid, redacted diagnostic receipt with owner-only mode.
8. Uninstalled the reversible proof app.
9. Reinstalled exact stapled RC8 from the accepted DMG.
10. Removed the proof app, rollback slot, staged bytes, diagnostic export,
    mounts, and private update state.
11. Reported the exact RC7-to-RC8 sequence passed without deployment.

The release driver exited zero. Its retained public-safe journal contained no
credential, signing secret, account identity, prompt, transcript, repository
content, or absolute private path.

## Remaining disposition

- This receipt supplies exact-candidate installed evidence for the core
  current-session ProductSpec execution journey and its renderer/app restart
  recovery. It supersedes RC7's installed-driver falsifier without rewriting
  the historical RC7 receipt.
- Together, the installed real-Codex journey and exact release-lifecycle
  journal satisfy the complete `CW-AC-18` candidate sequence.
- Deterministic suites still provide the narrow fault and falsifier evidence
  for the other criteria; the separate criterion audit decides their narrowest
  true disposition rather than promoting them from this candidate receipt.
- RC8 is not published and no update feed or registry was changed.
- Product-owner acceptance/waiver remains pending and was not fabricated.

# OpenAgents Codex Workroom RC8 candidate receipt

- Date: 2026-07-13
- Tracking issue: [#8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
- Source commit: `ee1626fa43`
- Candidate version: `0.1.0-rc.8`
- Result: exact installed signed/notarized 12-step real-Codex journey passed; update/rollback and owner gates remain

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

## Remaining disposition

- This receipt supplies exact-candidate installed evidence for the core
  current-session ProductSpec execution journey and its renderer/app restart
  recovery. It supersedes RC7's installed-driver falsifier without rewriting
  the historical RC7 receipt.
- `CW-AC-18` remains incomplete for interrupted update, signed-feed update,
  rollback/downgrade refusal, diagnostics export, and uninstall/reinstall plus
  cleanup as one exact-candidate sequence.
- Deterministic suites cover those update, rollback, diagnostics, cleanup, and
  remaining criterion clauses at fixture level; this receipt does not promote
  them to exact-candidate live proof.
- RC8 is not published and no update feed or registry was changed.
- Product-owner acceptance/waiver remains pending and was not fabricated.


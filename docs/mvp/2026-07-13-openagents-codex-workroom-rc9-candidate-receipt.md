# OpenAgents Codex Workroom RC9 candidate receipt

- Date: 2026-07-13
- Tracking issue: [#8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
- Source commit: `c388bf7e10`
- Candidate version: `0.1.0-rc.9`
- Result: exact installed signed/notarized real-Codex and RC8-to-RC9
  release-lifecycle journeys passed; owner gates remain

## Exact artifacts

| Artifact | Bytes | SHA-256 | Apple submission |
| --- | ---: | --- | --- |
| `OpenAgents-0.1.0-rc.9-arm64.dmg` | `304468181` | `2dbc860d5544a400e6cb498d506d27c7696b06fa43c6028ee1ea69e397ec64e3` | `8447f6bd-bf1b-4b5b-98e3-5941fd9ce817` (`Accepted`) |
| `OpenAgents-darwin-arm64-0.1.0-rc.9.zip` | `308690852` | `9f769e316fd6d43b7fe667c7873201be7dc4130f9e05fe0bcf88b3757484de91` | App submission `89438861-866f-4499-a08c-ee2fd8426516` (`Accepted`) |

The DMG digest is post-staple. The app passed deep strict signing and stapler
validation with bundle ID `com.openagents.desktop`, team ID `HQWSG26L43`,
Developer ID authority `OpenAgents, Inc.`, hardened runtime, and version
`0.1.0-rc.9`. The DMG was separately accepted, stapled, and validated.

RC9 contains the distinct Codex `quota_exhausted`, `rate_limited`,
`auth_revoked`, and `policy_denied` states added in commits `5665c7ab74` and
`01bbf54348`. This supersedes RC8 as the exact candidate for the revision-6
ProductSpec; it does not rewrite the historical RC8 receipt.

## Exact installed journey

The accepted DMG was mounted read-only and its app copied to the reversible
proof install `/Applications/OpenAgents RC9 Proof.app`. The driver launched
that exact executable with a unique temporary product profile and workspace.
The installed app used the user's ordinary logged-in Codex session. It did not
select, link, rotate, or fall back to a named Pylon account.

All 12 required journal steps passed:

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
12. A second installed app process restored the same plan, packets, and owner
    gates without redispatch.

The terminal verdict was `acceptance PASSED` and `summary OK
{"requiredSteps":12,"ownerAcceptanceFabricated":false}`. The proof copy is
reversible and is not a published installation.

## Exact release lifecycle

The generalized release-acceptance driver ran the product update client and
real macOS update applier against the exact accepted RC8 and RC9 DMGs. It used
the production-pinned release key to generate and self-verify a local signed
manifest, but did not deploy that manifest or mutate any public feed or
registry.

The public-safe journal passed all 11 entries:

1. Installed stapled RC8 as the reversible update source.
2. Verified the production-pinned signed RC9 manifest through the exact client
   seam.
3. Downloaded and digest-verified RC9, then proved the staged update survived
   update-host destruction and reopen.
4. Verified and atomically replaced the real RC8 app with notarized RC9.
5. Refused RC8 as a downgrade outside the retained rollback slot.
6. Consumed that slot and restored exact notarized RC8.
7. Exported a schema-valid, redacted diagnostic receipt with owner-only mode.
8. Uninstalled the reversible proof app.
9. Reinstalled exact stapled RC9 from the accepted DMG.
10. Removed the proof app, rollback slot, staged bytes, diagnostic export,
    mounts, and private update state.
11. Reported the exact RC8-to-RC9 sequence passed without deployment.

The release driver exited zero.

## Deterministic fault closure

The focused `CW-AC-16` corpus passed `95` tests with `0` failures. It includes
lost acknowledgement with one-effect replay, duplicate and out-of-order
delivery, cursor-gap replay from the durable cursor before returning live,
stale-generation fencing, grant revocation, and distinct quota, rate-limit,
authentication-revocation, and policy-denial classifications. The full Desktop
pre-push gate also passed `1,163` tests with `39` retired-surface skips and `0`
failures, followed by the built Electron smoke.

## Remaining disposition

- RC9 supplies the exact-candidate journey required by `CW-AC-18` and contains
  the complete `CW-AC-16` classification fixes.
- RC9 is not published and no update feed, release registry, behavior registry,
  Eval registry, or promise registry was changed.
- Product-owner gates remain pending and were not fabricated.

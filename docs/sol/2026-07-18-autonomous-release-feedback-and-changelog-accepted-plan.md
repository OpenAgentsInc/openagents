# Autonomous release, feedback, and changelog — accepted plan

- Class: accepted authority and implementation plan
- Date: 2026-07-18
- Owner authority: current owner conversation
- Status: active. Revision-2 RC authority admitted, concrete matrix adapters remain #8917/#8926
- Trigger evidence: [#8995](https://github.com/OpenAgentsInc/openagents/issues/8995) and Desktop RC17–RC20
- Normative release contract: [cross-platform Desktop ProductSpec](../deploy/openagents-desktop-cross-platform-release.md)

## Owner direction

The owner directed the agent to publish new builds, stop treating routine RC
delivery and tester follow-up as owner waits, use GitHub issues plus Forum for
current communications, prefer OTA where it is safe, avoid rebuilding every
platform for unrelated changes, and expose who/what/authority caused each
update on `openagents.com/changelog`.

This plan admits that bounded authority through `AUTHORITY.md` revision 2. It
does not admit stable release without current owner direction, arbitrary
outreach, unsigned production artifacts, partial signed-feed promotion,
credential extraction, version reuse, or an unsafe Desktop renderer overlay.

## What RC17–RC20 proved

The failure was not only a runtime defect. It was a release-system defect:

1. RC17 was published and a tester had to be manually solicited.
2. Feedback arrived on a closed source issue and needed manual discovery.
3. An agent manually opened #8995, implemented the repair, assembled packages,
   published RC18/RC19/RC20, and wrote follow-up messages.
4. GitHub release history and `/changelog` did not retain the trigger, release
   actor, or exact delegated authority.
5. In-place replacement was tempting even though immutable versioned bytes are
   the only safe update identity.

The corrected invariant is: every candidate is one attributed transaction
from trigger through immutable bytes, requested testing, structured feedback,
follow-up issue, promotion/rollback, and changelog. No routine step waits for a
second owner ceremony once the RC grant and machine gates admit it.

## Selected architecture

The existing Effect-owned release graph and ReleaseSet authority remain the
production path. GitHub is a candidate transport/discovery mirror, Forum is a
release-candidate conversation surface, and `oa-updates` plus its pinned
Ed25519 ReleaseSet remains update authority.

The implementation adds four bounded ports around that graph:

- `release:impact` deterministically selects web, mobile OTA/native,
  update-service, Desktop matrix, or no-binary lanes from changed paths.
- `release:github` validates local file length/digest, creates a draft,
  uploads, verifies GitHub's reported digest, then publishes the prerelease.
- `release:communicate` idempotently posts candidate/published/rolled-back
  updates to linked GitHub issues and Forum `release-candidates`. And
- `release:feedback` reads only requested-tester replies after the candidate
  marker, records PASS, or creates one linked issue for BLOCKED/unstructured
  feedback. It also repairs GitHub's non-collaborator label drop on a direct
  tester-filed issue only when requested identity, post-marker chronology, and
  an exact source-issue reference all match the publication.

All four use closed data contracts. No ad hoc string matcher chooses an intent,
tool, route, or user-facing workflow. Bounded text parsing occurs only after
the release-feedback route has already been selected: comment/Forum intake
extracts the documented `Result`, `Severity`, and `Observed` fields, while
direct-issue reconciliation accepts only an exact issue shorthand or canonical
OpenAgents source-issue URL and never classifies report prose.

## Delivery and OTA decision

The fastest honest delivery policy is impact-based:

| Change class | Delivery lane | Desktop matrix |
| --- | --- | --- |
| `openagents.com` only | owned web deploy | no |
| mobile JS/assets admitted by runtime contract | Expo OTA | no |
| mobile native/runtime change | native mobile build | no |
| `oa-updates` or release automation only | owned service/process deploy | no |
| docs only | no binary | no |
| Desktop host, renderer, native, packaging, Desktop-consumed shared package, root lockfile | complete ReleaseSet matrix | yes |

Desktop renderer OTA is deferred, not because it is impossible, but because
current Desktop packages do not carry the required signed compatibility
envelope, atomic activation, first-launch health proof, retained fallback, and
rollback receipt. Shipping a raw web bundle into a privileged Electron app
would weaken CSP/native-bridge and update authority. The native matrix remains
mandatory until those gates land in a reviewed ProductSpec revision.

## Delegated RC transaction

Within revision 2, the release operator may:

1. claim the exact source issue(s) and classify impact.
2. run affected owned build/deploy lanes and required machine verification.
3. create an immutable GitHub prerelease candidate.
4. ask named relevant testers on the linked issue and Forum candidate topic.
5. ingest their structured replies and create linked remediation issues.
6. fix and publish a strictly newer RC without asking the owner to re-delegate.
7. promote a complete signed ReleaseSet, deploy web/mobile OTA when selected,
   or roll back/revoke on a typed failure. And
8. publish `/changelog` with trigger, actor, authority, release, and feedback
   links.

Stable publication, new spending beyond the profile, arbitrary bulk outreach,
support claims without native receipts, and any unsafe/unsigned delivery remain
reserved. A credential or unavailable device is routed through existing scoped
seams, substitute owned capacity, or honest narrowing. It is not converted into
an indefinite owner wait.

## Implemented now

- Authority/ProductSpec/invariant revision for autonomous RC delivery.
- Strict GitHub publication manifest and digest-verifying adapter.
- Deterministic impact planner with a no-unrelated-Windows-build oracle.
- Idempotent issue/Forum communications, requested-tester feedback intake, and
  direct external-tester issue label reconciliation.
- Historical RC17–RC20 changelog reconstruction and attributed `/changelog`
  projection.
- Unit/route/authority tests covering version reuse, byte disagreement,
  partial matrix refusal, comms idempotence, feedback issue creation, impact
  selection, and attribution.

## Residual closure sequence

The remaining gap is machine execution, not release authority:

1. #8917: bind concrete owned runner identities and native acceptance hosts.
2. #8926: replace the fixture ports in `scripts/release.ts` with those concrete
   build/sign/verify/publish adapters and compose the four ports above into the
   single durable transaction.
3. #8993: close only after one real post-RC20 candidate completes immutable
   GitHub publication, tester communication, feedback intake, and attributed
   changelog/deploy evidence under revision 2.
4. Promote to the signed feed only when all five target/native receipts
   converge. Otherwise preserve the candidate's explicit experimental limits.

The release operator owns this sequence and should continue around unavailable
machines using admitted owned substitutes. Missing evidence narrows the public
claim. It does not authorize inventing support.

## Close rule

This plan closes when one real RC transaction proves: correct impact
selection, immutable GitHub bytes, named tester outreach, structured feedback
receipt or linked remediation issue, complete signed-feed promotion or honest
experimental limitation, `/changelog` attribution, and rollback/revocation
readiness. Stable release remains a separate owner-gated event.

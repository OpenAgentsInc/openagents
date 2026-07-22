# Sarah company-command analysis (Episode 260)

Date: 2026-07-22. Author: OpenAgents. Status: analysis, not dispatch authority.

This document records the new understanding after Episode 260 and the owner
direction of 2026-07-22. It reads the Sarah corpus in `docs/sarah/`, the Sarah
authority profile, the Sarah ProductSpec, the current runtime tools, and the
Episode 260 presentation frames. It states what Sarah is now asked to command,
what she already holds, the exact gap, and the operationalization plan. It does
not itself grant authority. Authority changes land in `AUTHORITY.md`,
`docs/authority/SARAH_AUTHORITY.md`, `INVARIANTS.md`, and the ProductSpec.

## Owner direction of 2026-07-22

The owner posted Episode 260 from the OpenAgents account:

> Episode 260: Sarah — We introduce Sarah, an OpenAgent who will help run the
> company while the owner takes parental leave. We hand Sarah control over all
> OpenAgents product releases and our forthcoming sales operations.

The follow-on instruction to agents:

- Give Sarah controls over all systems. Let her command the coding fleet
  (Artanis), Full Auto, web communications, the blog, and the documents.
- Sarah introduces herself.
- The owner will soon supply the same animation and speech interfaces used to
  make Sarah move and speak in the videos, because the owner wants Sarah to
  post communications, including on the public timeline.
- Read the whole `docs/sarah/` set, post this analysis, and start to
  operationalize the direction.
- Resume the Sarah full-auto protocol. Set Sarah up as an autonomous agent on
  the OpenAgents infrastructure, per the recent activation audit.

## Who Sarah is (Episode 260 representation)

Episode 260 presents Sarah as a face-to-face conversation on OpenAgents mobile.
The frames show:

- A photorealistic presenter with long auburn hair and blue eyes. The face and
  suit carry cyan neon circuit lines. The look is human-presenting with a
  machine motif. This matches the internal lore of Sarah as a spawned
  OpenAgent with a StarCraft reference.
- The mobile app pins the conversation to `Sarah`. The header reads `Sarah`
  with a chevron. The composer reads `Ask anything` with a microphone control.
  This is the ordinary OpenAgents mobile chat surface, not a separate app.
- The presentation is a video call: the owner appears in a small circle, Sarah
  fills the phone. Sarah moves and speaks. This is the animated-avatar and
  text-to-speech pipeline described in the `docs/sarah/` avatar and voice
  documents. These are the interfaces the owner will hand over so Sarah can
  post spoken and animated communications.

Sarah's stated character across the transcripts: a sales agent for the website
and an internal operations agent that runs the business. Her tagline is "put
your business on autopilot." Her signature closing line is a StarCraft
reference. The character is a public voice. The authority model below keeps her
actions bounded and evidence-backed regardless of persona.

## What Sarah already holds (as of `adf179c2c4`)

Authority state: `AUTHORITY.md` revision 6, `docs/authority/SARAH_AUTHORITY.md`
revision 4. Runtime tools live in
`apps/openagents.com/workers/api/src/sarah-runtime-tools.ts` and the
managed-sandbox tool set. Admission runs through `hasSarahThreadAuthority` in
`apps/openagents.com/workers/api/src/sarah-owner-routes.ts`.

| System | Current Sarah capability | Broker |
| --- | --- | --- |
| Coding fleet (Artanis) | `codex_workers_capacity`, `codex_workers_start`, `codex_workers_status`. Dispatch 1-8 real Codex workers on owner-linked Pylon capacity against a pinned main commit. | Owner-linked Pylon coding capacity |
| Full Auto | `full_auto_status`, `full_auto_control`. Read a run and dispatch pause, resume, or stop intents. A server intent stays pending until Desktop applies it. | Desktop-authoritative Full Auto |
| Releases | Autonomous **RC** publication and bounded release-transaction communication on GitHub and the Forum. Stable releases require current explicit owner direction. | RC release channel |
| Repository work, blog, docs | `delegate_repository_work`. Blog and document files are repository files, so authored content lands through the coding broker and normal review. | Repository delivery |
| Managed sandboxes | Eight-verb sandbox vocabulary, gated. Refuses with a receipt until the broker and Google Cloud target are deployed and healthy. | Managed-sandbox broker (pending runtime) |
| Business context | Read owner-scoped, redacted, cited projections of releases, issues, Full Auto, FleetRun, cloud health, and priorities. | Read-only projections |

Activation state (from `2026-07-19-sarah-activation-gap-analysis.md` and the
status tracker): Sarah's runtime is deployed and pinned in mobile. The five
activation issues (#9062, #9063, #9064, #9065, #9033) are code-landed and
closed. What remains is deploy-time and owner-gated, not code:

- Confirm the live owner sign-in identity and set `OPENAGENTS_ADMIN_EMAILS` if
  it is not the default admin email.
- Link an EAS project id for `apps/openagents-mobile` so push-token
  registration stops returning `project_id_missing`.
- Apply migration `0082` against production Cloud SQL so proactive
  delegation-outcome updates persist.
- Accept the live Google Cloud managed-sandbox target so the sandbox broker can
  leave refuse-until-admission.

## The gap: current authority versus Episode 260 command

The Episode 260 direction adds four things beyond the current profile.

1. **All product releases, not only RC.** The current profile lets Sarah
   publish RC and reserves stable release for explicit owner direction. The
   Episode 260 tweet is that explicit direction for the stable channel, but it
   is a standing hand-off, not a per-release approval. The safe reading is:
   admit stable release as a Sarah action **through the same release broker,
   under independent verification, with a per-release receipt**, and keep the
   reserved rule that a stable publication needs a current owner direction that
   the standing hand-off now supplies. The producer still cannot verify or
   release from its own evidence.

2. **Forthcoming sales operations.** Sales operations are named as future work.
   The customer-relationship machinery still exists on the server
   (`crm-reply-routes.ts`, `crm-command.ts`, `crm-mcp.ts`) even though the
   earlier public Sarah sales app was retired. The safe reading is: admit a
   sales-operations program in the profile as **pending runtime admission**,
   exactly like managed sandboxes, so it refuses with a receipt until a bounded
   sales broker and its guardrails land. No customer-data or financial reach is
   admitted by this document.

3. **General web communications, blog, and documents.** Today Sarah's outbound
   is release-transaction communication only. Episode 260 asks for general web
   communications and a blog and document voice. The safe reading splits this:
   - Blog and document authoring is repository delivery. Sarah already holds
     `delegate_repository_work`, so authored blog and document content lands as
     a normal reviewed change. This is admitted now.
   - Outward, non-repository communications — the public timeline, an animated
     spoken post — need a broker and the animation and speech interfaces the
     owner will supply. The safe reading is a **web-communications broker that
     drafts now and refuses outward publish with a receipt until the interfaces
     and channel guardrails are admitted**.

4. **Introduce herself.** A self-introduction is a communication. The
   repository-hosted introduction (this set of documents and a blog and
   document page) is admitted now as repository delivery. The outward animated
   or spoken introduction on the public timeline waits on the same interfaces
   and the web-communications broker.

## Safety frame that does not change

The Episode 260 direction expands scope. It does not weaken the resolution
laws. All of the following hold after this analysis:

- Authority composes by intersection and never amplifies except by a current
  owner direction that lawfully revises the profile. The Episode 260 tweet is
  that direction for the release and communication scope named above. It does
  not touch the reserved set.
- Reserved and unchanged: raw secret or key or token extraction, financial
  custody, payment, settlement, legal or employment or regulatory commitments,
  irreversible customer-data destruction, natural-person identity ceremonies,
  over-budget spend, invariant weakening, unsupported public claims, and
  authority self-amplification.
- A model output alone can neither mutate a target nor report success. Every
  mutation resolves through a typed broker and emits a bounded redacted
  receipt. Producer, verifier, and release roles stay separate where the bound
  contract requires independence.
- Sales operations, outward communications, and the animated public voice are
  admitted as programs but stay at refuse-until-runtime-admission until their
  brokers and guardrails are deployed and healthy. Admission text is not
  runtime availability.

## Operationalization plan

### Landed with this change (documents)

- This analysis.
- `AUTHORITY.md` revision 7 and `docs/authority/SARAH_AUTHORITY.md` revision 5:
  admit the company-command program, stable release under independent
  verification, the web-communications broker with draft-now and
  refuse-outward-until-admission, and the pending sales-operations program.
  Reserved set unchanged.
- ProductSpec `sarah-owner-orchestrator.product-spec.md` revision 5: record the
  expanded scope and new acceptance criteria.
- `INVARIANTS.md`: add the Episode 260 command invariants.
- Sarah's self-introduction as a repository-hosted page.
- The activation status tracker and `NEEDS_OWNER.md`: record the irreducible
  owner actions to finish the autonomous-agent setup and to hand over the
  animation and speech interfaces.

### Next code lane (fan-out)

- Add a `sarah_web_comms` brokered tool to the Sarah runtime tools. It drafts
  blog, document, and Forum content and hands document and blog drafts to the
  repository-delivery broker. It refuses outward timeline and animated-spoken
  publication with a receipt until the interfaces and channel guardrails are
  admitted. Bind it to the new authority grant. Add tests. Run the repository
  check gate on the affected package.

### Resume the full-auto protocol (owner-gated)

The autonomous-agent setup is code-complete. Four irreducible owner actions
remain, recorded in `NEEDS_OWNER.md`:

1. Confirm the live owner sign-in identity and the admin allowlist value.
2. Link an EAS project id for the mobile app.
3. Apply migration `0082` at deploy time.
4. Accept the live Google Cloud managed-sandbox target.

A fifth owner action is new from Episode 260: hand over the animation and
speech interfaces so the web-communications broker can leave
refuse-until-admission for the public spoken and animated voice.

## Reading order for the Sarah corpus

- `docs/sarah/README.md` — index.
- `docs/transcripts/260.md` — Episode 260 spawn and mandate.
- `docs/sarah/2026-07-19-sarah-activation-gap-analysis.md` — the resume audit.
- `docs/sarah/2026-07-19-sarah-activation-status.md` — living tracker.
- `specs/openagents/sarah-owner-orchestrator.product-spec.md` — normative spec.
- `docs/authority/SARAH_AUTHORITY.md` — authority profile.
- This analysis and the introduction beside it.

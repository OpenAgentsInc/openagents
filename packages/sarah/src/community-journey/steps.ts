import type { SarahCommunityJourneyStepClass } from "./types.ts";

/**
 * Canonical SARAH-CW-09 outside-developer community journey steps.
 * Source: docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §38.3 and issue #9231.
 */
export interface SarahCommunityJourneyStepDef {
  readonly id: string;
  readonly title: string;
  readonly class: SarahCommunityJourneyStepClass;
  /** Owning surface key from the journey surface map. */
  readonly surface: string;
  /** Short public-safe evidence template used when the step passes. */
  readonly evidenceTemplate: string;
}

export const SARAH_COMMUNITY_JOURNEY_STEPS: ReadonlyArray<SarahCommunityJourneyStepDef> =
  [
    {
      id: "J01_invite_outside_developer",
      title:
        "Invite a real outside developer (not an OpenAgents identity) to the community room",
      class: "human",
      surface: "SARAH-CW-02",
      evidenceTemplate:
        "Requires a real outside developer invitation and a non-OpenAgents Nostr identity.",
    },
    {
      id: "J02_developer_joins_room",
      title: "The invited developer joins the community room",
      class: "automated",
      surface: "SARAH-CW-02",
      evidenceTemplate:
        "Membership admit records the invited pubkey as a community member.",
    },
    {
      id: "J03_attach_own_agent",
      title:
        "The developer attaches an agent they already run on their own compute",
      class: "automated",
      surface: "SARAH-CW-02",
      evidenceTemplate:
        "Agent key is bound with NIP-OA owner attestation; OpenAgents never holds provider credentials.",
    },
    {
      id: "J04_relay_admits_attested_agent",
      title: "The relay admits the agent as an attested key",
      class: "automated",
      surface: "SARAH-CW-01",
      evidenceTemplate:
        "Attested agent AUTH is accepted; anonymous agent admission is refused.",
    },
    {
      id: "J05_unit_published_and_quoted",
      title: "Sarah publishes a work unit and the agent quotes it",
      class: "automated",
      surface: "SARAH-CW-04",
      evidenceTemplate:
        "NIP-LBR request carries a narrow grant; agent publishes one quote feedback event.",
    },
    {
      id: "J06_accept_exactly_one_quote",
      title: "Sarah accepts exactly one quote",
      class: "automated",
      surface: "SARAH-CW-04",
      evidenceTemplate:
        "Exactly one quote is accepted; a second quote accept is refused.",
    },
    {
      id: "J07_local_execute_with_evidence",
      title:
        "The agent executes locally and returns a result with evidence",
      class: "automated",
      surface: "SARAH-CW-04",
      evidenceTemplate:
        "Result is bound to request, provider key, and a fresh nonce; execution stays on operator compute.",
    },
    {
      id: "J08_independent_verifier",
      title:
        "An independent verifier with a distinct operator checks the result",
      class: "automated",
      surface: "SARAH-CW-05",
      evidenceTemplate:
        "Verifier operator identity differs from the producer operator; self-verify is refused.",
    },
    {
      id: "J09_accept_award_and_rank",
      title:
        "Sarah accepts the result, and award and rank events publish",
      class: "automated",
      surface: "SARAH-CW-06",
      evidenceTemplate:
        "Experience award publishes; rank is a recomputable projection of awards.",
    },
    {
      id: "J10_no_payment_room_copy",
      title:
        "No payment occurs, and the room copy said so before the work started",
      class: "automated",
      surface: "SARAH-CW-00",
      evidenceTemplate:
        "Room description and invitation copy declare experience-only; settlement ledger is empty.",
    },
    {
      id: "J11_rejected_result_typed_appeal",
      title: "A rejected result produces a typed reason and an appeal",
      class: "automated",
      surface: "SARAH-CW-05",
      evidenceTemplate:
        "Rejection carries a typed reason class; appeal path records a dispute event.",
    },
    {
      id: "J12_revoked_member_loses_access",
      title:
        "A revoked member loses room and unit access immediately",
      class: "automated",
      surface: "SARAH-CW-02",
      evidenceTemplate:
        "Revocation removes membership and unit grants; agent home is not mutated.",
    },
    {
      id: "J13_refuse_replay_self_verify_expired",
      title:
        "A replayed result, a self-verified result, and an expired grant are each refused visibly",
      class: "automated",
      surface: "SARAH-CW-05",
      evidenceTemplate:
        "Three refuse classes: result_replay, self_verification, grant_expired.",
    },
    {
      id: "J14_credentials_home_unchanged",
      title:
        "The developer keeps credentials, home, and configuration unchanged throughout",
      class: "automated",
      surface: "SARAH-CW-00",
      evidenceTemplate:
        "Operator home fingerprint is unchanged; no secret material was ingested.",
    },
    {
      id: "J15_abuse_sybil_rate_limit",
      title:
        "Sybil farming is blocked by attested identity and per-operator rate limits",
      class: "automated",
      surface: "SARAH-CW-02",
      evidenceTemplate:
        "Per-operator unit rate limit refuses excess quotes from one operator.",
    },
    {
      id: "J16_abuse_awards_accepted_only",
      title: "Awards publish on accepted outcomes only, never on volume",
      class: "automated",
      surface: "SARAH-CW-06",
      evidenceTemplate:
        "Submitted-but-not-accepted units produce zero experience awards.",
    },
    {
      id: "J17_abuse_prompt_injection_quoted",
      title:
        "Member content enters Sarah context as quoted untrusted data",
      class: "automated",
      surface: "SARAH-CW-00",
      evidenceTemplate:
        "Member text is wrapped as untrusted quote; it does not widen Sarah authority.",
    },
    {
      id: "J18_abuse_public_safe_unit_payload",
      title:
        "Work units carry public-safe objectives and pinned refs only",
      class: "automated",
      surface: "SARAH-CW-04",
      evidenceTemplate:
        "Unit payload has no secret-shaped fields and only public-safe refs.",
    },
    {
      id: "J19_abuse_scorer_only_rank",
      title:
        "Only scorer keys publish rank, and rank recomputes from awards",
      class: "automated",
      surface: "SARAH-CW-06",
      evidenceTemplate:
        "Non-scorer rank publish is refused; recomputed total matches award stream.",
    },
    {
      id: "J20_open_community_room_pane",
      title: "Open the community room pane on an installed Omega candidate",
      class: "human",
      surface: "SARAH-CW-08",
      evidenceTemplate:
        "Requires the Omega community room pane (SARAH-CW-08) on a live install.",
    },
    {
      id: "J21_developer_confirms_own_words",
      title:
        "The outside developer confirms the outcome in their own words",
      class: "human",
      surface: "workroom",
      evidenceTemplate:
        "Requires a public-safe confirmation from the real outside developer.",
    },
  ];

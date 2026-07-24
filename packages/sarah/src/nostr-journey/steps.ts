import type {
  SarahNostrJourneyStepClass,
} from "./types.ts";

/**
 * Canonical SARAH-NR-09 journey steps.
 * Combines OMEGA-SW-07 install/bind surface steps with the Nostr-only extension
 * from docs/omega/2026-07-24-sarah-workroom-mvp-spec.md §24.10 and issue #9223.
 */
export interface SarahNostrJourneyStepDef {
  readonly id: string;
  readonly title: string;
  readonly class: SarahNostrJourneyStepClass;
  /** Owning surface key from the journey surface map. */
  readonly surface: string;
  /** Short public-safe evidence template used when the step passes. */
  readonly evidenceTemplate: string;
}

export const SARAH_NOSTR_JOURNEY_STEPS: ReadonlyArray<SarahNostrJourneyStepDef> =
  [
    {
      id: "J01_install_clean_profile",
      title: "Install the candidate from a clean profile",
      class: "human",
      surface: "OMEGA-SW-07",
      evidenceTemplate:
        "Requires a signed Omega candidate and a clean local profile.",
    },
    {
      id: "J02_bind_omega_identity",
      title: "Bind the Omega identity to the OpenAgents account",
      class: "human",
      surface: "OMEGA-SW-01",
      evidenceTemplate: "Requires live OMEGA-SW-01 loopback bind.",
    },
    {
      id: "J03_open_workroom_pane",
      title: "Open the workroom pane",
      class: "human",
      surface: "OMEGA-SW-03",
      evidenceTemplate: "Requires installed Omega workroom UI.",
    },
    {
      id: "J04_confirm_principal_refs",
      title:
        "Confirm the principal reference, conversation reference, and authority revision",
      class: "automated",
      surface: "workroom",
      evidenceTemplate:
        "principal.sarah, conversation sarah.<digest>, authority revision present.",
    },
    {
      id: "J05_sarah_attested_auth",
      title: "Sarah authenticates to the owned relay with an attested key",
      class: "automated",
      surface: "SARAH-NR-04",
      evidenceTemplate:
        "Attested NIP-42 AUTH signed by sealed principal.sarah signer.",
    },
    {
      id: "J06_owner_encrypted_message",
      title:
        "The owner sends a message only the owner and Sarah can decrypt",
      class: "automated",
      surface: "SARAH-NR-00",
      evidenceTemplate:
        "Owner message content is ciphertext; plaintext is not on the wire.",
    },
    {
      id: "J07_relay_operator_blind",
      title: "The relay operator cannot read the conversation content",
      class: "automated",
      surface: "SARAH-NR-00",
      evidenceTemplate:
        "Wire events hold ciphertext only; no plaintext JSON payload.",
    },
    {
      id: "J08_release_state_answer",
      title: "Ask Sarah for current release state and read the cited answer",
      class: "human",
      surface: "OMEGA-SW-03",
      evidenceTemplate:
        "Requires live Sarah release tools on an installed candidate.",
    },
    {
      id: "J09_coding_capacity_ladder",
      title:
        "Ask for coding capacity and observe the live tool ladder with no gap",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Live NIP-AO frames and durable ladder entries are gap-free by seq.",
    },
    {
      id: "J10_full_auto_pending",
      title:
        "Ask her to control an existing Full Auto run; pending stays pending until the host applies it",
      class: "human",
      surface: "OMEGA-SW-05",
      evidenceTemplate:
        "Requires live Full Auto host application path on Omega.",
    },
    {
      id: "J11_refusal_receipt",
      title:
        "Trigger one refusal; the signed receipt names its reserved category",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Authority refusal receipt carries a reserved category label.",
    },
    {
      id: "J12_interrupt_terminal",
      title: "Interrupt one turn; confirm the terminal event",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Cancel frame plus durable turn.interrupted terminal entry.",
    },
    {
      id: "J13_restart_mid_turn",
      title: "Restart Omega mid-turn; one honest outcome, never two answers",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Claim store rejects a second claim after a terminal finish.",
    },
    {
      id: "J14_replay_from_relay",
      title:
        "The durable ladder replays after restart from relay history alone",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Memory relay history alone rebuilds the ordered durable ladder.",
    },
    {
      id: "J15_kill_effectd",
      title: "Kill omega-effectd; recovery without a duplicate answer",
      class: "human",
      surface: "OMEGA-SW-07",
      evidenceTemplate: "Requires a live omega-effectd process.",
    },
    {
      id: "J16_usage_metric_agree",
      title:
        "The exact token_usage_events row and the signed NIP-AM metric agree",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Simulated exact usage totals match the signed NIP-AM metric body.",
    },
    {
      id: "J17_second_relay",
      title:
        "A second admitted relay serves the same history after the first is stopped",
      class: "automated",
      surface: "SARAH-NR-06",
      evidenceTemplate:
        "Second mock relay returns the same durable event ids as the first.",
    },
    {
      id: "J18_offline_publish",
      title: "An event signed while offline publishes after reconnection",
      class: "automated",
      surface: "SARAH-NR-06",
      evidenceTemplate:
        "Pre-signed offline event is accepted after mock reconnect.",
    },
    {
      id: "J19_reject_bad_inputs",
      title:
        "A stale, duplicate, unsigned, revoked, and unauthorized input is each rejected and starts no turn",
      class: "automated",
      surface: "SARAH-NR-05",
      evidenceTemplate:
        "Five bad-input classes reject without starting a second turn.",
    },
    {
      id: "J20_export_causal_chain",
      title:
        "An export verifies the causal chain without reading Cloud SQL",
      class: "automated",
      surface: "SARAH-NR-00",
      evidenceTemplate:
        "Export walks parent e-tags from relay events only.",
    },
    {
      id: "J21_network_degraded",
      title: "Disconnect the network; a visible degraded state, not a hang",
      class: "human",
      surface: "OMEGA-SW-03",
      evidenceTemplate: "Requires live UI degraded-state observation.",
    },
    {
      id: "J22_no_secret_in_logs",
      title: "Confirm no token, credential, or private path appears in any log",
      class: "automated",
      surface: "SARAH-NR-04",
      evidenceTemplate:
        "Receipt and mock log projection pass public-safe redaction.",
    },
    {
      id: "J23_remove_omega_no_side_effect",
      title:
        "Remove Omega; confirm Zed and Electron data did not change",
      class: "human",
      surface: "OMEGA-SW-07",
      evidenceTemplate:
        "Requires a clean install/remove comparison on the owner machine.",
    },
  ];

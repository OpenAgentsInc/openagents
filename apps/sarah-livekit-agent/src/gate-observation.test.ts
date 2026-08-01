import { describe, expect, test } from "vite-plus/test";
import { SARAH_LIVEKIT_SFU_LOSS_BOUND_MS } from "./failure-matrix.js";
import {
  REQUIRED_OBSERVATIONS,
  assertPublicSafeGateObservationReceipt,
  digestGateRef,
  recordSarahLiveKitGateObservation,
  type GateBinding,
  type GateObservation,
  type GateRowId,
} from "./gate-observation.js";

const now = () => 1_780_000_000_000;
const dependencies = { now };

const binding: GateBinding = {
  omegaReleaseTag: "v0.2.0-rc29",
  omegaPackageSha256: "116ae5ae542d4d0faed14a59e98ebeac2402ab022927738ed132e6c231e10c34",
  openagentsSourceRevision: "fdd0db9bab10568a97d1ea2d3ddc0f936ea6df66",
  livekitConfigRevision: `sha256:${"b".repeat(64)}`,
  sarahWorkerImageDigest: `sha256:${"c".repeat(64)}`,
};

const operatorRefDigest = digestGateRef("operator.owner");
const digest = (seed: string) => digestGateRef(seed);

const unobserved = (key: string): GateObservation => ({
  key,
  finding: "not_observed",
  reason: "the journey did not reach this step",
  observedAtMs: now(),
});

/** Every required key of a row, all recorded as honestly unobserved. */
const allUnobserved = (row: GateRowId): GateObservation[] =>
  REQUIRED_OBSERVATIONS[row].map(unobserved);

const withObservation = (row: GateRowId, observation: GateObservation): GateObservation[] => [
  ...allUnobserved(row).filter((entry) => entry.key !== observation.key),
  observation,
];

const record = (row: GateRowId, observations: readonly GateObservation[]) =>
  recordSarahLiveKitGateObservation(
    { row, binding, operatorRefDigest, observations },
    dependencies,
  );

describe("fail-closed recording", () => {
  test("rejects a row whose required observations are simply absent", () => {
    expect(() => record("sarah-livekit-room", [])).toThrow(/missing required observations/u);
  });

  test("names every missing observation so the operator knows what to repeat", () => {
    const observations = allUnobserved("sarah-livekit-room").slice(0, 2);
    expect(() => record("sarah-livekit-room", observations)).toThrow(/moderator_stop_completed/u);
  });

  test("accepts an honestly incomplete row and reports it as incomplete", () => {
    const receipt = record("sarah-livekit-room", allUnobserved("sarah-livekit-room"));
    expect(receipt.outcome).toBe("incomplete");
  });

  test("requires a reason for an unobserved step", () => {
    const observations = withObservation("sarah-livekit-isolation", {
      key: "context_isolated",
      finding: "not_observed",
      observedAtMs: now(),
    });
    expect(() => record("sarah-livekit-isolation", observations)).toThrow(/must say why/u);
  });

  test("refuses a finding asserted with no evidence", () => {
    const observations = withObservation("sarah-livekit-isolation", {
      key: "context_isolated",
      finding: "satisfied",
      observedAtMs: now(),
    });
    expect(() => record("sarah-livekit-isolation", observations)).toThrow(/no evidence/u);
  });

  test("refuses an observation the row does not define", () => {
    const observations = [
      ...allUnobserved("sarah-livekit-isolation"),
      unobserved("invented_observation"),
    ];
    expect(() => record("sarah-livekit-isolation", observations)).toThrow(/does not define/u);
  });

  test("refuses a duplicated observation", () => {
    const observations = [...allUnobserved("sarah-livekit-room"), unobserved("floor_acquired")];
    expect(() => record("sarah-livekit-room", observations)).toThrow(/more than once/u);
  });
});

describe("truthful negatives", () => {
  test("preserves a contradicted observation and fails the row", () => {
    const observations = withObservation("sarah-livekit-room", {
      key: "non_floor_refused",
      finding: "contradicted",
      observedAtMs: now(),
      evidence: {
        kind: "refusal",
        attempted: "summon Sarah without holding the floor",
        refusalCode: "not_floor_holder",
        httpStatus: 409,
      },
    });
    const receipt = record("sarah-livekit-room", observations);
    expect(receipt.outcome).toBe("observed_fail");
    expect(receipt.observations.find((entry) => entry.key === "non_floor_refused")?.finding).toBe(
      "contradicted",
    );
  });

  test("a contradicted observation outranks an unobserved one", () => {
    const observations = withObservation("sarah-livekit-failure", {
      key: "hold_exhaustion_bounded",
      finding: "contradicted",
      observedAtMs: now(),
      evidence: {
        kind: "drill",
        faultInjected: "exhaust the reservation hold mid-turn",
        boundedWithinMs: 30_000,
        settlementState: "settled",
        settlementReceiptDigest: `sha256:${"d".repeat(64)}`,
      },
    });
    expect(record("sarah-livekit-failure", observations).outcome).toBe("observed_fail");
  });
});

describe("the room journey", () => {
  const threeDesktops = (audible: boolean) =>
    ["alpha", "bravo", "charlie"].map((seed) => ({
      participantRefDigest: digest(seed),
      clientKind: "packaged_omega_desktop" as const,
      packageSha256: binding.omegaPackageSha256,
      authenticated: true,
      audibleSarahOutputObserved: audible,
    }));

  test("records three authenticated packaged desktops", () => {
    const receipt = record(
      "sarah-livekit-room",
      withObservation("sarah-livekit-room", {
        key: "authenticated_desktop_count",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: { kind: "participants", participants: threeDesktops(true) },
      }),
    );
    expect(receipt.outcome).toBe("incomplete");
    expect(
      receipt.observations.find((entry) => entry.key === "authenticated_desktop_count")?.finding,
    ).toBe("satisfied");
  });

  test("refuses to call two desktops a three-desktop journey", () => {
    const observations = withObservation("sarah-livekit-room", {
      key: "authenticated_desktop_count",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: { kind: "participants", participants: threeDesktops(true).slice(0, 2) },
    });
    expect(() => record("sarah-livekit-room", observations)).toThrow(
      /at least three authenticated packaged desktops/u,
    );
  });

  test("refuses to count a headless subscriber as a packaged desktop", () => {
    const participants = [
      ...threeDesktops(true).slice(0, 2),
      {
        participantRefDigest: digest("headless"),
        clientKind: "headless_harness" as const,
        authenticated: true,
        audibleSarahOutputObserved: true,
      },
    ];
    const observations = withObservation("sarah-livekit-room", {
      key: "authenticated_desktop_count",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: { kind: "participants", participants },
    });
    expect(() => record("sarah-livekit-room", observations)).toThrow(
      /at least three authenticated packaged desktops/u,
    );
  });

  test("refuses a shared answer that not every participant heard", () => {
    const observations = withObservation("sarah-livekit-room", {
      key: "shared_answer_heard_by_all",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: {
        kind: "participants",
        participants: [
          ...threeDesktops(true).slice(0, 2),
          { ...threeDesktops(false)[2]!, audibleSarahOutputObserved: false },
        ],
      },
    });
    expect(() => record("sarah-livekit-room", observations)).toThrow(/every participant/u);
  });

  test("records a floor transfer between two distinct members", () => {
    const receipt = record(
      "sarah-livekit-room",
      withObservation("sarah-livekit-room", {
        key: "floor_transfer_completed",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "floor",
          state: "held",
          issuance: 4,
          fromHolderDigest: digest("alpha"),
          toHolderDigest: digest("bravo"),
        },
      }),
    );
    expect(
      receipt.observations.find((entry) => entry.key === "floor_transfer_completed")?.finding,
    ).toBe("satisfied");
  });

  test("refuses a transfer that never left the same member", () => {
    const observations = withObservation("sarah-livekit-room", {
      key: "floor_transfer_completed",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: {
        kind: "floor",
        state: "held",
        issuance: 4,
        fromHolderDigest: digest("alpha"),
        toHolderDigest: digest("alpha"),
      },
    });
    expect(() => record("sarah-livekit-room", observations)).toThrow(/two distinct members/u);
  });

  test("requires a moderator stop to actually be a moderator stop", () => {
    const observations = withObservation("sarah-livekit-room", {
      key: "moderator_stop_completed",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: { kind: "floor", state: "stopped", issuance: 5, stopReason: "timeout" },
    });
    expect(() => record("sarah-livekit-room", observations)).toThrow(/stopped by a moderator/u);
  });

  test("records the four named membership refusals with the server's own codes", () => {
    const refusals: Readonly<Record<string, string>> = {
      non_floor_refused: "not_floor_holder",
      removed_member_refused: "member_removed",
      stale_floor_grant_refused: "membership_changed",
      replayed_floor_grant_refused: "nonce_replayed",
    };
    const observations = allUnobserved("sarah-livekit-room").map((entry) =>
      refusals[entry.key] === undefined
        ? entry
        : ({
            key: entry.key,
            finding: "satisfied",
            observedAtMs: now(),
            evidence: {
              kind: "refusal",
              attempted: `claim the floor: ${entry.key}`,
              refusalCode: refusals[entry.key]!,
              httpStatus: 409,
            },
          } satisfies GateObservation),
    );
    const receipt = record("sarah-livekit-room", observations);
    expect(
      receipt.observations
        .filter((entry) => entry.finding === "satisfied")
        .map((entry) => entry.key),
    ).toEqual([
      "non_floor_refused",
      "removed_member_refused",
      "replayed_floor_grant_refused",
      "stale_floor_grant_refused",
    ]);
  });

  test("refuses a refusal recorded with a success status", () => {
    const observations = withObservation("sarah-livekit-room", {
      key: "non_floor_refused",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: {
        kind: "refusal",
        attempted: "claim the floor without holding it",
        refusalCode: "not_floor_holder",
        httpStatus: 200,
      },
    });
    expect(() => record("sarah-livekit-room", observations)).toThrow(/error status/u);
  });
});

describe("the connectivity matrix", () => {
  const icePath = (protocol: "udp" | "tcp" | "tls", relayed: boolean) => ({
    localCandidateType: relayed ? ("relay" as const) : ("host" as const),
    remoteCandidateType: "host" as const,
    protocol,
    relayed,
    packetsObserved: true as const,
  });

  const iceObservation = (
    key: string,
    protocol: "udp" | "tcp" | "tls",
    relayed: boolean,
    profile: "unrestricted" | "udp_blocked" | "udp_and_plaintext_tcp_blocked",
    observedKind: string,
  ): GateObservation => ({
    key,
    finding: "satisfied",
    observedAtMs: now(),
    evidence: {
      kind: "ice",
      forcedTransportProfile: profile,
      observedTransportKind: observedKind,
      publisher: icePath(protocol, relayed),
      subscriber: icePath(protocol, relayed),
      acceptanceResultDigest: `sha256:${"e".repeat(64)}`,
    },
  });

  test("records all three classified transport paths", () => {
    const receipt = record("sarah-livekit-connectivity", [
      iceObservation("direct_udp_completed", "udp", false, "unrestricted", "direct_udp"),
      iceObservation("tcp_fallback_completed", "tcp", false, "udp_blocked", "tcp_fallback"),
      iceObservation(
        "turn_tls_completed",
        "tls",
        true,
        "udp_and_plaintext_tcp_blocked",
        "turn_tls",
      ),
    ]);
    expect(receipt.outcome).toBe("observed_pass");
  });

  test("refuses a capture filed under the wrong matrix cell", () => {
    const observations = [
      iceObservation("direct_udp_completed", "udp", false, "unrestricted", "direct_udp"),
      // A direct UDP capture cannot be the TCP fallback row.
      iceObservation("tcp_fallback_completed", "udp", false, "udp_blocked", "direct_udp"),
      unobserved("turn_tls_completed"),
    ];
    expect(() => record("sarah-livekit-connectivity", observations)).toThrow(
      /classified as direct_udp/u,
    );
  });

  test("refuses an ICE path that carried no packets", () => {
    const observations = [
      {
        key: "direct_udp_completed",
        finding: "satisfied" as const,
        observedAtMs: now(),
        evidence: {
          kind: "ice" as const,
          forcedTransportProfile: "unrestricted" as const,
          observedTransportKind: "direct_udp",
          publisher: { ...icePath("udp", false), packetsObserved: false as unknown as true },
          subscriber: icePath("udp", false),
          acceptanceResultDigest: `sha256:${"e".repeat(64)}`,
        },
      },
      unobserved("tcp_fallback_completed"),
      unobserved("turn_tls_completed"),
    ];
    expect(() => record("sarah-livekit-connectivity", observations)).toThrow(/carried no packets/u);
  });
});

describe("isolation and the private journey", () => {
  test("records distinct concurrent provider generations", () => {
    const receipt = record(
      "sarah-livekit-isolation",
      withObservation("sarah-livekit-isolation", {
        key: "distinct_provider_generations",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "generation_comparison",
          leftComparableDigest: digest("private-generation"),
          rightComparableDigest: digest("community-generation"),
          distinct: true,
        },
      }),
    );
    expect(
      receipt.observations.find((entry) => entry.key === "distinct_provider_generations")?.finding,
    ).toBe("satisfied");
  });

  test("refuses a distinctness claim its own digests contradict", () => {
    const same = digest("one-generation");
    const observations = withObservation("sarah-livekit-isolation", {
      key: "distinct_provider_generations",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: {
        kind: "generation_comparison",
        leftComparableDigest: same,
        rightComparableDigest: same,
        distinct: true,
      },
    });
    expect(() => record("sarah-livekit-isolation", observations)).toThrow(
      /disagrees with its own digests/u,
    );
  });

  test("requires a reconnect to keep the same generation", () => {
    const observations = withObservation("sarah-livekit-private", {
      key: "reconnect_same_generation",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: {
        kind: "generation_comparison",
        leftComparableDigest: digest("before"),
        rightComparableDigest: digest("after"),
        distinct: true,
      },
    });
    expect(() => record("sarah-livekit-private", observations)).toThrow(/unchanged/u);
  });

  test("records admission terms displayed before capture", () => {
    const receipt = record(
      "sarah-livekit-private",
      withObservation("sarah-livekit-private", {
        key: "admission_terms_seen",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "admission_terms",
          priceMsat: 256_000,
          holdMsat: 256_000,
          rateMsatPerMillionTokens: 64_000_000,
          limitSeconds: 300,
          displayedAtMs: now() - 4_000,
          firstCaptureAtMs: now(),
        },
      }),
    );
    expect(
      receipt.observations.find((entry) => entry.key === "admission_terms_seen")?.finding,
    ).toBe("satisfied");
  });

  test("refuses admission terms shown only after capture began", () => {
    const observations = withObservation("sarah-livekit-private", {
      key: "admission_terms_seen",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: {
        kind: "admission_terms",
        priceMsat: 256_000,
        holdMsat: 256_000,
        rateMsatPerMillionTokens: 64_000_000,
        limitSeconds: 300,
        displayedAtMs: now(),
        firstCaptureAtMs: now() - 4_000,
      },
    });
    expect(() => record("sarah-livekit-private", observations)).toThrow(
      /not displayed before capture/u,
    );
  });

  test("keeps a source-code claim distinguishable from a live observation", () => {
    const receipt = record(
      "sarah-livekit-isolation",
      withObservation("sarah-livekit-isolation", {
        key: "capability_isolated",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "fact",
          fact: "community Sarah refused a privileged tool request in a live room",
          sourceKind: "live_client_observation",
        },
      }),
    );
    const observation = receipt.observations.find((entry) => entry.key === "capability_isolated");
    expect(observation?.evidence).toMatchObject({ sourceKind: "live_client_observation" });
  });
});

describe("failure drills", () => {
  test("requires eight clean scopes for the privacy scan", () => {
    const scopes = Array.from({ length: 7 }, (_unused, index) => `scope-${index}`);
    const observations = withObservation("sarah-livekit-failure", {
      key: "privacy_scope_count",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: { kind: "scan", scopes, residueFound: false, sameWindowComplete: true },
    });
    expect(() => record("sarah-livekit-failure", observations)).toThrow(/at least eight/u);
  });

  test("refuses a scan that found residue", () => {
    const scopes = Array.from({ length: 8 }, (_unused, index) => `scope-${index}`);
    const observations = withObservation("sarah-livekit-failure", {
      key: "privacy_scope_count",
      finding: "satisfied",
      observedAtMs: now(),
      evidence: { kind: "scan", scopes, residueFound: true, sameWindowComplete: true },
    });
    expect(() => record("sarah-livekit-failure", observations)).toThrow(/eight clean scopes/u);
  });

  test("records an executed drill with its deterministic settlement", () => {
    const receipt = record(
      "sarah-livekit-failure",
      withObservation("sarah-livekit-failure", {
        key: "worker_crash_bounded",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "drill",
          faultInjected: "terminate the Sarah worker mid-turn",
          boundedWithinMs: 12_000,
          settlementState: "settled",
          settlementReceiptDigest: `sha256:${"f".repeat(64)}`,
        },
      }),
    );
    expect(receipt.outcome).toBe("incomplete");
    expect(
      receipt.observations.find((entry) => entry.key === "worker_crash_bounded")?.finding,
    ).toBe("satisfied");
  });

  test("holds the SFU-loss drill to the failure matrix's fault and bound", () => {
    const sfuLoss = (overrides: Readonly<Record<string, unknown>>) =>
      withObservation("sarah-livekit-failure", {
        key: "sfu_loss_bounded",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "drill",
          faultInjected: "delete_exact_sfu_pod",
          boundedWithinMs: 4_200,
          settlementState: "settled",
          settlementReceiptDigest: `sha256:${"a".repeat(64)}`,
          ...overrides,
        },
      });

    const receipt = record("sarah-livekit-failure", sfuLoss({}));
    expect(receipt.observations.find((entry) => entry.key === "sfu_loss_bounded")?.finding).toBe(
      "satisfied",
    );

    // A drain is not a loss, and a node deletion is planned_worker_crash.
    expect(() =>
      record("sarah-livekit-failure", sfuLoss({ faultInjected: "drain the SFU pod" })),
    ).toThrow(/requires the delete_exact_sfu_pod fault/u);

    // Past the bound, the drill has only re-proved the session deadline.
    expect(() =>
      record(
        "sarah-livekit-failure",
        sfuLoss({ boundedWithinMs: SARAH_LIVEKIT_SFU_LOSS_BOUND_MS + 1 }),
      ),
    ).toThrow(`exceeded its ${SARAH_LIVEKIT_SFU_LOSS_BOUND_MS} ms bound`);

    // An exceeded bound is still a finding, so it must remain recordable.
    const contradicted = record(
      "sarah-livekit-failure",
      withObservation("sarah-livekit-failure", {
        key: "sfu_loss_bounded",
        finding: "contradicted",
        observedAtMs: now(),
        evidence: {
          kind: "drill",
          faultInjected: "delete_exact_sfu_pod",
          boundedWithinMs: 300_000,
          settlementState: "settled",
          settlementReceiptDigest: `sha256:${"b".repeat(64)}`,
        },
      }),
    );
    expect(contradicted.outcome).toBe("observed_fail");
  });
});

describe("binding and public safety", () => {
  test("refuses an unbound receipt", () => {
    expect(() =>
      recordSarahLiveKitGateObservation(
        {
          row: "sarah-livekit-room",
          binding: { ...binding, openagentsSourceRevision: "not-a-commit" },
          operatorRefDigest,
          observations: allUnobserved("sarah-livekit-room"),
        },
        dependencies,
      ),
    ).toThrow(/full lowercase Git commit/u);
  });

  test("refuses an operator identity that is not a digest", () => {
    expect(() =>
      recordSarahLiveKitGateObservation(
        {
          row: "sarah-livekit-room",
          binding,
          operatorRefDigest: "owner@example.com",
          observations: allUnobserved("sarah-livekit-room"),
        },
        dependencies,
      ),
    ).toThrow(/bare lowercase sha256 digest/u);
  });

  test("is content addressed and stable across observation ordering", () => {
    const observations = allUnobserved("sarah-livekit-failure");
    const forward = record("sarah-livekit-failure", observations);
    const reversed = record("sarah-livekit-failure", [...observations].reverse());
    expect(forward.resultDigest).toBe(reversed.resultDigest);
    expect(forward.receiptRef).toContain(forward.resultDigest.replace(":", "_"));
  });

  test("a different observation produces a different digest", () => {
    const base = record("sarah-livekit-failure", allUnobserved("sarah-livekit-failure"));
    const changed = record(
      "sarah-livekit-failure",
      withObservation("sarah-livekit-failure", {
        key: "media_key_rekey_proof",
        finding: "satisfied",
        observedAtMs: now(),
        evidence: {
          kind: "fact",
          fact: "a media key rekey was observed mid-session",
          sourceKind: "server_authority_response",
          refDigest: digest("rekey"),
        },
      }),
    );
    expect(changed.resultDigest).not.toBe(base.resultDigest);
  });

  test("rejects a receipt carrying an identity ref or a network address", () => {
    const receipt = record("sarah-livekit-room", allUnobserved("sarah-livekit-room"));
    expect(() =>
      assertPublicSafeGateObservationReceipt({
        ...receipt,
        observations: [
          ...receipt.observations.slice(1),
          {
            key: "floor_acquired",
            finding: "not_observed",
            reason: "the relay at 203.0.113.7 was unreachable",
            observedAtMs: now(),
          },
        ],
      }),
    ).toThrow(/private material/u);
  });

  test("rejects a receipt carrying a bearer token", () => {
    const receipt = record("sarah-livekit-room", allUnobserved("sarah-livekit-room"));
    expect(() =>
      assertPublicSafeGateObservationReceipt({
        ...receipt,
        operatorRefDigest: receipt.operatorRefDigest,
        note: `oa_omega_${"z".repeat(40)}`,
      }),
    ).toThrow(/private material/u);
  });

  test("never reports observed_pass for a row with an unobserved step", () => {
    for (const row of Object.keys(REQUIRED_OBSERVATIONS) as readonly GateRowId[]) {
      expect(record(row, allUnobserved(row)).outcome).not.toBe("observed_pass");
    }
  });
});

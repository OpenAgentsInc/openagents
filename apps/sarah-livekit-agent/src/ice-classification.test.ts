import { describe, expect, test } from "vite-plus/test";
import {
  classifySelectedIcePath,
  observedTransportKind,
  satisfiesForcedTransportProfile,
  type RtcStatsEntry,
  type SelectedIcePathClassification,
} from "./ice-classification.js";

const transport = (selectedCandidatePairId: string): RtcStatsEntry => ({
  stats: {
    case: "transport",
    value: { rtc: { id: "t1" }, transport: { selectedCandidatePairId } },
  },
});

const pair = (
  overrides: Partial<{
    id: string;
    localCandidateId: string;
    remoteCandidateId: string;
    nominated: boolean;
    packetsSent: bigint;
    packetsReceived: bigint;
  }> = {},
): RtcStatsEntry => ({
  stats: {
    case: "candidatePair",
    value: {
      rtc: { id: overrides.id ?? "p1" },
      candidatePair: {
        localCandidateId: overrides.localCandidateId ?? "lc1",
        remoteCandidateId: overrides.remoteCandidateId ?? "rc1",
        nominated: overrides.nominated ?? true,
        packetsSent: overrides.packetsSent ?? 120n,
        packetsReceived: overrides.packetsReceived ?? 96n,
      },
    },
  },
});

const candidate = (
  side: "localCandidate" | "remoteCandidate",
  id: string,
  candidateType: number | undefined,
  protocol: string | undefined,
  relayProtocol?: number,
): RtcStatsEntry => ({
  stats: {
    case: side,
    value: {
      rtc: { id },
      candidate: {
        ...(candidateType === undefined ? {} : { candidateType }),
        ...(protocol === undefined ? {} : { protocol }),
        ...(relayProtocol === undefined ? {} : { relayProtocol }),
      },
    },
  },
});

/** A direct host-to-host UDP pair, the unrestricted-network case. */
const directUdp: readonly RtcStatsEntry[] = [
  transport("p1"),
  pair(),
  candidate("localCandidate", "lc1", 0, "udp"),
  candidate("remoteCandidate", "rc1", 0, "udp"),
];

/** A server-reflexive pair over TCP, the UDP-blocked case. */
const tcpFallback: readonly RtcStatsEntry[] = [
  transport("p1"),
  pair(),
  candidate("localCandidate", "lc1", 1, "tcp"),
  candidate("remoteCandidate", "rc1", 1, "tcp"),
];

/** A TURN relay negotiated over TLS, the UDP-and-plaintext-TCP-blocked case. */
const turnTls: readonly RtcStatsEntry[] = [
  transport("p1"),
  pair(),
  candidate("localCandidate", "lc1", 3, "tcp", 2),
  candidate("remoteCandidate", "rc1", 0, "udp"),
];

describe("classifySelectedIcePath", () => {
  test("classifies a direct UDP path", () => {
    const result = classifySelectedIcePath(directUdp);
    expect(result).toEqual({
      classified: true,
      path: {
        localCandidateType: "host",
        remoteCandidateType: "host",
        protocol: "udp",
        relayed: false,
        packetsObserved: true,
      },
    });
  });

  test("classifies a TCP fallback path", () => {
    const result = classifySelectedIcePath(tcpFallback);
    expect(result.classified).toBe(true);
    expect(result.classified && result.path.protocol).toBe("tcp");
    expect(result.classified && result.path.relayed).toBe(false);
  });

  test("prefers the relay protocol so a TLS relay is not misread as plain TCP", () => {
    const result = classifySelectedIcePath(turnTls);
    expect(result.classified).toBe(true);
    expect(result.classified && result.path.protocol).toBe("tls");
    expect(result.classified && result.path.relayed).toBe(true);
    expect(result.classified && result.path.localCandidateType).toBe("relay");
  });

  test("treats a remote relay candidate as a relayed path", () => {
    const result = classifySelectedIcePath([
      transport("p1"),
      pair(),
      candidate("localCandidate", "lc1", 0, "udp"),
      candidate("remoteCandidate", "rc1", 3, "tcp", 2),
    ]);
    expect(result.classified && result.path.relayed).toBe(true);
  });

  test.each([
    [
      "no selected pair",
      [pair(), candidate("localCandidate", "lc1", 0, "udp")],
      "no_selected_candidate_pair",
    ],
    [
      "an unnominated pair",
      [transport("p1"), pair({ nominated: false })],
      "selected_pair_not_nominated",
    ],
    [
      "a pair that carried no packets",
      [transport("p1"), pair({ packetsSent: 0n, packetsReceived: 0n })],
      "no_packets_on_selected_pair",
    ],
    ["a missing candidate record", [transport("p1"), pair()], "selected_pair_candidates_missing"],
    [
      "an absent candidate type",
      [
        transport("p1"),
        pair(),
        candidate("localCandidate", "lc1", undefined, "udp"),
        candidate("remoteCandidate", "rc1", 0, "udp"),
      ],
      "candidate_type_absent",
    ],
    [
      "an unrecognized protocol",
      [
        transport("p1"),
        pair(),
        candidate("localCandidate", "lc1", 0, "sctp"),
        candidate("remoteCandidate", "rc1", 0, "sctp"),
      ],
      "candidate_protocol_unrecognized",
    ],
  ] as const)("fails closed on %s", (_label, entries, reason) => {
    const result = classifySelectedIcePath(entries as readonly RtcStatsEntry[]);
    expect(result).toEqual({ classified: false, reason });
  });

  test("never exposes an address, port, or URL even when the stats carry them", () => {
    const leaky: readonly RtcStatsEntry[] = [
      transport("p1"),
      pair(),
      // The wire message carries locating fields this module's type omits, so
      // they are attached through an unknown cast to prove they cannot escape.
      {
        stats: {
          case: "localCandidate",
          value: {
            rtc: { id: "lc1" },
            candidate: {
              candidateType: 0,
              protocol: "udp",
              address: "203.0.113.7",
              port: 51_820,
              url: "turn:turn.example",
            },
          },
        },
      } as unknown as RtcStatsEntry,
      candidate("remoteCandidate", "rc1", 0, "udp"),
    ];
    const result = classifySelectedIcePath(leaky);
    expect(result.classified).toBe(true);
    expect(JSON.stringify(result)).not.toContain("203.0.113.7");
    expect(JSON.stringify(result)).not.toContain("51820");
    expect(JSON.stringify(result)).not.toContain("turn:turn.example");
  });
});

describe("satisfiesForcedTransportProfile", () => {
  const path = (
    overrides: Partial<SelectedIcePathClassification>,
  ): SelectedIcePathClassification => ({
    localCandidateType: "host",
    remoteCandidateType: "host",
    protocol: "udp",
    relayed: false,
    packetsObserved: true,
    ...overrides,
  });

  test("admits anything when no constraint was imposed", () => {
    expect(satisfiesForcedTransportProfile("unrestricted", path({}))).toBe(true);
  });

  test("refuses a UDP capture that claims UDP was blocked", () => {
    expect(satisfiesForcedTransportProfile("udp_blocked", path({ protocol: "udp" }))).toBe(false);
  });

  test("admits TCP and TLS captures when UDP was blocked", () => {
    expect(satisfiesForcedTransportProfile("udp_blocked", path({ protocol: "tcp" }))).toBe(true);
    expect(
      satisfiesForcedTransportProfile("udp_blocked", path({ protocol: "tls", relayed: true })),
    ).toBe(true);
  });

  test("requires a TLS relay when UDP and plaintext TCP were blocked", () => {
    expect(
      satisfiesForcedTransportProfile(
        "udp_and_plaintext_tcp_blocked",
        path({ protocol: "tls", relayed: true }),
      ),
    ).toBe(true);
    expect(
      satisfiesForcedTransportProfile("udp_and_plaintext_tcp_blocked", path({ protocol: "tcp" })),
    ).toBe(false);
    expect(
      satisfiesForcedTransportProfile(
        "udp_and_plaintext_tcp_blocked",
        path({ protocol: "tls", relayed: false }),
      ),
    ).toBe(false);
  });
});

describe("observedTransportKind", () => {
  test.each([
    ["direct_udp", { protocol: "udp", relayed: false }],
    ["tcp_fallback", { protocol: "tcp", relayed: false }],
    ["turn_tls", { protocol: "tls", relayed: true }],
    ["relayed_udp", { protocol: "udp", relayed: true }],
  ] as const)("reports %s independently of any declaration", (expected, overrides) => {
    expect(
      observedTransportKind({
        localCandidateType: "host",
        remoteCandidateType: "host",
        packetsObserved: true,
        ...overrides,
      }),
    ).toBe(expected);
  });
});

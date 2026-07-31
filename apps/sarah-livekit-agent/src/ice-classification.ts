/**
 * Classification of the selected ICE candidate pair for one peer connection.
 *
 * The Sarah LiveKit connectivity release-gate row requires three forced
 * transport paths (direct UDP, WebRTC TCP, and TURN/TLS relay) to be observed
 * and classified. Before this module the harness recorded only the boolean
 * `selectedIcePathObserved`, which proves that some pair was nominated and
 * carried packets but never says which path it was, so even the one naturally
 * selected path could not be assigned to a row.
 *
 * Everything here is derived from `Room.getRtcStats()`. The LiveKit FFI stats
 * carry the candidate address, port, URL, and username fragment; those are
 * network-locating values and never leave this module. Only the candidate type,
 * the transport protocol, and the relay protocol are classified, and those three
 * are exactly what the gate row needs.
 */

/** ICE candidate type, mirroring `livekit.proto.IceCandidateType`. */
export const ICE_CANDIDATE_TYPES = ["host", "srflx", "prflx", "relay"] as const;
export type IceCandidateType = (typeof ICE_CANDIDATE_TYPES)[number];

/** Transport protocol carrying the nominated pair. */
export const ICE_TRANSPORT_PROTOCOLS = ["udp", "tcp", "tls"] as const;
export type IceTransportProtocol = (typeof ICE_TRANSPORT_PROTOCOLS)[number];

/**
 * The forced-transport matrix cell a capture was taken under.
 *
 * This is DECLARED by the operator who imposed the network constraint; the
 * harness cannot discover it. The declaration is then checked against the
 * observed classification, so a declaration that the observation contradicts
 * fails closed instead of being recorded as a passing row.
 */
export const FORCED_TRANSPORT_PROFILES = [
  /** No constraint imposed. Whatever ICE selects naturally. */
  "unrestricted",
  /** All UDP blocked, so media must fall back to WebRTC TCP or a TCP/TLS relay. */
  "udp_blocked",
  /** UDP and plaintext TCP blocked, so media must traverse a TLS TURN relay. */
  "udp_and_plaintext_tcp_blocked",
] as const;
export type ForcedTransportProfile = (typeof FORCED_TRANSPORT_PROFILES)[number];

/**
 * A classified selected candidate pair.
 *
 * `relayed` is derived rather than trusted from a single field: a pair is
 * relayed when either end is a `relay` candidate.
 */
export type SelectedIcePathClassification = Readonly<{
  localCandidateType: IceCandidateType;
  remoteCandidateType: IceCandidateType;
  protocol: IceTransportProtocol;
  relayed: boolean;
  packetsObserved: true;
}>;

/**
 * Why a selected pair could not be classified.
 *
 * Classification failure is never silently downgraded to an unclassified pass:
 * the connectivity row treats every one of these as "not observed".
 */
export type IceClassificationFailure =
  | "no_selected_candidate_pair"
  | "selected_pair_not_nominated"
  | "no_packets_on_selected_pair"
  | "selected_pair_candidates_missing"
  | "candidate_type_absent"
  | "candidate_protocol_unrecognized";

export type SelectedIcePathResult =
  | Readonly<{ classified: true; path: SelectedIcePathClassification }>
  | Readonly<{ classified: false; reason: IceClassificationFailure }>;

/**
 * The subset of the LiveKit FFI stats union this module reads.
 *
 * `address`, `port`, `url`, `relatedAddress`, and `usernameFragment` exist on
 * the wire message and are deliberately absent from this type so that no code
 * path can carry them into a receipt.
 */
export type RtcStatsEntry = Readonly<{
  stats?: Readonly<{
    case?: string;
    value?: Readonly<{
      rtc?: Readonly<{ id?: string }>;
      transport?: Readonly<{ selectedCandidatePairId?: string }>;
      candidatePair?: Readonly<{
        localCandidateId?: string;
        remoteCandidateId?: string;
        nominated?: boolean;
        packetsSent?: bigint;
        packetsReceived?: bigint;
      }>;
      candidate?: Readonly<{
        candidateType?: number;
        protocol?: string;
        relayProtocol?: number;
      }>;
    }>;
  }>;
}>;

/** `livekit.proto.IceCandidateType` ordinals. */
const CANDIDATE_TYPE_BY_ORDINAL: Readonly<Record<number, IceCandidateType>> = {
  0: "host",
  1: "srflx",
  2: "prflx",
  3: "relay",
};

/** `livekit.proto.IceServerTransportProtocol` ordinals. */
const RELAY_PROTOCOL_BY_ORDINAL: Readonly<Record<number, IceTransportProtocol>> = {
  0: "udp",
  1: "tcp",
  2: "tls",
};

const candidateType = (ordinal: number | undefined): IceCandidateType | undefined =>
  ordinal === undefined ? undefined : CANDIDATE_TYPE_BY_ORDINAL[ordinal];

/**
 * Resolve the protocol actually carrying the pair.
 *
 * For a relay candidate the pair's own `protocol` describes the leg between the
 * peer and the TURN server, which is the leg a transport block acts on, so
 * `relayProtocol` is authoritative when present. TURN/TLS is only distinguishable
 * this way: at the candidate-pair level a TLS relay still reports `tcp`.
 */
const transportProtocol = (
  candidate: Readonly<{ protocol?: string; relayProtocol?: number }> | undefined,
  type: IceCandidateType | undefined,
): IceTransportProtocol | undefined => {
  if (candidate === undefined) return undefined;
  if (type === "relay" && candidate.relayProtocol !== undefined) {
    return RELAY_PROTOCOL_BY_ORDINAL[candidate.relayProtocol];
  }
  const protocol = candidate.protocol?.trim().toLowerCase();
  if (protocol === "udp" || protocol === "tcp" || protocol === "tls") return protocol;
  return undefined;
};

const candidateById = (
  entries: readonly RtcStatsEntry[],
  cases: readonly string[],
  id: string | undefined,
): Readonly<{ candidateType?: number; protocol?: string; relayProtocol?: number }> | undefined => {
  if (id === undefined) return undefined;
  for (const entry of entries) {
    if (
      entry.stats?.case !== undefined &&
      cases.includes(entry.stats.case) &&
      entry.stats.value?.rtc?.id === id
    ) {
      return entry.stats.value.candidate;
    }
  }
  return undefined;
};

/**
 * Classify the selected candidate pair for one peer connection.
 *
 * Fails closed at every step: an unnominated pair, a pair with no packets, a
 * missing candidate record, an absent candidate type, and an unrecognized
 * protocol each produce a typed failure rather than a partial classification.
 */
export const classifySelectedIcePath = (
  entries: readonly RtcStatsEntry[],
): SelectedIcePathResult => {
  const selectedPairIds = new Set(
    entries.flatMap((entry) =>
      entry.stats?.case === "transport" &&
      entry.stats.value?.transport?.selectedCandidatePairId !== undefined
        ? [entry.stats.value.transport.selectedCandidatePairId]
        : [],
    ),
  );
  const selected = entries.find(
    (entry) =>
      entry.stats?.case === "candidatePair" &&
      entry.stats.value?.rtc?.id !== undefined &&
      selectedPairIds.has(entry.stats.value.rtc.id),
  );
  const pair = selected?.stats?.value?.candidatePair;
  if (pair === undefined) return { classified: false, reason: "no_selected_candidate_pair" };
  if (pair.nominated !== true) return { classified: false, reason: "selected_pair_not_nominated" };
  if ((pair.packetsSent ?? 0n) <= 0n && (pair.packetsReceived ?? 0n) <= 0n) {
    return { classified: false, reason: "no_packets_on_selected_pair" };
  }

  const local = candidateById(entries, ["localCandidate"], pair.localCandidateId);
  const remote = candidateById(entries, ["remoteCandidate"], pair.remoteCandidateId);
  if (local === undefined || remote === undefined) {
    return { classified: false, reason: "selected_pair_candidates_missing" };
  }

  const localType = candidateType(local.candidateType);
  const remoteType = candidateType(remote.candidateType);
  if (localType === undefined || remoteType === undefined) {
    return { classified: false, reason: "candidate_type_absent" };
  }

  const relayed = localType === "relay" || remoteType === "relay";
  const protocol =
    transportProtocol(local, localType) ??
    (relayed ? transportProtocol(remote, remoteType) : undefined);
  if (protocol === undefined) {
    return { classified: false, reason: "candidate_protocol_unrecognized" };
  }

  return {
    classified: true,
    path: {
      localCandidateType: localType,
      remoteCandidateType: remoteType,
      protocol,
      relayed,
      packetsObserved: true,
    },
  };
};

/**
 * Does an observed path satisfy the transport constraint the operator declared?
 *
 * This is what stops a mis-run matrix cell from being recorded as a pass. If the
 * operator declares that UDP was blocked but the capture rode direct UDP, the
 * block did not take effect and the capture is not evidence for that row.
 */
export const satisfiesForcedTransportProfile = (
  profile: ForcedTransportProfile,
  path: SelectedIcePathClassification,
): boolean => {
  if (profile === "unrestricted") return true;
  if (profile === "udp_blocked") {
    // A TLS or TCP relay also satisfies "no UDP", but a UDP relay leg does not.
    return path.protocol !== "udp";
  }
  return path.relayed && path.protocol === "tls";
};

/**
 * The forced-transport profile a classified path is direct evidence for.
 *
 * Used to report which matrix cell a capture actually filled, independently of
 * what the operator declared.
 */
export const observedTransportKind = (
  path: SelectedIcePathClassification,
): "direct_udp" | "tcp_fallback" | "turn_tls" | "relayed_udp" => {
  if (path.relayed && path.protocol === "tls") return "turn_tls";
  if (path.relayed && path.protocol === "udp") return "relayed_udp";
  if (path.protocol === "udp") return "direct_udp";
  return "tcp_fallback";
};

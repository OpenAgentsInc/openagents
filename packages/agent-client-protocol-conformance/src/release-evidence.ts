import type { AcpConformanceEvidenceRecord } from "@openagentsinc/agent-client-protocol/profiles";

import checkedReleaseMatrix from "../compatibility/release-matrix.json" with { type: "json" };

import { validateAcpReleaseMatrix } from "./release.ts";

export type AcpReleaseEvidencePeer = "grok" | "cursor";

export type AcpReleaseEvidenceCompilation =
  | Readonly<{
      _tag: "ReleaseEvidenceReady";
      openAgentsRevision: string;
      evidence: Readonly<
        Record<AcpReleaseEvidencePeer, ReadonlyArray<AcpConformanceEvidenceRecord>>
      >;
    }>
  | Readonly<{
      _tag: "ReleaseEvidenceUnavailable";
      diagnostics: ReadonlyArray<string>;
    }>;

type JsonObject = Record<string, unknown>;

const object = (value: unknown): JsonObject | undefined =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonObject)
    : undefined;

const string = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 ? value : undefined;

const scenarioPassedLive = (peer: JsonObject, id: string): boolean =>
  Array.isArray(peer.scenarios) &&
  peer.scenarios.some((value) => {
    const scenario = object(value);
    return scenario?.id === id && scenario.result === "live-pass";
  });

const fixtureSuites = (peer: AcpReleaseEvidencePeer): ReadonlyArray<string> =>
  peer === "grok" ? ["acp-wire-v1-conformance"] : ["acp-wire-v1-conformance", "cursor-t3-bde0a4c0"];

const featureSuites = (
  peer: AcpReleaseEvidencePeer,
  document: JsonObject,
): ReadonlyArray<string> => {
  if (peer === "grok") {
    return [
      ...(scenarioPassedLive(document, "permission-approval") &&
      scenarioPassedLive(document, "permission-refusal") &&
      scenarioPassedLive(document, "fs-terminal-enabled")
        ? ["grok-authority-reverse"]
        : []),
      ...(scenarioPassedLive(document, "grok-question-extensions")
        ? ["grok-question-extensions"]
        : []),
      ...(scenarioPassedLive(document, "auth-secondary") ? ["grok-api-key-auth"] : []),
    ];
  }
  return [
    ...(scenarioPassedLive(document, "permission-approval") &&
    scenarioPassedLive(document, "permission-refusal")
      ? ["cursor-authority-reverse"]
      : []),
    ...(scenarioPassedLive(document, "cursor-extensions-models")
      ? ["cursor-vendor-extensions", "cursor-model-discovery"]
      : []),
  ];
};

/**
 * Lowers the complete checked release matrix into the narrow evidence records
 * consumed by trusted peer admission. Matrix validation runs first and the
 * compiler never trusts a hand-authored claim label or partial scenario row.
 */
export const compileAcpReleaseEvidence = (
  value: unknown,
  options: Readonly<{ now?: Date; expectedOpenAgentsRevision?: string }> = {},
): AcpReleaseEvidenceCompilation => {
  const validation = validateAcpReleaseMatrix(value, {
    ...(options.now === undefined ? {} : { now: options.now }),
  });
  if (!validation.valid)
    return { _tag: "ReleaseEvidenceUnavailable", diagnostics: validation.errors };

  const matrix = object(value);
  const openAgents = object(matrix?.openAgents);
  const revision = string(openAgents?.revision);
  if (revision === undefined)
    return {
      _tag: "ReleaseEvidenceUnavailable",
      diagnostics: ["OpenAgents revision is unavailable after matrix validation"],
    };
  if (
    options.expectedOpenAgentsRevision !== undefined &&
    options.expectedOpenAgentsRevision !== revision
  )
    return {
      _tag: "ReleaseEvidenceUnavailable",
      diagnostics: ["checked release evidence does not match the expected OpenAgents revision"],
    };

  const testedPlatform = string(object(matrix?.platform)?.tested)?.split("-");
  const os = testedPlatform?.[0];
  const arch = testedPlatform?.slice(1).join("-");
  const recordedAt = string(matrix?.recordedAt);
  if (os === undefined || arch === undefined || arch.length === 0 || recordedAt === undefined)
    return {
      _tag: "ReleaseEvidenceUnavailable",
      diagnostics: ["checked release platform or timestamp is unavailable"],
    };

  const compiled: Record<AcpReleaseEvidencePeer, ReadonlyArray<AcpConformanceEvidenceRecord>> = {
    grok: [],
    cursor: [],
  };
  for (const rawPeer of Array.isArray(matrix?.peers) ? matrix.peers : []) {
    const peer = object(rawPeer);
    if (peer === undefined) continue;
    const peerName = peer?.peer;
    if ((peerName !== "grok" && peerName !== "cursor") || peer.releaseEligible !== true) continue;
    const binary = object(peer.binary);
    const negotiation = object(peer.negotiation);
    const identity = object(negotiation?.peerIdentity);
    const peerVersion = string(binary?.classifiedVersion) ?? string(identity?.version);
    const executableSha256 = string(binary?.sha256);
    if (peerVersion === undefined || executableSha256 === undefined) continue;
    const installationClosureSha256 = string(binary?.installationClosureSha256);
    const artifactRef =
      "packages/agent-client-protocol-conformance/compatibility/release-matrix.json";
    const liveRecord = (suiteId: string): AcpConformanceEvidenceRecord => ({
      suiteId,
      kind: "live",
      result: "pass",
      peerVersion,
      executableSha256,
      ...(peerName === "cursor" && installationClosureSha256 !== undefined
        ? { installationClosureSha256 }
        : {}),
      platform: { os, arch },
      recordedAt,
      artifactRef,
    });
    compiled[peerName] = Object.freeze([
      ...fixtureSuites(peerName).map(
        (suiteId): AcpConformanceEvidenceRecord => ({
          suiteId,
          kind: "fixture",
          result: "pass",
          peerVersion,
          recordedAt,
          artifactRef,
        }),
      ),
      liveRecord("acp-release-matrix-v1"),
      ...featureSuites(peerName, peer).map(liveRecord),
    ]);
  }

  if (compiled.grok.length === 0 || compiled.cursor.length === 0)
    return {
      _tag: "ReleaseEvidenceUnavailable",
      diagnostics: ["checked release matrix did not compile both eligible peers"],
    };
  return {
    _tag: "ReleaseEvidenceReady",
    openAgentsRevision: revision,
    evidence: Object.freeze(compiled),
  };
};

export const checkedAcpReleaseEvidence = (
  options: Readonly<{ now?: Date; expectedOpenAgentsRevision?: string }> = {},
): AcpReleaseEvidenceCompilation => compileAcpReleaseEvidence(checkedReleaseMatrix, options);

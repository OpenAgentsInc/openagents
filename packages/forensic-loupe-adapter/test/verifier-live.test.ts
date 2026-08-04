import { createHash } from "node:crypto";
import { cpSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { describe, expect, it } from "vite-plus/test";

import {
  evaluateLoupeVerificationReleaseGate,
  recordedLoupeControlPlane,
  runLoupeVerification,
  type LoupeControlPlaneTranscript,
  type LoupeVerificationRun,
  type LoupeVerificationSpec,
} from "../src/verifier.ts";

/**
 * Live acceptance evidence for openagents #9294 (OFR-007).
 *
 * `verifier.test.ts` runs the same driver against a simulated control plane and
 * is capped at `conformance_vector` for exactly that reason. This file is the
 * other half: the transport replays the recorded wire responses of three
 * admitted OpenAgents Cloud `live_gce` sandboxes, and the verifier re-derives
 * everything from them.
 *
 * That re-derivation is the load-bearing part. The transcript contains no plan,
 * no evidence receipt, no worker receipt and no verdict — only what the control
 * plane and the guests answered. If the verifier's derivation were wrong, or if
 * its ordering, provenance, receipt-resolution or control gates stopped
 * discriminating, this file would produce the wrong verdict rather than the
 * right one, and the falsifiers at the bottom would pass.
 */

const fixture = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/forensics/coldcard/${name}`, import.meta.url));

const TRANSCRIPT_FILE = "loupe-control-plane-transcript.v1.json.gz";
const transcriptBytes = readFileSync(fixture(TRANSCRIPT_FILE));
const transcriptJson = gunzipSync(transcriptBytes).toString("utf8");
const transcript = JSON.parse(transcriptJson) as LoupeControlPlaneTranscript;
const summary = JSON.parse(
  readFileSync(fixture("loupe-verification-live-run.v2.json"), "utf8"),
) as {
  readonly transcriptDigest: string;
  readonly recordedOriginRef: string;
  readonly spec: LoupeVerificationSpec;
  readonly result: Record<string, unknown>;
  readonly initialVerdict: Record<string, unknown>;
  readonly runs: ReadonlyArray<Record<string, unknown>>;
  readonly totalMeasuredCostMicrousd: number;
  readonly cloudResidue: number;
};

/**
 * The intent the live run was given. It is read from the fixture rather than
 * restated here, and replaying pins it: the recorded control plane matches on
 * request identity, so a spec that differs in any field the driver puts on the
 * wire produces requests the transcript has no answer for.
 */
const LIVE_RUN_SPEC = summary.spec;

/**
 * The durable ledger is copied into a temporary directory rather than written
 * in place, so a replay can never mutate the committed first verdict. The
 * verdict the run must re-derive is already in the checked-in file.
 */
const replay = async (
  mutate: (value: LoupeControlPlaneTranscript) => LoupeControlPlaneTranscript = (value) => value,
): Promise<LoupeVerificationRun> => {
  const ledgerDirectory = mkdtempSync(join(tmpdir(), "loupe-live-ledger-"));
  cpSync(fixture("loupe-first-verdict-ledger"), ledgerDirectory, { recursive: true });
  return await runLoupeVerification({
    spec: LIVE_RUN_SPEC,
    controlPlane: recordedLoupeControlPlane(mutate(transcript)),
    ledgerDirectory,
  });
};

/**
 * Rewrites one recorded response in place, so a falsifier can express itself
 * the only way an attacker could: by making the control plane answer
 * differently.
 */
const rewriteExchange = (
  match: (exchange: LoupeControlPlaneTranscript["exchanges"][number]) => boolean,
  rewrite: (body: Record<string, unknown>) => Record<string, unknown>,
) =>
  (value: LoupeControlPlaneTranscript): LoupeControlPlaneTranscript => ({
    ...value,
    exchanges: value.exchanges.map((exchange) =>
      match(exchange)
        ? { ...exchange, body: rewrite(exchange.body as Record<string, unknown>) }
        : exchange,
    ),
  });


describe("Loupe verification against admitted live_gce controls", () => {
  it("replays a transcript recorded from a live control plane, not a simulation", () => {
    expect(transcript.recordedOriginKind).toBe("live");
    expect(transcript.recordedOriginRef).toMatch(/^control-plane\.live\./);
    expect(`sha256:${createHash("sha256").update(transcriptJson.trimEnd()).digest("hex")}`).toBe(
      summary.transcriptDigest,
    );
    // The transcript carries wire traffic only. Nothing a verifier concludes is
    // in it: no plan, no evidence receipt, no verdict, no worker receipt record.
    expect(transcriptJson).not.toContain("openagents.loupe_verification_evidence");
    expect(transcriptJson).not.toContain("openagents.loupe_initial_verdict");
    expect(transcriptJson).not.toContain("openagents.loupe_admitted_worker_receipt");
    expect(transcriptJson).not.toContain("openagents.loupe_verification_plan");
  });

  it("recorded three admitted live_gce sandboxes on the pinned staging guest image", () => {
    // `recordedOriginRef` names `localhost:<port>` because the live run reached
    // the staging control node through an IAP tunnel. The hostname is not the
    // liveness evidence and should not be read as any. The liveness evidence is
    // below: every runtime receipt in this transcript was issued by a control
    // plane that provisioned `live_gce` workers on the pinned guest image, and
    // the verifier refuses a run whose workers are anything else.
    const creates = transcript.exchanges.filter(
      (exchange) => exchange.route === "runtime_operation" && exchange.action === "create",
    );
    expect(creates).toHaveLength(3);
    for (const exchange of creates) {
      const receipt = exchange.body as Record<string, unknown>;
      expect(receipt.providerKind).toBe("live_gce");
      expect(receipt.isolationClass).toBe("gce_vm");
      expect(receipt.imageDigest).toBe(LIVE_RUN_SPEC.worker.imageDigest);
      expect(receipt.profileDigest).toBe(LIVE_RUN_SPEC.worker.profileDigest);
      expect(receipt.readinessObserved).toBe(true);
    }
    const deletes = transcript.exchanges.filter(
      (exchange) => exchange.route === "runtime_operation" && exchange.action === "delete",
    );
    expect(deletes).toHaveLength(3);
    for (const exchange of deletes) {
      expect((exchange.body as Record<string, unknown>).cleanupObserved).toBe(true);
    }
  });

  it("re-derives a confirmation that reaches independent verification", async () => {
    const run = await replay();
    expect(run.result.evidenceProvenance).toBe("admitted_worker_run");
    expect(run.result.evidenceOriginRef).toBe(transcript.recordedOriginRef);
    expect(run.result.outcome).toBe("confirmed");
    expect(run.result.evidenceTier).toBe("independently_verified");
    expect(run.result.derivedVulnerableTestOutcome).toBe("failure");
    expect(run.result.derivedFixedTestOutcome).toBe("success");
    expect(run.result.vulnerableControlPassed).toBe(true);
    expect(run.result.fixedControlPassed).toBe(true);
    expect(run.result.admittedWorkerReceiptsResolved).toBe(true);
    expect(run.result.initialVerdictAuthority).toBe("durable_first_verdict_ledger");

    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.coldcard.live.v2",
      results: [run.result],
      evaluatedAt: run.completedAt,
    });
    expect(gate.productMode).toBe("independent_verification");
    expect(gate.blockerRefs).toEqual([]);
  });

  it("measured a vulnerable build that links the deterministic fallback and a fixed build that does not", async () => {
    const run = await replay();
    const roleRun = (role: string) => {
      const found = run.runs.find((entry) => entry.role === role);
      if (found === undefined) throw new Error(`live run missing ${role}`);
      return found;
    };
    const vulnerable = roleRun("control_vulnerable");
    const fixed = roleRun("control_fixed");
    expect(vulnerable.buildExitStatus).toBe(0);
    expect(fixed.buildExitStatus).toBe(0);
    // The two builds must not agree about which object provides rng_get. If
    // they ever do, the control pair proves nothing and this fails first.
    expect(vulnerable.detectorReport?.observedProviderRefs).not.toEqual(
      fixed.detectorReport?.observedProviderRefs,
    );
    expect(fixed.detectorReport?.observedProviderRefs).toEqual([
      "provider.object.boards/COLDCARD_MK4/rng.o",
    ]);
    expect(fixed.detectorReport?.observedForbiddenSymbolCount).toBe(0);
    expect(Number(vulnerable.detectorReport?.observedForbiddenSymbolCount)).toBeGreaterThan(0);
    expect(Number(fixed.detectorReport?.enumeratedSymbolCount)).toBeGreaterThan(1_000);
    expect(vulnerable.detectorExitStatus).toBe(1);
    expect(fixed.detectorExitStatus).toBe(0);
    // Three isolated sandboxes, and no Google Cloud residue after them.
    expect(new Set(run.runs.map((entry) => entry.sandboxRef)).size).toBe(3);
    expect(summary.cloudResidue).toBe(0);
  });

  it("locked the initial verdict before either control was observed", async () => {
    const run = await replay();
    const lockedAt = Date.parse(String(run.initialVerdict.lockedAt));
    expect(Number.isNaN(lockedAt)).toBe(false);
    for (const receipt of run.mechanicalEvidence) {
      expect(Date.parse(String(receipt.observedAt))).toBeLessThanOrEqual(lockedAt);
    }
    for (const receipt of run.controlEvidence) {
      expect(Date.parse(String(receipt.observedAt))).toBeGreaterThan(lockedAt);
    }
    // And the verdict the replay re-derives is the one the live run durably
    // committed, byte for byte, in the checked-in ledger.
    const ledgerFile = fixture(
      `loupe-first-verdict-ledger/${createHash("sha256")
        .update(LIVE_RUN_SPEC.verificationRef)
        .digest("hex")}.v1.json`,
    );
    const stored = JSON.parse(readFileSync(ledgerFile, "utf8")) as {
      readonly verdict: Record<string, unknown>;
    };
    expect(stored.verdict).toEqual(run.initialVerdict);
  });

  // -------------------------------------------------------------------------
  // Falsifiers. Every one of them perturbs the CONTROL PLANE, because that is
  // the only surface a caller has left.
  // -------------------------------------------------------------------------

  it("refuses when the control plane claims a provider kind that is not live_gce", async () => {
    await expect(
      replay(
        rewriteExchange(
          (exchange) => exchange.route === "runtime_operation" && exchange.action === "create",
          (body) => ({ ...body, providerKind: "fake_local" }),
        ),
      ),
    ).rejects.toThrow("requires live_gce workers");
  });

  it("refuses when the control plane admitted a different guest image than requested", async () => {
    await expect(
      replay(
        rewriteExchange(
          (exchange) => exchange.route === "runtime_operation" && exchange.action === "create",
          (body) => ({ ...body, imageDigest: `sha256:${"a".repeat(64)}` }),
        ),
      ),
    ).rejects.toThrow("different guest image");
  });

  it("refuses a transcript whose exchanges were reordered", async () => {
    await expect(
      replay((value) => ({
        ...value,
        exchanges: [value.exchanges[1]!, value.exchanges[0]!, ...value.exchanges.slice(2)],
      })),
    ).rejects.toThrow(/holds no response|expected a/);
  });

  it("refuses a transcript whose recorded request no longer matches what the verifier asks", async () => {
    // Order alone is not the check. A transcript entry that kept its position
    // and its route but was recorded against a different request must not be
    // able to answer this one.
    await expect(
      replay((value) => ({
        ...value,
        exchanges: value.exchanges.map((exchange, index) =>
          index === 0
            ? { ...exchange, stableRequestDigest: `sha256:${"e".repeat(64)}` }
            : exchange,
        ),
      })),
    ).rejects.toThrow("holds no response for this");
  });

  it("refuses a transcript whose recorded clock was rewritten", async () => {
    // Moving the clock is how you would try to reorder a run without touching
    // its responses. It does not work, and it fails earlier than the ordering
    // gate: the driver puts its clock readings on the wire, so a rewritten
    // clock produces requests this transcript has no answer for.
    await expect(
      replay((value) => ({
        ...value,
        clockReadings: value.clockReadings.map((reading) =>
          new Date(Date.parse(reading) - 3_600_000).toISOString().replace(/\.(\d{3})\d*Z$/, ".$1Z"),
        ),
      })),
    ).rejects.toThrow("holds no response for this");
  });

  it("refuses a transcript whose recorded origin was not live", async () => {
    expect(() =>
      recordedLoupeControlPlane({ ...transcript, recordedOriginKind: "conformance" }),
    ).toThrow("recorded from a live origin");
  });

  it("refuses a second, different verdict once the ledger holds one", async () => {
    // The vulnerable build now reports the approved provider, so the mechanical
    // tier reaches `dismissed` instead of `confirmed`. The durable ledger
    // already holds the confirmation, and the run refuses rather than relocking.
    await expect(
      replay(
        rewriteExchange(
          (exchange) => exchange.route === "guest_io" && exchange.action === "read_artifact",
          (body) => {
            const content = body.contentBase64;
            if (typeof content !== "string") return body;
            const decoded = JSON.parse(Buffer.from(content, "base64").toString("utf8")) as Record<
              string,
              unknown
            >;
            if (decoded.schema !== "openagents.artifact_witness_capture.v2") return body;
            return {
              ...body,
              contentBase64: Buffer.from(
                JSON.stringify({
                  ...decoded,
                  symbolProviders: [
                    {
                      symbolName: LIVE_RUN_SPEC.finding.symbolName,
                      providerRef: LIVE_RUN_SPEC.finding.approvedProviderRef,
                      linkMapArtifactRef: "artifact.rewritten",
                    },
                  ],
                }),
                "utf8",
              ).toString("base64"),
            };
          },
        ),
      ),
    ).rejects.toThrow("already durably locked");
  });
});

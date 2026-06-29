import { describe, expect, test } from "vitest"

import {
  checkR2ClipAvailability,
  checkRenderQueueHealth,
  checkSourceLag,
  checkTimelineFreshness,
  parseArgs,
  runVisibilityFreshnessSmoke,
  summarizeReport,
} from "./visibility-freshness-smoke.mjs"

const jsonResponse = (body: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(body), {
    headers: { "content-type": "application/json" },
    status: 200,
    ...init,
  })

const textResponse = (
  body: string,
  init: ResponseInit & { contentType?: string } = {},
) =>
  new Response(body, {
    headers: { "content-type": init.contentType ?? "text/plain" },
    status: init.status ?? 200,
  })

const timeline = {
  events: [{ cursor: "2026-06-19T00:00:00.000Z:pylon_api:event.1" }],
  generatedAt: "2026-06-19T00:00:00.000Z",
  schemaVersion: "openagents.public_activity_timeline.v1",
  sourceLag: [
    {
      caveatRefs: [],
      lagSeconds: 10,
      maxStalenessSeconds: 300,
      observedAt: "2026-06-19T00:00:00.000Z",
      sourceKind: "pylon_presence",
      sourceRefs: ["route:/api/public/pylon-stats"],
      status: "current",
    },
  ],
  staleness: {
    composition: "live_at_read",
    contractVersion: "projection_staleness.v1",
    maxStalenessSeconds: 0,
    rebuildsOn: ["public_activity_timeline_read"],
  },
}

describe("visibility-freshness-smoke (#5435)", () => {
  test("parses warn-only and repeated R2 manifest URLs", () => {
    expect(
      parseArgs([
        "--warn-only",
        "--source-lag-mode",
        "warn",
        "--r2-manifest-url",
        "https://clips.openagents.com/a.json",
        "--r2-manifest-url",
        "https://clips.openagents.com/b.json",
      ]),
    ).toMatchObject({
      r2ManifestUrls: [
        "https://clips.openagents.com/a.json",
        "https://clips.openagents.com/b.json",
      ],
      sourceLagMode: "warn",
      warnOnly: true,
    })
  })

  test("checks timeline schema, generatedAt freshness, and staleness contract", () => {
    const checks = checkTimelineFreshness(timeline, {
      maxGeneratedAgeSeconds: 120,
      now: new Date("2026-06-19T00:00:30.000Z"),
      url: "https://openagents.com/api/public/activity-timeline",
    })
    expect(checks.every(check => check.passed)).toBe(true)

    const stale = checkTimelineFreshness(
      { ...timeline, generatedAt: "2026-06-18T23:00:00.000Z" },
      {
        maxGeneratedAgeSeconds: 120,
        now: new Date("2026-06-19T00:00:30.000Z"),
      },
    )
    expect(stale.find(check => check.name === "timeline_generated_at_fresh"))
      .toMatchObject({ passed: false, severity: "error" })
  })

  test("identifies stale source lag rows with the source refs", () => {
    const checks = checkSourceLag(
      {
        sourceLag: [
          {
            caveatRefs: [
              "caveat.public.activity_timeline.source_lag_exceeds_contract",
            ],
            lagSeconds: 7490,
            maxStalenessSeconds: 300,
            sourceKind: "pylon_api",
            sourceRefs: ["route:/api/public/pylon-stats"],
            status: "stale",
          },
        ],
      },
      { mode: "fail" },
    )

    expect(checks).toEqual([
      expect.objectContaining({
        details: expect.objectContaining({
          sourceKind: "pylon_api",
          sourceRefs: ["route:/api/public/pylon-stats"],
        }),
        name: "source_lag_stale:pylon_api",
        passed: false,
        severity: "error",
      }),
    ])
  })

  test("detects stale active replay clip jobs", () => {
    const checks = checkRenderQueueHealth(
      {
        jobs: [
          {
            jobRef: "replay_clip_job.old",
            status: "queued",
            updatedAt: "2026-06-19T00:00:00.000Z",
          },
        ],
        staleness: {
          contractVersion: "projection_staleness.v1",
        },
      },
      {
        maxRenderQueueAgeSeconds: 60,
        now: new Date("2026-06-19T00:10:00.000Z"),
      },
    )

    expect(checks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: "render_queue_job_stale:replay_clip_job.old",
          passed: false,
        }),
      ]),
    )
  })

  test("checks R2 clip manifests and artifact URLs", async () => {
    const fetchImpl = async (url: string, init?: RequestInit) => {
      if (url.endsWith("manifest.json")) {
        return jsonResponse({
          artifacts: [
            {
              storageUrl: "https://clips.openagents.com/clip.mp4",
            },
          ],
        })
      }
      expect(init?.method).toBe("HEAD")
      return new Response("", { status: 200 })
    }

    const checks = await checkR2ClipAvailability({
      fetchImpl,
      manifestUrls: ["https://clips.openagents.com/manifest.json"],
      renderQueueBody: null,
      timeoutMs: 500,
    })

    expect(checks).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          name: "r2_clip_manifest_available",
          passed: true,
        }),
        expect.objectContaining({
          name: "r2_clip_artifact_available",
          passed: true,
        }),
      ]),
    )
  })

  test("summarizes failures separately from warnings", () => {
    expect(
      summarizeReport([
        { name: "ok", passed: true, severity: "info", details: {} },
        { name: "warn", passed: false, severity: "warning", details: {} },
        { name: "fail", passed: false, severity: "error", details: {} },
      ]),
    ).toMatchObject({
      errorCount: 1,
      status: "failed",
      warningCount: 1,
    })
  })

  test("runs the full smoke against fake public endpoints", async () => {
    const fetchImpl = async (url: string, init?: RequestInit) => {
      const parsed = new URL(url)
      if (parsed.pathname === "/api/public/activity-timeline/stream") {
        return textResponse(
          "event: activity_timeline_meta\ndata: {}\n\n",
          { contentType: "text/event-stream; charset=utf-8" },
        )
      }
      if (parsed.pathname === "/api/public/replay-clips") {
        return jsonResponse({
          jobs: [],
          staleness: { contractVersion: "projection_staleness.v1" },
        })
      }
      if (url.endsWith("manifest.json")) {
        return jsonResponse({
          artifacts: [{ storageUrl: "https://clips.openagents.test/clip.mp4" }],
        })
      }
      if (parsed.hostname === "clips.openagents.test") {
        return new Response("", { status: 200 })
      }
      if (init?.headers && parsed.pathname.includes("proof-replays")) {
        return jsonResponse({ bundleRef: "proof_replay_bundle.test" })
      }
      return jsonResponse(timeline)
    }

    const report = await runVisibilityFreshnessSmoke({
      baseUrl: "https://openagents.test",
      fetchImpl,
      now: new Date("2026-06-19T00:00:30.000Z"),
      r2ManifestUrls: ["https://clips.openagents.test/manifest.json"],
    })

    expect(report.summary.status).toBe("passed")
    expect(report.checks.map(check => check.name)).toContain("r2_clip_manifest_available")
  })
})

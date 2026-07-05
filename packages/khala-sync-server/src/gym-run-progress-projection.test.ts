import { publicScope } from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import {
  GYM_RUN_PROGRESS_PROJECTION_SYSTEM_REF,
  gymRunProgressPostImage,
  gymRunProgressPublicScope,
  projectGymRunProgressBestEffort,
  type RawGymRunProgressProjection,
} from "./gym-run-progress-projection.js"
import { runMigrations } from "./migrate.js"
import type { SyncSql } from "./sql.js"
import type { LocalPostgres } from "./test/local-postgres.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const webAuthorized = (
  runRef: string,
  overrides: Partial<
    Extract<RawGymRunProgressProjection, { publication: "web_authorized" }>
  > = {},
): RawGymRunProgressProjection => ({
  agent: "opencode",
  blockerRefs: [],
  caveatRefs: [],
  completionFraction: 0.1685,
  configId: `config.${runRef}`,
  counts: {
    cancelled: 0,
    completed: 13,
    completedFailed: 0,
    completedPassed: 13,
    error: 0,
    officialDenominator: 89,
    pending: 74,
    running: 2,
  },
  decisionGrade: false,
  elapsedMs: 540_000,
  inProgress: true,
  jobRef: `job.${runRef}`,
  lastUpdatedAt: "2026-07-04T15:20:11.412Z",
  passRateOverCompleted: 1,
  phase: "running",
  profile: {
    attribution: "Z.ai GLM-5.2 (REAP-504B)",
    contextWindowTokens: 65_536,
    hardwareProfile: "hydralisk-g4-4x-rtx-pro-6000",
    model: "openagents/glm-5.2-reap-504b",
    profileRef: "khala-public-heuristic",
    publicLabel: "Khala public heuristic",
  },
  publication: "web_authorized",
  runRef,
  tokens: {
    completionTokens: null,
    promptTokens: null,
    totalTokens: null,
  },
  ...overrides,
})

const localOnly = (runRef: string): RawGymRunProgressProjection => ({
  blockerRefs: ["blocker.gym.run_progress.not_authorized_for_web_publication"],
  decisionGrade: false,
  inProgress: true,
  lastUpdatedAt: "2026-07-04T15:20:11.412Z",
  publication: "local_only",
  runRef,
})

// ---------------------------------------------------------------------------
// Mapping + redaction (pure, no Postgres)
// ---------------------------------------------------------------------------

describe("gymRunProgressPostImage", () => {
  test("maps a web_authorized projection into the contract entity", () => {
    const entity = gymRunProgressPostImage(webAuthorized("run.web.alpha"))
    expect(entity.runRef).toBe("run.web.alpha")
    expect(entity.publication).toBe("web_authorized")
  })

  test("maps a local_only projection with NO live counts", () => {
    const entity = gymRunProgressPostImage(localOnly("run.local.beta"))
    expect(entity.runRef).toBe("run.local.beta")
    expect(entity.publication).toBe("local_only")
    expect("counts" in entity).toBe(false)
  })
})

describe("gymRunProgressPublicScope", () => {
  test("is the shared scope.public.gym-run-progress channel", () => {
    expect(String(gymRunProgressPublicScope())).toBe(
      String(publicScope("gym-run-progress")),
    )
  })
})

// ---------------------------------------------------------------------------
// Fail-soft wrapper (no working database: must return a diagnostic)
// ---------------------------------------------------------------------------

describe("projectGymRunProgressBestEffort fail-soft", () => {
  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused: postgres://user:secret@10.0.0.1")
      },
    } as unknown as SyncSql
    const outcome = await projectGymRunProgressBestEffort(
      broken,
      webAuthorized("run.broken.alpha"),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      // A plain thrown Error (not Bun-SQL-error-shaped) does not map to a
      // typed KhalaSyncStorageError, so it falls into the generic bucket —
      // same behavior as the public-counter projection's own broken-handle
      // test. Either way, nothing is ever echoed.
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("a forbidden-material post-image refuses without touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    // The ref-typed fields (runRef/jobRef/model/...) structurally refuse a
    // URL at DECODE time — the contract's first line of defense. To exercise
    // the SECOND line (the regex guard, defense in depth), leak through a
    // free-text label field instead (`attribution`, bounded only by length),
    // which decodes fine but must still trip the forbidden-material scan.
    const outcome = await projectGymRunProgressBestEffort(
      neverCalled,
      webAuthorized("run.leak.alpha", {
        profile: {
          attribution: "https://leaked.example.internal/secret",
          contextWindowTokens: 65_536,
          hardwareProfile: "hydralisk-g4-4x-rtx-pro-6000",
          model: "openagents/glm-5.2-reap-504b",
          profileRef: "khala-public-heuristic",
          publicLabel: "Khala public heuristic",
        },
      }),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("redaction_refused")
    }
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "gym run-progress projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_gym_run_progress")
      await admin.end()
      const url = pg.urlFor("khala_sync_gym_run_progress")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("projects a web_authorized snapshot into scope.public.gym-run-progress", async () => {
      const outcome = await projectGymRunProgressBestEffort(
        s(),
        webAuthorized("run.web.alpha"),
      )
      expect(outcome.ok).toBe(true)
      if (!outcome.ok) return
      expect(String(outcome.entry.scope)).toBe(
        String(gymRunProgressPublicScope()),
      )
      expect(String(outcome.entry.entityType)).toBe("gym_run_progress")
      expect(String(outcome.entry.entityId)).toBe("run.web.alpha")
      expect(outcome.entry.mutationRef).toBe(
        GYM_RUN_PROGRESS_PROJECTION_SYSTEM_REF,
      )
      const postImage = JSON.parse(outcome.entry.postImageJson ?? "{}") as {
        runRef?: string
        publication?: string
        counts?: { completedPassed?: number }
      }
      expect(postImage.runRef).toBe("run.web.alpha")
      expect(postImage.publication).toBe("web_authorized")
      expect(postImage.counts?.completedPassed).toBe(13)
    })

    test("re-projecting the same runRef upserts (one row per entity, versions advance)", async () => {
      await projectGymRunProgressBestEffort(s(), webAuthorized("run.web.beta"))
      const second = await projectGymRunProgressBestEffort(
        s(),
        webAuthorized("run.web.beta", {
          counts: {
            cancelled: 0,
            completed: 20,
            completedFailed: 0,
            completedPassed: 20,
            error: 0,
            officialDenominator: 89,
            pending: 67,
            running: 2,
          },
        }),
      )
      expect(second.ok).toBe(true)
      if (!second.ok) return
      const rows: Array<{ version: string | number; entity_id: string }> =
        await sql`
          SELECT version, entity_id FROM khala_sync_changelog
           WHERE scope = ${String(gymRunProgressPublicScope())}
             AND entity_id = ${"run.web.beta"}
           ORDER BY version
        `
      // Two appends, one per publish — both keyed by the same entity id, so
      // a bootstrap/log reader sees the latest post-image for this run.
      expect(rows.length).toBe(2)
      expect(rows.every((row) => row.entity_id === "run.web.beta")).toBe(true)
    })

    test("multiple runs share ONE scope, each its own entity id", async () => {
      await projectGymRunProgressBestEffort(s(), webAuthorized("run.multi.a"))
      await projectGymRunProgressBestEffort(s(), webAuthorized("run.multi.b"))
      const rows: Array<{ entity_id: string }> = await sql`
        SELECT DISTINCT entity_id FROM khala_sync_changelog
         WHERE scope = ${String(gymRunProgressPublicScope())}
           AND entity_id IN (${"run.multi.a"}, ${"run.multi.b"})
      `
      expect(rows.map((row) => row.entity_id).sort()).toStrictEqual([
        "run.multi.a",
        "run.multi.b",
      ])
    })

    test("projects a local_only degraded snapshot with no counts", async () => {
      const outcome = await projectGymRunProgressBestEffort(
        s(),
        localOnly("run.local.gamma"),
      )
      expect(outcome.ok).toBe(true)
      if (!outcome.ok) return
      const postImage = JSON.parse(outcome.entry.postImageJson ?? "{}") as {
        publication?: string
        counts?: unknown
      }
      expect(postImage.publication).toBe("local_only")
      expect(postImage.counts).toBeUndefined()
    })
  },
)

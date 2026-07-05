import {
  canonicalJson,
  decodeKhalaCodeShareProjectionRecipientEntity,
  decodeKhalaCodeTeamChatMessageEntity,
  decodeKhalaCodeTeamEntity,
  decodeKhalaCodeTeamInviteEntity,
  decodeKhalaCodeTeamMembershipEntity,
  decodeKhalaCodeThreadMessageEntity,
  teamScope,
} from "@openagentsinc/khala-sync"
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
  KHALA_CODE_POST_IMAGE_FORBIDDEN_PATTERN,
  scopeChangesForKhalaCodeProductStateRow,
  scopeTombstonesForKhalaCodeProductStateRow,
} from "./khala-code-product-state-projection.js"
import {
  deleteKhalaCodeProductStateRows,
  KHALA_CODE_PRODUCT_STATE_TABLES,
  upsertKhalaCodeProductStateRows,
  type KhalaCodeProductStateRow,
  type KhalaCodeProductStateTable,
} from "./khala-code-product-state-tables.js"
import { runMigrations } from "./migrate.js"
import { bootstrap } from "./read-service.js"
import { withSyncTransaction } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const NOW = "2026-07-04T18:00:00.000Z"

// ---------------------------------------------------------------------------
// Representative full rows (D1 column shapes) per scope-projected table
// ---------------------------------------------------------------------------

const ROWS: Partial<Record<KhalaCodeProductStateTable, KhalaCodeProductStateRow>> =
  {
    prefilled_workspaces: {
      access_mode: "private_team",
      archived_at: null,
      created_at: NOW,
      first_claimed_at: null,
      first_run_at: null,
      first_viewed_at: null,
      holder_ref: "prospect:jane@example.com",
      holder_user_id: "user_2",
      id: "workspace_1",
      intro_receipt_json: '{"summary":"secret prospect notes"}',
      invited_at: null,
      last_viewed_at: null,
      private_project_id: "project_9",
      private_team_id: "team_2",
      project_name: "Acme Migration",
      revisit_count: 3,
      status: "active",
      updated_at: NOW,
    },
    share_projections: {
      audience_json: '["bob@example.com"]',
      canonical_url: "https://openagents.com/share/share_1",
      created_at: NOW,
      expires_at: null,
      id: "share_1",
      owner_user_id: "user_1",
      project_id: null,
      projection_json: '{"messages":["full payload"]}',
      projection_object_key: "shares/share_1.json",
      projection_version: 2,
      redaction_policy_id: "default",
      revoked_at: null,
      source_id: "thread_1",
      source_kind: "team-thread",
      status: "active",
      summary: "A shared thread",
      team_id: "team_3",
      title: "Shared thread",
      updated_at: NOW,
    },
    share_projection_recipients: {
      created_at: NOW,
      // display_name carries the invitee's own name / an email-shaped label —
      // it must NEVER reach the projected post-image.
      display_name: "Jane Doe <jane@example.com>",
      share_id: "share_1",
      subject_id: "team_5",
      subject_kind: "team",
    },
    team_chat_messages: {
      agent_run_id: "run_5",
      archived_at: null,
      author_avatar_url: "https://avatars.example.com/user_1.png",
      author_github_username: "sarahchen",
      author_name: "Sarah Chen",
      author_user_id: "user_1",
      autopilot_thread_id: "thread_1",
      body: "hello — email me at chris@openagents.com about /Users/chris/repo",
      created_at: NOW,
      deleted_at: null,
      id: "msg_1",
      kind: "message",
      metadata_json: '{"local_path":"/Users/chris/secret"}',
      project_id: null,
      team_id: "team_1",
      updated_at: NOW,
    },
    team_memberships: {
      created_at: NOW,
      id: "membership_1",
      invited_by_user_id: null,
      joined_at: NOW,
      removed_at: null,
      role: "member",
      status: "active",
      team_id: "team_1",
      updated_at: NOW,
      user_id: "user_1",
    },
    team_projects: {
      archived_at: null,
      created_at: NOW,
      description: "A project",
      id: "project_1",
      metadata_json: '{"api_key":"sk-secret"}',
      name: "Project One",
      slug: "project-one",
      status: "active",
      team_id: "team_1",
      updated_at: NOW,
    },
    team_workspace_invites: {
      accepted_at: null,
      accepted_by_user_id: null,
      created_at: NOW,
      email_message_id: "email_1",
      expires_at: NOW,
      id: "invite_1",
      invited_by_actor_ref: "user:owner@example.com",
      invitee_email: "jane@example.com",
      invitee_email_normalized: "jane@example.com",
      last_sent_at: NOW,
      metadata_json: "{}",
      project_id: null,
      revoked_at: null,
      role: "member",
      send_count: 1,
      status: "pending",
      team_id: "team_1",
      token_hash: "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
      updated_at: NOW,
    },
    teams: {
      archived_at: null,
      created_at: NOW,
      credits: 424_242,
      id: "team_1",
      kind: "organization",
      logo_url: "https://cdn.example.com/logo.png",
      name: "Team One",
      owner_user_id: "user_1",
      plan: "pro",
      slug: "team-one",
      status: "active",
      updated_at: NOW,
    },
    thread_file_message_refs: {
      created_at: NOW,
      deleted_at: null,
      file_id: "file_1",
      id: "ref_1",
      message_id: "msg_9",
      reference_kind: "attachment",
      team_id: "team_1",
      thread_id: "thread_1",
      updated_at: NOW,
    },
    thread_files: {
      checksum_sha256:
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      content_type: "text/plain",
      created_at: NOW,
      deleted_at: null,
      download_enabled: 1,
      filename: "notes.txt",
      id: "file_1",
      metadata_json: "{}",
      object_key: "thread-files/team_1/file_1",
      owner_user_id: "user_1",
      scan_status: "passed",
      scope: "team",
      size_bytes: 2048,
      storage_provider: "r2",
      team_id: "team_1",
      thread_id: "thread_1",
      updated_at: NOW,
      upload_status: "uploaded",
    },
    thread_messages: {
      archived_at: null,
      author_id: "user_1",
      body_json: '{"role":"user","content":"hi"}',
      created_at: NOW,
      deleted_at: null,
      id: "tmsg_1",
      org_id: "org_1",
      thread_id: "thread_7",
      updated_at: NOW,
      version: 1,
    },
  }

/** D1 columns whose values must NEVER appear in any projected post-image. */
const FORBIDDEN_SOURCE_COLUMNS: ReadonlyArray<
  readonly [KhalaCodeProductStateTable, string]
> = [
  ["team_workspace_invites", "token_hash"],
  ["team_workspace_invites", "invitee_email"],
  ["team_workspace_invites", "invitee_email_normalized"],
  ["team_workspace_invites", "invited_by_actor_ref"],
  ["team_workspace_invites", "email_message_id"],
  ["thread_files", "object_key"],
  ["team_chat_messages", "metadata_json"],
  ["team_projects", "metadata_json"],
  ["teams", "credits"],
  ["teams", "logo_url"],
  ["prefilled_workspaces", "holder_ref"],
  ["prefilled_workspaces", "intro_receipt_json"],
  ["share_projections", "projection_json"],
  ["share_projections", "audience_json"],
  ["share_projections", "projection_object_key"],
  ["share_projections", "canonical_url"],
  ["share_projection_recipients", "display_name"],
]

describe("Khala Code product-state projection (typed post-images)", () => {
  test("team chat rows produce team and thread scope changes with contract post-images", () => {
    const changes = scopeChangesForKhalaCodeProductStateRow(
      "team_chat_messages",
      ROWS["team_chat_messages"]!,
    )

    expect(changes.map((change) => String(change.scope))).toEqual([
      "scope.team.team_1",
      "scope.thread.thread_1",
    ])
    expect(changes.map((change) => String(change.entityType))).toEqual([
      "team_chat_message",
      "team_chat_message",
    ])
    expect(changes.map((change) => String(change.entityId))).toEqual([
      "msg_1",
      "msg_1",
    ])
    const entity = decodeKhalaCodeTeamChatMessageEntity(changes[0]?.postImage)
    expect(entity.messageId).toBe("msg_1")
    expect(entity.teamId).toBe("team_1")
    expect(entity.autopilotThreadId).toBe("thread_1")
    // Content is the product for the authorized scope.
    expect(entity.body).toContain("hello")
    // KS-6.11 (#8422): denormalized author display-identity snapshot, joined
    // in by the Worker mirror's read-back (khala-code-product-state-store.ts)
    // and passed through here unchanged.
    expect(entity.authorName).toBe("Sarah Chen")
    expect(entity.authorAvatarUrl).toBe("https://avatars.example.com/user_1.png")
    expect(entity.authorGithubUsername).toBe("sarahchen")
  })

  // KS-6.11 (#8422): the generic backfill/verify sweep reads raw
  // `team_chat_messages` rows without the Worker mirror's author JOIN, so a
  // historical row genuinely may not carry these columns at all. The mapper
  // must fall back to `null`, not throw and skip the whole message.
  test("team chat rows without a joined author snapshot decode with null author-identity fields", () => {
    const rowWithoutAuthorJoin = {
      ...ROWS["team_chat_messages"],
      author_avatar_url: undefined,
      author_github_username: undefined,
      author_name: undefined,
    }

    const changes = scopeChangesForKhalaCodeProductStateRow(
      "team_chat_messages",
      rowWithoutAuthorJoin,
    )

    const entity = decodeKhalaCodeTeamChatMessageEntity(changes[0]?.postImage)
    expect(entity.authorUserId).toBe("user_1")
    expect(entity.authorName).toBeNull()
    expect(entity.authorAvatarUrl).toBeNull()
    expect(entity.authorGithubUsername).toBeNull()
  })

  test("membership, file, workspace, and share rows route to their sync scopes", () => {
    expect(
      scopeChangesForKhalaCodeProductStateRow(
        "team_memberships",
        ROWS["team_memberships"]!,
      ).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_1"])

    expect(
      scopeChangesForKhalaCodeProductStateRow(
        "thread_files",
        ROWS["thread_files"]!,
      ).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_1", "scope.thread.thread_1"])

    expect(
      scopeChangesForKhalaCodeProductStateRow(
        "prefilled_workspaces",
        ROWS["prefilled_workspaces"]!,
      ).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_2"])

    expect(
      scopeChangesForKhalaCodeProductStateRow(
        "share_projections",
        ROWS["share_projections"]!,
      ).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_3"])
  })

  test("share recipient rows fan out to the SUBJECT's own scope (email subjects never fan out)", () => {
    // team subject → the team's scope
    const teamRecipient = scopeChangesForKhalaCodeProductStateRow(
      "share_projection_recipients",
      ROWS["share_projection_recipients"]!,
    )
    expect(teamRecipient.map((c) => String(c.scope))).toEqual([
      "scope.team.team_5",
    ])
    expect(String(teamRecipient[0]?.entityType)).toBe(
      "share_projection_recipient",
    )
    expect(String(teamRecipient[0]?.entityId)).toBe("share_1:team:team_5")
    const recipient = decodeKhalaCodeShareProjectionRecipientEntity(
      teamRecipient[0]?.postImage,
    )
    expect(recipient.shareId).toBe("share_1")
    expect(recipient.subjectId).toBe("team_5")
    // display_name (email-shaped label) is structurally absent.
    expect(canonicalJson(teamRecipient[0]?.postImage)).not.toContain(
      "jane@example.com",
    )

    // user subject → the user's personal scope
    expect(
      scopeChangesForKhalaCodeProductStateRow("share_projection_recipients", {
        ...ROWS["share_projection_recipients"]!,
        subject_id: "user_9",
        subject_kind: "user",
      }).map((c) => String(c.scope)),
    ).toEqual(["scope.user.user_9"])

    // email subject → no sync scope, Postgres-mirror-only
    expect(
      scopeChangesForKhalaCodeProductStateRow("share_projection_recipients", {
        ...ROWS["share_projection_recipients"]!,
        subject_id: "jane@example.com",
        subject_kind: "email",
      }),
    ).toEqual([])
  })

  test("delete tombstones resolve scope/type/id without a post-image or the redaction mapper", () => {
    const tombstones = scopeTombstonesForKhalaCodeProductStateRow(
      "share_projection_recipients",
      ROWS["share_projection_recipients"]!,
    )
    expect(tombstones.map((t) => String(t.scope))).toEqual(["scope.team.team_5"])
    expect(String(tombstones[0]?.entityType)).toBe("share_projection_recipient")
    expect(String(tombstones[0]?.entityId)).toBe("share_1:team:team_5")
    expect(tombstones[0]).not.toHaveProperty("postImage")

    // A row that could NOT post-image-map (missing a non-key column) still
    // resolves a tombstone — the removal must replicate regardless.
    const partial = scopeTombstonesForKhalaCodeProductStateRow(
      "share_projection_recipients",
      { share_id: "share_1", subject_id: "team_5", subject_kind: "team" },
    )
    expect(partial.map((t) => String(t.scope))).toEqual(["scope.team.team_5"])

    // Scopeless (mirror-only) tables produce no tombstone, matching their
    // absent upsert fan-out.
    expect(
      scopeTombstonesForKhalaCodeProductStateRow("khala_feedback", {
        feedback_ref: "f_1",
      }),
    ).toEqual([])

    // email subjects (no sync scope) produce no tombstone.
    expect(
      scopeTombstonesForKhalaCodeProductStateRow("share_projection_recipients", {
        share_id: "share_1",
        subject_id: "jane@example.com",
        subject_kind: "email",
      }),
    ).toEqual([])
  })

  test("membership entityId is the natural key (membership-set equality rides it)", () => {
    const changes = scopeChangesForKhalaCodeProductStateRow(
      "team_memberships",
      ROWS["team_memberships"]!,
    )
    expect(String(changes[0]?.entityId)).toBe("team_1:user_1")
    const entity = decodeKhalaCodeTeamMembershipEntity(changes[0]?.postImage)
    expect(entity.role).toBe("member")
    expect(entity.status).toBe("active")
  })

  test("every scope-projected post-image decodes with its khala-sync contract", () => {
    const decoded: Record<string, unknown> = {}
    for (const [table, row] of Object.entries(ROWS) as Array<
      [KhalaCodeProductStateTable, KhalaCodeProductStateRow]
    >) {
      const changes = scopeChangesForKhalaCodeProductStateRow(table, row)
      expect(changes.length).toBeGreaterThan(0)
      decoded[table] = changes[0]?.postImage
    }
    expect(decodeKhalaCodeTeamEntity(decoded["teams"]).teamId).toBe("team_1")
    expect(
      decodeKhalaCodeThreadMessageEntity(decoded["thread_messages"]).threadId,
    ).toBe("thread_7")
    expect(
      decodeKhalaCodeTeamInviteEntity(decoded["team_workspace_invites"])
        .status,
    ).toBe("pending")
  })

  test("redaction property: forbidden D1 column values never reach a post-image", () => {
    for (const [table, column] of FORBIDDEN_SOURCE_COLUMNS) {
      const row = ROWS[table]!
      const value = row[column]
      expect(value).toBeDefined()
      const changes = scopeChangesForKhalaCodeProductStateRow(table, row)
      expect(changes.length).toBeGreaterThan(0)
      for (const change of changes) {
        const serialized = canonicalJson(change.postImage)
        expect(serialized).not.toContain(String(value))
      }
    }
  })

  test("redaction property: adversarial secrets in redacted columns are dropped; smuggled into structural columns they skip the projection", () => {
    // (a) Secrets in allowlist-excluded columns simply never appear —
    // regardless of shape (even ones that look like plain refs).
    for (const secret of [
      "sk-ant-api03-SECRETSECRETSECRET",
      "attacker@evil.example.com",
      "/Users/victim/.ssh/id_ed25519",
      "Bearer authorization-header-value",
    ]) {
      const invite = scopeChangesForKhalaCodeProductStateRow(
        "team_workspace_invites",
        { ...ROWS["team_workspace_invites"]!, token_hash: secret },
      )
      expect(invite.length).toBeGreaterThan(0)
      for (const change of invite) {
        expect(canonicalJson(change.postImage)).not.toContain(secret)
      }
    }

    // (b) An email/path/whitespace-shaped value smuggled into a STRUCTURAL
    // (ref) column cannot decode into the contract, so the change is
    // skipped fail-soft instead of leaking — and the skip reason never
    // echoes the value.
    for (const secret of [
      "attacker@evil.example.com",
      "/Users/victim/.ssh/id_ed25519",
      "Bearer authorization-header-value",
    ]) {
      const skips: Array<string> = []
      const smuggled = scopeChangesForKhalaCodeProductStateRow(
        "team_memberships",
        { ...ROWS["team_memberships"]!, user_id: secret },
        (skippedTable, reason) => skips.push(`${skippedTable}:${reason}`),
      )
      expect(smuggled).toEqual([])
      expect(skips.length).toBe(1)
      expect(skips[0]).not.toContain(secret)
    }
  })

  test("unmappable rows skip fail-soft with a bounded reason", () => {
    const skips: Array<string> = []
    const changes = scopeChangesForKhalaCodeProductStateRow(
      "teams",
      { id: "team_x", name: "No timestamps" },
      (table, reason) => skips.push(`${table}:${reason}`),
    )
    expect(changes).toEqual([])
    expect(skips.length).toBe(1)
    expect(skips[0]).toContain("teams:")
  })

  test("receipt/cloud/feedback tables stay Postgres-mirror-only (no scope fan-out)", () => {
    const scopeless: ReadonlyArray<KhalaCodeProductStateTable> = [
      "cloud_sandbox_sessions",
      "cloud_fine_tuning_jobs",
      "cloud_fine_tuned_models",
      "khala_feedback",
      "khala_head_to_head_snapshots",
      "khala_unsupported_requests",
      "khala_code_download_events",
      "khala_code_outside_user_run_receipts",
      "khala_code_trace_plugin_revenue_share_precedents",
      "workroom_kind_templates",
      "workroom_template_packages",
      "workroom_template_package_versions",
      "prefilled_workspace_seeded_memory",
      "prefilled_workspace_starter_workflows",
    ]
    for (const table of scopeless) {
      expect(KHALA_CODE_PRODUCT_STATE_TABLES).toContain(table)
      expect(
        scopeChangesForKhalaCodeProductStateRow(table, {
          receipt_ref: "r_1",
          gross_revenue_msats: 123n,
        }),
      ).toEqual([])
    }
  })

  test("forbidden pattern rejects secret-ish structural material", () => {
    for (const sample of [
      '{"tokenHash":"token_hash"}',
      '{"ref":"/Users/chris/repo"}',
      '{"contact":"someone@example.com"}',
    ]) {
      expect(KHALA_CODE_POST_IMAGE_FORBIDDEN_PATTERN.test(sample)).toBe(true)
    }
    expect(
      KHALA_CODE_POST_IMAGE_FORBIDDEN_PATTERN.test(
        '{"teamId":"team_1","status":"active"}',
      ),
    ).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Integration: mirror flow against local Postgres (row upsert + changelog)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "Khala Code product-state projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_code_product_state")
      await admin.end()
      const url = pg.urlFor("khala_code_product_state")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0017_khala_code_product_state.sql")
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    /** The same flow the Worker mirror runs after an accepted D1 write. */
    const mirrorRow = async (
      table: KhalaCodeProductStateTable,
      row: KhalaCodeProductStateRow,
    ) =>
      withSyncTransaction(sql as unknown as SyncSql, async (writer) => {
        await upsertKhalaCodeProductStateRows(writer.sql, table, [row])
        for (const change of scopeChangesForKhalaCodeProductStateRow(
          table,
          row,
        )) {
          await writer.appendChange({
            entityId: change.entityId,
            entityType: change.entityType,
            mutationRef: `d1-shadow:ks-8.13:${table}`,
            op: "upsert",
            postImage: change.postImage,
            scope: change.scope,
          })
        }
      })

    test("mirrored rows land in the twin AND append decodable scope changelog entries", async () => {
      await mirrorRow("teams", ROWS["teams"]!)
      await mirrorRow("team_memberships", ROWS["team_memberships"]!)
      await mirrorRow("team_chat_messages", ROWS["team_chat_messages"]!)
      await mirrorRow("thread_messages", ROWS["thread_messages"]!)

      const twin = await sql.unsafe(
        "SELECT id, name FROM teams WHERE id = 'team_1'",
      )
      expect(twin.length).toBe(1)

      const teamLog = await sql.unsafe(
        `SELECT scope, version, entity_type, entity_id, post_image_json
           FROM khala_sync_changelog
          WHERE scope = 'scope.team.team_1'
          ORDER BY version ASC`,
      )
      expect(teamLog.map((r: { entity_type: string }) => r.entity_type)).toEqual([
        "team",
        "team_membership",
        "team_chat_message",
      ])
      // Dense per-scope versions from day one (SPEC §2.3).
      expect(
        teamLog.map((r: { version: string | number }) => Number(r.version)),
      ).toEqual([1, 2, 3])

      const parse = (raw: string | object): unknown =>
        typeof raw === "string" ? JSON.parse(raw) : raw
      const team = decodeKhalaCodeTeamEntity(
        parse(teamLog[0].post_image_json as string),
      )
      expect(team.teamId).toBe("team_1")
      const membership = decodeKhalaCodeTeamMembershipEntity(
        parse(teamLog[1].post_image_json as string),
      )
      expect(membership.userId).toBe("user_1")

      const threadLog = await sql.unsafe(
        `SELECT scope, entity_type, post_image_json
           FROM khala_sync_changelog
          WHERE scope IN ('scope.thread.thread_1', 'scope.thread.thread_7')
          ORDER BY scope, version`,
      )
      expect(
        threadLog.map((r: { entity_type: string }) => r.entity_type),
      ).toEqual(["team_chat_message", "thread_message"])
      const threadMessage = decodeKhalaCodeThreadMessageEntity(
        parse(threadLog[1].post_image_json as string),
      )
      expect(threadMessage.threadId).toBe("thread_7")

      // Redaction holds end-to-end: nothing forbidden in any stored entry.
      for (const entry of [...teamLog, ...threadLog]) {
        const serialized = canonicalJson(parse(entry.post_image_json as string))
        expect(serialized).not.toContain("token_hash")
        expect(serialized).not.toContain("424242")
        expect(serialized).not.toContain("object_key")
      }
    })

    test("invite mirror keeps token/email out of the Postgres changelog while the twin row keeps them for cutover", async () => {
      await mirrorRow(
        "team_workspace_invites",
        ROWS["team_workspace_invites"]!,
      )
      // The Cloud SQL TWIN keeps full fidelity (it must be able to take
      // over from D1 at cutover)…
      const twin = await sql.unsafe(
        "SELECT token_hash, invitee_email FROM team_workspace_invites WHERE id = 'invite_1'",
      )
      expect(twin[0].token_hash).toBe(
        ROWS["team_workspace_invites"]!["token_hash"],
      )
      // …but the SCOPE POST-IMAGE never carries the secrets.
      const log = await sql.unsafe(
        `SELECT post_image_json FROM khala_sync_changelog
          WHERE entity_type = 'team_workspace_invite'`,
      )
      expect(log.length).toBe(1)
      const serialized =
        typeof log[0].post_image_json === "string"
          ? log[0].post_image_json
          : JSON.stringify(log[0].post_image_json)
      expect(serialized).not.toContain("jane@example.com")
      expect(serialized).not.toContain("deadbeef")
      expect(serialized).not.toContain("owner@example.com")
    })

    /** The flow the Worker mirror runs after an accepted D1 HARD delete. */
    const mirrorDelete = async (
      table: KhalaCodeProductStateTable,
      whereColumns: ReadonlyArray<string>,
      whereValues: ReadonlyArray<unknown>,
      deletedRows: ReadonlyArray<KhalaCodeProductStateRow>,
    ) =>
      withSyncTransaction(sql as unknown as SyncSql, async (writer) => {
        await deleteKhalaCodeProductStateRows(
          writer.sql,
          table,
          whereColumns,
          whereValues,
        )
        for (const row of deletedRows) {
          for (const tombstone of scopeTombstonesForKhalaCodeProductStateRow(
            table,
            row,
          )) {
            await writer.appendChange({
              entityId: tombstone.entityId,
              entityType: tombstone.entityType,
              mutationRef: `d1-shadow:ks-8.13:${table}`,
              op: "delete",
              scope: tombstone.scope,
            })
          }
        }
      })

    test("share-recipient hard-delete appends a dense tombstone into the subject scope and drops it from bootstrap", async () => {
      const recipientRow: KhalaCodeProductStateRow = {
        created_at: NOW,
        display_name: "Team Bertha <ops@example.com>",
        share_id: "share_del",
        subject_id: "team_del",
        subject_kind: "team",
      }
      const subjectScope = teamScope("team_del")

      // (1) The recipient is granted → upsert fan-out into the subject scope.
      await mirrorRow("share_projection_recipients", recipientRow)

      const afterUpsert = await bootstrap(sql as unknown as SyncSql, {
        scope: subjectScope,
      })
      expect(
        afterUpsert.entities.map((e) => String(e.entityId)),
      ).toContain("share_del:team:team_del")

      // (2) `replaceRecipients` hard-deletes the row (D1 read-before-delete
      // captured it) → tombstone appended, twin row removed.
      await mirrorDelete(
        "share_projection_recipients",
        ["share_id"],
        ["share_del"],
        [recipientRow],
      )

      // Twin row is gone…
      const twin = await sql.unsafe(
        "SELECT 1 FROM share_projection_recipients WHERE share_id = 'share_del'",
      )
      expect(twin.length).toBe(0)

      // …a delete-op changelog entry landed with a DENSE next version…
      const log = await sql.unsafe(
        `SELECT version, op, post_image_json
           FROM khala_sync_changelog
          WHERE scope = '${subjectScope}'
            AND entity_id = 'share_del:team:team_del'
          ORDER BY version ASC`,
      )
      expect(log.map((r: { op: string }) => r.op)).toEqual(["upsert", "delete"])
      expect(
        log.map((r: { version: string | number }) => Number(r.version)),
      ).toEqual([1, 2])
      // Tombstones carry no post-image.
      expect(log[1].post_image_json).toBeNull()

      // …and bootstrap now OMITS the tombstoned entity (latest row is delete).
      const afterDelete = await bootstrap(sql as unknown as SyncSql, {
        scope: subjectScope,
      })
      expect(
        afterDelete.entities.map((e) => String(e.entityId)),
      ).not.toContain("share_del:team:team_del")
    })
  },
)

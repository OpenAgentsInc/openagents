import {
  SarahBusinessContextSchema,
  SarahContextSourceSchema,
  sanitizeSarahConversationResponse,
  type SarahBusinessContext,
  type SarahContextSource,
} from "@openagentsinc/sarah";
import type { SyncSql } from "@openagentsinc/khala-sync-server";
import { Schema as S } from "effect";

import { recallSarahGraphMemory, type RecallSarahGraphMemoryInput } from "./sarah-graph-memory";

type Fetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

/**
 * Optional graph-memory recall for one hosted Sarah turn (issue #9189).
 * Default OFF: when omitted or `enabled: false`, no store is constructed and no
 * graph-memory source enters the context — zero behavior change.
 */
export type SarahGraphMemoryRecall = Pick<
  RecallSarahGraphMemoryInput,
  "enabled" | "query" | "storeLayer" | "maxItems" | "maxSummaryChars"
>;

type ConversationRow = Readonly<{
  source_ref: string;
  summary: string;
  observed_at: string;
}>;

const safeSummary = (value: string, limit = 700): string => {
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length <= limit ? normalized : `${normalized.slice(0, limit)}…`;
};

const currentSource = (
  input: Omit<SarahContextSource, "observedAt"> & { observedAt: string },
): SarahContextSource => S.decodeUnknownSync(SarahContextSourceSchema)(input);

const readConversation = async (
  sql: SyncSql,
  ownerUserId: string,
  threadRef: string,
): Promise<ReadonlyArray<SarahContextSource>> => {
  const rows: Array<ConversationRow> = await sql`
    SELECT source_ref, summary, observed_at
      FROM (
        SELECT ('source.sarah.message.' || message_id) AS source_ref,
               body AS summary,
               created_at AS observed_at
          FROM khala_sync_chat_messages
         WHERE thread_id = ${threadRef}
           AND author_user_id = ${ownerUserId}
           AND deleted_at IS NULL
        UNION ALL
        SELECT ('source.sarah.runtime.' || event_id) AS source_ref,
               (event_json ->> 'text') AS summary,
               observed_at
          FROM khala_sync_runtime_events
         WHERE thread_id = ${threadRef}
           AND owner_user_id = ${ownerUserId}
           AND kind = 'text.delta'
           AND event_json ? 'text'
      ) AS history
     ORDER BY observed_at DESC
     LIMIT 18
  `;
  return [...rows].reverse().map((row) =>
    currentSource({
      sourceRef: row.source_ref,
      kind: "conversation",
      observedAt: new Date(row.observed_at).toISOString(),
      freshness: "recent",
      sensitivity: "owner_private",
      summary: safeSummary(sanitizeSarahConversationResponse(row.summary)),
    }),
  );
};

const readFullAuto = async (
  sql: SyncSql,
  ownerUserId: string,
): Promise<ReadonlyArray<SarahContextSource>> => {
  const rows: Array<{
    run_ref: string;
    objective: string;
    lifecycle_state: string;
    updated_at: string;
  }> = await sql`
    SELECT run_ref, objective, lifecycle_state, updated_at
      FROM desktop_full_auto_run_projections
     WHERE owner_user_id = ${ownerUserId}
     LIMIT 1
  `;
  return rows.map((row) =>
    currentSource({
      sourceRef: `source.full_auto.${row.run_ref}`,
      kind: "full_auto",
      observedAt: new Date(row.updated_at).toISOString(),
      freshness: "live",
      sensitivity: "owner_private",
      summary: safeSummary(`Full Auto ${row.run_ref} is ${row.lifecycle_state}: ${row.objective}`),
    }),
  );
};

const readFleet = async (
  sql: SyncSql,
  ownerUserId: string,
): Promise<ReadonlyArray<SarahContextSource>> => {
  const rows: Array<{
    run_ref: string;
    status: string;
    worker_kind: string;
    updated_at: string;
  }> = await sql`
    SELECT run_ref, status, worker_kind, updated_at
      FROM sarah_fleet_run_requests
     WHERE owner_user_id = ${ownerUserId}
     ORDER BY updated_at DESC
     LIMIT 4
  `;
  return rows.map((row) =>
    currentSource({
      sourceRef: `source.fleet.${row.run_ref}`,
      kind: "fleet",
      observedAt: new Date(row.updated_at).toISOString(),
      freshness: "live",
      sensitivity: "owner_private",
      summary: `Fleet run ${row.run_ref} is ${row.status} on ${row.worker_kind}.`,
    }),
  );
};

const readForum = async (sql: SyncSql): Promise<ReadonlyArray<SarahContextSource>> => {
  const rows: Array<{
    post_id: string;
    actor_ref: string;
    topic_title: string;
    body_text: string;
    updated_at: string;
  }> = await sql`
    SELECT posts.id AS post_id,
           posts.actor_ref,
           topics.title AS topic_title,
           bodies.body_text,
           posts.updated_at
      FROM forum_posts AS posts
      JOIN forum_topics AS topics ON topics.id = posts.topic_id
      JOIN forum_post_bodies AS bodies ON bodies.post_id = posts.id
     WHERE posts.state = 'visible'
       AND posts.archived_at IS NULL
       AND topics.archived_at IS NULL
       AND bodies.archived_at IS NULL
     ORDER BY posts.updated_at DESC
     LIMIT 6
  `;
  return rows.map((row) =>
    currentSource({
      sourceRef: `source.forum.post.${row.post_id}`,
      kind: "forum",
      observedAt: new Date(row.updated_at).toISOString(),
      freshness: "recent",
      sensitivity: "public",
      summary: safeSummary(`${row.actor_ref} in “${row.topic_title}”: ${row.body_text}`),
    }),
  );
};

const GitHubReleaseSchema = S.Struct({
  html_url: S.String,
  name: S.NullOr(S.String),
  published_at: S.NullOr(S.String),
  tag_name: S.String,
});

const GitHubIssueSchema = S.Struct({
  html_url: S.String,
  number: S.Number,
  title: S.String,
  updated_at: S.String,
  pull_request: S.optional(S.Unknown),
});

const githubHeaders = {
  accept: "application/vnd.github+json",
  "user-agent": "openagents-sarah-context",
};

const readGitHub = async (fetchFn: Fetch): Promise<ReadonlyArray<SarahContextSource>> => {
  const [releaseResponse, issuesResponse] = await Promise.all([
    fetchFn("https://api.github.com/repos/OpenAgentsInc/openagents/releases/latest", {
      headers: githubHeaders,
    }),
    fetchFn(
      "https://api.github.com/repos/OpenAgentsInc/openagents/issues?state=open&sort=updated&direction=desc&per_page=8",
      { headers: githubHeaders },
    ),
  ]);
  const sources: Array<SarahContextSource> = [];
  if (releaseResponse.ok) {
    const release = S.decodeUnknownSync(GitHubReleaseSchema)(await releaseResponse.json());
    sources.push(
      currentSource({
        sourceRef: `source.github.release.${release.tag_name}`,
        kind: "github_release",
        observedAt: release.published_at ?? new Date().toISOString(),
        freshness: "live",
        sensitivity: "public",
        summary: `Latest GitHub release is ${release.name ?? release.tag_name} (${release.tag_name}): ${release.html_url}`,
      }),
    );
  }
  if (issuesResponse.ok) {
    const issues = S.decodeUnknownSync(S.Array(GitHubIssueSchema))(await issuesResponse.json());
    for (const issue of issues.filter((value) => value.pull_request === undefined).slice(0, 6)) {
      sources.push(
        currentSource({
          sourceRef: `source.github.issue.${issue.number}`,
          kind: "github_issue",
          observedAt: issue.updated_at,
          freshness: "live",
          sensitivity: "public",
          summary: `Open issue #${issue.number}: ${safeSummary(issue.title, 240)} (${issue.html_url})`,
        }),
      );
    }
  }
  return sources;
};

const readCloudHealth = async (fetchFn: Fetch): Promise<ReadonlyArray<SarahContextSource>> => {
  const observedAt = new Date().toISOString();
  const response = await fetchFn("https://openagents.com/internal/healthz");
  return [
    currentSource({
      sourceRef: "source.cloud.openagents-monolith.healthz",
      kind: "cloud_health",
      observedAt,
      freshness: "live",
      sensitivity: "public",
      summary: response.ok
        ? "OpenAgents Cloud monolith health check is passing."
        : `OpenAgents Cloud monolith health check returned HTTP ${response.status}.`,
    }),
  ];
};

const settleSources = async (
  tasks: ReadonlyArray<Promise<ReadonlyArray<SarahContextSource>>>,
): Promise<ReadonlyArray<SarahContextSource>> =>
  (await Promise.allSettled(tasks)).flatMap((result) =>
    result.status === "fulfilled" ? result.value : [],
  );

export const collectSarahBusinessContext = async (
  input: Readonly<{
    sql: SyncSql;
    ownerUserId: string;
    threadRef: string;
    fetch?: Fetch;
    now?: () => Date;
    graphMemoryRecall?: SarahGraphMemoryRecall;
  }>,
): Promise<SarahBusinessContext> => {
  const now = input.now?.() ?? new Date();
  const recall = input.graphMemoryRecall;
  const sources = [
    ...(await settleSources([
      readConversation(input.sql, input.ownerUserId, input.threadRef),
      readFullAuto(input.sql, input.ownerUserId),
      readFleet(input.sql, input.ownerUserId),
      readForum(input.sql),
      readGitHub(input.fetch ?? fetch),
      readCloudHealth(input.fetch ?? fetch),
      // Owner-scoped, redacted, fail-soft graph-memory recall. Default OFF: with
      // no recall config or `enabled: false`, this resolves to an empty slice
      // and never opens a store (issue #9189).
      recallSarahGraphMemory({
        ownerUserId: input.ownerUserId,
        enabled: recall?.enabled ?? false,
        query: recall?.query ?? "",
        now: () => now,
        ...(recall?.storeLayer === undefined ? {} : { storeLayer: recall.storeLayer }),
        ...(recall?.maxItems === undefined ? {} : { maxItems: recall.maxItems }),
        ...(recall?.maxSummaryChars === undefined
          ? {}
          : { maxSummaryChars: recall.maxSummaryChars }),
      }),
    ])),
  ];
  sources.push(
    currentSource({
      sourceRef: "source.contract.sarah-owner-orchestrator.rev1",
      kind: "product_contract",
      observedAt: now.toISOString(),
      freshness: "live",
      sensitivity: "owner_private",
      summary:
        "Sarah is the owner-authenticated orchestrator. Public /sarah and raw-secret access remain retired; actions require admitted capability and authority receipts.",
    }),
  );
  return SarahBusinessContextSchema.make({
    schema: "openagents.sarah.business_context.v1",
    threadRef: input.threadRef,
    generatedAt: S.decodeUnknownSync(S.DateTimeUtcFromDate)(now),
    sources: sources.slice(-32),
  });
};

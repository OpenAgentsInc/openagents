import type { SyncSql } from "@openagentsinc/khala-sync-server";
import { describe, expect, test } from "vitest";

import { collectSarahBusinessContext } from "./sarah-business-context";

const timestamp = "2026-07-18T15:00:00.000Z";

const sql = (async (strings: TemplateStringsArray) => {
  const query = strings.join(" ");
  if (query.includes("khala_sync_chat_messages")) {
    return [
      {
        source_ref: "source.sarah.message.message.fixture",
        summary: "What is our release status?",
        observed_at: timestamp,
      },
    ];
  }
  if (query.includes("desktop_full_auto_run_projections")) {
    return [
      {
        run_ref: "run.fixture",
        objective: "Finish Full Auto",
        lifecycle_state: "running",
        updated_at: timestamp,
      },
    ];
  }
  if (query.includes("sarah_fleet_run_requests")) {
    return [
      {
        run_ref: "fleet.fixture",
        status: "claimed_by_pylon",
        worker_kind: "auto",
        updated_at: timestamp,
      },
    ];
  }
  if (query.includes("forum_posts")) {
    return [
      {
        post_id: "post.fixture",
        actor_ref: "tester.fixture",
        topic_title: "Release candidates",
        body_text: "The latest candidate launches.",
        updated_at: timestamp,
      },
    ];
  }
  return [];
}) as unknown as SyncSql;

const fetchFixture: typeof fetch = async (input) => {
  const url = String(input);
  if (url.endsWith("/releases/latest")) {
    return Response.json({
      html_url: "https://github.com/OpenAgentsInc/openagents/releases/tag/v0.1.0-rc.21",
      name: "RC21",
      published_at: timestamp,
      tag_name: "v0.1.0-rc.21",
    });
  }
  if (url.includes("/issues?")) {
    return Response.json([
      {
        html_url: "https://github.com/OpenAgentsInc/openagents/issues/9001",
        number: 9001,
        title: "Full Auto runtime",
        updated_at: timestamp,
      },
    ]);
  }
  return Response.json({ ok: true });
};

describe("Sarah business context", () => {
  test("collects bounded cited conversation, release, issue, Forum, Full Auto, fleet, health, and contract sources", async () => {
    const context = await collectSarahBusinessContext({
      sql,
      ownerUserId: "owner.fixture",
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: fetchFixture,
      now: () => new Date(timestamp),
    });
    expect(new Set(context.sources.map((source) => source.kind))).toEqual(
      new Set([
        "conversation",
        "github_release",
        "github_issue",
        "forum",
        "full_auto",
        "fleet",
        "cloud_health",
        "product_contract",
      ]),
    );
    expect(context.sources.length).toBeLessThanOrEqual(32);
    expect(context.sources.every((source) => source.sourceRef.startsWith("source."))).toBe(true);
    expect(JSON.stringify(context)).not.toContain("private-password");
  });

  test("fails soft when optional live sources are unavailable", async () => {
    const context = await collectSarahBusinessContext({
      sql: (async () => {
        throw new Error("unavailable");
      }) as unknown as SyncSql,
      ownerUserId: "owner.fixture",
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: async () => {
        throw new Error("unavailable");
      },
      now: () => new Date(timestamp),
    });
    expect(context.sources).toHaveLength(1);
    expect(context.sources[0]?.kind).toBe("product_contract");
  });
});

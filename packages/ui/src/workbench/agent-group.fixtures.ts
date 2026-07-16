import type { DesktopAgentActivity } from "./agent-group.tsx";

/**
 * Fixture set for `DesktopAgentGroup` (issue 8870, epic 8857 T13 gallery
 * lane). Covers a single delegated agent in every `DesktopAgentStatus`, a
 * multi-agent `collabAgentToolCall` group, and the three `subAgentActivity`
 * ping kinds (`started`/`interacted`/`interrupted`) that are distinct from a
 * row's own lifecycle status.
 */
export type DesktopAgentGroupFixture = Readonly<{
  name: string;
  itemKey: string;
  operation?: string;
  prompt?: string;
  title?: string;
  agents: ReadonlyArray<DesktopAgentActivity>;
}>;

export const desktopAgentGroupFixtures: ReadonlyArray<DesktopAgentGroupFixture> = [
  {
    name: "single agent — running",
    itemKey: "fixture-agent-single-running",
    operation: "spawn",
    prompt: "Audit the reasoning delta pipeline for dropped chunks.",
    agents: [
      {
        agentKey: "agent-running",
        name: "protocol-scout",
        role: "Delegated agent",
        status: "running",
        detail: "Auditing the reasoning delta pipeline for dropped chunks.",
      },
    ],
  },
  {
    name: "single agent — completed",
    itemKey: "fixture-agent-single-completed",
    operation: "resume",
    agents: [
      {
        agentKey: "agent-completed",
        name: "protocol-scout",
        role: "Delegated agent",
        status: "completed",
        detail: "Confirmed no chunks were dropped; the delta buffer flushes on every newline.",
      },
    ],
  },
  {
    name: "single agent — failed",
    itemKey: "fixture-agent-single-failed",
    operation: "send",
    agents: [
      {
        agentKey: "agent-failed",
        name: "release-scout",
        role: "Delegated agent",
        status: "failed",
        detail: "Lost the app-server connection mid-turn.",
        transcript: [{ label: "Error", text: "ECONNRESET while awaiting item/completed." }],
      },
    ],
  },
  {
    name: "single agent — waiting (pendingInit)",
    itemKey: "fixture-agent-single-waiting",
    operation: "wait",
    agents: [
      {
        agentKey: "agent-waiting",
        name: "diff-reviewer",
        role: "Delegated agent",
        status: "waiting",
        statusLabel: "PENDING INIT",
        detail: "",
      },
    ],
  },
  {
    name: "multiple agents — mixed status",
    itemKey: "fixture-agent-multiple",
    operation: "spawn",
    prompt: "Split the T13 gallery work across three review passes.",
    title: "Delegated agents",
    agents: [
      {
        agentKey: "agent-multi-1",
        name: "component-auditor",
        role: "Delegated agent",
        status: "completed",
        detail: "Confirmed 28 of 28 barrel exports have a rendered story.",
      },
      {
        agentKey: "agent-multi-2",
        name: "fixture-writer",
        role: "Delegated agent",
        status: "running",
        detail: "Writing agent-group and context-meter fixtures.",
      },
      {
        agentKey: "agent-multi-3",
        name: "screenshot-runner",
        role: "Delegated agent",
        status: "waiting",
        statusLabel: "NOT FOUND",
        detail: "",
      },
    ],
  },
  {
    name: "subAgentActivity — started/interacted/interrupted pings",
    itemKey: "fixture-agent-activity-kinds",
    title: "Subagent activity",
    agents: [
      {
        agentKey: "activity-started",
        name: "background-indexer",
        role: "Subagent activity",
        status: "running",
        detail: "",
        activityKind: "started",
        path: "background-indexer",
      },
      {
        agentKey: "activity-interacted",
        name: "background-indexer",
        role: "Subagent activity",
        status: "running",
        detail: "",
        activityKind: "interacted",
        path: "background-indexer",
      },
      {
        agentKey: "activity-interrupted",
        name: "background-indexer",
        role: "Subagent activity",
        status: "failed",
        detail: "",
        activityKind: "interrupted",
        path: "background-indexer",
      },
    ],
  },
];

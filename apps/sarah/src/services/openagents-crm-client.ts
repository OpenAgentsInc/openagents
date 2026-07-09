import { z } from "zod";

import type { SalesToolMode } from "./openagents-sales-client";

const sourceRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/;

export const crmContactUpsertInputSchema = z.object({
  primaryEmail: z.email().max(320),
  fullName: z.string().min(1).max(200).nullable().default(null),
  firstName: z.string().min(1).max(100).nullable().default(null),
  lastName: z.string().min(1).max(100).nullable().default(null),
  jobTitle: z.string().min(1).max(160).nullable().default(null),
  accountId: z.string().min(1).max(220).nullable().default(null),
  notes: z.string().min(1).max(2_000).nullable().default(null),
  sourceRef: z
    .string()
    .regex(sourceRefPattern)
    .default("sarah.crm_contact.v1"),
});

export const crmActivityAppendInputSchema = z.object({
  contactId: z.string().min(1).max(220),
  activityType: z
    .string()
    .min(1)
    .max(120)
    .default("sarah_session_summary"),
  subject: z.string().min(1).max(300).nullable().default(null),
  summary: z.string().min(1).max(4_000),
  sourceRecordId: z.string().min(1).max(220).nullable().default(null),
  sourceRef: z
    .string()
    .regex(sourceRefPattern)
    .default("sarah.crm_activity.v1"),
});

export type CrmContactUpsertInput = z.infer<
  typeof crmContactUpsertInputSchema
>;
export type CrmActivityAppendInput = z.infer<
  typeof crmActivityAppendInputSchema
>;

type JsonRpcResponse = {
  error?: { message?: string };
  result?: {
    isError?: boolean;
    content?: Array<{ text?: string }>;
    structuredContent?: unknown;
  };
};

function openAgentsBaseUrl() {
  return (
    process.env.SARAH_OPENAGENTS_BASE_URL?.replace(/\/+$/, "") ??
    "https://openagents.com"
  );
}

function liveWritesEnabled() {
  return process.env.SARAH_OPENAGENTS_LIVE_WRITES === "1";
}

function crmMcpToken() {
  return process.env.SARAH_OPENAGENTS_CRM_MCP_TOKEN?.trim() || null;
}

function dryRunRef(prefix: string) {
  return `${prefix}.${crypto.randomUUID()}`;
}

async function callCrmMcpTool<T>(name: string, args: Record<string, unknown>) {
  const token = crmMcpToken();
  if (!token) {
    throw new Error("SARAH_OPENAGENTS_CRM_MCP_TOKEN is required.");
  }

  const response = await fetch(`${openAgentsBaseUrl()}/api/mcp`, {
    method: "POST",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      id: crypto.randomUUID(),
      jsonrpc: "2.0",
      method: "tools/call",
      params: { arguments: args, name },
    }),
  });
  const text = await response.text();
  const json = (text ? JSON.parse(text) : null) as JsonRpcResponse | null;

  if (!response.ok || json?.error) {
    throw new Error(
      `OpenAgents CRM MCP ${response.status}: ${
        json?.error?.message ?? text ?? "request failed"
      }`,
    );
  }

  if (json?.result?.isError) {
    const message =
      json.result.content?.map(block => block.text).filter(Boolean).join("\n") ||
      "tool call failed";
    throw new Error(`OpenAgents CRM MCP tool error: ${message}`);
  }

  return json?.result?.structuredContent as T;
}

export async function upsertOpenAgentsCrmContact(
  input: CrmContactUpsertInput,
) {
  if (!liveWritesEnabled() || !crmMcpToken()) {
    const contactId = dryRunRef("dry_run.crm_contact");
    return {
      ok: true,
      mode: "dry_run" as SalesToolMode,
      contactId,
      primaryEmail: input.primaryEmail,
      sourceRef: input.sourceRef,
      message:
        "Prepared a CRM contact upsert in Sarah test mode; no OpenAgents CRM row was written.",
    };
  }

  const payload = await callCrmMcpTool<{
    result?: {
      contact?: { id?: string; primaryEmail?: string };
      created?: boolean;
    };
  }>("crm.contact.upsert", {
    externalSourceId: input.sourceRef,
    externalSourceLabel: "sarah",
    primaryEmail: input.primaryEmail,
    fullName: input.fullName,
    firstName: input.firstName,
    lastName: input.lastName,
    jobTitle: input.jobTitle,
    accountId: input.accountId,
    notes: input.notes,
  });

  return {
    ok: true,
    mode: "live" as SalesToolMode,
    contactId: payload.result?.contact?.id ?? null,
    created: payload.result?.created ?? null,
    primaryEmail: payload.result?.contact?.primaryEmail ?? input.primaryEmail,
    sourceRef: input.sourceRef,
    response: payload,
    message: "Upserted the prospect in the OpenAgents CRM.",
  };
}

export async function appendOpenAgentsCrmActivity(
  input: CrmActivityAppendInput,
) {
  const sourceRecordId =
    input.sourceRecordId ?? dryRunRef("sarah_session_summary");

  if (!liveWritesEnabled() || !crmMcpToken()) {
    return {
      ok: true,
      mode: "dry_run" as SalesToolMode,
      activityRef: sourceRecordId,
      contactId: input.contactId,
      sourceRef: input.sourceRef,
      summary: input.summary,
      message:
        "Prepared a CRM activity append in Sarah test mode; no OpenAgents CRM activity was written.",
    };
  }

  const payload = await callCrmMcpTool<{
    result?: { id?: string; contactId?: string; activityType?: string };
  }>("crm.activity.append", {
    activityType: input.activityType,
    contactId: input.contactId,
    sourceRecordId,
    sourceRecordType: "sarah_session",
    sourceSystem: "openagents_sarah",
    subject: input.subject,
    summary: input.summary,
  });

  return {
    ok: true,
    mode: "live" as SalesToolMode,
    activityRef: payload.result?.id ?? sourceRecordId,
    activityType: payload.result?.activityType ?? input.activityType,
    contactId: payload.result?.contactId ?? input.contactId,
    sourceRef: input.sourceRef,
    summary: input.summary,
    response: payload,
    message: "Appended the Sarah session summary to the OpenAgents CRM.",
  };
}

export type SarahCrmContext = {
  contact: unknown;
  activities: unknown;
};

export async function readOpenAgentsCrmContext(contactId: string) {
  if (!crmMcpToken()) return null;

  const [contact, activities] = await Promise.all([
    callCrmMcpTool("crm.contact.get", { contactId }),
    callCrmMcpTool("crm.contact.activities.list", { contactId }),
  ]);

  return { activities, contact } satisfies SarahCrmContext;
}

import { createJiti } from "jiti";

const allowBlocked = process.env.SARAH_S7_ALLOW_BLOCKED === "1";
const liveWrites = process.env.SARAH_OPENAGENTS_LIVE_WRITES === "1";
const token = process.env.SARAH_OPENAGENTS_CRM_MCP_TOKEN?.trim();

function finish(payload, exitCode = 0) {
  console.log(JSON.stringify(payload, null, 2));
  process.exitCode = exitCode;
}

function blocked(reason, details = {}) {
  finish(
    {
      schema: "sarah.s7_live_crm_smoke.v1",
      generatedAt: new Date().toISOString(),
      status: "blocked",
      reason,
      requiredEnv: [
        "SARAH_OPENAGENTS_LIVE_WRITES=1",
        "SARAH_OPENAGENTS_CRM_MCP_TOKEN=<scoped oa_mcp_... token with operator_read + workspace_write>",
      ],
      ...details,
    },
    allowBlocked ? 0 : 2,
  );
}

if (!liveWrites) {
  blocked("live_writes_not_armed");
} else if (!token) {
  blocked("missing_sarah_openagents_crm_mcp_token");
} else {
  process.env.SARAH_SESSION_INDEX_PATH =
    process.env.SARAH_SESSION_INDEX_PATH ?? "s7-live-crm-session-index.json";

  const jiti = createJiti(import.meta.url);
  const {
    appendOpenAgentsCrmActivity,
    readOpenAgentsCrmContext,
    upsertOpenAgentsCrmContact,
  } = jiti("../src/lib/openagents-crm-client.ts");
  const {
    getSarahProspectCrmProjection,
    recordSarahCrmActivity,
    recordSarahCrmContact,
    recordSarahTranscriptTurn,
  } = jiti("../src/lib/session-index.ts");

  const runId = `s7-live-${Date.now()}`;
  const prospectRef = `prospect.${runId}`;
  const sessionId = `session.${runId}`;
  const threadId = `prospect:${prospectRef}`;
  const primaryEmail = `${runId}@example.com`;
  const summary =
    "Sarah S-7 live smoke: prospect asked about OpenAgents AI sales employees and requested CRM continuity proof.";

  try {
    await recordSarahTranscriptTurn({
      prospectRef,
      sessionId,
      threadId,
      turn: {
        modality: "text",
        role: "user",
        sourceEvent: "s7-live-smoke",
        text: `My email is ${primaryEmail}. Please remember this OpenAgents sales conversation.`,
      },
    });

    const contact = await upsertOpenAgentsCrmContact({
      accountId: null,
      firstName: "Sarah",
      fullName: "Sarah S7 Live Smoke",
      jobTitle: "CRM continuity smoke",
      lastName: "Smoke",
      notes: summary,
      primaryEmail,
      sourceRef: `sarah.s7_live_crm.${runId}`,
    });

    if (contact.mode !== "live" || typeof contact.contactId !== "string") {
      throw new Error(
        `CRM contact upsert did not return a live contact id: ${JSON.stringify({
          mode: contact.mode,
          hasContactId: Boolean(contact.contactId),
        })}`,
      );
    }

    await recordSarahCrmContact({
      prospectRef,
      contactEmail: primaryEmail,
      contactId: contact.contactId,
      mode: "live",
    });

    const activity = await appendOpenAgentsCrmActivity({
      activityType: "sarah_session_summary",
      contactId: contact.contactId,
      sourceRecordId: `sarah_session.${runId}`,
      sourceRef: `sarah.s7_live_crm_activity.${runId}`,
      subject: "Sarah S-7 live CRM smoke",
      summary,
    });

    if (activity.mode !== "live" || typeof activity.activityRef !== "string") {
      throw new Error(
        `CRM activity append did not return a live activity ref: ${JSON.stringify({
          mode: activity.mode,
          hasActivityRef: Boolean(activity.activityRef),
        })}`,
      );
    }

    await recordSarahCrmActivity({
      prospectRef,
      contactId: contact.contactId,
      activityRef: activity.activityRef,
      mode: "live",
      summary,
    });

    const context = await readOpenAgentsCrmContext(contact.contactId);
    const projection = await getSarahProspectCrmProjection(prospectRef);

    if (!context?.contact) {
      throw new Error("Live CRM context did not return the created contact.");
    }

    finish({
      schema: "sarah.s7_live_crm_smoke.v1",
      generatedAt: new Date().toISOString(),
      status: "passed",
      baseUrl:
        process.env.SARAH_OPENAGENTS_BASE_URL?.replace(/\/+$/, "") ??
        "https://openagents.com",
      prospectRef,
      sessionId,
      threadId,
      contact: {
        mode: contact.mode,
        contactId: contact.contactId,
        primaryEmail: contact.primaryEmail,
        created: contact.created ?? null,
      },
      activity: {
        mode: activity.mode,
        activityRef: activity.activityRef,
        activityType: activity.activityType ?? null,
        contactId: activity.contactId,
      },
      returningContext: {
        hasContact: Boolean(context.contact),
        hasActivities: Boolean(context.activities),
      },
      localProjection: projection,
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    blocked("live_crm_smoke_failed", { error: message });
  }
}

import { mkdir, readFile, writeFile } from "node:fs/promises";

import {
  persistSarahProspectContact,
  persistSarahTurn,
} from "./turn-store.ts";
import { dirname, join } from "node:path";

export type SarahTranscriptRecord = {
  modality: "text" | "voice";
  role: "user" | "assistant";
  sourceEvent: string;
  text: string;
  recordedAt: string;
};

export type SarahToolReceipt = {
  toolCallId: string;
  toolName: string;
  recordedAt: string;
  mode: "dry_run" | "live" | null;
  ok: boolean | null;
  quoteRef: string | null;
  checkoutRef: string | null;
  handoffRef: string | null;
  dealRuleRefs: string[];
  escalationReason: string | null;
  summary: string;
};

export type SarahSessionRecord = {
  sessionId: string;
  threadId: string;
  prospectRef: string;
  startedAt: string;
  updatedAt: string;
  closedAt?: string | null;
  transcript: SarahTranscriptRecord[];
  tools?: SarahToolReceipt[];
};

export type SarahCrmProjection = {
  contactEmail: string | null;
  contactId: string | null;
  lastActivityRef: string | null;
  lastSummary: string | null;
  mode: "dry_run" | "live" | null;
  updatedAt: string;
};

export type SarahSessionIndex = {
  schema: "sarah.session_index.v1";
  prospects: Record<
    string,
    {
      prospectRef: string;
      crm?: SarahCrmProjection;
      sessions: Record<string, SarahSessionRecord>;
    }
  >;
};

const defaultIndexPath = join(
  /* turbopackIgnore: true */ process.cwd(),
  ".sarah",
  "session-index.json",
);

function indexPath() {
  const configured = process.env.SARAH_SESSION_INDEX_PATH;
  if (!configured) return defaultIndexPath;

  return join(
    /* turbopackIgnore: true */ process.cwd(),
    ".sarah",
    configured,
  );
}

async function readIndex(): Promise<SarahSessionIndex> {
  try {
    const raw = await readFile(indexPath(), "utf8");
    const parsed = JSON.parse(raw) as SarahSessionIndex;
    if (parsed.schema === "sarah.session_index.v1") return parsed;
  } catch {
    // Start a new local projection when the file does not exist or is invalid.
  }

  return { schema: "sarah.session_index.v1", prospects: {} };
}

let writeQueue = Promise.resolve();

async function writeIndex(index: SarahSessionIndex) {
  const path = indexPath();
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(index, null, 2)}\n`);
}

export async function recordSarahTranscriptTurn({
  prospectRef,
  sessionId,
  threadId,
  turn,
}: {
  prospectRef: string;
  sessionId: string;
  threadId: string;
  turn: Omit<SarahTranscriptRecord, "recordedAt">;
}) {
  writeQueue = writeQueue.then(async () => {
    const index = await readIndex();
    const now = new Date().toISOString();
    const prospect = (index.prospects[prospectRef] ??= {
      prospectRef,
      sessions: {},
    });
    const session = (prospect.sessions[sessionId] ??= {
      sessionId,
      threadId,
      prospectRef,
      startedAt: now,
      updatedAt: now,
      transcript: [],
      tools: [],
    });

    session.updatedAt = now;
    session.tools ??= [];
    session.transcript.push({ ...turn, recordedAt: now });
    await writeIndex(index);
  });

  await writeQueue;
  // Durable record (the local JSON projection is ephemeral on Cloud Run).
  await persistSarahTurn({
    prospectRef,
    sessionId,
    threadId,
    modality: turn.modality,
    role: turn.role,
    sourceEvent: turn.sourceEvent,
    text: turn.text,
  });
}

function outputValue(output: Record<string, unknown>, key: string) {
  const value = output[key];
  return typeof value === "string" ? value : null;
}

function outputStringArray(output: Record<string, unknown>, key: string) {
  const value = output[key];
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

export async function getSarahSessionTranscript({
  prospectRef,
  sessionId,
  limit = 12,
}: {
  prospectRef: string;
  sessionId: string;
  limit?: number;
}): Promise<SarahTranscriptRecord[]> {
  const index = await readIndex();
  const transcript =
    index.prospects[prospectRef]?.sessions[sessionId]?.transcript ?? [];
  return transcript.slice(-limit);
}

export async function recordSarahToolReceipt({
  prospectRef,
  sessionId,
  threadId,
  toolCallId,
  toolName,
  result,
}: {
  prospectRef: string;
  sessionId: string;
  threadId: string;
  toolCallId: string;
  toolName: string;
  result: unknown;
}) {
  writeQueue = writeQueue.then(async () => {
    const index = await readIndex();
    const now = new Date().toISOString();
    const prospect = (index.prospects[prospectRef] ??= {
      prospectRef,
      sessions: {},
    });
    const session = (prospect.sessions[sessionId] ??= {
      sessionId,
      threadId,
      prospectRef,
      startedAt: now,
      updatedAt: now,
      transcript: [],
      tools: [],
    });
    const resultRecord =
      typeof result === "object" && result !== null
        ? (result as Record<string, unknown>)
        : {};
    const output =
      typeof resultRecord.output === "object" && resultRecord.output !== null
        ? (resultRecord.output as Record<string, unknown>)
        : resultRecord;

    session.updatedAt = now;
    session.tools ??= [];
    session.tools.push({
      toolCallId,
      toolName,
      recordedAt: now,
      mode:
        output.mode === "live"
          ? "live"
          : output.mode === "dry_run"
            ? "dry_run"
            : null,
      ok: typeof resultRecord.ok === "boolean" ? resultRecord.ok : null,
      quoteRef: outputValue(output, "quoteRef"),
      checkoutRef: outputValue(output, "checkoutRef"),
      handoffRef: outputValue(output, "handoffRef"),
      dealRuleRefs: outputStringArray(output, "dealRuleRefs").concat(
        outputStringArray(output, "ruleRefs"),
      ),
      escalationReason: outputValue(output, "reason"),
      summary:
        typeof output.message === "string"
          ? output.message
          : `${toolName} receipt recorded.`,
    });
    await writeIndex(index);
  });

  await writeQueue;
}

export async function recordSarahCrmContact({
  contactEmail,
  contactId,
  mode,
  prospectRef,
}: {
  contactEmail: string | null;
  contactId: string | null;
  mode: "dry_run" | "live";
  prospectRef: string;
}) {
  writeQueue = writeQueue.then(async () => {
    const index = await readIndex();
    const now = new Date().toISOString();
    const prospect = (index.prospects[prospectRef] ??= {
      prospectRef,
      sessions: {},
    });
    prospect.crm = {
      contactEmail: contactEmail ?? prospect.crm?.contactEmail ?? null,
      contactId: contactId ?? prospect.crm?.contactId ?? null,
      lastActivityRef: prospect.crm?.lastActivityRef ?? null,
      lastSummary: prospect.crm?.lastSummary ?? null,
      mode,
      updatedAt: now,
    };
    await writeIndex(index);
  });

  await writeQueue;
  await persistSarahProspectContact({
    prospectRef,
    contactId: contactId ?? null,
    contactEmail: contactEmail ?? null,
    mode: mode ?? null,
  });
}

export async function recordSarahCrmActivity({
  activityRef,
  contactId,
  mode,
  prospectRef,
  summary,
}: {
  activityRef: string | null;
  contactId: string | null;
  mode: "dry_run" | "live";
  prospectRef: string;
  summary: string | null;
}) {
  writeQueue = writeQueue.then(async () => {
    const index = await readIndex();
    const now = new Date().toISOString();
    const prospect = (index.prospects[prospectRef] ??= {
      prospectRef,
      sessions: {},
    });
    prospect.crm = {
      contactEmail: prospect.crm?.contactEmail ?? null,
      contactId: contactId ?? prospect.crm?.contactId ?? null,
      lastActivityRef: activityRef ?? prospect.crm?.lastActivityRef ?? null,
      lastSummary: summary ?? prospect.crm?.lastSummary ?? null,
      mode,
      updatedAt: now,
    };
    await writeIndex(index);
  });

  await writeQueue;
}

export async function getSarahProspectCrmProjection(prospectRef: string) {
  const index = await readIndex();
  return index.prospects[prospectRef]?.crm ?? null;
}

export async function findSarahProspectByContactEmail(contactEmail: string) {
  const normalized = contactEmail.trim().toLowerCase();
  if (!normalized) return null;

  const index = await readIndex();
  for (const prospect of Object.values(index.prospects)) {
    const storedEmail = prospect.crm?.contactEmail?.trim().toLowerCase();
    if (storedEmail !== normalized) continue;

    const latestSession = Object.values(prospect.sessions).sort((a, b) =>
      b.updatedAt.localeCompare(a.updatedAt),
    )[0];

    return {
      prospectRef: prospect.prospectRef,
      contactEmail: prospect.crm?.contactEmail ?? null,
      contactId: prospect.crm?.contactId ?? null,
      threadId: latestSession?.threadId ?? `prospect:${prospect.prospectRef}`,
      sessionId: latestSession?.sessionId ?? null,
      crm: prospect.crm ?? null,
    };
  }

  return null;
}

export async function listSarahProspectSessions() {
  const index = await readIndex();

  return Object.values(index.prospects).map((prospect) => ({
    crm: prospect.crm ?? null,
    prospectRef: prospect.prospectRef,
    sessions: Object.values(prospect.sessions).map((session) => ({
      sessionId: session.sessionId,
      threadId: session.threadId,
      startedAt: session.startedAt,
      updatedAt: session.updatedAt,
      closedAt: session.closedAt ?? null,
      transcriptTurns: session.transcript.length,
      transcript: session.transcript,
      tools: session.tools ?? [],
    })),
  }));
}

export async function listSarahSessionReceipts() {
  const prospects = await listSarahProspectSessions();

  return prospects.flatMap((prospect) =>
    prospect.sessions.map((session) => {
      const tools = session.tools ?? [];
      return {
        schema: "sarah.session_receipt.v1" as const,
        prospectRef: prospect.prospectRef,
        crm: prospect.crm,
        sessionId: session.sessionId,
        threadId: session.threadId,
        startedAt: session.startedAt,
        updatedAt: session.updatedAt,
        closedAt: session.closedAt ?? null,
        channels: Array.from(
          new Set(session.transcript.map((turn) => turn.modality)),
        ),
        transcriptTurns: session.transcriptTurns,
        toolsUsed: tools.map((tool) => ({
          toolCallId: tool.toolCallId,
          toolName: tool.toolName,
          recordedAt: tool.recordedAt,
          mode: tool.mode,
          ok: tool.ok,
        })),
        quoteRefs: tools
          .map((tool) => tool.quoteRef)
          .filter((ref): ref is string => Boolean(ref)),
        checkoutRefs: tools
          .map((tool) => tool.checkoutRef)
          .filter((ref): ref is string => Boolean(ref)),
        handoffRefs: tools
          .map((tool) => tool.handoffRef)
          .filter((ref): ref is string => Boolean(ref)),
        escalations: tools
          .filter((tool) => tool.escalationReason)
          .map((tool) => ({
            toolCallId: tool.toolCallId,
            toolName: tool.toolName,
            reason: tool.escalationReason,
            recordedAt: tool.recordedAt,
          })),
        receiptRefs: tools.flatMap((tool) =>
          [tool.quoteRef, tool.checkoutRef, tool.handoffRef].filter(
            (ref): ref is string => Boolean(ref),
          ),
        ),
      };
    }),
  );
}

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { enqueueSarahEmailDraft } from "./crm-email-rail";

export type SarahFollowUpStatus =
  | "scheduled"
  | "queued_for_approval"
  | "closed"
  | "opted_out"
  | "suppressed";

export type SarahFollowUpJob = {
  id: string;
  status: SarahFollowUpStatus;
  prospectRef: string;
  threadId: string;
  toEmail: string;
  subject: string;
  reason: "quiet_after_quote" | "manual";
  dueAt: string;
  quoteRefs: string[];
  checkoutRefs: string[];
  draftId: string | null;
  createdAt: string;
  updatedAt: string;
  lastRunAt: string | null;
  suppressionReason: string | null;
};

export type SarahFollowUpQueue = {
  schema: "sarah.follow_up_queue.v1";
  jobs: Record<string, SarahFollowUpJob>;
};

const defaultQueuePath = join(
  /* turbopackIgnore: true */ process.cwd(),
  ".sarah",
  "follow-ups.json",
);

function queuePath() {
  const configured = process.env.SARAH_FOLLOW_UP_QUEUE_PATH;
  if (!configured) return defaultQueuePath;

  return join(
    /* turbopackIgnore: true */ process.cwd(),
    ".sarah",
    configured,
  );
}

async function readQueue(): Promise<SarahFollowUpQueue> {
  try {
    const raw = await readFile(queuePath(), "utf8");
    const parsed = JSON.parse(raw) as SarahFollowUpQueue;
    if (parsed.schema === "sarah.follow_up_queue.v1") return parsed;
  } catch {
    // Missing or invalid local queue files should not block the scheduler.
  }

  return { schema: "sarah.follow_up_queue.v1", jobs: {} };
}

let writeQueue = Promise.resolve();

async function writeFollowUpQueue(queue: SarahFollowUpQueue) {
  const path = queuePath();
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(queue, null, 2)}\n`);
}

function makeFollowUpDraft(job: SarahFollowUpJob) {
  return [
    "Hi, this is Sarah, OpenAgents' AI sales employee.",
    "",
    "I am following up on the quote we discussed. If the timing is still useful, I can help confirm fit, route a human owner for anything custom, or continue from the existing quote references.",
    "",
    job.quoteRefs.length > 0
      ? `Quote refs: ${job.quoteRefs.join(", ")}`
      : "No quote ref is attached, so I will not state a price without rechecking the deal rules.",
    job.checkoutRefs.length > 0
      ? `Checkout refs: ${job.checkoutRefs.join(", ")}`
      : null,
  ]
    .filter(Boolean)
    .join("\n");
}

export async function scheduleSarahFollowUp(input: {
  prospectRef: string;
  threadId: string;
  toEmail: string;
  subject: string;
  dueAt: string;
  reason?: SarahFollowUpJob["reason"];
  quoteRefs?: string[];
  checkoutRefs?: string[];
}) {
  const id = `sarah_follow_up.${crypto.randomUUID()}`;
  const now = new Date().toISOString();
  const job: SarahFollowUpJob = {
    id,
    status: "scheduled",
    prospectRef: input.prospectRef,
    threadId: input.threadId,
    toEmail: input.toEmail,
    subject: input.subject,
    reason: input.reason ?? "quiet_after_quote",
    dueAt: input.dueAt,
    quoteRefs: input.quoteRefs ?? [],
    checkoutRefs: input.checkoutRefs ?? [],
    draftId: null,
    createdAt: now,
    updatedAt: now,
    lastRunAt: null,
    suppressionReason: null,
  };

  writeQueue = writeQueue.then(async () => {
    const queue = await readQueue();
    queue.jobs[id] = job;
    await writeFollowUpQueue(queue);
  });
  await writeQueue;

  return job;
}

export async function suppressSarahFollowUps(input: {
  prospectRef: string;
  status: Extract<SarahFollowUpStatus, "closed" | "opted_out" | "suppressed">;
  reason: string;
}) {
  const updated: SarahFollowUpJob[] = [];
  writeQueue = writeQueue.then(async () => {
    const queue = await readQueue();
    const now = new Date().toISOString();
    for (const job of Object.values(queue.jobs)) {
      if (job.prospectRef !== input.prospectRef) continue;
      if (job.status !== "scheduled") continue;
      job.status = input.status;
      job.suppressionReason = input.reason;
      job.updatedAt = now;
      updated.push(job);
    }
    await writeFollowUpQueue(queue);
  });
  await writeQueue;

  return updated;
}

export async function processDueSarahFollowUps(options: {
  now?: Date;
  limit?: number;
}) {
  const now = options.now ?? new Date();
  const claimed: SarahFollowUpJob[] = [];
  const queued: Array<{ job: SarahFollowUpJob; draftId: string }> = [];

  writeQueue = writeQueue.then(async () => {
    const queue = await readQueue();
    const due = Object.values(queue.jobs)
      .filter(
        (job) => job.status === "scheduled" && Date.parse(job.dueAt) <= +now,
      )
      .sort((a, b) => a.dueAt.localeCompare(b.dueAt))
      .slice(0, options.limit ?? 25);

    for (const job of due) {
      job.lastRunAt = now.toISOString();
      claimed.push({ ...job });
      const draft = await enqueueSarahEmailDraft({
        continuationToken: `follow-up:${job.id}`,
        fromEmail: "sarah@openagents.com",
        inboundText: `Scheduled follow-up trigger: ${job.reason}`,
        messageId: null,
        proposedReply: makeFollowUpDraft(job),
        prospectRef: job.prospectRef,
        subject: job.subject.startsWith("Re: ")
          ? job.subject
          : `Re: ${job.subject}`,
        threadId: job.threadId,
        toEmail: job.toEmail,
      });
      job.status = "queued_for_approval";
      job.draftId = draft.id;
      job.updatedAt = now.toISOString();
      queued.push({ job: { ...job }, draftId: draft.id });
    }

    await writeFollowUpQueue(queue);
  });
  await writeQueue;

  return {
    ok: true,
    now: now.toISOString(),
    claimed,
    queued,
  };
}

export async function listSarahFollowUps() {
  const queue = await readQueue();
  return Object.values(queue.jobs).sort((a, b) =>
    a.dueAt.localeCompare(b.dueAt),
  );
}

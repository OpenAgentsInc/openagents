import { readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import {
  listSarahFollowUps,
  processDueSarahFollowUps,
  scheduleSarahFollowUp,
  suppressSarahFollowUps,
} from "../src/services/follow-up-scheduler.ts";
import { listSarahEmailDrafts } from "../src/services/crm-email-rail.ts";
import {
  listSarahSessionReceipts,
  recordSarahToolReceipt,
  recordSarahTranscriptTurn,
} from "../src/services/session-index.ts";

process.env.SARAH_FOLLOW_UP_QUEUE_PATH = "s13-smoke-follow-ups.json";
process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH = "s13-smoke-email-queue.json";
process.env.SARAH_SESSION_INDEX_PATH = "s13-smoke-session-index.json";

// The smoke must be idempotent: it uses fixed prospect/session ids, so state
// left under .sarah/ by a previous run accumulates transcript turns and refs
// and the receipt asserts fail on rerun. Start from a clean slate every time.
for (const stateFile of [
  process.env.SARAH_FOLLOW_UP_QUEUE_PATH,
  process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH,
  process.env.SARAH_SESSION_INDEX_PATH,
]) {
  await rm(join(process.cwd(), ".sarah", stateFile), { force: true });
}

function assert(condition, message, evidence) {
  if (!condition) {
    const error = new Error(message);
    error.evidence = evidence;
    throw error;
  }
}

async function readMaybe(path) {
  try {
    return JSON.parse(await readFile(path, "utf8"));
  } catch {
    return null;
  }
}

const now = new Date("2026-07-08T06:30:00.000Z");
const due = await scheduleSarahFollowUp({
  prospectRef: "s13-smoke-prospect",
  threadId: "email:s13-smoke-prospect",
  toEmail: "prospect@example.com",
  subject: "OpenAgents quote",
  dueAt: now.toISOString(),
  quoteRefs: ["sarah_quote.1234567890abcdef12345678"],
  checkoutRefs: ["sarah.checkout.smoke"],
});
const future = await scheduleSarahFollowUp({
  prospectRef: "s13-smoke-suppressed",
  threadId: "email:s13-smoke-suppressed",
  toEmail: "suppressed@example.com",
  subject: "Suppressed quote",
  dueAt: new Date(+now + 60_000).toISOString(),
  quoteRefs: ["sarah_quote.ffffffffffffffffffffffff"],
});
await suppressSarahFollowUps({
  prospectRef: future.prospectRef,
  status: "closed",
  reason: "smoke closed conversation",
});
const processed = await processDueSarahFollowUps({ now, limit: 10 });
const drafts = await listSarahEmailDrafts();
const followUps = await listSarahFollowUps();

assert(processed.queued.length === 1, "Expected exactly one due follow-up draft.", processed);
assert(
  drafts.some(
    (draft) =>
      draft.id === processed.queued[0].draftId &&
      draft.status === "pending_approval" &&
      draft.bodyWithDisclosure.includes("Sarah is an AI sales employee for OpenAgents.") &&
      draft.bodyWithDisclosure.includes("To opt out of Sarah email follow-ups:"),
  ),
  "Queued follow-up draft was missing approval/disclosure guarantees.",
  drafts,
);
assert(
  followUps.some((job) => job.id === future.id && job.status === "closed"),
  "Suppressed/closed follow-up did not stay suppressed.",
  followUps,
);

await recordSarahTranscriptTurn({
  prospectRef: "s13-smoke-prospect",
  sessionId: "s13-smoke-session",
  threadId: "web-s13-smoke",
  turn: {
    modality: "text",
    role: "user",
    sourceEvent: "smoke",
    text: "We are ready for the traced quote.",
  },
});
await recordSarahToolReceipt({
  prospectRef: "s13-smoke-prospect",
  sessionId: "s13-smoke-session",
  threadId: "web-s13-smoke",
  toolCallId: "s13-smoke-tool-call",
  toolName: "checkout_link_create",
  result: {
    ok: true,
    output: {
      mode: "dry_run",
      quoteRef: "sarah_quote.1234567890abcdef12345678",
      checkoutRef: "sarah.checkout.smoke",
      dealRuleRefs: ["rule.cap.transaction_usd_10000"],
      message: "Prepared a test-mode checkout quote; no money moved.",
    },
  },
});
const receipts = await listSarahSessionReceipts();
const receipt = receipts.find((item) => item.sessionId === "s13-smoke-session");

assert(receipt, "Expected a complete session receipt.", receipts);
assert(
  receipt.quoteRefs.includes("sarah_quote.1234567890abcdef12345678") &&
    receipt.checkoutRefs.includes("sarah.checkout.smoke") &&
    receipt.toolsUsed.some((tool) => tool.toolName === "checkout_link_create") &&
    receipt.transcriptTurns === 1,
  "Session receipt was missing transcript/tool/quote/checkout evidence.",
  receipt,
);

const evidence = {
  schema: "sarah.s13_smoke.v1",
  generatedAt: new Date().toISOString(),
  followUp: {
    scheduled: due.id,
    queuedDraftId: processed.queued[0].draftId,
    suppressed: future.id,
  },
  receipt,
  files: {
    followUps: await readMaybe(join(process.cwd(), ".sarah", "s13-smoke-follow-ups.json")),
    emailQueue: await readMaybe(join(process.cwd(), ".sarah", "s13-smoke-email-queue.json")),
    sessionIndex: await readMaybe(join(process.cwd(), ".sarah", "s13-smoke-session-index.json")),
  },
};

console.log(JSON.stringify(evidence, null, 2));

import { mkdir, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import {
  enqueueSarahEmailDraft,
  listSarahEmailDrafts,
  reviewSarahEmailDraft,
} from "../src/services/crm-email-rail.ts";
import {
  isSarahEmailSuppressed,
  listSarahEmailSuppressions,
  suppressSarahEmail,
} from "../src/services/crm-email-rail.ts";

process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH =
  "s8-email-suppression-queue.json";
process.env.SARAH_EMAIL_SUPPRESSION_LIST_PATH =
  "s8-email-suppression-list.json";
process.env.SARAH_PUBLIC_BASE_URL = "https://openagents.com/sarah";
process.env.SARAH_EMAIL_SEND_LIVE = "0";

await rm(join(process.cwd(), ".sarah", process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH), {
  force: true,
});
await rm(join(process.cwd(), ".sarah", process.env.SARAH_EMAIL_SUPPRESSION_LIST_PATH), {
  force: true,
});

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const normal = await enqueueSarahEmailDraft({
  continuationToken: "email:normal@example.com:s8-suppression",
  fromEmail: "normal@example.com",
  inboundText: "Please follow up with me.",
  messageId: "s8-normal-message",
  proposedReply: "I'm Sarah, OpenAgents' AI sales employee. Happy to help.",
  prospectRef: "email:normal@example.com",
  subject: "OpenAgents follow up",
  threadId: "email:normal@example.com:s8-suppression",
  toEmail: "normal@example.com",
});
assert(normal.status === "pending_approval", "normal draft should wait for approval");
assert(
  normal.bodyWithDisclosure.includes("Sarah is an AI sales employee for OpenAgents."),
  "normal draft should carry AI disclosure",
);
assert(
  normal.bodyWithDisclosure.includes("To opt out of Sarah email follow-ups:"),
  "normal draft should carry opt-out footer",
);

const approved = await reviewSarahEmailDraft({
  decision: "approve",
  draftId: normal.id,
  note: "Smoke approval with live sending disarmed.",
  reviewerRef: "s8-smoke",
});
assert(
  approved?.status === "approved_pending_send",
  "unarmed approval should not send mail",
);

await suppressSarahEmail({
  email: "Suppressed@Example.com",
  reason: "unsubscribe",
  source: "s8-smoke",
});
const suppression = await isSarahEmailSuppressed("suppressed@example.com");
assert(suppression?.reason === "unsubscribe", "suppression should be durable");

const suppressed = await enqueueSarahEmailDraft({
  continuationToken: "email:suppressed@example.com:s8-suppression",
  fromEmail: "suppressed@example.com",
  inboundText: "Email me again.",
  messageId: "s8-suppressed-message",
  proposedReply: "I'm Sarah, OpenAgents' AI sales employee. Following up.",
  prospectRef: "email:suppressed@example.com",
  subject: "Suppressed follow up",
  threadId: "email:suppressed@example.com:s8-suppression",
  toEmail: "suppressed@example.com",
});
assert(suppressed.status === "suppressed", "suppressed draft should not queue for approval");

const reviewedSuppressed = await reviewSarahEmailDraft({
  decision: "approve",
  draftId: suppressed.id,
  note: "Attempted approval should remain suppressed.",
  reviewerRef: "s8-smoke",
});
assert(
  reviewedSuppressed?.status === "suppressed",
  "suppressed draft should not become approved",
);

const evidence = {
  schema: "sarah.email_suppression_smoke.v1",
  generatedAt: new Date().toISOString(),
  status: "passed",
  adapterCompatibility: {
    blockedPackage: "@resend/chat-sdk-adapter@0.2.2",
    reason:
      "Adapter still pulls Chat SDK chat@4.33.0 with ai@^6.0.182 peer dependency; Sarah remains pinned to AI SDK 7 canary for realtime.",
  },
  normalApproval: {
    draftId: normal.id,
    status: approved?.status,
    sendMode: "SARAH_EMAIL_SEND_LIVE=0",
  },
  suppression: {
    email: suppression?.email,
    reason: suppression?.reason,
    source: suppression?.source,
  },
  suppressedDraft: {
    draftId: suppressed.id,
    status: reviewedSuppressed?.status,
  },
  drafts: await listSarahEmailDrafts(),
  suppressions: await listSarahEmailSuppressions(),
};

const outPath = join(
  process.cwd(),
  "docs",
  "evidence",
  "2026-07-08-s8-email-suppression-and-send-gate.json",
);
await mkdir(dirname(outPath), { recursive: true });
await writeFile(outPath, `${JSON.stringify(evidence, null, 2)}\n`);

console.log(JSON.stringify(evidence, null, 2));

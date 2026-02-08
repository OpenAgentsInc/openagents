/**
 * Wire transcript fixtures (V1).
 *
 * Source of truth (raw JSONL):
 * - `docs/autopilot/fixtures/dse-kitchen-sink.stream.v1.jsonl`
 * - `docs/autopilot/fixtures/autopilot-gmail-review.stream.v1.jsonl`
 *
 * Why duplicated here:
 * - Storybook runs in both Vite (browser) and Wrangler (worker SSR) modes.
 * - Worker bundling cannot reliably import `.jsonl`, and runtime filesystem reads are not isomorphic.
 * - Keeping these fixtures importable as plain TS ensures Storybook + worker tests consume the exact same shapes.
 */

export type WireTranscriptEvent = {
  readonly seq: number
  readonly part: unknown
}

export const dseKitchenSinkStreamV1: ReadonlyArray<WireTranscriptEvent> = [
  {
    seq: 0,
    part: {
      type: "dse.signature",
      v: 1,
      id: "dsepart_sig_ks_1",
      state: "start",
      tsMs: 1700000100000,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      compiled_id: "c_example_router_v1",
    },
  },
  {
    seq: 1,
    part: {
      type: "dse.signature",
      v: 1,
      id: "dsepart_sig_ks_1",
      state: "ok",
      tsMs: 1700000100100,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      compiled_id: "c_example_router_v1",
      timing: { durationMs: 100 },
      budget: {
        limits: { maxTimeMs: 2500, maxLmCalls: 1, maxOutputChars: 8000 },
        usage: { elapsedMs: 100, lmCalls: 1, outputChars: 420 },
      },
      receiptId: "rcpt_ks_sig_1",
    },
  },
  {
    seq: 2,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_ks_1",
      state: "start",
      tsMs: 1700000100200,
      toolName: "gmail.searchThreads",
      toolCallId: "toolcall_ks_1",
      input: { query: "newer_than:2d", maxResults: 25 },
    },
  },
  {
    seq: 3,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_ks_1",
      state: "error",
      tsMs: 1700000100300,
      toolName: "gmail.searchThreads",
      toolCallId: "toolcall_ks_1",
      errorText: "Timeout after 2000ms",
    },
  },
  {
    seq: 4,
    part: {
      type: "dse.budget_exceeded",
      v: 1,
      id: "dsepart_budget_ks_1",
      state: "error",
      tsMs: 1700000100400,
      message: "Stopped after exceeding maxLmCalls=1",
      budget: { limits: { maxLmCalls: 1 }, usage: { elapsedMs: 430, lmCalls: 2, outputChars: 0 } },
    },
  },
  {
    seq: 5,
    part: {
      type: "dse.compile",
      v: 1,
      id: "dsepart_compile_ks_1",
      state: "start",
      tsMs: 1700000101000,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      jobHash: "job_9e13_example",
      candidates: 24,
    },
  },
  {
    seq: 6,
    part: {
      type: "dse.compile",
      v: 1,
      id: "dsepart_compile_ks_1",
      state: "ok",
      tsMs: 1700000105000,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      jobHash: "job_9e13_example",
      best: { compiled_id: "c_new_example", reward: 0.71 },
      candidates: 24,
      reportId: "compile_report_ks_1",
    },
  },
  {
    seq: 7,
    part: {
      type: "dse.promote",
      v: 1,
      id: "dsepart_promote_ks_1",
      state: "ok",
      tsMs: 1700000105500,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      from: "c_example_router_v1",
      to: "c_new_example",
      reason: "compile job job_9e13_example improved reward 0.59 -> 0.71",
    },
  },
  {
    seq: 8,
    part: {
      type: "dse.rollback",
      v: 1,
      id: "dsepart_rollback_ks_1",
      state: "ok",
      tsMs: 1700000106000,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      from: "c_new_example",
      to: "c_example_router_v1",
      reason: "manual rollback after canary regression",
    },
  },
  { seq: 9, part: { type: "text-start", id: "t1" } },
  { seq: 10, part: { type: "text-delta", id: "t1", delta: "(kitchen-sink fixture)\\n" } },
  { seq: 11, part: { type: "text-end", id: "t1" } },
  { seq: 12, part: { type: "finish", reason: "stop", usage: { inputTokens: 1, outputTokens: 1, totalTokens: 2 } } },
] as const

export const autopilotGmailReviewStreamV1: ReadonlyArray<WireTranscriptEvent> = [
  {
    seq: 0,
    part: {
      type: "dse.signature",
      v: 1,
      id: "dsepart_sig_1",
      state: "start",
      tsMs: 1700000000000,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      compiled_id: "c_example_router_v1",
    },
  },
  {
    seq: 1,
    part: {
      type: "dse.signature",
      v: 1,
      id: "dsepart_sig_1",
      state: "ok",
      tsMs: 1700000000042,
      signatureId: "@openagents/autopilot/blueprint/SelectTool.v1",
      compiled_id: "c_example_router_v1",
      timing: { durationMs: 42 },
      budget: {
        limits: { maxTimeMs: 2500, maxLmCalls: 1, maxOutputChars: 8000 },
        usage: { elapsedMs: 42, lmCalls: 1, outputChars: 512 },
      },
      receiptId: "rcpt_select_tool_1",
      outputPreview: { toolName: "gmail.connect", reason: "User asked to review recent Gmail activity" },
    },
  },
  {
    seq: 2,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_1",
      state: "start",
      tsMs: 1700000000100,
      toolName: "gmail.connect",
      toolCallId: "toolcall_gmail_connect_1",
      input: { scopes: ["gmail.readonly"], account: "primary" },
    },
  },
  {
    seq: 3,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_1",
      state: "ok",
      tsMs: 1700000000300,
      toolName: "gmail.connect",
      toolCallId: "toolcall_gmail_connect_1",
      timing: { durationMs: 200 },
      output: { connected: true, account: "primary" },
    },
  },
  { seq: 4, part: { type: "text-start", id: "t1" } },
  {
    seq: 5,
    part: {
      type: "text-delta",
      id: "t1",
      delta: "I'll review your recent Gmail threads. One moment.\\n\\n",
    },
  },
  {
    seq: 6,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_2",
      state: "start",
      tsMs: 1700000000400,
      toolName: "gmail.searchThreads",
      toolCallId: "toolcall_gmail_search_1",
      input: { query: "newer_than:2d -category:promotions", maxResults: 3 },
    },
  },
  {
    seq: 7,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_2",
      state: "ok",
      tsMs: 1700000000600,
      toolName: "gmail.searchThreads",
      toolCallId: "toolcall_gmail_search_1",
      timing: { durationMs: 200 },
      output: { threadIds: ["th_1", "th_2", "th_3"] },
    },
  },
  {
    seq: 8,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_3",
      state: "start",
      tsMs: 1700000000700,
      toolName: "gmail.getThread",
      toolCallId: "toolcall_gmail_get_1",
      input: { threadId: "th_1" },
    },
  },
  {
    seq: 9,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_3",
      state: "ok",
      tsMs: 1700000000800,
      toolName: "gmail.getThread",
      toolCallId: "toolcall_gmail_get_1",
      timing: { durationMs: 100 },
      output: {
        threadId: "th_1",
        subject: "Project update",
        from: "contact_1",
        receivedAt: "2026-02-08T10:12:00Z",
        snippet: "Ship date moved to Friday; needs your approval.",
      },
    },
  },
  {
    seq: 10,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_4",
      state: "start",
      tsMs: 1700000000900,
      toolName: "gmail.getThread",
      toolCallId: "toolcall_gmail_get_2",
      input: { threadId: "th_2" },
    },
  },
  {
    seq: 11,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_4",
      state: "ok",
      tsMs: 1700000001000,
      toolName: "gmail.getThread",
      toolCallId: "toolcall_gmail_get_2",
      timing: { durationMs: 100 },
      output: {
        threadId: "th_2",
        subject: "Invoice requires attention",
        from: "contact_2",
        receivedAt: "2026-02-08T09:40:00Z",
        snippet: "Payment link expired; please confirm address details.",
      },
    },
  },
  {
    seq: 12,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_5",
      state: "start",
      tsMs: 1700000001100,
      toolName: "gmail.getThread",
      toolCallId: "toolcall_gmail_get_3",
      input: { threadId: "th_3" },
    },
  },
  {
    seq: 13,
    part: {
      type: "dse.tool",
      v: 1,
      id: "dsepart_tool_5",
      state: "ok",
      tsMs: 1700000001220,
      toolName: "gmail.getThread",
      toolCallId: "toolcall_gmail_get_3",
      timing: { durationMs: 120 },
      output: {
        threadId: "th_3",
        subject: "Meeting agenda",
        from: "contact_3",
        receivedAt: "2026-02-08T08:15:00Z",
        snippet: "Agenda attached; please add any additional topics.",
      },
    },
  },
  {
    seq: 14,
    part: {
      type: "dse.signature",
      v: 1,
      id: "dsepart_sig_2",
      state: "start",
      tsMs: 1700000001300,
      signatureId: "@openagents/autopilot/gmail/SummarizeThreads.v1",
      compiled_id: "c_example_summarize_v1",
    },
  },
  {
    seq: 15,
    part: {
      type: "dse.signature",
      v: 1,
      id: "dsepart_sig_2",
      state: "ok",
      tsMs: 1700000001500,
      signatureId: "@openagents/autopilot/gmail/SummarizeThreads.v1",
      compiled_id: "c_example_summarize_v1",
      timing: { durationMs: 200 },
      budget: {
        limits: { maxTimeMs: 3000, maxLmCalls: 1, maxOutputChars: 8000 },
        usage: { elapsedMs: 200, lmCalls: 1, outputChars: 1320 },
      },
      receiptId: "rcpt_summarize_threads_1",
      outputPreview: {
        items: [
          { threadId: "th_1", action: "Approve the Friday ship date change (reply needed)." },
          { threadId: "th_2", action: "Confirm address details to re-issue the invoice/payment link." },
          { threadId: "th_3", action: "Add agenda topics (optional) before the meeting." },
        ],
      },
    },
  },
  {
    seq: 16,
    part: {
      type: "text-delta",
      id: "t1",
      delta:
        "Here are the top 3 things from the last 48 hours:\\n\\n1) **Project update**: ship date moved to Friday; they need your approval.\\n2) **Invoice requires attention**: payment link expired; they need address confirmation to re-issue.\\n3) **Meeting agenda**: you can add topics before the meeting.\\n\\nWant me to draft replies for (1) and (2)?\\n",
    },
  },
  { seq: 17, part: { type: "text-end", id: "t1" } },
  {
    seq: 18,
    part: { type: "finish", reason: "stop", usage: { inputTokens: 123, outputTokens: 247, totalTokens: 370 } },
  },
] as const


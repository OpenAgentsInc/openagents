import { makeFunctionReference } from "convex/server";

import type { AttentionShell, AggregateType, WorkShell, WorkTranscriptPage } from "./contracts";

export const attentionInboxQuery = makeFunctionReference<
  "query",
  { limit?: number },
  ReadonlyArray<AttentionShell>
>("workShells:attentionInbox");

export const workTranscriptQuery = makeFunctionReference<
  "query",
  { aggregateType: AggregateType; aggregateId: string; limit?: number },
  WorkTranscriptPage
>("workShells:listTranscript");

export const workShellQuery = makeFunctionReference<
  "query",
  { aggregateType: AggregateType; aggregateId: string },
  WorkShell | null
>("workShells:get");
